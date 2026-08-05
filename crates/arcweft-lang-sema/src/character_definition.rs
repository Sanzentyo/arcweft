//! Request-scoped character references and Sans-I/O definition queries.

mod query;
mod request_budget;

#[cfg(test)]
#[path = "character_definition/tests.rs"]
mod tests;

pub use query::query_character_definition;
pub use request_budget::{
    CharacterDefinitionBudgetCheckpoint, CharacterDefinitionRequestBudget,
    CharacterDefinitionWorkKind, CharacterDefinitionWorkReceipt,
};

use arcweft_character::{
    id::{CharacterLookId, CharacterPartId, CharacterVariantId},
    symbol::CharacterSymbolDescriptor,
};
use arcweft_lang_hir::symbol::{
    ProjectSymbolResolutionError, ProjectSymbolRevision, ProjectSymbolTargetId,
    ProjectSymbolWorldId,
};
use arcweft_lang_syntax::{
    ast::{
        items::TypedSyntaxTree,
        module_path::CanonicalModulePath,
        symbol_path::{SpannedProjectSymbolPath, SymbolPath},
    },
    expr::{Expr, parse_expr},
    parser::recovery::ParseError,
};
use arcweft_source::{
    SourceDocument, SourceDocumentIdentity, SourceRange, SourceSpan, identity::SourceSnapshotId,
};
use thiserror::Error;

use crate::{
    check::{TypeCheckReport, TypeJudgmentSubject},
    registration::{
        CharacterDeclarationSource, CharacterDefinitionLimitKind, CharacterDefinitionLimits,
        ExternalOwnerLookupError, RegisteredCharacterResolutionError, RegisteredSemanticWorld,
    },
    types::{CharacterNominalType, EntityKind},
};

/// One request-scoped inventory bound to exact semantic and source identities.
#[derive(Clone, Debug)]
pub struct CharacterReferenceInventory {
    world: ProjectSymbolWorldId,
    symbol_revision: ProjectSymbolRevision,
    document: SourceDocumentIdentity,
    module: CanonicalModulePath,
    syntax_snapshot: Option<SourceSnapshotId>,
    facts: Vec<CharacterReferenceFact>,
}

/// One parsed character reference and its typed resolution outcome.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CharacterReferenceFact {
    reference_span: SourceSpan,
    selection_span: SourceSpan,
    form: CharacterReferenceForm,
    resolution: CharacterReferenceResolution,
}

/// Authored form retained for definition and future rename policy.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CharacterReferenceForm {
    OwnerPath {
        path: SymbolPath,
    },
    LocalMember {
        spelling: String,
        expected: Option<CharacterNominalType>,
    },
}

/// Typed resolution stored with one current reference fact.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CharacterReferenceResolution {
    Resolved(CharacterSymbolDescriptor),
    Unresolved(CharacterDefinitionIssue),
}

/// Resolved occurrence boundary reserved for the later rename contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterReferenceOccurrence {
    descriptor: CharacterSymbolDescriptor,
    reference_span: SourceSpan,
    selection_span: SourceSpan,
    form: CharacterReferenceForm,
}

/// Exact current analysis values used to collect character references.
#[derive(Clone, Copy)]
pub struct CharacterReferenceInput<'a> {
    document: &'a SourceDocument,
    module: &'a CanonicalModulePath,
    typed_tree: &'a TypedSyntaxTree,
    type_report: &'a TypeCheckReport,
    parse_diagnostics: &'a [ParseError],
    syntax_snapshot: Option<&'a SourceSnapshotId>,
}

impl<'a> CharacterReferenceInput<'a> {
    #[allow(
        clippy::too_many_arguments,
        reason = "the request boundary keeps all independently checked source and semantic identities explicit"
    )]
    pub fn new(
        document: &'a SourceDocument,
        module: &'a CanonicalModulePath,
        typed_tree: &'a TypedSyntaxTree,
        type_report: &'a TypeCheckReport,
        parse_diagnostics: &'a [ParseError],
        syntax_snapshot: Option<&'a SourceSnapshotId>,
    ) -> Self {
        Self {
            document,
            module,
            typed_tree,
            type_report,
            parse_diagnostics,
            syntax_snapshot,
        }
    }
}

