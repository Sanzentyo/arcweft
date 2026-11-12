//! Frozen producer projections and correlated, separately recorded value uses.

use std::{collections::BTreeMap, marker::PhantomData, rc::Rc, sync::Arc};

use super::super::{OriginalProducerContext, TypeConstraintParameterScope};
use super::{
    ConstraintClosurePolicy, ConstraintDomain, ConstraintPath, SourceProbeTerm,
    SuspendedConstraintFrontier, TypeConstraintAccounting, TypeConstraintContext,
    TypeConstraintError, TypeConstraintRejection, project_type, resolve_probe_term,
};
use crate::{
    effect_row::{
        EffectPredicate,
        prepared::{PreparedEffectFailure, PreparedEffectGraph, PreparedEffectPredicate},
    },
    types::{
        ArrayLength, GenericConstReference, GenericScope, GenericTypeReference, PreparedEffects,
        PreparedTypeRetentionFailure, TypeInstantiationError, TypeKind, TypeProjectionError,
    },
};

mod original;
#[cfg(test)]
mod tests;
pub(crate) use original::OriginalConstraintForest;

#[derive(Debug, thiserror::Error)]
pub(crate) enum PreparedValueFailure {
    #[error(transparent)]
    Constraint(#[from] TypeConstraintError),
    #[error(transparent)]
    Instantiation(#[from] TypeInstantiationError),
    #[error(transparent)]
    Effect(#[from] PreparedEffectFailure<TypeConstraintError>),
    #[error("prepared value reference belongs to another component")]
    ForeignIssuer,
    #[error("prepared value reference is absent from its component")]
    StaleReference,
    #[error("prepared value-use ticket is in the wrong protocol state")]
    UseState,
    #[error("prepared value-use result contradicts unavailable inputs")]
    InputState,
    #[error("an independently registered producer contains a parent-dependent frontier")]
    DependentRoot,
    #[error("one producer contains frontiers from different semantic applications")]
    ProducerApplication,
    #[error("one producer contains frontiers from different original source contexts")]
    ProducerContext,
    #[error("prepared value-use output belongs to another source context")]
    UseContext,
    #[error("prepared value ordinal overflow")]
    OrdinalOverflow,
}

#[derive(Debug)]
struct ValueIssuer;
struct UseLease;

#[derive(Clone, Copy, Eq, PartialEq)]
enum UsePhase {
    Open,
    Pending(u64),
    Exhausted,
    Failed,
}

/// Operational identity only. It contains no result type or semantic equality
/// certificate, and cannot mutate the initializer's original constraints.
pub(crate) struct PreparedValueRef<D> {
    issuer: Rc<ValueIssuer>,
    ordinal: u64,
    domain: PhantomData<fn() -> D>,
}

impl<D> Clone for PreparedValueRef<D> {
    fn clone(&self) -> Self {
        Self {
            issuer: Rc::clone(&self.issuer),
            ordinal: self.ordinal,
            domain: PhantomData,
        }
    }
}

impl<D> PartialEq for PreparedValueRef<D> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.issuer, &other.issuer) && self.ordinal == other.ordinal
    }
}
impl<D> Eq for PreparedValueRef<D> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedValueUnavailable {
    RejectedProducer,
    AmbiguousProducer,
    RejectedUse,
    BlockedUse,
    Deferred,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProducerBranch {
    Constraint { frontier: usize, path: usize },
    Rejected,
    Ambiguous,
}

type OriginAssignments = Box<[(u64, ProducerBranch)]>;

struct ValueAlternative {
    origins: OriginAssignments,
    predicate: PreparedEffectPredicate,
    preparation: usize,
    value: ValueContent,
}

enum ValueContent {
    Available { ty: TypeKind<PreparedEffects> },
    Unavailable(PreparedValueUnavailable),
    UnresolvedProjection(TypeConstraintRejection),
}

enum ValueDefinition<D: ConstraintDomain> {
    Constraint {
        frontiers: Box<[SuspendedConstraintFrontier<D>]>,
    },
    Intrinsic,
    Use(Box<[PreparedValueRef<D>]>),
}

/// Prepared projection and original proof terms have distinct roles: use
/// checking reads the former; eligibility/completion retains the latter.
struct RegisteredValue<D: ConstraintDomain, P> {
    original: OriginalProducerContext,
    definition: ValueDefinition<D>,
    preparations: Box<[P]>,
    alternatives: Box<[ValueAlternative]>,
}

/// Read-only original inference input, including parameter eligibility and
/// absent bindings. A projected value type alone cannot replace this authority.
/// This is neither a completed instantiation nor permission to bind a producer
/// again from one of its later uses.
pub(crate) struct PreparedValueOriginalConstraint<'a, D: ConstraintDomain> {
    original: &'a super::super::OriginalProducerContext,
    application: D::Application,
    parameters: &'a TypeConstraintParameterScope,
    path: &'a ConstraintPath<D>,
}

