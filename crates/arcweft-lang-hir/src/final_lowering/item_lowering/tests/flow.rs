use super::*;

use crate::arena::HirArenaPayload;
use crate::dialogue_application::HirLinePlanItem;
use crate::expr::{HirExprKind, HirThreadBodyOwner, HirThreadFlowItem};
use crate::item::{
    HirContractMode, HirFlowContractClause, HirFlowIdentity, HirFlowIssueClass, HirFlowIssueOwner,
    HirFlowItem, HirFlowReturn, HirFunctionBody,
};
use crate::module::HirModuleStatus;
use crate::scope::LocalLookup;
use crate::source_index::{
    HirExprSourceRole, HirFlowContractSourcePart, HirFlowParameterSourcePart,
    HirFlowReturnSourcePart, HirFlowSourceRole, HirItemSourceRole, HirSourceCommitInvariantError,
    HirSourceLookup, HirSourceOwnerStatus, HirSourcePresence, HirSourceQuery, HirSourceQueryError,
    HirSourceRequirement, HirStmtSourceRole, HirThreadBodySourceRole, HirThreadFlowItemSourcePart,
};
use crate::stmt::{
    HirStmtChildRole, HirStmtKind, HirStmtMatchArmBody, HirStmtPoisonState, HirStmtRecoveryIssue,
};

fn resolve_flow(
    module: &HirModule,
    ordinal: usize,
) -> (crate::identity::ItemId, &HirItem, &HirFlowItem) {
    let owner = module.source_ordered_items()[ordinal];
    let item = resolve_item(module, ordinal);
    let HirItemKind::Flow(flow) = item.kind() else {
        panic!("source-ordered item {ordinal} must be an ordinary Flow")
    };
    (owner, item, flow)
}

#[test]
fn dialogue_line_plan_owns_typed_let_callback_and_out_items() {
    let parsed = parse(
        "arcweft-test://proof/dialogue-line-plan-final-hir",
        concat!(
            "entry cli @entry.main { goto @flow.line_handles }\n",
            "pub character alice { display_name = \"Alice\" }\n",
            "flow line_handles() -> String {\n",
            "    let (_, cue) = alice(voice=auto)[聞いて。[p]]\n",
            "    with:\n",
            "        let actor = alice.stage.acquire(scope=line)\n",
            "        let cue = at(0.42s):\n",
            "            actor.look(.worried, crossfade=120ms)\n",
            "        let voice = line.voice_handle()\n",
            "        out (voice, cue)\n",
            "    log.info(\"cue kept\", cue = cue)\n",
            "    return \"done\"\n",
            "}\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(
        module.status(),
        HirModuleStatus::Clean,
        "{:#?}",
        module.diagnostics()
    );
    let (_, _, flow) = resolve_flow(&module, 2);
    let [
        HirThreadFlowItem::Statement(binding),
        HirThreadFlowItem::Statement(_),
        HirThreadFlowItem::Statement(_),
    ] = flow.body().items()
    else {
        panic!("line binding, log, and return must remain source-ordered statements")
    };
    let HirStmtKind::Let { initializer, .. } = module.resolve_stmt(*binding).unwrap().kind() else {
        panic!("dialogue line binding must remain a Let statement")
    };
    let HirExprKind::DialogueContentApplication(application) =
        module.resolve_expr(*initializer).unwrap().kind()
    else {
        panic!("Let initializer must be the Dialogue application")
    };
    let plan = application
        .plan()
        .expect("Dialogue application owns its line plan");
    assert_eq!(plan.items().len(), 4);
    assert!(matches!(plan.items()[0], HirLinePlanItem::Let { .. }));
    let HirLinePlanItem::Let { value: cue, .. } = plan.items()[1] else {
        panic!("timed cue binding remains a typed line-plan Let")
    };
    assert!(matches!(
        module.resolve_expr(cue).unwrap().kind(),
        HirExprKind::Call(_)
    ));
    assert!(matches!(plan.items()[2], HirLinePlanItem::Let { .. }));
    assert!(matches!(plan.items()[3], HirLinePlanItem::Out(_)));
}

#[test]
fn let_else_owns_failure_block_and_publishes_success_bindings() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-let-else",
        concat!(
            "flow main() -> String {\n",
            "    let Some(route) = Some(@flow.done) else {\n",
            "        return \"missing\"\n",
            "    }\n",
            "    goto route\n",
            "}\n",
            "flow done() -> String { return \"done\" }\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(
        module.status(),
        HirModuleStatus::Clean,
        "diagnostics: {:#?}; statements: {:#?}; patterns: {:#?}; locals: {:#?}",
        module.diagnostics(),
        module.statements().collect::<Vec<_>>(),
        module.patterns().collect::<Vec<_>>(),
        module.locals().collect::<Vec<_>>()
    );
    let (_, _, flow) = resolve_flow(&module, 0);
    let [
        HirThreadFlowItem::Statement(binding),
        HirThreadFlowItem::Statement(_),
    ] = flow.body().items()
    else {
        panic!("LetElse and Goto must remain source-ordered statements")
    };
    let HirStmtKind::LetElse {
        else_scope,
        else_body,
        locals,
        ..
    } = module.resolve_stmt(*binding).unwrap().kind()
    else {
        panic!("first statement must retain LetElse")
    };
    assert_eq!(else_body.len(), 1);
    assert_eq!(locals.len(), 1);
    assert_eq!(
        module.resolve_scope(*else_scope).unwrap().owner(),
        &crate::scope::HirScopeOwner::Stmt(*binding)
    );
}

fn flow_query<'module>(
    module: &'module HirModule,
    parsed: &ParsedSource,
    owner: crate::identity::ItemId,
    role: HirFlowSourceRole,
) -> HirSourceLookup<'module> {
    flow_query_result(module, parsed, owner, role).unwrap()
}

fn flow_query_result<'module>(
    module: &'module HirModule,
    parsed: &ParsedSource,
    owner: crate::identity::ItemId,
    role: HirFlowSourceRole,
) -> Result<HirSourceLookup<'module>, HirSourceQueryError> {
    module.source_site(parsed.document().identity(), flow_source_query(owner, role))
}

const fn flow_source_query(
    owner: crate::identity::ItemId,
    role: HirFlowSourceRole,
) -> HirSourceQuery {
    HirSourceQuery::Item {
        owner,
        role: HirItemSourceRole::Flow(role),
    }
}