impl CharacterReferenceInventory {
    pub const fn world(&self) -> &ProjectSymbolWorldId {
        &self.world
    }

    pub const fn symbol_revision(&self) -> &ProjectSymbolRevision {
        &self.symbol_revision
    }

    pub const fn document(&self) -> &SourceDocumentIdentity {
        &self.document
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn syntax_snapshot(&self) -> Option<&SourceSnapshotId> {
        self.syntax_snapshot.as_ref()
    }

    pub fn facts(&self) -> impl ExactSizeIterator<Item = &CharacterReferenceFact> {
        self.facts.iter()
    }

    pub fn resolved_occurrences(&self) -> impl Iterator<Item = CharacterReferenceOccurrence> + '_ {
        self.facts.iter().filter_map(|fact| {
            let CharacterReferenceResolution::Resolved(descriptor) = fact.resolution() else {
                return None;
            };
            Some(CharacterReferenceOccurrence {
                descriptor: descriptor.clone(),
                reference_span: fact.reference_span.clone(),
                selection_span: fact.selection_span.clone(),
                form: fact.form.clone(),
            })
        })
    }
}

impl CharacterReferenceFact {
    pub const fn reference_span(&self) -> &SourceSpan {
        &self.reference_span
    }

    pub const fn selection_span(&self) -> &SourceSpan {
        &self.selection_span
    }

    pub const fn form(&self) -> &CharacterReferenceForm {
        &self.form
    }

    pub const fn resolution(&self) -> &CharacterReferenceResolution {
        &self.resolution
    }
}

impl CharacterReferenceOccurrence {
    pub const fn descriptor(&self) -> &CharacterSymbolDescriptor {
        &self.descriptor
    }

    pub const fn reference_span(&self) -> &SourceSpan {
        &self.reference_span
    }

    pub const fn selection_span(&self) -> &SourceSpan {
        &self.selection_span
    }

    pub const fn form(&self) -> &CharacterReferenceForm {
        &self.form
    }
}

/// Recoverable semantic issue for a syntactically complete character reference.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CharacterDefinitionIssue {
    UnknownOwner {
        reference: SymbolPath,
    },
    AmbiguousAlias {
        reference: SymbolPath,
        candidates: Vec<ProjectSymbolTargetId>,
    },
    WrongOwnerKind {
        reference: SymbolPath,
        actual: ProjectSymbolTargetId,
    },
    UnknownMember {
        spelling: String,
        expected: Option<CharacterNominalType>,
    },
    AmbiguousMember {
        spelling: String,
        candidates: Vec<CharacterSymbolDescriptor>,
    },
    AmbiguousSemanticContext {
        spelling: String,
        candidates: Vec<CharacterNominalType>,
    },
    WrongNominalFamily {
        spelling: String,
        expected: CharacterNominalType,
        candidates: Vec<CharacterSymbolDescriptor>,
    },
    WrongOwningPart {
        spelling: String,
        expected: CharacterNominalType,
        candidates: Vec<CharacterSymbolDescriptor>,
    },
    RecoveredToken {
        source: SourceSpan,
    },
}