impl<D: ConstraintDomain> PreparedValueOriginalConstraint<'_, D> {
    pub(crate) fn original_context(&self) -> &super::super::OriginalProducerContext {
        self.original
    }
    pub(crate) fn application(&self) -> D::Application {
        self.application
    }

    pub(crate) fn scope(&self) -> &GenericScope {
        self.original.scope()
    }

    pub(crate) fn parameters(&self) -> &TypeConstraintParameterScope {
        self.parameters
    }

    pub(crate) fn type_bindings(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &GenericTypeReference,
            super::super::normalization::ConstraintBindingView<'_, TypeKind>,
        ),
    > {
        self.path.bindings.iter().map(|(parameter, value)| {
            let scope = value.scope();
            (
                parameter,
                super::super::normalization::ConstraintBindingView {
                    value: value.value(),
                    scope,
                },
            )
        })
    }

    pub(crate) fn const_bindings(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            &GenericConstReference,
            super::super::normalization::ConstraintBindingView<'_, ArrayLength>,
        ),
    > {
        self.path.const_bindings.iter().map(|(parameter, value)| {
            let scope = value.scope();
            (
                parameter,
                super::super::normalization::ConstraintBindingView {
                    value: value.value(),
                    scope,
                },
            )
        })
    }

    pub(crate) fn projection_view(
        &self,
    ) -> super::super::normalization::ConstraintProjectionView<'_, D> {
        self.path.projection_view()
    }
}

pub(crate) enum PreparedValueInput<'a, P> {
    Available {
        ty: &'a TypeKind<PreparedEffects>,
        original: &'a OriginalProducerContext,
        preparation: &'a P,
    },
    Unavailable {
        original: &'a OriginalProducerContext,
        reason: PreparedValueUnavailable,
        preparation: &'a P,
    },
    UnresolvedProjection {
        original: &'a OriginalProducerContext,
        reason: &'a TypeConstraintRejection,
        preparation: &'a P,
    },
}

pub(crate) struct PreparedValueOutput<P> {
    pub(crate) ty: TypeKind<PreparedEffects>,
    pub(crate) original: OriginalProducerContext,
    pub(crate) preparation: P,
}

impl<P> PreparedValueInput<'_, P> {
    pub(crate) fn original_context(&self) -> &OriginalProducerContext {
        match self {
            Self::Available { original, .. }
            | Self::Unavailable { original, .. }
            | Self::UnresolvedProjection { original, .. } => original,
        }
    }
}

/// Rejection remains an outcome. It never filters the producing frontier.
pub(crate) enum PreparedValueOutcome<P> {
    Accepted(PreparedValueOutput<P>),
    Rejected(P),
    Blocked(P),
    Deferred(P),
    Conditional {
        fits: PreparedEffectPredicate,
        accepted: PreparedValueOutput<P>,
        rejected: P,
    },
}

#[must_use = "finish or discard the affine value-use ticket"]
pub(crate) struct PreparedValueUseTicket<D, P> {
    original: OriginalProducerContext,
    issuer: Rc<ValueIssuer>,
    lease: Rc<UseLease>,
    ordinal: u64,
    inputs: Box<[PreparedValueRef<D>]>,
    roles: Box<[usize]>,
    positions: Vec<usize>,
    next: Vec<usize>,
    depth: usize,
    empty_yielded: bool,
    phase: UsePhase,
    next_alternative: u64,
    outputs: Vec<RecordedUse<P>>,
}

#[must_use = "submit this alternative into the ticket that issued it"]
pub(crate) struct PreparedValueUseAlternative {
    issuer: Rc<ValueIssuer>,
    use_ordinal: u64,
    ordinal: u64,
    positions: Box<[usize]>,
    origins: OriginAssignments,
    predicate: PreparedEffectPredicate,
    blocked: bool,
    deferred: bool,
}