#[test]
fn let_await_result_match_lowers_cleanly() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-let-await-with-inline",
        concat!(
            "extern capability fs {\n",
            "    type FsError\n",
            "    fn read_text(path: VirtualPath) -> Need<Result<String, FsError>> effects { fs.read }\n",
            "}\n",
            "extern capability path { fn save(path: String) -> VirtualPath }\n",
            "entry cli @entry.main { goto @flow.main }\n",
            "flow main() -> String effects { fs.read(save) } {\n",
            "    let value = match (await fs.read_text(path.save(\"profile.json\"))) {\n",
            "        .Ok(value) => value\n",
            "        .Err(_) => \"fallback\"\n",
            "    }\n",
            "    return value\n",
            "}\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert_eq!(
        module.status(),
        HirModuleStatus::Clean,
        "poisoned expressions: {:#?}; statements: {:#?}; patterns: {:#?}; locals: {:#?}; types: {:#?}; scopes: {:#?}; captures: {:#?}",
        module
            .expressions()
            .filter(|(_, value)| value.is_poisoned())
            .collect::<Vec<_>>(),
        module
            .statements()
            .filter(|(_, value)| value.is_poisoned())
            .collect::<Vec<_>>(),
        module
            .patterns()
            .filter(|(_, value)| value.is_poisoned())
            .collect::<Vec<_>>(),
        module
            .locals()
            .filter(|(_, value)| value.is_poisoned())
            .collect::<Vec<_>>(),
        module
            .types()
            .filter(|(_, value)| value.is_poisoned())
            .collect::<Vec<_>>(),
        module
            .scopes()
            .filter(|(_, value)| value.is_poisoned())
            .collect::<Vec<_>>(),
        module
            .captures()
            .filter(|(_, value)| value.is_poisoned())
            .collect::<Vec<_>>()
    );
    let (_, item, flow) = resolve_flow(&module, 3);
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Clean,
        "flow poison: {:?}; body: {:?}",
        flow.poison(),
        flow.body()
    );
    let [
        HirThreadFlowItem::Statement(statement),
        HirThreadFlowItem::Statement(_),
    ] = flow.body().items()
    else {
        panic!("Let and Return must remain two statement items")
    };
    let statement = module.resolve_stmt(*statement).unwrap();
    assert!(matches!(statement.kind(), HirStmtKind::Let { .. }));
    assert!(!statement.is_poisoned());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the ordinary Flow test asserts one complete signature/contract/body/source owner graph"
)]
fn ordinary_flow_lowers_one_shared_signature_contract_and_body_graph() {
    let source = concat!(
        "pub flow @flow.ordered ordered<T>(value: T) -> T where T: Bound\n",
        "requires prove ready(value)\n",
        "effects { asset.read }\n",
        "ensures check result\n",
        "reads { value.field }\n",
        "invariant debug stable(value)\n",
        "ensures no_effect network.request\n",
        "modifies { value.field }\n",
        "assume external_ok\n",
        "decreases value.remaining\n",
        "{}\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-flow-clean", source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);

    assert_eq!(
        item.state(),
        &HirItemPoisonState::Clean,
        "Flow poison: {:?}",
        flow.poison()
    );
    assert!(matches!(
        flow.identity(),
        HirFlowIdentity::PublicIdAndName { public_id, name }
            if public_id.absolute_family() == Some("flow") && name.as_str() == "ordered"
    ));
    assert_eq!(flow.generic_parameters().len(), 1);
    assert_eq!(flow.parameters().len(), 1);
    assert!(matches!(flow.result(), HirFlowReturn::Authored(_)));
    assert_eq!(flow.where_predicates().len(), 1);
    assert_eq!(flow.contracts().len(), 9);
    assert!(matches!(
        flow.contracts()[0],
        HirFlowContractClause::Requires(ref condition)
            if condition.mode() == HirContractMode::Prove
    ));
    assert!(matches!(
        flow.contracts()[2],
        HirFlowContractClause::Ensures(ref condition)
            if condition.mode() == HirContractMode::CheckRuntime
    ));
    assert!(matches!(
        flow.contracts()[4],
        HirFlowContractClause::Invariant(ref condition)
            if condition.mode() == HirContractMode::DebugCheck
    ));
    assert!(matches!(
        flow.contracts()[5],
        HirFlowContractClause::NoEffect { .. }
    ));
    let HirFlowContractClause::Effects(effects) = &flow.contracts()[1] else {
        panic!("second Flow contract must be the authored effects clause")
    };
    let HirFlowContractClause::NoEffect {
        expression: no_effect,
    } = &flow.contracts()[5]
    else {
        unreachable!("checked above")
    };
    assert_eq!(
        item.kind().effect_expression_roots(),
        effects
            .operands()
            .iter()
            .copied()
            .chain(std::iter::once(*no_effect))
            .collect::<Vec<_>>(),
        "only effects/no_effect operands enter the central effect-identity inventory"
    );
    assert_eq!(
        flow.contracts()
            .iter()
            .filter_map(HirFlowContractClause::admitted_effect_operands)
            .flatten()
            .copied()
            .collect::<Vec<_>>(),
        effects.operands(),
        "only effects operands enter the Flow's exposed effect row"
    );
    assert!(flow.body().items().is_empty());

    let callable = module
        .arenas()
        .scopes()
        .resolve(module.slots(), flow.callable_scope())
        .unwrap();
    assert_eq!(callable.kind(), HirScopeKind::Callable);
    assert_eq!(callable.parent(), Some(item.scope()));
    assert_eq!(callable.owner(), &HirScopeOwner::Item(owner));
    assert_eq!(
        callable.children(),
        [
            flow.requires_scope(),
            flow.ensures_scope(),
            flow.body_scope()
        ]
    );
    assert_eq!(callable.locals(), flow.parameters()[0].locals());
    let requires = module
        .arenas()
        .scopes()
        .resolve(module.slots(), flow.requires_scope())
        .unwrap();
    assert_eq!(requires.kind(), HirScopeKind::ContractRequires);
    assert!(requires.locals().is_empty());
    let ensures = module
        .arenas()
        .scopes()
        .resolve(module.slots(), flow.ensures_scope())
        .unwrap();
    let [result] = ensures.locals() else {
        panic!("one shared postcondition result local")
    };
    assert_eq!(flow.result_local().unwrap().local(), *result);
    let result = module
        .arenas()
        .locals()
        .resolve(module.slots(), *result)
        .unwrap();
    assert_eq!(result.kind(), HirLocalKind::PostconditionResult);
    assert_eq!(result.annotation(), flow.result().authored_type());
    let body = module
        .arenas()
        .scopes()
        .resolve(module.slots(), flow.body_scope())
        .unwrap();
    assert_eq!(body.kind(), HirScopeKind::Flow);
    assert_eq!(body.parent(), Some(flow.callable_scope()));

    let whole = flow_query(&module, &parsed, owner, HirFlowSourceRole::Whole);
    assert!(matches!(whole.presence(), HirSourcePresence::Present(_)));
    assert_eq!(whole.owner_status(), HirSourceOwnerStatus::Clean);
    let clause_keyword = flow_query(
        &module,
        &parsed,
        owner,
        HirFlowSourceRole::ContractClause {
            ordinal: 5,
            part: HirFlowContractSourcePart::ClauseKeyword,
        },
    );
    let no_effect_keyword = flow_query(
        &module,
        &parsed,
        owner,
        HirFlowSourceRole::ContractClause {
            ordinal: 5,
            part: HirFlowContractSourcePart::NoEffectKeyword,
        },
    );
    let (HirSourcePresence::Present(clause_keyword), HirSourcePresence::Present(no_effect_keyword)) =
        (clause_keyword.presence(), no_effect_keyword.presence())
    else {
        panic!("both authored no-effect keywords must retain exact source sites")
    };
    assert_ne!(clause_keyword, no_effect_keyword);
    let operand = flow_query(
        &module,
        &parsed,
        owner,
        HirFlowSourceRole::ContractClause {
            ordinal: 5,
            part: HirFlowContractSourcePart::Operand { ordinal: 0 },
        },
    );
    let operand_expression = module
        .source_site(
            parsed.document().identity(),
            HirSourceQuery::Expr {
                owner: *no_effect,
                role: HirExprSourceRole::Whole,
            },
        )
        .expect("NoEffect operand expression source");
    let HirSourcePresence::Present(operand_site) = operand.presence() else {
        panic!("NoEffect operand must retain its authored source component")
    };
    assert_eq!(operand.presence(), operand_expression.presence());
    assert_ne!(clause_keyword, operand_site);
    assert_ne!(no_effect_keyword, operand_site);
}

