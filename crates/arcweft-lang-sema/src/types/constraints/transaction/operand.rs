//! Scoped equation terms retain their exact source or template authority.

use crate::types::PreparedEffects;

use super::*;
use crate::types::constraints::{
    ConstraintSourceValueOwner, OriginalProducerContext, lexical::ConstraintLexicalScope,
};

mod function_scheme;
mod original_value;
pub(super) use original_value::ConstraintEffectPhase;
mod parameter;
mod projection;
pub(crate) use function_scheme::{ConstraintResultUseSource, PreparedConstraintFunctionScheme};
pub(in crate::types::constraints) use projection::ConstraintOperandChildren;
use projection::ConstraintOperandDerivation;
use projection::ConstraintOperandProjection;
pub(in crate::types::constraints) use projection::{
    AdmittedConstraintFunctionHeader, ConstraintEffectField, ConstraintEffectHeaderDerivation,
    ConstraintOperandRebuild, ConstraintProjectedOperand,
};
pub(in crate::types) use projection::{
    ConstraintConstOperandView, ConstraintFunctionHeaderView, ConstraintOperandView,
};

pub(super) enum ResolvedConstraintSource<D: ConstraintDomain> {
    Value(Arc<D::SourceValue>),
    Result(ResolvedConstraintResult<D>),
    #[cfg(test)]
    Fixture,
}

impl<D: ConstraintDomain> Clone for ResolvedConstraintSource<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Value(value) => Self::Value(Arc::clone(value)),
            Self::Result(result) => Self::Result(result.clone()),
            #[cfg(test)]
            Self::Fixture => Self::Fixture,
        }
    }
}

impl<D: ConstraintDomain> ResolvedConstraintSource<D> {
    pub(super) fn value<A: TypeConstraintAccounting>(
        owner: Arc<D::SourceValue>,
        target: &ConstraintLexicalScope,
        phase: &mut ConstraintEffectPhase<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(TypeKind<PreparedEffects>, Self), TypeConstraintError> {
        context.check_cancelled()?;
        let original = owner.original_context();
        // The probe already charged its root. Closed leaves retain the
        // original value owner without traversing its generic scope.
        let value = if owner.value_type().constraint_shape().is_closed_leaf() {
            owner.value_type().clone()
        } else {
            phase.admit_type(owner.value_type(), target, original, context)?
        };
        Ok((value, Self::Value(owner)))
    }

    pub(super) fn projection(&self) -> Option<ConstraintResultProjection<D>> {
        match self {
            Self::Result(result) => Some(result.projection()),
            _ => None,
        }
    }

    pub(super) fn into_result(self) -> Result<ResolvedConstraintResult<D>, TypeConstraintError> {
        match self {
            Self::Result(result) => Ok(result),
            _ => Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            )),
        }
    }
}

/// Issued only by resolving one unique projection on its admitted owner.
/// Keeping the request itself prevents a later equal type from replacing the
/// selected result's authority.
pub(super) struct ResolvedConstraintResult<D: ConstraintDomain> {
    owner: Arc<ConstraintApplicationScope<D>>,
    request: Arc<ProjectionRequest<D>>,
}

impl<D: ConstraintDomain> Clone for ResolvedConstraintResult<D> {
    fn clone(&self) -> Self {
        Self {
            owner: Arc::clone(&self.owner),
            request: Arc::clone(&self.request),
        }
    }
}

