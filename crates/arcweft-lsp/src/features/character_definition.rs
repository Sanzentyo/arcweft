//! Character-aware definition dispatch over one accepted semantic generation.

use std::sync::Arc;

use arcweft_compiler::project::CompiledProject;
use arcweft_lang_hir::module::HirModule;
use arcweft_lang_sema::character_definition::{
    CharacterDefinitionIntegrityError, CharacterDefinitionQueryResult,
    CharacterDefinitionRequestBudget, CharacterDefinitionResourceError, CharacterDefinitionStale,
    CharacterDefinitionWorkKind, CharacterReferenceInput, CharacterReferenceInventory,
    CharacterReferenceInventoryError, collect_character_references, query_character_definition,
};
use arcweft_source::{SourceDocument, SourceDocumentIdentity};
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{GotoDefinitionResponse, LocationLink};
use thiserror::Error;

use crate::{
    documents::{DocumentSnapshot, DocumentStore},
    positions::CheckedPositionError,
    profiles::{
        LspProfile,
        caches::{CharacterDefinitionCacheKey, CharacterReferenceCacheKey},
        state::{AcceptedEnvironmentGeneration, AcceptedProfileEnvironment, AcceptedProfileKey},
    },
};

/// Whether the exact cursor belongs to the character-definition feature.
#[derive(Debug)]
pub(crate) enum CharacterDefinitionDispatch {
    NotCharacter,
    Character(Option<GotoDefinitionResponse>),
}

/// Accepted-generation mismatch detected before protocol output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptedCharacterDefinitionStale {
    Core(CharacterDefinitionStale),
    Generation {
        expected: AcceptedEnvironmentGeneration,
        actual: Option<AcceptedEnvironmentGeneration>,
    },
    DocumentVersion {
        uri: String,
        expected: i32,
        actual: Option<i32>,
    },
    Profile {
        expected: AcceptedProfileKey,
        actual: Option<AcceptedProfileKey>,
    },
    OverlaySet,
    TargetDocument {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    MultiRoot {
        expected: AcceptedProfileKey,
        actual: AcceptedProfileKey,
    },
}

/// Request failure categories with stable protocol-code mapping.
#[derive(Debug, Error)]
pub enum CharacterDefinitionRequestError {
    #[error(transparent)]
    InvalidPosition(#[from] CheckedPositionError),
    #[error("accepted character definition input changed during the request")]
    Stale(Box<AcceptedCharacterDefinitionStale>),
    #[error("character definition exhausted its bounded request resources")]
    Resource(CharacterDefinitionResourceError),
    #[error("accepted character definition state violated an integrity invariant")]
    Integrity(CharacterDefinitionIntegrityError),
}

impl CharacterDefinitionRequestError {
    fn stale(stale: AcceptedCharacterDefinitionStale) -> Self {
        Self::Stale(Box::new(stale))
    }

    fn admitted_stale(
        budget: &mut CharacterDefinitionRequestBudget,
        stale: AcceptedCharacterDefinitionStale,
    ) -> Self {
        budget
            .charge(CharacterDefinitionWorkKind::AdmittedErrorCandidate)
            .map_or_else(Self::Resource, |()| Self::stale(stale))
    }

    fn admitted_integrity(
        budget: &mut CharacterDefinitionRequestBudget,
        error: CharacterDefinitionIntegrityError,
    ) -> Self {
        budget
            .charge(CharacterDefinitionWorkKind::AdmittedErrorCandidate)
            .map_or_else(Self::Resource, |()| Self::Integrity(error))
    }

    pub const fn lsp_code(&self) -> i32 {
        match self {
            Self::InvalidPosition(_) => -32_602,
            Self::Stale(_) => -32_801,
            Self::Resource(_) => -32_803,
            Self::Integrity(_) => -32_603,
        }
    }