#[test]
#[allow(clippy::too_many_lines)]
fn root_and_nested_scope_kinds_are_allocated_exactly() {
    let source = concat!(
        "fn scoped()\n",
        "requires true\n",
        "ensures result == ()\n",
        "{\n",
        "    let conditional = if let value = source when ready { value } else { 0 };\n",
        "    let selected = match source { value => { value } };\n",
        "    let worker = thread { loop {} };\n",
        "    || ()\n",
        "}\n",
        "flow directed() {}\n",
        "predicate valid() = true\n",
        "proof checked() = ()\n",
    );
    let parsed = parse("arcweft-test://proof/final-hir-scope-kind-matrix", source);
    assert!(
        parsed.diagnostics().is_empty(),
        "unexpected syntax diagnostics: {:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    assert!(
        module.diagnostics().is_empty(),
        "{:#?}",
        module.diagnostics()
    );

    let [function_owner, flow_owner, predicate_owner, proof_owner] = module.source_ordered_items()
    else {
        panic!("scope fixture must retain four source-ordered callable items");
    };
    let function_item = module.resolve_item(*function_owner).unwrap();
    let flow_item = module.resolve_item(*flow_owner).unwrap();
    let predicate_item = module.resolve_item(*predicate_owner).unwrap();
    let proof_item = module.resolve_item(*proof_owner).unwrap();
    let HirItemKind::Function(function) = function_item.kind() else {
        panic!("first scope fixture item must be a Function");
    };
    let HirItemKind::Flow(flow) = flow_item.kind() else {
        panic!("second scope fixture item must be a Flow");
    };
    let HirItemKind::Predicate(predicate) = predicate_item.kind() else {
        panic!("third scope fixture item must be a Predicate");
    };
    let HirItemKind::Proof(proof) = proof_item.kind() else {
        panic!("fourth scope fixture item must be a Proof");
    };

    let root_id = function_item.scope();
    assert_eq!(flow_item.scope(), root_id);
    assert_eq!(predicate_item.scope(), root_id);
    assert_eq!(proof_item.scope(), root_id);
    let roots = module
        .scopes()
        .filter(|(_, scope)| scope.parent().is_none())
        .collect::<Vec<_>>();
    let [(resolved_root_id, root)] = roots.as_slice() else {
        panic!("accepted module must own exactly one root scope");
    };
    assert_eq!(*resolved_root_id, root_id);
    assert_eq!(root.kind(), HirScopeKind::Module);
    assert_eq!(root.owner(), &HirScopeOwner::Module(root_id.module()));
    assert_eq!(
        root.children(),
        [
            function.callable_scope(),
            flow.callable_scope(),
            predicate.callable_scope(),
            proof.callable_scope(),
        ]
    );

    let HirFunctionBody::Block {
        scope: function_body,
        ..
    } = function.body()
    else {
        panic!("scope fixture Function must retain its authored block");
    };
    let callable_rows = [
        (
            function.callable_scope(),
            *function_owner,
            function.requires_scope(),
            function.ensures_scope(),
            *function_body,
            HirScopeKind::Block,
        ),
        (
            flow.callable_scope(),
            *flow_owner,
            flow.requires_scope(),
            flow.ensures_scope(),
            flow.body_scope(),
            HirScopeKind::Flow,
        ),
        (
            predicate.callable_scope(),
            *predicate_owner,
            predicate.requires_scope(),
            predicate.ensures_scope(),
            predicate.body().scope(),
            HirScopeKind::Predicate,
        ),
        (
            proof.callable_scope(),
            *proof_owner,
            proof.requires_scope(),
            proof.ensures_scope(),
            proof.body().scope(),
            HirScopeKind::Proof,
        ),
    ];
    for (callable_id, owner, requires_id, ensures_id, body_id, body_kind) in callable_rows {
        let callable = module.resolve_scope(callable_id).unwrap();
        assert_eq!(callable.kind(), HirScopeKind::Callable);
        assert_eq!(callable.parent(), Some(root_id));
        assert_eq!(callable.owner(), &HirScopeOwner::Item(owner));
        assert_eq!(callable.children(), [requires_id, ensures_id, body_id]);

        let requires = module.resolve_scope(requires_id).unwrap();
        let ensures = module.resolve_scope(ensures_id).unwrap();
        let body = module.resolve_scope(body_id).unwrap();
        assert_eq!(requires.kind(), HirScopeKind::ContractRequires);
        assert_eq!(ensures.kind(), HirScopeKind::ContractEnsures);
        assert_eq!(body.kind(), body_kind);
        for child in [requires, ensures, body] {
            assert_eq!(child.parent(), Some(callable_id));
            assert_eq!(child.owner(), &HirScopeOwner::Item(owner));
        }
    }

    let (thread_id, thread) = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Thread(thread) => Some((owner, thread)),
            _ => None,
        })
        .expect("scope fixture Thread expression");
    let (closure_id, closure) = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Closure(closure) => Some((owner, closure)),
            _ => None,
        })
        .expect("scope fixture closure expression");
    let function_body_scope = module.resolve_scope(*function_body).unwrap();
    let (if_owner, if_let) = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::IfLet(if_let) => Some((owner, if_let)),
            _ => None,
        })
        .expect("IfLet scope owner");
    let conditional_scope = if_let.scope();
    let HirExprKind::Block(then_block) = module.resolve_expr(if_let.then_branch()).unwrap().kind()
    else {
        panic!("IfLet then branch must retain its authored Block scope")
    };
    let HirExprKind::Block(else_block) = module.resolve_expr(if_let.else_branch()).unwrap().kind()
    else {
        panic!("IfLet else branch must retain its authored Block scope")
    };
    let (match_owner, match_expression) = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Match(match_expression) => Some((owner, match_expression)),
            _ => None,
        })
        .expect("Match scope owner");
    let [match_arm] = match_expression.arms() else {
        panic!("scope fixture Match must retain one arm")
    };
    let match_scope = match_arm.scope();
    let HirExprKind::Block(match_value_block) =
        module.resolve_expr(match_arm.value()).unwrap().kind()
    else {
        panic!("Match arm value must retain its authored Block scope")
    };
    assert_eq!(
        function_body_scope.children(),
        [
            conditional_scope,
            else_block.scope(),
            match_scope,
            thread.scope(),
            closure.scope()
        ]
    );
    for (scope_id, kind, owner) in [
        (
            conditional_scope,
            HirScopeKind::Conditional,
            HirScopeOwner::Expr(if_owner),
        ),
        (
            else_block.scope(),
            HirScopeKind::Block,
            HirScopeOwner::Expr(if_let.else_branch()),
        ),
        (
            match_scope,
            HirScopeKind::MatchArm,
            HirScopeOwner::Expr(match_owner),
        ),
        (
            thread.scope(),
            HirScopeKind::Block,
            HirScopeOwner::Expr(thread_id),
        ),
        (
            closure.scope(),
            HirScopeKind::Closure,
            HirScopeOwner::Expr(closure_id),
        ),
    ] {
        let scope = module.resolve_scope(scope_id).unwrap();
        assert_eq!(scope.kind(), kind);
        assert_eq!(scope.parent(), Some(*function_body));
        assert_eq!(scope.owner(), &owner);
    }
    for (scope_id, parent, owner) in [
        (then_block.scope(), conditional_scope, if_let.then_branch()),
        (match_value_block.scope(), match_scope, match_arm.value()),
    ] {
        let scope = module.resolve_scope(scope_id).unwrap();
        assert_eq!(scope.kind(), HirScopeKind::Block);
        assert_eq!(scope.parent(), Some(parent));
        assert_eq!(scope.owner(), &HirScopeOwner::Expr(owner));
    }

    let [HirThreadFlowItem::Statement(loop_owner)] = thread.body().items() else {
        panic!("Thread fixture must retain one loop expression statement");
    };
    let loop_statement = module.resolve_stmt(*loop_owner).unwrap();
    let HirStmtKind::Expression { expression } = loop_statement.kind() else {
        panic!("Thread loop item must retain an ordinary expression statement");
    };
    let HirExprKind::Loop(loop_expression) = module.resolve_expr(*expression).unwrap().kind()
    else {
        panic!("Thread expression statement must retain the Loop expression payload");
    };
    let loop_scope = module.resolve_scope(loop_expression.scope()).unwrap();
    assert_eq!(loop_scope.kind(), HirScopeKind::Block);
    assert_eq!(loop_scope.parent(), Some(thread.scope()));
    assert_eq!(loop_scope.owner(), &HirScopeOwner::Expr(*expression));
    assert_eq!(
        module.resolve_scope(thread.scope()).unwrap().children(),
        [loop_expression.scope()]
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn postcondition_result_is_ensures_only() {
    let source = concat!(
        "flow counted() -> I32\n",
        "requires ready\n",
        "ensures result > 0\n",
        "{}\n",
        "flow unit()\n",
        "requires ready\n",
        "ensures result == ()\n",
        "{}\n",
        "flow no_postcondition() -> I32\n",
        "requires ready\n",
        "{}\n",
    );
    let parsed = parse(
        "arcweft-test://proof/final-hir-postcondition-result-scope",
        source,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);

    let (_, _, counted) = resolve_flow(&module, 0);
    let result_id = counted
        .result_local()
        .expect("non-Unit Flow with ensures owns result")
        .local();
    let result = module.resolve_local(result_id).unwrap();
    assert_eq!(result.kind(), HirLocalKind::PostconditionResult);
    assert_eq!(result.name().as_str(), "result");
    assert_eq!(result.scope(), counted.ensures_scope());
    assert_eq!(result.generation(), LocalGeneration::FIRST);
    assert!(!result.is_mutable_binding());
    assert_eq!(result.annotation(), counted.result().authored_type());
    assert!(
        module
            .resolve_scope(counted.callable_scope())
            .unwrap()
            .locals()
            .is_empty()
    );
    assert!(
        module
            .resolve_scope(counted.requires_scope())
            .unwrap()
            .locals()
            .is_empty()
    );
    assert!(
        module
            .resolve_scope(counted.body_scope())
            .unwrap()
            .locals()
            .is_empty()
    );
    assert_eq!(
        module
            .resolve_scope(counted.ensures_scope())
            .unwrap()
            .locals(),
        [result_id]
    );

    let requires = counted
        .contracts()
        .iter()
        .find_map(|contract| match contract {
            HirFlowContractClause::Requires(condition) => Some(condition.expression()),
            _ => None,
        })
        .expect("counted requires expression");
    let ensures = counted
        .contracts()
        .iter()
        .find_map(|contract| match contract {
            HirFlowContractClause::Ensures(condition) => Some(condition.expression()),
            _ => None,
        })
        .expect("counted ensures expression");
    let source_span = |expression| match module.metadata(expression).unwrap().source_site() {
        HirSourceSite::Span(span) => span.clone(),
        HirSourceSite::Insertion(_) => panic!("authored contract expression must own a span"),
    };
    assert_eq!(
        module.lookup_local(
            counted.requires_scope(),
            result.name(),
            source_span(requires)
        ),
        Ok(LocalLookup::NotFound)
    );
    assert_eq!(
        module.lookup_local(counted.ensures_scope(), result.name(), source_span(ensures)),
        Ok(LocalLookup::Found(result_id))
    );
    let body_start = source.find("{}\nflow unit").unwrap();
    let body_span = parsed
        .document()
        .span(SourceRange::new(body_start, body_start + 1))
        .unwrap();
    assert_eq!(
        module.lookup_local(counted.body_scope(), result.name(), body_span),
        Ok(LocalLookup::NotFound)
    );

    let (_, _, unit) = resolve_flow(&module, 1);
    let unit_result = unit
        .result_local()
        .expect("ensures allocates result even for omitted semantic Unit")
        .local();
    let unit_result = module.resolve_local(unit_result).unwrap();
    assert_eq!(unit_result.scope(), unit.ensures_scope());
    assert_eq!(unit_result.annotation(), None);
    assert_eq!(unit_result.kind(), HirLocalKind::PostconditionResult);

    let (_, _, no_postcondition) = resolve_flow(&module, 2);
    assert!(no_postcondition.result_local().is_none());
    assert!(
        module
            .resolve_scope(no_postcondition.ensures_scope())
            .unwrap()
            .locals()
            .is_empty()
    );
}

#[test]
fn ordinary_flow_identity_matrix_retains_raw_ids_and_typed_poison() {
    for (ordinal, (source, expected_clean)) in [
        ("flow opening {}", true),
        ("flow @flow.opening {}", true),
        ("flow @flow.opening opening {}", true),
        ("flow @flow:. opening {}", true),
        ("flow @view.opening {}", false),
        ("flow @flow.opening start {}", false),
        ("flow {}", false),
    ]
    .into_iter()
    .enumerate()
    {
        let parsed = parse(
            &format!("arcweft-test://proof/final-hir-flow-identity-{ordinal}"),
            source,
        );
        let key = module_key(&parsed);
        let mut database = HirDatabase::try_new().unwrap();
        let mut transaction = stage(&database, &parsed, &key);
        transaction.lower_parsed_source_items(&parsed).unwrap();
        let module = transaction
            .finish(&mut database)
            .unwrap_or_else(|error| panic!("{source}: {error:?}"))
            .into_module();
        let (owner, item, flow) = resolve_flow(&module, 0);
        assert_eq!(!item.is_poisoned(), expected_clean, "{source}");
        assert_eq!(flow.poison().is_poisoned(), !expected_clean, "{source}");
        if !expected_clean {
            assert_eq!(
                flow.poison().primary().unwrap().class(),
                HirFlowIssueClass::Identity,
                "{source}"
            );
            assert_eq!(
                flow_query(&module, &parsed, owner, HirFlowSourceRole::Whole).owner_status(),
                HirSourceOwnerStatus::Poisoned,
                "{source}"
            );
        }
    }
}

#[test]
fn flow_id_name_mismatch_keeps_name_primary_and_public_id_related() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-identity-mismatch-order",
        "flow @flow.opening start {}",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);

    assert!(item.is_poisoned());
    let primary = flow.poison().primary().unwrap();
    assert_eq!(primary.class(), HirFlowIssueClass::Identity);
    assert_eq!(primary.owner(), HirFlowIssueOwner::Item(owner));
    assert_eq!(
        primary.source(),
        &flow_source_query(owner, HirFlowSourceRole::Name)
    );
    let [public_id] = flow.poison().related() else {
        panic!("ID/name mismatch must retain exactly one related public-ID issue")
    };
    assert_eq!(public_id.class(), HirFlowIssueClass::Identity);
    assert_eq!(public_id.owner(), HirFlowIssueOwner::Item(owner));
    assert_eq!(
        public_id.source(),
        &flow_source_query(owner, HirFlowSourceRole::PublicId)
    );
}