/// Failure to bind a reference inventory to one exact registered world.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterReferenceInventoryError {
    #[error("character reference inventory world is stale")]
    StaleWorld {
        expected: ProjectSymbolWorldId,
        actual: ProjectSymbolWorldId,
    },
    #[error("character reference inventory symbol revision is stale")]
    StaleSymbolRevision {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    #[error("character reference inventory document differs from the typed source")]
    DocumentMismatch {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    #[error("character reference inventory resource limit exceeded")]
    Limit {
        kind: CharacterDefinitionLimitKind,
        observed: u64,
        maximum: u64,
    },
    #[error("character reference inventory counter overflowed")]
    ArithmeticOverflow {
        counter: CharacterDefinitionLimitKind,
    },
}

impl From<CharacterDefinitionResourceError> for CharacterReferenceInventoryError {
    fn from(error: CharacterDefinitionResourceError) -> Self {
        match error {
            CharacterDefinitionResourceError::Limit {
                kind,
                observed,
                maximum,
            } => Self::Limit {
                kind,
                observed,
                maximum,
            },
            CharacterDefinitionResourceError::ArithmeticOverflow { counter } => {
                Self::ArithmeticOverflow { counter }
            }
        }
    }
}

/// Collects typed owner/member references from one current source analysis.
#[allow(
    clippy::result_large_err,
    reason = "the public contract preserves complete typed stale identities without boxing"
)]
pub fn collect_character_references(
    world: &RegisteredSemanticWorld,
    input: CharacterReferenceInput<'_>,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<CharacterReferenceInventory, CharacterReferenceInventoryError> {
    ensure_world_integrity(world, budget)?;
    budget
        .charge(CharacterDefinitionWorkKind::IdentityCheck)
        .map_err(CharacterReferenceInventoryError::from)?;
    if input.typed_tree.source() != input.document.text() {
        admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
        let actual = SourceDocument::try_new(
            input.document.identity().id().clone(),
            input.document.display_name().clone(),
            input.typed_tree.source(),
        )
        .map_err(|_| CharacterReferenceInventoryError::ArithmeticOverflow {
            counter: CharacterDefinitionLimitKind::SourceBytes,
        })?;
        return Err(CharacterReferenceInventoryError::DocumentMismatch {
            expected: input.document.identity().clone(),
            actual: actual.identity().clone(),
        });
    }

    let limits = CharacterDefinitionLimits::PRODUCTION;
    let mut facts = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for judgment in &input.type_report.judgments {
        budget
            .charge(CharacterDefinitionWorkKind::ParserFact)
            .map_err(CharacterReferenceInventoryError::from)?;
        let TypeJudgmentSubject::Expr { kind, .. } = &judgment.subject else {
            continue;
        };
        if !matches!(*kind, "entity_ref" | "short_variant") {
            continue;
        }
        let Some(range) = judgment.source_range else {
            continue;
        };
        if !seen.insert((range, *kind)) {
            continue;
        }
        let Some(source) = input.document.text().get(range.as_range()) else {
            admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
            return Err(CharacterReferenceInventoryError::DocumentMismatch {
                expected: input.document.identity().clone(),
                actual: input.document.identity().clone(),
            });
        };
        budget
            .charge(CharacterDefinitionWorkKind::ParserFact)
            .map_err(CharacterReferenceInventoryError::from)?;
        let Ok(expr) = parse_expr(source) else {
            continue;
        };
        let fact = match expr {
            Expr::EntityRef(entity)
                if judgment.ty.is_entity_ref_kind(&EntityKind::Character)
                    || judgment
                        .expected_type()
                        .is_some_and(|ty| ty.is_entity_ref_kind(&EntityKind::Character)) =>
            {
                owner_fact(world, &input, range, &entity, budget)?
            }
            Expr::ShortVariant(name) => {
                local_member_fact(world, &input, range, name.as_str(), budget)?
            }
            _ => None,
        };
        if let Some(fact) = fact {
            admit_reference_fact(&mut facts, fact, limits.candidates())?;
        }
    }
    facts.sort();
    facts.dedup();
    Ok(CharacterReferenceInventory {
        world: world.symbols().world().clone(),
        symbol_revision: *world.symbols().revision(),
        document: input.document.identity().clone(),
        module: input.module.clone(),
        syntax_snapshot: input.syntax_snapshot.cloned(),
        facts,
    })
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn admit_reference_fact(
    facts: &mut Vec<CharacterReferenceFact>,
    fact: CharacterReferenceFact,
    maximum: u64,
) -> Result<(), CharacterReferenceInventoryError> {
    let observed = u64::try_from(facts.len())
        .ok()
        .and_then(|count| count.checked_add(1))
        .ok_or(CharacterReferenceInventoryError::ArithmeticOverflow {
            counter: CharacterDefinitionLimitKind::Candidates,
        })?;
    if observed > maximum {
        return Err(CharacterReferenceInventoryError::Limit {
            kind: CharacterDefinitionLimitKind::Candidates,
            observed,
            maximum,
        });
    }
    facts.push(fact);
    Ok(())
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn owner_fact(
    world: &RegisteredSemanticWorld,
    input: &CharacterReferenceInput<'_>,
    range: arcweft_lang_syntax::ast::common::TextRange,
    entity: &arcweft_lang_syntax::ast::ids::EntityRefSyntax,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<Option<CharacterReferenceFact>, CharacterReferenceInventoryError> {
    let Some(entity) = entity.as_absolute() else {
        return Ok(None);
    };
    if !entity.is_authored() || entity.is_delimited() {
        return Ok(None);
    }
    let body = entity.body();
    let Some(authored_body_range) = entity.authored_body_range() else {
        return Ok(None);
    };
    let Some(body_base) = range.start().checked_add(authored_body_range.start()) else {
        return Ok(None);
    };
    let Ok(spanned) = SpannedProjectSymbolPath::parse_at(body, body_base) else {
        return Ok(None);
    };
    let Ok(path) = SymbolPath::try_from(spanned.path()) else {
        return Ok(None);
    };
    let Some(selection_range) = spanned.segment_ranges().last().copied() else {
        return Ok(None);
    };
    let reference_span = input
        .document
        .span(SourceRange::new(range.start(), range.end()))
        .expect("a type judgment range belongs to its parsed source");
    let selection_span = input
        .document
        .span(SourceRange::new(
            selection_range.start(),
            selection_range.end(),
        ))
        .expect("a parsed path segment range belongs to its source");
    let form = CharacterReferenceForm::OwnerPath { path: path.clone() };
    let resolution = if intersects_recovery(range, input.parse_diagnostics, budget)? {
        admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::RecoveredToken {
            source: reference_span.clone(),
        })
    } else {
        match world.environment().resolve_character_owner(
            world.symbols(),
            input.module,
            &path,
            &reference_span,
        ) {
            Ok(character) => {
                budget
                    .charge(CharacterDefinitionWorkKind::ProjectSymbolCandidate)
                    .map_err(CharacterReferenceInventoryError::from)?;
                CharacterReferenceResolution::Resolved(CharacterSymbolDescriptor::Owner {
                    character,
                })
            }
            Err(error) => map_owner_issue(path, error, budget)?,
        }
    };
    Ok(Some(CharacterReferenceFact {
        reference_span,
        selection_span,
        form,
        resolution,
    }))
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn map_owner_issue(
    reference: SymbolPath,
    error: RegisteredCharacterResolutionError,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<CharacterReferenceResolution, CharacterReferenceInventoryError> {
    Ok(CharacterReferenceResolution::Unresolved(match error {
        RegisteredCharacterResolutionError::Symbol(
            ProjectSymbolResolutionError::Unknown { .. }
            | ProjectSymbolResolutionError::InvalidPath { .. },
        )
        | RegisteredCharacterResolutionError::Owner(ExternalOwnerLookupError::Unknown { .. }) => {
            admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
            CharacterDefinitionIssue::UnknownOwner { reference }
        }
        RegisteredCharacterResolutionError::Symbol(ProjectSymbolResolutionError::Ambiguous {
            mut candidates,
            ..
        }) => {
            candidates.sort();
            candidates.dedup();
            let maximum = CharacterDefinitionLimits::PRODUCTION.candidates();
            let mut candidate_count = 0_u64;
            for _ in &candidates {
                budget
                    .charge(CharacterDefinitionWorkKind::ProjectSymbolCandidate)
                    .map_err(CharacterReferenceInventoryError::from)?;
                let observed = candidate_count.checked_add(1).ok_or(
                    CharacterReferenceInventoryError::ArithmeticOverflow {
                        counter: CharacterDefinitionLimitKind::Candidates,
                    },
                )?;
                if observed > maximum {
                    return Err(CharacterReferenceInventoryError::Limit {
                        kind: CharacterDefinitionLimitKind::Candidates,
                        observed,
                        maximum,
                    });
                }
                candidate_count = observed;
            }
            admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
            let candidates = admit_error_payload(budget, candidates.iter())
                .map_err(CharacterReferenceInventoryError::from)?;
            CharacterDefinitionIssue::AmbiguousAlias {
                reference,
                candidates,
            }
        }
        RegisteredCharacterResolutionError::Symbol(ProjectSymbolResolutionError::NotCallable {
            actual,
            ..
        })
        | RegisteredCharacterResolutionError::NotExternal { actual } => {
            budget
                .charge(CharacterDefinitionWorkKind::ProjectSymbolCandidate)
                .map_err(CharacterReferenceInventoryError::from)?;
            admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
            CharacterDefinitionIssue::WrongOwnerKind { reference, actual }
        }
        RegisteredCharacterResolutionError::Owner(ExternalOwnerLookupError::WrongKind {
            declaration,
            ..
        }) => {
            budget
                .charge(CharacterDefinitionWorkKind::ProjectSymbolCandidate)
                .map_err(CharacterReferenceInventoryError::from)?;
            admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
            CharacterDefinitionIssue::WrongOwnerKind {
                reference,
                actual: ProjectSymbolTargetId::External(declaration),
            }
        }
        RegisteredCharacterResolutionError::Owner(ExternalOwnerLookupError::Stale {
            expected_world,
            actual_world,
            expected_revision,
            actual_revision,
        }) => {
            admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
            if expected_world != actual_world {
                return Err(CharacterReferenceInventoryError::StaleWorld {
                    expected: expected_world,
                    actual: actual_world,
                });
            }
            return Err(CharacterReferenceInventoryError::StaleSymbolRevision {
                expected: expected_revision,
                actual: actual_revision,
            });
        }
    }))
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn local_member_fact(
    world: &RegisteredSemanticWorld,
    input: &CharacterReferenceInput<'_>,
    range: arcweft_lang_syntax::ast::common::TextRange,
    spelling: &str,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<Option<CharacterReferenceFact>, CharacterReferenceInventoryError> {
    let Some(selection_start) = range.start().checked_add(1) else {
        return Err(CharacterReferenceInventoryError::ArithmeticOverflow {
            counter: CharacterDefinitionLimitKind::QueryWork,
        });
    };
    if selection_start > range.end() {
        return Ok(None);
    }
    let reference_span = input
        .document
        .span(SourceRange::new(range.start(), range.end()))
        .expect("a type judgment range belongs to its parsed source");
    let selection_span = input
        .document
        .span(SourceRange::new(selection_start, range.end()))
        .expect("a local-member identifier belongs to its parsed source");
    let expected = expected_nominal(input.type_report, selection_span.range(), budget)?;
    let form_expected = match &expected {
        ExpectedNominal::Missing | ExpectedNominal::Ambiguous(_) => None,
        ExpectedNominal::Unique(expected) => Some(expected.clone()),
    };
    let form = CharacterReferenceForm::LocalMember {
        spelling: spelling.to_owned(),
        expected: form_expected,
    };
    let resolution = if intersects_recovery(range, input.parse_diagnostics, budget)? {
        admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
        CharacterReferenceResolution::Unresolved(CharacterDefinitionIssue::RecoveredToken {
            source: reference_span.clone(),
        })
    } else {
        match expected {
            ExpectedNominal::Missing => resolve_local_member(world, spelling, None, budget)?,
            ExpectedNominal::Unique(expected) => {
                resolve_local_member(world, spelling, Some(expected), budget)?
            }
            ExpectedNominal::Ambiguous(candidates) => {
                admit_nonresource_error(budget, candidates.len())
                    .map_err(CharacterReferenceInventoryError::from)?;
                CharacterReferenceResolution::Unresolved(
                    CharacterDefinitionIssue::AmbiguousSemanticContext {
                        spelling: spelling.to_owned(),
                        candidates,
                    },
                )
            }
        }
    };
    Ok(Some(CharacterReferenceFact {
        reference_span,
        selection_span,
        form,
        resolution,
    }))
}

enum ExpectedNominal {
    Missing,
    Unique(CharacterNominalType),
    Ambiguous(Vec<CharacterNominalType>),
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn expected_nominal(
    report: &TypeCheckReport,
    selection: SourceRange,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<ExpectedNominal, CharacterReferenceInventoryError> {
    let mut closest_width = None;
    let mut closest = std::collections::BTreeSet::new();
    for judgment in &report.judgments {
        budget
            .charge(CharacterDefinitionWorkKind::ParserFact)
            .map_err(CharacterReferenceInventoryError::from)?;
        let Some(range) = judgment.source_range else {
            continue;
        };
        if selection.start() < range.start() || range.end() < selection.end() {
            continue;
        }
        let Some(nominal) = judgment
            .expected_type()
            .and_then(|ty| ty.character_nominal())
            .or_else(|| judgment.ty.character_nominal())
        else {
            continue;
        };
        let width = range.end().checked_sub(range.start()).ok_or(
            CharacterReferenceInventoryError::ArithmeticOverflow {
                counter: CharacterDefinitionLimitKind::QueryWork,
            },
        )?;
        match closest_width {
            Some(current) if current < width => continue,
            Some(current) if width < current => {
                closest.clear();
                closest_width = Some(width);
            }
            None => closest_width = Some(width),
            Some(_) => {}
        }
        if closest.contains(nominal) {
            continue;
        }
        let observed = u64::try_from(closest.len())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or(CharacterReferenceInventoryError::ArithmeticOverflow {
                counter: CharacterDefinitionLimitKind::Candidates,
            })?;
        let maximum = CharacterDefinitionLimits::PRODUCTION.candidates();
        if observed > maximum {
            return Err(CharacterReferenceInventoryError::Limit {
                kind: CharacterDefinitionLimitKind::Candidates,
                observed,
                maximum,
            });
        }
        closest.insert(nominal.clone());
    }
    let closest = closest.into_iter().collect::<Vec<_>>();
    match closest.as_slice() {
        [expected] => Ok(ExpectedNominal::Unique(expected.clone())),
        [] => Ok(ExpectedNominal::Missing),
        _ => Ok(ExpectedNominal::Ambiguous(closest)),
    }
}

#[allow(
    clippy::result_large_err,
    clippy::too_many_lines,
    reason = "the linear charged scan keeps candidate admission, classification, and payload materialization in canonical order"
)]
fn resolve_local_member(
    world: &RegisteredSemanticWorld,
    spelling: &str,
    expected: Option<CharacterNominalType>,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<CharacterReferenceResolution, CharacterReferenceInventoryError> {
    let index = world.character_definition_index();
    if let Some(expected) = expected.as_ref() {
        budget
            .charge(CharacterDefinitionWorkKind::TypedMemberCandidate)
            .map_err(CharacterReferenceInventoryError::from)?;
        let descriptor = match expected {
            CharacterNominalType::Look { character } => CharacterLookId::try_new(spelling)
                .ok()
                .map(|look| CharacterSymbolDescriptor::Look {
                    character: character.clone(),
                    look,
                }),
            CharacterNominalType::Part { character } => CharacterPartId::try_new(spelling)
                .ok()
                .map(|part| CharacterSymbolDescriptor::Part {
                    character: character.clone(),
                    part,
                }),
            CharacterNominalType::Variant { character, part } => {
                CharacterVariantId::try_new(spelling).ok().map(|variant| {
                    CharacterSymbolDescriptor::Variant {
                        character: character.clone(),
                        part: part.clone(),
                        variant,
                    }
                })
            }
        };
        if let Some(descriptor) = descriptor
            && index.declaration(&descriptor).is_some()
        {
            return Ok(CharacterReferenceResolution::Resolved(descriptor));
        }
    }

    let maximum = CharacterDefinitionLimits::PRODUCTION.candidates();
    let mut candidate_count = 0_u64;
    let mut wrong_owning_part = false;
    for candidate in index.member_candidates(spelling) {
        budget
            .charge(CharacterDefinitionWorkKind::TypedMemberCandidate)
            .map_err(CharacterReferenceInventoryError::from)?;
        let observed = candidate_count.checked_add(1).ok_or(
            CharacterReferenceInventoryError::ArithmeticOverflow {
                counter: CharacterDefinitionLimitKind::Candidates,
            },
        )?;
        if observed > maximum {
            return Err(CharacterReferenceInventoryError::Limit {
                kind: CharacterDefinitionLimitKind::Candidates,
                observed,
                maximum,
            });
        }
        candidate_count = observed;
        wrong_owning_part |= matches!(
            (&expected, candidate),
            (
                Some(CharacterNominalType::Variant { character, part }),
                CharacterSymbolDescriptor::Variant {
                    character: actual_character,
                    part: actual_part,
                    ..
                }
            ) if actual_character == character && actual_part != part
        );
    }

    if expected.is_none() && candidate_count == 1 {
        let descriptor = index
            .member_candidates(spelling)
            .next()
            .expect("the charged immutable member index contained one candidate")
            .clone();
        return Ok(CharacterReferenceResolution::Resolved(descriptor));
    }
    if candidate_count == 0 {
        admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
        return Ok(CharacterReferenceResolution::Unresolved(
            CharacterDefinitionIssue::UnknownMember {
                spelling: spelling.to_owned(),
                expected,
            },
        ));
    }

    admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
    let candidates = admit_error_payload(budget, index.member_candidates(spelling))
        .map_err(CharacterReferenceInventoryError::from)?;
    let issue = match expected {
        None => CharacterDefinitionIssue::AmbiguousMember {
            spelling: spelling.to_owned(),
            candidates,
        },
        Some(expected) if wrong_owning_part => CharacterDefinitionIssue::WrongOwningPart {
            spelling: spelling.to_owned(),
            expected,
            candidates,
        },
        Some(expected) => CharacterDefinitionIssue::WrongNominalFamily {
            spelling: spelling.to_owned(),
            expected,
            candidates,
        },
    };
    Ok(CharacterReferenceResolution::Unresolved(issue))
}

fn admit_nonresource_error(
    budget: &mut CharacterDefinitionRequestBudget,
    payload_len: usize,
) -> Result<(), CharacterDefinitionResourceError> {
    budget.charge(CharacterDefinitionWorkKind::AdmittedErrorCandidate)?;
    for _ in 0..payload_len {
        budget.charge(CharacterDefinitionWorkKind::AdmittedErrorCandidate)?;
    }
    Ok(())
}

fn admit_error_payload<'a, T: Clone + 'a>(
    budget: &mut CharacterDefinitionRequestBudget,
    candidates: impl Iterator<Item = &'a T>,
) -> Result<Vec<T>, CharacterDefinitionResourceError> {
    let mut payload = Vec::new();
    for candidate in candidates {
        budget.charge(CharacterDefinitionWorkKind::AdmittedErrorCandidate)?;
        payload.push(candidate.clone());
    }
    Ok(payload)
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn intersects_recovery(
    reference: arcweft_lang_syntax::ast::common::TextRange,
    diagnostics: &[ParseError],
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<bool, CharacterReferenceInventoryError> {
    for diagnostic in diagnostics {
        budget
            .charge(CharacterDefinitionWorkKind::ParserFact)
            .map_err(CharacterReferenceInventoryError::from)?;
        let recovery = diagnostic.range();
        if reference.start() < recovery.end() && recovery.start() < reference.end() {
            return Ok(true);
        }
    }
    Ok(false)
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn ensure_world_integrity(
    world: &RegisteredSemanticWorld,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<(), CharacterReferenceInventoryError> {
    let expected_world = world.symbols().world();
    for actual in [
        world.environment().world(),
        world.character_definition_index().world(),
    ] {
        budget
            .charge(CharacterDefinitionWorkKind::IdentityCheck)
            .map_err(CharacterReferenceInventoryError::from)?;
        if actual != expected_world {
            admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
            return Err(CharacterReferenceInventoryError::StaleWorld {
                expected: expected_world.clone(),
                actual: actual.clone(),
            });
        }
    }
    let expected_revision = world.symbols().revision();
    for actual in [
        world.environment().symbol_revision(),
        world.character_definition_index().symbol_revision(),
    ] {
        budget
            .charge(CharacterDefinitionWorkKind::IdentityCheck)
            .map_err(CharacterReferenceInventoryError::from)?;
        if actual != expected_revision {
            admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
            return Err(CharacterReferenceInventoryError::StaleSymbolRevision {
                expected: *expected_revision,
                actual: *actual,
            });
        }
    }
    Ok(())
}

/// Result of a typed character definition query at one byte cursor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDefinitionQueryResult {
    Resolved(CharacterDefinition),
    NotApplicable(CharacterDefinitionNotApplicable),
    Unresolved(CharacterDefinitionIssue),
    Stale(CharacterDefinitionStale),
    Exhausted(CharacterDefinitionResourceError),
    Integrity(CharacterDefinitionIntegrityError),
}

/// Owned core definition result before protocol URI/range adaptation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDefinition {
    descriptor: CharacterSymbolDescriptor,
    origin_selection: SourceSpan,
    declarations: Vec<CharacterDeclarationSource>,
}

impl CharacterDefinition {
    pub const fn descriptor(&self) -> &CharacterSymbolDescriptor {
        &self.descriptor
    }

    pub const fn origin_selection(&self) -> &SourceSpan {
        &self.origin_selection
    }

    pub fn declarations(&self) -> impl ExactSizeIterator<Item = &CharacterDeclarationSource> {
        self.declarations.iter()
    }
}

/// Cursor classification outside a complete character identifier selection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDefinitionNotApplicable {
    Whitespace,
    Delimiter,
    Qualification,
    NonCharacterToken,
    EndBoundary,
    RecoveredSyntax,
}

/// Exact identity mismatch preventing reuse of a request-scoped inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDefinitionStale {
    World {
        expected: ProjectSymbolWorldId,
        actual: ProjectSymbolWorldId,
    },
    SymbolRevision {
        expected: ProjectSymbolRevision,
        actual: ProjectSymbolRevision,
    },
    Document {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    SyntaxSnapshot {
        expected: SourceSnapshotId,
        actual: SourceSnapshotId,
    },
}

/// Bounded resource failure while constructing an owned query result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDefinitionResourceError {
    Limit {
        kind: CharacterDefinitionLimitKind,
        observed: u64,
        maximum: u64,
    },
    ArithmeticOverflow {
        counter: CharacterDefinitionLimitKind,
    },
}

/// Impossible accepted-state inconsistency found by a core query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterDefinitionIntegrityError {
    MissingDeclaration {
        descriptor: CharacterSymbolDescriptor,
    },
    MissingOwnedDocument {
        source: SourceDocumentIdentity,
    },
    AmbiguousCursorFacts {
        source: SourceSpan,
        candidates: Vec<CharacterSymbolDescriptor>,
    },
    InvalidSourceRange {
        source: SourceSpan,
    },
    AcceptedWorldInvariant {
        world: ProjectSymbolWorldId,
        revision: ProjectSymbolRevision,
    },
}