impl<D: ConstraintDomain> ResolvedConstraintResult<D> {
    pub(super) fn request<'a>(
        result: &ConstraintResultProjection<D>,
        path: &'a ConstraintPath<D>,
    ) -> Result<&'a Arc<ProjectionRequest<D>>, TypeConstraintError> {
        let mut requests = path.lineage.projections.iter().filter(|request| {
            request.application == result.application && request.key == result.key
        });
        let request = requests
            .next()
            .ok_or_else(|| protocol_error(TypeConstraintSourceProtocolInvariant::WrongSource))?;
        if requests.next().is_some() {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::Outcome,
            ));
        }
        Ok(request)
    }

    pub(super) fn resolve<A: TypeConstraintAccounting>(
        result: ConstraintResultProjection<D>,
        path: &ConstraintPath<D>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(TypeKind<PreparedEffects>, Self), TypeConstraintError> {
        let owner = path
            .applications
            .require_application_owner(result.application)?;
        let request = Self::request(&result, path)?;
        let target = Arc::new(context.capture_lexical_scope()?);
        let value = if request.value.scope() == target.as_ref() {
            context.enter_node()?;
            request.value.value().clone()
        } else {
            request
                .value
                .view()
                .transport(&target, graph, context)
                .map_err(|error| match error {
                    crate::types::TypeProjectionError::Control(error) => error,
                    crate::types::TypeProjectionError::Instantiation(error) => {
                        TypeConstraintInvariant::Instantiation(error).into()
                    }
                })?
                .value()
                .clone()
        };
        Ok((
            value,
            Self {
                owner: Arc::clone(owner),
                request: Arc::clone(request),
            },
        ))
    }

    pub(super) fn projection(&self) -> ConstraintResultProjection<D> {
        ConstraintResultProjection {
            application: self.request.application,
            key: Arc::clone(&self.request.key),
        }
    }
}

/// Evidence is held together with the value, never in a binding-to-captures
/// table. These source contexts retain provenance; owner-specific Free
/// authorization also requires the checked value and binding-flow migration.
enum ConstraintOperandOrigin<D: ConstraintDomain> {
    /// A closed grammar leaf has no generic ports or source permissions.
    ClosedScalar,
    Original(OriginalProducerContext),
    ComputedEffect {
        context: Arc<super::super::normalization::ConstraintEffectCompletionContext<D>>,
        support: super::super::application::ImportedGenericParameterScopeLease,
    },
    Parameter(super::super::application::ApplicationParameterScopeLease),
    Template(Arc<ConstraintApplicationScope<D>>),
    Source {
        original: OriginalProducerContext,
        receipt: ConstraintSourceReceipt<D>,
        value: ResolvedConstraintSource<D>,
    },
    Result(ResolvedConstraintResult<D>),
    Completed {
        original: OriginalProducerContext,
        solution: Arc<TypeConstraintSolution>,
        parameter: super::super::ConstraintGenericParameterId,
    },
    FunctionSchemeInstantiation {
        instance: ResolvedConstraintResult<D>,
        source: Arc<ConstraintOperand<D>>,
    },
    SourceExpectation {
        projection: CheckedConstraintSourceProjection<PreparedEffects>,
        actual: ConstraintOperand<D>,
        value_expected: ConstraintOperand<D>,
    },
    Projection {
        root: Arc<TypeKind<PreparedEffects>>,
        root_scope: Arc<ConstraintLexicalScope>,
        occurrence_scope: Arc<ConstraintLexicalScope>,
        // Result association stays with the observed root, including aliases.
        producer: Arc<ConstraintOperandOrigin<D>>,
        // Value admission follows the corresponding child or alias terminal.
        selected: Arc<ConstraintOperandOrigin<D>>,
        address: Option<Arc<ConstraintOperandProjection>>,
    },
    ConstOccurrence(ConstraintOperand<D, ArrayLength>),
    Rebuilt {
        source: Arc<ConstraintOperandOrigin<D>>,
        contents_scope: Arc<ConstraintLexicalScope>,
        children: Box<[ConstraintOperandDerivation<D, TypeKind<PreparedEffects>>]>,
        length: Option<ConstraintOperandDerivation<D, ArrayLength>>,
        header: Option<super::super::normalization::ConstraintClosedEffectHeader<D>>,
    },
    TypeAlias {
        steps: Box<[ConstraintAliasStep<D, TypeKind<PreparedEffects>>]>,
        terminal: ConstraintOperandDerivation<D, TypeKind<PreparedEffects>>,
    },
    ConstAlias {
        steps: Box<[ConstraintAliasStep<D, ArrayLength>]>,
        terminal: ConstraintOperandDerivation<D, ArrayLength>,
    },
}

