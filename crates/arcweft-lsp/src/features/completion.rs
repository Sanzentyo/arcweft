use crate::documents::DocumentSnapshot;
use crate::features::dialogue_view_metadata::{DialogueViewTypeMetadata, dialogue_view_types};
use crate::features::view_part_metadata::ViewPartMetadataIndex;
use crate::profiles::LspProfile;
use arcweft_character::manifest::{CharacterManifest, CharacterPart, CharacterVariant};
use arcweft_lang_sema::types::TypeKind;
use arcweft_verify_lsp::profile_completions;
use lsp_types::{CompletionItem, CompletionItemKind, Documentation, Position};
use std::collections::BTreeSet;

/// Computes completion items from resolved adapter, runtime-host, and character facts.
pub fn completions(
    profile: &LspProfile,
    document: Option<&DocumentSnapshot>,
) -> Vec<CompletionItem> {
    let mut items = profile_completions(&profile.context());
    items.extend(crate::features::entry_roles::callable_completions(profile));
    items.extend(character_metadata_completions(profile));
    items.extend(enum_variant_completions(profile));
    items.extend(dialogue_view_completions(profile, document));
    if let Some(document) = document {
        items.extend(crate::features::nominal_types::completions(
            profile, document,
        ));
    }
    dedup_completion_items(items)
}

/// Computes completions including position-sensitive authored View-part syntax.
pub fn completions_at(
    profile: &LspProfile,
    document: Option<&DocumentSnapshot>,
    position: Position,
) -> Vec<CompletionItem> {
    let mut items = completions(profile, document);
    if let Some(document) = document {
        let Ok(offset) = document
            .line_index()
            .try_byte_offset_from_position(position)
        else {
            return Vec::new();
        };
        if let Some(metadata) = ViewPartMetadataIndex::for_document(profile, document) {
            items.extend(metadata.completions(document.text(), offset));
        }
    }
    dedup_completion_items(items)
}

fn dialogue_view_completions(
    profile: &LspProfile,
    document: Option<&DocumentSnapshot>,
) -> Vec<CompletionItem> {
    dialogue_view_types(profile, document)
        .into_iter()
        .flat_map(|model| {
            let declaration = model.declaration();
            let mut items = vec![CompletionItem {
                label: model.name.clone(),
                kind: Some(CompletionItemKind::STRUCT),
                detail: Some(declaration.clone()),
                documentation: Some(Documentation::String(format!(
                    "Dialogue View input model.\n\n{declaration}"
                ))),
                ..CompletionItem::default()
            }];
            items.extend(DialogueViewTypeMetadata::fields().map(|(projection, ty)| {
                CompletionItem {
                    label: projection.field().to_owned(),
                    kind: Some(CompletionItemKind::FIELD),
                    detail: Some(format!(
                        "{}.{}: {}",
                        model.name,
                        projection.field(),
                        type_kind_label(&ty)
                    )),
                    documentation: Some(Documentation::String(format!(
                        "Runtime-supplied `{}` field of dialogue View model `{}`.",
                        projection.field(),
                        model.name
                    ))),
                    ..CompletionItem::default()
                }
            }));
            items
        })
        .collect()
}

fn character_metadata_completions(profile: &LspProfile) -> Vec<CompletionItem> {
    profile
        .characters()
        .manifests()
        .flat_map(|manifest| {
            let mut items = vec![character_completion(manifest)];
            items.extend(manifest.looks().iter().map(|look| CompletionItem {
                label: format!(".{}", look.id().as_str()),
                kind: Some(CompletionItemKind::ENUM_MEMBER),
                detail: Some(format!(
                    "{}.{}",
                    TypeKind::character_look(manifest.character().clone()).source_label(),
                    look.id().as_str()
                )),
                documentation: Some(Documentation::String(format!(
                    "Look `{}` for `{}`.\n{}",
                    look.id(),
                    manifest.character(),
                    look
                        .selections()
                        .iter()
                        .map(|selection| format!(
                            "- {} = {}",
                            selection.part(),
                            selection.variant()
                        ))
                        .collect::<Vec<_>>()
                        .join("\n")
                ))),
                filter_text: Some(format!(".{}", look.id().as_str())),
                insert_text: Some(format!(".{}", look.id().as_str())),
                ..CompletionItem::default()
            }));
            items.extend(
                manifest
                    .parts()
                    .iter()
                    .map(|part| part_completion(manifest, part)),
            );
            items.extend(manifest.parts().iter().flat_map(|part| {
                part.variants()
                    .iter()
                    .map(move |variant| variant_completion(manifest, part, variant))
            }));
            items
        })
        .collect()
}

