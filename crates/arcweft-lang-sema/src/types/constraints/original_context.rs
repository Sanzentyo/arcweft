//! The original declaration and actual ambient environment of a producer.

use std::{any::Any, borrow::Cow, fmt::Debug, sync::Arc};

use crate::{
    effect_row::EffectPredicate,
    types::{
        GenericConstReference, GenericDeclarationBinder, GenericEffectReference, GenericScope,
        GenericScopeError, GenericTypeReference,
    },
};

use super::{
    ConstraintDomain, ImportedGenericParameterScopeLease, TypeConstraintError,
    TypeConstraintInvariant,
    context::{TypeConstraintAccounting, TypeConstraintContext},
    lexical::ConstraintLexicalScope,
};

/// Declaration formals and named captures admitted by the actual source owner.
/// This view borrows its owner; it does not establish another declaration table.
pub(crate) struct OriginalRigidScope<'a> {
    pub(crate) types: &'a [GenericTypeReference],
    pub(crate) consts: &'a [GenericConstReference],
    pub(crate) effects: &'a [GenericEffectReference],
    pub(crate) imported: Option<&'a ImportedGenericParameterScopeLease>,
}

/// The source layer implements this boundary using its accepted declaration
/// lease. No callable or analyzer types enter this lower layer, as with
/// imported application authority. A candidate schema is not a source.
pub(crate) trait OriginalProducerSource: Any + Debug {
    fn as_any(&self) -> &dyn Any;
    fn same_source(&self, other: &dyn OriginalProducerSource) -> bool;
    fn rigid_scope(&self) -> OriginalRigidScope<'_>;
    fn declaration_formals(&self) -> Result<GenericDeclarationBinder, GenericScopeError>;
    fn domain(&self) -> Cow<'_, EffectPredicate>;
}

struct OriginalProducer {
    source: Arc<dyn OriginalProducerSource>,
    formals: GenericDeclarationBinder,
    gamma: ConstraintLexicalScope,
}

/// Shared unchanged across a root's branches, suspension and original value
/// registration. Inner function binders are not added to this ambient lease.
#[derive(Clone)]
pub(crate) struct OriginalProducerContext(Arc<OriginalProducer>);

impl PartialEq for OriginalProducerContext {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
            || (self.0.source.same_source(other.0.source.as_ref())
                && self.0.formals == other.0.formals
                && self.0.gamma == other.0.gamma)
    }
}

impl Eq for OriginalProducerContext {}

impl Debug for OriginalProducerContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OriginalProducerContext")
            .field("formals", &self.0.formals)
            .field("gamma", &self.0.gamma)
            .finish_non_exhaustive()
    }
}

impl OriginalProducerContext {
    #[cfg(test)]
    pub(super) fn ground_fixture(gamma: ConstraintLexicalScope) -> Self {
        Self(Arc::new(OriginalProducer {
            source: Arc::new(GroundFixtureSource { imported: None }),
            formals: GenericDeclarationBinder::new(
                GenericScope::default(),
                Box::new([]),
                Box::new([]),
                Box::new([]),
            )
            .unwrap(),
            gamma,
        }))
    }
    pub(crate) fn domain(&self) -> Cow<'_, EffectPredicate> {
        self.0.source.domain()
    }

    pub(crate) fn declaration_formals(&self) -> &GenericDeclarationBinder {
        &self.0.formals
    }

    pub(crate) fn scope(&self) -> &GenericScope {
        self.0.gamma.shape()
    }

    pub(super) fn gamma(&self) -> &ConstraintLexicalScope {
        &self.0.gamma
    }
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain> TypeConstraintContext<'_, A, D> {
    #[cfg(test)]
    pub(crate) fn fixture_original_context(
        &mut self,
        imported: Option<&ImportedGenericParameterScopeLease>,
    ) -> Result<OriginalProducerContext, TypeConstraintError> {
        self.original_producer_context(&Arc::new(GroundFixtureSource {
            imported: imported.cloned(),
        }))
    }
    /// Reserve the source/header and frame work before projecting or sharing
    /// the declaration transport. Only its own formals are quantified; the
    /// imported/captured frames remain part of the real ambient environment.
    pub(crate) fn original_producer_context<S: OriginalProducerSource>(
        &mut self,
        source: &Arc<S>,
    ) -> Result<OriginalProducerContext, TypeConstraintError> {
        self.enter_node()?;
        let rigid = source.rigid_scope();
        for parameter in rigid.types {
            self.enter_node()?;
            if parameter.free_parameter().is_none() {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::OriginalProducer,
                ));
            }
        }
        for parameter in rigid.consts {
            self.enter_node()?;
            if parameter.free_parameter().is_none() {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::OriginalProducer,
                ));
            }
        }
        for parameter in rigid.effects {
            self.enter_node()?;
            if parameter.free_parameter().is_none() {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::OriginalProducer,
                ));
            }
        }
        for variable in source.domain().variables() {
            self.enter_node()?;
            if rigid.effects.binary_search(variable).is_err() {
                return Err(TypeConstraintError::Invariant(
                    TypeConstraintInvariant::OriginalProducer,
                ));
            }
        }
        let mut gamma = self.capture_lexical_scope()?;
        if let Some(imported) = rigid.imported {
            imported.merge_original_gamma(&mut gamma, self)?;
        }
        let formals = source.declaration_formals()?;
        if formals
            .type_parameters()
            .iter()
            .any(|parameter| rigid.types.binary_search(parameter).is_err())
            || formals
                .const_parameters()
                .iter()
                .any(|parameter| rigid.consts.binary_search(parameter).is_err())
            || formals
                .effect_parameters()
                .iter()
                .any(|parameter| rigid.effects.binary_search(parameter).is_err())
        {
            return Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::OriginalProducer,
            ));
        }
        let retained_source = Arc::clone(source);
        Ok(OriginalProducerContext(Arc::new(OriginalProducer {
            source: retained_source,
            formals,
            gamma,
        })))
    }
}