pub(in crate::types::constraints) struct ConstraintAliasStep<D: ConstraintDomain, T> {
    pub(in crate::types::constraints) occurrence: ConstraintOperand<D, T>,
    pub(in crate::types::constraints) replacement: ConstraintOperand<D, T>,
}

pub(in crate::types) struct ConstraintOperand<D: ConstraintDomain, T = TypeKind<PreparedEffects>> {
    value: Arc<T>,
    scope: Arc<ConstraintLexicalScope>,
    origin: Arc<ConstraintOperandOrigin<D>>,
}

impl<D: ConstraintDomain, T> Clone for ConstraintOperand<D, T> {
    fn clone(&self) -> Self {
        Self {
            value: Arc::clone(&self.value),
            scope: Arc::clone(&self.scope),
            origin: Arc::clone(&self.origin),
        }
    }
}

impl<D: ConstraintDomain, T> ConstraintOperand<D, T> {
    pub(in crate::types::constraints) fn from_effect_primitive(
        primitive: super::super::normalization::ConstraintEffectPrimitive<D, T>,
    ) -> Self {
        let (value, context, support) = primitive.into_parts();
        Self {
            value,
            scope: Arc::clone(context.scope()),
            origin: Arc::new(ConstraintOperandOrigin::ComputedEffect { context, support }),
        }
    }

    pub(in crate::types::constraints::transaction) fn template(
        value: impl Into<Arc<T>>,
        scope: Arc<ConstraintLexicalScope>,
        owner: &Arc<ConstraintApplicationScope<D>>,
    ) -> Self {
        Self {
            value: value.into(),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Template(Arc::clone(owner))),
        }
    }

    pub(super) fn original(
        value: impl Into<Arc<T>>,
        scope: Arc<ConstraintLexicalScope>,
        original: OriginalProducerContext,
    ) -> Self {
        Self {
            value: value.into(),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Original(original)),
        }
    }

    #[cfg(test)]
    pub(in crate::types::constraints) fn fixture_template(
        value: T,
        scope: Arc<ConstraintLexicalScope>,
        owner: &Arc<ConstraintApplicationScope<D>>,
    ) -> Self {
        Self::template(value, scope, owner)
    }

    fn valid_alias_steps(
        steps: &[ConstraintAliasStep<D, T>],
        terminal: &Self,
        target: &ConstraintLexicalScope,
    ) -> bool {
        let same_term = |left: &Self, right: &Self| {
            Arc::ptr_eq(&left.value, &right.value)
                && Arc::ptr_eq(&left.origin, &right.origin)
                && left.scope == right.scope
        };
        steps
            .first()
            .is_some_and(|step| step.occurrence.scope() == target)
            && steps
                .last()
                .is_some_and(|step| same_term(&step.replacement, terminal))
            && steps
                .windows(2)
                .all(|pair| same_term(&pair[0].replacement, &pair[1].occurrence))
    }

    pub(in crate::types::constraints) fn associated_result(
        &self,
    ) -> Option<ConstraintResultProjection<D>> {
        self.origin.associated_result()
    }

    pub(in crate::types) fn value(&self) -> &T {
        &self.value
    }

    pub(in crate::types::constraints) fn shared_value(&self) -> &Arc<T> {
        &self.value
    }

    pub(in crate::types::constraints) fn scope(&self) -> &ConstraintLexicalScope {
        &self.scope
    }

    pub(in crate::types::constraints) fn shared_scope(&self) -> Arc<ConstraintLexicalScope> {
        Arc::clone(&self.scope)
    }

    pub(in crate::types::constraints) fn completed(
        value: T,
        scope: Arc<ConstraintLexicalScope>,
        original: OriginalProducerContext,
        solution: Arc<TypeConstraintSolution>,
        parameter: super::super::ConstraintGenericParameterId,
    ) -> Self {
        Self {
            value: Arc::new(value),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Completed {
                original,
                solution,
                parameter,
            }),
        }
    }
}

