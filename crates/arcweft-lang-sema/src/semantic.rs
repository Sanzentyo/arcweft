use crate::env::TypeCheckEnv;
mod facts;
use crate::fact_layer::{
    Capability, EffectScope, ProofFacts, ResourceAccess, resource_accesses_from_expr,
    resource_write_for_lifetime, resource_write_for_signal, write_capability_for_call,
    write_capability_for_method,
};
use arcweft_lang_hir::model::{
    HirAwait, HirBorrow, HirChoice, HirFlowItem, HirFor, HirFunction, HirIf, HirIfLet, HirLoop,
    HirMatch, HirModule, HirScope, HirScopeExpr, HirSelect, HirTopLevelDecl, HirWhile, HirWhileLet,
};
use arcweft_lang_hir::syntax::{
    ast::{
        choice::{ChoiceBlock, ChoicePlanItem},
        common::TextRange,
        flow::{
            AwaitWith, FlowItem, LoopBlock, ScopeExprBlock, SelectBranchHead, Stmt, ThreadBlock,
            WaitTarget,
        },
        ids::{EntityRefSyntax, IdRef},
        line_plan::{LinePlan, LinePlanItem, TriggerPattern},
        pattern::Pattern,
    },
    expr::{Expr, LifetimeKey, LifetimeScopeKind, Literal},
};
use facts::{BlockFlow, DeferredCleanup, ExitPath, ExitReason, FlowFacts, transfer_reason};
use std::collections::{BTreeMap, BTreeSet, HashSet};

type FlowState = FlowFacts;

const LOOP_FIXPOINT_LIMIT: usize = 16;

/// Semantic verification mode selected by compiler, CLI, or LSP tooling.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SemanticMode {
    /// Collect obligations as warnings unless the source is structurally unsafe.
    #[default]
    Dev,
    /// Test policy: proof obligations without a discharge are errors.
    Test,
    /// Release policy: missing proofs and audited unsafe are both errors.
    Release,
}

/// Semantic analysis policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SemanticPolicy {
    pub mode: SemanticMode,
}

/// Tool-facing severity emitted by semantic analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticSeverity {
    Info,
    Warning,
    Error,
}

/// Semantic obligation families produced before solver-specific verification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticObligationKind {
    LifetimePromotion,
    UnsafeLifetimeAudit,
    MustDropDischarge,
    ThreadCapture,
    ThreadJoinTyping,
    UpperLifetimeWrite,
    EffectCapability,
    ProofBody,
    TrustedAssumption,
    RawSyntax,
    RuntimeConflict,
}

/// How a semantic obligation is discharged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticDischarge {
    Automatic,
    FormalProof { id: String },
    AuditedUnsafe { id: String },
    TrustedAxiom { id: String },
    Missing,
}

/// Stable source span used by semantic diagnostics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SemanticSourceSpan {
    pub start: usize,
    pub end: usize,
}

/// One semantic proof or safety obligation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticObligation {
    pub id: String,
    pub kind: SemanticObligationKind,
    pub message: String,
    pub subject: Option<String>,
    pub source: Option<SemanticSourceSpan>,
    pub discharge: SemanticDischarge,
}

/// Diagnostic produced by the semantic pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDiagnostic {
    pub id: String,
    pub severity: SemanticSeverity,
    pub message: String,
    pub source: Option<SemanticSourceSpan>,
    pub obligation: Option<String>,
    pub related_ids: Vec<String>,
}

/// Summary of a source-level proof item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticProofSummary {
    pub id: String,
    pub source: SemanticSourceSpan,
}

/// Summary of a trusted axiom declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTrustedAxiomSummary {
    pub id: String,
    pub source: SemanticSourceSpan,
}

/// Summary of an audited unsafe lifetime region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticUnsafeAuditSummary {
    pub id: String,
    pub source: Option<SemanticSourceSpan>,
    pub has_reason: bool,
    pub has_safety_doc: bool,
}

/// Structured semantic analysis result consumed by verifier, CLI, and LSP.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SemanticReport {
    pub policy: SemanticPolicy,
    pub diagnostics: Vec<SemanticDiagnostic>,
    pub obligations: Vec<SemanticObligation>,
    pub proofs: Vec<SemanticProofSummary>,
    pub trusted_axioms: Vec<SemanticTrustedAxiomSummary>,
    pub unsafe_audits: Vec<SemanticUnsafeAuditSummary>,
}

/// Runs the Phase 1.9 semantic verification spine over HIR.
pub fn analyze_semantics(
    module: &HirModule,
    env: &TypeCheckEnv,
    policy: SemanticPolicy,
) -> SemanticReport {
    let mut analyzer = SemanticAnalyzer::new(env, policy);
    analyzer.collect_module(module);
    analyzer.finish()
}

struct SemanticAnalyzer<'a> {
    env: &'a TypeCheckEnv,
    policy: SemanticPolicy,
    report: SemanticReport,
    next_obligation: usize,
    unsafe_stack: Vec<String>,
    known_proofs: BTreeMap<String, ProofFacts>,
    known_axioms: BTreeSet<String>,
    effect_stack: Vec<EffectScope>,
    reported_must_drop: HashSet<String>,
}

impl<'a> SemanticAnalyzer<'a> {
    fn new(env: &'a TypeCheckEnv, policy: SemanticPolicy) -> Self {
        Self {
            env,
            policy,
            report: SemanticReport {
                policy,
                ..SemanticReport::default()
            },
            next_obligation: 0,
            unsafe_stack: Vec::new(),
            known_proofs: BTreeMap::new(),
            known_axioms: BTreeSet::new(),
            effect_stack: Vec::new(),
            reported_must_drop: HashSet::new(),
        }
    }

    fn collect_module(&mut self, module: &HirModule) {
        self.collect_declarations(module);
        for flow in module.flows() {
            self.effect_stack
                .push(EffectScope::from_contracts(flow.contracts()));
            let flow = self.analyze_flow_items(flow.body(), vec![FlowFacts::default()]);
            self.effect_stack.pop();
            self.finish_block_flow(flow, ExitReason::Completed);
        }
        for function in module.functions() {
            self.collect_function(function);
        }
        let top_level =
            self.analyze_flow_items(module.top_level_items(), vec![FlowFacts::default()]);
        self.finish_block_flow(top_level, ExitReason::Completed);
    }

    fn collect_declarations(&mut self, module: &HirModule) {
        for declaration in module.declarations() {
            if let HirTopLevelDecl::TrustedAxiom(axiom) = declaration {
                self.known_axioms.insert(id_ref_label(axiom.id(), "axiom"));
            }
        }
        for declaration in module.declarations() {
            match declaration {
                HirTopLevelDecl::Proof(proof) => {
                    let id = id_ref_label(proof.id(), "proof");
                    let facts = ProofFacts::from_clauses(proof.clauses(), &self.known_axioms);
                    for issue in facts.issues() {
                        self.add_obligation(
                            SemanticObligationKind::ProofBody,
                            format!("proof `{id}`: {}", issue.message()),
                            issue
                                .subject()
                                .map(str::to_owned)
                                .or_else(|| Some(id.clone())),
                            SemanticDischarge::Missing,
                        );
                    }
                    self.known_proofs.insert(id.clone(), facts);
                    self.report.proofs.push(SemanticProofSummary {
                        id,
                        source: span_from_range(proof.range()),
                    });
                }
                HirTopLevelDecl::TrustedAxiom(axiom) => {
                    let id = id_ref_label(axiom.id(), "axiom");
                    self.report
                        .trusted_axioms
                        .push(SemanticTrustedAxiomSummary {
                            id,
                            source: span_from_range(axiom.range()),
                        });
                }
                HirTopLevelDecl::Hook(hook) => {
                    self.effect_stack
                        .push(EffectScope::from_effects(hook.effects()));
                    self.collect_stmt_list(hook.body_statements());
                    self.effect_stack.pop();
                }
                HirTopLevelDecl::MemoFn(item) => self.collect_stmt_list(item.body_statements()),
                HirTopLevelDecl::Parser(item) => self.collect_stmt_list(item.body_statements()),
                HirTopLevelDecl::Source(item) => self.collect_stmt_list(item.body_statements()),
                _ => {}
            }
        }
    }

    fn collect_function(&mut self, function: &HirFunction) {
        self.effect_stack
            .push(EffectScope::from_contracts(function.contracts()));
        let mut flow = self.analyze_stmts(
            function.statements(),
            vec![FlowFacts::default()],
            ExitReason::Completed,
        );
        if let Some(value) = function.value() {
            for facts in &mut flow.fallthrough {
                self.collect_expr(value, facts);
            }
        }
        self.effect_stack.pop();
        self.finish_block_flow(flow, ExitReason::Completed);
    }

    fn analyze_flow_items(&mut self, items: &[HirFlowItem], initial: Vec<FlowFacts>) -> BlockFlow {
        let mut fallthrough = initial;
        let mut exits = Vec::new();
        let mut sibling_writes = BTreeMap::<ResourceAccess, String>::new();

        for item in items {
            if fallthrough.is_empty() {
                break;
            }
            if let HirFlowItem::Stmt(Stmt::Thread(thread)) = item {
                self.check_sibling_thread_conflicts(thread, &mut sibling_writes);
            }

            let mut next = Vec::new();
            for facts in fallthrough {
                let flow = self.analyze_flow_item(item, facts);
                next.extend(flow.fallthrough);
                exits.extend(flow.exits);
            }
            fallthrough = next;
        }

        BlockFlow { fallthrough, exits }
    }