#[test]
fn flow_reserved_result_and_missing_body_commit_roleful_recovery() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-reserved-result",
        "flow constrained(result: Bool) ensures result {}",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (_, item, flow) = resolve_flow(&module, 0);
    assert!(item.is_poisoned());
    assert_eq!(
        flow.poison().primary().unwrap().class(),
        HirFlowIssueClass::Signature
    );
    let parameter_local = module
        .arenas()
        .locals()
        .resolve(module.slots(), flow.parameters()[0].locals()[0])
        .unwrap();
    assert_eq!(parameter_local.name().as_str(), "result");
    assert!(parameter_local.is_poisoned());
    assert!(flow.result_local().is_some());

    let source = "flow unfinished";
    let parsed = parse("arcweft-test://proof/final-hir-flow-missing-body", source);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);
    assert_eq!(module.source_ordered_items(), [owner]);
    assert_eq!(module.status(), HirModuleStatus::Recovered);
    assert!(!module.is_executable());
    assert!(!module.is_cache_eligible());
    assert_eq!(
        item.state(),
        &HirItemPoisonState::Poisoned(HirItemIssue::MissingBody)
    );
    assert_eq!(
        flow.poison().primary().unwrap().class(),
        HirFlowIssueClass::MissingBody
    );
    assert!(flow.body().items().is_empty());
    assert_eq!(flow.body().scope(), flow.body_scope());

    let callable = module.resolve_scope(flow.callable_scope()).unwrap();
    assert_eq!(callable.kind(), HirScopeKind::Callable);
    assert_eq!(callable.parent(), Some(item.scope()));
    assert_eq!(callable.owner(), &HirScopeOwner::Item(owner));
    assert_eq!(
        callable.children(),
        [
            flow.requires_scope(),
            flow.ensures_scope(),
            flow.body_scope()
        ]
    );
    for (scope, kind) in [
        (flow.requires_scope(), HirScopeKind::ContractRequires),
        (flow.ensures_scope(), HirScopeKind::ContractEnsures),
        (flow.body_scope(), HirScopeKind::Flow),
    ] {
        let scope = module.resolve_scope(scope).unwrap();
        assert_eq!(scope.kind(), kind);
        assert_eq!(scope.parent(), Some(flow.callable_scope()));
        assert_eq!(scope.owner(), &HirScopeOwner::Item(owner));
    }

    let body_query = flow_source_query(owner, HirFlowSourceRole::Body);
    assert_eq!(
        module.source_components().requirement(&body_query),
        Some(HirSourceRequirement::Required)
    );
    let body_source = module
        .source_site(parsed.document().identity(), body_query)
        .unwrap();
    let HirSourcePresence::Present(HirSourceSite::Insertion(insertion)) = body_source.presence()
    else {
        panic!("missing required Flow body must publish its checked insertion")
    };
    assert_eq!(insertion.offset(), source.len());
    assert_eq!(insertion.source_identity(), parsed.document().identity());
}