impl<D: ConstraintDomain> ConstraintOperand<D, TypeKind> {
    /// Relocate a completed value without reopening its source calculation.
    pub(in crate::types::constraints) fn transport_canonical<A: TypeConstraintAccounting>(
        &self,
        target: &Arc<ConstraintLexicalScope>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, crate::types::TypeProjectionError<TypeConstraintError>> {
        let value = target.transport_canonical_type(&self.scope, &self.value, context)?;
        context
            .enter_node()
            .map_err(crate::types::TypeProjectionError::Control)?;
        Ok(Self {
            value: Arc::new(value),
            scope: Arc::clone(target),
            origin: Arc::clone(&self.origin),
        })
    }
}

impl<D: ConstraintDomain> ConstraintOperand<D> {
    pub(super) fn shared_result_observation(&self) -> Option<&Arc<TypeKind<PreparedEffects>>> {
        match self.origin.as_ref() {
            ConstraintOperandOrigin::Source {
                value: ResolvedConstraintSource::Result(_),
                ..
            } => Some(&self.value),
            _ => None,
        }
    }

    pub(super) fn source_origin(
        &self,
        source: super::ConstraintSourceId<D::Source>,
    ) -> Result<(&OriginalProducerContext, Option<&Arc<D::SourceValue>>), TypeConstraintError> {
        match self.origin.as_ref() {
            ConstraintOperandOrigin::Source {
                original,
                receipt,
                value,
            } if receipt.source() == source => {
                let owner = match value {
                    ResolvedConstraintSource::Value(owner) => {
                        self.require_source_receipt(receipt)?;
                        Some(owner)
                    }
                    ResolvedConstraintSource::Result(_) => None,
                    #[cfg(test)]
                    ResolvedConstraintSource::Fixture => None,
                };
                Ok((original, owner))
            }
            #[cfg(test)]
            ConstraintOperandOrigin::Original(original) => Ok((original, None)),
            _ => Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::WrongSource,
            )),
        }
    }

    /// Open once while this projection still owns a live graph edit. Final
    /// readers retain this exact value and its real occurrence frames.
    pub(in crate::types::constraints::transaction) fn open_template<A: TypeConstraintAccounting>(
        value: &TypeKind<PreparedEffects>,
        path: &ConstraintPath<D>,
        application: ConstraintApplicationId,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        let scope = Arc::new(context.capture_lexical_scope()?);
        let value = context.open_template_type(value, path, application, graph)?;
        Ok(Self::template(
            value,
            scope,
            path.applications.require_application_owner(application)?,
        ))
    }

    pub(in crate::types::constraints) fn require_source_receipt(
        &self,
        expected: &ConstraintSourceReceipt<D>,
    ) -> Result<(), TypeConstraintError> {
        let ConstraintOperandOrigin::Source { receipt, value, .. } = self.origin.as_ref() else {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::WrongSource,
            ));
        };
        let ResolvedConstraintSource::Value(owner) = value else {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::WrongSource,
            ));
        };
        if !receipt.matches(expected)
            || owner.source() != expected.source()
            || !owner
                .probe_receipt()
                .is_some_and(|receipt| receipt.matches(expected))
        {
            return Err(protocol_error(
                TypeConstraintSourceProtocolInvariant::WrongSource,
            ));
        }
        Ok(())
    }

    pub(in crate::types::constraints) fn aliased(
        mut self,
        steps: Box<[ConstraintAliasStep<D, TypeKind<PreparedEffects>>]>,
        terminal: Self,
    ) -> Result<Self, TypeConstraintError> {
        if !Self::valid_alias_steps(&steps, &terminal, &self.scope) {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        self.origin = Arc::new(ConstraintOperandOrigin::TypeAlias {
            steps,
            terminal: terminal.derivation(),
        });
        Ok(self)
    }
    pub(super) fn source(
        value: TypeKind<PreparedEffects>,
        scope: Arc<ConstraintLexicalScope>,
        original: OriginalProducerContext,
        receipt: ConstraintSourceReceipt<D>,
        value_owner: ResolvedConstraintSource<D>,
    ) -> Self {
        Self {
            value: Arc::new(value),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Source {
                original,
                receipt,
                value: value_owner,
            }),
        }
    }

    pub(super) fn result(
        value: TypeKind<PreparedEffects>,
        scope: Arc<ConstraintLexicalScope>,
        result: ResolvedConstraintResult<D>,
    ) -> Self {
        Self {
            value: Arc::new(value),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Result(result)),
        }
    }

    pub(in crate::types) fn view(&self) -> ConstraintOperandView<'_, D> {
        ConstraintOperandView::root(self)
    }

    pub(in crate::types::constraints) fn transport<A: TypeConstraintAccounting>(
        &self,
        target: &Arc<ConstraintLexicalScope>,
        graph: &mut crate::effect_row::prepared::PreparedEffectGraph,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, crate::types::TypeProjectionError<TypeConstraintError>> {
        Ok(Self {
            value: Arc::new(target.transport_type(&self.scope, &self.value, graph, context)?),
            scope: Arc::clone(target),
            origin: Arc::clone(&self.origin),
        })
    }
}

