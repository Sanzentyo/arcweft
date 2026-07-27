//! Semantic inventory for authored dialogue View input records.
//!
//! Dialogue presentation is selected by a View reference. The View receives a
//! nominal record whose `#[dialogue_view]` role identifies the projections the
//! runtime supplies. Lowering consults this inventory instead of recognizing a
//! particular type-name spelling.

use crate::{nominal::ProjectNominalShapeCatalog, types::TypeKind};
use arcweft_lang_hir::model::{HirModule, HirTopLevelDecl};
use arcweft_lang_hir::symbol::ProjectSymbolTable;
use arcweft_lang_syntax::ast::common::Visibility;
use arcweft_lang_syntax::ast::items::StructItem;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Attribute that assigns the dialogue View input role to a nominal record.
pub const DIALOGUE_VIEW_ATTRIBUTE: &str = "dialogue_view";

/// Canonical standard-prelude dialogue View input record.
pub const STANDARD_DIALOGUE_VIEW_TYPE: &str = "DialogueView";

/// Reserved engine-owned dialogue View resource.
pub const STANDARD_DIALOGUE_VIEW_RESOURCE: &str = "std.view.dialogue";

/// Canonical rich dialogue content value exposed by the standard prelude.
pub const DIALOGUE_CONTENT_TYPE: &str = "DialogueContent";

/// Stable identity of one active dialogue occurrence.
pub const DIALOGUE_OCCURRENCE_ID_TYPE: &str = "DialogueOccurrenceId";

/// Current lifecycle stage of one active dialogue occurrence.
pub const DIALOGUE_STAGE_TYPE: &str = "DialogueStage";

/// Current typed reveal state exposed to the authored View.
pub const DIALOGUE_REVEAL_TYPE: &str = "DialogueReveal";

/// Typed primary interaction supplied by the dialogue runtime.
pub const DIALOGUE_ACTION_TYPE: &str = "DialogueAction";

/// Runtime-supplied projection of one dialogue View input record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DialogueViewProjection {
    Speaker,
    Content,
    Occurrence,
    Stage,
    Reveal,
    PrimaryAction,
}

impl DialogueViewProjection {
    /// Canonical field that exposes this projection.
    pub const fn field(self) -> &'static str {
        match self {
            Self::Speaker => "speaker",
            Self::Content => "content",
            Self::Occurrence => "occurrence",
            Self::Stage => "stage",
            Self::Reveal => "reveal",
            Self::PrimaryAction => "primary_action",
        }
    }

    /// Resolves a field through the closed dialogue View projection contract.
    pub fn from_field(field: &str) -> Option<Self> {
        match field {
            "speaker" => Some(Self::Speaker),
            "content" => Some(Self::Content),
            "occurrence" => Some(Self::Occurrence),
            "stage" => Some(Self::Stage),
            "reveal" => Some(Self::Reveal),
            "primary_action" => Some(Self::PrimaryAction),
            _ => None,
        }
    }

    /// Semantic value type required for this projection.
    pub fn value_type(self) -> TypeKind {
        match self {
            Self::Speaker => TypeKind::String,
            Self::Content => TypeKind::Named(DIALOGUE_CONTENT_TYPE.to_owned()),
            Self::Occurrence => TypeKind::Named(DIALOGUE_OCCURRENCE_ID_TYPE.to_owned()),
            Self::Stage => TypeKind::Named(DIALOGUE_STAGE_TYPE.to_owned()),
            Self::Reveal => TypeKind::Named(DIALOGUE_REVEAL_TYPE.to_owned()),
            Self::PrimaryAction => TypeKind::Named(DIALOGUE_ACTION_TYPE.to_owned()),
        }
    }
}

/// One nominal record registered for the dialogue View input role.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueViewModel {
    type_name: String,
}

impl DialogueViewModel {
    fn new(type_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
        }
    }

    /// Nominal source type carrying this role.
    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    /// Resolves a field on this role-bearing record.
    pub fn projection(&self, field: &str) -> Option<DialogueViewProjection> {
        DialogueViewProjection::from_field(field)
    }
}

/// Closed inventory of standard and source-declared dialogue View models.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueViewModelRegistry {
    models: BTreeMap<String, DialogueViewModel>,
}

impl Default for DialogueViewModelRegistry {
    fn default() -> Self {
        Self::empty()
    }
}

impl DialogueViewModelRegistry {
    /// Creates an inventory without standard or source-declared models.
    pub fn empty() -> Self {
        Self {
            models: BTreeMap::new(),
        }
    }

    /// Creates the standard-prelude inventory.
    pub fn standard() -> Self {
        let model = DialogueViewModel::new(STANDARD_DIALOGUE_VIEW_TYPE);
        Self {
            models: BTreeMap::from([(STANDARD_DIALOGUE_VIEW_TYPE.to_owned(), model)]),
        }
    }

