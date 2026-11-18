//! The accepted source root that owns an original constraint operation.

use arcweft_lang_hir::project::{
    HirProjectEvaluationTopology, HirSemanticPathOwnerId, HirSemanticPathRoot,
};

use super::*;

/// Failure to authenticate the original declaration or item that owns a type.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CheckedConstraintSourceOwnerError {
    #[error("source owner belongs to another accepted generation")]
    GenerationMismatch,
    #[error("source owner does not match its registered callable")]
    CandidateMismatch,
    #[error("source owner has no registered callable record")]
    MissingRegisteredRecord,
    #[error("source owner does not retain the registered record allocation")]
    RecordPointerMismatch,
    #[error("source owner has no authenticated HIR semantic path")]
    SourceIdentityMismatch,
}

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
    /// Retain an accepted annotation in this actual producer's template
    /// scope. This transition carries omitted rows unchanged; it does not
    /// admit the value into a candidate or issue a completed type identity.
    pub(crate) fn retain_value_type<C: crate::types::TypeProjectionControl>(
        &self,
        ty: &TypeKind,
        control: &mut C,
    ) -> Result<TypeKind<crate::types::PreparedEffects>, crate::types::TypeProjectionError<C::Error>>
    {
        let formals = self.declaration_formals_with_control(control)?;
        ty.prepare_effects(formals.template_scope(), control)
    }

    pub(crate) fn declaration_formals_with_control<C: crate::types::TypeProjectionControl>(
        &self,
        control: &mut C,
    ) -> Result<crate::types::GenericDeclarationBinder, crate::types::TypeProjectionError<C::Error>>
    {
        control
            .check()
            .map_err(crate::types::TypeProjectionError::Control)?;
        control
            .visit_binding()
            .map_err(crate::types::TypeProjectionError::Control)?;
        if let Some(signature) = self.signature() {
            let inventory = signature.generic_inventory();
            // Charge before constructing the declaration binder's vectors.
            for count in [
                inventory.types().len(),
                inventory.consts().len(),
                inventory.effects().len(),
            ] {
                for _ in 0..count {
                    control
                        .visit_binding()
                        .map_err(crate::types::TypeProjectionError::Control)?;
                }
            }
        }
        self.declaration_formals().map_err(Into::into)
    }

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

impl CheckedConstraintSourceOwner {
    fn admit(
        authority: CheckedCallableAuthorityLease,
        topology: &Arc<HirProjectEvaluationTopology>,
        owner: HirSemanticPathOwnerId,
        declaration_record: impl FnOnce(
            &CallableDeclarationKey,
        ) -> Result<
            (CheckedCallableId, Arc<CallableRecord>),
            CheckedConstraintSourceOwnerError,
        >,
    ) -> Result<Self, CheckedConstraintSourceOwnerError> {
        if !authority.admits_generation(topology.generation()) || authority.registered.is_none() {
            return Err(CheckedConstraintSourceOwnerError::GenerationMismatch);
        }
        let location = topology
            .semantic_path(owner)
            .map_err(|_| CheckedConstraintSourceOwnerError::SourceIdentityMismatch)?
            .ok_or(CheckedConstraintSourceOwnerError::SourceIdentityMismatch)?;
        let contract = match location.root() {
            HirSemanticPathRoot::Declaration(declaration) => {
                let (id, record) = declaration_record(declaration)?;
                let candidate = CallableCandidateId::Project(declaration.clone());
                if record.id() != &candidate || !authority.admits_callable_id(&id) {
                    return Err(CheckedConstraintSourceOwnerError::CandidateMismatch);
                }
                let accepted = authority
                    .registered
                    .as_ref()
                    .expect("registered authority admitted")
                    .record(&candidate)
                    .ok_or(CheckedConstraintSourceOwnerError::MissingRegisteredRecord)?;
                if !Arc::ptr_eq(accepted, &record) {
                    return Err(CheckedConstraintSourceOwnerError::RecordPointerMismatch);
                }
                SourceContract::Declaration { id, record }
            }
            HirSemanticPathRoot::Item { .. } => SourceContract::Item,
        };
        Ok(Self {
            topology: Arc::clone(topology),
            authority,
            root: location.root().clone(),
            contract,
        })
    }
}