struct RecordedUse<P> {
    origins: OriginAssignments,
    predicate: PreparedEffectPredicate,
    outcome: PreparedValueOutcome<P>,
}

#[must_use = "retain this observed use in its component"]
pub(crate) struct PreparedValueUse<D, P> {
    original: OriginalProducerContext,
    issuer: Rc<ValueIssuer>,
    lease: Rc<UseLease>,
    inputs: Box<[PreparedValueRef<D>]>,
    outputs: Vec<RecordedUse<P>>,
}

/// Roots form a forest; none is fabricated as another root's parent. Producer
/// metadata and original frontiers are immutable after registration. Each use
/// owns only its observations and correlated input coordinates.
pub(crate) struct OpenConstraintComponent<D: ConstraintDomain, P> {
    issuer: Rc<ValueIssuer>,
    use_lease: Rc<UseLease>,
    effects: PreparedEffectGraph,
    next_value: u64,
    next_use: u64,
    values: BTreeMap<u64, RegisteredValue<D, P>>,
}

impl<D: ConstraintDomain, P> OpenConstraintComponent<D, P> {
    #[cfg(test)]
    pub(crate) fn issue_fixture_use<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        inputs: &[PreparedValueRef<D>],
    ) -> Result<PreparedValueUseTicket<D, P>, PreparedValueFailure> {
        let original = context.fixture_original_context(None)?;
        self.issue_use(context, original, inputs)
    }
    /// Fixture cohorts use the authority retained by their first real root.
    /// Empty/diagnostic-only producers must pass an explicit source context.
    #[cfg(test)]
    pub(crate) fn admit_fixture_roots<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        roots: Vec<SuspendedConstraintFrontier<D>>,
        preparation: P,
    ) -> Result<PreparedValueRef<D>, PreparedValueFailure> {
        let original = roots
            .first()
            .ok_or(PreparedValueFailure::ProducerContext)?
            .original_context()
            .clone();
        self.admit_roots(context, original, roots, preparation)
    }
    pub(crate) fn new<A: TypeConstraintAccounting>(
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, PreparedValueFailure> {
        // Admit all three fixed issuer allocations before creating any.
        for _ in 0..3 {
            context.enter_node()?;
        }
        Ok(Self {
            issuer: Rc::new(ValueIssuer),
            use_lease: Rc::new(UseLease),
            effects: PreparedEffectGraph::default(),
            next_value: 0,
            next_use: 0,
            values: BTreeMap::new(),
        })
    }

    pub(crate) fn effects_mut(&mut self) -> &mut PreparedEffectGraph {
        &mut self.effects
    }

    pub(crate) fn has_live_uses(&self) -> bool {
        Rc::strong_count(&self.use_lease) != 1
    }

    fn value(
        &self,
        reference: &PreparedValueRef<D>,
    ) -> Result<&RegisteredValue<D, P>, PreparedValueFailure> {
        if !Rc::ptr_eq(&self.issuer, &reference.issuer) {
            return Err(PreparedValueFailure::ForeignIssuer);
        }
        self.values
            .get(&reference.ordinal)
            .ok_or(PreparedValueFailure::StaleReference)
    }

    fn insert<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        value: RegisteredValue<D, P>,
    ) -> Result<PreparedValueRef<D>, PreparedValueFailure> {
        let ordinal = self.next_value;
        let next = ordinal
            .checked_add(1)
            .ok_or(PreparedValueFailure::OrdinalOverflow)?;
        context.enter_node()?;
        self.values.insert(ordinal, value);
        self.next_value = next;
        Ok(PreparedValueRef {
            issuer: Rc::clone(&self.issuer),
            ordinal,
            domain: PhantomData,
        })
    }

    /// One registration owns every original candidate. The two diagnostic
    /// alternatives are selection outcomes to classify, not free choices or
    /// additional applicable candidates.
    pub(crate) fn admit_roots<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        original: super::super::OriginalProducerContext,
        roots: Vec<SuspendedConstraintFrontier<D>>,
        preparation: P,
    ) -> Result<PreparedValueRef<D>, PreparedValueFailure> {
        let mut application = None;
        for root in &roots {
            context.enter_node()?;
            if root.original_context() != &original {
                return Err(PreparedValueFailure::ProducerContext);
            }
            // A parent's branch is part of the child's identity. Registering
            // that child as a fresh root would silently erase the correlation.
            if root.transaction.parent_receipt.is_some() {
                return Err(PreparedValueFailure::DependentRoot);
            }
            for path in &root.transaction.frontier {
                context.enter_node()?;
                if path.root_id() != root.transaction.application {
                    return Err(PreparedValueFailure::DependentRoot);
                }
                let owner = path
                    .applications
                    .require_application(root.transaction.application)
                    .map_err(TypeConstraintError::Invariant)?
                    .application();
                if application.is_some_and(|application| application != owner) {
                    return Err(PreparedValueFailure::ProducerApplication);
                }
                application = Some(owner);
            }
        }
        let mut alternatives = Vec::new();
        for (frontier, root) in roots.iter().enumerate() {
            for (path_index, path) in root.transaction.frontier.iter().enumerate() {
                context.enter_node()?;
                for _ in path.lineage.scope.shape().binders() {
                    context.enter_node()?;
                }
                let projection =
                    context.with_original_scope(path.lineage.scope.clone(), |context| {
                        let (opened, _) = resolve_probe_term(
                            SourceProbeTerm::Result(root.result.clone()),
                            path,
                            context,
                        )?;
                        project_type(
                            &opened,
                            path.projection_view(),
                            ConstraintClosurePolicy::Hint,
                            context,
                        )
                    });
                let value = match projection {
                    Ok(projection) => {
                        let value = original
                            .gamma()
                            .transport_type(&path.lineage.scope, &projection.value, context)
                            .map_err(Self::projection_failure)?;
                        let ty = value
                            .prepare_effects(original.scope(), context)
                            .map_err(Self::projection_failure)?;
                        context.enter_node()?;
                        ValueContent::Available { ty }
                    }
                    Err(TypeConstraintError::Rejected(reason)) => {
                        ValueContent::UnresolvedProjection(reason)
                    }
                    Err(error) => return Err(error.into()),
                };
                context.enter_node()?;
                alternatives.push(ValueAlternative {
                    origins: Box::new([(
                        self.next_value,
                        ProducerBranch::Constraint {
                            frontier,
                            path: path_index,
                        },
                    )]),
                    predicate: Self::always(),
                    preparation: 0,
                    value,
                });
            }
        }
        for (branch, reason) in [
            (
                ProducerBranch::Rejected,
                PreparedValueUnavailable::RejectedProducer,
            ),
            (
                ProducerBranch::Ambiguous,
                PreparedValueUnavailable::AmbiguousProducer,
            ),
        ] {
            context.enter_node()?;
            alternatives.push(ValueAlternative {
                origins: Box::new([(self.next_value, branch)]),
                predicate: Self::always(),
                preparation: 0,
                value: ValueContent::Unavailable(reason),
            });
        }
        context.enter_node()?;
        self.insert(
            context,
            RegisteredValue {
                original,
                definition: ValueDefinition::Constraint {
                    frontiers: roots.into_boxed_slice(),
                },
                preparations: Box::new([preparation]),
                alternatives: alternatives.into_boxed_slice(),
            },
        )
    }

    pub(crate) fn admit_value<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        value: PreparedValueOutput<P>,
    ) -> Result<PreparedValueRef<D>, PreparedValueFailure> {
        let (ty, original, preparation) = self.retain_output(context, value)?;
        context.enter_node()?;
        self.insert(
            context,
            RegisteredValue {
                original,
                definition: ValueDefinition::Intrinsic,
                preparations: Box::new([preparation]),
                alternatives: Box::new([ValueAlternative {
                    origins: Box::new([]),
                    predicate: Self::always(),
                    preparation: 0,
                    value: ValueContent::Available { ty },
                }]),
            },
        )
    }

    pub(crate) fn issue_use<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        original: OriginalProducerContext,
        inputs: &[PreparedValueRef<D>],
    ) -> Result<PreparedValueUseTicket<D, P>, PreparedValueFailure> {
        let ordinal = self.next_use;
        let next = ordinal
            .checked_add(1)
            .ok_or(PreparedValueFailure::OrdinalOverflow)?;
        for input in inputs {
            context.enter_node()?;
            self.value(input)?;
        }
        for _ in 0..2 {
            context.enter_node()?;
        }
        let mut unique = Vec::with_capacity(inputs.len());
        let mut roles = Vec::with_capacity(inputs.len());
        let mut indices = BTreeMap::new();
        for input in inputs {
            context.enter_node()?;
            if let Some(index) = indices.get(&input.ordinal) {
                roles.push(*index);
            } else {
                context.enter_node()?;
                indices.insert(input.ordinal, unique.len());
                roles.push(unique.len());
                unique.push(input.clone());
            }
        }
        for _ in 0..2 {
            context.enter_node()?;
        }
        let positions = vec![0; unique.len()];
        let next_indices = vec![0; unique.len()];
        self.next_use = next;
        Ok(PreparedValueUseTicket {
            original,
            issuer: Rc::clone(&self.issuer),
            lease: Rc::clone(&self.use_lease),
            ordinal,
            inputs: unique.into_boxed_slice(),
            roles: roles.into_boxed_slice(),
            positions,
            next: next_indices,
            depth: 0,
            empty_yielded: false,
            phase: UsePhase::Open,
            next_alternative: 0,
            outputs: Vec::new(),
        })
    }

    fn validate_ticket(
        &self,
        ticket: &PreparedValueUseTicket<D, P>,
    ) -> Result<(), PreparedValueFailure> {
        if !Rc::ptr_eq(&self.issuer, &ticket.issuer) || !Rc::ptr_eq(&self.use_lease, &ticket.lease)
        {
            return Err(PreparedValueFailure::ForeignIssuer);
        }
        Ok(())
    }

    fn validate_alternative(
        &self,
        ticket: &PreparedValueUseTicket<D, P>,
        alternative: &PreparedValueUseAlternative,
    ) -> Result<(), PreparedValueFailure> {
        self.validate_ticket(ticket)?;
        if !Rc::ptr_eq(&self.issuer, &alternative.issuer)
            || ticket.ordinal != alternative.use_ordinal
            || ticket.phase != UsePhase::Pending(alternative.ordinal)
        {
            return Err(PreparedValueFailure::UseState);
        }
        Ok(())
    }

    pub(crate) fn next_use_alternative<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        ticket: &mut PreparedValueUseTicket<D, P>,
    ) -> Result<Option<PreparedValueUseAlternative>, PreparedValueFailure> {
        self.validate_ticket(ticket)?;
        if ticket.phase == UsePhase::Exhausted {
            return Ok(None);
        }
        if ticket.phase != UsePhase::Open {
            return Err(PreparedValueFailure::UseState);
        }
        let result = self.advance_use(context, ticket);
        if result.is_err() {
            ticket.phase = UsePhase::Failed;
        }
        result
    }

    fn advance_use<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        ticket: &mut PreparedValueUseTicket<D, P>,
    ) -> Result<Option<PreparedValueUseAlternative>, PreparedValueFailure> {
        if ticket.inputs.is_empty() {
            if ticket.empty_yielded {
                ticket.phase = UsePhase::Exhausted;
                return Ok(None);
            }
            ticket.empty_yielded = true;
        } else {
            loop {
                context.enter_node()?;
                if ticket.depth == ticket.inputs.len() {
                    ticket.depth -= 1;
                    break;
                }
                let depth = ticket.depth;
                let value = self.value(&ticket.inputs[depth])?;
                let index = ticket.next[depth];
                if index == value.alternatives.len() {
                    ticket.next[depth] = 0;
                    if depth == 0 {
                        ticket.phase = UsePhase::Exhausted;
                        return Ok(None);
                    }
                    ticket.depth -= 1;
                    continue;
                }
                ticket.next[depth] += 1;
                let candidate = &value.alternatives[index];
                let mut compatible = true;
                for previous in 0..depth {
                    context.enter_node()?;
                    let previous = &self.value(&ticket.inputs[previous])?.alternatives
                        [ticket.positions[previous]];
                    for (origin, branch) in &candidate.origins {
                        context.enter_node()?;
                        if let Ok(index) = previous
                            .origins
                            .binary_search_by_key(origin, |(key, _)| *key)
                            && previous.origins[index].1 != *branch
                        {
                            compatible = false;
                            break;
                        }
                    }
                    if !compatible {
                        break;
                    }
                }
                if compatible {
                    ticket.positions[depth] = index;
                    ticket.depth += 1;
                }
            }
        }
        let mut origins = BTreeMap::new();
        let mut predicates = Vec::new();
        let mut blocked = false;
        let mut deferred = false;
        for (index, input) in ticket.inputs.iter().enumerate() {
            let alternative = &self.value(input)?.alternatives[ticket.positions[index]];
            for (origin, branch) in &alternative.origins {
                context.enter_node()?;
                origins.insert(*origin, *branch);
            }
            context.enter_node()?;
            predicates.push(alternative.predicate.clone());
            match &alternative.value {
                ValueContent::Available { .. } => {}
                ValueContent::Unavailable(PreparedValueUnavailable::Deferred)
                | ValueContent::UnresolvedProjection(_) => deferred = true,
                ValueContent::Unavailable(_) => blocked = true,
            }
        }
        for _ in &ticket.positions {
            context.enter_node()?;
        }
        let ordinal = ticket.next_alternative;
        ticket.next_alternative = ordinal
            .checked_add(1)
            .ok_or(PreparedValueFailure::OrdinalOverflow)?;
        let predicate = self.effects.and(predicates, context)?;
        ticket.phase = UsePhase::Pending(ordinal);
        Ok(Some(PreparedValueUseAlternative {
            issuer: Rc::clone(&self.issuer),
            use_ordinal: ticket.ordinal,
            ordinal,
            positions: ticket.positions.clone().into_boxed_slice(),
            origins: origins.into_iter().collect(),
            predicate,
            blocked,
            deferred,
        }))
    }

    pub(crate) fn input<'a>(
        &'a self,
        ticket: &PreparedValueUseTicket<D, P>,
        alternative: &PreparedValueUseAlternative,
        role: usize,
    ) -> Result<PreparedValueInput<'a, P>, PreparedValueFailure> {
        self.validate_alternative(ticket, alternative)?;
        let input = *ticket
            .roles
            .get(role)
            .ok_or(PreparedValueFailure::UseState)?;
        let value = self.value(&ticket.inputs[input])?;
        let alternative = &value.alternatives[alternative.positions[input]];
        let preparation = &value.preparations[alternative.preparation];
        Ok(match &alternative.value {
            ValueContent::Available { ty } => PreparedValueInput::Available {
                ty,
                original: &value.original,
                preparation,
            },
            ValueContent::Unavailable(reason) => PreparedValueInput::Unavailable {
                original: &value.original,
                reason: *reason,
                preparation,
            },
            ValueContent::UnresolvedProjection(reason) => {
                PreparedValueInput::UnresolvedProjection {
                    original: &value.original,
                    reason,
                    preparation,
                }
            }
        })
    }

    /// Visit each correlated producer branch once, even if several input roles
    /// or derived values read it. The references point into the retained root
    /// forest; no bindings, scope, or source observations are reconstructed.
    pub(crate) fn visit_original_constraints<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
        ticket: &PreparedValueUseTicket<D, P>,
        alternative: &PreparedValueUseAlternative,
        mut visit: impl FnMut(
            PreparedValueOriginalConstraint<'_, D>,
            &mut TypeConstraintContext<'_, A, D>,
        ) -> Result<(), PreparedValueFailure>,
    ) -> Result<(), PreparedValueFailure> {
        self.validate_alternative(ticket, alternative)?;
        context.check_cancelled()?;
        for (origin, branch) in &alternative.origins {
            context.enter_node()?;
            let value = self
                .values
                .get(origin)
                .ok_or(PreparedValueFailure::StaleReference)?;
            let ValueDefinition::Constraint { frontiers } = &value.definition else {
                return Err(PreparedValueFailure::InputState);
            };
            // Diagnostic alternatives have no source path. Their unavailable
            // input remains visible through input(); never invent one here.
            let ProducerBranch::Constraint { frontier, path } = branch else {
                continue;
            };
            let root = frontiers
                .get(*frontier)
                .ok_or(PreparedValueFailure::InputState)?;
            let path = root
                .transaction
                .frontier
                .get(*path)
                .ok_or(PreparedValueFailure::InputState)?;
            let owner = path
                .applications
                .require_application(root.transaction.application)
                .map_err(TypeConstraintError::Invariant)?;
            visit(
                PreparedValueOriginalConstraint {
                    original: &value.original,
                    application: owner.application(),
                    parameters: owner.parameters(),
                    path,
                },
                context,
            )?;
        }
        Ok(())
    }

    pub(crate) fn submit_use<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
        ticket: &mut PreparedValueUseTicket<D, P>,
        alternative: PreparedValueUseAlternative,
        outcome: PreparedValueOutcome<P>,
    ) -> Result<(), PreparedValueFailure> {
        self.validate_alternative(ticket, &alternative)?;
        let result = self.record_use(context, ticket, alternative, outcome);
        if result.is_err() {
            ticket.phase = UsePhase::Failed;
        }
        result
    }

    fn record_use<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
        ticket: &mut PreparedValueUseTicket<D, P>,
        alternative: PreparedValueUseAlternative,
        outcome: PreparedValueOutcome<P>,
    ) -> Result<(), PreparedValueFailure> {
        let valid = if alternative.blocked {
            matches!(outcome, PreparedValueOutcome::Blocked(_))
        } else if alternative.deferred {
            matches!(outcome, PreparedValueOutcome::Deferred(_))
        } else {
            !matches!(outcome, PreparedValueOutcome::Blocked(_))
        };
        if !valid {
            return Err(PreparedValueFailure::InputState);
        }
        let output_context = match &outcome {
            PreparedValueOutcome::Accepted(output)
            | PreparedValueOutcome::Conditional {
                accepted: output, ..
            } => Some(&output.original),
            PreparedValueOutcome::Rejected(_)
            | PreparedValueOutcome::Blocked(_)
            | PreparedValueOutcome::Deferred(_) => None,
        };
        if output_context.is_some_and(|original| original != &ticket.original) {
            return Err(PreparedValueFailure::UseContext);
        }
        context.enter_node()?;
        ticket.outputs.push(RecordedUse {
            origins: alternative.origins,
            predicate: alternative.predicate,
            outcome,
        });
        ticket.phase = UsePhase::Open;
        Ok(())
    }

    pub(crate) fn finish_use(
        &self,
        ticket: PreparedValueUseTicket<D, P>,
    ) -> Result<PreparedValueUse<D, P>, PreparedValueFailure> {
        self.validate_ticket(&ticket)?;
        if ticket.phase != UsePhase::Exhausted {
            return Err(PreparedValueFailure::UseState);
        }
        Ok(PreparedValueUse {
            original: ticket.original,
            issuer: ticket.issuer,
            lease: ticket.lease,
            inputs: ticket.inputs,
            outputs: ticket.outputs,
        })
    }

    pub(crate) fn retain_use<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        observed: PreparedValueUse<D, P>,
    ) -> Result<PreparedValueRef<D>, PreparedValueFailure> {
        if !Rc::ptr_eq(&self.issuer, &observed.issuer)
            || !Rc::ptr_eq(&self.use_lease, &observed.lease)
        {
            return Err(PreparedValueFailure::ForeignIssuer);
        }
        let ordinal = self.next_value;
        let next = ordinal
            .checked_add(1)
            .ok_or(PreparedValueFailure::OrdinalOverflow)?;
        context.enter_node()?;
        let mut checkpoint = self.effects.checkpoint(context)?;
        let built = self.build_use(context, observed);
        match built {
            Ok(value) => {
                self.effects.commit(&checkpoint).map_err(|error| {
                    PreparedValueFailure::Effect(PreparedEffectFailure::Invariant(error))
                })?;
                self.values.insert(ordinal, value);
                self.next_value = next;
                Ok(PreparedValueRef {
                    issuer: Rc::clone(&self.issuer),
                    ordinal,
                    domain: PhantomData,
                })
            }
            Err(error) => {
                self.effects.rollback(&mut checkpoint).map_err(|rollback| {
                    PreparedValueFailure::Effect(PreparedEffectFailure::Invariant(rollback))
                })?;
                Err(error)
            }
        }
    }

    fn build_use<A: TypeConstraintAccounting>(
        &mut self,
        context: &mut TypeConstraintContext<'_, A, D>,
        observed: PreparedValueUse<D, P>,
    ) -> Result<RegisteredValue<D, P>, PreparedValueFailure> {
        let mut alternatives = Vec::new();
        let mut preparations = Vec::new();
        for recorded in observed.outputs {
            let mut emit = |origins, predicate, value, preparation| {
                context.enter_node()?;
                let index = preparations.len();
                preparations.push(preparation);
                alternatives.push(ValueAlternative {
                    origins,
                    predicate,
                    preparation: index,
                    value,
                });
                Ok::<_, PreparedValueFailure>(())
            };
            match recorded.outcome {
                PreparedValueOutcome::Accepted(output) => {
                    let (ty, _, preparation) = self.retain_output(context, output)?;
                    context.enter_node()?;
                    let index = preparations.len();
                    preparations.push(preparation);
                    alternatives.push(ValueAlternative {
                        origins: recorded.origins,
                        predicate: recorded.predicate,
                        preparation: index,
                        value: ValueContent::Available { ty },
                    });
                }
                PreparedValueOutcome::Rejected(preparation) => emit(
                    recorded.origins,
                    recorded.predicate,
                    ValueContent::Unavailable(PreparedValueUnavailable::RejectedUse),
                    preparation,
                )?,
                PreparedValueOutcome::Blocked(preparation) => emit(
                    recorded.origins,
                    recorded.predicate,
                    ValueContent::Unavailable(PreparedValueUnavailable::BlockedUse),
                    preparation,
                )?,
                PreparedValueOutcome::Deferred(preparation) => emit(
                    recorded.origins,
                    recorded.predicate,
                    ValueContent::Unavailable(PreparedValueUnavailable::Deferred),
                    preparation,
                )?,
                PreparedValueOutcome::Conditional {
                    fits,
                    accepted,
                    rejected,
                } => {
                    let (ty, _, preparation) = self.retain_output(context, accepted)?;
                    self.effects
                        .validate_retained_predicate(&fits)
                        .map_err(|error| {
                            PreparedValueFailure::Effect(PreparedEffectFailure::Invariant(error))
                        })?;
                    for _ in &recorded.origins {
                        context.enter_node()?;
                    }
                    let rejected_origins = recorded.origins.clone();
                    let rejected_fits = self.effects.not(fits.clone(), context)?;
                    let accepted_guard = self
                        .effects
                        .and(vec![recorded.predicate.clone(), fits], context)?;
                    let rejected_guard = self
                        .effects
                        .and(vec![recorded.predicate, rejected_fits], context)?;
                    for (origins, predicate, value, preparation) in [
                        (
                            recorded.origins,
                            accepted_guard,
                            ValueContent::Available { ty },
                            preparation,
                        ),
                        (
                            rejected_origins,
                            rejected_guard,
                            ValueContent::Unavailable(PreparedValueUnavailable::RejectedUse),
                            rejected,
                        ),
                    ] {
                        context.enter_node()?;
                        let index = preparations.len();
                        preparations.push(preparation);
                        alternatives.push(ValueAlternative {
                            origins,
                            predicate,
                            preparation: index,
                            value,
                        });
                    }
                }
            }
        }
        Ok(RegisteredValue {
            original: observed.original,
            definition: ValueDefinition::Use(observed.inputs),
            preparations: preparations.into_boxed_slice(),
            alternatives: alternatives.into_boxed_slice(),
        })
    }

    fn retain_output<A: TypeConstraintAccounting>(
        &self,
        context: &mut TypeConstraintContext<'_, A, D>,
        output: PreparedValueOutput<P>,
    ) -> Result<(TypeKind<PreparedEffects>, OriginalProducerContext, P), PreparedValueFailure> {
        let ty = output
            .ty
            .retain_in_effect_graph(output.original.scope(), &self.effects, context)
            .map_err(Self::retention_failure)?;
        context.enter_node()?;
        Ok((ty, output.original, output.preparation))
    }

    fn projection_failure(error: TypeProjectionError<TypeConstraintError>) -> PreparedValueFailure {
        match error {
            TypeProjectionError::Control(error) => error.into(),
            TypeProjectionError::Instantiation(error) => error.into(),
        }
    }

    fn retention_failure(
        error: TypeProjectionError<PreparedTypeRetentionFailure<TypeConstraintError>>,
    ) -> PreparedValueFailure {
        match error {
            TypeProjectionError::Instantiation(error) => error.into(),
            TypeProjectionError::Control(PreparedTypeRetentionFailure::Control(error)) => {
                error.into()
            }
            TypeProjectionError::Control(PreparedTypeRetentionFailure::Effect(error)) => {
                PreparedValueFailure::Effect(PreparedEffectFailure::Invariant(error))
            }
        }
    }

    fn always() -> PreparedEffectPredicate {
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
    }
}