    pub(crate) fn from_project_shapes(
        module: &HirModule,
        symbols: &ProjectSymbolTable,
        shapes: &ProjectNominalShapeCatalog,
    ) -> Result<Self, Vec<DialogueViewModelError>> {
        let mut registry = Self::standard();
        let mut errors = Vec::new();
        for (module_path, declaration) in module.declarations_with_modules() {
            let HirTopLevelDecl::Struct(item) = declaration else {
                continue;
            };
            if item
                .attrs()
                .iter()
                .any(|attribute| attribute.name() == DIALOGUE_VIEW_ATTRIBUTE)
            {
                let declaration = module
                    .project_source_span(module_path, *item.name_range())
                    .and_then(|source| {
                        symbols
                            .nominal_symbols()
                            .find(|declaration| declaration.source().name() == &source)
                    });
                let mut field_type = |_: &StructItem, field_name: &str| {
                    declaration
                        .and_then(|declaration| shapes.struct_fields(declaration.id()))
                        .and_then(|fields| fields.get(field_name))
                        .cloned()
                };
                match DialogueViewModel::try_from_struct(item, &mut field_type) {
                    Ok(model) => {
                        registry.models.insert(model.type_name.clone(), model);
                    }
                    Err(error) => errors.push(error),
                }
            }
        }
        if errors.is_empty() {
            Ok(registry)
        } else {
            Err(errors)
        }
    }

    /// Resolves the semantic role of one nominal source type.
    pub fn model(&self, type_name: &str) -> Option<&DialogueViewModel> {
        self.models.get(type_name)
    }

    /// Iterates role-bearing records in deterministic nominal-name order.
    pub fn models(&self) -> impl Iterator<Item = &DialogueViewModel> {
        self.models.values()
    }
}

impl DialogueViewModel {
    fn try_from_struct(
        item: &StructItem,
        field_type: &mut impl FnMut(&StructItem, &str) -> Option<TypeKind>,
    ) -> Result<Self, DialogueViewModelError> {
        let attribute = item
            .attrs()
            .iter()
            .find(|attribute| attribute.name() == DIALOGUE_VIEW_ATTRIBUTE)
            .expect("dialogue View structs are filtered by their role attribute");
        if attribute.args().is_some_and(|args| !args.trim().is_empty()) {
            return Err(DialogueViewModelError::AttributeArguments {
                type_name: item.name().to_owned(),
            });
        }
        if item.visibility() != Some(Visibility::Public) {
            return Err(DialogueViewModelError::NotPublic {
                type_name: item.name().to_owned(),
            });
        }
        let mut seen_fields = BTreeSet::new();
        for field in item.fields() {
            if DialogueViewProjection::from_field(field.name()).is_none() {
                return Err(DialogueViewModelError::UnexpectedField {
                    type_name: item.name().to_owned(),
                    field: field.name().to_owned(),
                });
            }
            if !seen_fields.insert(field.name()) {
                return Err(DialogueViewModelError::DuplicateField {
                    type_name: item.name().to_owned(),
                    field: field.name().to_owned(),
                });
            }
        }
        for projection in [
            DialogueViewProjection::Speaker,
            DialogueViewProjection::Content,
            DialogueViewProjection::Occurrence,
            DialogueViewProjection::Stage,
            DialogueViewProjection::Reveal,
            DialogueViewProjection::PrimaryAction,
        ] {
            let Some(_) = item
                .fields()
                .iter()
                .find(|field| field.name() == projection.field())
            else {
                return Err(DialogueViewModelError::MissingField {
                    type_name: item.name().to_owned(),
                    field: projection.field(),
                });
            };
            let actual = field_type(item, projection.field()).ok_or_else(|| {
                DialogueViewModelError::MissingFieldTypeEvidence {
                    type_name: item.name().to_owned(),
                    field: projection.field(),
                }
            })?;
            let expected = projection.value_type();
            if actual != expected {
                return Err(DialogueViewModelError::FieldType {
                    type_name: item.name().to_owned(),
                    field: projection.field(),
                    expected: Box::new(expected),
                    actual: Box::new(actual),
                });
            }
        }
        Ok(Self::new(item.name()))
    }
}

/// Invalid source declaration of the `#[dialogue_view]` semantic role.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DialogueViewModelError {
    #[error("dialogue View model `{type_name}` must use `#[dialogue_view]` without arguments")]
    AttributeArguments { type_name: String },
    #[error("dialogue View model `{type_name}` must be public")]
    NotPublic { type_name: String },
    #[error("dialogue View model `{type_name}` is missing required field `{field}`")]
    MissingField {
        type_name: String,
        field: &'static str,
    },
    #[error("dialogue View model `{type_name}` has unsupported field `{field}`")]
    UnexpectedField { type_name: String, field: String },
    #[error("dialogue View model `{type_name}` repeats field `{field}`")]
    DuplicateField { type_name: String, field: String },
    #[error(
        "dialogue View model `{type_name}` field `{field}` has no accepted type-resolution evidence"
    )]
    MissingFieldTypeEvidence {
        type_name: String,
        field: &'static str,
    },
    #[error(
        "dialogue View model `{type_name}` field `{field}` must have type {expected:?}, found {actual:?}"
    )]
    FieldType {
        type_name: String,
        field: &'static str,
        expected: Box<TypeKind>,
        actual: Box<TypeKind>,
    },
}

