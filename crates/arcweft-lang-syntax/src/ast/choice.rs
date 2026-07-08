use crate::expr::Expr;

use super::common::TextRange;
use super::flow::Stmt;
use super::ids::{EntityRefSyntax, IdRef};
use super::items::RawSyntax;
use super::line_plan::{BlockStyle, TriggerPattern};
use super::pattern::Pattern;

/// `choice @choice.id { ... }` flow item with option rows.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceBlock {
    id: Option<IdRef>,
    items: Vec<ChoiceItem>,
    options: Vec<ChoiceOption>,
    plan: Option<ChoicePlan>,
    range: TextRange,
}

/// Choice lifecycle plan attached with `with { ... }` or `with:`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoicePlan {
    style: BlockStyle,
    items: Vec<ChoicePlanItem>,
    range: TextRange,
}

/// Item inside a choice lifecycle plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChoicePlanItem {
    Option {
        name: String,
        value: Expr,
    },
    Timeout {
        duration: Expr,
        body: Vec<Stmt>,
    },
    Cancel {
        trigger: TriggerPattern,
        body: Vec<Stmt>,
    },
    OnSelect {
        pattern: Pattern,
        body: Vec<Stmt>,
    },
    Raw(RawSyntax),
}

/// Item inside a choice body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChoiceItem {
    Let {
        pattern: Pattern,
        expr: Expr,
    },
    If {
        condition: Expr,
        items: Vec<ChoiceItem>,
    },
    For {
        pattern: Pattern,
        source: Expr,
        items: Vec<ChoiceItem>,
    },
    Match {
        expr: Expr,
        arms: Vec<ChoiceMatchArm>,
    },
    Option(Box<ChoiceOption>),
    Raw(RawSyntax),
}

/// One branch of a `match` item inside a choice body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceMatchArm {
    pattern: Pattern,
    guard: Option<Expr>,
    items: Vec<ChoiceItem>,
}

/// One option in a choice block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceOption {
    id: Option<IdRef>,
    id_expr: Option<Expr>,
    label: String,
    label_text_key: Option<IdRef>,
    value: Option<Expr>,
    enabled: Option<Expr>,
    visible: Option<Expr>,
    order: Option<Expr>,
    hotkey: Option<Expr>,
    view_fields: Vec<ChoiceViewField>,
    action: ChoiceAction,
    range: TextRange,
}

/// View state propagated from a choice option to rendering, accessibility, and Agent observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceViewField {
    name: String,
    value: Expr,
}

/// Action performed by a selected choice option.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChoiceAction {
    Goto(EntityRefSyntax),
    Out(Expr),
    SelectBlock(Vec<Stmt>),
    None,
}

impl ChoiceOption {
    pub(crate) const fn new(
        id: Option<IdRef>,
        label: String,
        action: ChoiceAction,
        range: TextRange,
    ) -> Self {
        Self {
            id,
            id_expr: None,
            label,
            label_text_key: None,
            value: None,
            enabled: None,
            visible: None,
            order: None,
            hotkey: None,
            view_fields: Vec::new(),
            action,
            range,
        }
    }

    pub(crate) fn with_id_expr(mut self, id_expr: Expr) -> Self {
        self.id_expr = Some(id_expr);
        self
    }

    pub(crate) fn with_enabled(mut self, enabled: Expr) -> Self {
        self.enabled = Some(enabled);
        self
    }

    pub(crate) fn with_label_text_key(mut self, text_key: IdRef) -> Self {
        self.label_text_key = Some(text_key);
        self
    }

    pub(crate) fn with_value(mut self, value: Expr) -> Self {
        self.value = Some(value);
        self
    }

    pub(crate) fn with_visible(mut self, visible: Expr) -> Self {
        self.visible = Some(visible);
        self
    }

    pub(crate) fn with_order(mut self, order: Expr) -> Self {
        self.order = Some(order);
        self
    }