#[test]
fn flow_absent_roles_are_not_manifest_rows_and_win_before_source_identity() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-inapplicable",
        "flow plain {}",
    );
    let wrong_source = parse(
        "arcweft-test://proof/final-hir-flow-inapplicable-wrong-source",
        "flow other {}",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, _, _) = resolve_flow(&module, 0);

    for role in [
        HirFlowSourceRole::Visibility,
        HirFlowSourceRole::PublicId,
        HirFlowSourceRole::GenericGroup,
        HirFlowSourceRole::ParameterGroup,
        HirFlowSourceRole::Return {
            part: HirFlowReturnSourcePart::Whole,
        },
        HirFlowSourceRole::Return {
            part: HirFlowReturnSourcePart::Arrow,
        },
        HirFlowSourceRole::Return {
            part: HirFlowReturnSourcePart::Type,
        },
        HirFlowSourceRole::WhereClause,
    ] {
        let query = flow_source_query(owner, role);
        assert_eq!(
            module.source_components().requirement(&query),
            None,
            "{role:?}"
        );
        assert!(matches!(
            module.source_site(wrong_source.document().identity(), query),
            Err(HirSourceQueryError::ItemRoleNotApplicable {
                owner: actual,
                role: HirItemSourceRole::Flow(actual_role),
            }) if actual == owner && actual_role == role
        ));
    }

    assert!(matches!(
        flow_query(&module, &parsed, owner, HirFlowSourceRole::Name).presence(),
        HirSourcePresence::Present(_)
    ));
}