    fn analyze_flow_item(&mut self, item: &HirFlowItem, mut facts: FlowFacts) -> BlockFlow {
        match item {
            HirFlowItem::Stmt(stmt) => self.analyze_stmt(stmt, facts, ExitReason::Completed),
            HirFlowItem::If(block) => {
                self.collect_expr(block.condition(), &mut facts);
                let mut flow = BlockFlow::from_fallthrough(facts.clone());
                flow.append(self.analyze_flow_items(block.body(), vec![facts]));
                flow
            }
            HirFlowItem::IfLet(block) => {
                self.collect_expr(block.expr(), &mut facts);
                if let Some(guard) = block.guard() {
                    self.collect_expr(guard, &mut facts);
                }
                let mut flow = BlockFlow::from_fallthrough(facts.clone());
                flow.append(self.analyze_flow_items(block.body(), vec![facts]));
                flow
            }
            HirFlowItem::Match(block) => {
                self.collect_expr(block.expr(), &mut facts);
                let mut flow = BlockFlow::default();
                for arm in block.arms() {
                    let mut arm_facts = facts.clone();
                    if let Some(guard) = arm.guard() {
                        self.collect_expr(guard, &mut arm_facts);
                    }
                    flow.append(self.analyze_flow_items(arm.body(), vec![arm_facts]));
                }
                if flow.fallthrough.is_empty() && flow.exits.is_empty() {
                    flow.fallthrough.push(facts);
                }
                flow
            }
            HirFlowItem::Loop(block) | HirFlowItem::LetLoop { block, .. } => {
                self.analyze_loop_flow_items(block.body(), facts, false)
            }
            HirFlowItem::While(block) => {
                self.collect_expr(block.condition(), &mut facts);
                self.analyze_loop_flow_items(block.body(), facts, true)
            }
            HirFlowItem::WhileLet(block) => {
                self.collect_expr(block.expr(), &mut facts);
                if let Some(guard) = block.guard() {
                    self.collect_expr(guard, &mut facts);
                }
                self.analyze_loop_flow_items(block.body(), facts, true)
            }
            HirFlowItem::For(block) => {
                self.collect_expr(block.source(), &mut facts);
                self.analyze_loop_flow_items(block.body(), facts, true)
            }
            HirFlowItem::Dialogue(dialogue) => {
                for arg in dialogue.args() {
                    self.collect_expr(arg.value(), &mut facts);
                }
                if let Some(plan) = dialogue.plan() {
                    self.collect_line_plan(plan);
                }
                BlockFlow::from_fallthrough(facts)
            }
            _ => {
                self.collect_flow_item(item, &mut facts);
                BlockFlow::from_fallthrough(facts)
            }
        }
    }

    fn analyze_loop_flow_items(
        &mut self,
        body: &[HirFlowItem],
        facts: FlowFacts,
        may_skip: bool,
    ) -> BlockFlow {
        let mut head = facts.clone();
        let mut exits = Vec::new();
        let mut breaks = Vec::new();

        for _ in 0..LOOP_FIXPOINT_LIMIT {
            let body_flow = self.analyze_flow_items(body, vec![head.clone()]);
            let mut changed = false;
            for body_facts in body_flow.fallthrough {
                changed |= head.merge_from(&body_facts);
            }
            for exit in body_flow.exits {
                match exit.reason {
                    ExitReason::Continue => changed |= head.merge_from(&exit.facts),
                    ExitReason::Break => push_unique_facts(&mut breaks, exit.facts),
                    _ => push_unique_exit(&mut exits, exit),
                }
            }
            if !changed {
                break;
            }
        }

        let mut fallthrough = Vec::new();
        if may_skip {
            push_unique_facts(&mut fallthrough, facts);
            push_unique_facts(&mut fallthrough, head);
        }
        for facts in breaks {
            push_unique_facts(&mut fallthrough, facts);
        }
        BlockFlow { fallthrough, exits }
    }

    fn collect_flow_items(&mut self, items: &[HirFlowItem], state: &mut FlowState) {
        let mut sibling_writes = BTreeMap::<ResourceAccess, String>::new();
        for item in items {
            if let HirFlowItem::Stmt(Stmt::Thread(thread)) = item {
                self.check_sibling_thread_conflicts(thread, &mut sibling_writes);
            }
            self.collect_flow_item(item, state);
        }
    }

