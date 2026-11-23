//! The original declaration and actual ambient environment of a producer.

use std::{any::Any, borrow::Cow, fmt::Debug, sync::Arc};

use crate::{
    effect_row::EffectPredicate,
    types::{
        GenericConstReference, GenericDeclarationBinder, GenericEffectReference, GenericScope,
        GenericScopeError, GenericTypeReference, TypeProjectionError,
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

impl OriginalRigidScope<'_> {
    fn declares(&self, parameter: &super::ConstraintGenericParameterId) -> bool {
        match parameter {
            super::ConstraintGenericParameterId::Type(reference) => {
                self.types.binary_search(reference).is_ok()
            }
            super::ConstraintGenericParameterId::Const(reference) => {
                self.consts.binary_search(reference).is_ok()
            }
            super::ConstraintGenericParameterId::Effect(reference) => {
                self.effects.binary_search(reference).is_ok()
            }
        }
    }
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
    gamma: Arc<ConstraintLexicalScope>,
    /// Bound references in the source predicate use its authored frames.
    /// Keep those frames separately when imported frames extend Gamma.
    domain_scope: Option<Arc<ConstraintLexicalScope>>,
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
                && self.0.gamma == other.0.gamma
                && self.0.domain_scope == other.0.domain_scope)
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
    pub(super) fn rigid_scope(&self) -> OriginalRigidScope<'_> {
        self.0.source.rigid_scope()
    }

    pub(super) fn validate_imported_in<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        inventory: &super::application::ConstraintApplicationInventory<D>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<(), TypeConstraintError> {
        match self.0.source.rigid_scope().imported {
            Some(imported) => imported.validate_in(inventory, context),
            None => Ok(()),
        }
    }

    pub(super) fn declares_parameter(
        &self,
        parameter: &super::ConstraintGenericParameterId,
    ) -> bool {
        self.rigid_scope().declares(parameter)
    }

    pub(super) fn admits_parameter(&self, parameter: &super::ConstraintGenericParameterId) -> bool {
        let scope = self.rigid_scope();
        scope.declares(parameter)
            || scope
                .imported
                .is_some_and(|lease| lease.admits_original_parameter(parameter))
    }

    pub(crate) fn belongs_to<S: OriginalProducerSource>(&self, source: &Arc<S>) -> bool {
        self.0.source.same_source(source.as_ref())
    }

    pub(crate) fn source_as<S: OriginalProducerSource>(&self) -> Option<&S> {
        self.0.source.as_any().downcast_ref()
    }
    #[cfg(test)]
    pub(super) fn ground_fixture(gamma: ConstraintLexicalScope) -> Self {
        Self(Arc::new(OriginalProducer {
            source: Arc::new(FixtureOriginalSource::Ground { imported: None }),
            formals: GenericDeclarationBinder::new(
                GenericScope::default(),
                Box::new([]),
                Box::new([]),
                Box::new([]),
            )
            .unwrap(),
            gamma: Arc::new(gamma),
            domain_scope: None,
        }))
    }

    #[cfg(test)]
    pub(crate) fn domain(&self) -> Cow<'_, EffectPredicate> {
        self.0.source.domain()
    }

    pub(super) fn project_domain<A: TypeConstraintAccounting, D: ConstraintDomain>(
        &self,
        target: &ConstraintLexicalScope,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<EffectPredicate, TypeProjectionError<TypeConstraintError>> {
        let domain = self.0.source.domain();
        crate::types::projection_control::visit_effect_predicate(context, &domain, 1)?;
        let source = self.0.domain_scope.as_deref().unwrap_or(self.gamma());
        target.transport_predicate(source, &domain, context)
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

    pub(super) fn retain_gamma(&self) -> Arc<ConstraintLexicalScope> {
        Arc::clone(&self.0.gamma)
    }

    /// Shape in which the producer's original prepared values occur.
    pub(crate) fn occurrence_scope(&self) -> &GenericScope {
        self.0.gamma.shape()
    }
}

impl<A: TypeConstraintAccounting, D: ConstraintDomain> TypeConstraintContext<'_, A, D> {
    #[cfg(test)]
    pub(crate) fn fixture_original_context(
        &mut self,
        imported: Option<&ImportedGenericParameterScopeLease>,
    ) -> Result<OriginalProducerContext, TypeConstraintError> {
        self.original_producer_context(&Arc::new(FixtureOriginalSource::Ground {
            imported: imported.cloned(),
        }))
    }

    /// Fixture declarations are supplied independently of the value being
    /// admitted and of the receiving candidate's application inventory.
    #[cfg(test)]
    pub(crate) fn fixture_original_context_with_rigid(
        &mut self,
        rigid: OriginalRigidScope<'_>,
    ) -> Result<OriginalProducerContext, TypeConstraintError> {
        self.original_producer_context(&Arc::new(FixtureOriginalSource::Declared {
            identity: Arc::new(()),
            types: rigid.types.into(),
            consts: rigid.consts.into(),
            effects: rigid.effects.into(),
            imported: rigid.imported.cloned(),
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
        if rigid.types.windows(2).any(|pair| pair[0] >= pair[1])
            || rigid.consts.windows(2).any(|pair| pair[0] >= pair[1])
            || rigid.effects.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(TypeConstraintInvariant::OriginalProducer.into());
        }
        let mut has_bound_domain = false;
        for variable in source.domain().variables() {
            self.enter_node()?;
            match variable {
                GenericEffectReference::Bound(bound) => {
                    self.lexical_frames()
                        .shape()
                        .bound_effect(bound.depth(), bound.slot())?;
                    has_bound_domain = true;
                }
                GenericEffectReference::Free(_)
                    if rigid.effects.binary_search(variable).is_ok() => {}
                _ => return Err(TypeConstraintInvariant::OriginalProducer.into()),
            }
        }
        let mut gamma = self.capture_lexical_scope()?;
        if let Some(imported) = rigid.imported {
            imported.merge_original_gamma(&mut gamma, self)?;
        }
        let domain_scope = if has_bound_domain && &gamma != self.lexical_frames() {
            Some(Arc::new(self.capture_lexical_scope()?))
        } else {
            None
        };
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
            gamma: Arc::new(gamma),
            domain_scope,
        })))
    }
}