#[test]
fn flow_role_validation_is_bounds_first_and_rejects_default_mode() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-role-order",
        "flow guarded\nrequires ready\nreads asset\n{}\n",
    );
    let wrong_source = parse(
        "arcweft-test://proof/final-hir-flow-role-order-wrong-source",
        "flow other {}",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, _, _) = resolve_flow(&module, 0);

    let mode = HirFlowSourceRole::ContractClause {
        ordinal: 0,
        part: HirFlowContractSourcePart::Mode,
    };
    assert_eq!(
        module
            .source_components()
            .requirement(&flow_source_query(owner, mode)),
        None
    );
    assert!(matches!(
        flow_query_result(&module, &wrong_source, owner, mode),
        Err(HirSourceQueryError::ItemRoleNotApplicable { .. })
    ));

    for part in [
        HirFlowContractSourcePart::OpenDelimiter,
        HirFlowContractSourcePart::CloseDelimiter,
    ] {
        let role = HirFlowSourceRole::ContractClause { ordinal: 1, part };
        assert_eq!(
            module
                .source_components()
                .requirement(&flow_source_query(owner, role)),
            None
        );
        assert!(matches!(
            flow_query_result(&module, &wrong_source, owner, role),
            Err(HirSourceQueryError::ItemRoleNotApplicable { .. })
        ));
    }

    for (role, length) in [
        (
            HirFlowSourceRole::Parameter {
                ordinal: 0,
                part: HirFlowParameterSourcePart::Whole,
            },
            0,
        ),
        (
            HirFlowSourceRole::ContractClause {
                ordinal: 2,
                part: HirFlowContractSourcePart::Whole,
            },
            2,
        ),
        (HirFlowSourceRole::TrailingRecovery { ordinal: 0 }, 0),
    ] {
        assert!(matches!(
            flow_query_result(&module, &wrong_source, owner, role),
            Err(HirSourceQueryError::ItemOrdinalOutOfBounds {
                owner: actual,
                role: HirItemSourceRole::Flow(actual_role),
                length: actual_length,
            }) if actual == owner && actual_role == role && actual_length == length
        ));
    }
}

#[test]
fn flow_signature_recovery_uses_one_committed_trailing_ordinal_family() {
    let source = "flow invalid(first: Int = make_value())(second: Int) -> Int {}";
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-signature-recovery-source",
        source,
    );
    let wrong_source = parse(
        "arcweft-test://proof/final-hir-flow-signature-recovery-wrong-source",
        "flow other {}",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, _, flow) = resolve_flow(&module, 0);

    assert_eq!(flow.parameters().len(), 1);
    let issues = flow
        .poison()
        .primary()
        .into_iter()
        .chain(flow.poison().related())
        .collect::<Vec<_>>();
    assert_eq!(issues.len(), 2);
    for (ordinal, expected_start) in [
        (0, source.find('=').unwrap()),
        (1, source.find("(second").unwrap()),
    ] {
        let issue = issues[usize::try_from(ordinal).unwrap()];
        assert_eq!(issue.class(), HirFlowIssueClass::Signature);
        assert_eq!(issue.owner(), HirFlowIssueOwner::Item(owner));
        assert_eq!(
            issue.source(),
            &flow_source_query(owner, HirFlowSourceRole::TrailingRecovery { ordinal },)
        );
        let lookup = flow_query(
            &module,
            &parsed,
            owner,
            HirFlowSourceRole::TrailingRecovery { ordinal },
        );
        let HirSourcePresence::Present(HirSourceSite::Span(span)) = lookup.presence() else {
            panic!("signature recovery {ordinal} must retain an authored span")
        };
        assert_eq!(span.range().start(), expected_start);
    }

    assert!(matches!(
        flow_query_result(
            &module,
            &wrong_source,
            owner,
            HirFlowSourceRole::TrailingRecovery { ordinal: 2 },
        ),
        Err(HirSourceQueryError::ItemOrdinalOutOfBounds { length: 2, .. })
    ));
}

#[test]
fn flow_body_projects_the_shared_fifteen_variant_inventory_without_a_tail() {
    let source = format!("flow matrix {{\n{}}}\n", thread_flow_matrix_body());
    let parsed = parse("arcweft-test://proof/final-hir-flow-body-matrix", &source);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (flow_owner, item, flow) = resolve_flow(&module, 0);
    assert!(item.is_poisoned(), "the Error row is recovery-only");
    assert_eq!(flow.body().items().len(), 15);
    assert_eq!(
        flow.poison().primary().unwrap().class(),
        HirFlowIssueClass::BodyChild
    );
    assert!(matches!(
        flow.body().items()[0],
        crate::expr::HirThreadFlowItem::Statement(_)
    ));
    assert!(matches!(
        flow.body().items()[1],
        crate::expr::HirThreadFlowItem::DialogueApplication(_)
    ));
    assert!(matches!(
        flow.body().items()[14],
        crate::expr::HirThreadFlowItem::Error(_)
    ));

    let Some(HirThreadFlowItem::Match(match_owner)) = flow.body().items().get(5) else {
        panic!("Flow body ordinal 5 must retain its typed Match statement ID");
    };
    let statement = module
        .resolve_stmt(*match_owner)
        .expect("Flow Match statement at ordinal 5");
    let HirStmtKind::Match(matched) = statement.kind() else {
        panic!("Flow body ordinal 5 must resolve to the Match statement family");
    };
    let [arm] = matched.arms() else {
        panic!("Flow matrix Match must retain one source-ordered arm");
    };
    let arm_scope = module
        .resolve_scope(arm.scope())
        .expect("Flow Match braced-arm scope");
    assert_eq!(arm_scope.kind(), HirScopeKind::Block);
    assert_eq!(arm_scope.parent(), Some(flow.body().scope()));
    assert_eq!(arm_scope.owner(), &HirScopeOwner::Stmt(*match_owner));
    let HirStmtMatchArmBody::Body(body) = arm.body() else {
        panic!("Flow Match braced arm must retain one contextual body");
    };
    assert_eq!(body.scope(), arm.scope());
    assert_eq!(
        body.thread_body()
            .expect("Flow Match braced arm uses the shared nested body owner")
            .scope(),
        arm.scope()
    );

    let item_query = HirSourceQuery::ThreadBody {
        owner: HirThreadBodyOwner::Flow(flow_owner),
        role: HirThreadBodySourceRole::Item {
            ordinal: 5,
            part: HirThreadFlowItemSourcePart::Whole,
        },
    };
    let child_query = HirSourceQuery::ThreadBody {
        owner: HirThreadBodyOwner::Flow(flow_owner),
        role: HirThreadBodySourceRole::Item {
            ordinal: 5,
            part: HirThreadFlowItemSourcePart::ChildWhole,
        },
    };
    let statement_query = HirSourceQuery::Stmt {
        owner: *match_owner,
        role: HirStmtSourceRole::Whole,
    };
    let item_source = module
        .source_site(parsed.document().identity(), item_query)
        .expect("Flow ordinal 5 source component");
    let child_source = module
        .source_site(parsed.document().identity(), child_query)
        .expect("Flow ordinal 5 child source relation");
    let statement_source = module
        .source_site(parsed.document().identity(), statement_query)
        .expect("Flow Match statement Whole source relation");
    assert_eq!(item_source.presence(), statement_source.presence());
    assert_eq!(child_source.presence(), statement_source.presence());
    let HirSourcePresence::Present(HirSourceSite::Span(match_span)) = item_source.presence() else {
        panic!("Flow Match ordinal must retain its authored source span");
    };
    assert_eq!(
        match_span.range().start(),
        source
            .find("match value")
            .expect("Flow Match source offset")
    );
}