    fn collect_flow_item(&mut self, item: &HirFlowItem, state: &mut FlowState) {
        match item {
            HirFlowItem::Stmt(stmt) => self.collect_stmt(stmt, state),
            HirFlowItem::Dialogue(dialogue) => {
                for arg in dialogue.args() {
                    self.collect_expr(arg.value(), state);
                }
                if let Some(plan) = dialogue.plan() {
                    self.collect_line_plan(plan);
                }
            }
            HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
                self.collect_choice(choice);
            }
            HirFlowItem::LetScope { scope, .. } => self.collect_scope_expr(scope),
            HirFlowItem::LetLoop { block, .. } | HirFlowItem::Loop(block) => {
                self.collect_loop(block);
            }
            HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
                self.collect_await(await_with);
            }
            HirFlowItem::If(block) => self.collect_if(block, state),
            HirFlowItem::IfLet(block) => self.collect_if_let(block, state),
            HirFlowItem::Match(block) => self.collect_match(block, state),
            HirFlowItem::While(block) => self.collect_while(block, state),
            HirFlowItem::WhileLet(block) => self.collect_while_let(block, state),
            HirFlowItem::For(block) => self.collect_for(block, state),
            HirFlowItem::Select(block) => self.collect_select(block),
            HirFlowItem::Borrow(block) => self.collect_borrow(block),
            HirFlowItem::SourceLocale(block) => self.collect_flow_items(block.body(), state),
            HirFlowItem::Scope(block) => self.collect_scope(block),
            HirFlowItem::Scenario { args, .. } => {
                for arg in args {
                    self.collect_expr(arg, state);
                }
            }
            HirFlowItem::Include(_) => {}
        }
    }

    fn collect_stmt_list(&mut self, stmts: &[Stmt]) {
        let flow = self.analyze_stmts(stmts, vec![FlowFacts::default()], ExitReason::Completed);
        self.finish_block_flow(flow, ExitReason::Completed);
    }

    fn analyze_stmts(
        &mut self,
        stmts: &[Stmt],
        initial: Vec<FlowFacts>,
        context: ExitReason,
    ) -> BlockFlow {
        let mut fallthrough = initial;
        let mut exits = Vec::new();
        let mut sibling_writes = BTreeMap::<ResourceAccess, String>::new();

        for stmt in stmts {
            if fallthrough.is_empty() {
                break;
            }
            if let Stmt::Thread(thread) = stmt {
                self.check_sibling_thread_conflicts(thread, &mut sibling_writes);
            }

            let mut next = Vec::new();
            for facts in fallthrough {
                let flow = self.analyze_stmt(stmt, facts, context);
                next.extend(flow.fallthrough);
                exits.extend(flow.exits);
            }
            fallthrough = next;
        }

        BlockFlow { fallthrough, exits }
    }

    fn analyze_stmt(
        &mut self,
        stmt: &Stmt,
        mut facts: FlowFacts,
        context: ExitReason,
    ) -> BlockFlow {
        if let Some(reason) = transfer_reason(stmt, context) {
            self.collect_transfer_stmt_expr(stmt, &mut facts);
            return BlockFlow::from_exit(reason, facts);
        }

        match stmt {
            Stmt::LetElse {
                expr, else_body, ..
            } => {
                self.collect_expr(expr, &mut facts);
                let mut flow = BlockFlow::from_fallthrough(facts.clone());
                flow.exits
                    .extend(self.analyze_stmts(else_body, vec![facts], context).exits);
                flow
            }
            Stmt::If { condition, body } => {
                self.collect_expr(condition, &mut facts);
                let mut flow = BlockFlow::from_fallthrough(facts.clone());
                flow.append(self.analyze_stmts(body, vec![facts], context));
                flow
            }
            Stmt::Match { expr, arms } => {
                self.collect_expr(expr, &mut facts);
                let mut flow = BlockFlow::default();
                for arm in arms {
                    let mut arm_facts = facts.clone();
                    if let Some(guard) = arm.guard() {
                        self.collect_expr(guard, &mut arm_facts);
                    }
                    flow.append(self.analyze_stmts(arm.body(), vec![arm_facts], context));
                }
                if flow.fallthrough.is_empty() && flow.exits.is_empty() {
                    flow.fallthrough.push(facts);
                }
                flow
            }
            Stmt::Loop { body } => self.analyze_loop_stmts(body, facts, context, false),
            Stmt::While { condition, body } => {
                self.collect_expr(condition, &mut facts);
                self.analyze_loop_stmts(body, facts, context, true)
            }
            Stmt::WhileLet {
                expr, guard, body, ..
            } => {
                self.collect_expr(expr, &mut facts);
                if let Some(guard) = guard {
                    self.collect_expr(guard, &mut facts);
                }
                self.analyze_loop_stmts(body, facts, context, true)
            }
            Stmt::For { source, body, .. } => {
                self.collect_expr(source, &mut facts);
                self.analyze_loop_stmts(body, facts, context, true)
            }
            Stmt::DeferBlock {
                outcome,
                statements,
            } => {
                facts.register_cleanup(DeferredCleanup::new(
                    *outcome,
                    drop_keys_in_stmts(statements),
                ));
                self.inspect_cleanup_stmts(statements);
                BlockFlow::from_fallthrough(facts)
            }
            Stmt::Defer { outcome, expr } => {
                facts.register_cleanup(DeferredCleanup::new(*outcome, drop_keys_in_expr(expr)));
                self.inspect_cleanup_expr(expr);
                BlockFlow::from_fallthrough(facts)
            }
            Stmt::Thread(thread) => {
                self.collect_thread(thread);
                BlockFlow::from_fallthrough(facts)
            }
            Stmt::On { trigger, body } => {
                self.collect_trigger(trigger, &mut facts);
                let flow = self.analyze_stmts(body, vec![FlowFacts::default()], context);
                self.finish_block_flow(flow, context);
                BlockFlow::from_fallthrough(facts)
            }
            Stmt::UnsafeLifetime {
                id,
                reason,
                has_safety_doc,
                body,
            } => {
                self.collect_unsafe_lifetime(id, reason.as_ref(), *has_safety_doc, body);
                BlockFlow::from_fallthrough(facts)
            }
            _ => {
                self.collect_stmt(stmt, &mut facts);
                BlockFlow::from_fallthrough(facts)
            }
        }
    }

    fn analyze_loop_stmts(
        &mut self,
        body: &[Stmt],
        facts: FlowFacts,
        context: ExitReason,
        may_skip: bool,
    ) -> BlockFlow {
        let mut head = facts.clone();
        let mut exits = Vec::new();
        let mut breaks = Vec::new();

        for _ in 0..LOOP_FIXPOINT_LIMIT {
            let body_flow = self.analyze_stmts(body, vec![head.clone()], context);
            let mut changed = false;
            for body_facts in body_flow.fallthrough {
                changed |= head.merge_from(&body_facts);
            }
            for exit in body_flow.exits {
                match exit.reason {
                    ExitReason::Continue => changed |= head.merge_from(&exit.facts),
                    ExitReason::Break => push_unique_facts(&mut breaks, exit.facts),
                    _ => push_unique_exit(&mut exits, exit),
                }
            }
            if !changed {
                break;
            }
        }

        let mut fallthrough = Vec::new();
        if may_skip {
            push_unique_facts(&mut fallthrough, facts);
            push_unique_facts(&mut fallthrough, head);
        }
        for facts in breaks {
            push_unique_facts(&mut fallthrough, facts);
        }
        BlockFlow { fallthrough, exits }
    }

    fn collect_transfer_stmt_expr(&mut self, stmt: &Stmt, facts: &mut FlowFacts) {
        match stmt {
            Stmt::Return(expr)
            | Stmt::Close(expr)
            | Stmt::Goto(expr)
            | Stmt::Yield(expr)
            | Stmt::Panic(expr)
            | Stmt::Fail(expr)
            | Stmt::Bail(expr)
            | Stmt::Out { expr, .. }
            | Stmt::Break {
                expr: Some(expr), ..
            } => self.collect_expr(expr, facts),
            _ => {}
        }
    }

    fn collect_stmts(&mut self, stmts: &[Stmt], state: &mut FlowState) {
        let mut sibling_writes = BTreeMap::<ResourceAccess, String>::new();
        for stmt in stmts {
            if let Stmt::Thread(thread) = stmt {
                self.check_sibling_thread_conflicts(thread, &mut sibling_writes);
            }
            self.collect_stmt(stmt, state);
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "statement traversal intentionally mirrors Stmt so semantic coverage stays auditable"
    )]
    fn collect_stmt(&mut self, stmt: &Stmt, state: &mut FlowState) {
        match stmt {
            Stmt::LetElse {
                expr, else_body, ..
            } => {
                self.collect_expr(expr, state);
                let mut else_state = state.clone();
                self.collect_stmts(else_body, &mut else_state);
                state.live_must_drop.extend(else_state.live_must_drop);
            }
            Stmt::LetChoice { choice, .. } => self.collect_choice_syntax(choice),
            Stmt::LetScope { scope, .. } => self.collect_scope_expr_syntax(scope),
            Stmt::LetLoop { block, .. } => self.collect_loop_syntax(block),
            Stmt::LetAwait { await_with, .. } => self.collect_await_syntax(await_with),
            Stmt::Return(expr)
            | Stmt::Close(expr)
            | Stmt::Expr(expr)
            | Stmt::Panic(expr)
            | Stmt::Fail(expr)
            | Stmt::Bail(expr)
            | Stmt::Select(expr)
            | Stmt::Goto(expr)
            | Stmt::Yield(expr)
            | Stmt::Defer { expr, .. }
            | Stmt::Let { expr, .. }
            | Stmt::Out { expr, .. }
            | Stmt::Break {
                expr: Some(expr), ..
            } => self.collect_expr(expr, state),
            Stmt::Ensure { condition, message } => {
                self.collect_expr(condition, state);
                self.collect_expr(message, state);
            }
            Stmt::Thread(thread) => self.collect_thread(thread),
            Stmt::DeferBlock { statements, .. } => {
                self.collect_stmts(statements, state);
            }
            Stmt::Signal { target, value }
            | Stmt::LifetimeSet {
                target,
                expr: value,
            } => {
                if matches!(stmt, Stmt::LifetimeSet { .. }) {
                    self.collect_lifetime_write(target);
                }
                self.collect_expr(target, state);
                self.collect_expr(value, state);
            }
            Stmt::Wait(target) => self.collect_wait(target, state),
            Stmt::On { trigger, body } => {
                self.collect_trigger(trigger, state);
                self.collect_stmt_list(body);
            }
            Stmt::UnsafeLifetime {
                id,
                reason,
                has_safety_doc,
                body,
            } => self.collect_unsafe_lifetime(id, reason.as_ref(), *has_safety_doc, body),
            Stmt::Command(command) => {
                for arg in command.args() {
                    self.collect_expr(arg, state);
                }
            }
            Stmt::If { condition, body } => {
                self.collect_expr(condition, state);
                let mut body_state = state.clone();
                self.collect_stmts(body, &mut body_state);
                state.live_must_drop.extend(body_state.live_must_drop);
            }
            Stmt::Loop { body } | Stmt::While { body, .. } => {
                if let Stmt::While { condition, .. } = stmt {
                    self.collect_expr(condition, state);
                }
                let mut body_state = state.clone();
                self.collect_stmts(body, &mut body_state);
                state.live_must_drop.extend(body_state.live_must_drop);
            }
            Stmt::WhileLet {
                expr, guard, body, ..
            } => {
                self.collect_expr(expr, state);
                if let Some(guard) = guard {
                    self.collect_expr(guard, state);
                }
                let mut body_state = state.clone();
                self.collect_stmts(body, &mut body_state);
                state.live_must_drop.extend(body_state.live_must_drop);
            }
            Stmt::For { source, body, .. } => {
                self.collect_expr(source, state);
                let mut body_state = state.clone();
                self.collect_stmts(body, &mut body_state);
                state.live_must_drop.extend(body_state.live_must_drop);
            }
            Stmt::Match { expr, arms } => {
                self.collect_expr(expr, state);
                for arm in arms {
                    let mut arm_state = state.clone();
                    if let Some(guard) = arm.guard() {
                        self.collect_expr(guard, &mut arm_state);
                    }
                    self.collect_stmts(arm.body(), &mut arm_state);
                    state.live_must_drop.extend(arm_state.live_must_drop);
                }
            }
            Stmt::Break { expr: None, .. } | Stmt::Continue { .. } => {}
            Stmt::Raw(raw) => self.add_raw_obligation(
                format!("raw {:?} recovery node: {}", raw.family(), raw.source()),
                raw.range().map(|range| format!("{range:?}")),
            ),
        }
    }

    fn collect_choice(&mut self, choice: &HirChoice) {
        let mut state = FlowState::default();
        for option in choice.options() {
            if let Some(condition) = option.condition() {
                self.collect_expr(condition, &mut state);
            }
            if let Some(value) = option.value() {
                self.collect_expr(value, &mut state);
            }
        }
        if let Some(plan) = choice.plan() {
            for item in plan.items() {
                self.collect_choice_plan_item(item);
            }
        }
        self.finish_scope(&state);
    }

    fn collect_choice_syntax(&mut self, choice: &ChoiceBlock) {
        if let Some(plan) = choice.plan() {
            for item in plan.items() {
                self.collect_choice_plan_item(item);
            }
        }
    }

    fn collect_choice_plan_item(&mut self, item: &ChoicePlanItem) {
        let mut state = FlowState::default();
        match item {
            ChoicePlanItem::Option { value, .. } => self.collect_expr(value, &mut state),
            ChoicePlanItem::Timeout { duration, body } => {
                self.collect_expr(duration, &mut state);
                self.collect_stmts(body, &mut state);
            }
            ChoicePlanItem::Cancel { trigger, body } => {
                self.collect_trigger(trigger, &mut state);
                self.collect_stmts(body, &mut state);
            }
            ChoicePlanItem::OnSelect { body, .. } => {
                self.collect_stmts(body, &mut state);
            }
            ChoicePlanItem::Raw(raw) => {
                self.add_raw_obligation(
                    format!("raw {:?} recovery node: {}", raw.family(), raw.source()),
                    raw.range().map(|range| format!("{range:?}")),
                );
            }
        }
        self.finish_scope(&state);
    }

    fn collect_line_plan(&mut self, plan: &LinePlan) {
        self.check_line_plan_child_conflicts(plan.items(), false);
        let flow = self.analyze_line_plan_items(plan.items(), vec![FlowFacts::default()]);
        self.finish_block_flow(flow, ExitReason::Completed);
    }

    fn analyze_line_plan_items(
        &mut self,
        items: &[LinePlanItem],
        initial: Vec<FlowFacts>,
    ) -> BlockFlow {
        let mut fallthrough = initial;
        let mut exits = Vec::new();

        for item in items {
            if fallthrough.is_empty() {
                break;
            }
            let mut next = Vec::new();
            for facts in fallthrough {
                let flow = self.analyze_line_plan_item(item, facts);
                next.extend(flow.fallthrough);
                exits.extend(flow.exits);
            }
            fallthrough = next;
        }

        BlockFlow { fallthrough, exits }
    }

    fn analyze_line_plan_item(&mut self, item: &LinePlanItem, mut facts: FlowFacts) -> BlockFlow {
        match item {
            LinePlanItem::Init(stmts) => {
                self.analyze_stmts(stmts, vec![facts], ExitReason::Completed)
            }
            LinePlanItem::Stmt(stmt) => self.analyze_stmt(stmt, facts, ExitReason::Completed),
            LinePlanItem::Out(value) => {
                self.collect_expr(value, &mut facts);
                BlockFlow::from_exit(ExitReason::Completed, facts)
            }
            LinePlanItem::Let { expr, .. }
            | LinePlanItem::Option { value: expr, .. }
            | LinePlanItem::Assert { expr, .. }
            | LinePlanItem::Expr(expr) => {
                self.collect_expr(expr, &mut facts);
                BlockFlow::from_fallthrough(facts)
            }
            LinePlanItem::TimedCue { anchor, body } => {
                self.collect_expr(anchor, &mut facts);
                self.collect_expr(body, &mut facts);
                BlockFlow::from_fallthrough(facts)
            }
            LinePlanItem::Memo { options, .. } => {
                for (_, value) in options {
                    self.collect_expr(value, &mut facts);
                }
                BlockFlow::from_fallthrough(facts)
            }
            LinePlanItem::CancelRule(rule) => {
                let flow =
                    self.analyze_stmts(rule.action(), vec![facts.clone()], ExitReason::Cancelled);
                self.finish_block_flow(flow, ExitReason::Cancelled);
                BlockFlow::from_fallthrough(facts)
            }
            LinePlanItem::Thread(thread) => {
                self.collect_thread(thread);
                BlockFlow::from_fallthrough(facts)
            }
            LinePlanItem::On { trigger, body } => {
                self.collect_trigger(trigger, &mut facts);
                let flow =
                    self.analyze_stmts(body, vec![FlowFacts::default()], ExitReason::Completed);
                self.finish_block_flow(flow, ExitReason::Completed);
                BlockFlow::from_fallthrough(facts)
            }
            LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
                self.check_line_plan_child_conflicts(
                    items,
                    matches!(item, LinePlanItem::TogetherGroup(_)),
                );
                let flow = self.analyze_line_plan_items(items, vec![FlowFacts::default()]);
                self.finish_block_flow(flow, ExitReason::Completed);
                BlockFlow::from_fallthrough(facts)
            }
            LinePlanItem::Raw(raw) => {
                self.add_raw_obligation(
                    format!("raw {:?} recovery node: {}", raw.family(), raw.source()),
                    raw.range().map(|range| format!("{range:?}")),
                );
                BlockFlow::from_fallthrough(facts)
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "expression traversal mirrors the public Expr enum so semantic coverage is auditable"
    )]
    fn collect_expr(&mut self, expr: &Expr, state: &mut FlowState) {
        match expr {
            Expr::Literal(_) | Expr::Path(_) | Expr::Placeholder(_) | Expr::EntityRef(_) => {}
            Expr::Raw(raw) => self.add_raw_obligation(format!("raw expression: {raw}"), None),
            Expr::LifetimePath { key, .. } => {
                if is_must_drop_key(key) {
                    state.add_must_drop(key.clone());
                }
            }
            Expr::Tuple(items) | Expr::BracketSeq(items) => {
                for item in items {
                    self.collect_expr(item, state);
                }
            }
            Expr::ArrayRepeat { value, len } => {
                self.collect_expr(value, state);
                self.collect_expr(len, state);
            }
            Expr::Call { callee, args } => self.collect_call(callee, args, state),
            Expr::NamedArg { value, .. } => self.collect_expr(value, state),
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => self.collect_method_call(receiver, method, args, state),
            Expr::Field { target, .. } => self.collect_expr(target, state),
            Expr::DialogueCall { callee, plan, .. } => {
                self.collect_expr(callee, state);
                if let Some(plan) = plan {
                    self.collect_line_plan(plan);
                }
            }
            Expr::Index { target, index } => {
                self.collect_expr(target, state);
                self.collect_expr(index, state);
            }
            Expr::Pipe { lhs, rhs } => {
                if Self::collect_lifetime_drop_pipe(lhs, rhs, state) {
                    self.collect_expr(rhs, state);
                    return;
                }
                self.collect_expr(lhs, state);
                self.collect_expr(rhs, state);
            }
            Expr::Try { expr } | Expr::Await { expr, .. } | Expr::Unary { expr, .. } => {
                self.collect_expr(expr, state);
            }
            Expr::Thread { block } => self.collect_thread(block),
            Expr::Range { start, end, .. } => {
                if let Some(start) = start {
                    self.collect_expr(start, state);
                }
                if let Some(end) = end {
                    self.collect_expr(end, state);
                }
            }
            Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
                for (_, value) in fields {
                    self.collect_expr(value, state);
                }
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.collect_expr(lhs, state);
                self.collect_expr(rhs, state);
            }
            Expr::Closure { body, .. } => self.collect_expr(body, state),
            Expr::Block { statements, value }
            | Expr::ComputationBlock {
                statements, value, ..
            }
            | Expr::NamedBlock {
                statements, value, ..
            } => {
                let mut block_state = state.clone();
                self.collect_stmts(statements, &mut block_state);
                if let Some(value) = value {
                    self.collect_expr(value, &mut block_state);
                }
                state.live_must_drop.extend(block_state.live_must_drop);
            }
            Expr::MemoBlock {
                options,
                statements,
                value,
            } => {
                for (_, value) in options {
                    self.collect_expr(value, state);
                }
                let mut block_state = state.clone();
                self.collect_stmts(statements, &mut block_state);
                if let Some(value) = value {
                    self.collect_expr(value, &mut block_state);
                }
                state.live_must_drop.extend(block_state.live_must_drop);
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_expr(condition, state);
                let mut then_state = state.clone();
                self.collect_expr(then_branch, &mut then_state);
                state.live_must_drop.extend(then_state.live_must_drop);
                if let Some(else_branch) = else_branch {
                    let mut else_state = state.clone();
                    self.collect_expr(else_branch, &mut else_state);
                    state.live_must_drop.extend(else_state.live_must_drop);
                }
            }
            Expr::IfLet {
                expr,
                guard,
                then_branch,
                else_branch,
                ..
            } => {
                self.collect_expr(expr, state);
                if let Some(guard) = guard {
                    self.collect_expr(guard, state);
                }
                let mut then_state = state.clone();
                self.collect_expr(then_branch, &mut then_state);
                state.live_must_drop.extend(then_state.live_must_drop);
                if let Some(else_branch) = else_branch {
                    let mut else_state = state.clone();
                    self.collect_expr(else_branch, &mut else_state);
                    state.live_must_drop.extend(else_state.live_must_drop);
                }
            }
            Expr::Match { scrutinee, arms } => {
                self.collect_expr(scrutinee, state);
                for arm in arms {
                    let mut arm_state = state.clone();
                    if let Some(guard) = arm.guard() {
                        self.collect_expr(guard, &mut arm_state);
                    }
                    self.collect_expr(arm.value(), &mut arm_state);
                    state.live_must_drop.extend(arm_state.live_must_drop);
                }
            }
        }
    }

    fn collect_call(&mut self, callee: &Expr, args: &[Expr], state: &mut FlowState) {
        if let Some(capability) = write_capability_for_call(callee) {
            self.add_effect_capability_obligation(&capability);
        }
        if let Expr::Path(name) = callee {
            match name.as_str() {
                "promote" => self.add_promote_obligation(args, false),
                "promote_unchecked" => self.add_promote_obligation(args, true),
                "drop" | "drop_optional" | "on_drop" => {
                    Self::collect_drop_args(args, state);
                    self.collect_expr(callee, state);
                    return;
                }
                "assume" => self.add_assume_obligation(args),
                _ => {}
            }
        }
        self.collect_expr(callee, state);
        for arg in args {
            self.collect_expr(arg, state);
        }
    }

    fn collect_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
        state: &mut FlowState,
    ) {
        match method {
            "set" => {
                if let Some(capability) = write_capability_for_method(receiver, method) {
                    self.add_effect_capability_obligation(&capability);
                }
            }
            "promote" => self.add_promote_obligation(args, false),
            "promote_unchecked" => self.add_promote_obligation(args, true),
            "drop" | "drop_optional" | "on_drop" => {
                if let Expr::LifetimePath { key, .. } = receiver {
                    state.remove_must_drop(key);
                    for arg in args {
                        self.collect_expr(arg, state);
                    }
                    return;
                }
            }
            _ => {}
        }
        self.collect_expr(receiver, state);
        for arg in args {
            self.collect_expr(arg, state);
        }
    }

    fn add_promote_obligation(&mut self, args: &[Expr], unchecked: bool) {
        let proof = proof_arg(args);
        let target = args.first().and_then(lifetime_label_arg);
        let discharge = if unchecked {
            self.unsafe_stack
                .last()
                .cloned()
                .map_or(SemanticDischarge::Missing, |id| {
                    SemanticDischarge::AuditedUnsafe { id }
                })
        } else if let Some(id) = proof {
            if self.proof_discharges_target(&id, target.as_deref()) {
                SemanticDischarge::FormalProof { id }
            } else {
                SemanticDischarge::Missing
            }
        } else {
            SemanticDischarge::Missing
        };
        let message = target.as_ref().map_or_else(
            || "lifetime promotion requires proof or audit".to_owned(),
            |target| format!("lifetime promotion to `{target}` requires proof or audit"),
        );
        self.add_obligation(
            SemanticObligationKind::LifetimePromotion,
            message,
            target,
            discharge,
        );
    }

    fn proof_discharges_target(&self, id: &str, target: Option<&str>) -> bool {
        let Some(proof) = self.known_proofs.get(id) else {
            return false;
        };
        target.is_some_and(|target| proof.discharges_target(target))
    }

    fn add_effect_capability_obligation(&mut self, capability: &Capability) {
        let discharge = if self.has_capability(capability) {
            SemanticDischarge::Automatic
        } else {
            SemanticDischarge::Missing
        };
        self.add_obligation(
            SemanticObligationKind::EffectCapability,
            format!("effect call requires capability `{}`", capability.as_str()),
            Some(capability.as_str().to_owned()),
            discharge,
        );
    }

    fn has_capability(&self, capability: &Capability) -> bool {
        self.env.has_capability(capability.as_str())
            || self
                .effect_stack
                .iter()
                .rev()
                .any(|scope| scope.contains(capability))
    }

    fn add_assume_obligation(&mut self, args: &[Expr]) {
        let discharge = axiom_arg(args)
            .filter(|id| self.known_axioms.contains(id))
            .map_or(SemanticDischarge::Missing, |id| {
                SemanticDischarge::TrustedAxiom { id }
            });
        self.add_obligation(
            SemanticObligationKind::TrustedAssumption,
            "assume requires a reason or trusted axiom".to_owned(),
            None,
            discharge,
        );
    }

    fn collect_thread(&mut self, thread: &ThreadBlock) {
        let body_flow = self.analyze_stmts(
            thread.body(),
            vec![FlowFacts::default()],
            ExitReason::Completed,
        );
        let output_types =
            thread_result_type_labels(thread.body(), !body_flow.fallthrough.is_empty());
        if output_types.len() > 1 {
            self.add_obligation(
                SemanticObligationKind::ThreadJoinTyping,
                "thread join result branches must produce one compatible type".to_owned(),
                thread.name().map(str::to_owned),
                SemanticDischarge::Missing,
            );
        }
        if thread.is_detached() || body_flow.has_touched_must_drop() {
            self.add_obligation(
                SemanticObligationKind::ThreadCapture,
                "thread capture must not move borrowed or MustDrop state across its scope"
                    .to_owned(),
                thread.name().map(str::to_owned),
                SemanticDischarge::Missing,
            );
        } else {
            self.add_obligation(
                SemanticObligationKind::ThreadCapture,
                "thread capture is scoped and joinable".to_owned(),
                thread.name().map(str::to_owned),
                SemanticDischarge::Automatic,
            );
        }
        self.finish_block_flow(body_flow, ExitReason::Completed);
    }

    fn check_sibling_thread_conflicts(
        &mut self,
        thread: &ThreadBlock,
        sibling_writes: &mut BTreeMap<ResourceAccess, String>,
    ) {
        for access in write_accesses_in_stmts(thread.body()) {
            if let Some(previous) = sibling_writes.insert(
                access.clone(),
                thread.name().unwrap_or("<anonymous>").to_owned(),
            ) {
                let current = thread.name().unwrap_or("<anonymous>");
                let key = access.key();
                self.add_obligation(
                    SemanticObligationKind::RuntimeConflict,
                    format!(
                        "concurrent write conflict on `{key}` between child tasks `{previous}` and `{current}`"
                    ),
                    Some(key),
                    SemanticDischarge::Missing,
                );
            }
        }
    }

    fn check_line_plan_child_conflicts(&mut self, items: &[LinePlanItem], include_all: bool) {
        let mut sibling_writes = BTreeMap::<ResourceAccess, String>::new();
        for (index, item) in items.iter().enumerate() {
            if !include_all && !is_line_plan_child_item(item) {
                continue;
            }
            let child_name = line_plan_child_name(item, index);
            for access in write_accesses_in_line_plan_item(item) {
                if let Some(previous) = sibling_writes.insert(access.clone(), child_name.clone()) {
                    let key = access.key();
                    self.add_obligation(
                        SemanticObligationKind::RuntimeConflict,
                        format!(
                            "concurrent write conflict on `{key}` between line child tasks `{previous}` and `{child_name}`"
                        ),
                        Some(key),
                        SemanticDischarge::Missing,
                    );
                }
            }
        }
    }

    fn inspect_cleanup_stmts(&mut self, statements: &[Stmt]) {
        let mut facts = FlowFacts::default();
        for statement in statements {
            self.collect_stmt(statement, &mut facts);
        }
    }

    fn inspect_cleanup_expr(&mut self, expr: &Expr) {
        let mut facts = FlowFacts::default();
        self.collect_expr(expr, &mut facts);
    }

    fn collect_unsafe_lifetime(
        &mut self,
        id: &IdRef,
        reason: Option<&Expr>,
        has_safety_doc: bool,
        body: &[Stmt],
    ) {
        let id = id_ref_label(id, "unsafe");
        let mut state = FlowState::default();
        if let Some(reason) = reason {
            self.collect_expr(reason, &mut state);
        }
        self.report.unsafe_audits.push(SemanticUnsafeAuditSummary {
            id: id.clone(),
            source: None,
            has_reason: reason.is_some(),
            has_safety_doc,
        });
        let has_reason = reason.is_some_and(is_non_empty_string_literal);
        let contains_unchecked = stmts_contain_unchecked_promotion(body);
        let discharge = if has_reason && has_safety_doc && contains_unchecked {
            SemanticDischarge::AuditedUnsafe { id: id.clone() }
        } else {
            SemanticDischarge::Missing
        };
        let message = if contains_unchecked {
            format!("unsafe lifetime audit `{id}` must include string reason and SAFETY docs")
        } else {
            format!(
                "unsafe lifetime audit `{id}` must contain the unchecked promotion it justifies"
            )
        };
        self.add_obligation(
            SemanticObligationKind::UnsafeLifetimeAudit,
            message,
            Some(id.clone()),
            discharge,
        );
        self.unsafe_stack.push(id);
        let flow = self.analyze_stmts(body, vec![state], ExitReason::Completed);
        self.unsafe_stack.pop();
        self.finish_block_flow(flow, ExitReason::Completed);
    }

    fn collect_lifetime_write(&mut self, target: &Expr) {
        if let Expr::LifetimePath { key, .. } = target
            && is_upper_lifetime(key.scope())
        {
            let capability = format!("state.write({})", key.scope().as_str());
            let capability = Capability::new(capability);
            let discharge = if self.has_capability(&capability) {
                SemanticDischarge::Automatic
            } else {
                SemanticDischarge::Missing
            };
            self.add_obligation(
                SemanticObligationKind::UpperLifetimeWrite,
                format!(
                    "upper lifetime write to `{}` needs effect capability or proof",
                    key.as_dotted()
                ),
                Some(key.as_dotted()),
                discharge,
            );
        }
    }

    fn collect_lifetime_drop_pipe(lhs: &Expr, rhs: &Expr, state: &mut FlowState) -> bool {
        let Expr::LifetimePath { key, .. } = lhs else {
            return false;
        };
        if is_drop_expr(rhs) {
            state.remove_must_drop(key);
            return true;
        }
        false
    }

    fn collect_drop_args(args: &[Expr], state: &mut FlowState) {
        for arg in args {
            if let Expr::LifetimePath { key, .. } = arg {
                state.remove_must_drop(key);
            }
        }
    }

    fn collect_wait(&mut self, target: &WaitTarget, state: &mut FlowState) {
        match target {
            WaitTarget::Duration(expr) | WaitTarget::Expr(expr) => self.collect_expr(expr, state),
            WaitTarget::Mark(_) => {}
        }
    }

    fn collect_trigger(&mut self, trigger: &TriggerPattern, state: &mut FlowState) {
        match trigger {
            TriggerPattern::Signal { target, value } => {
                self.collect_expr(target, state);
                if let Some(value) = value {
                    self.collect_pattern(value);
                }
            }
            TriggerPattern::Timeout(target) | TriggerPattern::Expr(target) => {
                self.collect_expr(target, state);
            }
            TriggerPattern::Input(pattern)
            | TriggerPattern::Event(pattern)
            | TriggerPattern::Mark(pattern)
            | TriggerPattern::Select(pattern)
            | TriggerPattern::Task(pattern)
            | TriggerPattern::Scope(pattern) => self.collect_pattern(pattern),
        }
    }

    fn collect_pattern(&mut self, pattern: &Pattern) {
        if let Pattern::Raw(raw) = pattern {
            self.add_raw_obligation(format!("raw pattern: {raw}"), None);
        }
    }

    fn collect_scope_expr(&mut self, scope: &HirScopeExpr) {
        let mut state = FlowState::default();
        self.collect_stmts(scope.statements(), &mut state);
        if let Some(value) = scope.value() {
            self.collect_expr(value, &mut state);
        }
        self.finish_scope(&state);
    }

    fn collect_scope_expr_syntax(&mut self, scope: &ScopeExprBlock) {
        let mut state = FlowState::default();
        self.collect_stmts(scope.statements(), &mut state);
        if let Some(value) = scope.value() {
            self.collect_expr(value, &mut state);
        }
        self.finish_scope(&state);
    }

    fn collect_loop(&mut self, block: &HirLoop) {
        let mut state = FlowState::default();
        self.collect_flow_items(block.body(), &mut state);
        self.finish_scope(&state);
    }

    fn collect_loop_syntax(&mut self, block: &LoopBlock) {
        let mut state = FlowState::default();
        for item in block.body() {
            self.collect_flow_item_syntax(item, &mut state);
        }
        self.finish_scope(&state);
    }

    fn collect_if(&mut self, block: &HirIf, state: &mut FlowState) {
        self.collect_expr(block.condition(), state);
        let mut body_state = state.clone();
        self.collect_flow_items(block.body(), &mut body_state);
        state.live_must_drop.extend(body_state.live_must_drop);
    }

    fn collect_if_let(&mut self, block: &HirIfLet, state: &mut FlowState) {
        self.collect_expr(block.expr(), state);
        if let Some(guard) = block.guard() {
            self.collect_expr(guard, state);
        }
        let mut body_state = state.clone();
        self.collect_flow_items(block.body(), &mut body_state);
        state.live_must_drop.extend(body_state.live_must_drop);
    }

    fn collect_match(&mut self, block: &HirMatch, state: &mut FlowState) {
        self.collect_expr(block.expr(), state);
        for arm in block.arms() {
            let mut arm_state = state.clone();
            if let Some(guard) = arm.guard() {
                self.collect_expr(guard, &mut arm_state);
            }
            self.collect_flow_items(arm.body(), &mut arm_state);
            state.live_must_drop.extend(arm_state.live_must_drop);
        }
    }

    fn collect_for(&mut self, block: &HirFor, state: &mut FlowState) {
        self.collect_expr(block.source(), state);
        let mut body_state = state.clone();
        self.collect_flow_items(block.body(), &mut body_state);
        state.live_must_drop.extend(body_state.live_must_drop);
    }

    fn collect_while(&mut self, block: &HirWhile, state: &mut FlowState) {
        self.collect_expr(block.condition(), state);
        let mut body_state = state.clone();
        self.collect_flow_items(block.body(), &mut body_state);
        state.live_must_drop.extend(body_state.live_must_drop);
    }

    fn collect_while_let(&mut self, block: &HirWhileLet, state: &mut FlowState) {
        self.collect_expr(block.expr(), state);
        if let Some(guard) = block.guard() {
            self.collect_expr(guard, state);
        }
        let mut body_state = state.clone();
        self.collect_flow_items(block.body(), &mut body_state);
        state.live_must_drop.extend(body_state.live_must_drop);
    }

    fn collect_await(&mut self, await_with: &HirAwait) {
        let mut state = FlowState::default();
        self.collect_expr(await_with.expr(), &mut state);
        for branch in await_with.branches() {
            self.collect_flow_items(branch.body(), &mut state);
        }
        self.finish_scope(&state);
    }

    fn collect_await_syntax(&mut self, await_with: &AwaitWith) {
        let mut state = FlowState::default();
        self.collect_expr(await_with.expr(), &mut state);
        self.finish_scope(&state);
    }

    fn collect_select(&mut self, block: &HirSelect) {
        let mut state = FlowState::default();
        for branch in block.branches() {
            match branch.head() {
                SelectBranchHead::Bind { source, .. } => {
                    self.collect_expr(source, &mut state);
                }
                SelectBranchHead::Frame(pattern) | SelectBranchHead::Event(pattern) => {
                    self.collect_pattern(pattern);
                }
                SelectBranchHead::Raw(raw) => {
                    self.add_raw_obligation(format!("raw select branch head: {raw}"), None);
                }
            }
            self.collect_flow_items(branch.body(), &mut state);
        }
        self.finish_scope(&state);
    }

    fn collect_borrow(&mut self, block: &HirBorrow) {
        let mut state = FlowState::default();
        self.collect_expr(block.source(), &mut state);
        self.collect_flow_items(block.body(), &mut state);
        self.finish_scope(&state);
    }

    fn collect_scope(&mut self, block: &HirScope) {
        let mut state = FlowState::default();
        self.collect_flow_items(block.body(), &mut state);
        self.finish_scope(&state);
    }

    fn collect_flow_item_syntax(&mut self, item: &FlowItem, state: &mut FlowState) {
        match item {
            FlowItem::Stmt(stmt) => self.collect_stmt(stmt, state),
            FlowItem::Raw(raw) => {
                self.add_raw_obligation(
                    format!("raw {:?} recovery node: {}", raw.family(), raw.source()),
                    raw.range().map(|range| format!("{range:?}")),
                );
            }
            _ => {}
        }
    }

    fn finish_scope(&mut self, state: &FlowState) {
        self.finish_facts(state, ExitReason::Completed);
    }

    fn finish_block_flow(&mut self, flow: BlockFlow, fallthrough_reason: ExitReason) {
        for facts in flow.fallthrough {
            self.finish_facts(&facts, fallthrough_reason);
        }
        for exit in flow.exits {
            self.finish_facts(&exit.facts, exit.reason);
        }
    }

    fn finish_facts(&mut self, facts: &FlowFacts, reason: ExitReason) {
        for key in facts.live_after_cleanup(reason) {
            let label = key.as_dotted();
            if !self.reported_must_drop.insert(label.clone()) {
                continue;
            }
            self.add_obligation(
                SemanticObligationKind::MustDropDischarge,
                format!(
                    "MustDrop lifetime value `{label}` must be explicitly dropped or transferred"
                ),
                Some(label),
                SemanticDischarge::Missing,
            );
        }
    }

    fn add_raw_obligation(&mut self, message: String, subject: Option<String>) {
        self.add_obligation(
            SemanticObligationKind::RawSyntax,
            message,
            subject,
            SemanticDischarge::Missing,
        );
    }

    fn add_obligation(
        &mut self,
        kind: SemanticObligationKind,
        message: String,
        subject: Option<String>,
        discharge: SemanticDischarge,
    ) {
        self.next_obligation += 1;
        let id = format!("semantic.obligation.{:04}", self.next_obligation);
        let severity = self.severity_for(kind, &discharge);
        let emit_diagnostic =
            severity != SemanticSeverity::Info || discharge == SemanticDischarge::Missing;
        self.report.obligations.push(SemanticObligation {
            id: id.clone(),
            kind,
            message: message.clone(),
            subject: subject.clone(),
            source: None,
            discharge,
        });
        if emit_diagnostic {
            self.report.diagnostics.push(SemanticDiagnostic {
                id: format!("semantic.diagnostic.{:04}", self.next_obligation),
                severity,
                message,
                source: None,
                obligation: Some(id),
                related_ids: subject.into_iter().collect(),
            });
        }
    }

    fn severity_for(
        &self,
        kind: SemanticObligationKind,
        discharge: &SemanticDischarge,
    ) -> SemanticSeverity {
        if matches!(
            discharge,
            SemanticDischarge::Automatic | SemanticDischarge::FormalProof { .. }
        ) {
            return SemanticSeverity::Info;
        }
        if matches!(
            kind,
            SemanticObligationKind::RawSyntax | SemanticObligationKind::RuntimeConflict
        ) {
            return SemanticSeverity::Error;
        }
        match self.policy.mode {
            SemanticMode::Dev => SemanticSeverity::Warning,
            SemanticMode::Test
                if matches!(
                    (kind, discharge),
                    (
                        SemanticObligationKind::UnsafeLifetimeAudit,
                        SemanticDischarge::AuditedUnsafe { .. }
                    ) | (_, SemanticDischarge::TrustedAxiom { .. })
                ) =>
            {
                SemanticSeverity::Warning
            }
            SemanticMode::Test | SemanticMode::Release => SemanticSeverity::Error,
        }
    }

    fn finish(mut self) -> SemanticReport {
        self.report
            .diagnostics
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.report
    }
}

