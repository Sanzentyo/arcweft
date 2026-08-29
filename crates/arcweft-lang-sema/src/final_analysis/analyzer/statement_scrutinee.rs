//! Short-lived contextual statement scrutinee selection.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_lang_hir::{
    identity::{ExprId, HirModuleId, PatternId, StmtId, TypeId},
    module::HirModule,
    project::{HirExecutableProjectView, HirProjectEvaluationTopology},
    stmt::{HirSelectBranchHead, HirSelectStmt, HirStmtKind, HirTrigger},
    symbol::CallableDeclarationKey,
};

use crate::{
    env::TypeCheckEnv,
    final_analysis::{
        PreparedSelectBranchHeadProof, PreparedSelectScrutineeProof,
        PreparedStatementScrutineeProof, PreparedTriggerScrutineeProof,
    },
    registration::RegisteredStatementIngressTypes,
    types::{EntityKind, TypeKind},
};

use super::{
    FinalSemanticAnalysisError, SemanticFactState,
    executable_ingress::{PreparedExecutableDeclarationInventory, PreparedExecutableIngressFacts},
    patterns::{PatternSeedContext, seed_pattern_locals},
};

/// Closed contextual pattern source vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StatementScrutineeRole {
    TriggerInput,
    TriggerEvent,
    TriggerSignal,
    TriggerSelect,
    TriggerTask,
    TriggerScope,
    SelectFrame,
    SelectEvent,
}

/// Borrowed selector over the sole registered and Entry-rooted authorities.
///
/// This type deliberately owns no `TypeKind`, implements no `Clone`, and is
/// dropped before either semantic facts or ingress facts are mutated.
pub(super) struct StatementScrutineeTypeAuthority<'a> {
    standard: &'a RegisteredStatementIngressTypes,
    project: HirExecutableProjectView<'a>,
    topology: &'a HirProjectEvaluationTopology,
    types: &'a BTreeMap<TypeId, TypeKind>,
    ingress: &'a PreparedExecutableIngressFacts,
}

impl<'a> StatementScrutineeTypeAuthority<'a> {
    const fn new(
        standard: &'a RegisteredStatementIngressTypes,
        project: HirExecutableProjectView<'a>,
        topology: &'a HirProjectEvaluationTopology,
        types: &'a BTreeMap<TypeId, TypeKind>,
        ingress: &'a PreparedExecutableIngressFacts,
    ) -> Self {
        Self {
            standard,
            project,
            topology,
            types,
            ingress,
        }
    }

