//! Bounded typed-HIR call-surface selection before semantic checking.

use std::cmp::Ordering;

use arcweft_lang_hir::{
    entry::HirEntryItem,
    model::{HirFlowItem, HirModule, HirTopLevelDecl},
};
use arcweft_lang_syntax::{
    ast::{
        choice::{ChoiceAction, ChoiceBlock, ChoiceItem, ChoiceOption, ChoicePlanItem},
        dialogue::{DialogueContent, DialogueToken},
        flow::{ContractClause, FlowItem, SelectBranchHead, Stmt, StmtMatchArm, WaitTarget},
        items::ImplMember,
        line_plan::{LinePlan, LinePlanItem, TriggerPattern},
        pattern::{Pattern, VariantPatternPayload},
    },
    expr::{CallArgumentRecoverySyntax, CallExpr, Expr, MatchExprArm},
};
use arcweft_source::SourceDocument;

use crate::{
    callable::{
        ResolveCallError, SignatureQueryStep, SignatureQueryStepControl, SignatureQueryWorkMeter,
        SignatureWorkKind,
    },
    checker::FocusedCallSite,
};

use super::{SignatureQueryControl, SignatureQueryError, map_signature_accounting_error};

pub(super) struct SignatureSurfaceSelection {
    pub(super) site: Option<FocusedCallSite>,
    pub(super) unsupported_surface: bool,
}

pub(super) fn select_signature_surface(
    module: &HirModule,
    document: &SourceDocument,
    byte_offset: usize,
    control: SignatureQueryControl<'_>,
    work: &mut SignatureQueryWorkMeter,
) -> Result<SignatureSurfaceSelection, SignatureQueryError> {
    let mut scanner = SurfaceScanner {
        module,
        document,
        byte_offset,
        control,
        work,
        selected: None,
        unsupported_surface: false,
    };
    scanner.scan_module()?;
    Ok(SignatureSurfaceSelection {
        site: scanner.selected,
        unsupported_surface: scanner.unsupported_surface,
    })
}

struct SurfaceScanner<'a> {
    module: &'a HirModule,
    document: &'a SourceDocument,
    byte_offset: usize,
    control: SignatureQueryControl<'a>,
    work: &'a mut SignatureQueryWorkMeter,
    selected: Option<FocusedCallSite>,
    unsupported_surface: bool,
}