fn span_from_range(range: &TextRange) -> SemanticSourceSpan {
    SemanticSourceSpan {
        start: range.start(),
        end: range.end(),
    }
}

fn push_unique_facts(facts: &mut Vec<FlowFacts>, value: FlowFacts) {
    if !facts.contains(&value) {
        facts.push(value);
    }
}

fn push_unique_exit(exits: &mut Vec<ExitPath>, value: ExitPath) {
    if !exits.contains(&value) {
        exits.push(value);
    }
}

fn id_ref_label(id: &IdRef, default_family: &str) -> String {
    match id {
        IdRef::Absolute(entity) => entity.body().to_owned(),
        IdRef::Relative(relative) => format!("{default_family}.{}", relative.suffix()),
        IdRef::FamilyRelative(relative) => {
            format!("{}.{}", relative.family(), relative.relative().suffix())
        }
    }
}

fn proof_arg(args: &[Expr]) -> Option<String> {
    named_entity_arg(args, "proof")
}

fn axiom_arg(args: &[Expr]) -> Option<String> {
    named_entity_arg(args, "axiom").or_else(|| named_entity_arg(args, "trusted_axiom"))
}

fn named_entity_arg(args: &[Expr], name: &str) -> Option<String> {
    args.iter().find_map(|arg| match arg {
        Expr::NamedArg {
            name: arg_name,
            value,
        } if arg_name == name => match value.as_ref() {
            Expr::EntityRef(entity) => entity_label(entity),
            Expr::Path(path) => Some(path.clone()),
            _ => None,
        },
        _ => None,
    })
}

