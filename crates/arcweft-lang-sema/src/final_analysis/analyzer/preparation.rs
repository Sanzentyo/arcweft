//! Type resolution and local-binding preparation.

use arcweft_lang_hir::item::{HirCapabilityFunction, HirCapabilityMember};

use crate::{
    callable::{CallableName, CallablePath, associated_scope_for},
    nominal::AssociatedTypeScope,
};

use super::super::report::merge_type_resolution_fact;
use super::{
    Analyzer, BTreeMap, BTreeSet, ExprId, FinalSemanticAnalysisError, GenericTypeScope,
    HirCallArgument, HirExprKind, HirFunctionBody, HirImplMember, HirItemKind, HirModule,
    HirPatternKind, HirPredicateBody, HirProofBody, HirStmtKind, NominalResolutionLimits,
    PatternId, ProjectNominalType, ResolvedTypeRefOutcome, ScopeId, SelfTypeScope, StmtId, TypeId,
    TypeKind, TypeResolutionInput,
    expression_types::literal_type,
    items::function_body_roles,
    patterns::{PatternSeedContext, seed_item_parameter_types, seed_pattern_locals},
    resolve_type_ref,
    statements::{enclosing_item, generic_scope},
};

fn simple_binding_source(statement: &HirStmtKind) -> Option<(PatternId, ExprId, Option<TypeId>)> {
    match statement {
        HirStmtKind::Let {
            pattern,
            annotation,
            initializer,
            ..
        }
        | HirStmtKind::LetElse {
            pattern,
            annotation,
            initializer,
            ..
        } => Some((*pattern, *initializer, *annotation)),
        HirStmtKind::LetChoice {
            pattern, choice, ..
        } => Some((*pattern, *choice, None)),
        HirStmtKind::LetScope {
            pattern,
            scope_expr,
            ..
        } => Some((*pattern, *scope_expr, None)),
        HirStmtKind::LetActionReceive {
            pattern, action, ..
        } => Some((*pattern, *action, None)),
        _ => None,
    }
}

fn scope_descends_from(module: &HirModule, mut scope: ScopeId, ancestor: ScopeId) -> bool {
    loop {
        if scope == ancestor {
            return true;
        }
        let Ok(current) = module.resolve_scope(scope) else {
            return false;
        };
        let Some(parent) = current.parent() else {
            return false;
        };
        scope = parent;
    }
}

fn capability_function_owns_type(
    module: &HirModule,
    function: &HirCapabilityFunction,
    target: TypeId,
) -> bool {
    function
        .parameter_groups()
        .iter()
        .flat_map(|group| group.parameters())
        .map(arcweft_lang_hir::item::HirParameter::ty)
        .chain(function.return_type())
        .any(|root| type_tree_contains(module, root, target))
}

fn type_tree_contains(module: &HirModule, root: TypeId, target: TypeId) -> bool {
    root == target
        || module.resolve_type(root).is_ok_and(|node| {
            node.kind()
                .direct_type_children()
                .into_iter()
                .any(|child| type_tree_contains(module, child, target))
        })
}

/// Result of resolving a nominal type used as an associated-call receiver.
///
/// Wrong generic arity is the one semantic receiver failure whose Call owner
/// retains candidate-neutral argument facts without entering the shared
/// resolver. All other poisoned or detached nominal outcomes remain terminal.
pub(super) enum AssociatedReceiverTypeResolution {
    Complete(TypeKind),
    WrongArity(TypeKind),
}