impl SurfaceScanner<'_> {
    fn visit_node(&mut self) -> Result<(), SignatureQueryError> {
        self.control
            .check_signature_query_step(SignatureQueryStep::SurfaceTraversal)
            .map_err(map_control_error)?;
        self.work
            .charge(SignatureWorkKind::NodeVisits, 1)
            .map_err(map_signature_accounting_error)
    }

    fn poll_operation(&self) -> Result<(), SignatureQueryError> {
        self.control
            .check_signature_query_step(SignatureQueryStep::SurfaceTraversal)
            .map_err(map_control_error)
    }

    fn scan_module(&mut self) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        for flow in self.module.flows() {
            self.visit_node()?;
            if !self.owns_module(flow.module_path()) {
                continue;
            }
            self.scan_contracts(flow.contracts())?;
            self.scan_flow_items(flow.body())?;
        }
        for function in self.module.functions() {
            self.visit_node()?;
            if !self.owns_module(function.module_path()) {
                continue;
            }
            for parameter in function
                .signature()
                .param_groups()
                .iter()
                .flat_map(arcweft_lang_syntax::types::FnParamGroup::params)
            {
                self.visit_node()?;
                if let Some(default) = parameter.default() {
                    self.scan_expr(default)?;
                }
            }
            self.scan_contracts(function.contracts())?;
            self.scan_stmts(function.statements())?;
            if let Some(value) = function.value() {
                self.scan_expr(value.expr())?;
            }
        }
        for declaration in self.module.declarations() {
            self.scan_declaration(declaration)?;
        }
        Ok(())
    }

    fn owns_module(
        &self,
        owner: Option<&arcweft_lang_syntax::ast::module_path::CanonicalModulePath>,
    ) -> bool {
        let owner = owner.unwrap_or_else(|| self.module.module_path());
        self.module
            .project_source_document(owner)
            .is_some_and(|source| source.identity() == self.document.identity())
    }

    fn scan_declaration(
        &mut self,
        declaration: &HirTopLevelDecl,
    ) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        match declaration {
            HirTopLevelDecl::Impl(item) => {
                for member in item.members() {
                    if let ImplMember::Function {
                        body_statements,
                        body_value,
                        ..
                    } = member
                    {
                        self.scan_stmts(body_statements)?;
                        if let Some(value) = body_value {
                            self.scan_expr(value.expr())?;
                        }
                    }
                }
            }
            HirTopLevelDecl::Entry(item) => {
                for item in item.items() {
                    if let HirEntryItem::Option { value, .. } = item {
                        self.scan_expr(value)?;
                    }
                }
            }
            HirTopLevelDecl::TypeAlias(item) => {
                for clause in item.where_clauses() {
                    self.scan_expr(clause)?;
                }
            }
            HirTopLevelDecl::Source(source) => {
                if self.owns_module(source.module_path()) {
                    self.scan_stmts(source.item().body_statements())?;
                }
            }
            HirTopLevelDecl::DialogueDefaults(item) => {
                for assignment in item.assignments() {
                    self.scan_expr(assignment.value())?;
                }
            }
            HirTopLevelDecl::Trait(_)
            | HirTopLevelDecl::Enum(_)
            | HirTopLevelDecl::EntityDecl(_)
            | HirTopLevelDecl::ExternCapability(_)
            | HirTopLevelDecl::ExternMod(_)
            | HirTopLevelDecl::Proof(_)
            | HirTopLevelDecl::Test(_)
            | HirTopLevelDecl::Bench(_)
            | HirTopLevelDecl::Struct(_)
            | HirTopLevelDecl::Style(_) => {}
        }
        Ok(())
    }

    fn scan_contracts(&mut self, contracts: &[ContractClause]) -> Result<(), SignatureQueryError> {
        for contract in contracts {
            self.visit_node()?;
            match contract {
                ContractClause::Requires { expr, .. }
                | ContractClause::Ensures { expr, .. }
                | ContractClause::Invariant { expr, .. }
                | ContractClause::Assume { expr }
                | ContractClause::NoEffect(expr)
                | ContractClause::Decreases(expr) => self.scan_expr(expr)?,
                ContractClause::Reads(items)
                | ContractClause::Effects(items)
                | ContractClause::Modifies(items) => {
                    for item in items {
                        self.scan_expr(item)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn scan_flow_items(&mut self, items: &[HirFlowItem]) -> Result<(), SignatureQueryError> {
        for item in items {
            self.visit_node()?;
            match item {
                HirFlowItem::Stmt(stmt) => self.scan_stmt(stmt)?,
                HirFlowItem::Dialogue(dialogue) => self.scan_dialogue(dialogue)?,
                HirFlowItem::Choice(choice) | HirFlowItem::LetChoice { choice, .. } => {
                    self.scan_hir_choice(choice)?;
                }
                HirFlowItem::LetScope { scope, .. } => {
                    self.scan_stmts(scope.statements())?;
                    if let Some(value) = scope.value() {
                        self.scan_expr(value)?;
                    }
                }
                HirFlowItem::LetLoop { block, .. } | HirFlowItem::Loop(block) => {
                    self.scan_flow_items(block.body())?;
                }
                HirFlowItem::LetAwait { await_with, .. } | HirFlowItem::Await(await_with) => {
                    self.scan_expr(await_with.expr())?;
                    for branch in await_with.branches() {
                        self.scan_pattern(branch.pattern())?;
                        self.scan_flow_items(branch.body())?;
                    }
                }
                HirFlowItem::Thread(thread) => self.scan_flow_items(thread.body())?,
                HirFlowItem::If(block) => {
                    self.scan_expr(block.condition())?;
                    self.scan_flow_items(block.body())?;
                    self.scan_flow_items(block.else_body())?;
                }
                HirFlowItem::IfLet(block) => {
                    self.scan_expr(block.expr())?;
                    if let Some(guard) = block.guard() {
                        self.scan_expr(guard)?;
                    }
                    self.scan_flow_items(block.body())?;
                    self.scan_flow_items(block.else_body())?;
                }
                HirFlowItem::Match(block) => {
                    self.scan_expr(block.expr())?;
                    for arm in block.arms() {
                        self.scan_pattern(arm.pattern())?;
                        if let Some(guard) = arm.guard() {
                            self.scan_expr(guard)?;
                        }
                        self.scan_flow_items(arm.body())?;
                    }
                }
                HirFlowItem::While(block) => {
                    self.scan_expr(block.condition())?;
                    self.scan_flow_items(block.body())?;
                }
                HirFlowItem::WhileLet(block) => {
                    self.scan_expr(block.expr())?;
                    if let Some(guard) = block.guard() {
                        self.scan_expr(guard)?;
                    }
                    self.scan_flow_items(block.body())?;
                }
                HirFlowItem::For(block) => {
                    self.scan_expr(block.source())?;
                    self.scan_flow_items(block.body())?;
                }
                HirFlowItem::Select(block) => {
                    for branch in block.branches() {
                        self.scan_select_head(branch.head())?;
                        self.scan_flow_items(branch.body())?;
                    }
                }
                HirFlowItem::SourceLocale(block) => self.scan_flow_items(block.body())?,
                HirFlowItem::Scope(block) => self.scan_flow_items(block.body())?,
                HirFlowItem::Include(_) => {}
            }
        }
        Ok(())
    }

    fn scan_dialogue(
        &mut self,
        dialogue: &arcweft_lang_hir::model::HirDialogue,
    ) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        for value in [
            dialogue.look(),
            dialogue.stage(),
            dialogue.portrait(),
            dialogue.focus(),
            dialogue.cleanup(),
        ]
        .into_iter()
        .flatten()
        {
            self.scan_expr(value)?;
        }
        self.scan_dialogue_content(dialogue.content())?;
        if let Some(plan) = dialogue.plan() {
            self.scan_line_plan(plan)?;
        }
        Ok(())
    }

    fn scan_hir_choice(
        &mut self,
        choice: &arcweft_lang_hir::model::HirChoice,
    ) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        for item in choice.items() {
            self.scan_choice_item(item)?;
        }
        for option in choice.options() {
            if let Some(condition) = option.condition() {
                self.scan_expr(condition)?;
            }
            if let Some(value) = option.value() {
                self.scan_expr(value)?;
            }
            self.scan_choice_action(option.action())?;
        }
        if let Some(plan) = choice.plan() {
            for item in plan.items() {
                self.scan_choice_plan_item(item)?;
            }
        }
        Ok(())
    }

    fn scan_stmts(&mut self, statements: &[Stmt]) -> Result<(), SignatureQueryError> {
        for statement in statements {
            self.scan_stmt(statement)?;
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive statement walk defines which typed expressions consume search work"
    )]
    fn scan_stmt(&mut self, statement: &Stmt) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        match statement {
            Stmt::Assertion(assertion) => {
                for condition in assertion.conditions() {
                    self.scan_expr(condition)?;
                }
            }
            Stmt::Let { expr, .. } | Stmt::Return { expr, .. } | Stmt::Expr { expr, .. } => {
                self.scan_expr(expr)?;
            }
            Stmt::Goto(expr) => {
                if let Some(range) = expr.range() {
                    self.mark_unsupported_range(range);
                }
                self.scan_expr(expr.expr())?;
            }
            Stmt::Defer { expr, .. }
            | Stmt::Yield(expr)
            | Stmt::Close(expr)
            | Stmt::Out { expr, .. }
            | Stmt::Select(expr)
            | Stmt::Break {
                expr: Some(expr), ..
            } => self.scan_expr(expr.expr())?,
            Stmt::Assign { target, expr }
            | Stmt::Signal {
                target,
                value: expr,
            }
            | Stmt::LifetimeSet { target, expr } => {
                self.scan_expr(target.expr())?;
                self.scan_expr(expr.expr())?;
            }
            Stmt::LetElse {
                expr, else_body, ..
            } => {
                self.scan_expr(expr.expr())?;
                self.scan_stmts(else_body)?;
            }
            Stmt::LetActionReceive { pattern, action } => {
                self.scan_pattern(pattern)?;
                self.scan_expr(action.expr())?;
            }
            Stmt::LetChoice { choice, .. } => self.scan_choice_block(choice)?,
            Stmt::LetScope { scope, .. } => {
                self.scan_stmts(scope.statements())?;
                if let Some(value) = scope.value() {
                    self.scan_expr(value)?;
                }
            }
            Stmt::LetAwait { .. }
            | Stmt::LetLoop { .. }
            | Stmt::Break { expr: None, .. }
            | Stmt::Continue { .. }
            | Stmt::Raw(_) => {}
            Stmt::Thread(thread) => self.scan_syntax_flow_items(thread.body())?,
            Stmt::DeferBlock { statements, .. }
            | Stmt::On {
                body: statements, ..
            }
            | Stmt::Loop { body: statements } => self.scan_stmts(statements)?,
            Stmt::UnsafeLifetime { reason, body, .. } => {
                if let Some(reason) = reason {
                    self.scan_expr(reason)?;
                }
                self.scan_stmts(body)?;
            }
            Stmt::Wait(WaitTarget::Duration(expr) | WaitTarget::Expr(expr)) => {
                self.scan_expr(expr.expr())?;
            }
            Stmt::If {
                condition,
                body,
                else_body,
            } => {
                self.scan_expr(condition.expr())?;
                self.scan_stmts(body)?;
                self.scan_stmts(else_body)?;
            }
            Stmt::While { condition, body } => {
                self.scan_expr(condition.expr())?;
                self.scan_stmts(body)?;
            }
            Stmt::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                self.scan_pattern(pattern)?;
                self.scan_expr(expr.expr())?;
                if let Some(guard) = guard {
                    self.scan_expr(guard.expr())?;
                }
                self.scan_stmts(body)?;
            }
            Stmt::For {
                pattern,
                source,
                body,
            } => {
                self.scan_pattern(pattern)?;
                self.scan_expr(source.expr())?;
                self.scan_stmts(body)?;
            }
            Stmt::Match { expr, arms } => self.scan_stmt_match(expr.expr(), arms)?,
        }
        Ok(())
    }

    fn scan_syntax_flow_items(&mut self, items: &[FlowItem]) -> Result<(), SignatureQueryError> {
        for item in items {
            self.visit_node()?;
            match item {
                FlowItem::Stmt(stmt) => self.scan_stmt(stmt)?,
                FlowItem::Choice(choice) => self.scan_choice_block(choice)?,
                FlowItem::If(block) => {
                    self.scan_expr(block.condition())?;
                    self.scan_syntax_flow_items(block.body())?;
                    self.scan_syntax_flow_items(block.else_body())?;
                }
                FlowItem::IfLet(block) => {
                    self.scan_pattern(block.pattern())?;
                    self.scan_expr(block.expr())?;
                    if let Some(guard) = block.guard() {
                        self.scan_expr(guard)?;
                    }
                    self.scan_syntax_flow_items(block.body())?;
                    self.scan_syntax_flow_items(block.else_body())?;
                }
                FlowItem::Match(block) => {
                    self.scan_expr(block.expr())?;
                    for arm in block.arms() {
                        self.scan_pattern(arm.pattern())?;
                        if let Some(guard) = arm.guard() {
                            self.scan_expr(guard)?;
                        }
                        self.scan_syntax_flow_items(arm.body())?;
                    }
                }
                FlowItem::Loop(block) => self.scan_syntax_flow_items(block.body())?,
                FlowItem::While(block) => {
                    self.scan_expr(block.condition())?;
                    self.scan_syntax_flow_items(block.body())?;
                }
                FlowItem::WhileLet(block) => {
                    self.scan_pattern(block.pattern())?;
                    self.scan_expr(block.expr())?;
                    if let Some(guard) = block.guard() {
                        self.scan_expr(guard)?;
                    }
                    self.scan_syntax_flow_items(block.body())?;
                }
                FlowItem::For(block) => {
                    self.scan_pattern(block.pattern())?;
                    self.scan_expr(block.source())?;
                    self.scan_syntax_flow_items(block.body())?;
                }
                FlowItem::Select(block) => {
                    for branch in block.branches() {
                        self.scan_select_head(branch.head())?;
                        self.scan_syntax_flow_items(branch.body())?;
                    }
                }
                FlowItem::SourceLocale(block) => self.scan_syntax_flow_items(block.body())?,
                FlowItem::Scope(block) => self.scan_syntax_flow_items(block.body())?,
                FlowItem::AwaitWith(_)
                | FlowItem::SpeakerLine(_)
                | FlowItem::ContentCall(_)
                | FlowItem::Include(_)
                | FlowItem::Raw(_) => {}
            }
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive expression walk is the typed search and accounting boundary"
    )]
    fn scan_expr(&mut self, expression: &Expr) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        match expression {
            Expr::Call(call) => {
                self.scan_call(call)?;
                self.scan_expr(call.callee())?;
                for argument in call.args() {
                    self.scan_expr(argument.value())?;
                }
            }
            Expr::Tuple(items) | Expr::BracketSeq(items) => {
                for item in items {
                    self.scan_expr(item)?;
                }
            }
            Expr::ArrayRepeat { value, len } => {
                self.scan_expr(value)?;
                self.scan_expr(len)?;
            }
            Expr::Select(select) => self.scan_expr(select.target())?,
            Expr::DialogueCall {
                callee,
                content,
                plan,
            } => {
                self.scan_expr(callee)?;
                self.scan_dialogue_content(content)?;
                if let Some(plan) = plan {
                    self.scan_line_plan(plan)?;
                }
            }
            Expr::Index { target, index } => {
                self.scan_expr(target)?;
                self.scan_expr(index)?;
            }
            Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, rhs, .. } => {
                self.scan_expr(lhs)?;
                self.scan_expr(rhs)?;
            }
            Expr::Try(try_expr) => self.scan_expr(try_expr.operand())?,
            Expr::Closure { body, .. } | Expr::Unary { expr: body, .. } => self.scan_expr(body)?,
            Expr::Await(awaited) => self.scan_expr(awaited.operand())?,
            Expr::Borrow(borrow) => self.scan_expr(borrow.operand())?,
            Expr::Deref(deref) => self.scan_expr(deref.operand())?,
            Expr::Thread { block } => self.scan_syntax_flow_items(block.body())?,
            Expr::Range { start, end, .. } => {
                if let Some(start) = start {
                    self.scan_expr(start)?;
                }
                if let Some(end) = end {
                    self.scan_expr(end)?;
                }
            }
            Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
                for (_, value) in fields {
                    self.scan_expr(value)?;
                }
            }
            Expr::Block { statements, value }
            | Expr::ComputationBlock {
                statements, value, ..
            }
            | Expr::NamedBlock {
                statements, value, ..
            } => {
                self.scan_stmts(statements)?;
                if let Some(value) = value {
                    self.scan_expr(value)?;
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.scan_expr(condition)?;
                self.scan_expr(then_branch)?;
                if let Some(else_branch) = else_branch {
                    self.scan_expr(else_branch)?;
                }
            }
            Expr::IfLet {
                pattern,
                expr,
                guard,
                then_branch,
                else_branch,
            } => {
                self.scan_pattern(pattern)?;
                self.scan_expr(expr)?;
                if let Some(guard) = guard {
                    self.scan_expr(guard)?;
                }
                self.scan_expr(then_branch)?;
                if let Some(else_branch) = else_branch {
                    self.scan_expr(else_branch)?;
                }
            }
            Expr::Match { scrutinee, arms } => {
                self.scan_expr(scrutinee)?;
                for arm in arms {
                    self.scan_match_expr_arm(arm)?;
                }
            }
            Expr::Literal(_)
            | Expr::Path(_)
            | Expr::ShortVariant(_)
            | Expr::Placeholder(_)
            | Expr::EntityRef(_)
            | Expr::LifetimePath { .. }
            | Expr::NumericBracketSeq(_)
            | Expr::Raw(_) => {}
        }
        Ok(())
    }

    fn scan_call(&mut self, call: &CallExpr) -> Result<(), SignatureQueryError> {
        self.work
            .charge(SignatureWorkKind::CandidateCalls, 1)
            .map_err(map_signature_accounting_error)?;
        for _ in call.args() {
            self.poll_operation()?;
            self.work
                .charge(SignatureWorkKind::Arguments, 1)
                .map_err(map_signature_accounting_error)?;
        }
        let Some(parenthesized) = call.parenthesized_syntax() else {
            self.unsupported_surface |=
                call.range().start() <= self.byte_offset && self.byte_offset <= call.range().end();
            return Ok(());
        };
        let arguments = parenthesized.argument_list();
        let recovery_nodes = arguments
            .arguments()
            .iter()
            .filter(|argument| {
                matches!(
                    argument.recovery(),
                    CallArgumentRecoverySyntax::Recovered { .. }
                )
            })
            .count()
            + usize::from(arguments.terminator().close_paren().is_none());
        for _ in 0..recovery_nodes {
            self.poll_operation()?;
            self.work
                .charge(SignatureWorkKind::RecoveryNodes, 1)
                .map_err(map_signature_accounting_error)?;
        }
        if !arguments.contains_signature_cursor(self.byte_offset) {
            return Ok(());
        }
        self.poll_operation()?;
        self.work
            .charge(SignatureWorkKind::NestedCalls, 1)
            .map_err(map_signature_accounting_error)?;
        let Some(candidate) = FocusedCallSite::from_call(call, self.document, self.byte_offset)
        else {
            return Ok(());
        };
        match self.selected.as_ref() {
            None => self.selected = Some(candidate),
            Some(current) => match candidate.compare_focus(current) {
                Ordering::Greater => self.selected = Some(candidate),
                Ordering::Less => {}
                Ordering::Equal
                    if candidate.call() == current.call()
                        && candidate.arguments() == current.arguments() => {}
                Ordering::Equal => {
                    return Err(super::SignatureSemanticUnavailable::AmbiguousCallRange {
                        document: self.document.identity().clone(),
                        byte_offset: self.byte_offset,
                    }
                    .into());
                }
            },
        }
        Ok(())
    }

    fn scan_pattern(&mut self, pattern: &Pattern) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        match pattern {
            Pattern::Literal(expression) => self.scan_expr(expression)?,
            Pattern::Tuple(items) | Pattern::BracketSeq { items, .. } => {
                for item in items {
                    self.scan_pattern(item)?;
                }
            }
            Pattern::Record { fields, .. } => {
                for field in fields {
                    self.scan_pattern(field.pattern())?;
                }
            }
            Pattern::Whole { pattern, .. } => self.scan_pattern(pattern)?,
            Pattern::Variant {
                payload: Some(payload),
                ..
            } => match payload {
                VariantPatternPayload::Tuple(items) => {
                    for item in items {
                        self.scan_pattern(item)?;
                    }
                }
                VariantPatternPayload::Record { fields, .. } => {
                    for field in fields {
                        self.scan_pattern(field.pattern())?;
                    }
                }
            },
            Pattern::Raw(_)
            | Pattern::Entity(_)
            | Pattern::Ident(_)
            | Pattern::MutIdent(_)
            | Pattern::Variant { payload: None, .. }
            | Pattern::Discard
            | Pattern::Typed { .. } => {}
        }
        Ok(())
    }

    fn scan_choice_block(&mut self, choice: &ChoiceBlock) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        for option in choice.options() {
            self.scan_choice_option(option)?;
        }
        Ok(())
    }

    fn scan_choice_item(&mut self, item: &ChoiceItem) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        match item {
            ChoiceItem::Let { pattern, expr } => {
                self.scan_pattern(pattern)?;
                self.scan_expr(expr)?;
            }
            ChoiceItem::If { condition, items } => {
                self.scan_expr(condition)?;
                for item in items {
                    self.scan_choice_item(item)?;
                }
            }
            ChoiceItem::For {
                pattern,
                source,
                items,
            } => {
                self.scan_pattern(pattern)?;
                self.scan_expr(source)?;
                for item in items {
                    self.scan_choice_item(item)?;
                }
            }
            ChoiceItem::Match { expr, arms } => {
                self.scan_expr(expr)?;
                for arm in arms {
                    self.scan_pattern(arm.pattern())?;
                    if let Some(guard) = arm.guard() {
                        self.scan_expr(guard)?;
                    }
                    for item in arm.items() {
                        self.scan_choice_item(item)?;
                    }
                }
            }
            ChoiceItem::Option(option) => self.scan_choice_option(option)?,
            ChoiceItem::Raw(_) => {}
        }
        Ok(())
    }

    fn scan_choice_option(&mut self, option: &ChoiceOption) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        for expression in [
            option.id_expr(),
            option.value(),
            option.enabled(),
            option.visible(),
            option.order(),
            option.hotkey(),
        ]
        .into_iter()
        .flatten()
        {
            self.scan_expr(expression)?;
        }
        for field in option.view_fields() {
            self.scan_expr(field.value())?;
        }
        self.scan_choice_action(option.action())
    }

    fn scan_choice_action(&mut self, action: &ChoiceAction) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        match action {
            ChoiceAction::Out(expression) => self.scan_expr(expression)?,
            ChoiceAction::SelectBlock(statements) => self.scan_stmts(statements)?,
            ChoiceAction::Goto(_) | ChoiceAction::None => {}
        }
        Ok(())
    }

    fn scan_choice_plan_item(&mut self, item: &ChoicePlanItem) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        match item {
            ChoicePlanItem::Option { value, .. } => self.scan_expr(value)?,
            ChoicePlanItem::Timeout { duration, body } => {
                self.scan_expr(duration)?;
                self.scan_stmts(body)?;
            }
            ChoicePlanItem::Cancel { body, .. } => self.scan_stmts(body)?,
            ChoicePlanItem::OnSelect { pattern, body } => {
                self.scan_pattern(pattern)?;
                self.scan_stmts(body)?;
            }
            ChoicePlanItem::Raw(_) => {}
        }
        Ok(())
    }

    fn scan_stmt_match(
        &mut self,
        expression: &Expr,
        arms: &[StmtMatchArm],
    ) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        self.scan_expr(expression)?;
        for arm in arms {
            self.scan_pattern(arm.pattern())?;
            if let Some(guard) = arm.guard() {
                self.scan_expr(guard)?;
            }
            self.scan_stmts(arm.body())?;
        }
        Ok(())
    }

    fn scan_match_expr_arm(&mut self, arm: &MatchExprArm) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        self.scan_pattern(arm.pattern())?;
        if let Some(guard) = arm.guard() {
            self.scan_expr(guard)?;
        }
        self.scan_expr(arm.value())
    }

    fn scan_select_head(&mut self, head: &SelectBranchHead) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        match head {
            SelectBranchHead::Bind { source, .. } => self.scan_expr(source)?,
            SelectBranchHead::Frame(pattern) | SelectBranchHead::Event(pattern) => {
                self.scan_pattern(pattern)?;
            }
            SelectBranchHead::Raw(_) => {}
        }
        Ok(())
    }

    fn scan_dialogue_content(
        &mut self,
        content: &DialogueContent,
    ) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        for token in content.tokens() {
            match token {
                DialogueToken::Expr(expression) => self.scan_expr(expression.expr())?,
                DialogueToken::Tag(tag) | DialogueToken::InferredTag(tag) => {
                    if let Some(range) = content.source_range(tag.range()) {
                        self.mark_unsupported_range(range);
                    }
                }
                DialogueToken::Text(_)
                | DialogueToken::Raw(_)
                | DialogueToken::Mark(_)
                | DialogueToken::EndTag(_)
                | DialogueToken::InferredEndTag
                | DialogueToken::Ruby { .. }
                | DialogueToken::Escape(_) => {}
            }
        }
        Ok(())
    }

    fn mark_unsupported_range(&mut self, range: arcweft_lang_syntax::ast::common::TextRange) {
        self.unsupported_surface |=
            range.start() <= self.byte_offset && self.byte_offset <= range.end();
    }

    fn scan_line_plan(&mut self, plan: &LinePlan) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        for item in plan.items() {
            self.scan_line_plan_item(item)?;
        }
        Ok(())
    }

    fn scan_line_plan_item(&mut self, item: &LinePlanItem) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        match item {
            LinePlanItem::Init(statements) => self.scan_stmts(statements)?,
            LinePlanItem::Thread(thread) => self.scan_syntax_flow_items(thread.body())?,
            LinePlanItem::On { trigger, body } => {
                self.scan_trigger(trigger)?;
                self.scan_stmts(body)?;
            }
            LinePlanItem::Stmt(statement) => self.scan_stmt(statement)?,
            LinePlanItem::Option { value, .. }
            | LinePlanItem::Let { expr: value, .. }
            | LinePlanItem::Out(value)
            | LinePlanItem::Expr(value) => self.scan_expr(value)?,
            LinePlanItem::TimedCue { anchor, body } => {
                self.scan_expr(anchor)?;
                self.scan_expr(body)?;
            }
            LinePlanItem::CancelRule(rule) => self.scan_stmts(rule.action())?,
            LinePlanItem::TimelineAssert(assertion) => self.scan_expr(assertion.condition())?,
            LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
                for item in items {
                    self.scan_line_plan_item(item)?;
                }
            }
            LinePlanItem::Raw(_) => {}
        }
        Ok(())
    }

    fn scan_trigger(&mut self, trigger: &TriggerPattern) -> Result<(), SignatureQueryError> {
        self.visit_node()?;
        match trigger {
            TriggerPattern::Signal { target, value } => {
                self.scan_expr(target)?;
                if let Some(value) = value {
                    self.scan_pattern(value)?;
                }
            }
            TriggerPattern::Timeout(expression) | TriggerPattern::Expr(expression) => {
                self.scan_expr(expression)?;
            }
            TriggerPattern::Input(pattern)
            | TriggerPattern::Event(pattern)
            | TriggerPattern::Mark(pattern)
            | TriggerPattern::Select(pattern)
            | TriggerPattern::Task(pattern)
            | TriggerPattern::Scope(pattern) => self.scan_pattern(pattern)?,
        }
        Ok(())
    }
}

fn map_control_error(error: ResolveCallError) -> SignatureQueryError {
    match error {
        ResolveCallError::Cancelled => SignatureQueryError::Cancelled,
        ResolveCallError::DeadlineExceeded => SignatureQueryError::DeadlineExceeded,
        error => SignatureQueryError::Resolve(error),
    }
}