fn entity_label(entity: &EntityRefSyntax) -> Option<String> {
    entity
        .as_absolute()
        .map(|absolute| absolute.body().to_owned())
        .or_else(|| Some(entity.body().to_owned()))
}

fn lifetime_label_arg(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) if path.starts_with('\'') => Some(path.clone()),
        Expr::LifetimePath { key, .. } => Some(format!("'{}", key.as_dotted())),
        _ => None,
    }
}

fn is_non_empty_string_literal(expr: &Expr) -> bool {
    matches!(expr, Expr::Literal(Literal::String(value)) if !value.trim().is_empty())
}

fn is_upper_lifetime(scope: &LifetimeScopeKind) -> bool {
    matches!(
        scope,
        LifetimeScopeKind::Flow
            | LifetimeScopeKind::Session
            | LifetimeScopeKind::Global
            | LifetimeScopeKind::Persistent
    )
}

fn stmts_contain_unchecked_promotion(stmts: &[Stmt]) -> bool {
    stmts.iter().any(stmt_contains_unchecked_promotion)
}

fn stmt_contains_unchecked_promotion(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Let { expr, .. }
        | Stmt::LetElse { expr, .. }
        | Stmt::Return(expr)
        | Stmt::Out { expr, .. }
        | Stmt::Goto(expr)
        | Stmt::Defer { expr, .. }
        | Stmt::Yield(expr)
        | Stmt::Panic(expr)
        | Stmt::Fail(expr)
        | Stmt::Bail(expr)
        | Stmt::Expr(expr)
        | Stmt::Close(expr)
        | Stmt::Select(expr)
        | Stmt::Break {
            expr: Some(expr), ..
        }
        | Stmt::Wait(WaitTarget::Duration(expr) | WaitTarget::Expr(expr)) => {
            expr_contains_unchecked_promotion(expr)
        }
        Stmt::Ensure { condition, message } => {
            expr_contains_unchecked_promotion(condition)
                || expr_contains_unchecked_promotion(message)
        }
        Stmt::Signal { target, value }
        | Stmt::LifetimeSet {
            target,
            expr: value,
        } => expr_contains_unchecked_promotion(target) || expr_contains_unchecked_promotion(value),
        Stmt::LetChoice { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::Wait(WaitTarget::Mark(_))
        | Stmt::Command(_)
        | Stmt::Break { expr: None, .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => false,
        Stmt::Thread(thread) => stmts_contain_unchecked_promotion(thread.body()),
        Stmt::DeferBlock { statements, .. }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
            body: statements, ..
        }
        | Stmt::If {
            body: statements, ..
        }
        | Stmt::Loop { body: statements }
        | Stmt::While {
            body: statements, ..
        }
        | Stmt::WhileLet {
            body: statements, ..
        }
        | Stmt::For {
            body: statements, ..
        } => stmts_contain_unchecked_promotion(statements),
        Stmt::Match { arms, .. } => arms
            .iter()
            .any(|arm| stmts_contain_unchecked_promotion(arm.body())),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "expression traversal mirrors Expr so unsafe audit coverage stays auditable"
)]
fn expr_contains_unchecked_promotion(expr: &Expr) -> bool {
    match expr {
        Expr::Call { callee, args } => {
            matches!(callee.as_ref(), Expr::Path(path) if path == "promote_unchecked")
                || expr_contains_unchecked_promotion(callee)
                || args.iter().any(expr_contains_unchecked_promotion)
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
        } => {
            method == "promote_unchecked"
                || expr_contains_unchecked_promotion(receiver)
                || args.iter().any(expr_contains_unchecked_promotion)
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            items.iter().any(expr_contains_unchecked_promotion)
        }
        Expr::ArrayRepeat { value, len } => {
            expr_contains_unchecked_promotion(value) || expr_contains_unchecked_promotion(len)
        }
        Expr::NamedArg { value, .. }
        | Expr::Field { target: value, .. }
        | Expr::Try { expr: value }
        | Expr::Await { expr: value, .. }
        | Expr::Unary { expr: value, .. }
        | Expr::Closure { body: value, .. } => expr_contains_unchecked_promotion(value),
        Expr::Binary { lhs, rhs, .. }
        | Expr::Pipe { lhs, rhs }
        | Expr::Index {
            target: lhs,
            index: rhs,
        } => expr_contains_unchecked_promotion(lhs) || expr_contains_unchecked_promotion(rhs),
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => fields
            .iter()
            .any(|(_, value)| expr_contains_unchecked_promotion(value)),
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        }
        | Expr::MemoBlock {
            statements, value, ..
        } => {
            stmts_contain_unchecked_promotion(statements)
                || value
                    .as_ref()
                    .is_some_and(|value| expr_contains_unchecked_promotion(value))
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains_unchecked_promotion(condition)
                || expr_contains_unchecked_promotion(then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(|value| expr_contains_unchecked_promotion(value))
        }
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_unchecked_promotion(expr)
                || guard
                    .as_ref()
                    .is_some_and(|guard| expr_contains_unchecked_promotion(guard))
                || expr_contains_unchecked_promotion(then_branch)
                || else_branch
                    .as_ref()
                    .is_some_and(|value| expr_contains_unchecked_promotion(value))
        }
        Expr::Match { scrutinee, arms } => {
            expr_contains_unchecked_promotion(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard().is_some_and(expr_contains_unchecked_promotion)
                        || expr_contains_unchecked_promotion(arm.value())
                })
        }
        Expr::Thread { block } => stmts_contain_unchecked_promotion(block.body()),
        Expr::Range { start, end, .. } => {
            start
                .as_ref()
                .is_some_and(|value| expr_contains_unchecked_promotion(value))
                || end
                    .as_ref()
                    .is_some_and(|value| expr_contains_unchecked_promotion(value))
        }
        Expr::DialogueCall { callee, plan, .. } => {
            expr_contains_unchecked_promotion(callee)
                || plan.as_ref().is_some_and(|plan| {
                    plan.items()
                        .iter()
                        .any(line_plan_item_contains_unchecked_promotion)
                })
        }
        Expr::Literal(_)
        | Expr::Path(_)
        | Expr::Placeholder(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Raw(_) => false,
    }
}

