//! Direct attached `Pattern` lowering into final Pattern and Local arenas.

pub(crate) mod binding_plan;
pub(crate) mod leaf;

use std::collections::BTreeMap;

use arcweft_lang_syntax::attachment::{
    AttachedCandidateNode, AttachedCandidatePatternChild, AttachedCandidatePatternProjection,
    AttachedPatternChild, AttachedPatternNode, AttachedTypeRefNode,
};
use arcweft_lang_syntax::patterns::{
    PatternBindingSyntax, PatternComponentRole, PatternFieldPart, PatternNameSyntax,
    PatternNodePath, PatternNodeStep, PatternPathIssue, PatternRecordFieldSyntax,
    PatternRecordSyntax, PatternRecoveryIssue, PatternRestPart, PatternSequenceRestIssue,
    PatternSequenceRestSyntax, PatternSequenceSyntax, PatternSyntaxKind, PatternSyntaxNode,
    PatternSyntaxState, PatternTypeChildRelation, PatternUnqualifiedVariantForm,
    PatternVariantHead, PatternVariantHeadSyntax, PatternVariantPayloadIssue,
    PatternVariantPayloadSyntax, PatternVariantSyntax,
};

use crate::arena::ArenaReservation;
use crate::diagnostic::{HirRecoveryDiagnostic, HirRecoveryPrimary};
use crate::expr::{HirPoisonState, HirRecoveryIssue};
use crate::identity::{
    LocalGeneration, LocalId, PatternId, ScopeId, SyntheticKey, SyntheticOwner, SyntheticRole,
    TypeId,
};
use crate::lowering::{HirInvariantFailure, HirLowerFailure};
use crate::pattern::{
    HirGenericPatternIssue, HirPattern, HirPatternBinding, HirPatternError, HirPatternField,
    HirPatternKind, HirPatternRecoveryIssue, HirPatternSequenceRest, HirPatternSequenceRestIssue,
    HirUnqualifiedVariantForm, HirVariantPattern, HirVariantPatternHead,
    HirVariantPatternHeadIssue, HirVariantPatternHeadValue, HirVariantPatternName,
    HirVariantPatternNameIssue, HirVariantPatternPayload, HirVariantPatternPayloadIssue,
};
use crate::scope::{HirLocal, HirPatternBindingPolicy};
use crate::source_index::{HirInsertionPoint, HirPatternSourceRole, HirSourceQuery, HirSourceSite};

use self::binding_plan::{RecordFieldDisposition, binding_issue, classify_record_fields};
use self::leaf::{path_value, record_path};

use self::binding_plan::{BindingAllocation, BindingPlan, BindingSite, BindingSiteRole};
use self::leaf::preflight_path_segments;

use super::{
    LocalGenerationLedgerEntry, StagedHirModuleTransaction,
    expression_lowering::CandidateCursor,
    id_ref_projection::id_ref,
    literal_projection::literal,
    name_projection::{name, name_issue},
};

#[derive(Clone)]
pub(super) enum PatternInput<'a> {
    Source(AttachedPatternNode),
    Candidate(AttachedCandidatePatternProjection<'a>),
}

impl PatternInput<'_> {
    fn source_owner_id(&self) -> arcweft_lang_syntax::attachment::SyntaxNodeId {
        match self {
            Self::Source(pattern) => pattern.id(),
            Self::Candidate(pattern) => pattern.source_owner_id(),
        }
    }

    fn path(&self) -> &PatternNodePath {
        match self {
            Self::Source(pattern) => pattern.path(),
            Self::Candidate(pattern) => pattern.path(),
        }
    }

    fn value(&self) -> &PatternSyntaxNode {
        match self {
            Self::Source(pattern) => pattern.value(),
            Self::Candidate(pattern) => (*pattern).value(),
        }
    }

    fn state(&self) -> &PatternSyntaxState {
        match self {
            Self::Source(pattern) => pattern.state(),
            Self::Candidate(pattern) => (*pattern).state(),
        }
    }

    fn whole_source_span(&self) -> arcweft_source::SourceSpan {
        match self {
            Self::Source(pattern) => pattern.whole_source_span(),
            Self::Candidate(pattern) => (*pattern).whole_source_span(),
        }
    }

    fn component(&self, role: PatternComponentRole) -> Option<arcweft_source::SourceSpan> {
        match self {
            Self::Source(pattern) => pattern.component(role),
            Self::Candidate(pattern) => (*pattern).component(role),
        }
    }

    fn snapshot_id(&self) -> &arcweft_lang_syntax::attachment::SyntaxSnapshotId {
        match self {
            Self::Source(pattern) => pattern.snapshot_id(),
            Self::Candidate(pattern) => pattern.snapshot_id(),
        }
    }
}

#[derive(Clone)]
enum TypeInput<'a> {
    Source(AttachedTypeRefNode),
    Candidate(AttachedCandidateNode<'a>),
}

enum PatternAllocationMode<'cursor> {
    Source,
    Candidate(&'cursor mut CandidateCursor),
}

struct PatternLoweringContext<'cursor> {
    outer: PatternId,
    plan: BindingPlan,
    locals: BTreeMap<BindingAllocation, LocalId>,
    generations: BTreeMap<crate::leaf::HirName, LocalGeneration>,
    policy: HirPatternBindingPolicy,
    context_poisoned: bool,
    allocation: PatternAllocationMode<'cursor>,
}