#[test]
fn flow_match_missing_arm_body_retains_typed_child_and_roleful_recovery() {
    let source = "flow recovered {\n    match subject { value => }\n}\n";
    let parsed = parse("arcweft-test://proof/final-hir-flow-match-recovery", source);
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (flow_owner, item, flow) = resolve_flow(&module, 0);

    assert!(item.is_poisoned());
    let [HirThreadFlowItem::Match(match_owner)] = flow.body().items() else {
        panic!("malformed Flow Match must retain its typed statement item");
    };
    let statement = module
        .resolve_stmt(*match_owner)
        .expect("recovered Flow Match statement");
    let HirStmtKind::Match(matched) = statement.kind() else {
        panic!("malformed Flow Match must not collapse to Error or raw source");
    };
    assert_eq!(
        statement.state(),
        &HirStmtPoisonState::Poisoned(HirStmtRecoveryIssue::RecoveredChild {
            role: HirStmtChildRole::MatchArmBody { arm: 0 },
        })
    );
    let [arm] = matched.arms() else {
        panic!("recovered Flow Match must retain one typed arm");
    };
    let HirStmtMatchArmBody::Expression(body) = arm.body() else {
        panic!("missing Flow Match arm body must retain a typed expression recovery");
    };
    assert!(matches!(
        module
            .slots()
            .resolve(*body)
            .expect("missing Flow Match arm body slot")
            .origin(),
        HirOrigin::Synthetic(key)
            if key.owner() == SyntheticOwner::Scope(arm.scope())
                && key.role() == SyntheticRole::MissingRequiredTail
                && key.ordinal() == 0
    ));

    let issue = flow
        .poison()
        .primary()
        .expect("malformed Flow Match publishes one roleful body-child issue");
    assert_eq!(issue.class(), HirFlowIssueClass::BodyChild);
    assert_eq!(issue.owner(), HirFlowIssueOwner::Stmt(*match_owner));
    assert_eq!(
        issue.source(),
        &HirSourceQuery::ThreadBody {
            owner: HirThreadBodyOwner::Flow(flow_owner),
            role: HirThreadBodySourceRole::Item {
                ordinal: 0,
                part: HirThreadFlowItemSourcePart::ChildWhole,
            },
        }
    );
}

#[test]
fn flow_body_retains_every_recovered_child_before_the_missing_close() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-body-recovery-order",
        "flow recovered {\n    ???\n    ???\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);

    assert!(item.is_poisoned());
    assert_eq!(flow.body().items().len(), 2);
    let issues = flow
        .poison()
        .primary()
        .into_iter()
        .chain(flow.poison().related())
        .collect::<Vec<_>>();
    assert_eq!(issues.len(), 3);
    for (ordinal, issue) in issues[..2].iter().enumerate() {
        let ordinal = u32::try_from(ordinal).unwrap();
        assert_eq!(issue.class(), HirFlowIssueClass::BodyChild);
        assert!(matches!(issue.owner(), HirFlowIssueOwner::Stmt(_)));
        assert_eq!(
            issue.source(),
            &HirSourceQuery::ThreadBody {
                owner: crate::expr::HirThreadBodyOwner::Flow(owner),
                role: HirThreadBodySourceRole::Item {
                    ordinal,
                    part: HirThreadFlowItemSourcePart::ChildWhole,
                },
            }
        );
    }
    assert_eq!(issues[2].class(), HirFlowIssueClass::UnclosedBody);
    assert_eq!(issues[2].owner(), HirFlowIssueOwner::Item(owner));
    assert_eq!(
        issues[2].source(),
        &flow_source_query(owner, HirFlowSourceRole::BodyClose)
    );
}

#[test]
fn flow_contract_poison_retains_each_missing_operand_owner_in_clause_order() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-contract-recovery-order",
        "flow recovered_contracts()\nrequires\nensures no_effect\n{}\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);

    assert!(item.is_poisoned());
    assert_eq!(flow.contracts().len(), 2);
    let issues = flow
        .poison()
        .primary()
        .into_iter()
        .chain(flow.poison().related())
        .collect::<Vec<_>>();
    assert_eq!(issues.len(), 2);
    for (ordinal, issue) in issues.iter().enumerate() {
        let ordinal = u16::try_from(ordinal).unwrap();
        assert_eq!(issue.class(), HirFlowIssueClass::Contract);
        assert!(matches!(issue.owner(), HirFlowIssueOwner::Expr(_)));
        assert_eq!(
            issue.source(),
            &flow_source_query(
                owner,
                HirFlowSourceRole::ContractClause {
                    ordinal,
                    part: HirFlowContractSourcePart::Operand { ordinal: 0 },
                },
            )
        );
    }
}

#[test]
fn duplicate_decreases_keeps_later_keyword_primary_and_first_keyword_related() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-duplicate-decreases",
        "flow measure()\ndecreases first\ndecreases second\n{}\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &key);
    let (owner, item, flow) = resolve_flow(&module, 0);

    assert!(item.is_poisoned());
    assert_eq!(flow.contracts().len(), 2);
    let primary = flow.poison().primary().unwrap();
    assert_eq!(primary.class(), HirFlowIssueClass::Contract);
    assert_eq!(
        primary.source(),
        &flow_source_query(
            owner,
            HirFlowSourceRole::ContractClause {
                ordinal: 1,
                part: HirFlowContractSourcePart::ClauseKeyword,
            },
        )
    );
    let [first] = flow.poison().related() else {
        panic!("duplicate decreases must retain the first keyword as related evidence")
    };
    assert_eq!(first.class(), HirFlowIssueClass::Contract);
    assert_eq!(
        first.source(),
        &flow_source_query(
            owner,
            HirFlowSourceRole::ContractClause {
                ordinal: 0,
                part: HirFlowContractSourcePart::ClauseKeyword,
            },
        )
    );
}