fn line_plan_item_contains_unchecked_promotion(item: &LinePlanItem) -> bool {
    match item {
        LinePlanItem::Init(stmts) => stmts_contain_unchecked_promotion(stmts),
        LinePlanItem::Thread(thread) => stmts_contain_unchecked_promotion(thread.body()),
        LinePlanItem::On { body, .. } => stmts_contain_unchecked_promotion(body),
        LinePlanItem::Let { expr, .. }
        | LinePlanItem::Option { value: expr, .. }
        | LinePlanItem::Assert { expr, .. }
        | LinePlanItem::Expr(expr)
        | LinePlanItem::Out(expr) => expr_contains_unchecked_promotion(expr),
        LinePlanItem::Stmt(stmt) => stmt_contains_unchecked_promotion(stmt),
        LinePlanItem::TimedCue { anchor, body } => {
            expr_contains_unchecked_promotion(anchor) || expr_contains_unchecked_promotion(body)
        }
        LinePlanItem::CancelRule(rule) => stmts_contain_unchecked_promotion(rule.action()),
        LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => items
            .iter()
            .any(line_plan_item_contains_unchecked_promotion),
        LinePlanItem::Memo { options, .. } => options
            .iter()
            .any(|(_, value)| expr_contains_unchecked_promotion(value)),
        LinePlanItem::Raw(_) => false,
    }
}

