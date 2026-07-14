//! Character manifest metadata helpers for LSP completion and hover.

use crate::profiles::LspProfile;
use arcweft_character::manifest::{
    CharacterLook, CharacterManifest, CharacterPart, CharacterVariant,
};
use arcweft_lang_sema::types::{CharacterNominalType, TypeKind};

/// Returns markdown-style hover text for a manifest-derived character token.
///
/// An expected structural nominal type wins when available. Without expected
/// type evidence, a member spelling shared by multiple manifests is reported as
/// ambiguous instead of returning metadata for whichever manifest happened to
/// be visited first.
pub fn character_hover_markdown(
    profile: &LspProfile,
    word: &str,
    expected: Option<&TypeKind>,
) -> Option<String> {
    let character = word.strip_prefix('@').unwrap_or(word);
    if let Some(manifest) = profile.characters().get_by_str(character) {
        return Some(character_manifest_hover(manifest));
    }
    let local = word.strip_prefix('.')?;

    if let Some(nominal) = expected.and_then(TypeKind::character_nominal) {
        return expected_member_hover(profile, local, nominal);
    }

    let matches = profile
        .characters()
        .manifests()
        .flat_map(|manifest| manifest_member_hovers(manifest, local))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => None,
        [(_, hover)] => Some(hover.clone()),
        _ => Some(ambiguous_member_hover(word, &matches)),
    }
}

fn expected_member_hover(
    profile: &LspProfile,
    local: &str,
    nominal: &CharacterNominalType,
) -> Option<String> {
    let manifest = profile
        .characters()
        .get_by_str(nominal.character().as_str())?;
    match nominal {
        CharacterNominalType::Look { .. } => manifest
            .looks()
            .iter()
            .find(|look| look.id().as_str() == local)
            .map(|look| look_hover(manifest, look)),
        CharacterNominalType::Part { .. } => manifest
            .parts()
            .iter()
            .find(|part| part.id().as_str() == local)
            .map(|part| part_hover(manifest, part)),
        CharacterNominalType::Variant { part, .. } => {
            manifest.part(part).and_then(|manifest_part| {
                manifest_part
                    .variants()
                    .iter()
                    .find(|variant| variant.id().as_str() == local)
                    .map(|variant| variant_hover(manifest, manifest_part, variant))
            })
        }
    }
}

fn manifest_member_hovers(manifest: &CharacterManifest, local: &str) -> Vec<(TypeKind, String)> {
    let mut matches = Vec::new();
    if let Some(look) = manifest
        .looks()
        .iter()
        .find(|look| look.id().as_str() == local)
    {
        matches.push((
            TypeKind::character_look(manifest.character().clone()),
            look_hover(manifest, look),
        ));
    }
    if let Some(part) = manifest
        .parts()
        .iter()
        .find(|part| part.id().as_str() == local)
    {
        matches.push((
            TypeKind::character_part(manifest.character().clone()),
            part_hover(manifest, part),
        ));
    }
    matches.extend(manifest.parts().iter().filter_map(|part| {
        part.variants()
            .iter()
            .find(|variant| variant.id().as_str() == local)
            .map(|variant| {
                (
                    TypeKind::character_variant(manifest.character().clone(), part.id().clone()),
                    variant_hover(manifest, part, variant),
                )
            })
    }));
    matches
}

fn ambiguous_member_hover(word: &str, matches: &[(TypeKind, String)]) -> String {
    let mut candidates = matches
        .iter()
        .map(|(ty, _)| format!("- `{}.{}`", ty.source_label(), word.trim_start_matches('.')))
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();
    format!(
        "ambiguous character member `{word}`\n\nCandidates:\n{}\n\nA typed use site resolves this member from its expected character nominal type.",
        candidates.join("\n")
    )
}

fn look_hover(manifest: &CharacterManifest, look: &CharacterLook) -> String {
    let selections = look
        .selections()
        .iter()
        .map(|selection| {
            let source = manifest
                .part(selection.part())
                .and_then(|part| part.variant(selection.variant()))
                .and_then(CharacterVariant::source_layer)
                .map(|source| {
                    format!(
                        " source PSD layer: {} / {} (#{})",
                        source.group(),
                        source.layer(),
                        source.index()
                    )
                })
                .unwrap_or_default();
            format!("- {} = {}{}", selection.part(), selection.variant(), source)
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "character look `{}` for `{}`\ntype: `{}`\n{}",
        look.id(),
        manifest.character(),
        TypeKind::character_look(manifest.character().clone()).source_label(),
        selections
    )
}

fn character_manifest_hover(manifest: &CharacterManifest) -> String {
    format!(
        "character `{}`\ncanvas: {}x{}\nanchor: {},{}\ndefault look: {}\nparts: {}\nlooks: {}",
        manifest.character(),
        manifest.canvas().width(),
        manifest.canvas().height(),
        manifest.anchor().x(),
        manifest.anchor().y(),
        manifest.default_look(),
        manifest.parts().len(),
        manifest.looks().len()
    )
}

fn part_hover(manifest: &CharacterManifest, part: &CharacterPart) -> String {
    format!(
        "character part `{}` for `{}`\ntype: `{}`\nz: {}\nvariants:\n{}",
        part.id(),
        manifest.character(),
        TypeKind::character_part(manifest.character().clone()).source_label(),
        part.z(),
        part.variants()
            .iter()
            .map(|variant| format!("- {}", variant.id()))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn variant_hover(
    manifest: &CharacterManifest,
    part: &CharacterPart,
    variant: &CharacterVariant,
) -> String {
    let rect = variant.rect();
    let mut lines = vec![format!(
        "character variant `{}.{}` for `{}`\ntype: `{}`\nasset: {}\nrect: {},{} {}x{}\nblend: {:?}\nclipping: {}",
        part.id(),
        variant.id(),
        manifest.character(),
        TypeKind::character_variant(manifest.character().clone(), part.id().clone()).source_label(),
        variant.asset().as_str(),
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
        variant.blend(),
        variant.clipping()
    )];
    if let Some(source) = variant.source_layer() {
        lines.push(format!(
            "source PSD layer: {} / {} (#{})",
            source.group(),
            source.layer(),
            source.index()
        ));
    }
    lines.join("\n")
}