impl<D: ConstraintDomain> ConstraintOperand<D, ArrayLength> {
    pub(in crate::types::constraints) fn aliased(
        mut self,
        steps: Box<[ConstraintAliasStep<D, ArrayLength>]>,
        terminal: Self,
    ) -> Result<Self, TypeConstraintError> {
        if !Self::valid_alias_steps(&steps, &terminal, &self.scope) {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        self.origin = Arc::new(ConstraintOperandOrigin::ConstAlias {
            steps,
            terminal: terminal.derivation(),
        });
        Ok(self)
    }
    pub(in crate::types) fn view(&self) -> ConstraintConstOperandView<'_, D> {
        ConstraintConstOperandView::root(self)
    }

    pub(in crate::types::constraints) fn transport<A: TypeConstraintAccounting>(
        &self,
        target: &Arc<ConstraintLexicalScope>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, crate::types::TypeProjectionError<TypeConstraintError>> {
        Ok(Self {
            value: Arc::new(target.transport_length(&self.scope, &self.value, context)?),
            scope: Arc::clone(target),
            origin: Arc::clone(&self.origin),
        })
    }
}

#[cfg(test)]
impl<D: ConstraintDomain, T> ConstraintOperand<D, T> {
    pub(in crate::types::constraints) fn completed_origin(
        &self,
    ) -> Option<(
        &Arc<TypeConstraintSolution>,
        &super::super::ConstraintGenericParameterId,
    )> {
        match self.origin.as_ref() {
            ConstraintOperandOrigin::Completed {
                solution,
                parameter,
                ..
            } => Some((solution, parameter)),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn original_context(&self) -> &OriginalProducerContext {
        self.origin.original_context()
    }

    #[cfg(test)]
    pub(super) fn result_origin(&self) -> Option<ConstraintResultProjection<D>> {
        self.origin.associated_result()
    }

    #[cfg(test)]
    pub(super) fn source_receipt(&self) -> Option<&ConstraintSourceReceipt<D>> {
        self.origin.source_receipt()
    }
}

#[cfg(test)]
impl<D: ConstraintDomain> ConstraintOperandOrigin<D> {
    fn original_context(&self) -> &OriginalProducerContext {
        match self {
            ConstraintOperandOrigin::ClosedScalar => {
                panic!("a closed scalar has no source context")
            }
            ConstraintOperandOrigin::ComputedEffect { context, .. } => context.original_context(),
            ConstraintOperandOrigin::Original(original)
            | ConstraintOperandOrigin::Source { original, .. }
            | ConstraintOperandOrigin::Completed { original, .. } => original,
            ConstraintOperandOrigin::Parameter(owner) => owner.original_context(),
            ConstraintOperandOrigin::Template(owner) => owner.original_context(),
            ConstraintOperandOrigin::Result(result) => result.owner.original_context(),
            ConstraintOperandOrigin::FunctionSchemeInstantiation { instance, .. } => {
                instance.owner.original_context()
            }
            ConstraintOperandOrigin::SourceExpectation { value_expected, .. } => {
                value_expected.original_context()
            }
            ConstraintOperandOrigin::Projection { producer, .. } => producer.original_context(),
            ConstraintOperandOrigin::ConstOccurrence(root) => root.original_context(),
            ConstraintOperandOrigin::Rebuilt { source, .. } => source.original_context(),
            ConstraintOperandOrigin::TypeAlias { steps, .. } => {
                steps[0].occurrence.original_context()
            }
            ConstraintOperandOrigin::ConstAlias { steps, .. } => {
                steps[0].occurrence.original_context()
            }
        }
    }

    fn source_receipt(&self) -> Option<&ConstraintSourceReceipt<D>> {
        match self {
            ConstraintOperandOrigin::Source { receipt, .. } => Some(receipt),
            ConstraintOperandOrigin::SourceExpectation { actual, .. } => actual.source_receipt(),
            ConstraintOperandOrigin::Projection { producer, .. } => producer.source_receipt(),
            ConstraintOperandOrigin::ConstOccurrence(root) => root.source_receipt(),
            ConstraintOperandOrigin::Rebuilt { source, .. } => source.source_receipt(),
            ConstraintOperandOrigin::TypeAlias { steps, terminal } => {
                terminal.source_receipt().or_else(|| {
                    steps
                        .iter()
                        .find_map(|step| step.occurrence.source_receipt())
                })
            }
            ConstraintOperandOrigin::ConstAlias { steps, terminal } => {
                terminal.source_receipt().or_else(|| {
                    steps
                        .iter()
                        .find_map(|step| step.occurrence.source_receipt())
                })
            }
            ConstraintOperandOrigin::FunctionSchemeInstantiation { source, .. } => {
                source.source_receipt()
            }
            _ => None,
        }
    }
}

enum ConstraintOperandAssociation<D: ConstraintDomain> {
    Value,
    Result(ConstraintResultProjection<D>),
}

impl<D: ConstraintDomain> ConstraintOperandOrigin<D> {
    fn associated_result(&self) -> Option<ConstraintResultProjection<D>> {
        match self.association() {
            Some(ConstraintOperandAssociation::Result(result)) => Some(result),
            Some(ConstraintOperandAssociation::Value) | None => None,
        }
    }

    fn association(&self) -> Option<ConstraintOperandAssociation<D>> {
        match self {
            Self::Result(result) => Some(ConstraintOperandAssociation::Result(result.projection())),
            Self::Source { value, .. } => Some(match value.projection() {
                Some(result) => ConstraintOperandAssociation::Result(result),
                None => ConstraintOperandAssociation::Value,
            }),
            Self::SourceExpectation { actual, .. } => actual.origin.association(),
            Self::Projection { producer, .. } => producer.association(),
            Self::ConstOccurrence(root) => root.origin.association(),
            Self::Rebuilt { source, .. } => source.association(),
            Self::TypeAlias { steps, terminal } => steps
                .iter()
                .find_map(|step| step.occurrence.origin.association())
                .or_else(|| terminal.origin.association()),
            Self::ConstAlias { steps, terminal } => steps
                .iter()
                .find_map(|step| step.occurrence.origin.association())
                .or_else(|| terminal.origin.association()),
            Self::FunctionSchemeInstantiation { source, .. } => source.origin.association(),
            _ => None,
        }
    }
}
