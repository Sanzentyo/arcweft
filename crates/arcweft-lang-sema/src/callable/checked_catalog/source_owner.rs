//! The accepted source root that owns an original constraint operation.

use arcweft_lang_hir::project::{
    HirProjectEvaluationTopology, HirSemanticPathOwnerId, HirSemanticPathRoot,
};

use super::*;

/// Keeps the original caller's immutable declaration, rather than the schema
/// of whichever candidate is currently being considered. Item roots retain
/// their actual HIR entry without inventing a callable declaration for them.
#[derive(Clone, Debug)]
pub(crate) struct CheckedConstraintSourceOwner {
    topology: Arc<HirProjectEvaluationTopology>,
    authority: CheckedCallableAuthorityLease,
    root: HirSemanticPathRoot,
    contract: SourceContract,
}

#[derive(Clone, Debug)]
enum SourceContract {
    Declaration {
        id: CheckedCallableId,
        record: Arc<CallableRecord>,
    },
    Item,
}

impl PartialEq for CheckedConstraintSourceOwner {
    fn eq(&self, other: &Self) -> bool {
        self.authority == other.authority
            && self.root == other.root
            && match (&self.contract, &other.contract) {
                (
                    SourceContract::Declaration {
                        id: left,
                        record: left_record,
                    },
                    SourceContract::Declaration {
                        id: right,
                        record: right_record,
                    },
                ) => left == right && Arc::ptr_eq(left_record, right_record),
                (SourceContract::Item, SourceContract::Item) => true,
                _ => false,
            }
    }
}

impl Eq for CheckedConstraintSourceOwner {}

impl CheckedConstraintSourceOwner {
    pub(crate) fn domain(&self) -> std::borrow::Cow<'_, crate::effect_row::EffectPredicate> {
        match &self.contract {
            SourceContract::Declaration { record, .. } => {
                std::borrow::Cow::Borrowed(record.schema().effect_predicate())
            }
            // This is an authenticated, non-generic item root, not a missing
            // declaration whose contract could be supplied by a candidate.
            SourceContract::Item => {
                std::borrow::Cow::Owned(crate::effect_row::EffectPredicate::unconstrained())
            }
        }
    }

    pub(crate) fn declaration_formals(
        &self,
    ) -> Result<crate::types::GenericDeclarationBinder, crate::types::GenericScopeError> {
        match &self.contract {
            SourceContract::Declaration { record, .. } => record.schema().function_value_binder(),
            SourceContract::Item => crate::types::GenericDeclarationBinder::new(
                crate::types::GenericScope::default(),
                Box::new([]),
                Box::new([]),
                Box::new([]),
            ),
        }
    }

    pub(crate) fn signature(&self) -> Option<&super::super::CallableSignatureSchema> {
        match &self.contract {
            SourceContract::Declaration { record, .. } => Some(record.schema()),
            SourceContract::Item => None,
        }
    }

    pub(crate) fn admits(&self, expression: ExprId) -> bool {
        self.topology
            .semantic_path(HirSemanticPathOwnerId::Expression(expression))
            .is_ok_and(|location| location.is_some_and(|location| location.root() == &self.root))
    }
}