impl Analyzer<'_, '_, '_> {
    pub(super) fn resolve_all_types(&mut self) -> Result<(), FinalSemanticAnalysisError> {
        let trait_reference_roots =
            super::super::validation::implementation_trait_reference_roots(&self.modules);
        let deferred_call_receivers = self
            .modules
            .values()
            .flat_map(|module| module.expressions())
            .filter_map(|(_, expression)| {
                let HirExprKind::Call(call) = expression.kind() else {
                    return None;
                };
                match call.callee() {
                    arcweft_lang_hir::expr::HirCallCallee::UnresolvedDot {
                        nominal_receiver,
                        ..
                    } => nominal_receiver.type_id(),
                    arcweft_lang_hir::expr::HirCallCallee::Associated { receiver, .. } => {
                        receiver.type_id()
                    }
                    arcweft_lang_hir::expr::HirCallCallee::Value { .. } => None,
                }
            })
            .collect::<BTreeSet<_>>();
        let children = self
            .modules
            .values()
            .flat_map(|module| {
                module
                    .types()
                    .flat_map(|(_, ty)| ty.kind().direct_type_children())
            })
            .collect::<BTreeSet<_>>();
        let owners = self
            .modules
            .values()
            .flat_map(|module| module.types().map(|(owner, _)| owner))
            .filter(|owner| {
                !children.contains(owner)
                    && !deferred_call_receivers.contains(owner)
                    && !trait_reference_roots.contains(owner)
            })
            .collect::<Vec<_>>();
        for owner in owners {
            self.resolve_type(owner, false)?;
        }
        Ok(())
    }

    pub(super) fn resolve_type(
        &mut self,
        owner: TypeId,
        resolving_impl_self: bool,
    ) -> Result<TypeKind, FinalSemanticAnalysisError> {
        self.resolve_type_with_associated_recovery(owner, resolving_impl_self, false)
            .and_then(|resolved| match resolved {
                AssociatedReceiverTypeResolution::Complete(ty) => Ok(ty),
                AssociatedReceiverTypeResolution::WrongArity(_) => {
                    Err(FinalSemanticAnalysisError::TypeResolutionFailed { owner })
                }
            })
    }

    pub(super) fn resolve_associated_receiver_type(
        &mut self,
        owner: TypeId,
    ) -> Result<AssociatedReceiverTypeResolution, FinalSemanticAnalysisError> {
        self.resolve_type_with_associated_recovery(owner, false, true)
    }

    fn resolve_type_with_associated_recovery(
        &mut self,
        owner: TypeId,
        resolving_impl_self: bool,
        accept_wrong_arity: bool,
    ) -> Result<AssociatedReceiverTypeResolution, FinalSemanticAnalysisError> {
        if let Some(ty) = self.types.get(&owner) {
            let wrong_arity = self
                .type_reports
                .get(&owner)
                .is_some_and(|report| type_report_root_has_wrong_arity(report, owner));
            if wrong_arity && !accept_wrong_arity {
                return Err(FinalSemanticAnalysisError::TypeResolutionFailed { owner });
            }
            if ty.contains_nominal_poison() && !wrong_arity {
                return Err(FinalSemanticAnalysisError::TypeResolutionFailed { owner });
            }
            return Ok(if wrong_arity {
                AssociatedReceiverTypeResolution::WrongArity(ty.clone())
            } else {
                AssociatedReceiverTypeResolution::Complete(ty.clone())
            });
        }
        self.control.check()?;
        let module = self.module(owner.module())?;
        let node = module
            .resolve_type(owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let generic_scope = generic_scope(module, node.scope(), self.symbols, owner)?;
        let self_scope = if resolving_impl_self {
            SelfTypeScope::Absent
        } else {
            self.self_type_scope(module, node.scope(), owner, &generic_scope)?
        };
        let associated = self.associated_type_scope(module, node.scope(), owner)?;
        let input = if let Some(associated) = associated.as_ref() {
            TypeResolutionInput::accepted_with_associated(
                owner,
                module,
                self.project,
                self.symbols,
                self.catalogs.world.environment().nominal_world(),
                &generic_scope,
                self_scope,
                associated,
                NominalResolutionLimits::PRODUCTION,
            )
        } else {
            TypeResolutionInput::accepted(
                owner,
                module,
                self.project,
                self.symbols,
                self.catalogs.world.environment().nominal_world(),
                &generic_scope,
                self_scope,
                NominalResolutionLimits::PRODUCTION,
            )
        }
        .map_err(|_| FinalSemanticAnalysisError::TypeResolutionInput { owner })?;
        let report = resolve_type_ref(&input)
            .map_err(|_| FinalSemanticAnalysisError::TypeResolutionFailed { owner })?;
        let wrong_arity = accept_wrong_arity && type_report_root_has_wrong_arity(&report, owner);
        let ty = match report.outcome() {
            ResolvedTypeRefOutcome::Complete(product) => product.recovered().clone(),
            ResolvedTypeRefOutcome::Poisoned(poisoned) if wrong_arity => {
                poisoned.product().recovered().clone()
            }
            ResolvedTypeRefOutcome::Poisoned(_) | ResolvedTypeRefOutcome::Detached(_) => {
                return Err(FinalSemanticAnalysisError::TypeResolutionFailed { owner });
            }
        };
        for node in report.outcome().product().nodes() {
            if node.is_contextual_alias_target() {
                continue;
            }
            let Some(recovered) = node.recovered() else {
                continue;
            };
            merge_type_resolution_fact(&mut self.types, node.node(), recovered)?;
        }
        self.type_reports.insert(owner, report);
        Ok(if wrong_arity {
            AssociatedReceiverTypeResolution::WrongArity(ty)
        } else {
            AssociatedReceiverTypeResolution::Complete(ty)
        })
    }

    fn associated_type_scope(
        &self,
        module: &HirModule,
        scope: ScopeId,
        owner: TypeId,
    ) -> Result<Option<AssociatedTypeScope>, FinalSemanticAnalysisError> {
        let Some(item_id) = enclosing_item(module, scope)? else {
            return Ok(None);
        };
        let item = module
            .resolve_item(item_id)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let HirItemKind::ExternCapability(capability) = item.kind() else {
            return Ok(None);
        };
        let function = capability
            .members()
            .iter()
            .filter_map(|member| match member {
                HirCapabilityMember::Function(function) => Some(function),
                HirCapabilityMember::AssociatedType(_) | HirCapabilityMember::Error => None,
            })
            .find(|function| {
                scope_descends_from(module, scope, function.callable_scope())
                    || capability_function_owns_type(module, function, owner)
            })
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let Some(capability_name) = capability.name().resolved() else {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        };
        let Some(function_name) = function.name().resolved() else {
            return Err(FinalSemanticAnalysisError::RecoveredOwner);
        };
        let path = CallablePath::try_new([
            CallableName::try_new(capability_name.as_str())
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?,
            CallableName::try_new(function_name.as_str())
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?,
        ])
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        let world = self.catalogs.world.environment().nominal_world();
        let Some(contract) = world.host_call_contract(&path) else {
            return Ok(None);
        };
        let projected = world
            .try_project_host_call_contract(contract, NominalResolutionLimits::PRODUCTION)
            .map_err(|_| FinalSemanticAnalysisError::TypeResolutionFailed { owner })?;
        Ok(Some(associated_scope_for(
            capability,
            projected.domain_error(),
        )))
    }

    fn self_type_scope(
        &mut self,
        module: &HirModule,
        scope: ScopeId,
        owner: TypeId,
        generics: &GenericTypeScope,
    ) -> Result<SelfTypeScope, FinalSemanticAnalysisError> {
        let Some(item_id) = enclosing_item(module, scope)? else {
            return Ok(SelfTypeScope::Absent);
        };
        let item = module
            .resolve_item(item_id)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if let HirItemKind::Impl(implementation) = item.kind() {
            if implementation.target() == owner {
                return Ok(SelfTypeScope::Absent);
            }
            return self
                .resolve_type(implementation.target(), true)
                .map(SelfTypeScope::Known);
        }
        let Some(nominal) = self
            .symbols
            .nominal_symbols()
            .find(|declaration| declaration.owner() == item_id)
        else {
            return Ok(SelfTypeScope::Absent);
        };
        let arguments = generics
            .bindings()
            .map(|binding| TypeKind::GenericParam(binding.id().clone()))
            .collect::<Vec<_>>();
        Ok(SelfTypeScope::Known(TypeKind::ProjectNominal(
            ProjectNominalType::new(nominal.id().clone(), arguments),
        )))
    }

    pub(super) fn seed_local_types(&mut self) -> Result<(), FinalSemanticAnalysisError> {
        for module in self.modules.values() {
            for (owner, local) in module.locals() {
                if let Some(annotation) = local.annotation() {
                    let ty = self.types.get(&annotation).cloned().ok_or(
                        FinalSemanticAnalysisError::TypeResolutionFailed { owner: annotation },
                    )?;
                    self.facts.set_local_type(owner, ty);
                }
            }
            for (_, item) in module.items() {
                let mut locals = BTreeMap::new();
                seed_item_parameter_types(item, &self.types, &mut locals)?;
                for (owner, ty) in locals {
                    self.facts.set_local_type(owner, ty);
                }
            }
            for (_, expression) in module.expressions() {
                if let HirExprKind::Closure(closure) = expression.kind() {
                    for parameter in closure.parameters() {
                        let Some(ty) = parameter.ty() else { continue };
                        let semantic = self.types.get(&ty).cloned().ok_or(
                            FinalSemanticAnalysisError::TypeResolutionFailed { owner: ty },
                        )?;
                        let mut locals = BTreeMap::new();
                        let mut patterns = BTreeMap::new();
                        seed_pattern_locals(
                            PatternSeedContext {
                                module,
                                types: &self.types,
                                symbols: self.symbols,
                                environment: self.catalogs.world.environment().typecheck_env(),
                            },
                            parameter.pattern(),
                            &semantic,
                            &mut locals,
                            &mut patterns,
                        )?;
                        for (owner, ty) in locals {
                            self.facts.set_local_type(owner, ty);
                        }
                        for (owner, ty) in patterns {
                            self.facts.set_pattern_type(owner, ty);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn infer_statement_bindings(&mut self) -> Result<(), FinalSemanticAnalysisError> {
        let statements = self
            .modules
            .values()
            .flat_map(|module| {
                module
                    .statements()
                    .map(|(owner, statement)| (owner, statement.kind().clone()))
            })
            .collect::<Vec<_>>();
        for (owner, statement) in statements {
            self.control.check()?;
            if let Some((pattern, initializer, annotation)) = simple_binding_source(&statement) {
                self.infer_simple_statement_binding(owner, pattern, initializer, annotation)?;
            } else {
                self.infer_control_statement_bindings(owner, statement)?;
            }
        }
        Ok(())
    }

    fn infer_simple_statement_binding(
        &mut self,
        owner: StmtId,
        pattern: PatternId,
        initializer: ExprId,
        annotation: Option<TypeId>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        self.infer_nested_expression_bindings(initializer)?;
        let authored_type = match annotation {
            Some(annotation) => Some(annotation),
            None => self
                .module(owner.module())?
                .resolve_pattern(pattern)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                .kind()
                .authored_type(),
        };
        let expected = authored_type
            .map(|annotation| {
                self.types
                    .get(&annotation)
                    .cloned()
                    .ok_or(FinalSemanticAnalysisError::TypeResolutionFailed { owner: annotation })
            })
            .transpose()?;
        let actual = self.check_expression(initializer, expected.as_ref())?;
        let binding = match expected {
            Some(expected) if expected.accepts(actual.ty()) => expected,
            Some(_) => {
                return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                    owner: initializer,
                });
            }
            None => actual.ty().clone(),
        };
        let module = self.module(owner.module())?;
        self.seed_contextual_pattern_locals(module, pattern, &binding)
    }

    pub(super) fn infer_nested_expression_bindings(
        &mut self,
        owner: ExprId,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let statements = {
            let module = self.module(owner.module())?;
            let expression = module
                .resolve_expr(owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            match expression.kind() {
                HirExprKind::Block(block) => block.statements().to_vec(),
                HirExprKind::ComputationBlock(block) => block.statements().to_vec(),
                HirExprKind::NamedBlock(block) => block.statements().to_vec(),
                HirExprKind::Loop(loop_expression) => loop_expression.statements().to_vec(),
                _ => return Ok(()),
            }
        };
        for statement in statements {
            let kind = self
                .module(statement.module())?
                .resolve_stmt(statement)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?
                .kind()
                .clone();
            if let Some((pattern, initializer, annotation)) = simple_binding_source(&kind) {
                self.infer_simple_statement_binding(statement, pattern, initializer, annotation)?;
            } else {
                self.infer_control_statement_bindings(statement, kind)?;
            }
        }
        Ok(())
    }

    fn infer_control_statement_bindings(
        &mut self,
        owner: StmtId,
        statement: HirStmtKind,
    ) -> Result<(), FinalSemanticAnalysisError> {
        match statement {
            HirStmtKind::IfLet(statement) => {
                let scrutinee = self.check_expression(statement.scrutinee(), None)?;
                let module = self.module(owner.module())?;
                self.seed_contextual_pattern_locals(module, statement.pattern(), scrutinee.ty())?;
            }
            HirStmtKind::WhileLet(statement) => {
                let scrutinee = self.check_expression(statement.scrutinee(), None)?;
                let module = self.module(owner.module())?;
                self.seed_contextual_pattern_locals(module, statement.pattern(), scrutinee.ty())?;
            }
            HirStmtKind::Match(statement) => {
                let scrutinee = self.check_expression(statement.scrutinee(), None)?;
                let module = self.module(owner.module())?;
                for arm in statement.arms() {
                    self.seed_contextual_pattern_locals(module, arm.pattern(), scrutinee.ty())?;
                }
            }
            HirStmtKind::For(statement) => {
                self.check_expression(statement.source(), None)?;
                self.check_expression(statement.iterator(), None)?;
                let iteration = self.iteration_facts.get(&statement.iterator()).ok_or(
                    FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: statement.iterator(),
                    },
                )?;
                let item = super::statements::iteration_item(iteration).clone();
                let module = self.module(owner.module())?;
                self.seed_contextual_pattern_locals(module, statement.pattern(), &item)?;
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn analyze_all_expressions(&mut self) -> Result<(), FinalSemanticAnalysisError> {
        // Call arguments are context-owned by their enclosing Call.  Walking
        // them again as context-free roots would perform one extra physical
        // evaluation outside the candidate transaction and could publish a
        // fact selected under the wrong expected type.
        let contextual_call_arguments = self
            .modules
            .values()
            .flat_map(|module| module.expressions().map(|(_, expression)| expression))
            .filter_map(|expression| match expression.kind() {
                HirExprKind::Call(call) => Some(call.arguments()),
                _ => None,
            })
            .flatten()
            .map(HirCallArgument::value)
            .collect::<BTreeSet<_>>();
        let expression_children = self
            .modules
            .values()
            .flat_map(|module| module.expressions().map(|(_, expression)| expression))
            .flat_map(|expression| expression.kind().direct_expression_children())
            .collect::<BTreeSet<_>>();
        let owners = self
            .modules
            .values()
            .flat_map(|module| module.expressions().map(|(owner, _)| owner))
            .filter(|owner| !expression_children.contains(owner))
            .collect::<BTreeSet<_>>();
        for owner in owners {
            self.check_expression(owner, None)?;
        }
        // A fixed compact spread is physically checked through typed element
        // coordinates, so no candidate pass evaluates the sequence container
        // itself.  Materialize only such still-missing argument owners after
        // the owning Call has committed; this publishes the required
        // expression fact without inventing another candidate-slot visit.
        for owner in contextual_call_arguments {
            if !self.facts.expressions().contains_key(&owner) {
                self.check_expression(owner, None)?;
            }
        }
        Ok(())
    }

    pub(super) fn validate_callable_body_results(
        &mut self,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let mut expectations = Vec::new();
        for module in self.modules.values().copied() {
            for (_, item) in module.items() {
                match item.kind() {
                    HirItemKind::Function(function) => {
                        let (yield_count, _) = function_body_roles(module, function.body())?;
                        let expected = if yield_count == 0 {
                            resolved_callable_result(function.return_type(), &self.types)?
                        } else {
                            TypeKind::Unit
                        };
                        append_function_body_result_expectations(
                            module,
                            function.body(),
                            expected,
                            &mut expectations,
                        )?;
                    }
                    HirItemKind::Predicate(predicate) => expectations.push((
                        predicate_body_tail(predicate.body())?,
                        resolved_required_result(predicate.return_type(), &self.types)?,
                    )),
                    HirItemKind::Proof(proof) => expectations.push((
                        proof_body_tail(proof.body())?,
                        resolved_required_result(proof.return_type(), &self.types)?,
                    )),
                    HirItemKind::Impl(implementation) => {
                        for member in implementation.members() {
                            let HirImplMember::Function(function) = member else {
                                continue;
                            };
                            let Some(body) = function.body() else {
                                continue;
                            };
                            let (yield_count, _) = function_body_roles(module, body)?;
                            let expected = if yield_count == 0 {
                                resolved_callable_result(function.return_type(), &self.types)?
                            } else {
                                TypeKind::Unit
                            };
                            append_function_body_result_expectations(
                                module,
                                body,
                                expected,
                                &mut expectations,
                            )?;
                        }
                    }
                    _ => {}
                }
            }
        }
        for (owner, expected) in expectations {
            let checked = self.check_expression(owner, Some(&expected))?;
            if !expected.accepts(checked.ty()) {
                return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner });
            }
        }
        Ok(())
    }

    pub(super) fn finalize_unannotated_locals(&mut self) -> Result<(), FinalSemanticAnalysisError> {
        for module in self.modules.values().copied() {
            for (owner, local) in module.locals() {
                if local.is_poisoned() {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                }
                if self.facts.locals().contains_key(&owner) {
                    continue;
                }
                let inferred = local
                    .pattern()
                    .and_then(|pattern| self.pattern_type_hint(module, pattern))
                    .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner })?;
                self.facts.set_local_type(owner, inferred);
            }
        }
        Ok(())
    }

    pub(super) fn pattern_type_hint(
        &self,
        module: &HirModule,
        owner: PatternId,
    ) -> Option<TypeKind> {
        if let Some(ty) = self.facts.patterns().get(&owner) {
            return Some(ty.clone());
        }
        let pattern = module.resolve_pattern(owner).ok()?;
        match pattern.kind() {
            HirPatternKind::TypedBinding { ty, .. } => self.types.get(ty).cloned(),
            HirPatternKind::Literal(literal) => literal_type(literal, None).map(|(ty, _)| ty),
            _ => None,
        }
    }
}

fn type_report_root_has_wrong_arity(report: &super::TypeResolutionReport, owner: TypeId) -> bool {
    matches!(report.outcome(), ResolvedTypeRefOutcome::Poisoned(_))
        && report.outcome().product().nodes().iter().any(|node| {
            node.node() == owner
                && matches!(
                    node.outcome(),
                    super::TypeNameResolution::Failed(
                        super::TypeResolutionFailure::WrongArity { .. }
                    )
                )
        })
}

fn resolved_callable_result(
    owner: Option<TypeId>,
    types: &std::collections::BTreeMap<TypeId, TypeKind>,
) -> Result<TypeKind, FinalSemanticAnalysisError> {
    owner.map_or(Ok(TypeKind::Unit), |owner| {
        resolved_required_result(owner, types)
    })
}

fn resolved_required_result(
    owner: TypeId,
    types: &std::collections::BTreeMap<TypeId, TypeKind>,
) -> Result<TypeKind, FinalSemanticAnalysisError> {
    types
        .get(&owner)
        .cloned()
        .ok_or(FinalSemanticAnalysisError::TypeResolutionFailed { owner })
}

fn append_function_body_result_expectations(
    module: &HirModule,
    body: &HirFunctionBody,
    expected: TypeKind,
    expectations: &mut Vec<(ExprId, TypeKind)>,
) -> Result<(), FinalSemanticAnalysisError> {
    let HirFunctionBody::Block {
        statements, tail, ..
    } = body
    else {
        return Err(FinalSemanticAnalysisError::RecoveredOwner);
    };
    for statement in statements {
        let statement = module
            .resolve_stmt(*statement)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        if let HirStmtKind::Return { value } = statement.kind() {
            expectations.push((*value, expected.clone()));
        }
    }
    let tail_expected = match statements.last() {
        Some(statement)
            if module
                .resolve_stmt(*statement)
                .is_ok_and(|statement| matches!(statement.kind(), HirStmtKind::Return { .. })) =>
        {
            TypeKind::Unit
        }
        _ => expected,
    };
    expectations.push((*tail, tail_expected));
    Ok(())
}

fn predicate_body_tail(body: &HirPredicateBody) -> Result<ExprId, FinalSemanticAnalysisError> {
    match body {
        HirPredicateBody::Expression { expression, .. } => Ok(*expression),
        HirPredicateBody::Block { tail, .. } => Ok(*tail),
        HirPredicateBody::Error { .. } => Err(FinalSemanticAnalysisError::RecoveredOwner),
    }
}

fn proof_body_tail(body: &HirProofBody) -> Result<ExprId, FinalSemanticAnalysisError> {
    match body {
        HirProofBody::Expression { expression, .. } => Ok(*expression),
        HirProofBody::Block { tail, .. } => Ok(*tail),
        HirProofBody::Error { .. } => Err(FinalSemanticAnalysisError::RecoveredOwner),
    }
}
