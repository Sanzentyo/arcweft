//! Scoped equation terms retain their exact source or template authority.

use super::*;
use crate::types::constraints::{OriginalProducerContext, lexical::ConstraintLexicalScope};

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
        result: Option<ResolvedConstraintResult<D>>,
    },
    Result(ResolvedConstraintResult<D>),
    Derived {
        producer: Arc<ConstraintOperandOrigin<D>>,
        input: Arc<ConstraintOperandOrigin<D>>,
    },
}

pub(super) struct ConstraintOperand<D: ConstraintDomain> {
    value: TypeKind,
    scope: Arc<ConstraintLexicalScope>,
    origin: Arc<ConstraintOperandOrigin<D>>,
}

impl<D: ConstraintDomain> Clone for ConstraintOperand<D> {
    fn clone(&self) -> Self {
        Self {
            value: self.value.clone(),
            scope: Arc::clone(&self.scope),
            origin: Arc::clone(&self.origin),
        }
    }
}

impl<D: ConstraintDomain> ConstraintOperand<D> {
    pub(super) fn original(
        value: TypeKind,
        scope: Arc<ConstraintLexicalScope>,
        original: OriginalProducerContext,
    ) -> Self {
        Self {
            value,
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Original(original)),
        }
    }

    pub(super) fn template(
        value: TypeKind,
        scope: Arc<ConstraintLexicalScope>,
        owner: &Arc<ConstraintApplicationScope<D>>,
    ) -> Self {
        Self {
            value,
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Template(Arc::clone(owner))),
        }
    }

    pub(super) fn source(
        value: TypeKind,
        scope: Arc<ConstraintLexicalScope>,
        original: OriginalProducerContext,
        receipt: ConstraintSourceReceipt<D>,
        result: Option<ResolvedConstraintResult<D>>,
    ) -> Self {
        Self {
            value,
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Source {
                original,
                receipt,
                result,
            }),
        }
    }

    pub(super) fn result(
        value: TypeKind,
        scope: Arc<ConstraintLexicalScope>,
        result: ResolvedConstraintResult<D>,
    ) -> Self {
        Self {
            value,
            scope,
            origin: Arc::new(ConstraintOperandOrigin::Result(result)),
        }
    }

    pub(super) fn value(&self) -> &TypeKind {
        &self.value
    }

    /// Retain the input used by a typed composition or specialization. This
    /// records a value-flow edge, not an application-wide permission union.
    pub(super) fn derived_from(mut self, input: &Self) -> Self {
        self.origin = Arc::new(ConstraintOperandOrigin::Derived {
            producer: self.origin,
            input: Arc::clone(&input.origin),
        });
        self
    }

    pub(super) fn scope(&self) -> &ConstraintLexicalScope {
        &self.scope
    }

    /// A shared projection/closure keeps the exact originating authority.
    /// This is internal to the lower's derivations, not a source constructor.
    pub(super) fn projected(&self, value: TypeKind) -> Self {
        Self {
            value,
            scope: Arc::clone(&self.scope),
            origin: Arc::clone(&self.origin),
        }
    }

    pub(super) fn transport<A: TypeConstraintAccounting>(
        &self,
        target: &Arc<ConstraintLexicalScope>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, crate::types::TypeProjectionError<TypeConstraintError>> {
        Ok(Self {
            value: target.transport_type(&self.scope, &self.value, context)?,
            scope: Arc::clone(target),
            origin: Arc::clone(&self.origin),
        })
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
            | ConstraintOperandOrigin::Source { original, .. } => original,
            ConstraintOperandOrigin::Template(owner) => owner.original_context(),
            ConstraintOperandOrigin::Result(result) => result.owner.original_context(),
            ConstraintOperandOrigin::Derived { producer, .. } => producer.original_context(),
        }
    }

    fn result_origin(&self) -> Option<ConstraintResultProjection<D>> {
        match self {
            ConstraintOperandOrigin::Result(result)
            | ConstraintOperandOrigin::Source {
                result: Some(result),
                ..
            } => Some(result.projection()),
            ConstraintOperandOrigin::Derived { producer, input } => {
                producer.result_origin().or_else(|| input.result_origin())
            }
            _ => None,
        }
    }

    fn source_receipt(&self) -> Option<&ConstraintSourceReceipt<D>> {
        match self {
            ConstraintOperandOrigin::Source { receipt, .. } => Some(receipt),
            ConstraintOperandOrigin::Derived { producer, input } => {
                producer.source_receipt().or_else(|| input.source_receipt())
            }
            _ => None,
        }
    }
}