impl CheckedCallableCatalogBuilder {
    pub(crate) fn constraint_source_owner(
        &self,
        topology: &Arc<HirProjectEvaluationTopology>,
        expression: ExprId,
    ) -> Result<CheckedConstraintSourceOwner, CheckedCallableCatalogBuildError> {
        let authority = CheckedCallableAuthorityLease {
            generation: self.generation.clone(),
            registered: self.registered.clone(),
        };
        if !authority.admits_generation(topology.generation()) || self.registered.is_none() {
            return Err(CheckedCallableCatalogBuildError::GenerationMismatch);
        }
        let location = topology
            .semantic_path(HirSemanticPathOwnerId::Expression(expression))
            .map_err(|_| CheckedCallableCatalogBuildError::SourceIdentityMismatch)?
            .ok_or(CheckedCallableCatalogBuildError::SourceIdentityMismatch)?;
        let contract = match location.root() {
            HirSemanticPathRoot::Declaration(declaration) => {
                let pending = self
                    .pending_by_candidate(&CallableCandidateId::Project(declaration.clone()))
                    .map_err(|_| CheckedCallableCatalogBuildError::MissingRegisteredRecord)?;
                // The sealed semantic path already identifies this actual
                // declaration's source. Defaults also belong to declarations
                // with fixed or bodyless contracts; body inference is not the
                // authority for source ownership.
                SourceContract::Declaration {
                    id: pending.id().clone(),
                    record: Arc::clone(pending.record()),
                }
            }
            HirSemanticPathRoot::Item { .. } => SourceContract::Item,
        };
        Ok(CheckedConstraintSourceOwner {
            topology: Arc::clone(topology),
            authority,
            root: location.root().clone(),
            contract,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::final_analysis::{
        FinalSemanticAnalysis,
        tests::{analyze, fixture},
    };
    use crate::types::constraints::{
        NoConstraintClient,
        context::{LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits},
    };

    fn builder(analysis: &FinalSemanticAnalysis) -> CheckedCallableCatalogBuilder {
        let checked = analysis.checked_callables();
        let mut builder = CheckedCallableCatalogBuilder::for_registered(
            Arc::clone(checked.registered_catalog().unwrap()),
            Arc::clone(analysis.hir_topology().generation()),
            checked.generation.standard,
        )
        .unwrap();
        for facts in checked.records() {
            match &facts.effects {
                CheckedCallableEffects::Body { contract, .. } => {
                    builder
                        .insert_body_shell(
                            Arc::clone(facts.record()),
                            contract.clone(),
                            &contract.source.anchor,
                        )
                        .unwrap();
                }
                CheckedCallableEffects::BodylessTraitRequirement { contract } => {
                    builder
                        .insert_bodyless_trait_shell(
                            Arc::clone(facts.record()),
                            facts.execution.clone(),
                            contract.clone(),
                        )
                        .unwrap();
                }
                CheckedCallableEffects::RecordFixed => {
                    builder
                        .insert_fixed_shell(Arc::clone(facts.record()), facts.execution.clone())
                        .unwrap();
                }
            }
        }
        builder.begin_inference().unwrap();
        builder
    }

    fn expression(analysis: &FinalSemanticAnalysis, name: &str) -> ExprId {
        analysis.hir_topology().expression_owners().find(|expression| {
            let location = analysis.hir_topology().semantic_path(HirSemanticPathOwnerId::Expression(*expression)).unwrap().unwrap();
            matches!(location.root(), HirSemanticPathRoot::Declaration(CallableDeclarationKey::Existing(declaration)) if declaration.name() == name)
        }).unwrap()
    }

    #[test]
    fn original_source_owner_keeps_the_callers_actual_declaration() {
        let world = fixture(
            "fn id<T>(value: T) -> T { value }\nfn left<T>(value: T) -> T { id(value) }\nfn right<T>(value: T) -> T { id(value) }",
            None,
        );
        let analysis = analyze(&world).unwrap();
        let builder = builder(&analysis);
        let topology = analysis.hir_topology();
        let left = expression(&analysis, "left");
        let right = expression(&analysis, "right");
        let source = builder.constraint_source_owner(topology, left).unwrap();
        assert!(source.admits(left));
        assert!(!source.admits(right));
        assert_eq!(source, source.clone());
        assert_eq!(
            source,
            builder.constraint_source_owner(topology, left).unwrap()
        );
        assert_ne!(
            source,
            builder.constraint_source_owner(topology, right).unwrap()
        );
        let SourceContract::Declaration { id, record } = &source.contract else {
            panic!("actual caller body")
        };
        let pending = builder.pending_by_id(id).unwrap();
        assert!(Arc::ptr_eq(record, pending.record()));
        assert!(std::ptr::eq(
            source.signature().unwrap(),
            pending.record().schema()
        ));
        let scope =
            super::super::super::EnclosingGenericParameterScope::for_source(source.clone(), None)
                .unwrap();
        assert_eq!(
            scope.types(),
            &[source.signature().unwrap().generic_inventory().types()[0]
                .parameter()
                .clone()]
        );
        let callee = builder
            .constraint_source_owner(topology, expression(&analysis, "id"))
            .unwrap();
        assert_ne!(
            scope.types(),
            super::super::super::EnclosingGenericParameterScope::for_source(callee, None)
                .unwrap()
                .types()
        );
        let binder = crate::types::GenericBinder::new(1, 0, 0);
        let local = crate::types::GenericScope::default().with_binder(binder);
        let parameter = TypeKind::GenericParam(local.bound_type(0, 0).unwrap());
        let function = TypeKind::function_with_binder(
            binder,
            [parameter.clone()],
            parameter,
            EffectRow::closed(EffectSet::new()),
        );
        let graph = super::super::super::PreparedCallGraph::<()>::new();
        assert!(
            graph
                .prepare_root_function_specialization(
                    left,
                    &function,
                    &scope,
                    &super::super::super::PRODUCTION_CALLABLE_LIMITS,
                )
                .is_ok()
        );
        assert!(matches!(
            graph.prepare_root_function_specialization(
                right,
                &function,
                &scope,
                &super::super::super::PRODUCTION_CALLABLE_LIMITS,
            ),
            Err(super::super::super::CallConstraintInvariant::PreparedCallSiteMismatch)
        ));
        drop(analysis);
        assert!(source.admits(left));
    }

    #[test]
    fn original_source_owner_rejects_a_foreign_accepted_generation() {
        let text = "fn id<T>(value: T) -> T { value }";
        let world = fixture(text, None);
        let analysis = analyze(&world).unwrap();
        let foreign_world = fixture(text, None);
        let foreign = analyze(&foreign_world).unwrap();
        let builder = builder(&analysis);
        assert!(matches!(
            builder.constraint_source_owner(foreign.hir_topology(), expression(&foreign, "id")),
            Err(CheckedCallableCatalogBuildError::GenerationMismatch)
        ));
        let source = builder
            .constraint_source_owner(analysis.hir_topology(), expression(&analysis, "id"))
            .unwrap();
        let mut separately_registered = builder;
        separately_registered.registered = Some(Arc::new(
            separately_registered
                .registered
                .as_ref()
                .unwrap()
                .as_ref()
                .clone(),
        ));
        let other = separately_registered
            .constraint_source_owner(analysis.hir_topology(), expression(&analysis, "id"))
            .unwrap();
        assert_ne!(
            source, other,
            "equal records do not replace the registered authority allocation"
        );
    }

    #[test]
    fn original_source_owner_keeps_non_callable_item_roots_distinct() {
        let world = fixture(
            "style First { token size: Length = 1px }\nstyle Second { token size: Length = 2px }",
            None,
        );
        let analysis = analyze(&world).unwrap();
        let builder = builder(&analysis);
        let topology = analysis.hir_topology();
        let items = topology
            .expression_owners()
            .filter(|expression| {
                topology
                    .semantic_path(HirSemanticPathOwnerId::Expression(*expression))
                    .unwrap()
                    .is_some_and(|location| {
                        matches!(location.root(), HirSemanticPathRoot::Item { .. })
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(items.len(), 2);
        let source = builder.constraint_source_owner(topology, items[0]).unwrap();
        assert!(matches!(source.contract, SourceContract::Item));
        assert!(source.signature().is_none());
        assert!(source.admits(items[0]));
        assert!(!source.admits(items[1]));
        assert_ne!(
            source,
            builder.constraint_source_owner(topology, items[1]).unwrap()
        );
    }

    #[test]
    fn original_source_owner_keeps_defaults_of_fixed_declarations() {
        let world = fixture(
            r##"#[fx]
fn emphasis(accent: Color = rgb("#ffd060")) -> Fx {
    Fx.text(color = accent)
}"##,
            None,
        );
        let analysis = analyze(&world).unwrap();
        let builder = builder(&analysis);
        let owner = expression(&analysis, "emphasis");
        let source = builder
            .constraint_source_owner(analysis.hir_topology(), owner)
            .unwrap();
        let SourceContract::Declaration { id, record } = &source.contract else {
            panic!("actual fixed declaration")
        };
        let pending = builder.pending_by_id(id).unwrap();
        assert!(matches!(
            pending.contract,
            PendingCallableEffectContract::RecordFixed
        ));
        assert!(Arc::ptr_eq(record, pending.record()));
        assert!(source.admits(owner));
        assert!(source.signature().is_some());
    }

    #[test]
    fn original_source_context_retains_real_frames_without_flattening_captures() {
        let world = fixture("fn identity<T>(value: T) -> T { value }", None);
        let analysis = analyze(&world).unwrap();
        let builder = builder(&analysis);
        let source = builder
            .constraint_source_owner(analysis.hir_topology(), expression(&analysis, "identity"))
            .unwrap();
        let enclosing = Arc::new(
            super::super::super::EnclosingGenericParameterScope::for_source(source.clone(), None)
                .unwrap(),
        );
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(10_000, 10_000, 100, 100),
                &cancellation,
            ),
        );
        let large = crate::types::GenericBinder::new(u16::MAX, 0, 0);
        let original = work
            .with_binder(large, |work| {
                work.with_binder(large, |work| {
                    let first = enclosing.original_context(work)?;
                    let shared = enclosing.original_context(work)?;
                    assert_eq!(first, shared);
                    Ok(first)
                })
            })
            .unwrap();
        assert_eq!(original.scope().binders(), &[large, large]);
        assert_eq!(
            original.declaration_formals().type_parameters(),
            enclosing.types()
        );
        assert_eq!(
            original.declaration_formals().scope().binders(),
            &[crate::types::GenericBinder::new(1, 0, 0)]
        );
        assert!(std::ptr::eq(
            original.domain().as_ref(),
            source.signature().unwrap().effect_predicate()
        ));
        let other = work
            .with_binder(large, |work| {
                work.with_binder(large, |work| enclosing.original_context(work))
            })
            .unwrap();
        assert_eq!(original.scope(), other.scope());
        assert_ne!(
            original, other,
            "an equally shaped opening is a different ambient environment"
        );
    }

    #[test]
    fn original_source_context_keeps_equal_ground_domains_owned_by_distinct_bodies() {
        let world = fixture("fn left() {}\nfn right() {}", None);
        let analysis = analyze(&world).unwrap();
        let builder = builder(&analysis);
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(1000, 1000, 100, 100),
                &cancellation,
            ),
        );
        let contexts = ["left", "right"].map(|name| {
            let source = builder
                .constraint_source_owner(analysis.hir_topology(), expression(&analysis, name))
                .unwrap();
            let enclosing =
                super::super::super::EnclosingGenericParameterScope::for_source(source, None)
                    .unwrap();
            enclosing.original_context(&mut work).unwrap()
        });
        assert_eq!(contexts[0].domain(), contexts[1].domain());
        assert_eq!(contexts[0].scope(), contexts[1].scope());
        assert_eq!(
            contexts[0].declaration_formals(),
            contexts[1].declaration_formals()
        );
        assert_ne!(contexts[0], contexts[1]);
    }
}
