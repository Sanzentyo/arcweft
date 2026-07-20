use arcweft_lang_syntax::{
    ast::{
        choice::{ChoiceAction, ChoiceItem, ChoicePlanItem},
        common::TextRange,
        dialogue::{DialogueContent, SpeakerLine},
        flow::{AuthoredExpr, FlowItem, Stmt, WaitTarget},
        items::{ImplMember, Item, TraitMember},
        line_plan::{LinePlan, LinePlanItem},
    },
    expr::{Expr, collect_dialogue_call_content_ranges, collect_expr_source_ranges},
    source::ParsedSource,
};
use std::ops::Range;

/// One dialogue-content occurrence with a typed route back to document bytes.
pub(crate) enum DialogueContentSite<'a> {
    Parsed {
        content: &'a DialogueContent,
        speaker_line: Option<&'a SpeakerLine>,
    },
    Expression {
        raw: &'a str,
        source_range: TextRange,
    },
}

impl<'a> DialogueContentSite<'a> {
    pub(crate) fn raw(&self) -> &str {
        match self {
            Self::Parsed { content, .. } => content.raw(),
            Self::Expression { raw, .. } => raw,
        }
    }

    pub(crate) fn source_range(&self, relative: TextRange) -> Option<TextRange> {
        match self {
            Self::Parsed { content, .. } => content.source_range(relative),
            Self::Expression { raw, source_range } => {
                if relative.start() > relative.end() || relative.end() > raw.len() {
                    return None;
                }
                Some(TextRange::new(
                    source_range.start() + relative.start(),
                    source_range.start() + relative.end(),
                ))
            }
        }
    }

    fn authored_range(&self) -> Option<TextRange> {
        self.source_range(TextRange::new(0, self.raw().len()))
    }

    pub(crate) const fn speaker_line(&self) -> Option<&'a SpeakerLine> {
        match self {
            Self::Parsed { speaker_line, .. } => *speaker_line,
            Self::Expression { .. } => None,
        }
    }
}

pub(crate) fn visit_dialogue_contents<'a>(
    parsed: &'a ParsedSource,
    visit: impl FnMut(DialogueContentSite<'a>),
) {
    let mut visitor = DialogueContentVisitor {
        source: parsed.source(),
        visit,
        visited_site_ranges: Vec::new(),
    };
    for item in parsed.typed_tree().items() {
        visitor.visit_item(item);
    }
}

pub(crate) fn collect_dialogue_content_ranges(parsed: &ParsedSource) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    visit_dialogue_contents(parsed, |site| {
        if let Some(range) = site.authored_range() {
            ranges.push(range.as_range());
        }
    });
    ranges
}

pub(crate) fn collect_speaker_lines(parsed: &ParsedSource) -> Vec<&SpeakerLine> {
    let mut lines = Vec::new();
    visit_dialogue_contents(parsed, |site| {
        if let Some(line) = site.speaker_line() {
            lines.push(line);
        }
    });
    lines
}

struct DialogueContentVisitor<'a, F> {
    source: &'a str,
    visit: F,
    visited_site_ranges: Vec<TextRange>,
}