    pub(crate) fn with_hotkey(mut self, hotkey: Expr) -> Self {
        self.hotkey = Some(hotkey);
        self
    }

    pub(crate) fn with_view_fields(mut self, view_fields: Vec<ChoiceViewField>) -> Self {
        self.view_fields = view_fields;
        self
    }

    pub const fn id(&self) -> Option<&IdRef> {
        self.id.as_ref()
    }

    pub const fn id_expr(&self) -> Option<&Expr> {
        self.id_expr.as_ref()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub const fn label_text_key(&self) -> Option<&IdRef> {
        self.label_text_key.as_ref()
    }

    pub const fn value(&self) -> Option<&Expr> {
        self.value.as_ref()
    }

    pub const fn condition(&self) -> Option<&Expr> {
        self.enabled.as_ref()
    }

    pub const fn enabled(&self) -> Option<&Expr> {
        self.enabled.as_ref()
    }

    pub const fn visible(&self) -> Option<&Expr> {
        self.visible.as_ref()
    }

    pub const fn order(&self) -> Option<&Expr> {
        self.order.as_ref()
    }

    pub const fn hotkey(&self) -> Option<&Expr> {
        self.hotkey.as_ref()
    }

    pub fn view_fields(&self) -> &[ChoiceViewField] {
        &self.view_fields
    }

    pub const fn action(&self) -> &ChoiceAction {
        &self.action
    }

    pub const fn target(&self) -> Option<&EntityRefSyntax> {
        match &self.action {
            ChoiceAction::Goto(target) => Some(target),
            _ => None,
        }
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ChoiceViewField {
    pub(crate) const fn new(name: String, value: Expr) -> Self {
        Self { name, value }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn value(&self) -> &Expr {
        &self.value
    }
}

impl ChoiceBlock {
    pub(crate) fn new(
        id: Option<IdRef>,
        items: Vec<ChoiceItem>,
        plan: Option<ChoicePlan>,
        range: TextRange,
    ) -> Self {
        let options = collect_choice_options(&items);
        Self {
            id,
            items,
            options,
            plan,
            range,
        }
    }

    pub const fn id(&self) -> Option<&IdRef> {
        self.id.as_ref()
    }

    pub fn options(&self) -> &[ChoiceOption] {
        &self.options
    }

    pub fn items(&self) -> &[ChoiceItem] {
        &self.items
    }

    pub const fn plan(&self) -> Option<&ChoicePlan> {
        self.plan.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ChoicePlan {
    pub(crate) const fn new(
        style: BlockStyle,
        items: Vec<ChoicePlanItem>,
        range: TextRange,
    ) -> Self {
        Self {
            style,
            items,
            range,
        }
    }

    pub const fn style(&self) -> BlockStyle {
        self.style
    }

    pub fn items(&self) -> &[ChoicePlanItem] {
        &self.items
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

fn collect_choice_options(items: &[ChoiceItem]) -> Vec<ChoiceOption> {
    items
        .iter()
        .flat_map(|item| match item {
            ChoiceItem::Option(option) => vec![option.as_ref().clone()],
            ChoiceItem::If { items, .. } | ChoiceItem::For { items, .. } => {
                collect_choice_options(items)
            }
            ChoiceItem::Match { arms, .. } => arms
                .iter()
                .flat_map(|arm| collect_choice_options(arm.items()))
                .collect(),
            ChoiceItem::Let { .. } | ChoiceItem::Raw(_) => Vec::new(),
        })
        .collect()
}

impl ChoiceMatchArm {
    pub(crate) const fn new(pattern: Pattern, guard: Option<Expr>, items: Vec<ChoiceItem>) -> Self {
        Self {
            pattern,
            guard,
            items,
        }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn guard(&self) -> Option<&Expr> {
        self.guard.as_ref()
    }

    pub fn items(&self) -> &[ChoiceItem] {
        &self.items
    }
}