pub(super) struct LoweredPattern {
    pub(super) owner: PatternId,
    pub(super) locals: Box<[LocalId]>,
    pub(super) poisoned: bool,
}

impl StagedHirModuleTransaction<'_> {
    /// Lowers one attached semantic Pattern and every owned Pattern/Type/Local
    /// child through this transaction's sole storage authority.
    ///
    /// The source-backed outer Pattern is reserved before binding preflight.
    /// No Local or child Pattern is staged until every Or alternative and the
    /// outer destructured-binding limit has been validated.
    #[cfg(test)]
    pub(crate) fn lower_attached_pattern(
        &mut self,
        attached: &AttachedPatternNode,
        scope: ScopeId,
    ) -> Result<PatternId, HirLowerFailure> {
        let result = self
            .lower_attached_pattern_root(attached, scope, HirPatternBindingPolicy::PatternBinding)
            .map(|lowered| lowered.owner);
        if result.is_err() {
            self.slots.poison();
        }
        result
    }

    pub(super) fn lower_attached_pattern_binding(
        &mut self,
        attached: &AttachedPatternNode,
        scope: ScopeId,
        policy: HirPatternBindingPolicy,
    ) -> Result<LoweredPattern, HirLowerFailure> {
        let result = self.lower_attached_pattern_root(attached, scope, policy);
        if result.is_err() {
            self.slots.poison();
        }
        result
    }

    fn lower_attached_pattern_root(
        &mut self,
        attached: &AttachedPatternNode,
        scope: ScopeId,
        policy: HirPatternBindingPolicy,
    ) -> Result<LoweredPattern, HirLowerFailure> {
        let input = PatternInput::Source(attached.clone());
        self.validate_pattern_input(&input, scope)?;
        let reservation = self.arenas.patterns().reserve_source(
            &mut self.slots,
            attached.id(),
            HirSourceSite::Span(attached.whole_source_span()),
        )?;
        self.lower_pattern_root_with_reservation(
            &input,
            scope,
            policy,
            reservation,
            PatternAllocationMode::Source,
        )
    }

    pub(super) fn lower_candidate_pattern_binding(
        &mut self,
        attached: AttachedCandidatePatternProjection<'_>,
        scope: ScopeId,
        policy: HirPatternBindingPolicy,
        cursor: &mut CandidateCursor,
    ) -> Result<LoweredPattern, HirLowerFailure> {
        let input = PatternInput::Candidate(attached);
        self.validate_pattern_input(&input, scope)?;
        let ordinal = cursor.take_pattern_ordinal()?;
        let key =
            SyntheticKey::try_new(SyntheticOwner::Expr(cursor.owner()), cursor.role(), ordinal)
                .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
        let site = HirSourceSite::from_attached_span(
            self.request.source().document(),
            &input.whole_source_span(),
        )
        .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
        let reservation = self
            .arenas
            .patterns()
            .reserve_synthetic(&mut self.slots, key, site)?;
        self.lower_pattern_root_with_reservation(
            &input,
            scope,
            policy,
            reservation,
            PatternAllocationMode::Candidate(cursor),
        )
    }

    fn lower_pattern_root_with_reservation(
        &mut self,
        input: &PatternInput<'_>,
        scope: ScopeId,
        policy: HirPatternBindingPolicy,
        reservation: ArenaReservation<PatternId>,
        allocation: PatternAllocationMode<'_>,
    ) -> Result<LoweredPattern, HirLowerFailure> {
        let outer = reservation.id();
        if !reservation.is_first_touch() {
            let owner = self.validate_reused_pattern(outer, scope)?;
            let locals = self
                .pattern_locals
                .get(&owner)
                .cloned()
                .ok_or(HirInvariantFailure::InvalidLocalTimeline)?;
            let poisoned = policy.requires_irrefutable()
                && !binding_plan::pattern_is_irrefutable(input)
                || self
                    .arenas
                    .patterns()
                    .resolve_staged(&self.slots, owner)?
                    .is_poisoned()
                || locals.iter().copied().any(|local| {
                    self.arenas
                        .locals()
                        .resolve_staged(&self.slots, local)
                        .is_ok_and(HirLocal::is_poisoned)
                });
            return Ok(LoweredPattern {
                owner,
                locals,
                poisoned,
            });
        }

        let plan = BindingPlan::build(input, reverse_pattern_child_insertion(self))?;
        let context_poisoned =
            policy.requires_irrefutable() && !binding_plan::pattern_is_irrefutable(input);
        let mut context = PatternLoweringContext {
            outer,
            plan,
            locals: BTreeMap::new(),
            generations: BTreeMap::new(),
            policy,
            context_poisoned,
            allocation,
        };
        let owner = self.lower_reserved_pattern(input, scope, reservation, &mut context)?;
        let locals = context
            .plan
            .ordered_allocations()
            .iter()
            .map(|allocation| {
                context
                    .locals
                    .get(allocation)
                    .copied()
                    .ok_or_else(|| HirInvariantFailure::InvalidLocalTimeline.into())
            })
            .collect::<Result<Vec<_>, HirLowerFailure>>()?
            .into_boxed_slice();
        if self.pattern_locals.insert(owner, locals.clone()).is_some() {
            return Err(HirInvariantFailure::InvalidLocalTimeline.into());
        }
        let poisoned = context.context_poisoned
            || self
                .arenas
                .patterns()
                .resolve_staged(&self.slots, owner)?
                .is_poisoned()
            || locals.iter().copied().any(|local| {
                self.arenas
                    .locals()
                    .resolve_staged(&self.slots, local)
                    .is_ok_and(HirLocal::is_poisoned)
            });
        Ok(LoweredPattern {
            owner,
            locals,
            poisoned,
        })
    }

    fn lower_pattern_child_input(
        &mut self,
        attached: &PatternInput<'_>,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<PatternId, HirLowerFailure> {
        self.validate_pattern_input(attached, scope)?;
        let reservation = match (&mut context.allocation, attached) {
            (PatternAllocationMode::Source, PatternInput::Source(source)) => {
                self.arenas.patterns().reserve_source(
                    &mut self.slots,
                    source.id(),
                    HirSourceSite::Span(source.whole_source_span()),
                )?
            }
            (PatternAllocationMode::Candidate(cursor), PatternInput::Candidate(_)) => {
                let ordinal = cursor.take_pattern_ordinal()?;
                let key = SyntheticKey::try_new(
                    SyntheticOwner::Expr(cursor.owner()),
                    cursor.role(),
                    ordinal,
                )
                .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?;
                let site = HirSourceSite::from_attached_span(
                    self.request.source().document(),
                    &attached.whole_source_span(),
                )
                .map_err(|_| HirInvariantFailure::InvalidSourceSpan)?;
                self.arenas
                    .patterns()
                    .reserve_synthetic(&mut self.slots, key, site)?
            }
            _ => return Err(HirInvariantFailure::InvalidArenaCommit.into()),
        };
        let owner = reservation.id();
        if !reservation.is_first_touch() {
            return self.validate_reused_pattern(owner, scope);
        }
        self.lower_reserved_pattern(attached, scope, reservation, context)
    }

    fn lower_reserved_pattern(
        &mut self,
        attached: &PatternInput<'_>,
        scope: ScopeId,
        reservation: ArenaReservation<PatternId>,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<PatternId, HirLowerFailure> {
        let owner = reservation.id();
        let kind = self.project_pattern(owner, attached, scope, context)?;
        let state = projected_pattern_state(&kind, attached.state(), scope, self);
        let recovery = match &state {
            HirPoisonState::Clean => None,
            HirPoisonState::Poisoned(issue) => Some(issue.clone()),
        };
        let payload = HirPattern::try_new(kind.clone(), scope, state, self)
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;

        if let PatternInput::Source(source) = attached {
            self.source_components.stage_attached_pattern(
                self.request.source(),
                owner,
                source,
                &kind,
            )?;
        }
        if recovery.is_some() {
            let primary = match attached {
                PatternInput::Source(_) => HirRecoveryPrimary::query(HirSourceQuery::Pattern {
                    owner,
                    role: HirPatternSourceRole::Whole,
                }),
                PatternInput::Candidate(_) => {
                    HirRecoveryPrimary::owner_whole(SyntheticOwner::Pattern(owner))
                }
            };
            self.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
                SyntheticOwner::Pattern(owner),
                primary,
                HirSourceSite::Span(attached.whole_source_span()),
            ));
        }
        self.arenas
            .patterns()
            .finalize(&mut self.slots, reservation, payload)
            .map_err(HirLowerFailure::from)
    }

    fn project_pattern(
        &mut self,
        owner: PatternId,
        attached: &PatternInput<'_>,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<HirPatternKind, HirLowerFailure> {
        let children = attached_children(attached, reverse_pattern_child_insertion(self))?;
        match attached.value().kind() {
            PatternSyntaxKind::Binding(binding) => {
                self.project_node_binding(owner, attached, binding, scope, false, context)
            }
            PatternSyntaxKind::MutableBinding(binding) => {
                self.project_node_binding(owner, attached, binding, scope, true, context)
            }
            PatternSyntaxKind::Literal(value) => Ok(HirPatternKind::Literal(literal(value)?)),
            PatternSyntaxKind::EntityReference(value) => {
                Ok(HirPatternKind::EntityReference(id_ref(value)?))
            }
            PatternSyntaxKind::Variant(value) => {
                self.project_variant(value, &children, scope, context)
            }
            PatternSyntaxKind::Discard => Ok(HirPatternKind::Discard),
            PatternSyntaxKind::Tuple(elements) => Ok(HirPatternKind::Tuple {
                elements: self.lower_indexed_patterns(&children, elements.len(), scope, context)?,
            }),
            PatternSyntaxKind::Record(record) => {
                self.project_record(owner, attached, record, &children, scope, context)
            }
            PatternSyntaxKind::BracketSequence(sequence) => {
                self.project_sequence(owner, attached, sequence, &children, scope, context)
            }
            PatternSyntaxKind::WholeBinding { binding, .. } => {
                self.project_whole_binding(owner, attached, binding, &children, scope, context)
            }
            PatternSyntaxKind::Or(alternatives) => Ok(HirPatternKind::Or {
                alternatives: self.lower_indexed_patterns(
                    &children,
                    alternatives.len(),
                    scope,
                    context,
                )?,
            }),
            PatternSyntaxKind::TypedBinding(binding) => {
                self.project_typed_binding(owner, attached, binding, &children, scope, context)
            }
            PatternSyntaxKind::Error => Ok(HirPatternKind::Error(HirPatternError::new(
                HirGenericPatternIssue::UnclassifiedSyntax,
            ))),
        }
    }

    fn project_node_binding(
        &mut self,
        owner: PatternId,
        attached: &PatternInput<'_>,
        binding: &PatternBindingSyntax,
        scope: ScopeId,
        mutable: bool,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<HirPatternKind, HirLowerFailure> {
        let binding = self.project_binding(
            owner,
            attached,
            BindingSiteRole::Node,
            binding,
            scope,
            None,
            mutable,
            context,
        )?;
        Ok(if mutable {
            HirPatternKind::MutableBinding(binding)
        } else {
            HirPatternKind::Binding(binding)
        })
    }

    fn project_variant(
        &mut self,
        value: &PatternVariantSyntax,
        children: &AttachedChildren<'_>,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<HirPatternKind, HirLowerFailure> {
        let head = match value.head() {
            PatternVariantHeadSyntax::Resolved(PatternVariantHead::Qualified(path)) => {
                HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Qualified(path_value(
                    path,
                )?))
            }
            PatternVariantHeadSyntax::Resolved(PatternVariantHead::Unqualified(form)) => {
                HirVariantPatternHeadValue::Resolved(HirVariantPatternHead::Unqualified(
                    match form {
                        PatternUnqualifiedVariantForm::DotShorthand => {
                            HirUnqualifiedVariantForm::DotShorthand
                        }
                        PatternUnqualifiedVariantForm::BareExpectedType => {
                            HirUnqualifiedVariantForm::BareExpectedType
                        }
                    },
                ))
            }
            PatternVariantHeadSyntax::Recovered(recovery) => {
                preflight_path_segments(recovery.segments().iter().map(AsRef::as_ref))?;
                let segment_count = u32::try_from(recovery.segments().len())
                    .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                match recovery.issue() {
                    PatternPathIssue::InvalidRootDepth => {
                        return Err(HirInvariantFailure::InvalidArenaCommit.into());
                    }
                    PatternPathIssue::MissingSegment | PatternPathIssue::InvalidSegment { .. } => {}
                }
                HirVariantPatternHeadValue::Recovered(
                    HirVariantPatternHeadIssue::InvalidQualifiedPath { segment_count },
                )
            }
            PatternVariantHeadSyntax::Absent => {
                HirVariantPatternHeadValue::Recovered(HirVariantPatternHeadIssue::Missing)
            }
        };
        let variant_name = match value.name() {
            PatternNameSyntax::Resolved(value) => HirVariantPatternName::Resolved(name(value)?),
            PatternNameSyntax::Recovered(issue) => HirVariantPatternName::Recovered(
                HirVariantPatternNameIssue::Invalid(name_issue(issue)),
            ),
            PatternNameSyntax::Absent => {
                HirVariantPatternName::Recovered(HirVariantPatternNameIssue::Missing)
            }
        };
        let payload = self.project_variant_payload(value.payload(), children, scope, context)?;
        Ok(HirPatternKind::Variant(
            HirVariantPattern::try_new(head, variant_name, payload, scope, self)
                .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?,
        ))
    }

    fn project_variant_payload(
        &mut self,
        value: &PatternVariantPayloadSyntax,
        children: &AttachedChildren<'_>,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<HirVariantPatternPayload, HirLowerFailure> {
        match value {
            PatternVariantPayloadSyntax::Absent => Ok(HirVariantPatternPayload::Absent),
            PatternVariantPayloadSyntax::Resolved(_) => Ok(HirVariantPatternPayload::Pattern(
                self.lower_pattern_child(
                    children,
                    PatternNodeStep::VariantPayload,
                    scope,
                    context,
                )?,
            )),
            PatternVariantPayloadSyntax::Recovered {
                value: Some(_),
                issue,
            } => Ok(HirVariantPatternPayload::Recovered {
                pattern: Some(self.lower_pattern_child(
                    children,
                    PatternNodeStep::VariantPayload,
                    scope,
                    context,
                )?),
                issue: variant_payload_issue(issue),
            }),
            PatternVariantPayloadSyntax::Recovered { value: None, issue } => {
                Ok(HirVariantPatternPayload::Recovered {
                    pattern: None,
                    issue: variant_payload_issue(issue),
                })
            }
        }
    }

    fn project_record(
        &mut self,
        owner: PatternId,
        attached: &PatternInput<'_>,
        record: &PatternRecordSyntax,
        children: &AttachedChildren<'_>,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<HirPatternKind, HirLowerFailure> {
        let dispositions = classify_record_fields(record.fields())?;
        let fields = record
            .fields()
            .iter()
            .zip(dispositions)
            .enumerate()
            .map(|(index, (field, disposition))| {
                let ordinal =
                    u32::try_from(index).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                self.project_record_field(
                    owner,
                    attached,
                    field,
                    disposition,
                    children,
                    scope,
                    ordinal,
                    context,
                )
            })
            .collect::<Result<Vec<_>, HirLowerFailure>>()?;
        Ok(HirPatternKind::Record {
            path: record_path(record.path())?,
            fields: fields.into_boxed_slice(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn project_record_field(
        &mut self,
        owner: PatternId,
        attached: &PatternInput<'_>,
        field: &PatternRecordFieldSyntax,
        disposition: RecordFieldDisposition,
        children: &AttachedChildren<'_>,
        scope: ScopeId,
        ordinal: u32,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<HirPatternField, HirLowerFailure> {
        match (field, disposition) {
            (
                PatternRecordFieldSyntax::Explicit { .. },
                RecordFieldDisposition::Explicit { name },
            ) => Ok(HirPatternField::Explicit {
                name,
                pattern: self.lower_pattern_child(
                    children,
                    PatternNodeStep::RecordField(ordinal),
                    scope,
                    context,
                )?,
            }),
            (
                PatternRecordFieldSyntax::Shorthand(binding),
                RecordFieldDisposition::Shorthand { name },
            ) => Ok(HirPatternField::Shorthand {
                name: name.clone(),
                local: self.allocate_binding_local(
                    owner,
                    attached,
                    BindingSiteRole::RecordShorthand(ordinal),
                    binding,
                    name,
                    scope,
                    None,
                    false,
                    context,
                )?,
            }),
            (PatternRecordFieldSyntax::Rest(binding), RecordFieldDisposition::Rest) => {
                let binding = binding
                    .as_ref()
                    .map(|binding| {
                        let source_name = binding
                            .name()
                            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
                        self.allocate_binding_local(
                            owner,
                            attached,
                            BindingSiteRole::RecordRest(ordinal),
                            binding,
                            name(source_name)?,
                            scope,
                            None,
                            false,
                            context,
                        )
                    })
                    .transpose()?;
                Ok(HirPatternField::Rest { binding })
            }
            (_, RecordFieldDisposition::Invalid(issue)) => Ok(HirPatternField::Invalid { issue }),
            _ => Err(HirInvariantFailure::InvalidArenaCommit.into()),
        }
    }

    fn project_sequence(
        &mut self,
        owner: PatternId,
        attached: &PatternInput<'_>,
        sequence: &PatternSequenceSyntax,
        children: &AttachedChildren<'_>,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<HirPatternKind, HirLowerFailure> {
        let elements =
            self.lower_indexed_patterns(children, sequence.elements().len(), scope, context)?;
        let rest = match sequence.rest() {
            PatternSequenceRestSyntax::Absent => HirPatternSequenceRest::Absent,
            PatternSequenceRestSyntax::Unbound => HirPatternSequenceRest::Unbound,
            PatternSequenceRestSyntax::Binding(binding) => HirPatternSequenceRest::Bound(
                self.project_rest_local(owner, attached, binding, scope, context)?,
            ),
            PatternSequenceRestSyntax::Recovered { binding, .. } => match binding {
                Some(binding @ PatternBindingSyntax::Resolved(_)) => HirPatternSequenceRest::Bound(
                    self.project_rest_local(owner, attached, binding, scope, context)?,
                ),
                Some(PatternBindingSyntax::Recovered(issue)) => HirPatternSequenceRest::Recovered(
                    HirPatternSequenceRestIssue::InvalidBinding(binding_issue(issue)),
                ),
                None => recovered_sequence_rest(sequence.rest()),
            },
        };
        Ok(HirPatternKind::BracketSequence { elements, rest })
    }

    fn project_whole_binding(
        &mut self,
        owner: PatternId,
        attached: &PatternInput<'_>,
        binding: &PatternBindingSyntax,
        children: &AttachedChildren<'_>,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<HirPatternKind, HirLowerFailure> {
        let binding = self.project_binding(
            owner,
            attached,
            BindingSiteRole::Node,
            binding,
            scope,
            None,
            false,
            context,
        )?;
        let pattern =
            self.lower_pattern_child(children, PatternNodeStep::NestedPattern, scope, context)?;
        Ok(HirPatternKind::WholeBinding { binding, pattern })
    }

    fn project_typed_binding(
        &mut self,
        owner: PatternId,
        attached: &PatternInput<'_>,
        binding: &PatternBindingSyntax,
        children: &AttachedChildren<'_>,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<HirPatternKind, HirLowerFailure> {
        let ty = self.lower_type_child(children, scope, context)?;
        Ok(HirPatternKind::TypedBinding {
            binding: self.project_binding(
                owner,
                attached,
                BindingSiteRole::Node,
                binding,
                scope,
                Some(ty),
                false,
                context,
            )?,
            ty,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn project_binding(
        &mut self,
        owner: PatternId,
        attached: &PatternInput<'_>,
        role: BindingSiteRole,
        binding: &PatternBindingSyntax,
        scope: ScopeId,
        annotation: Option<TypeId>,
        mutable: bool,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<HirPatternBinding, HirLowerFailure> {
        match binding {
            PatternBindingSyntax::Resolved(source_name) => {
                let allocation = context
                    .plan
                    .allocation(&BindingSite::new(attached.path(), role))?;
                let projected_name = name(source_name)?;
                let local = self.allocate_binding_local_with_allocation(
                    owner,
                    attached,
                    role,
                    allocation,
                    projected_name.clone(),
                    scope,
                    annotation,
                    mutable,
                    context,
                )?;
                Ok(HirPatternBinding::Bound {
                    name: projected_name,
                    local,
                })
            }
            PatternBindingSyntax::Recovered(issue) => Ok(HirPatternBinding::Recovered {
                issue: binding_issue(issue),
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn allocate_binding_local(
        &mut self,
        owner: PatternId,
        attached: &PatternInput<'_>,
        role: BindingSiteRole,
        binding: &PatternBindingSyntax,
        name: crate::leaf::HirName,
        scope: ScopeId,
        annotation: Option<TypeId>,
        mutable: bool,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<LocalId, HirLowerFailure> {
        if binding.name().is_none() {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        let allocation = context
            .plan
            .allocation(&BindingSite::new(attached.path(), role))?;
        self.allocate_binding_local_with_allocation(
            owner, attached, role, allocation, name, scope, annotation, mutable, context,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn allocate_binding_local_with_allocation(
        &mut self,
        immediate_owner: PatternId,
        attached: &PatternInput<'_>,
        role: BindingSiteRole,
        allocation: BindingAllocation,
        name: crate::leaf::HirName,
        scope: ScopeId,
        annotation: Option<TypeId>,
        mutable: bool,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<LocalId, HirLowerFailure> {
        if let Some(local) = context.locals.get(&allocation).copied() {
            let retained = self
                .arenas
                .locals()
                .resolve_staged(&self.slots, local)
                .map_err(HirLowerFailure::from)?;
            if retained.scope() != scope || retained.name() != &name {
                return Err(HirInvariantFailure::InvalidLocalTimeline.into());
            }
            return Ok(local);
        }
        let (source_key, pattern_owner, duplicate) = match allocation {
            BindingAllocation::Ordinary { ordinal, poisoned } => (
                Some((context.outer, SyntheticRole::DestructuredBinding, ordinal)),
                context.outer,
                poisoned,
            ),
            BindingAllocation::Rest {
                ref canonical,
                poisoned,
            } => {
                if canonical != &BindingSite::new(attached.path(), role) {
                    return Err(HirInvariantFailure::InvalidLocalTimeline.into());
                }
                (
                    Some((immediate_owner, SyntheticRole::PatternRest, 0)),
                    immediate_owner,
                    poisoned,
                )
            }
        };
        let key = match &mut context.allocation {
            PatternAllocationMode::Source => {
                let (owner, role, ordinal) =
                    source_key.ok_or(HirInvariantFailure::InvalidSlotCommit)?;
                SyntheticKey::try_new(SyntheticOwner::Pattern(owner), role, ordinal)
                    .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?
            }
            PatternAllocationMode::Candidate(cursor) => {
                let ordinal = cursor.take_local_ordinal()?;
                SyntheticKey::try_new(SyntheticOwner::Expr(cursor.owner()), cursor.role(), ordinal)
                    .map_err(|_| HirInvariantFailure::InvalidSlotCommit)?
            }
        };
        let source_site = self.binding_source_site(attached, role)?;
        let binding_name_start = Self::binding_name_start(attached, role)?;
        let (generation, advances_ledger) =
            self.next_binding_generation(scope, &name, binding_name_start, context)?;
        let poisoned = duplicate
            || context.context_poisoned
            || context.policy.forbids_mutable() && mutable
            || context.policy.reserves_result() && name.as_str() == "result";
        let reservation =
            self.arenas
                .locals()
                .reserve_synthetic(&mut self.slots, key, source_site.clone())?;
        let payload = HirLocal::try_new(
            scope,
            context.policy.local_kind(),
            name.clone(),
            generation,
            Some(pattern_owner),
            annotation,
            mutable,
            poisoned,
        )
        .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
        let local = self
            .arenas
            .locals()
            .finalize(&mut self.slots, reservation, payload)?;
        if advances_ledger {
            self.local_timelines
                .entry((scope, name.clone()))
                .or_default()
                .publish(LocalGenerationLedgerEntry::new(
                    local,
                    generation,
                    binding_name_start,
                ))?;
            context.generations.insert(name, generation);
        }
        if poisoned {
            let owner = SyntheticOwner::Local(local);
            self.stage_recovery_diagnostic(HirRecoveryDiagnostic::new(
                owner,
                HirRecoveryPrimary::owner_whole(owner),
                source_site,
            ));
        }
        context.locals.insert(allocation, local);
        Ok(local)
    }

    fn next_binding_generation(
        &self,
        scope: ScopeId,
        name: &crate::leaf::HirName,
        binding_name_start: usize,
        context: &PatternLoweringContext<'_>,
    ) -> Result<(LocalGeneration, bool), HirLowerFailure> {
        if let Some(generation) = context.generations.get(name).copied() {
            return Ok((generation, false));
        }
        let generation = self.next_sequential_local_generation(scope, name, binding_name_start)?;
        Ok((generation, true))
    }

    fn project_rest_local(
        &mut self,
        owner: PatternId,
        attached: &PatternInput<'_>,
        binding: &PatternBindingSyntax,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<LocalId, HirLowerFailure> {
        let source_name = binding
            .name()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        let projected_name = name(source_name)?;
        self.allocate_binding_local(
            owner,
            attached,
            BindingSiteRole::SequenceRest,
            binding,
            projected_name,
            scope,
            None,
            false,
            context,
        )
    }

    fn binding_source_site(
        &self,
        attached: &PatternInput<'_>,
        role: BindingSiteRole,
    ) -> Result<HirSourceSite, HirLowerFailure> {
        let component = match role {
            BindingSiteRole::Node => match attached.value().kind() {
                PatternSyntaxKind::WholeBinding { .. } => PatternComponentRole::WholeBindingName,
                _ => PatternComponentRole::Name,
            },
            BindingSiteRole::RecordShorthand(field) => PatternComponentRole::PatternField {
                field,
                part: PatternFieldPart::Name,
            },
            BindingSiteRole::RecordRest(field) => PatternComponentRole::PatternField {
                field,
                part: PatternFieldPart::Whole,
            },
            BindingSiteRole::SequenceRest => {
                PatternComponentRole::SequenceRest(PatternRestPart::Whole)
            }
        };
        let span = attached
            .component(component)
            .ok_or(HirInvariantFailure::InvalidSourceIndex)?;
        let offset = match role {
            BindingSiteRole::RecordRest(_) | BindingSiteRole::SequenceRest => span.range().end(),
            BindingSiteRole::Node | BindingSiteRole::RecordShorthand(_) => span.range().start(),
        };
        HirInsertionPoint::try_new(self.request.source().document(), offset)
            .map(HirSourceSite::Insertion)
            .map_err(|_| HirInvariantFailure::InvalidSourceSpan.into())
    }

    fn binding_name_start(
        attached: &PatternInput<'_>,
        role: BindingSiteRole,
    ) -> Result<usize, HirLowerFailure> {
        let component = match role {
            BindingSiteRole::Node => match attached.value().kind() {
                PatternSyntaxKind::WholeBinding { .. } => PatternComponentRole::WholeBindingName,
                _ => PatternComponentRole::Name,
            },
            BindingSiteRole::RecordShorthand(field) => PatternComponentRole::PatternField {
                field,
                part: PatternFieldPart::Name,
            },
            BindingSiteRole::RecordRest(field) => PatternComponentRole::PatternField {
                field,
                part: PatternFieldPart::RestBinding,
            },
            BindingSiteRole::SequenceRest => {
                PatternComponentRole::SequenceRest(PatternRestPart::Binding)
            }
        };
        attached
            .component(component)
            .map(|span| span.range().start())
            .ok_or_else(|| HirInvariantFailure::InvalidSourceIndex.into())
    }

    fn lower_indexed_patterns(
        &mut self,
        children: &AttachedChildren<'_>,
        count: usize,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<Box<[PatternId]>, HirLowerFailure> {
        (0..count)
            .map(|index| {
                let ordinal =
                    u32::try_from(index).map_err(|_| HirInvariantFailure::InvalidArenaCommit)?;
                self.lower_pattern_child(
                    children,
                    PatternNodeStep::Element(ordinal),
                    scope,
                    context,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }

    fn lower_pattern_child(
        &mut self,
        children: &AttachedChildren<'_>,
        step: PatternNodeStep,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<PatternId, HirLowerFailure> {
        let attached = children
            .patterns
            .get(&step)
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        self.lower_pattern_child_input(attached, scope, context)
    }

    fn lower_type_child(
        &mut self,
        children: &AttachedChildren<'_>,
        scope: ScopeId,
        context: &mut PatternLoweringContext<'_>,
    ) -> Result<TypeId, HirLowerFailure> {
        let attached = children
            .typed_binding
            .as_ref()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?;
        match attached {
            TypeInput::Source(attached) => self.lower_attached_type(attached, scope),
            TypeInput::Candidate(attached) => {
                let PatternAllocationMode::Candidate(cursor) = &mut context.allocation else {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                };
                self.lower_candidate_type(*attached, scope, cursor)
            }
        }
    }

    fn validate_pattern_input(
        &self,
        attached: &PatternInput<'_>,
        scope: ScopeId,
    ) -> Result<(), HirLowerFailure> {
        if attached.snapshot_id() != self.request.source().snapshot_id() {
            return Err(HirLowerFailure::StaleSource {
                current: self.request.source().snapshot_id().clone(),
                supplied: attached.snapshot_id().clone(),
            });
        }
        let source_span = attached.whole_source_span();
        if source_span.source() != self.request.source().document().identity() {
            return Err(HirLowerFailure::SourceIdentityMismatch {
                expected: self.request.source().document().identity().clone(),
                actual: source_span.source().clone(),
            });
        }
        if !crate::pattern::HirPatternResolver::scope_is_live(self, scope) {
            return Err(HirInvariantFailure::InvalidArenaCommit.into());
        }
        Ok(())
    }

    fn validate_reused_pattern(
        &mut self,
        owner: PatternId,
        scope: ScopeId,
    ) -> Result<PatternId, HirLowerFailure> {
        let retained = self
            .arenas
            .patterns()
            .resolve_staged(&self.slots, owner)
            .map_err(HirLowerFailure::from)?;
        if retained.scope() == scope {
            Ok(owner)
        } else {
            Err(HirInvariantFailure::InvalidArenaCommit.into())
        }
    }

    #[cfg(test)]
    fn reverse_pattern_child_insertion_for_test(&mut self) {
        self.reverse_pattern_child_insertion = true;
    }
}

/// Re-derives the exact final poison state from the parser-owned recovery
/// inventory and the already projected semantic child graph.
pub(crate) fn projected_pattern_state<R: crate::pattern::HirPatternResolver + ?Sized>(
    kind: &HirPatternKind,
    source: &PatternSyntaxState,
    scope: ScopeId,
    resolver: &R,
) -> HirPoisonState {
    let inferred = kind.inferred_state(scope, resolver);
    let override_issue = source.issues().iter().find_map(container_recovery_issue);
    match (&inferred, override_issue) {
        (
            HirPoisonState::Clean
            | HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(
                HirPatternRecoveryIssue::RecoveredChild { .. },
            )),
            Some(issue),
        ) => HirPoisonState::Poisoned(HirRecoveryIssue::InvalidPattern(issue)),
        _ => inferred,
    }
}

#[cfg(test)]
const fn reverse_pattern_child_insertion(transaction: &StagedHirModuleTransaction<'_>) -> bool {
    transaction.reverse_pattern_child_insertion
}

#[cfg(not(test))]
const fn reverse_pattern_child_insertion(_: &StagedHirModuleTransaction<'_>) -> bool {
    false
}

struct AttachedChildren<'a> {
    patterns: BTreeMap<PatternNodeStep, PatternInput<'a>>,
    typed_binding: Option<TypeInput<'a>>,
}

fn attached_children<'a>(
    attached: &PatternInput<'a>,
    reverse_child_insertion: bool,
) -> Result<AttachedChildren<'a>, HirLowerFailure> {
    let mut patterns = BTreeMap::new();
    let mut typed_binding = None;
    let mut children = match attached {
        PatternInput::Source(attached) => attached
            .children()
            .map_err(|_| HirInvariantFailure::InvalidArenaCommit)?
            .into_iter()
            .map(|child| match child {
                AttachedPatternChild::Pattern { step, node } => {
                    CandidateOrSourcePatternChild::Pattern {
                        step,
                        pattern: PatternInput::Source(node),
                    }
                }
                AttachedPatternChild::Type { relation, node } => {
                    CandidateOrSourcePatternChild::Type {
                        relation,
                        ty: TypeInput::Source(node),
                    }
                }
            })
            .collect::<Vec<_>>(),
        PatternInput::Candidate(attached) => attached
            .children()
            .ok_or(HirInvariantFailure::InvalidArenaCommit)?
            .into_iter()
            .map(|child| match child {
                AttachedCandidatePatternChild::Pattern { step, projection } => {
                    CandidateOrSourcePatternChild::Pattern {
                        step,
                        pattern: PatternInput::Candidate(projection),
                    }
                }
                AttachedCandidatePatternChild::Type { relation, node } => {
                    CandidateOrSourcePatternChild::Type {
                        relation,
                        ty: TypeInput::Candidate(node),
                    }
                }
            })
            .collect::<Vec<_>>(),
    };
    if reverse_child_insertion {
        children.reverse();
    }
    for child in children {
        match child {
            CandidateOrSourcePatternChild::Pattern { step, pattern } => {
                if patterns.insert(step, pattern).is_some() {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
            }
            CandidateOrSourcePatternChild::Type { relation, ty } => {
                if relation != PatternTypeChildRelation::TypedBinding || typed_binding.is_some() {
                    return Err(HirInvariantFailure::InvalidArenaCommit.into());
                }
                typed_binding = Some(ty);
            }
        }
    }
    Ok(AttachedChildren {
        patterns,
        typed_binding,
    })
}

enum CandidateOrSourcePatternChild<'a> {
    Pattern {
        step: PatternNodeStep,
        pattern: PatternInput<'a>,
    },
    Type {
        relation: PatternTypeChildRelation,
        ty: TypeInput<'a>,
    },
}

fn recovered_sequence_rest(rest: &PatternSequenceRestSyntax) -> HirPatternSequenceRest {
    let issue = rest.issues().iter().find_map(|issue| match issue {
        PatternSequenceRestIssue::InvalidBinding(issue) => Some(
            HirPatternSequenceRestIssue::InvalidBinding(binding_issue(issue)),
        ),
        PatternSequenceRestIssue::MultipleRest { .. } => None,
    });
    match issue {
        Some(issue) => HirPatternSequenceRest::Recovered(issue),
        None => HirPatternSequenceRest::Unbound,
    }
}

const fn variant_payload_issue(
    issue: &PatternVariantPayloadIssue,
) -> HirVariantPatternPayloadIssue {
    match issue {
        PatternVariantPayloadIssue::MissingPattern => HirVariantPatternPayloadIssue::MissingPattern,
        PatternVariantPayloadIssue::MissingCloseDelimiter => {
            HirVariantPatternPayloadIssue::MissingCloseDelimiter
        }
        PatternVariantPayloadIssue::InvalidPattern => HirVariantPatternPayloadIssue::InvalidPattern,
    }
}

fn container_recovery_issue(issue: &PatternRecoveryIssue) -> Option<HirPatternRecoveryIssue> {
    match issue {
        PatternRecoveryIssue::MissingCloseDelimiter => {
            Some(HirPatternRecoveryIssue::MissingCloseDelimiter)
        }
        PatternRecoveryIssue::MissingOrAlternative { ordinal } => {
            Some(HirPatternRecoveryIssue::MissingOrAlternative { ordinal: *ordinal })
        }
        PatternRecoveryIssue::SequenceRest(PatternSequenceRestIssue::MultipleRest { ordinal }) => {
            Some(HirPatternRecoveryIssue::SequenceRest(
                HirPatternSequenceRestIssue::MultipleRest { ordinal: *ordinal },
            ))
        }
        PatternRecoveryIssue::MissingPattern
        | PatternRecoveryIssue::UnexpectedPattern
        | PatternRecoveryIssue::Binding(_)
        | PatternRecoveryIssue::Literal(_)
        | PatternRecoveryIssue::EntityReference(_)
        | PatternRecoveryIssue::VariantName(_)
        | PatternRecoveryIssue::VariantHead(_)
        | PatternRecoveryIssue::VariantPayload(_)
        | PatternRecoveryIssue::InvalidRecordField { .. }
        | PatternRecoveryIssue::OrBindings(_)
        | PatternRecoveryIssue::SequenceRest(PatternSequenceRestIssue::InvalidBinding(_))
        | PatternRecoveryIssue::InvalidType => None,
    }
}

#[cfg(test)]
#[path = "pattern_lowering/tests.rs"]
mod tests;