    pub fn stable_code(&self) -> &'static str {
        match self {
            Self::InvalidPosition(_) => "aw.character.definition.invalid_position",
            Self::Stale(stale) => match stale.as_ref() {
                AcceptedCharacterDefinitionStale::Core(CharacterDefinitionStale::World {
                    ..
                }) => "aw.character.definition.stale_world",
                AcceptedCharacterDefinitionStale::Core(
                    CharacterDefinitionStale::SymbolRevision { .. },
                ) => "aw.character.definition.stale_symbol_revision",
                AcceptedCharacterDefinitionStale::Core(CharacterDefinitionStale::Document {
                    ..
                }) => "aw.character.definition.stale_document",
                AcceptedCharacterDefinitionStale::Core(
                    CharacterDefinitionStale::SyntaxSnapshot { .. },
                ) => "aw.character.definition.stale_snapshot",
                AcceptedCharacterDefinitionStale::Generation { .. } => {
                    "aw.character.definition.stale_generation"
                }
                AcceptedCharacterDefinitionStale::DocumentVersion { .. } => {
                    "aw.character.definition.stale_version"
                }
                AcceptedCharacterDefinitionStale::Profile { .. } => {
                    "aw.character.definition.stale_profile"
                }
                AcceptedCharacterDefinitionStale::OverlaySet => {
                    "aw.character.definition.stale_overlay"
                }
                AcceptedCharacterDefinitionStale::TargetDocument { .. } => {
                    "aw.character.definition.stale_target"
                }
                AcceptedCharacterDefinitionStale::MultiRoot { .. } => {
                    "aw.character.definition.stale_multi_root"
                }
            },
            Self::Resource(_) => "aw.character.definition.limit",
            Self::Integrity(_) => "aw.character.definition.internal_invariant",
        }
    }

    pub fn schedules_profile_rebuild(&self) -> bool {
        let Self::Stale(stale) = self else {
            return false;
        };
        matches!(
            stale.as_ref(),
            AcceptedCharacterDefinitionStale::TargetDocument { .. }
        )
    }
}

impl From<CharacterDefinitionResourceError> for CharacterDefinitionRequestError {
    fn from(error: CharacterDefinitionResourceError) -> Self {
        Self::Resource(error)
    }
}

impl From<CharacterReferenceInventoryError> for CharacterDefinitionRequestError {
    fn from(error: CharacterReferenceInventoryError) -> Self {
        match error {
            CharacterReferenceInventoryError::StaleWorld { expected, actual } => {
                Self::stale(AcceptedCharacterDefinitionStale::Core(
                    CharacterDefinitionStale::World { expected, actual },
                ))
            }
            CharacterReferenceInventoryError::StaleSymbolRevision { expected, actual } => {
                Self::stale(AcceptedCharacterDefinitionStale::Core(
                    CharacterDefinitionStale::SymbolRevision { expected, actual },
                ))
            }
            CharacterReferenceInventoryError::ProjectModuleMismatch { module } => {
                Self::Integrity(CharacterDefinitionIntegrityError::AcceptedModuleInvariant {
                    module,
                })
            }
            CharacterReferenceInventoryError::SemanticGenerationMismatch => Self::Integrity(
                CharacterDefinitionIntegrityError::AcceptedSemanticGenerationInvariant,
            ),
            CharacterReferenceInventoryError::SemanticOwnerMismatch { owner } => Self::Integrity(
                CharacterDefinitionIntegrityError::AcceptedExpressionInvariant { owner },
            ),
            CharacterReferenceInventoryError::Limit {
                kind,
                observed,
                maximum,
            } => Self::Resource(CharacterDefinitionResourceError::Limit {
                kind,
                observed,
                maximum,
            }),
            CharacterReferenceInventoryError::ArithmeticOverflow { counter } => {
                Self::Resource(CharacterDefinitionResourceError::ArithmeticOverflow { counter })
            }
        }
    }
}

pub(crate) fn character_definition(
    profile: &LspProfile,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    cursor: usize,
) -> Result<CharacterDefinitionDispatch, CharacterDefinitionRequestError> {
    let mut budget = CharacterDefinitionRequestBudget::for_request();
    character_definition_with_budget(profile, documents, document, cursor, &mut budget)
}

