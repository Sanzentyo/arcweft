//! Tooling projection of the semantic dialogue View model inventory.

use crate::documents::DocumentSnapshot;
use crate::profiles::LspProfile;
use arcweft_lang_sema::dialogue_view::DialogueViewProjection;
use arcweft_lang_sema::types::TypeKind;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DialogueViewTypeMetadata {
    pub(crate) name: String,
}

impl DialogueViewTypeMetadata {
    pub(crate) fn fields() -> [(DialogueViewProjection, TypeKind); 6] {
        [
            DialogueViewProjection::Speaker,
            DialogueViewProjection::Content,
            DialogueViewProjection::Occurrence,
            DialogueViewProjection::Stage,
            DialogueViewProjection::Reveal,
            DialogueViewProjection::PrimaryAction,
        ]
        .map(|projection| (projection, projection.value_type()))
    }

    pub(crate) fn declaration(&self) -> String {
        let fields = Self::fields()
            .map(|(projection, ty)| format!("    {}: {}", projection.field(), type_label(&ty)))
            .join("\n");
        format!(
            "#[dialogue_view]\npub struct {} {{\n{fields}\n}}",
            self.name
        )
    }
}

pub(crate) fn dialogue_view_types(
    profile: &LspProfile,
    document: Option<&DocumentSnapshot>,
) -> Vec<DialogueViewTypeMetadata> {
    let mut names = profile
        .typecheck_env()
        .dialogue_view_models()
        .models()
        .map(|model| model.type_name().to_owned())
        .collect::<BTreeSet<_>>();
    if let Some(document) = document {
        names.extend(
            profile
                .accepted_environment()
                .filter(|accepted| {
                    accepted
                        .project()
                        .sources()
                        .by_uri(document.uri())
                        .is_some_and(|source| source.document().text() == document.text())
                })
                .into_iter()
                .flat_map(|accepted| {
                    accepted
                        .project()
                        .typecheck()
                        .dialogue_view_models
                        .models()
                        .map(|model| model.type_name().to_owned())
                        .collect::<Vec<_>>()
                }),
        );
    }
    names
        .into_iter()
        .map(|name| DialogueViewTypeMetadata { name })
        .collect()
}

fn type_label(ty: &TypeKind) -> String {
    match ty {
        TypeKind::String => "String".to_owned(),
        TypeKind::Named(name) => name.clone(),
        other => format!("{other:?}"),
    }
}
