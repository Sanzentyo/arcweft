//! Scoped equation terms retain their exact source or template authority.

use super::*;
use crate::types::constraints::{OriginalProducerContext, lexical::ConstraintLexicalScope};

mod projection;
pub(in crate::types::constraints) use projection::ConstraintOperandChildren;
use projection::ConstraintOperandDerivation;
use projection::ConstraintOperandProjection;
pub(in crate::types) use projection::{ConstraintConstOperandView, ConstraintOperandView};
pub(in crate::types::constraints) use projection::{
    ConstraintOperandRebuild, ConstraintProjectedOperand,
};

pub(super) enum ResolvedConstraintSource<D: ConstraintDomain> {
    Value(Arc<dyn super::super::ConstraintSourceValueOwner<D>>),
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
    pub(super) fn resolve<A: TypeConstraintAccounting>(
        result: ConstraintResultProjection<D>,
        path: &ConstraintPath<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(TypeKind, Self), TypeConstraintError> {
        let owner = path
            .applications
            .require_application_owner(result.application)?;
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
        let value = context.open_template_type(&request.value, path, result.application)?;
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
    Original(OriginalProducerContext),
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
    Derived {
        producer: Arc<ConstraintOperandOrigin<D>>,
        input: Arc<ConstraintOperandOrigin<D>>,
    },
    Projection {
        root: Arc<TypeKind>,
        root_scope: Arc<ConstraintLexicalScope>,
        occurrence_scope: Arc<ConstraintLexicalScope>,
        producer: Arc<ConstraintOperandOrigin<D>>,
        address: Option<Arc<ConstraintOperandProjection>>,
    },
    ConstOccurrence(ConstraintOperand<D, ArrayLength>),
    Rebuilt {
        source: Arc<ConstraintOperandOrigin<D>>,
        contents_scope: Arc<ConstraintLexicalScope>,
        children: Box<[ConstraintOperandDerivation<D, TypeKind>]>,
        length: Option<ConstraintOperandDerivation<D, ArrayLength>>,
    },
    TypeAlias {
        steps: Box<[ConstraintAliasStep<D, TypeKind>]>,
        terminal: ConstraintOperandDerivation<D, TypeKind>,
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

pub(in crate::types) struct ConstraintOperand<D: ConstraintDomain, T = TypeKind> {
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

    pub(in crate::types::constraints) fn original(
        value: T,
        scope: Arc<ConstraintLexicalScope>,
        original: OriginalProducerContext,
    ) -> Self {
        Self {
            value: Arc::new(value),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Original(original)),
        }
    }

    pub(in crate::types) fn value(&self) -> &T {
        &self.value
    }

    /// Retain the input used by a typed composition or specialization. This
    /// records a value-flow edge, not an application-wide permission union.
    pub(super) fn derived_from<U>(mut self, input: &ConstraintOperand<D, U>) -> Self {
        self.origin = Arc::new(ConstraintOperandOrigin::Derived {
            producer: self.origin,
            input: Arc::clone(&input.origin),
        });
        self
    }

    pub(in crate::types::constraints) fn scope(&self) -> &ConstraintLexicalScope {
        &self.scope
    }

    pub(in crate::types::constraints) fn shared_scope(&self) -> Arc<ConstraintLexicalScope> {
        Arc::clone(&self.scope)
    }

    /// A shared projection/closure keeps the exact originating authority.
    /// This is internal to the lower's derivations, not a source constructor.
    pub(in crate::types::constraints) fn projected(&self, value: T) -> Self {
        Self {
            value: Arc::new(value),
            scope: Arc::clone(&self.scope),
            origin: Arc::clone(&self.origin),
        }
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

impl<D: ConstraintDomain> ConstraintOperand<D> {
    pub(in crate::types::constraints) fn aliased(
        mut self,
        steps: Box<[ConstraintAliasStep<D, TypeKind>]>,
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
    pub(super) fn template(
        value: TypeKind,
        scope: Arc<ConstraintLexicalScope>,
        owner: &Arc<ConstraintApplicationScope<D>>,
    ) -> Self {
        Self {
            value: Arc::new(value),
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Template(Arc::clone(owner))),
        }
    }

    pub(super) fn source(
        value: TypeKind,
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
        value: TypeKind,
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
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, crate::types::TypeProjectionError<TypeConstraintError>> {
        Ok(Self {
            value: Arc::new(target.transport_type(&self.scope, &self.value, context)?),
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
        self.origin.result_origin()
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
            ConstraintOperandOrigin::Original(original)
            | ConstraintOperandOrigin::Source { original, .. }
            | ConstraintOperandOrigin::Completed { original, .. } => original,
            ConstraintOperandOrigin::Template(owner) => owner.original_context(),
            ConstraintOperandOrigin::Result(result) => result.owner.original_context(),
            ConstraintOperandOrigin::Derived { producer, .. } => producer.original_context(),
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

    fn result_origin(&self) -> Option<ConstraintResultProjection<D>> {
        match self {
            ConstraintOperandOrigin::Result(result) => Some(result.projection()),
            ConstraintOperandOrigin::Source { value, .. } => value.projection(),
            ConstraintOperandOrigin::Projection { producer, .. } => producer.result_origin(),
            ConstraintOperandOrigin::ConstOccurrence(root) => root.result_origin(),
            ConstraintOperandOrigin::Rebuilt { source, .. } => source.result_origin(),
            ConstraintOperandOrigin::TypeAlias { steps, terminal } => {
                terminal.result_origin().or_else(|| {
                    steps
                        .iter()
                        .find_map(|step| step.occurrence.result_origin())
                })
            }
            ConstraintOperandOrigin::ConstAlias { steps, terminal } => {
                terminal.result_origin().or_else(|| {
                    steps
                        .iter()
                        .find_map(|step| step.occurrence.result_origin())
                })
            }
            ConstraintOperandOrigin::Derived { producer, input } => {
                producer.result_origin().or_else(|| input.result_origin())
            }
            _ => None,
        }
    }

    fn source_receipt(&self) -> Option<&ConstraintSourceReceipt<D>> {
        match self {
            ConstraintOperandOrigin::Source { receipt, .. } => Some(receipt),
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
            ConstraintOperandOrigin::Derived { producer, input } => {
                producer.source_receipt().or_else(|| input.source_receipt())
            }
            _ => None,
        }
    }
}