#[cfg(test)]
mod tests {
    use super::DialogueViewProjection;
    use crate::{
        checker::{TypeCheckReport, analyze_registered_project_types, typecheck_hir},
        env::TypeCheckEnv,
        registration::ProjectRegistrationFacts,
        test_support::character_project::{register, root_project_source},
    };
    use arcweft_lang_hir::lower::lower_document_to_hir;
    use arcweft_lang_syntax::parser::parse_source;

    fn registered_report(source: &str) -> TypeCheckReport {
        let (document, project, world) = root_project_source("dialogue-view-model", source);
        let facts = ProjectRegistrationFacts::try_new(
            world,
            vec![document],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .expect("dialogue View registration facts");
        let registered = register(&project, &facts, TypeCheckEnv::standard(), None)
            .expect("dialogue View semantic world");
        analyze_registered_project_types(&project.linked_module(), &registered)
    }

    #[test]
    fn attributed_public_record_registers_the_dialogue_view_role() {
        let source = "#[dialogue_view]\npub struct StoryDialogue {\n speaker: String\n content: DialogueContent\n occurrence: DialogueOccurrenceId\n stage: DialogueStage\n reveal: DialogueReveal\n primary_action: DialogueAction\n}\n";
        let parsed = parse_source(source);
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("lower dialogue View record");
        let report = registered_report(source);
        assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);
        let registry = report.dialogue_view_models;
        let model = registry.model("StoryDialogue").expect("registered model");
        assert_eq!(
            model.projection("content"),
            Some(DialogueViewProjection::Content)
        );
    }

    #[test]
    fn attributed_record_must_satisfy_the_closed_field_contract() {
        let source = "#[dialogue_view]\npub struct BrokenDialogue {\n speaker: String\n content: String\n occurrence: DialogueOccurrenceId\n stage: DialogueStage\n reveal: DialogueReveal\n primary_action: DialogueAction\n}\n";
        let parsed = parse_source(source);
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("lower invalid dialogue View record");
        let report = registered_report(source);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|error| error.to_string().contains("content")),
            "{:?}",
            report.diagnostics
        );
    }

    #[test]
    fn attributed_record_rejects_fields_outside_the_closed_runtime_projection() {
        let source = "#[dialogue_view]\npub struct ExtendedDialogue {\n speaker: String\n content: DialogueContent\n occurrence: DialogueOccurrenceId\n stage: DialogueStage\n reveal: DialogueReveal\n primary_action: DialogueAction\n mood: String\n}\n";
        let parsed = parse_source(source);
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("lower invalid dialogue View record");
        let report = registered_report(source);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|error| error.to_string().contains("mood")),
            "{:?}",
            report.diagnostics
        );
    }

    #[test]
    fn standard_model_types_text_and_primary_action_projections() {
        let parsed = parse_source(
            r#"
pub view DialoguePanel(dialogue: DialogueView) {
    Column {
        Text(dialogue.speaker)
        RichText(dialogue.content)
        Button("Continue").on_click { dialogue.primary_action }
    }
}
"#,
        );
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("lower dialogue View");
        typecheck_hir(&hir, &TypeCheckEnv::standard()).expect("typed dialogue projections");
    }

    #[test]
    fn dialogue_content_requires_the_rich_text_surface() {
        let parsed =
            parse_source("pub view Broken(dialogue: DialogueView) {\n Text(dialogue.content)\n}\n");
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("lower invalid dialogue View");
        let errors = typecheck_hir(&hir, &TypeCheckEnv::standard())
            .expect_err("plain Text must reject dialogue rich content");
        assert!(
            errors
                .iter()
                .any(|error| error.to_string().contains("must be emitted by `RichText"))
        );
    }

    #[test]
    fn authored_view_cannot_redeclare_the_standard_dialogue_resource() {
        let parsed =
            parse_source("pub view @std.view.dialogue Dialogue() {\n Text(\"reserved\")\n}\n");
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
            .expect("lower reserved View fixture");
        let errors = typecheck_hir(&hir, &TypeCheckEnv::standard())
            .expect_err("reserved standard View id must be rejected");
        assert!(
            errors
                .iter()
                .any(|error| { error.to_string().contains("std.view.dialogue` is reserved") })
        );
    }
}