fn is_must_drop_key(key: &LifetimeKey) -> bool {
    key.scope() == &LifetimeScopeKind::Line
        && key.path().first().is_some_and(|part| part == "focus")
}

fn is_drop_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop")
    ) || matches!(
        expr,
        Expr::Call { callee, .. }
            if matches!(callee.as_ref(), Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop"))
    )
}

fn write_accesses_in_stmts(stmts: &[Stmt]) -> BTreeSet<ResourceAccess> {
    let mut accesses = BTreeSet::new();
    for stmt in stmts {
        collect_stmt_write_accesses(stmt, &mut accesses);
    }
    accesses
}

fn write_accesses_in_line_plan_item(item: &LinePlanItem) -> BTreeSet<ResourceAccess> {
    let mut accesses = BTreeSet::new();
    collect_line_plan_item_write_accesses(item, &mut accesses);
    accesses
}

fn collect_line_plan_item_write_accesses(
    item: &LinePlanItem,
    accesses: &mut BTreeSet<ResourceAccess>,
) {
    match item {
        LinePlanItem::Init(stmts) => {
            for stmt in stmts {
                collect_stmt_write_accesses(stmt, accesses);
            }
        }
        LinePlanItem::Thread(thread) => {
            for stmt in thread.body() {
                collect_stmt_write_accesses(stmt, accesses);
            }
        }
        LinePlanItem::On { body, .. } => {
            for stmt in body {
                collect_stmt_write_accesses(stmt, accesses);
            }
        }
        LinePlanItem::Stmt(stmt) => collect_stmt_write_accesses(stmt, accesses),
        LinePlanItem::TimedCue { body, .. } | LinePlanItem::Expr(body) => {
            collect_expr_write_accesses(body, accesses);
        }
        LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
            for item in items {
                collect_line_plan_item_write_accesses(item, accesses);
            }
        }
        _ => {}
    }
}

