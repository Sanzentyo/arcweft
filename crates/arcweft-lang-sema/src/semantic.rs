use crate::check::TypeCheckEnv;
use arcweft_lang_hir::{
    ChoicePlanItem, Expr, HirAwait, HirBorrow, HirChoice, HirFlowItem, HirFor, HirFunction, HirIf,
    HirIfLet, HirLoop, HirMatch, HirModule, HirScope, HirScopeExpr, HirSelect, HirTopLevelDecl,
    HirWhile, HirWhileLet, IdRef, LifetimeKey, LifetimeScopeKind, LinePlan, LinePlanItem, Pattern,
    Stmt, TextRange, ThreadBlock, TriggerPattern,
};
use arcweft_lang_syntax::{Literal, LoopBlock, SelectBranchHead};
use std::collections::{BTreeMap, BTreeSet, HashSet};

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

#[derive(Clone, Debug, Default)]
struct FlowState {
    live_must_drop: HashSet<LifetimeKey>,
}

struct SemanticAnalyzer<'a> {
    env: &'a TypeCheckEnv,
    policy: SemanticPolicy,
    report: SemanticReport,
    next_obligation: usize,
    unsafe_stack: Vec<String>,
    known_proofs: BTreeSet<String>,
    known_axioms: BTreeSet<String>,
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
            known_proofs: BTreeSet::new(),
            known_axioms: BTreeSet::new(),
        }
    }

    fn collect_module(&mut self, module: &HirModule) {
        self.collect_declarations(module);
        for flow in module.flows() {
            let mut state = FlowState::default();
            self.collect_flow_items(flow.body(), &mut state);
            self.finish_scope(state);
        }
        for function in module.functions() {
            self.collect_function(function);
        }
        let mut top_level = FlowState::default();
        self.collect_flow_items(module.top_level_items(), &mut top_level);
        self.finish_scope(top_level);
    }

    fn collect_declarations(&mut self, module: &HirModule) {
        for declaration in module.declarations() {
            match declaration {
                HirTopLevelDecl::Proof(proof) => {
                    let id = id_ref_label(proof.id(), "proof");
                    self.known_proofs.insert(id.clone());
                    self.report.proofs.push(SemanticProofSummary {
                        id,
                        source: span_from_range(proof.range()),
                    });
                }
                HirTopLevelDecl::TrustedAxiom(axiom) => {
                    let id = id_ref_label(axiom.id(), "axiom");
                    self.known_axioms.insert(id.clone());
                    self.report
                        .trusted_axioms
                        .push(SemanticTrustedAxiomSummary {
                            id,
                            source: span_from_range(axiom.range()),
                        });
                }
                HirTopLevelDecl::Hook(hook) => self.collect_stmt_list(hook.body_statements()),
                HirTopLevelDecl::MemoFn(item) => self.collect_stmt_list(item.body_statements()),
                HirTopLevelDecl::Parser(item) => self.collect_stmt_list(item.body_statements()),
                HirTopLevelDecl::Source(item) => self.collect_stmt_list(item.body_statements()),
                _ => {}
            }
        }
    }

    fn collect_function(&mut self, function: &HirFunction) {
        self.collect_stmt_list(function.statements());
        if let Some(value) = function.value() {
            let mut state = FlowState::default();
            self.collect_expr(value, &mut state);
            self.finish_scope(state);
        }
    }

    fn collect_flow_items(&mut self, items: &[HirFlowItem], state: &mut FlowState) {
        let mut sibling_writes = BTreeMap::<String, String>::new();
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
        let mut state = FlowState::default();
        self.collect_stmts(stmts, &mut state);
        self.finish_scope(state);
    }

    fn collect_stmts(&mut self, stmts: &[Stmt], state: &mut FlowState) {
        let mut sibling_writes = BTreeMap::<String, String>::new();
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
            | Stmt::Out { expr, .. } => self.collect_expr(expr, state),
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
            Stmt::Break { expr, .. } => {
                if let Some(expr) = expr {
                    self.collect_expr(expr, state);
                }
            }
            Stmt::Continue { .. } => {}
            Stmt::Raw(raw) => self.add_raw_obligation(format!("raw statement: {raw}"), None),
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
        self.finish_scope(state);
    }

    fn collect_choice_syntax(&mut self, choice: &arcweft_lang_hir::ChoiceBlock) {
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
                self.add_raw_obligation(format!("raw choice plan item: {raw}"), None);
            }
        }
        self.finish_scope(state);
    }

    fn collect_line_plan(&mut self, plan: &LinePlan) {
        for item in plan.items() {
            self.collect_line_plan_item(item);
        }
    }

    fn collect_line_plan_item(&mut self, item: &LinePlanItem) {
        let mut state = FlowState::default();
        match item {
            LinePlanItem::Init(stmts) => self.collect_stmts(stmts, &mut state),
            LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
                for item in items {
                    self.collect_line_plan_item(item);
                }
            }
            LinePlanItem::Thread(thread) => self.collect_thread(thread),
            LinePlanItem::On { trigger, body } => {
                self.collect_trigger(trigger, &mut state);
                self.collect_stmts(body, &mut state);
            }
            LinePlanItem::Option { value, .. }
            | LinePlanItem::Let { expr: value, .. }
            | LinePlanItem::Out(value)
            | LinePlanItem::Assert { expr: value, .. }
            | LinePlanItem::Expr(value) => self.collect_expr(value, &mut state),
            LinePlanItem::Stmt(stmt) => self.collect_stmt(stmt, &mut state),
            LinePlanItem::CancelRule(rule) => self.collect_stmts(rule.action(), &mut state),
            LinePlanItem::TimedCue { anchor, body } => {
                self.collect_expr(anchor, &mut state);
                self.collect_expr(body, &mut state);
            }
            LinePlanItem::Memo { options, .. } => {
                for (_, value) in options {
                    self.collect_expr(value, &mut state);
                }
            }
            LinePlanItem::Raw(raw) => {
                self.add_raw_obligation(format!("raw line plan item: {raw}"), None);
            }
        }
        self.finish_scope(state);
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
                    state.live_must_drop.insert(key.clone());
                }
            }
            Expr::Tuple(items) | Expr::List(items) => {
                for item in items {
                    self.collect_expr(item, state);
                }
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
            "promote" => self.add_promote_obligation(args, false),
            "promote_unchecked" => self.add_promote_obligation(args, true),
            "drop" | "drop_optional" | "on_drop" => {
                if let Expr::LifetimePath { key, .. } = receiver {
                    state.live_must_drop.remove(key);
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
            if self.known_proofs.contains(&id) {
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
        let mut body_state = FlowState::default();
        self.collect_stmts(thread.body(), &mut body_state);
        let output_types = out_type_labels(thread.body());
        if output_types.len() > 1 {
            self.add_obligation(
                SemanticObligationKind::ThreadJoinTyping,
                "thread join result branches must produce one compatible type".to_owned(),
                thread.name().map(str::to_owned),
                SemanticDischarge::Missing,
            );
        }
        if thread.is_detached() || !body_state.live_must_drop.is_empty() {
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
        self.finish_scope(body_state);
    }

    fn check_sibling_thread_conflicts(
        &mut self,
        thread: &ThreadBlock,
        sibling_writes: &mut BTreeMap<String, String>,
    ) {
        for key in write_keys_in_stmts(thread.body()) {
            if let Some(previous) = sibling_writes.insert(
                key.clone(),
                thread.name().unwrap_or("<anonymous>").to_owned(),
            ) {
                let current = thread.name().unwrap_or("<anonymous>");
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
        let discharge = if reason.is_some() && has_safety_doc {
            SemanticDischarge::AuditedUnsafe { id: id.clone() }
        } else {
            SemanticDischarge::Missing
        };
        self.add_obligation(
            SemanticObligationKind::UnsafeLifetimeAudit,
            format!("unsafe lifetime audit `{id}` must include reason and SAFETY docs"),
            Some(id.clone()),
            discharge,
        );
        self.unsafe_stack.push(id);
        self.collect_stmts(body, &mut state);
        self.unsafe_stack.pop();
        self.finish_scope(state);
    }

    fn collect_lifetime_write(&mut self, target: &Expr) {
        if let Expr::LifetimePath { key, .. } = target
            && is_upper_lifetime(key.scope())
        {
            let capability = format!("state.write({})", key.scope().as_str());
            let discharge = if self.env.has_capability(&capability) {
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
            state.live_must_drop.remove(key);
            return true;
        }
        false
    }

    fn collect_drop_args(args: &[Expr], state: &mut FlowState) {
        for arg in args {
            if let Expr::LifetimePath { key, .. } = arg {
                state.live_must_drop.remove(key);
            }
        }
    }

    fn collect_wait(&mut self, target: &arcweft_lang_hir::WaitTarget, state: &mut FlowState) {
        match target {
            arcweft_lang_hir::WaitTarget::Duration(expr)
            | arcweft_lang_hir::WaitTarget::Expr(expr) => self.collect_expr(expr, state),
            arcweft_lang_hir::WaitTarget::Mark(_) => {}
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
        self.finish_scope(state);
    }

    fn collect_scope_expr_syntax(&mut self, scope: &arcweft_lang_hir::ScopeExprBlock) {
        let mut state = FlowState::default();
        self.collect_stmts(scope.statements(), &mut state);
        if let Some(value) = scope.value() {
            self.collect_expr(value, &mut state);
        }
        self.finish_scope(state);
    }

    fn collect_loop(&mut self, block: &HirLoop) {
        let mut state = FlowState::default();
        self.collect_flow_items(block.body(), &mut state);
        self.finish_scope(state);
    }

    fn collect_loop_syntax(&mut self, block: &LoopBlock) {
        let mut state = FlowState::default();
        for item in block.body() {
            self.collect_flow_item_syntax(item, &mut state);
        }
        self.finish_scope(state);
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
        self.finish_scope(state);
    }

    fn collect_await_syntax(&mut self, await_with: &arcweft_lang_hir::AwaitWith) {
        let mut state = FlowState::default();
        self.collect_expr(await_with.expr(), &mut state);
        self.finish_scope(state);
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
        self.finish_scope(state);
    }

    fn collect_borrow(&mut self, block: &HirBorrow) {
        let mut state = FlowState::default();
        self.collect_expr(block.source(), &mut state);
        self.collect_flow_items(block.body(), &mut state);
        self.finish_scope(state);
    }

    fn collect_scope(&mut self, block: &HirScope) {
        let mut state = FlowState::default();
        self.collect_flow_items(block.body(), &mut state);
        self.finish_scope(state);
    }

    fn collect_flow_item_syntax(
        &mut self,
        item: &arcweft_lang_hir::FlowItem,
        state: &mut FlowState,
    ) {
        match item {
            arcweft_lang_hir::FlowItem::Stmt(stmt) => self.collect_stmt(stmt, state),
            arcweft_lang_hir::FlowItem::Raw(raw) => {
                self.add_raw_obligation(format!("raw flow item: {raw}"), None);
            }
            _ => {}
        }
    }

    fn finish_scope(&mut self, state: FlowState) {
        for key in state.live_must_drop {
            self.add_obligation(
                SemanticObligationKind::MustDropDischarge,
                format!(
                    "MustDrop lifetime value `{}` must be explicitly dropped or transferred",
                    key.as_dotted()
                ),
                Some(key.as_dotted()),
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

fn entity_label(entity: &arcweft_lang_hir::EntityRefSyntax) -> Option<String> {
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

fn is_upper_lifetime(scope: &LifetimeScopeKind) -> bool {
    matches!(
        scope,
        LifetimeScopeKind::Flow
            | LifetimeScopeKind::Session
            | LifetimeScopeKind::Global
            | LifetimeScopeKind::Persistent
    )
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

fn write_keys_in_stmts(stmts: &[Stmt]) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for stmt in stmts {
        collect_stmt_write_keys(stmt, &mut keys);
    }
    keys
}

fn collect_stmt_write_keys(stmt: &Stmt, keys: &mut BTreeSet<String>) {
    match stmt {
        Stmt::LifetimeSet {
            target: Expr::LifetimePath { key, .. },
            ..
        } => {
            keys.insert(format!("lifetime:{}", key.as_dotted()));
        }
        Stmt::Signal { target, .. } => {
            keys.insert(format!("signal:{}", expr_label(target)));
        }
        Stmt::Expr(expr) | Stmt::Defer { expr, .. } => collect_expr_write_keys(expr, keys),
        Stmt::DeferBlock { statements, .. } => {
            for stmt in statements {
                collect_stmt_write_keys(stmt, keys);
            }
        }
        Stmt::On { .. } | Stmt::Thread(_) => {
            let body = match stmt {
                Stmt::On { body, .. } => body.as_slice(),
                Stmt::Thread(thread) => thread.body(),
                _ => &[],
            };
            for stmt in body {
                collect_stmt_write_keys(stmt, keys);
            }
        }
        _ => {}
    }
}

fn collect_expr_write_keys(expr: &Expr, keys: &mut BTreeSet<String>) {
    match expr {
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if method == "set"
            && matches!(receiver.as_ref(), Expr::Path(path) if matches!(path.as_str(), "signal" | "metric")) =>
        {
            if let Some(target) = args.first() {
                keys.insert(format!("{}:{}", expr_label(receiver), expr_label(target)));
            }
        }
        Expr::Call { args, .. } | Expr::Tuple(args) | Expr::List(args) => {
            for arg in args {
                collect_expr_write_keys(arg, keys);
            }
        }
        Expr::NamedArg { value, .. }
        | Expr::Field { target: value, .. }
        | Expr::Try { expr: value }
        | Expr::Await { expr: value, .. }
        | Expr::Unary { expr: value, .. } => collect_expr_write_keys(value, keys),
        Expr::MethodCall { receiver, args, .. } => {
            collect_expr_write_keys(receiver, keys);
            for arg in args {
                collect_expr_write_keys(arg, keys);
            }
        }
        Expr::Binary { lhs, rhs, .. }
        | Expr::Pipe { lhs, rhs }
        | Expr::Index {
            target: lhs,
            index: rhs,
        } => {
            collect_expr_write_keys(lhs, keys);
            collect_expr_write_keys(rhs, keys);
        }
        _ => {}
    }
}

fn out_type_labels(stmts: &[Stmt]) -> BTreeSet<&'static str> {
    let mut labels = BTreeSet::new();
    for stmt in stmts {
        collect_out_type_labels(stmt, &mut labels);
    }
    labels
}

fn collect_out_type_labels(stmt: &Stmt, labels: &mut BTreeSet<&'static str>) {
    match stmt {
        Stmt::Out { expr, .. } => {
            labels.insert(expr_type_label(expr));
        }
        Stmt::If { .. }
        | Stmt::Loop { .. }
        | Stmt::While { .. }
        | Stmt::WhileLet { .. }
        | Stmt::For { .. }
        | Stmt::DeferBlock { .. }
        | Stmt::On { .. }
        | Stmt::Thread(_) => {
            let body = match stmt {
                Stmt::Thread(thread) => thread.body(),
                Stmt::If { body, .. }
                | Stmt::Loop { body }
                | Stmt::While { body, .. }
                | Stmt::WhileLet { body, .. }
                | Stmt::For { body, .. }
                | Stmt::DeferBlock {
                    statements: body, ..
                }
                | Stmt::On { body, .. } => body.as_slice(),
                _ => &[],
            };
            for stmt in body {
                collect_out_type_labels(stmt, labels);
            }
        }
        _ => {}
    }
}

fn expr_type_label(expr: &Expr) -> &'static str {
    match expr {
        Expr::Literal(Literal::String(_)) => "String",
        Expr::Literal(Literal::Char { .. }) => "Char",
        Expr::Literal(Literal::Int(_)) => "Int",
        Expr::Literal(Literal::Float(_)) => "Float",
        Expr::Literal(Literal::Bool(_)) => "Bool",
        Expr::Literal(Literal::Duration { .. }) => "Duration",
        Expr::Tuple(_) => "Tuple",
        Expr::List(_) => "List",
        Expr::EntityRef(_) => "EntityRef",
        _ => "Unknown",
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
