//! Request-scoped character references and Sans-I/O definition queries.

mod query;
mod request_budget;

pub use query::query_character_definition;
pub use request_budget::{
    CharacterDefinitionBudgetCheckpoint, CharacterDefinitionRequestBudget,
    CharacterDefinitionWorkKind, CharacterDefinitionWorkReceipt,
};

use arcweft_character::{
    id::{CharacterLookId, CharacterPartId, CharacterVariantId},
    symbol::CharacterSymbolDescriptor,
};
use arcweft_lang_hir::{
    expr::HirExprKind,
    identity::ExprId,
    leaf::{HirIdRef, HirIdRefShape, HirIdRefValue, HirShortVariantName},
    module::HirModule,
    project::HirExecutableProjectView,
    source_index::{
        HirExprSourceRole, HirIdRefSourcePart, HirSourcePresence, HirSourceQuery, HirSourceSite,
    },
    symbol::{ProjectSymbolRevision, ProjectSymbolTargetId, ProjectSymbolWorldId},
};
use arcweft_lang_syntax::ast::{
    module_path::{CanonicalModulePath, ModulePathRoot},
    symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
};
use arcweft_source::{SourceDocumentIdentity, SourceSpan, identity::SourceSnapshotId};
use thiserror::Error;

use crate::{
    final_analysis::{CheckedExpressionResolution, CheckedValueResolution, FinalSemanticAnalysis},
    registration::{
        CharacterDeclarationSource, CharacterDefinitionLimitKind, CharacterDefinitionLimits,
        RegisteredSemanticWorld,
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
    syntax_snapshot: SourceSnapshotId,
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
    /// A source-backed owner reference whose typed HIR leaf retained recovery
    /// instead of fabricating a semantic path.
    RecoveredOwner,
    /// A source-backed short member whose typed HIR leaf retained recovery
    /// instead of fabricating a semantic identifier.
    RecoveredLocalMember,
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
    project: HirExecutableProjectView<'a>,
    module: &'a HirModule,
    analysis: &'a FinalSemanticAnalysis,
}

impl<'a> CharacterReferenceInput<'a> {
    pub fn new(
        project: HirExecutableProjectView<'a>,
        module: &'a HirModule,
        analysis: &'a FinalSemanticAnalysis,
    ) -> Self {
        Self {
            project,
            module,
            analysis,
        }
    }

    fn document(&self) -> &arcweft_source::SourceDocument {
        self.module.provenance().document().as_ref()
    }

    const fn module_path(&self) -> &CanonicalModulePath {
        self.module.key().path()
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

    pub const fn syntax_snapshot(&self) -> &SourceSnapshotId {
        &self.syntax_snapshot
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
    #[error("character reference inventory module lease is not owned by its accepted project")]
    ProjectModuleMismatch { module: CanonicalModulePath },
    #[error("character reference inventory semantic analysis belongs to a different generation")]
    SemanticGenerationMismatch,
    #[error("character reference inventory expression owner is not present in its accepted HIR")]
    SemanticOwnerMismatch { owner: ExprId },
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
    let module_path = input.module_path();
    let Some(project_module) = input.project.module(module_path) else {
        admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
        return Err(CharacterReferenceInventoryError::ProjectModuleMismatch {
            module: module_path.clone(),
        });
    };
    if !std::ptr::eq(project_module.as_ref(), input.module) {
        admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
        return Err(CharacterReferenceInventoryError::ProjectModuleMismatch {
            module: module_path.clone(),
        });
    }
    budget
        .charge(CharacterDefinitionWorkKind::IdentityCheck)
        .map_err(CharacterReferenceInventoryError::from)?;
    if input
        .analysis
        .validate_generation(input.project, world.symbols())
        .is_err()
    {
        admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
        return Err(CharacterReferenceInventoryError::SemanticGenerationMismatch);
    }

    let limits = CharacterDefinitionLimits::PRODUCTION;
    let mut facts = Vec::new();
    for (id, checked) in input.analysis.expressions() {
        budget
            .charge(CharacterDefinitionWorkKind::ParserFact)
            .map_err(CharacterReferenceInventoryError::from)?;
        if id.module() != input.module.module_id() {
            continue;
        }
        budget
            .charge(CharacterDefinitionWorkKind::ParserFact)
            .map_err(CharacterReferenceInventoryError::from)?;
        let expression = input
            .module
            .resolve_expr(id)
            .map_err(|_| CharacterReferenceInventoryError::SemanticOwnerMismatch { owner: id })?;
        let fact = match expression.kind() {
            HirExprKind::EntityReference(reference)
                if checked.ty().is_entity_ref_kind(&EntityKind::Character) =>
            {
                owner_fact(&input, id, reference, budget)?
            }
            HirExprKind::ShortVariant(name) => local_member_fact(world, &input, id, name, budget)?,
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
        document: input.document().identity().clone(),
        module: input.module_path().clone(),
        syntax_snapshot: input.module.provenance().source_snapshot().clone(),
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
    input: &CharacterReferenceInput<'_>,
    owner: ExprId,
    reference: &HirIdRefValue,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<Option<CharacterReferenceFact>, CharacterReferenceInventoryError> {
    let Some(reference_span) = expression_source_span(
        input,
        owner,
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::Whole),
        budget,
    )?
    else {
        return Ok(None);
    };
    let (selection_ordinal, form, resolution) = match reference.as_resolved() {
        Some(HirIdRef::Absolute(entity)) => {
            let Some(path) = character_owner_path(entity) else {
                return Ok(None);
            };
            let Some(selection_ordinal) = entity.segment_count().checked_sub(1) else {
                return Ok(None);
            };
            let selection_ordinal = u32::try_from(selection_ordinal).map_err(|_| {
                CharacterReferenceInventoryError::ArithmeticOverflow {
                    counter: CharacterDefinitionLimitKind::Candidates,
                }
            })?;
            let checked = input
                .analysis
                .expression(owner)
                .ok_or(CharacterReferenceInventoryError::SemanticOwnerMismatch { owner })?;
            let CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item)) =
                checked.resolution()
            else {
                return Err(CharacterReferenceInventoryError::SemanticOwnerMismatch { owner });
            };
            let character = item
                .character()
                .ok_or(CharacterReferenceInventoryError::SemanticOwnerMismatch { owner })?;
            budget
                .charge(CharacterDefinitionWorkKind::ProjectSymbolCandidate)
                .map_err(CharacterReferenceInventoryError::from)?;
            let resolution =
                CharacterReferenceResolution::Resolved(CharacterSymbolDescriptor::Owner {
                    character,
                });
            (
                selection_ordinal,
                CharacterReferenceForm::OwnerPath { path },
                resolution,
            )
        }
        Some(HirIdRef::Relative(_) | HirIdRef::FamilyRelative(_)) => return Ok(None),
        None => {
            let Some(selection_ordinal) = recovered_owner_selection_ordinal(reference) else {
                return Ok(None);
            };
            admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
            (
                selection_ordinal,
                CharacterReferenceForm::RecoveredOwner,
                CharacterReferenceResolution::Unresolved(
                    CharacterDefinitionIssue::RecoveredToken {
                        source: reference_span.clone(),
                    },
                ),
            )
        }
    };
    let Some(selection_span) = expression_source_span(
        input,
        owner,
        HirExprSourceRole::EntityReference(HirIdRefSourcePart::SuffixSegment {
            ordinal: selection_ordinal,
        }),
        budget,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(CharacterReferenceFact {
        reference_span,
        selection_span,
        form,
        resolution,
    }))
}

fn character_owner_path(
    reference: &arcweft_lang_hir::leaf::HirEntityReference,
) -> Option<SymbolPath> {
    let segments = reference
        .segments()
        .map(|segment| ProjectSymbolSegment::try_new(segment.to_owned()))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let path = ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, segments).ok()?;
    SymbolPath::try_from(&path).ok()
}

fn recovered_owner_selection_ordinal(reference: &HirIdRefValue) -> Option<u32> {
    let recovery = reference.recovery()?;
    let count = match recovery.shape() {
        HirIdRefShape::Absolute { segment_count } => segment_count,
        HirIdRefShape::Relative {
            suffix_segment_count,
            ..
        }
        | HirIdRefShape::FamilyRelative {
            suffix_segment_count,
            ..
        } => suffix_segment_count,
        HirIdRefShape::Missing => 0,
    };
    count.checked_sub(1)
}

#[allow(
    clippy::result_large_err,
    reason = "inventory failures retain complete typed stale identities"
)]
fn local_member_fact(
    world: &RegisteredSemanticWorld,
    input: &CharacterReferenceInput<'_>,
    owner: ExprId,
    name: &HirShortVariantName,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<Option<CharacterReferenceFact>, CharacterReferenceInventoryError> {
    let Some(reference_span) =
        expression_source_span(input, owner, HirExprSourceRole::Whole, budget)?
    else {
        return Ok(None);
    };
    let Some(selection_span) =
        expression_source_span(input, owner, HirExprSourceRole::ShortVariantName, budget)?
    else {
        return Ok(None);
    };
    let Some(spelling) = name
        .as_resolved()
        .map(arcweft_lang_hir::leaf::HirName::as_str)
    else {
        admit_nonresource_error(budget, 0).map_err(CharacterReferenceInventoryError::from)?;
        return Ok(Some(CharacterReferenceFact {
            reference_span: reference_span.clone(),
            selection_span,
            form: CharacterReferenceForm::RecoveredLocalMember,
            resolution: CharacterReferenceResolution::Unresolved(
                CharacterDefinitionIssue::RecoveredToken {
                    source: reference_span,
                },
            ),
        }));
    };
    let expected = expected_nominal(input.analysis, owner, budget)?;
    let form_expected = expected.clone();
    let form = CharacterReferenceForm::LocalMember {
        spelling: spelling.to_owned(),
        expected: form_expected,
    };
    let resolution = resolve_local_member(world, spelling, expected, budget)?;
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
fn expected_nominal(
    analysis: &FinalSemanticAnalysis,
    owner: ExprId,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<Option<CharacterNominalType>, CharacterReferenceInventoryError> {
    budget
        .charge(CharacterDefinitionWorkKind::ParserFact)
        .map_err(CharacterReferenceInventoryError::from)?;
    Ok(analysis
        .expression(owner)
        .and_then(|checked| checked.ty().character_nominal())
        .cloned())
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
fn expression_source_span(
    input: &CharacterReferenceInput<'_>,
    owner: ExprId,
    role: HirExprSourceRole,
    budget: &mut CharacterDefinitionRequestBudget,
) -> Result<Option<SourceSpan>, CharacterReferenceInventoryError> {
    budget
        .charge(CharacterDefinitionWorkKind::ParserFact)
        .map_err(CharacterReferenceInventoryError::from)?;
    let lookup = input
        .module
        .source_site(
            input.document().identity(),
            HirSourceQuery::Expr { owner, role },
        )
        .map_err(|_| CharacterReferenceInventoryError::SemanticOwnerMismatch { owner })?;
    Ok(match lookup.presence() {
        HirSourcePresence::Present(HirSourceSite::Span(span)) => Some(span.clone()),
        HirSourcePresence::Present(HirSourceSite::Insertion(_))
        | HirSourcePresence::AbsentOptional => None,
    })
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
    AcceptedModuleInvariant {
        module: CanonicalModulePath,
    },
    AcceptedExpressionInvariant {
        owner: ExprId,
    },
    AcceptedSemanticGenerationInvariant,
}
