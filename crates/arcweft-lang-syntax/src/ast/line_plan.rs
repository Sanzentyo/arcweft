use crate::expr::Expr;

use super::common::TextRange;
use super::flow::{Stmt, ThreadBlock};
use super::items::RawSyntax;
use super::pattern::Pattern;

/// Canonical `with { ... }` line plan, plus `with:` indentation sugar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinePlan {
    style: BlockStyle,
    label: Option<String>,
    items: Vec<LinePlanItem>,
    range: TextRange,
}

/// Source style used for a parsed block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockStyle {
    Brace,
    Indent,
    Flat,
}

/// Item allowed inside a line plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinePlanItem {
    /// `init { ... }` / `init:` setup statements that run before reveal.
    Init(Vec<Stmt>),
    /// `thread name { ... }` child task scoped to this line.
    Thread(ThreadBlock),
    /// `on .mark { ... }` line-local mark/event handler.
    On {
        trigger: TriggerPattern,
        body: Vec<Stmt>,
    },
    Option {
        name: String,
        value: Expr,
    },
    Let {
        pattern: Pattern,
        expr: Expr,
    },
    /// Statement item such as `defer { ... }` preserved in a line plan.
    Stmt(Stmt),
    Out(Expr),
    CancelRule(CancelRuleSyntax),
    TimedCue {
        anchor: Expr,
        body: Expr,
    },
    StartGroup(Vec<LinePlanItem>),
    TogetherGroup(Vec<LinePlanItem>),
    Memo {
        name: String,
        options: Vec<(String, Expr)>,
    },
    Assert {
        debug: bool,
        expr: Expr,
    },
    Expr(Expr),
    Raw(RawSyntax),
}

/// Parsed cancellation syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelRuleSyntax {
    trigger: TriggerPattern,
    action: Vec<Stmt>,
}

/// Shared trigger syntax used by `on`, `cancel on`, and hook-like filters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TriggerPattern {
    Input(Pattern),
    Event(Pattern),
    Signal {
        target: Expr,
        value: Option<Pattern>,
    },
    Timeout(Expr),
    Mark(Pattern),
    Select(Pattern),
    Task(Pattern),
    Scope(Pattern),
    Expr(Expr),
}

/// Exit outcome guard attached to scoped cleanup.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DeferOutcome {
    /// Run for completed, cancelled, and failed scope exits.
    #[default]
    Always,
    /// Run only when the scope exits normally.
    Completed,
    /// Run only when the scope exits through a cancellation path.
    Cancelled,
    /// Run only when the scope exits through a failure path.
    Failed,
}

impl LinePlan {
    pub(crate) const fn new(style: BlockStyle, items: Vec<LinePlanItem>, range: TextRange) -> Self {
        Self {
            style,
            label: None,
            items,
            range,
        }
    }

    pub(crate) fn with_label(mut self, label: String) -> Self {
        self.label = Some(label);
        self
    }

    pub const fn style(&self) -> BlockStyle {
        self.style
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn items(&self) -> &[LinePlanItem] {
        &self.items
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl CancelRuleSyntax {
    pub(crate) const fn new(trigger: TriggerPattern, action: Vec<Stmt>) -> Self {
        Self { trigger, action }
    }

    pub const fn trigger(&self) -> &TriggerPattern {
        &self.trigger
    }

    pub fn action(&self) -> &[Stmt] {
        &self.action
    }
}

impl TriggerPattern {
    pub fn label(&self) -> String {
        match self {
            Self::Input(pattern) => format!("input {}", pattern_label(pattern)),
            Self::Event(pattern) => format!("event {}", pattern_label(pattern)),
            Self::Signal { target, value } => value.as_ref().map_or_else(
                || format!("signal {}", expr_label(target)),
                |value| format!("signal {} {}", expr_label(target), pattern_label(value)),
            ),
            Self::Timeout(expr) => format!("timeout {}", expr_label(expr)),
            Self::Mark(pattern) => format!("mark {}", pattern_label(pattern)),
            Self::Select(pattern) => format!("select {}", pattern_label(pattern)),
            Self::Task(pattern) => format!("task {}", pattern_label(pattern)),
            Self::Scope(pattern) => format!("scope {}", pattern_label(pattern)),
            Self::Expr(expr) => expr_label(expr),
        }
    }
}

fn expr_label(expr: &Expr) -> String {
    match expr {
        Expr::Path(path) => path.clone(),
        Expr::EntityRef(entity) => entity.body().to_owned(),
        Expr::Literal(literal) => format!("{literal:?}"),
        _ => format!("{expr:?}"),
    }
}

fn pattern_label(pattern: &Pattern) -> String {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) => name.clone(),
        Pattern::Variant { name, .. } if name.starts_with('.') => name.clone(),
        Pattern::Variant {
            path: None,
            name,
            payload: None,
        } => format!(".{name}"),
        Pattern::Entity(entity) => entity.body().to_owned(),
        Pattern::Discard => "_".to_owned(),
        _ => format!("{pattern:?}"),
    }
}