pub(crate) fn character_definition_with_budget(
    profile: &LspProfile,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    cursor: usize,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<CharacterDefinitionDispatch, CharacterDefinitionRequestError> {
    let Some(context) = prepare_character_request(profile, document, budget)? else {
        return Ok(CharacterDefinitionDispatch::NotCharacter);
    };
    let Some(inventory) = character_reference_inventory(&context, budget)? else {
        return Ok(CharacterDefinitionDispatch::NotCharacter);
    };
    let definition_key = CharacterDefinitionCacheKey::new(
        context.reference_key.clone(),
        context
            .executable
            .registered_world()
            .character_definition_index()
            .source_revision(),
        cursor,
    );
    let query = if let Some(query) = context
        .accepted
        .cached_character_definition(&definition_key, budget)?
    {
        query
    } else {
        let checkpoint = budget.checkpoint();
        let result = query_character_definition(
            context.executable.registered_world(),
            &inventory,
            context.document.identity(),
            cursor,
            budget,
        );
        let result = Arc::new(result);
        if matches!(
            result.as_ref(),
            CharacterDefinitionQueryResult::Resolved(_)
                | CharacterDefinitionQueryResult::NotApplicable(_)
                | CharacterDefinitionQueryResult::Unresolved(_)
        ) {
            let work = budget.receipt_since(checkpoint)?;
            context
                .accepted
                .cache_character_definition(definition_key, Arc::clone(&result), work);
        }
        result
    };
    dispatch_character_query(
        profile,
        documents,
        document,
        &context,
        query.as_ref(),
        budget,
    )
}

struct CharacterRequestContext {
    accepted: Arc<AcceptedProfileEnvironment>,
    executable: Arc<CompiledProject>,
    document: Arc<SourceDocument>,
    hir: Arc<HirModule>,
    reference_key: CharacterReferenceCacheKey,
}

fn prepare_character_request(
    profile: &LspProfile,
    document: &DocumentSnapshot,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<Option<CharacterRequestContext>, CharacterDefinitionRequestError> {
    let Some(accepted) = profile.accepted_environment() else {
        return Ok(None);
    };
    let Some(executable) = accepted.executable().cloned() else {
        return Ok(None);
    };
    budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
    let Some(accepted_origin) = accepted.project().sources().by_uri(document.uri()) else {
        return Ok(None);
    };
    budget.charge(CharacterDefinitionWorkKind::SourceAdaptation)?;
    budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
    if !Arc::ptr_eq(document.source_document(), accepted_origin.document()) {
        return Err(CharacterDefinitionRequestError::admitted_stale(
            budget,
            AcceptedCharacterDefinitionStale::Core(CharacterDefinitionStale::Document {
                expected: accepted_origin.document().identity().clone(),
                actual: document.source_document().identity().clone(),
            }),
        ));
    }
    let exact_document = Arc::clone(document.source_document());

    budget.charge(CharacterDefinitionWorkKind::SourceAdaptation)?;
    let project = accepted.project();
    budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
    let Some(module_key) = project.module_key(accepted_origin.document().identity()) else {
        return Ok(None);
    };
    let module = module_key.module().clone();
    let hir = Arc::clone(project.hir(&module_key).map_err(|_| {
        CharacterDefinitionRequestError::admitted_integrity(
            budget,
            CharacterDefinitionIntegrityError::AcceptedModuleInvariant {
                module: module.clone(),
            },
        )
    })?);
    let reference_key = CharacterReferenceCacheKey::new(
        accepted.profile().clone(),
        accepted.generation(),
        executable.registered_world().symbols().world().clone(),
        *executable.registered_world().symbols().revision(),
        exact_document.identity().clone(),
        module.clone(),
        hir.provenance().source_snapshot().clone(),
        document.version(),
    );
    Ok(Some(CharacterRequestContext {
        accepted,
        executable,
        document: exact_document,
        hir,
        reference_key,
    }))
}

fn character_reference_inventory(
    context: &CharacterRequestContext,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<Option<Arc<CharacterReferenceInventory>>, CharacterDefinitionRequestError> {
    if let Some(inventory) = context
        .accepted
        .cached_character_references(&context.reference_key, budget)?
    {
        return Ok(Some(inventory));
    }
    let checkpoint = budget.checkpoint();
    let project = context.accepted.project();
    let inventory = collect_character_references(
        context.executable.registered_world(),
        CharacterReferenceInput::new(
            project.hir_project().executable_view().map_err(|_| {
                CharacterDefinitionRequestError::admitted_integrity(
                    budget,
                    CharacterDefinitionIntegrityError::AcceptedModuleInvariant {
                        module: context.hir.key().path().clone(),
                    },
                )
            })?,
            context.hir.as_ref(),
            context.executable.final_analysis().as_ref(),
        ),
        budget,
    )
    .map_err(CharacterDefinitionRequestError::from)?;
    let work = budget.receipt_since(checkpoint)?;
    let inventory = Arc::new(inventory);
    context.accepted.cache_character_references(
        context.reference_key.clone(),
        Arc::clone(&inventory),
        work,
    );
    Ok(Some(inventory))
}

fn dispatch_character_query(
    profile: &LspProfile,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    context: &CharacterRequestContext,
    query: &CharacterDefinitionQueryResult,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<CharacterDefinitionDispatch, CharacterDefinitionRequestError> {
    match query {
        CharacterDefinitionQueryResult::Resolved(definition) => {
            let response =
                adapt_definition(&context.accepted, documents, document, definition, budget)?;
            final_request_check(profile, context, documents, document, budget)?;
            Ok(CharacterDefinitionDispatch::Character(response))
        }
        CharacterDefinitionQueryResult::Unresolved(_) => {
            Ok(CharacterDefinitionDispatch::Character(None))
        }
        CharacterDefinitionQueryResult::NotApplicable(reason) => {
            if matches!(
                reason,
                arcweft_lang_sema::character_definition::CharacterDefinitionNotApplicable::NonCharacterToken
            ) {
                Ok(CharacterDefinitionDispatch::NotCharacter)
            } else {
                Ok(CharacterDefinitionDispatch::Character(None))
            }
        }
        CharacterDefinitionQueryResult::Stale(stale) => {
            Err(CharacterDefinitionRequestError::stale(
                AcceptedCharacterDefinitionStale::Core(stale.clone()),
            ))
        }
        CharacterDefinitionQueryResult::Exhausted(error) => {
            Err(CharacterDefinitionRequestError::Resource(error.clone()))
        }
        CharacterDefinitionQueryResult::Integrity(error) => {
            Err(CharacterDefinitionRequestError::Integrity(error.clone()))
        }
    }
}

fn adapt_definition(
    accepted: &Arc<AcceptedProfileEnvironment>,
    documents: &DocumentStore,
    origin: &DocumentSnapshot,
    definition: &arcweft_lang_sema::character_definition::CharacterDefinition,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<Option<GotoDefinitionResponse>, CharacterDefinitionRequestError> {
    budget.charge(CharacterDefinitionWorkKind::SourceAdaptation)?;
    let origin_selection_range = origin.line_index().range_from_byte_span(
        definition.origin_selection().range().start(),
        definition.origin_selection().range().end(),
    );
    let mut links = Vec::with_capacity(definition.declarations().len());
    for declaration in definition.declarations() {
        budget.charge(CharacterDefinitionWorkKind::SourceAdaptation)?;
        budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
        let Some(target) = accepted
            .project()
            .sources()
            .get(declaration.selection_span().source())
        else {
            return Err(CharacterDefinitionRequestError::admitted_integrity(
                budget,
                CharacterDefinitionIntegrityError::MissingOwnedDocument {
                    source: declaration.selection_span().source().clone(),
                },
            ));
        };
        let Some(target_uri) = target.locator().uri().cloned() else {
            budget.charge(CharacterDefinitionWorkKind::AdmittedErrorCandidate)?;
            return Ok(None);
        };
        budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
        if let Some(open) = documents.get(&target_uri) {
            budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
            if open.source_document().identity() != target.document().identity() {
                return Err(CharacterDefinitionRequestError::admitted_stale(
                    budget,
                    AcceptedCharacterDefinitionStale::TargetDocument {
                        expected: target.document().identity().clone(),
                        actual: open.source_document().identity().clone(),
                    },
                ));
            }
        } else if let Some(path) = target.locator().path() {
            budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
            match arcweft_project_loader::source_document::validate_exact_file_document(
                path,
                target.document().identity(),
            ) {
                Ok(_) => {}
                Err(arcweft_project_loader::source_document::ExactFileDocumentError::IdentityMismatch {
                    actual,
                    ..
                }) => {
                    return Err(CharacterDefinitionRequestError::admitted_stale(
                        budget,
                        AcceptedCharacterDefinitionStale::TargetDocument {
                            expected: target.document().identity().clone(),
                            actual,
                        },
                    ));
                }
                Err(
                    arcweft_project_loader::source_document::ExactFileDocumentError::Read { .. }
                    | arcweft_project_loader::source_document::ExactFileDocumentError::Utf8 { .. }
                    | arcweft_project_loader::source_document::ExactFileDocumentError::SourceBytes { .. }
                    | arcweft_project_loader::source_document::ExactFileDocumentError::Document(_),
                ) => {
                    budget.charge(CharacterDefinitionWorkKind::AdmittedErrorCandidate)?;
                    return Ok(None);
                }
            }
        }
        budget.charge(CharacterDefinitionWorkKind::SourceAdaptation)?;
        let target_range = target.line_index().range_from_byte_span(
            declaration.value_span().range().start(),
            declaration.value_span().range().end(),
        );
        budget.charge(CharacterDefinitionWorkKind::SourceAdaptation)?;
        let target_selection_range = target.line_index().range_from_byte_span(
            declaration.selection_span().range().start(),
            declaration.selection_span().range().end(),
        );
        budget.charge(CharacterDefinitionWorkKind::SourceAdaptation)?;
        links.push(LocationLink {
            origin_selection_range: Some(origin_selection_range),
            target_uri,
            target_range,
            target_selection_range,
        });
    }
    Ok((!links.is_empty()).then_some(GotoDefinitionResponse::Link(links)))
}

fn final_request_check(
    profile: &LspProfile,
    context: &CharacterRequestContext,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<(), CharacterDefinitionRequestError> {
    let current = profile.accepted_environment();
    budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
    if current
        .as_ref()
        .is_none_or(|current| !Arc::ptr_eq(current, &context.accepted))
    {
        return Err(CharacterDefinitionRequestError::admitted_stale(
            budget,
            AcceptedCharacterDefinitionStale::Generation {
                expected: context.accepted.generation(),
                actual: current.as_ref().map(|current| current.generation()),
            },
        ));
    }
    budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
    let actual = documents.get(document.uri()).map(DocumentSnapshot::version);
    if actual != Some(document.version()) {
        return Err(CharacterDefinitionRequestError::admitted_stale(
            budget,
            AcceptedCharacterDefinitionStale::DocumentVersion {
                uri: document.uri().to_string(),
                expected: document.version(),
                actual,
            },
        ));
    }
    let current = current.expect("accepted generation was checked above");
    budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
    if current.profile() != context.accepted.profile() {
        return Err(CharacterDefinitionRequestError::admitted_stale(
            budget,
            AcceptedCharacterDefinitionStale::Profile {
                expected: context.accepted.profile().clone(),
                actual: Some(current.profile().clone()),
            },
        ));
    }
    budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
    if current.overlays() != context.accepted.overlays() {
        return Err(CharacterDefinitionRequestError::admitted_stale(
            budget,
            AcceptedCharacterDefinitionStale::OverlaySet,
        ));
    }
    budget.charge(CharacterDefinitionWorkKind::IdentityCheck)?;
    if current
        .project()
        .sources()
        .by_uri(document.uri())
        .is_none_or(|source| !Arc::ptr_eq(source.document(), &context.document))
    {
        return Err(CharacterDefinitionRequestError::admitted_stale(
            budget,
            AcceptedCharacterDefinitionStale::OverlaySet,
        ));
    }
    Ok(())
}