fn character_completion(manifest: &CharacterManifest) -> CompletionItem {
    CompletionItem {
        label: format!("@{}", manifest.character().as_str()),
        kind: Some(CompletionItemKind::CLASS),
        detail: Some(".awchar character".to_owned()),
        documentation: Some(Documentation::String(format!(
            "Character package `{}`\ncanvas: {}x{}\nanchor: {},{}\ndefault look: {}",
            manifest.character(),
            manifest.canvas().width(),
            manifest.canvas().height(),
            manifest.anchor().x(),
            manifest.anchor().y(),
            manifest.default_look()
        ))),
        filter_text: Some(manifest.character().as_str().to_owned()),
        insert_text: Some(format!("@{}", manifest.character().as_str())),
        ..CompletionItem::default()
    }
}

fn part_completion(manifest: &CharacterManifest, part: &CharacterPart) -> CompletionItem {
    CompletionItem {
        label: format!(".{}", part.id().as_str()),
        kind: Some(CompletionItemKind::PROPERTY),
        detail: Some(format!(
            "{}.{}",
            TypeKind::character_part(manifest.character().clone()).source_label(),
            part.id().as_str()
        )),
        documentation: Some(Documentation::String(format!(
            "Part `{}` for `{}` with {} variant(s).",
            part.id(),
            manifest.character(),
            part.variants().len()
        ))),
        filter_text: Some(format!(".{}", part.id().as_str())),
        insert_text: Some(format!(".{}", part.id().as_str())),
        ..CompletionItem::default()
    }
}

fn variant_completion(
    manifest: &CharacterManifest,
    part: &CharacterPart,
    variant: &CharacterVariant,
) -> CompletionItem {
    CompletionItem {
        label: format!(".{}", variant.id().as_str()),
        kind: Some(CompletionItemKind::ENUM_MEMBER),
        detail: Some(format!(
            "{}.{}",
            TypeKind::character_variant(manifest.character().clone(), part.id().clone())
                .source_label(),
            variant.id().as_str()
        )),
        documentation: Some(Documentation::String(variant_documentation(part, variant))),
        filter_text: Some(format!(".{}", variant.id().as_str())),
        insert_text: Some(format!(".{}", variant.id().as_str())),
        ..CompletionItem::default()
    }
}

fn variant_documentation(part: &CharacterPart, variant: &CharacterVariant) -> String {
    let rect = variant.rect();
    let mut lines = vec![format!(
        "Variant `{}.{}`\nasset: {}\nrect: {},{} {}x{}\nz: {}",
        part.id(),
        variant.id(),
        variant.asset().as_str(),
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
        part.z()
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

fn enum_variant_completions(profile: &LspProfile) -> Vec<CompletionItem> {
    profile
        .typecheck_env()
        .enum_variant_sets()
        .into_iter()
        .flat_map(|(ty, variants)| {
            let ty_label = type_kind_label(&ty);
            variants.into_iter().map(move |variant| {
                let label = format!(".{variant}");
                let qualified = format!("{ty_label}.{variant}");
                CompletionItem {
                    label: label.clone(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    detail: Some(qualified.clone()),
                    documentation: Some(Documentation::String(format!(
                        "Short enum variant for `{qualified}` when `{ty_label}` is expected."
                    ))),
                    filter_text: Some(label.clone()),
                    insert_text: Some(label),
                    ..CompletionItem::default()
                }
            })
        })
        .collect()
}

fn dedup_completion_items(items: Vec<CompletionItem>) -> Vec<CompletionItem> {
    let mut seen = BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert((item.label.clone(), item.detail.clone())))
        .collect()
}

fn type_kind_label(ty: &TypeKind) -> String {
    ty.source_label()
}
