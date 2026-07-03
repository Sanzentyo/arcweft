//! Character manifest metadata helpers for LSP completion and hover.

use crate::profiles::LspProfile;
use arcweft_character::manifest::{CharacterManifest, CharacterPart, CharacterVariant};

/// Returns markdown-style hover text for a manifest-derived character token.
pub fn character_hover_markdown(profile: &LspProfile, word: &str) -> Option<String> {
    let character = word.strip_prefix('@').unwrap_or(word);
    if let Some(manifest) = profile.characters().get_by_str(character) {
        return Some(character_manifest_hover(manifest));
    }
    let local = word.strip_prefix('.')?;
    profile.characters().manifests().find_map(|manifest| {
        manifest
            .looks()
            .iter()
            .find(|look| look.id().as_str() == local)
            .map(|look| look_hover(manifest, look))
            .or_else(|| {
                manifest
                    .parts()
                    .iter()
                    .find(|part| part.id().as_str() == local)
                    .map(|part| part_hover(manifest, part))
            })
            .or_else(|| {
                manifest.parts().iter().find_map(|part| {
                    part.variants()
                        .iter()
                        .find(|variant| variant.id().as_str() == local)
                        .map(|variant| variant_hover(manifest, part, variant))
                })
            })
    })
}

fn look_hover(
    manifest: &CharacterManifest,
    look: &arcweft_character::manifest::CharacterLook,
) -> String {
    let selections = look
        .selections()
        .iter()
        .map(|selection| {
            let source = manifest
                .part(selection.part())
                .and_then(|part| part.variant(selection.variant()))
                .and_then(|variant| variant.source_layer())
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
        "character look `{}` for `{}`\n{}",
        look.id(),
        manifest.character(),
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
        "character part `{}` for `{}`\nz: {}\nvariants:\n{}",
        part.id(),
        manifest.character(),
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
        "character variant `{}.{}` for `{}`\nasset: {}\nrect: {},{} {}x{}\nblend: {:?}\nclipping: {}",
        part.id(),
        variant.id(),
        manifest.character(),
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
