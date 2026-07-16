//! Character-aware definition dispatch over one accepted semantic generation.

use std::{path::Path, sync::Arc};

use arcweft_lang_hir::lower::lower_document_to_hir;
use arcweft_lang_sema::{
    character_definition::{
        CharacterDefinitionIntegrityError, CharacterDefinitionQueryResult,
        CharacterDefinitionResourceError, CharacterDefinitionStale, CharacterReferenceInput,
        CharacterReferenceInventory, CharacterReferenceInventoryError,
        collect_character_references, query_character_definition,
    },
    check::analyze_registered_project_types,
};
use arcweft_lang_syntax::{ast::module_path::CanonicalModulePath, parser::parse_source};
use arcweft_source::{SourceDocument, SourceDocumentIdentity, SourceSpan};
use arcweft_verify_lsp::LspPositionMapper;
use lsp_types::{GotoDefinitionResponse, LocationLink};
use thiserror::Error;

use crate::{
    documents::{DocumentSnapshot, DocumentStore, rebind_overlay},
    positions::CheckedPositionError,
    profiles::{
        LspProfile,
        cache::{
            AcceptedEnvironmentGeneration, AcceptedProfileEnvironment, AcceptedProfileKey,
            CharacterDefinitionCacheKey, CharacterReferenceCacheKey,
        },
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
        expected: Option<i32>,
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

/// Explicit source-adapter failure for one declaration target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDefinitionSourceError {
    MissingSource { identity: SourceDocumentIdentity },
    UnreadableSource { identity: SourceDocumentIdentity },
    UnmappedSource { identity: SourceDocumentIdentity },
    InvalidUri { identity: SourceDocumentIdentity },
    RangeConversion { source: SourceSpan },
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

pub(crate) fn character_definition(
    profile: &LspProfile,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    cursor: usize,
) -> Result<CharacterDefinitionDispatch, CharacterDefinitionRequestError> {
    let Some(context) = prepare_character_request(profile, document)? else {
        return Ok(CharacterDefinitionDispatch::NotCharacter);
    };
    let Some(inventory) = character_reference_inventory(&context)? else {
        return Ok(CharacterDefinitionDispatch::NotCharacter);
    };
    let definition_key = CharacterDefinitionCacheKey::new(
        context.reference_key.clone(),
        context
            .accepted
            .world()
            .character_definition_index()
            .source_revision(),
        cursor,
    );
    let query = context
        .accepted
        .cached_character_definition(&definition_key)
        .unwrap_or_else(|| {
            let result = query_character_definition(
                context.accepted.world(),
                &inventory,
                context.rebound.identity(),
                cursor,
            );
            if matches!(
                &result,
                CharacterDefinitionQueryResult::Resolved(_)
                    | CharacterDefinitionQueryResult::NotApplicable(_)
                    | CharacterDefinitionQueryResult::Unresolved(_)
            ) {
                context
                    .accepted
                    .cache_character_definition(definition_key, result.clone());
            }
            result
        });
    dispatch_character_query(
        profile, documents, document, &context, &inventory, cursor, query,
    )
}

struct CharacterRequestContext {
    accepted: Arc<AcceptedProfileEnvironment>,
    rebound: Arc<SourceDocument>,
    module: CanonicalModulePath,
    reference_key: CharacterReferenceCacheKey,
}

fn prepare_character_request(
    profile: &LspProfile,
    document: &DocumentSnapshot,
) -> Result<Option<CharacterRequestContext>, CharacterDefinitionRequestError> {
    let Some(accepted) = profile.accepted_environment() else {
        return Ok(None);
    };
    let Some(accepted_origin) = accepted.sources().by_uri(document.uri()) else {
        return Ok(None);
    };
    let rebound = rebind_overlay(document, accepted_origin).map_err(|_| {
        CharacterDefinitionRequestError::stale(AcceptedCharacterDefinitionStale::Profile {
            expected: accepted.profile().clone(),
            actual: profile
                .accepted_environment()
                .map(|environment| environment.profile().clone()),
        })
    })?;
    if rebound.identity() != accepted_origin.document().identity() {
        return Err(CharacterDefinitionRequestError::stale(
            AcceptedCharacterDefinitionStale::Core(CharacterDefinitionStale::Document {
                expected: accepted_origin.document().identity().clone(),
                actual: rebound.identity().clone(),
            }),
        ));
    }

    let Some(path) = accepted_origin.locator().path() else {
        return Ok(None);
    };
    let Ok(loaded) = arcweft_project_loader::project::load_discovered(path) else {
        return Ok(None);
    };
    let normalized = normalized_path(path);
    let Some(project_source) = loaded
        .sources()
        .modules()
        .find(|source| normalized_path(source.path()) == normalized)
    else {
        return Ok(None);
    };
    let module = project_source.module().clone();
    let reference_key = CharacterReferenceCacheKey::new(
        accepted.profile().clone(),
        accepted.generation(),
        accepted.world().symbols().world().clone(),
        *accepted.world().symbols().revision(),
        rebound.identity().clone(),
        module.clone(),
        None,
        document.version(),
    );
    Ok(Some(CharacterRequestContext {
        accepted,
        rebound,
        module,
        reference_key,
    }))
}

fn character_reference_inventory(
    context: &CharacterRequestContext,
) -> Result<Option<Arc<CharacterReferenceInventory>>, CharacterDefinitionRequestError> {
    if let Some(inventory) = context
        .accepted
        .cached_character_references(&context.reference_key)
    {
        return Ok(Some(inventory));
    }
    let parsed = parse_source(context.rebound.text());
    let Ok(hir) = lower_document_to_hir(&context.rebound, parsed.typed_tree()) else {
        return Ok(None);
    };
    let report = analyze_registered_project_types(&hir, context.accepted.world());
    let inventory = collect_character_references(
        context.accepted.world(),
        CharacterReferenceInput::new(
            &context.rebound,
            &context.module,
            parsed.typed_tree(),
            &report,
            parsed.errors(),
            None,
        ),
    )
    .map_err(character_reference_error)?;
    let inventory = Arc::new(inventory);
    context
        .accepted
        .cache_character_references(context.reference_key.clone(), Arc::clone(&inventory));
    Ok(Some(inventory))
}

fn character_reference_error(
    error: CharacterReferenceInventoryError,
) -> CharacterDefinitionRequestError {
    match error {
        CharacterReferenceInventoryError::StaleWorld { expected, actual } => {
            CharacterDefinitionRequestError::stale(AcceptedCharacterDefinitionStale::Core(
                CharacterDefinitionStale::World { expected, actual },
            ))
        }
        CharacterReferenceInventoryError::StaleSymbolRevision { expected, actual } => {
            CharacterDefinitionRequestError::stale(AcceptedCharacterDefinitionStale::Core(
                CharacterDefinitionStale::SymbolRevision { expected, actual },
            ))
        }
        CharacterReferenceInventoryError::DocumentMismatch { expected, actual } => {
            CharacterDefinitionRequestError::stale(AcceptedCharacterDefinitionStale::Core(
                CharacterDefinitionStale::Document { expected, actual },
            ))
        }
        CharacterReferenceInventoryError::Limit {
            kind,
            observed,
            maximum,
        } => CharacterDefinitionRequestError::Resource(CharacterDefinitionResourceError::Limit {
            kind,
            observed,
            maximum,
        }),
        CharacterReferenceInventoryError::ArithmeticOverflow { counter } => {
            CharacterDefinitionRequestError::Resource(
                CharacterDefinitionResourceError::ArithmeticOverflow { counter },
            )
        }
    }
}

fn dispatch_character_query(
    profile: &LspProfile,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
    context: &CharacterRequestContext,
    inventory: &CharacterReferenceInventory,
    cursor: usize,
    query: CharacterDefinitionQueryResult,
) -> Result<CharacterDefinitionDispatch, CharacterDefinitionRequestError> {
    match query {
        CharacterDefinitionQueryResult::Resolved(definition) => {
            let response = adapt_definition(&context.accepted, documents, document, &definition)?;
            final_request_check(profile, &context.accepted, documents, document)?;
            Ok(CharacterDefinitionDispatch::Character(response))
        }
        CharacterDefinitionQueryResult::Unresolved(_) => {
            Ok(CharacterDefinitionDispatch::Character(None))
        }
        CharacterDefinitionQueryResult::NotApplicable(reason) => {
            let within_reference = inventory.facts().any(|fact| {
                let range = fact.reference_span().range();
                range.start() <= cursor && cursor < range.end()
            });
            if within_reference
                || !matches!(
                    reason,
                    arcweft_lang_sema::character_definition::CharacterDefinitionNotApplicable::NonCharacterToken
                )
            {
                Ok(CharacterDefinitionDispatch::Character(None))
            } else {
                Ok(CharacterDefinitionDispatch::NotCharacter)
            }
        }
        CharacterDefinitionQueryResult::Stale(stale) => Err(
            CharacterDefinitionRequestError::stale(AcceptedCharacterDefinitionStale::Core(stale)),
        ),
        CharacterDefinitionQueryResult::Exhausted(error) => {
            Err(CharacterDefinitionRequestError::Resource(error))
        }
        CharacterDefinitionQueryResult::Integrity(error) => {
            Err(CharacterDefinitionRequestError::Integrity(error))
        }
    }
}

fn adapt_definition(
    accepted: &Arc<AcceptedProfileEnvironment>,
    documents: &DocumentStore,
    origin: &DocumentSnapshot,
    definition: &arcweft_lang_sema::character_definition::CharacterDefinition,
) -> Result<Option<GotoDefinitionResponse>, CharacterDefinitionRequestError> {
    let mut links = Vec::with_capacity(definition.declarations().len());
    for declaration in definition.declarations() {
        let Some(target) = accepted
            .sources()
            .get(declaration.selection_span().source())
        else {
            return Err(CharacterDefinitionRequestError::Integrity(
                CharacterDefinitionIntegrityError::MissingOwnedDocument {
                    source: declaration.selection_span().source().clone(),
                },
            ));
        };
        let Some(target_uri) = target.locator().uri().cloned() else {
            return Ok(None);
        };
        if let Some(open) = documents.get(&target_uri) {
            let rebound = rebind_overlay(open, target).map_err(|_| {
                CharacterDefinitionRequestError::stale(
                    AcceptedCharacterDefinitionStale::TargetDocument {
                        expected: target.document().identity().clone(),
                        actual: open.source_document().identity().clone(),
                    },
                )
            })?;
            if rebound.identity() != target.document().identity() {
                return Err(CharacterDefinitionRequestError::stale(
                    AcceptedCharacterDefinitionStale::TargetDocument {
                        expected: target.document().identity().clone(),
                        actual: rebound.identity().clone(),
                    },
                ));
            }
        } else if let Some(path) = target.locator().path() {
            match arcweft_project_loader::source_document::validate_exact_file_document(
                path,
                target.document().identity(),
            ) {
                Ok(_) => {}
                Err(arcweft_project_loader::source_document::ExactFileDocumentError::IdentityMismatch {
                    actual,
                    ..
                }) => {
                    return Err(CharacterDefinitionRequestError::stale(
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
                ) => return Ok(None),
            }
        }
        links.push(LocationLink {
            origin_selection_range: Some(origin.line_index().range_from_byte_span(
                definition.origin_selection().range().start(),
                definition.origin_selection().range().end(),
            )),
            target_uri,
            target_range: target.line_index().range_from_byte_span(
                declaration.value_span().range().start(),
                declaration.value_span().range().end(),
            ),
            target_selection_range: target.line_index().range_from_byte_span(
                declaration.selection_span().range().start(),
                declaration.selection_span().range().end(),
            ),
        });
    }
    Ok((!links.is_empty()).then_some(GotoDefinitionResponse::Link(links)))
}

fn final_request_check(
    profile: &LspProfile,
    accepted: &Arc<AcceptedProfileEnvironment>,
    documents: &DocumentStore,
    document: &DocumentSnapshot,
) -> Result<(), CharacterDefinitionRequestError> {
    let current = profile.accepted_environment();
    if current
        .as_ref()
        .is_none_or(|current| !Arc::ptr_eq(current, accepted))
    {
        return Err(CharacterDefinitionRequestError::stale(
            AcceptedCharacterDefinitionStale::Generation {
                expected: accepted.generation(),
                actual: current.as_ref().map(|current| current.generation()),
            },
        ));
    }
    let actual = documents
        .get(document.uri())
        .and_then(DocumentSnapshot::version);
    if actual != document.version() {
        return Err(CharacterDefinitionRequestError::stale(
            AcceptedCharacterDefinitionStale::DocumentVersion {
                uri: document.uri().to_string(),
                expected: document.version(),
                actual,
            },
        ));
    }
    Ok(())
}

fn normalized_path(path: &Path) -> std::path::PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