impl<'a, F> DialogueContentVisitor<'a, F>
where
    F: FnMut(DialogueContentSite<'a>),
{
    fn visit_item(&mut self, item: &'a Item) {
        match item {
            Item::Flow(flow) => self.visit_flow_items(flow.body()),
            Item::Function(function) => {
                self.visit_stmts(function.body_statements());
                if let Some(value) = function.body_value() {
                    self.visit_authored_expr(value);
                }
            }
            Item::Trait(trait_item) => {
                for member in trait_item.members() {
                    if let TraitMember::Function {
                        body_statements,
                        body_value,
                        ..
                    } = member
                    {
                        self.visit_stmts(body_statements);
                        if let Some(value) = body_value {
                            self.visit_authored_expr(value);
                        }
                    }
                }
            }
            Item::Impl(impl_item) => {
                for member in impl_item.members() {
                    if let ImplMember::Function {
                        body_statements,
                        body_value,
                        ..
                    } = member
                    {
                        self.visit_stmts(body_statements);
                        if let Some(value) = body_value {
                            self.visit_authored_expr(value);
                        }
                    }
                }
            }
            Item::Source(source) => {
                self.visit_stmts(source.body_statements());
                for handler in source.handlers() {
                    self.visit_stmts(handler.body());
                }
            }
            Item::Callable(_)
            | Item::Enum(_)
            | Item::Struct(_)
            | Item::TypeAlias(_)
            | Item::EntityDecl(_)
            | Item::Entry(_)
            | Item::ExternCapability(_)
            | Item::ExternMod(_)
            | Item::Style(_)
            | Item::DialogueDefaults(_)
            | Item::Proof(_)
            | Item::Test(_)
            | Item::Bench(_)
            | Item::Raw(_) => {}
        }
    }

    fn visit_flow_items(&mut self, items: &'a [FlowItem]) {
        for item in items {
            self.visit_flow_item(item);
        }
    }

    fn visit_flow_item(&mut self, item: &'a FlowItem) {
        match item {
            FlowItem::SpeakerLine(line) => {
                self.visit_parsed_content(line.content(), Some(line));
            }
            FlowItem::ContentCall(call) => {
                self.visit_parsed_content(call.content(), None);
            }
            FlowItem::Stmt(stmt) => self.visit_stmt(stmt),
            FlowItem::Choice(choice) => self.visit_choice(choice),
            FlowItem::If(block) => {
                self.visit_authored_expr(block.condition_authored());
                self.visit_flow_items(block.body());
                self.visit_flow_items(block.else_body());
            }
            FlowItem::IfLet(block) => {
                self.visit_authored_expr(block.expr_authored());
                if let Some(guard) = block.guard_authored() {
                    self.visit_authored_expr(guard);
                }
                self.visit_flow_items(block.body());
                self.visit_flow_items(block.else_body());
            }
            FlowItem::Match(block) => {
                self.visit_authored_expr(block.expr_authored());
                for arm in block.arms() {
                    if let Some(guard) = arm.guard_authored() {
                        self.visit_authored_expr(guard);
                    }
                    self.visit_flow_items(arm.body());
                }
            }
            FlowItem::Loop(block) => self.visit_flow_items(block.body()),
            FlowItem::While(block) => {
                self.visit_authored_expr(block.condition_authored());
                self.visit_flow_items(block.body());
            }
            FlowItem::WhileLet(block) => {
                self.visit_authored_expr(block.expr_authored());
                if let Some(guard) = block.guard_authored() {
                    self.visit_authored_expr(guard);
                }
                self.visit_flow_items(block.body());
            }
            FlowItem::For(block) => {
                self.visit_authored_expr(block.source_authored());
                self.visit_flow_items(block.body());
            }
            FlowItem::Select(block) => {
                for branch in block.branches() {
                    self.visit_flow_items(branch.body());
                }
            }
            FlowItem::SourceLocale(block) => self.visit_flow_items(block.body()),
            FlowItem::Scope(block) => self.visit_flow_items(block.body()),
            FlowItem::AwaitWith(await_with) => {
                self.visit_authored_expr(await_with.expr_authored());
                for branch in await_with.branches() {
                    self.visit_flow_items(branch.body());
                }
            }
            FlowItem::Include(_) | FlowItem::Raw(_) => {}
        }
    }

    fn visit_choice(&mut self, choice: &'a arcweft_lang_syntax::ast::choice::ChoiceBlock) {
        for item in choice.items() {
            self.visit_choice_item(item);
        }
        if let Some(plan) = choice.plan() {
            for item in plan.items() {
                match item {
                    ChoicePlanItem::Timeout { body, .. }
                    | ChoicePlanItem::Cancel { body, .. }
                    | ChoicePlanItem::OnSelect { body, .. } => self.visit_stmts(body),
                    ChoicePlanItem::Option { .. } | ChoicePlanItem::Raw(_) => {}
                }
            }
        }
    }

    fn visit_choice_item(&mut self, item: &'a ChoiceItem) {
        match item {
            ChoiceItem::If { items, .. } | ChoiceItem::For { items, .. } => {
                for item in items {
                    self.visit_choice_item(item);
                }
            }
            ChoiceItem::Match { arms, .. } => {
                for arm in arms {
                    for item in arm.items() {
                        self.visit_choice_item(item);
                    }
                }
            }
            ChoiceItem::Option(option) => {
                if let ChoiceAction::SelectBlock(statements) = option.action() {
                    self.visit_stmts(statements);
                }
            }
            ChoiceItem::Let { .. } | ChoiceItem::Raw(_) => {}
        }
    }

    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::Assertion(assertion) => self.visit_assertion(assertion),
            Stmt::Let {
                expr,
                expr_source,
                expr_range,
                ..
            }
            | Stmt::Return {
                expr,
                expr_source,
                expr_range,
            }
            | Stmt::Expr {
                expr,
                expr_source,
                expr_range,
            } => self.visit_sourced_expr(expr, expr_source.as_deref(), *expr_range),
            Stmt::Assign { target, expr }
            | Stmt::LifetimeSet { target, expr }
            | Stmt::Signal {
                target,
                value: expr,
            } => {
                self.visit_authored_expr(target);
                self.visit_authored_expr(expr);
            }
            Stmt::LetElse {
                expr, else_body, ..
            } => {
                self.visit_authored_expr(expr);
                self.visit_stmts(else_body);
            }
            Stmt::LetChoice { choice, .. } => self.visit_choice(choice),
            Stmt::LetScope { scope, .. } => self.visit_stmts(scope.statements()),
            Stmt::LetLoop { block, .. } => self.visit_flow_items(block.body()),
            Stmt::LetAwait { await_with, .. } => {
                self.visit_authored_expr(await_with.expr_authored());
                for branch in await_with.branches() {
                    self.visit_flow_items(branch.body());
                }
            }
            Stmt::LetActionReceive { action, .. } => self.visit_authored_expr(action),
            Stmt::Out { expr, .. }
            | Stmt::Goto(expr)
            | Stmt::Defer { expr, .. }
            | Stmt::Yield(expr)
            | Stmt::Close(expr)
            | Stmt::Select(expr)
            | Stmt::Break {
                expr: Some(expr), ..
            } => self.visit_authored_expr(expr),
            Stmt::Thread(thread) => self.visit_flow_items(thread.body()),
            Stmt::DeferBlock { statements, .. }
            | Stmt::On {
                body: statements, ..
            }
            | Stmt::UnsafeLifetime {
                body: statements, ..
            }
            | Stmt::Loop { body: statements } => self.visit_stmts(statements),
            Stmt::Wait(WaitTarget::Duration(expr) | WaitTarget::Expr(expr)) => {
                self.visit_authored_expr(expr);
            }
            Stmt::If {
                condition,
                body,
                else_body,
            } => {
                self.visit_authored_expr(condition);
                self.visit_stmts(body);
                self.visit_stmts(else_body);
            }
            Stmt::While { condition, body } => {
                self.visit_authored_expr(condition);
                self.visit_stmts(body);
            }
            Stmt::WhileLet {
                expr, guard, body, ..
            } => {
                self.visit_authored_expr(expr);
                if let Some(guard) = guard {
                    self.visit_authored_expr(guard);
                }
                self.visit_stmts(body);
            }
            Stmt::For { source, body, .. } => {
                self.visit_authored_expr(source);
                self.visit_stmts(body);
            }
            Stmt::Match { expr, arms } => self.visit_match_stmt(expr, arms),
            Stmt::Break { expr: None, .. } | Stmt::Continue { .. } | Stmt::Raw(_) => {}
        }
    }

    fn visit_assertion(&mut self, assertion: &'a arcweft_lang_syntax::assertion::AssertionStmt) {
        for condition in assertion.conditions() {
            self.visit_expr_owned_bodies(condition);
        }
    }

    fn visit_match_stmt(
        &mut self,
        expression: &'a AuthoredExpr,
        arms: &'a [arcweft_lang_syntax::ast::flow::StmtMatchArm],
    ) {
        self.visit_authored_expr(expression);
        for arm in arms {
            if let Some(guard) = arm.guard_authored() {
                self.visit_authored_expr(guard);
            }
            self.visit_stmts(arm.body());
        }
    }

    fn visit_stmts(&mut self, statements: &'a [Stmt]) {
        for statement in statements {
            self.visit_stmt(statement);
        }
    }

    fn visit_parsed_content(
        &mut self,
        content: &'a DialogueContent,
        speaker_line: Option<&'a SpeakerLine>,
    ) {
        let Some(source_range) = content.source_range(TextRange::new(0, content.raw().len()))
        else {
            return;
        };
        if self.visited_site_ranges.contains(&source_range) {
            return;
        }
        self.visited_site_ranges.push(source_range);
        (self.visit)(DialogueContentSite::Parsed {
            content,
            speaker_line,
        });
    }

    fn visit_authored_expr(&mut self, authored: &'a AuthoredExpr) {
        self.visit_sourced_expr(authored.expr(), authored.source(), authored.range());
    }

    fn visit_sourced_expr(
        &mut self,
        expr: &'a Expr,
        authored_source: Option<&str>,
        authored_range: Option<TextRange>,
    ) {
        let (Some(authored_source), Some(authored_range)) = (authored_source, authored_range)
        else {
            return;
        };
        let Some(document_source) = self.source.get(authored_range.as_range()) else {
            return;
        };
        if document_source != authored_source
            && document_source.replace("\r\n", "\n") != authored_source
        {
            return;
        }

        let content_ranges =
            collect_dialogue_call_content_ranges(expr, document_source, authored_range);
        let expression_nodes = collect_expr_source_ranges(expr, document_source, authored_range);
        for source_range in content_ranges {
            if self.visited_site_ranges.contains(&source_range) {
                continue;
            }
            let Some(raw) = self.source.get(source_range.as_range()) else {
                continue;
            };
            self.visited_site_ranges.push(source_range);
            (self.visit)(DialogueContentSite::Expression { raw, source_range });
        }
        for expression in expression_nodes {
            self.visit_expr_owned_bodies(expression.expr());
        }
    }

    fn visit_expr_owned_bodies(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Block { statements, .. }
            | Expr::ComputationBlock { statements, .. }
            | Expr::NamedBlock { statements, .. } => self.visit_stmts(statements),
            Expr::Thread { block } => self.visit_flow_items(block.body()),
            Expr::DialogueCall {
                plan: Some(plan), ..
            } => self.visit_line_plan(plan),
            _ => {}
        }
    }

    fn visit_line_plan(&mut self, plan: &'a LinePlan) {
        self.visit_line_plan_items(plan.items());
    }

    fn visit_line_plan_items(&mut self, items: &'a [LinePlanItem]) {
        for item in items {
            match item {
                LinePlanItem::Init(statements)
                | LinePlanItem::On {
                    body: statements, ..
                } => self.visit_stmts(statements),
                LinePlanItem::Thread(thread) => self.visit_flow_items(thread.body()),
                LinePlanItem::Stmt(statement) => self.visit_stmt(statement),
                LinePlanItem::CancelRule(rule) => self.visit_stmts(rule.action()),
                LinePlanItem::StartGroup(items) | LinePlanItem::TogetherGroup(items) => {
                    self.visit_line_plan_items(items);
                }
                LinePlanItem::Option { .. }
                | LinePlanItem::Let { .. }
                | LinePlanItem::Out(_)
                | LinePlanItem::TimedCue { .. }
                | LinePlanItem::TimelineAssert(_)
                | LinePlanItem::Expr(_)
                | LinePlanItem::Raw(_) => {}
            }
        }
    }
}