/// Explicit ground source used by lower fixtures without a source catalog.
/// Production initializers must receive an accepted source-layer authority.
#[cfg(test)]
#[derive(Debug)]
struct GroundFixtureSource {
    imported: Option<ImportedGenericParameterScopeLease>,
}

#[cfg(test)]
impl OriginalProducerSource for GroundFixtureSource {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn same_source(&self, other: &dyn OriginalProducerSource) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| self.imported == other.imported)
    }
    fn rigid_scope(&self) -> OriginalRigidScope<'_> {
        OriginalRigidScope {
            types: &[],
            consts: &[],
            effects: &[],
            imported: self.imported.as_ref(),
        }
    }
    fn declaration_formals(&self) -> Result<GenericDeclarationBinder, GenericScopeError> {
        GenericDeclarationBinder::new(
            GenericScope::default(),
            Box::new([]),
            Box::new([]),
            Box::new([]),
        )
    }
    fn domain(&self) -> Cow<'_, EffectPredicate> {
        Cow::Owned(EffectPredicate::unconstrained())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::constraints::{
        NoConstraintClient,
        context::{LocalConstraintAccounting, TypeConstraintLimits},
    };
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    #[derive(Debug)]
    struct FixtureSource {
        projections: Arc<AtomicUsize>,
        domain: EffectPredicate,
    }

    impl OriginalProducerSource for FixtureSource {
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn same_source(&self, other: &dyn OriginalProducerSource) -> bool {
            other
                .as_any()
                .downcast_ref::<Self>()
                .is_some_and(|other| Arc::ptr_eq(&self.projections, &other.projections))
        }
        fn rigid_scope(&self) -> OriginalRigidScope<'_> {
            OriginalRigidScope {
                types: &[],
                consts: &[],
                effects: &[],
                imported: None,
            }
        }
        fn declaration_formals(&self) -> Result<GenericDeclarationBinder, GenericScopeError> {
            self.projections.fetch_add(1, Ordering::Relaxed);
            GenericDeclarationBinder::new(
                GenericScope::default(),
                Box::new([]),
                Box::new([]),
                Box::new([]),
            )
        }
        fn domain(&self) -> Cow<'_, EffectPredicate> {
            Cow::Borrowed(&self.domain)
        }
    }

    #[test]
    fn original_source_context_preflights_before_declaration_projection() {
        let projections = Arc::new(AtomicUsize::new(0));
        let source = Arc::new(FixtureSource {
            projections: Arc::clone(&projections),
            domain: EffectPredicate::unconstrained(),
        });
        let cancellation = AtomicBool::new(false);
        let mut exhausted = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
            LocalConstraintAccounting::new(TypeConstraintLimits::new(0, 0, 0, 0), &cancellation),
        );
        assert!(matches!(
            exhausted.original_producer_context(&source),
            Err(TypeConstraintError::Abort(_))
        ));
        assert_eq!(projections.load(Ordering::Relaxed), 0);
        let mut allowed = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(100, 100, 100, 100),
                &cancellation,
            ),
        );
        let context = allowed.original_producer_context(&source).unwrap();
        assert_eq!(projections.load(Ordering::Relaxed), 1);
        assert!(context.domain().is_unconstrained());
    }
}