impl CheckedCallableCatalogBuilder {
    pub(crate) fn constraint_source_owner(
        &self,
        topology: &Arc<HirProjectEvaluationTopology>,
        owner: impl Into<HirSemanticPathOwnerId>,
    ) -> Result<CheckedConstraintSourceOwner, CheckedConstraintSourceOwnerError> {
        CheckedConstraintSourceOwner::admit(
            CheckedCallableAuthorityLease {
                generation: self.generation.clone(),
                registered: self.registered.clone(),
            },
            topology,
            owner.into(),
            |declaration| {
                let pending = self
                    .pending_by_candidate(&CallableCandidateId::Project(declaration.clone()))
                    .map_err(|_| CheckedConstraintSourceOwnerError::MissingRegisteredRecord)?;
                Ok((pending.id().clone(), Arc::clone(pending.record())))
            },
        )
    }
}

impl CheckedCallableCatalog {
    pub(crate) fn constraint_source_owner(
        &self,
        topology: &Arc<HirProjectEvaluationTopology>,
        owner: impl Into<HirSemanticPathOwnerId>,
    ) -> Result<CheckedConstraintSourceOwner, CheckedConstraintSourceOwnerError> {
        CheckedConstraintSourceOwner::admit(
            self.authority_lease(),
            topology,
            owner.into(),
            |declaration| {
                let facts = self
                    .project_callable(declaration)
                    .map_err(|_| CheckedConstraintSourceOwnerError::MissingRegisteredRecord)?;
                Ok((facts.id().clone(), Arc::clone(facts.record())))
            },
        )
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
    fn inferred_application_result_keeps_the_declaration_term_after_row_caching() {
        use crate::callable::{
            CallableInstantiation, PRODUCTION_CALLABLE_LIMITS, PreparedApplicationResult,
            PreparedCallGraph, PreparedCallableEffectDeclarations,
            PreparedCallableEffectProjectionSite, PreparedCatalogCallee, PreparedResolvedCallable,
            ResolverWork, map_call_arguments,
        };
        use crate::effect_row::prepared::PreparedEffectRow;
        use crate::types::TypeKind;
        use arcweft_lang_hir::expr::HirExprKind;

        let world = fixture(
            "fn staged(first: i64)(second: i64) -> i64 { first + second }\n\
             fn caller() { let _ = staged(1i64); }",
            None,
        );
        let analysis = analyze(&world).unwrap();
        let builder = builder(&analysis);
        let source = builder
            .constraint_source_owner(analysis.hir_topology(), expression(&analysis, "staged"))
            .unwrap();
        let SourceContract::Declaration { id, record } = &source.contract else {
            panic!("staged declaration");
        };
        let candidate = PreparedResolvedCallable::try_from_checked_record(
            id.clone(),
            Arc::clone(record),
            PreparedCatalogCallee::Direct {
                project_binding: None,
            },
            CallableInstantiation::None,
            Vec::new(),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .unwrap();
        let project = world.project.analysis_view().unwrap();
        let (owner, module, call) = analysis
            .hir_topology()
            .expression_owners()
            .find_map(|owner| {
                let (_, module) = project
                    .modules()
                    .find(|(_, module)| module.module_id() == owner.module())?;
                match module.resolve_expr(owner).ok()?.kind() {
                    HirExprKind::Call(call) => Some((owner, module, call)),
                    _ => None,
                }
            })
            .unwrap();
        let schema = candidate.schema();
        let mapping = map_call_arguments(
            module,
            schema,
            candidate.id(),
            candidate.position(),
            call.arguments(),
            None,
        )
        .unwrap();
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut resolver = ResolverWork::new(1);
        let mut control = resolver
            .begin_candidate_constraint_session(PRODUCTION_CALLABLE_LIMITS, &cancellation)
            .unwrap();
        let declarations =
            PreparedCallableEffectDeclarations::declare([(id, schema)], &mut control).unwrap();
        let mut graph = PreparedCallGraph::<(), ()>::default();
        graph.initialize_effect_declarations(declarations).unwrap();

        let pending = graph
            .prepare_application_result(&builder, &candidate, &mapping, &mut control)
            .unwrap();
        let exposed = analysis
            .checked_callables()
            .callable(id)
            .unwrap()
            .exposed_row();
        let checkpoint = graph.begin_delta(&mut control).unwrap();
        let cache = graph
            .request_effect_projection(
                PreparedCallableEffectProjectionSite::CallableValue(owner),
                &candidate,
            )
            .unwrap();
        graph
            .complete_effect_projection(&cache, id, exposed.clone(), None)
            .unwrap();
        graph.commit_delta(checkpoint).unwrap();
        let cached = graph
            .prepare_application_result(&builder, &candidate, &mapping, &mut control)
            .unwrap();

        let (effects_graph, headers) = graph.effect_declarations_mut().unwrap().relation();
        let header = headers.get(id).unwrap().validate_schema(schema).unwrap();
        effects_graph
            .prepare_body(
                header.scheme(),
                PreparedEffectRow::Canonical(exposed.clone()),
                Vec::new(),
                &mut control,
            )
            .unwrap();
        let closed = effects_graph.solve_principal(&mut control).unwrap();
        for result in [pending, cached] {
            let PreparedApplicationResult::Value(TypeKind::Function {
                params,
                return_type,
                effects,
                ..
            }) = result
            else {
                panic!("remaining invocation arrow");
            };
            assert_eq!(params, [TypeKind::I64]);
            assert_eq!(*return_type, TypeKind::I64);
            assert!(matches!(effects, PreparedEffectRow::Term(_)));
            assert_eq!(
                &closed.resolve_row(&effects, &mut control).unwrap(),
                exposed
            );
        }
    }

    #[test]
    fn source_type_retention_keeps_caller_formals_nested_rows_and_original_aborts() {
        use crate::effect_row::prepared::PreparedEffectRow;
        use crate::types::constraints::{TypeConstraintAbort, TypeConstraintError};
        use crate::types::{GenericBinder, GenericScope, TypeProjectionError};

        let world = fixture("fn caller<T>(value: T) -> T { value }", None);
        let analysis = analyze(&world).unwrap();
        let builder = builder(&analysis);
        let source = builder
            .constraint_source_owner(analysis.hir_topology(), expression(&analysis, "caller"))
            .unwrap();
        let HirSemanticPathRoot::Declaration(declaration) = &source.root else {
            panic!("caller declaration root")
        };
        let module = world
            .project
            .analysis_view()
            .unwrap()
            .modules()
            .find(|(_, module)| module.module_id() == expression(&analysis, "caller").module())
            .unwrap()
            .1;
        let paths = analysis
            .hir_topology()
            .declaration_semantic_paths(declaration)
            .unwrap();
        let (pattern, local) = paths
            .pattern_owners()
            .find_map(|pattern| {
                use arcweft_lang_hir::pattern::{HirPatternBinding, HirPatternKind};
                match module.resolve_pattern(pattern).unwrap().kind() {
                    HirPatternKind::Binding(HirPatternBinding::Bound { local, .. }) => {
                        Some((pattern, *local))
                    }
                    _ => None,
                }
            })
            .expect("actual caller parameter binding");
        for owner in [
            HirSemanticPathOwnerId::Pattern(pattern),
            HirSemanticPathOwnerId::Local(local),
        ] {
            assert_eq!(
                source,
                builder
                    .constraint_source_owner(analysis.hir_topology(), owner)
                    .unwrap()
            );
        }
        let parameter = TypeKind::GenericParam(
            source.signature().unwrap().generic_inventory().types()[0]
                .parameter()
                .clone()
                .into(),
        );
        let binder = GenericBinder::new(0, 0, 1);
        let local = GenericScope::default().with_binder(binder);
        let row = EffectRow::open(EffectSet::new(), local.bound_effect(0, 0).unwrap());
        let nested = TypeKind::function_with_binder(
            binder,
            [parameter.clone()],
            parameter.clone(),
            row.clone(),
        );
        let original = TypeKind::function([nested], parameter);
        let cancellation = std::sync::atomic::AtomicBool::new(false);
        let mut work = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(10_000, 10_000, 100, 100),
                &cancellation,
            ),
        );
        let retained = source.retain_value_type(&original, &mut work).unwrap();
        let TypeKind::Function {
            params,
            return_type,
            effects,
            ..
        } = retained
        else {
            panic!("retained outer function")
        };
        assert_eq!(effects, PreparedEffectRow::Canonical(EffectRow::unknown()));
        let TypeKind::GenericParam(actual) = return_type.as_ref() else {
            panic!("caller formal remains free")
        };
        assert_eq!(
            actual,
            &source.signature().unwrap().generic_inventory().types()[0]
                .parameter()
                .clone()
                .into()
        );
        let TypeKind::Function {
            binder: actual_binder,
            params,
            return_type,
            effects,
            ..
        } = &params[0]
        else {
            panic!("retained nested function")
        };
        assert_eq!(*actual_binder, binder);
        assert_eq!(effects, &PreparedEffectRow::Canonical(row));
        assert_eq!(params[0], **return_type);

        let mut exhausted = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(0, 10_000, 100, 100),
                &cancellation,
            ),
        );
        assert!(matches!(
            source.retain_value_type(&original, &mut exhausted),
            Err(TypeProjectionError::Control(TypeConstraintError::Abort(
                TypeConstraintAbort::WorkLimit {
                    consumed: 0,
                    limit: 0,
                    ..
                }
            )))
        ));
        cancellation.store(true, std::sync::atomic::Ordering::Release);
        assert_eq!(
            source.retain_value_type(&original, &mut work),
            Err(TypeProjectionError::Control(TypeConstraintError::Abort(
                TypeConstraintAbort::Cancelled
            )))
        );
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
        use crate::types::constraints::{
            NoConstraintClient,
            context::{LocalConstraintAccounting, TypeConstraintContext, TypeConstraintLimits},
        };
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        let mut context = TypeConstraintContext::<_, NoConstraintClient>::with_accounting(
            LocalConstraintAccounting::new(
                TypeConstraintLimits::new(4096, 2048, 32, 32),
                &cancelled,
            ),
        );
        let function = function
            .prepare_effects(&crate::types::GenericScope::default(), &mut context)
            .unwrap();
        let mut graph = super::super::super::PreparedCallGraph::<()>::new();
        graph
            .initialize_effect_declarations(
                crate::callable::PreparedCallableEffectDeclarations::declare([], &mut context)
                    .unwrap(),
            )
            .unwrap();
        assert!(
            graph
                .prepare_root_function_specialization(
                    left,
                    &function,
                    &scope,
                    &super::super::super::PRODUCTION_CALLABLE_LIMITS,
                    &mut context,
                )
                .is_ok()
        );
        assert!(matches!(
            graph.prepare_root_function_specialization(
                right,
                &function,
                &scope,
                &super::super::super::PRODUCTION_CALLABLE_LIMITS,
                &mut context,
            ),
            Err(crate::callable::FunctionSpecializationFailure::Prepared(
                super::super::super::CallConstraintInvariant::PreparedCallSiteMismatch,
            ))
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
            Err(CheckedConstraintSourceOwnerError::GenerationMismatch)
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