/// Explicit declarations and imports are test inputs, never inferred from
/// the value or from a candidate that happens to admit the same reference.
#[cfg(test)]
#[derive(Debug)]
enum FixtureOriginalSource {
    Ground {
        imported: Option<ImportedGenericParameterScopeLease>,
    },
    Declared {
        identity: Arc<()>,
        types: Box<[GenericTypeReference]>,
        consts: Box<[GenericConstReference]>,
        effects: Box<[GenericEffectReference]>,
        imported: Option<ImportedGenericParameterScopeLease>,
    },
}

#[cfg(test)]
impl OriginalProducerSource for FixtureOriginalSource {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn same_source(&self, other: &dyn OriginalProducerSource) -> bool {
        match (self, other.as_any().downcast_ref::<Self>()) {
            (Self::Ground { imported }, Some(Self::Ground { imported: other })) => {
                imported == other
            }
            (
                Self::Declared { identity, .. },
                Some(Self::Declared {
                    identity: other, ..
                }),
            ) => Arc::ptr_eq(identity, other),
            _ => false,
        }
    }
    fn rigid_scope(&self) -> OriginalRigidScope<'_> {
        match self {
            Self::Ground { imported } => OriginalRigidScope {
                types: &[],
                consts: &[],
                effects: &[],
                imported: imported.as_ref(),
            },
            Self::Declared {
                types,
                consts,
                effects,
                imported,
                ..
            } => OriginalRigidScope {
                types,
                consts,
                effects,
                imported: imported.as_ref(),
            },
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
        imported: Option<ImportedGenericParameterScopeLease>,
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
                imported: self.imported.as_ref(),
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
            imported: None,
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

    #[test]
    fn source_domain_uses_the_authored_effect_frame_when_imports_extend_gamma() {
        use super::super::{TypeConstraintParameterEligibility, TypeConstraintParameterScope};
        use crate::{
            effect_row::{EffectFormula, prepared::PreparedEffectGraph},
            types::{
                DetachedGenericOwnerId, GenericBinder, GenericParameterOwnerId,
                GenericTypeParameterId,
            },
        };

        let cancellation = AtomicBool::new(false);
        let mut context = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(4096, 4096, 128, 128),
                &cancellation,
            ),
        );
        let mut graph = PreparedEffectGraph::default();
        let binder = GenericBinder::new(0, 0, 1);
        let key = GenericTypeParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(9013)),
            0,
        );
        let parameters = TypeConstraintParameterScope::new([(
            key.clone(),
            TypeConstraintParameterEligibility::Bindable,
        )])
        .unwrap();
        let reference = parameters.type_reference(&key.into()).unwrap();
        let imported = context
            .with_binder(binder, |context| {
                let owner = context.fixture_application_scope((), parameters, &mut graph)?;
                let path = context.start_path(owner, &mut graph)?;
                path.applications
                    .import_parameters(&[reference.into()], context)
            })
            .unwrap();

        let authored_scope = GenericScope::default().with_binder(binder);
        let authored = authored_scope.bound_effect(0, 0).unwrap();
        let domain = EffectFormula::variable(authored.clone(), &mut context)
            .unwrap()
            .subset(&EffectFormula::empty(), &mut context)
            .unwrap();
        let source = Arc::new(FixtureSource {
            projections: Arc::new(AtomicUsize::new(0)),
            domain,
            imported: Some(imported),
        });
        assert!(matches!(
            context.original_producer_context(&source),
            Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::GenericScope(_)
            ))
        ));
        assert_eq!(source.projections.load(Ordering::Relaxed), 0);

        context
            .with_binder(binder, |context| {
                let authored_frames = context.capture_lexical_scope()?;
                let original = context.original_producer_context(&source)?;
                let source_coordinate = original.scope().bound_effect(1, 0)?;
                assert_eq!(
                    original
                        .project_domain(original.gamma(), context)
                        .unwrap()
                        .variables()
                        .cloned()
                        .collect::<Vec<_>>(),
                    [source_coordinate]
                );
                assert_eq!(
                    original.project_domain(&authored_frames, context).unwrap(),
                    source.domain,
                );
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn explicit_fixture_declarations_preserve_identity_and_reject_invalid_rigid_inputs() {
        use crate::types::{
            DetachedGenericOwnerId, GenericBinder, GenericParameterOwnerId, GenericTypeParameterId,
        };
        let cancellation = AtomicBool::new(false);
        let mut context = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(100, 100, 100, 100),
                &cancellation,
            ),
        );
        let reference: GenericTypeReference = GenericTypeParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(9012)),
            0,
        )
        .into();
        let declarations = [reference.clone()];
        let rigid = || OriginalRigidScope {
            types: &declarations,
            consts: &[],
            effects: &[],
            imported: None,
        };
        let first = context
            .fixture_original_context_with_rigid(rigid())
            .unwrap();
        let second = context
            .fixture_original_context_with_rigid(rigid())
            .unwrap();
        assert_eq!(first, first.clone());
        assert_ne!(first, second);
        assert!(first.admits_parameter(&reference.clone().into()));
        let duplicates = [reference.clone(), reference];
        assert_eq!(
            context
                .fixture_original_context_with_rigid(OriginalRigidScope {
                    types: &duplicates,
                    consts: &[],
                    effects: &[],
                    imported: None,
                })
                .unwrap_err(),
            TypeConstraintError::Invariant(TypeConstraintInvariant::OriginalProducer)
        );
        let bound = GenericScope::default()
            .with_binder(GenericBinder::new(0, 0, 1))
            .bound_effect(0, 0)
            .unwrap();
        assert_eq!(
            context
                .fixture_original_context_with_rigid(OriginalRigidScope {
                    types: &[],
                    consts: &[],
                    effects: &[bound],
                    imported: None,
                })
                .unwrap_err(),
            TypeConstraintError::Invariant(TypeConstraintInvariant::OriginalProducer)
        );
    }
}
