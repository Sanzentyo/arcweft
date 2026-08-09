//! Tooling projection of the semantic dialogue View model inventory.

use crate::documents::DocumentSnapshot;
use crate::profiles::LspProfile;
use arcweft_lang_hir::{
    item::HirItemKind,
    leaf::{HirPath, HirPathRoot, HirPathSegment},
};
use arcweft_lang_sema::dialogue_view::{DialogueViewProjection, STANDARD_DIALOGUE_VIEW_TYPE};
use arcweft_lang_sema::types::TypeKind;
use std::{collections::BTreeSet, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DialogueViewTypeMetadata {
    pub(crate) name: String,
}

impl DialogueViewTypeMetadata {
    pub(crate) fn fields() -> [(DialogueViewProjection, TypeKind); 6] {
        [
            DialogueViewProjection::CharacterDisplayName,
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
    let mut names = BTreeSet::from([STANDARD_DIALOGUE_VIEW_TYPE.to_owned()]);
    if let Some(document) = document {
        names.extend(
            profile
                .accepted_environment()
                .filter(|accepted| {
                    accepted
                        .project()
                        .sources()
                        .by_uri(document.uri())
                        .is_some_and(|source| {
                            Arc::ptr_eq(source.document(), document.source_document())
                        })
                })
                .into_iter()
                .flat_map(|accepted| {
                    accepted
                        .project()
                        .hir_project()
                        .view()
                        .items()
                        .filter_map(|item| {
                            let HirItemKind::Struct(declaration) = item.item().kind() else {
                                return None;
                            };
                            if !item
                                .item()
                                .prefix()
                                .attributes()
                                .iter()
                                .any(|attribute| is_dialogue_view_attribute(attribute.path()))
                            {
                                return None;
                            }
                            Some(declaration.name().resolved()?.as_str().to_owned())
                        })
                        .collect::<Vec<_>>()
                }),
        );
    }
    names
        .into_iter()
        .map(|name| DialogueViewTypeMetadata { name })
        .collect()
}

fn is_dialogue_view_attribute(path: &HirPath) -> bool {
    if path.root() != HirPathRoot::ImplicitCrate {
        return false;
    }
    matches!(
        path.segments(),
        [HirPathSegment::Identifier(name)] if name.as_str() == "dialogue_view"
    )
}

fn type_label(ty: &TypeKind) -> String {
    match ty {
        TypeKind::String => "String".to_owned(),
        TypeKind::Named(name) => name.clone(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use arcweft_lang_sema::dialogue_view::STANDARD_DIALOGUE_VIEW_TYPE;
    use arcweft_runtime_host::RuntimeHostRunnerKind;

    use super::{LspProfile, dialogue_view_types};

    #[test]
    fn standard_dialogue_view_uses_the_typed_language_identity_before_project_acceptance() {
        let profile = LspProfile::default_for_runner(RuntimeHostRunnerKind::Native);
        let types = dialogue_view_types(&profile, None);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].name, STANDARD_DIALOGUE_VIEW_TYPE);
    }
}