    fn registered(
        &self,
        role: StatementScrutineeRole,
    ) -> Result<&'a TypeKind, FinalSemanticAnalysisError> {
        match role {
            StatementScrutineeRole::TriggerInput => Ok(self.standard.input()),
            StatementScrutineeRole::TriggerTask => Ok(self.standard.task()),
            StatementScrutineeRole::TriggerScope => Ok(self.standard.scope()),
            StatementScrutineeRole::SelectFrame => Ok(self.standard.frame()),
            StatementScrutineeRole::TriggerEvent
            | StatementScrutineeRole::TriggerSignal
            | StatementScrutineeRole::TriggerSelect
            | StatementScrutineeRole::SelectEvent => {
                Err(FinalSemanticAnalysisError::WrongPayloadFamily)
            }
        }
    }

    fn event(
        &self,
        role: StatementScrutineeRole,
        declaration: &CallableDeclarationKey,
    ) -> Result<&'a TypeKind, FinalSemanticAnalysisError> {
        if !matches!(
            role,
            StatementScrutineeRole::TriggerEvent | StatementScrutineeRole::SelectEvent
        ) {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let owner = self
            .ingress
            .event_type(declaration)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let ty = self
            .types
            .get(&owner)
            .ok_or(FinalSemanticAnalysisError::TypeResolutionFailed { owner })?;
        if ty.semantic_identity_digest()
            != self
                .ingress
                .event_digest(declaration)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        Ok(ty)
    }

    fn signal_payload<'b>(
        &self,
        role: StatementScrutineeRole,
        target: &'b TypeKind,
    ) -> Result<&'b TypeKind, FinalSemanticAnalysisError> {
        if role != StatementScrutineeRole::TriggerSignal {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let TypeKind::Ref(signal) = target else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        if signal.kind() != &EntityKind::Signal {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        signal
            .value()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)
    }

    fn require_choice_lifecycle(
        &self,
        role: StatementScrutineeRole,
        statement: StmtId,
    ) -> Result<(), FinalSemanticAnalysisError> {
        if role != StatementScrutineeRole::TriggerSelect {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if !self
            .project
            .modules()
            .any(|(_, module)| module.module_id() == statement.module())
        {
            return Err(FinalSemanticAnalysisError::InvalidOwner);
        }
        self.topology
            .enclosing_choice_lifecycle(statement)
            .map(|_| ())
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct StatementScrutineeExpressionRequest {
    owner: ExprId,
    expected: Option<StatementScrutineeExpressionType>,
}

impl StatementScrutineeExpressionRequest {
    pub(super) const fn owner(self) -> ExprId {
        self.owner
    }

    pub(super) fn expected(self) -> Option<TypeKind> {
        match self.expected {
            Some(StatementScrutineeExpressionType::Duration) => Some(TypeKind::Duration),
            Some(StatementScrutineeExpressionType::Bool) => Some(TypeKind::Bool),
            None => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum StatementScrutineeExpressionType {
    Duration,
    Bool,
}

pub(super) fn has_event_scrutinee(statement: &HirStmtKind) -> bool {
    match statement {
        HirStmtKind::On {
            trigger: HirTrigger::Event(_),
            ..
        } => true,
        HirStmtKind::Select(HirSelectStmt::Branches { branches, .. }) => branches
            .iter()
            .any(|branch| matches!(branch.head(), HirSelectBranchHead::Event { .. })),
        _ => false,
    }
}

pub(super) fn expression_requests_for_statement(
    module: &HirModule,
    owner: StmtId,
    statement: &HirStmtKind,
) -> Result<Vec<StatementScrutineeExpressionRequest>, FinalSemanticAnalysisError> {
    let mut requests = Vec::new();
    if owner.module() != module.module_id() {
        return Err(FinalSemanticAnalysisError::InvalidOwner);
    }
    match statement {
        HirStmtKind::Wait { target } => {
            requests.push(StatementScrutineeExpressionRequest {
                owner: *target,
                expected: None,
            });
        }
        HirStmtKind::On { trigger, .. } => match trigger {
            HirTrigger::Signal { target, .. } => {
                requests.push(StatementScrutineeExpressionRequest {
                    owner: *target,
                    expected: None,
                })
            }
            HirTrigger::Timeout(expression) => requests.push(StatementScrutineeExpressionRequest {
                owner: *expression,
                expected: Some(StatementScrutineeExpressionType::Duration),
            }),
            HirTrigger::Expression(expression) => {
                requests.push(StatementScrutineeExpressionRequest {
                    owner: *expression,
                    expected: Some(StatementScrutineeExpressionType::Bool),
                })
            }
            HirTrigger::Input(_)
            | HirTrigger::Event(_)
            | HirTrigger::Mark(_)
            | HirTrigger::Select(_)
            | HirTrigger::Task(_)
            | HirTrigger::Scope(_) => {}
            HirTrigger::Recovered(_) => {
                return Err(FinalSemanticAnalysisError::RecoveredOwner);
            }
        },
        HirStmtKind::Select(HirSelectStmt::Branches { branches, .. }) => {
            for branch in branches {
                match branch.head() {
                    HirSelectBranchHead::Bind { source, .. } => {
                        requests.push(StatementScrutineeExpressionRequest {
                            owner: *source,
                            expected: None,
                        })
                    }
                    HirSelectBranchHead::Frame { .. } | HirSelectBranchHead::Event { .. } => {}
                    HirSelectBranchHead::Recovered => {
                        return Err(FinalSemanticAnalysisError::RecoveredOwner);
                    }
                }
            }
        }
        HirStmtKind::Select(HirSelectStmt::Operand(expression)) => {
            requests.push(StatementScrutineeExpressionRequest {
                owner: *expression,
                expected: None,
            })
        }
        _ => {}
    }
    let mut seen = BTreeSet::new();
    requests.retain(|request| seen.insert(request.owner()));
    Ok(requests)
}

#[expect(
    clippy::too_many_arguments,
    reason = "the borrowed selector keeps registered, HIR, type, and mutable fact authorities explicit"
)]
pub(super) fn seed_non_event_scrutinees(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    types: &BTreeMap<TypeId, TypeKind>,
    symbols: &arcweft_lang_hir::symbol::ProjectSymbolTable,
    environment: &TypeCheckEnv,
    standard: &RegisteredStatementIngressTypes,
    project: HirExecutableProjectView<'_>,
    topology: &HirProjectEvaluationTopology,
    ingress: &PreparedExecutableIngressFacts,
    inventory: &PreparedExecutableDeclarationInventory,
    facts: &mut SemanticFactState,
) -> Result<(), FinalSemanticAnalysisError> {
    for declaration in inventory.values() {
        seed_static_scrutinees(
            modules,
            types,
            symbols,
            environment,
            StatementScrutineeTypeAuthority::new(standard, project, topology, types, ingress),
            declaration.declaration(),
            declaration.statements(),
            false,
            facts,
        )?;
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "the borrowed selector keeps registered, HIR, type, and mutable fact authorities explicit"
)]
pub(super) fn seed_declaration_scrutinees(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    types: &BTreeMap<TypeId, TypeKind>,
    symbols: &arcweft_lang_hir::symbol::ProjectSymbolTable,
    environment: &TypeCheckEnv,
    standard: &RegisteredStatementIngressTypes,
    project: HirExecutableProjectView<'_>,
    topology: &HirProjectEvaluationTopology,
    ingress: &PreparedExecutableIngressFacts,
    declaration: &CallableDeclarationKey,
    statements: &[StmtId],
    facts: &mut SemanticFactState,
) -> Result<(), FinalSemanticAnalysisError> {
    seed_static_scrutinees(
        modules,
        types,
        symbols,
        environment,
        StatementScrutineeTypeAuthority::new(standard, project, topology, types, ingress),
        declaration,
        statements,
        true,
        facts,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "the borrowed selector keeps registered, HIR, type, and mutable fact authorities explicit"
)]
fn seed_static_scrutinees(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    types: &BTreeMap<TypeId, TypeKind>,
    symbols: &arcweft_lang_hir::symbol::ProjectSymbolTable,
    environment: &TypeCheckEnv,
    authority: StatementScrutineeTypeAuthority<'_>,
    declaration: &CallableDeclarationKey,
    statements: &[StmtId],
    include_event: bool,
    facts: &mut SemanticFactState,
) -> Result<(), FinalSemanticAnalysisError> {
    for owner in statements {
        let module = modules
            .get(&owner.module())
            .copied()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let statement = module
            .resolve_stmt(*owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        match statement.kind() {
            HirStmtKind::On { trigger, .. } => match trigger {
                HirTrigger::Input(pattern) => seed_registered(
                    module,
                    types,
                    symbols,
                    environment,
                    facts,
                    *pattern,
                    authority.registered(StatementScrutineeRole::TriggerInput)?,
                )?,
                HirTrigger::Event(pattern) if include_event => seed_registered(
                    module,
                    types,
                    symbols,
                    environment,
                    facts,
                    *pattern,
                    authority.event(StatementScrutineeRole::TriggerEvent, declaration)?,
                )?,
                HirTrigger::Select(pattern) => {
                    authority
                        .require_choice_lifecycle(StatementScrutineeRole::TriggerSelect, *owner)?;
                    let ty = TypeKind::entity_ref(EntityKind::ChoiceOption);
                    seed_registered(module, types, symbols, environment, facts, *pattern, &ty)?;
                }
                HirTrigger::Task(pattern) => seed_registered(
                    module,
                    types,
                    symbols,
                    environment,
                    facts,
                    *pattern,
                    authority.registered(StatementScrutineeRole::TriggerTask)?,
                )?,
                HirTrigger::Scope(pattern) => seed_registered(
                    module,
                    types,
                    symbols,
                    environment,
                    facts,
                    *pattern,
                    authority.registered(StatementScrutineeRole::TriggerScope)?,
                )?,
                HirTrigger::Event(_)
                | HirTrigger::Signal { .. }
                | HirTrigger::Timeout(_)
                | HirTrigger::Mark(_)
                | HirTrigger::Expression(_) => {}
                HirTrigger::Recovered(_) => {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                }
            },
            HirStmtKind::Select(HirSelectStmt::Branches { branches, .. }) => {
                for branch in branches {
                    match branch.head() {
                        HirSelectBranchHead::Frame { pattern, .. } => seed_registered(
                            module,
                            types,
                            symbols,
                            environment,
                            facts,
                            *pattern,
                            authority.registered(StatementScrutineeRole::SelectFrame)?,
                        )?,
                        HirSelectBranchHead::Event { pattern, .. } if include_event => {
                            seed_registered(
                                module,
                                types,
                                symbols,
                                environment,
                                facts,
                                *pattern,
                                authority
                                    .event(StatementScrutineeRole::SelectEvent, declaration)?,
                            )?;
                        }
                        HirSelectBranchHead::Bind { .. } | HirSelectBranchHead::Event { .. } => {}
                        HirSelectBranchHead::Recovered => {
                            return Err(FinalSemanticAnalysisError::RecoveredOwner);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn seed_dynamic_scrutinee(
    module: &HirModule,
    types: &BTreeMap<TypeId, TypeKind>,
    symbols: &arcweft_lang_hir::symbol::ProjectSymbolTable,
    environment: &TypeCheckEnv,
    owner: StmtId,
    statement: &HirStmtKind,
    facts: &mut SemanticFactState,
) -> Result<(), FinalSemanticAnalysisError> {
    if owner.module() != module.module_id() {
        return Err(FinalSemanticAnalysisError::InvalidOwner);
    }
    match statement {
        HirStmtKind::On {
            trigger: HirTrigger::Signal { target, value },
            ..
        } => {
            let payload = {
                let target_type = facts
                    .expressions()
                    .get(target)
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: *target,
                    })?
                    .ty();
                let TypeKind::Ref(signal) = target_type else {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                };
                if signal.kind() != &EntityKind::Signal {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                signal
                    .value()
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
                    .clone()
            };
            if let Some(pattern) = value {
                seed_registered(
                    module,
                    types,
                    symbols,
                    environment,
                    facts,
                    *pattern,
                    &payload,
                )?;
            }
        }
        HirStmtKind::Select(HirSelectStmt::Branches { branches, .. }) => {
            for branch in branches {
                let HirSelectBranchHead::Bind { binding, source } = branch.head() else {
                    continue;
                };
                let local = binding
                    .resolved()
                    .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                let ty = facts
                    .expressions()
                    .get(source)
                    .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: *source,
                    })?
                    .ty()
                    .clone();
                set_local(facts, local, ty)?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "post-checks compare the completed facts against the same registered, HIR, and Entry-rooted authorities"
)]
pub(super) fn validate_declaration_scrutinees(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    types: &BTreeMap<TypeId, TypeKind>,
    standard: &RegisteredStatementIngressTypes,
    project: HirExecutableProjectView<'_>,
    topology: &HirProjectEvaluationTopology,
    ingress: &PreparedExecutableIngressFacts,
    declaration: &CallableDeclarationKey,
    statements: &[StmtId],
    facts: &SemanticFactState,
) -> Result<(), FinalSemanticAnalysisError> {
    let authority =
        StatementScrutineeTypeAuthority::new(standard, project, topology, types, ingress);
    for owner in statements {
        let module = modules
            .get(&owner.module())
            .copied()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let statement = module
            .resolve_stmt(*owner)
            .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
        match statement.kind() {
            HirStmtKind::On { trigger, .. } => match trigger {
                HirTrigger::Timeout(expression)
                    if facts.expressions().get(expression).map(|fact| fact.ty())
                        != Some(&TypeKind::Duration) =>
                {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: *expression,
                    });
                }
                HirTrigger::Expression(expression)
                    if facts.expressions().get(expression).map(|fact| fact.ty())
                        != Some(&TypeKind::Bool) =>
                {
                    return Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                        owner: *expression,
                    });
                }
                HirTrigger::Event(pattern) => {
                    let expected =
                        authority.event(StatementScrutineeRole::TriggerEvent, declaration)?;
                    require_pattern(facts, *pattern, expected)?;
                }
                HirTrigger::Signal { target, value } => {
                    let target_type = facts
                        .expressions()
                        .get(target)
                        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                            owner: *target,
                        })?
                        .ty();
                    let payload = authority
                        .signal_payload(StatementScrutineeRole::TriggerSignal, target_type)?;
                    if let Some(value) = value {
                        require_pattern(facts, *value, payload)?;
                    }
                }
                HirTrigger::Input(pattern) => require_pattern(
                    facts,
                    *pattern,
                    authority.registered(StatementScrutineeRole::TriggerInput)?,
                )?,
                HirTrigger::Select(pattern) => {
                    authority
                        .require_choice_lifecycle(StatementScrutineeRole::TriggerSelect, *owner)?;
                    require_pattern(
                        facts,
                        *pattern,
                        &TypeKind::entity_ref(EntityKind::ChoiceOption),
                    )?;
                }
                HirTrigger::Task(pattern) => require_pattern(
                    facts,
                    *pattern,
                    authority.registered(StatementScrutineeRole::TriggerTask)?,
                )?,
                HirTrigger::Scope(pattern) => require_pattern(
                    facts,
                    *pattern,
                    authority.registered(StatementScrutineeRole::TriggerScope)?,
                )?,
                HirTrigger::Mark(_) | HirTrigger::Timeout(_) | HirTrigger::Expression(_) => {}
                HirTrigger::Recovered(_) => {
                    return Err(FinalSemanticAnalysisError::RecoveredOwner);
                }
            },
            HirStmtKind::Select(HirSelectStmt::Branches { branches, .. }) => {
                for branch in branches {
                    match branch.head() {
                        HirSelectBranchHead::Bind { binding, source } => {
                            let local = binding
                                .resolved()
                                .ok_or(FinalSemanticAnalysisError::RecoveredOwner)?;
                            let source_type = facts
                                .expressions()
                                .get(source)
                                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                                    owner: *source,
                                })?
                                .ty();
                            if facts.locals().get(&local) != Some(source_type) {
                                return Err(FinalSemanticAnalysisError::LocalTypeUnavailable {
                                    owner: local,
                                });
                            }
                        }
                        HirSelectBranchHead::Event { pattern, locals } => {
                            let expected = authority
                                .event(StatementScrutineeRole::SelectEvent, declaration)?;
                            require_pattern(facts, *pattern, expected)?;
                            for local in locals {
                                if !facts.locals().contains_key(local) {
                                    return Err(FinalSemanticAnalysisError::LocalTypeUnavailable {
                                        owner: *local,
                                    });
                                }
                            }
                        }
                        HirSelectBranchHead::Frame { pattern, locals } => {
                            let expected =
                                authority.registered(StatementScrutineeRole::SelectFrame)?;
                            require_pattern(facts, *pattern, expected)?;
                            require_locals(facts, locals, expected)?;
                        }
                        HirSelectBranchHead::Recovered => {
                            return Err(FinalSemanticAnalysisError::RecoveredOwner);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn prepare_scrutinee_proofs(
    modules: &BTreeMap<HirModuleId, &HirModule>,
    inventory: &PreparedExecutableDeclarationInventory,
) -> Result<BTreeMap<StmtId, PreparedStatementScrutineeProof>, FinalSemanticAnalysisError> {
    let mut proofs = BTreeMap::new();
    for declaration in inventory.values() {
        for owner in declaration.statements() {
            let module = modules
                .get(&owner.module())
                .copied()
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
            let statement = module
                .resolve_stmt(*owner)
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            let proof = match statement.kind() {
                HirStmtKind::On { trigger, .. } => {
                    Some(PreparedStatementScrutineeProof::Trigger(match trigger {
                        HirTrigger::Input(_) => PreparedTriggerScrutineeProof::Input,
                        HirTrigger::Event(_) => PreparedTriggerScrutineeProof::Event,
                        HirTrigger::Signal { .. } => PreparedTriggerScrutineeProof::Signal,
                        HirTrigger::Timeout(_) => PreparedTriggerScrutineeProof::Timeout,
                        HirTrigger::Mark(mark) => PreparedTriggerScrutineeProof::Mark(*mark),
                        HirTrigger::Select(_) => PreparedTriggerScrutineeProof::Select,
                        HirTrigger::Task(_) => PreparedTriggerScrutineeProof::Task,
                        HirTrigger::Scope(_) => PreparedTriggerScrutineeProof::Scope,
                        HirTrigger::Expression(_) => PreparedTriggerScrutineeProof::Expression,
                        HirTrigger::Recovered(_) => {
                            return Err(FinalSemanticAnalysisError::RecoveredOwner);
                        }
                    }))
                }
                HirStmtKind::Select(HirSelectStmt::Operand(_)) => Some(
                    PreparedStatementScrutineeProof::Select(PreparedSelectScrutineeProof::Operand),
                ),
                HirStmtKind::Select(HirSelectStmt::Branches { branches, .. }) => {
                    let heads = branches
                        .iter()
                        .map(|branch| match branch.head() {
                            HirSelectBranchHead::Bind { .. } => {
                                Ok(PreparedSelectBranchHeadProof::Bind)
                            }
                            HirSelectBranchHead::Frame { .. } => {
                                Ok(PreparedSelectBranchHeadProof::Frame)
                            }
                            HirSelectBranchHead::Event { .. } => {
                                Ok(PreparedSelectBranchHeadProof::Event)
                            }
                            HirSelectBranchHead::Recovered => {
                                Err(FinalSemanticAnalysisError::RecoveredOwner)
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()?
                        .into_boxed_slice();
                    Some(PreparedStatementScrutineeProof::Select(
                        PreparedSelectScrutineeProof::Branches(heads),
                    ))
                }
                _ => None,
            };
            if let Some(proof) = proof
                && proofs.insert(*owner, proof).is_some()
            {
                return Err(FinalSemanticAnalysisError::DuplicateFact {
                    family: super::SemanticFactFamily::Statement,
                });
            }
        }
    }
    Ok(proofs)
}

fn seed_registered(
    module: &HirModule,
    types: &BTreeMap<TypeId, TypeKind>,
    symbols: &arcweft_lang_hir::symbol::ProjectSymbolTable,
    environment: &TypeCheckEnv,
    facts: &mut SemanticFactState,
    pattern: PatternId,
    ty: &TypeKind,
) -> Result<(), FinalSemanticAnalysisError> {
    let mut locals = BTreeMap::new();
    let mut patterns = BTreeMap::new();
    seed_pattern_locals(
        PatternSeedContext {
            module,
            types,
            symbols,
            environment,
        },
        pattern,
        ty,
        &mut locals,
        &mut patterns,
    )?;
    for (owner, ty) in locals {
        set_local(facts, owner, ty)?;
    }
    for (owner, ty) in patterns {
        if facts
            .patterns()
            .get(&owner)
            .is_some_and(|existing| existing != &ty)
        {
            return Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner });
        }
        if !facts.patterns().contains_key(&owner) {
            facts
                .set_pattern_type(owner, ty)
                .map_err(FinalSemanticAnalysisError::from)?;
        }
    }
    Ok(())
}

fn set_local(
    facts: &mut SemanticFactState,
    owner: arcweft_lang_hir::identity::LocalId,
    ty: TypeKind,
) -> Result<(), FinalSemanticAnalysisError> {
    if facts
        .locals()
        .get(&owner)
        .is_some_and(|existing| existing != &ty)
    {
        return Err(FinalSemanticAnalysisError::LocalTypeUnavailable { owner });
    }
    if !facts.locals().contains_key(&owner) {
        facts
            .set_local_type(owner, ty)
            .map_err(FinalSemanticAnalysisError::from)?;
    }
    Ok(())
}

fn require_pattern(
    facts: &SemanticFactState,
    owner: PatternId,
    expected: &TypeKind,
) -> Result<(), FinalSemanticAnalysisError> {
    if facts.patterns().get(&owner) == Some(expected) {
        Ok(())
    } else {
        Err(FinalSemanticAnalysisError::PatternTypeUnavailable { owner })
    }
}

fn require_locals(
    facts: &SemanticFactState,
    locals: &[arcweft_lang_hir::identity::LocalId],
    expected: &TypeKind,
) -> Result<(), FinalSemanticAnalysisError> {
    for owner in locals {
        if facts.locals().get(owner) != Some(expected) {
            return Err(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: *owner });
        }
    }
    Ok(())
}