fn line_plan_child_name(item: &LinePlanItem, index: usize) -> String {
    match item {
        LinePlanItem::Thread(thread) => thread
            .name()
            .map_or_else(|| format!("thread#{index}"), str::to_owned),
        LinePlanItem::On { .. } => format!("on#{index}"),
        LinePlanItem::TimedCue { .. } => format!("at#{index}"),
        LinePlanItem::StartGroup(_) => format!("start#{index}"),
        LinePlanItem::TogetherGroup(_) => format!("together#{index}"),
        _ => format!("item#{index}"),
    }
}

fn is_line_plan_child_item(item: &LinePlanItem) -> bool {
    matches!(
        item,
        LinePlanItem::Thread(_)
            | LinePlanItem::On { .. }
            | LinePlanItem::TimedCue { .. }
            | LinePlanItem::StartGroup(_)
            | LinePlanItem::TogetherGroup(_)
    )
}

fn drop_keys_in_stmts(stmts: &[Stmt]) -> HashSet<LifetimeKey> {
    let mut keys = HashSet::new();
    for stmt in stmts {
        collect_stmt_drop_keys(stmt, &mut keys);
    }
    keys
}

fn collect_stmt_drop_keys(stmt: &Stmt, keys: &mut HashSet<LifetimeKey>) {
    match stmt {
        Stmt::Expr(expr) | Stmt::Defer { expr, .. } => collect_expr_drop_keys(expr, keys),
        Stmt::DeferBlock { statements, .. }
        | Stmt::If {
            body: statements, ..
        }
        | Stmt::Loop { body: statements }
        | Stmt::While {
            body: statements, ..
        }
        | Stmt::WhileLet {
            body: statements, ..
        }
        | Stmt::For {
            body: statements, ..
        }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
            body: statements, ..
        } => {
            for stmt in statements {
                collect_stmt_drop_keys(stmt, keys);
            }
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                for stmt in arm.body() {
                    collect_stmt_drop_keys(stmt, keys);
                }
            }
        }
        _ => {}
    }
}

fn drop_keys_in_expr(expr: &Expr) -> HashSet<LifetimeKey> {
    let mut keys = HashSet::new();
    collect_expr_drop_keys(expr, &mut keys);
    keys
}

fn collect_expr_drop_keys(expr: &Expr, keys: &mut HashSet<LifetimeKey>) {
    match expr {
        Expr::Pipe { lhs, rhs } if is_drop_expr(rhs) => {
            if let Expr::LifetimePath { key, .. } = lhs.as_ref() {
                keys.insert(key.clone());
            }
            collect_expr_drop_keys(rhs, keys);
        }
        Expr::Call { callee, args } if matches!(callee.as_ref(), Expr::Path(path) if matches!(path.as_str(), "drop" | "drop_optional" | "on_drop")) => {
            for arg in args {
                if let Expr::LifetimePath { key, .. } = arg {
                    keys.insert(key.clone());
                }
            }
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if matches!(method.as_str(), "drop" | "drop_optional" | "on_drop") => {
            if let Expr::LifetimePath { key, .. } = receiver.as_ref() {
                keys.insert(key.clone());
            }
            for arg in args {
                collect_expr_drop_keys(arg, keys);
            }
        }
        Expr::Call { callee, args } => {
            collect_expr_drop_keys(callee, keys);
            for arg in args {
                collect_expr_drop_keys(arg, keys);
            }
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            for item in items {
                collect_expr_drop_keys(item, keys);
            }
        }
        Expr::NamedArg { value, .. }
        | Expr::Field { target: value, .. }
        | Expr::Try { expr: value }
        | Expr::Await { expr: value, .. }
        | Expr::Unary { expr: value, .. } => collect_expr_drop_keys(value, keys),
        Expr::MethodCall { receiver, args, .. } => {
            collect_expr_drop_keys(receiver, keys);
            for arg in args {
                collect_expr_drop_keys(arg, keys);
            }
        }
        Expr::Binary { lhs, rhs, .. }
        | Expr::Pipe { lhs, rhs }
        | Expr::Index {
            target: lhs,
            index: rhs,
        } => {
            collect_expr_drop_keys(lhs, keys);
            collect_expr_drop_keys(rhs, keys);
        }
        _ => {}
    }
}

fn collect_stmt_write_accesses(stmt: &Stmt, accesses: &mut BTreeSet<ResourceAccess>) {
    match stmt {
        Stmt::LifetimeSet {
            target: Expr::LifetimePath { key, .. },
            ..
        } => {
            accesses.insert(resource_write_for_lifetime(key.as_dotted()));
        }
        Stmt::Signal { target, .. } => {
            accesses.insert(resource_write_for_signal(expr_label(target)));
        }
        Stmt::Expr(expr) | Stmt::Defer { expr, .. } => {
            collect_expr_write_accesses(expr, accesses);
        }
        Stmt::DeferBlock { statements, .. } => {
            for stmt in statements {
                collect_stmt_write_accesses(stmt, accesses);
            }
        }
        Stmt::On { .. } | Stmt::Thread(_) => {
            let body = match stmt {
                Stmt::On { body, .. } => body.as_slice(),
                Stmt::Thread(thread) => thread.body(),
                _ => &[],
            };
            for stmt in body {
                collect_stmt_write_accesses(stmt, accesses);
            }
        }
        _ => {}
    }
}

fn collect_expr_write_accesses(expr: &Expr, accesses: &mut BTreeSet<ResourceAccess>) {
    accesses.extend(resource_accesses_from_expr(expr));
    match expr {
        Expr::Call { args, .. } | Expr::Tuple(args) | Expr::BracketSeq(args) => {
            for arg in args {
                collect_expr_write_accesses(arg, accesses);
            }
        }
        Expr::ArrayRepeat { value, len } => {
            collect_expr_write_accesses(value, accesses);
            collect_expr_write_accesses(len, accesses);
        }
        Expr::NamedArg { value, .. }
        | Expr::Field { target: value, .. }
        | Expr::Try { expr: value }
        | Expr::Await { expr: value, .. }
        | Expr::Unary { expr: value, .. } => collect_expr_write_accesses(value, accesses),
        Expr::MethodCall { receiver, args, .. } => {
            collect_expr_write_accesses(receiver, accesses);
            for arg in args {
                collect_expr_write_accesses(arg, accesses);
            }
        }
        Expr::Binary { lhs, rhs, .. }
        | Expr::Pipe { lhs, rhs }
        | Expr::Index {
            target: lhs,
            index: rhs,
        } => {
            collect_expr_write_accesses(lhs, accesses);
            collect_expr_write_accesses(rhs, accesses);
        }
        _ => {}
    }
}

fn thread_result_type_labels(stmts: &[Stmt], can_fallthrough: bool) -> BTreeSet<String> {
    let mut labels = BTreeSet::new();
    for stmt in stmts {
        collect_thread_result_type_labels(stmt, &mut labels);
    }
    if can_fallthrough {
        labels.insert("Unit".to_owned());
    }
    if labels.len() > 1 {
        labels.remove("Unknown");
    }
    labels
}

fn collect_thread_result_type_labels(stmt: &Stmt, labels: &mut BTreeSet<String>) {
    match stmt {
        Stmt::Out { expr, .. } => {
            labels.insert(expr_type_label(expr));
        }
        Stmt::If { .. }
        | Stmt::Loop { .. }
        | Stmt::While { .. }
        | Stmt::WhileLet { .. }
        | Stmt::For { .. } => {
            let body = match stmt {
                Stmt::If { body, .. }
                | Stmt::Loop { body }
                | Stmt::While { body, .. }
                | Stmt::WhileLet { body, .. }
                | Stmt::For { body, .. } => body.as_slice(),
                _ => &[],
            };
            for stmt in body {
                collect_thread_result_type_labels(stmt, labels);
            }
        }
        Stmt::Match { arms, .. } => {
            for arm in arms {
                for stmt in arm.body() {
                    collect_thread_result_type_labels(stmt, labels);
                }
            }
        }
        Stmt::LetElse { else_body, .. } => {
            for stmt in else_body {
                collect_thread_result_type_labels(stmt, labels);
            }
        }
        _ => {}
    }
}

fn expr_type_label(expr: &Expr) -> String {
    match expr {
        Expr::Literal(Literal::String(_)) => "String".to_owned(),
        Expr::Literal(Literal::Char { .. }) => "Char".to_owned(),
        Expr::Literal(Literal::Int(_)) => "Int".to_owned(),
        Expr::Literal(Literal::Float(_)) => "Float".to_owned(),
        Expr::Literal(Literal::Bool(_)) => "Bool".to_owned(),
        Expr::Literal(Literal::Duration { .. }) => "Duration".to_owned(),
        Expr::Tuple(items) => {
            let labels = items.iter().map(expr_type_label).collect::<Vec<_>>();
            format!("({})", labels.join(", "))
        }
        Expr::BracketSeq(items) => items.first().map_or_else(
            || "Vec<Unknown>".to_owned(),
            |item| format!("Vec<{}>", expr_type_label(item)),
        ),
        Expr::ArrayRepeat { value, len } => {
            format!("Array<{}, {}>", expr_type_label(value), expr_label(len))
        }
        Expr::EntityRef(_) => "EntityRef".to_owned(),
        _ => "Unknown".to_owned(),
    }
}

fn expr_label(expr: &Expr) -> String {
    match expr {
        Expr::Path(path) => path.clone(),
        Expr::EntityRef(entity) => entity.body().to_owned(),
        Expr::LifetimePath { key, .. } => key.as_dotted(),
        Expr::Literal(literal) => format!("{literal:?}"),
        _ => format!("{expr:?}"),
    }
}