#[test]
fn flow_source_freeze_rejects_typed_component_substitution_and_retries_deterministically() {
    let parsed = parse(
        "arcweft-test://proof/final-hir-flow-source-freeze",
        "flow frozen {}\n",
    );
    let key = module_key(&parsed);
    let mut database = HirDatabase::try_new().unwrap();
    let mut transaction = stage(&database, &parsed, &key);
    transaction
        .lower_parsed_source_items(&parsed)
        .expect("valid Flow lowers before source substitution");
    let [failed_owner] = transaction.staged_source_ordered_items() else {
        panic!("source-freeze fixture must stage one ordinary Flow")
    };
    let failed_owner = *failed_owner;
    let failed_snapshot = transaction.snapshot_id();

    let items = parsed.items().unwrap();
    let [attached_flow @ TypedItemNode::Flow(_)] = items.as_slice() else {
        panic!("source-freeze fixture must retain one typed Flow item")
    };
    let query = flow_source_query(failed_owner, HirFlowSourceRole::Name);
    assert_eq!(
        transaction
            .source_components()
            .inject_component_for_test(&query, HirSourceSite::Span(attached_flow.source_span()),),
        Err(HirSourceCommitInvariantError::ConflictingComponent {
            query: query.clone(),
        })
    );
    assert!(matches!(
        transaction.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));
    assert!(database.current(&key).is_none());

    let mut retry = stage(&database, &parsed, &key);
    assert_eq!(retry.snapshot_id(), failed_snapshot);
    retry
        .lower_parsed_source_items(&parsed)
        .expect("valid Flow retry after rejected source substitution");
    assert_eq!(retry.staged_source_ordered_items(), [failed_owner]);
    let accepted = retry.finish(&mut database).unwrap().into_module();
    assert_eq!(
        flow_query(&accepted, &parsed, failed_owner, HirFlowSourceRole::Name).owner_status(),
        HirSourceOwnerStatus::Clean
    );
}

#[test]
fn flow_source_queries_reject_wrong_document_and_stale_revision() {
    let name = SourceName::path("proof/final-hir-flow-source-query.arcw");
    let document_id = "arcweft-test://proof/final-hir-flow-source-query";
    let source = "flow stable {}\n";
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial
                    .document()
                    .span(SourceRange::new(source.len(), source.len()))
                    .unwrap(),
                " ",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&revised);
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &revised, &key);
    let owner = module.source_ordered_items()[0];
    let query = flow_source_query(owner, HirFlowSourceRole::Name);

    assert!(matches!(
        module.source_site(initial.document().identity(), query.clone()),
        Err(HirSourceQueryError::StaleSourceRevision { expected, actual })
            if expected == revised.document().identity().revision()
                && actual == initial.document().identity().revision()
    ));

    let foreign = parse(
        "arcweft-test://proof/final-hir-flow-source-query-foreign",
        source,
    );
    assert!(matches!(
        module.source_site(foreign.document().identity(), query),
        Err(HirSourceQueryError::WrongSourceDocument { expected, actual })
            if expected == *revised.document().identity().id()
                && actual == *foreign.document().identity().id()
    ));
}

#[test]
fn rejected_flow_revision_preserves_prior_publication_and_retry_identity() {
    let name = SourceName::path("proof/final-hir-flow-publication.arcw");
    let document_id = "arcweft-test://proof/final-hir-flow-publication";
    let source = "flow stable {}\n";
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let initial = syntax
        .parse_initial(
            SourceSnapshotId::initial(name.clone()),
            source_document(document_id, &name, source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let revised = syntax
        .reparse(
            &initial,
            &[SourceEdit::new(
                initial.document().span(SourceRange::new(0, 0)).unwrap(),
                " ",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let key = module_key(&initial);
    let revised_key = module_key(&revised);
    assert_eq!(revised_key.package(), key.package());
    assert_eq!(revised_key.path(), key.path());
    assert_ne!(revised_key.source(), key.source());

    let mut database = HirDatabase::try_new().unwrap();
    let prior = lower(&mut database, &initial, &key);
    let prior_owner = prior.source_ordered_items()[0];
    let prior_snapshot = prior.snapshot_id();
    let prior_epoch = prior.invalidation_epoch();
    let prior_name_site =
        match flow_query(&prior, &initial, prior_owner, HirFlowSourceRole::Name).presence() {
            HirSourcePresence::Present(site) => site.clone(),
            HirSourcePresence::AbsentOptional => panic!("accepted Flow name must be present"),
        };
    let before = database.test_state();

    let mut rejected = stage(&database, &revised, &key);
    rejected
        .lower_parsed_source_items(&revised)
        .expect("revised Flow lowers before source-manifest rejection");
    let [failed_owner] = rejected.staged_source_ordered_items() else {
        panic!("revised fixture must stage one ordinary Flow")
    };
    let failed_owner = *failed_owner;
    let failed_snapshot = rejected.snapshot_id();
    assert!(
        rejected
            .source_components()
            .remove_staged_query(&flow_source_query(failed_owner, HirFlowSourceRole::Name))
    );
    assert!(matches!(
        rejected.finish(&mut database),
        Err(HirLowerFailure::Invariant(
            HirInvariantFailure::InvalidSourceIndex
        ))
    ));

    assert_eq!(database.test_state(), before);
    let retained = database.current(&key).expect("prior Flow remains current");
    assert!(Arc::ptr_eq(&retained, &prior));
    assert_eq!(retained.snapshot_id(), prior_snapshot);
    assert_eq!(retained.invalidation_epoch(), prior_epoch);
    assert_eq!(
        flow_query(&retained, &initial, prior_owner, HirFlowSourceRole::Name,).presence(),
        HirSourcePresence::Present(&prior_name_site)
    );
    assert!(matches!(
        flow_query_result(
            &retained,
            &revised,
            prior_owner,
            HirFlowSourceRole::Name,
        ),
        Err(HirSourceQueryError::StaleSourceRevision { expected, actual })
            if expected == initial.document().identity().revision()
                && actual == revised.document().identity().revision()
    ));

    let mut retry = stage(&database, &revised, &key);
    assert_eq!(retry.snapshot_id(), failed_snapshot);
    retry
        .lower_parsed_source_items(&revised)
        .expect("valid Flow retry after rejected publication");
    assert_eq!(retry.staged_source_ordered_items(), [failed_owner]);
    let output = retry.finish(&mut database).unwrap();
    assert_eq!(output.invalidations().previous(), Some(prior_snapshot));
    assert_eq!(output.invalidations().current(), failed_snapshot);
    assert!(output.invalidations().is_empty());
    assert_eq!(
        output.module().invalidation_epoch().get(),
        prior_epoch.get() + 1
    );
    let accepted = database
        .current(&revised_key)
        .expect("retried Flow is current");
    assert!(Arc::ptr_eq(&accepted, output.module()));
    assert_eq!(
        flow_query(&accepted, &revised, failed_owner, HirFlowSourceRole::Name,).owner_status(),
        HirSourceOwnerStatus::Clean
    );
}

fn thread_flow_matrix_body() -> &'static str {
    concat!(
        "    return unit\n",
        "    alice[こんにちは。]\n",
        "    choice {}\n",
        "    if ready {}\n",
        "    if let value = source {}\n",
        "    match value { _ => {} }\n",
        "    loop {}\n",
        "    while ready {}\n",
        "    while let value = source {}\n",
        "    for value in source {}\n",
        "    select {}\n",
        "    source locale en-US {}\n",
        "    scope local {}\n",
        "    include @flow.shared\n",
        "    ???\n",
    )
}
