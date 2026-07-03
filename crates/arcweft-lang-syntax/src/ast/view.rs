//! Syntax-level View DSL nodes for Arcweft Components.
//!
//! These nodes are intentionally still syntax/HIR-facing. They preserve source
//! structure for diagnostics and lowering without depending on runtime, UI, or
//! renderer crates.

use crate::ast::common::TextRange;
use crate::ast::ids::EntityRefSyntax;
use crate::ast::pattern::Pattern;
use crate::expr::Expr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentViewBody {
    locals: Vec<ViewLocalState>,
    stylesheets: Vec<EntityRefSyntax>,
    value: ViewExpr,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewLocalState {
    name: String,
    ty: Option<String>,
    initial: Expr,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewExpr {
    Fragment(Vec<ViewExpr>),
    Element(ViewElement),
    ComponentCall(ViewComponentCall),
    Text(ViewText),
    Image(ViewImage),
    TextField(ViewTextField),
    Button(ViewButton),
    If(ViewIf),
    Match(ViewMatch),
    ForEach(ViewForEach),
    Await(ViewAwait),
    Expr(Expr),
    Raw(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewElement {
    callee: String,
    args: Vec<ViewArg>,
    children: Vec<ViewExpr>,
    modifiers: Vec<ViewModifier>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewComponentCall {
    component: Expr,
    args: Vec<ViewArg>,
    modifiers: Vec<ViewModifier>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewText {
    source: Expr,
    rich_surface: Option<String>,
    modifiers: Vec<ViewModifier>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewImage {
    source: Expr,
    modifiers: Vec<ViewModifier>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewTextField {
    value: Expr,
    mode: ViewTextFieldMode,
    args: Vec<ViewArg>,
    modifiers: Vec<ViewModifier>,
    input: Option<EntityRefSyntax>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewButton {
    label: ViewButtonLabel,
    id: Option<EntityRefSyntax>,
    enabled: Option<Expr>,
    focusable: bool,
    modifiers: Vec<ViewModifier>,
    activation: Option<ViewAction>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewButtonLabel {
    Literal(String),
    Expr(Expr),
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewAction {
    TextSubmit(ViewTextSubmitAction),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewTextSubmitAction {
    input: EntityRefSyntax,
    ime_policy: ViewTextSubmitImePolicy,
    range: TextRange,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewTextSubmitImePolicy {
    #[default]
    Commit,
    Cancel,
    Reject,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewTextFieldMode {
    #[default]
    TextField,
    TextArea,
    SecureField,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewIf {
    condition: Expr,
    then_branch: Box<ViewExpr>,
    else_branch: Option<Box<ViewExpr>>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewMatch {
    scrutinee: Expr,
    arms: Vec<ViewMatchArm>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewMatchArm {
    pattern: Pattern,
    guard: Option<Expr>,
    value: ViewExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewForEach {
    pattern: Pattern,
    source: Expr,
    key: Option<Expr>,
    body: Box<ViewExpr>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewAwait {
    source: Expr,
    branches: Vec<ViewAwaitBranch>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewAwaitBranch {
    pattern: Pattern,
    value: ViewExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewModifier {
    Style(ViewStyleModifier),
    Part(String),
    AgentTarget(EntityRefSyntax),
    Placeholder(Expr),
    SubmitAction(Expr),
    Enabled(Expr),
    Focusable(bool),
    OnEvent {
        name: String,
        body: Expr,
        ime_policy: Option<ViewTextSubmitImePolicy>,
    },
    Environment(Vec<ViewArg>),
    Focus(String),
    Raw(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewStyleModifier {
    Named(EntityRefSyntax),
    InlineArcweft(String),
    InlineCss(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewArg {
    Positional(Expr),
    Named { name: String, value: Expr },
}

impl ComponentViewBody {
    pub const fn new(
        locals: Vec<ViewLocalState>,
        stylesheets: Vec<EntityRefSyntax>,
        value: ViewExpr,
        range: TextRange,
    ) -> Self {
        Self {
            locals,
            stylesheets,
            value,
            range,
        }
    }

    pub fn locals(&self) -> &[ViewLocalState] {
        &self.locals
    }

    pub fn stylesheets(&self) -> &[EntityRefSyntax] {
        &self.stylesheets
    }

    pub const fn value(&self) -> &ViewExpr {
        &self.value
    }

    pub fn text_control_inputs(&self) -> Vec<&EntityRefSyntax> {
        let mut inputs = Vec::new();
        collect_text_control_inputs(&self.value, &mut inputs);
        inputs
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewElement {
    pub const fn new(
        callee: String,
        args: Vec<ViewArg>,
        children: Vec<ViewExpr>,
        modifiers: Vec<ViewModifier>,
        range: TextRange,
    ) -> Self {
        Self {
            callee,
            args,
            children,
            modifiers,
            range,
        }
    }

    pub fn callee(&self) -> &str {
        &self.callee
    }

    pub fn args(&self) -> &[ViewArg] {
        &self.args
    }

    pub fn children(&self) -> &[ViewExpr] {
        &self.children
    }

    pub fn modifiers(&self) -> &[ViewModifier] {
        &self.modifiers
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewComponentCall {
    pub const fn new(
        component: Expr,
        args: Vec<ViewArg>,
        modifiers: Vec<ViewModifier>,
        range: TextRange,
    ) -> Self {
        Self {
            component,
            args,
            modifiers,
            range,
        }
    }

    pub const fn component(&self) -> &Expr {
        &self.component
    }

    pub fn args(&self) -> &[ViewArg] {
        &self.args
    }

    pub fn modifiers(&self) -> &[ViewModifier] {
        &self.modifiers
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewImage {
    pub const fn new(source: Expr, modifiers: Vec<ViewModifier>, range: TextRange) -> Self {
        Self {
            source,
            modifiers,
            range,
        }
    }

    pub const fn source(&self) -> &Expr {
        &self.source
    }

    pub fn modifiers(&self) -> &[ViewModifier] {
        &self.modifiers
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewText {
    pub const fn new(source: Expr, modifiers: Vec<ViewModifier>, range: TextRange) -> Self {
        Self {
            source,
            rich_surface: None,
            modifiers,
            range,
        }
    }

    #[must_use]
    pub fn with_rich_surface(mut self, rich_surface: impl Into<String>) -> Self {
        self.rich_surface = Some(rich_surface.into());
        self
    }

    pub const fn source(&self) -> &Expr {
        &self.source
    }
    pub const fn rich_surface(&self) -> Option<&String> {
        self.rich_surface.as_ref()
    }
    pub fn modifiers(&self) -> &[ViewModifier] {
        &self.modifiers
    }
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewTextField {
    pub const fn new(
        value: Expr,
        mode: ViewTextFieldMode,
        args: Vec<ViewArg>,
        modifiers: Vec<ViewModifier>,
        range: TextRange,
    ) -> Self {
        Self {
            value,
            mode,
            args,
            modifiers,
            input: None,
            range,
        }
    }

    #[must_use]
    pub fn with_input(mut self, input: EntityRefSyntax) -> Self {
        self.input = Some(input);
        self
    }

    pub const fn value(&self) -> &Expr {
        &self.value
    }
    pub const fn mode(&self) -> ViewTextFieldMode {
        self.mode
    }
    pub fn args(&self) -> &[ViewArg] {
        &self.args
    }
    pub fn modifiers(&self) -> &[ViewModifier] {
        &self.modifiers
    }
    pub const fn input(&self) -> Option<&EntityRefSyntax> {
        self.input.as_ref()
    }
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewButton {
    pub const fn new(
        label: ViewButtonLabel,
        modifiers: Vec<ViewModifier>,
        range: TextRange,
    ) -> Self {
        Self {
            label,
            id: None,
            enabled: None,
            focusable: true,
            modifiers,
            activation: None,
            range,
        }
    }

    #[must_use]
    pub fn with_id(mut self, id: Option<EntityRefSyntax>) -> Self {
        self.id = id;
        self
    }

    #[must_use]
    pub fn with_enabled(mut self, enabled: Option<Expr>) -> Self {
        self.enabled = enabled;
        self
    }

    #[must_use]
    pub const fn with_focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    #[must_use]
    pub fn with_activation(mut self, activation: Option<ViewAction>) -> Self {
        self.activation = activation;
        self
    }

    pub const fn label(&self) -> &ViewButtonLabel {
        &self.label
    }

    pub const fn id(&self) -> Option<&EntityRefSyntax> {
        self.id.as_ref()
    }

    pub const fn enabled(&self) -> Option<&Expr> {
        self.enabled.as_ref()
    }

    pub const fn focusable(&self) -> bool {
        self.focusable
    }

    pub fn modifiers(&self) -> &[ViewModifier] {
        &self.modifiers
    }

    pub const fn activation(&self) -> Option<&ViewAction> {
        self.activation.as_ref()
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewTextSubmitAction {
    pub const fn new(
        input: EntityRefSyntax,
        ime_policy: ViewTextSubmitImePolicy,
        range: TextRange,
    ) -> Self {
        Self {
            input,
            ime_policy,
            range,
        }
    }

    pub const fn input(&self) -> &EntityRefSyntax {
        &self.input
    }

    pub const fn ime_policy(&self) -> ViewTextSubmitImePolicy {
        self.ime_policy
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewStyleModifier {
    pub fn named(name: EntityRefSyntax) -> Self {
        Self::Named(name)
    }
    pub fn inline_arcweft(source: impl Into<String>) -> Self {
        Self::InlineArcweft(source.into())
    }
    pub fn inline_css(source: impl Into<String>) -> Self {
        Self::InlineCss(source.into())
    }

    pub const fn syntax_name(&self) -> Option<&'static str> {
        match self {
            Self::Named(_) => None,
            Self::InlineArcweft(_) => Some("Arcweft"),
            Self::InlineCss(_) => Some("Css"),
        }
    }
}

impl ViewModifier {
    pub fn style_arcweft(source: impl Into<String>) -> Self {
        Self::Style(ViewStyleModifier::inline_arcweft(source))
    }

    pub fn style_css(source: impl Into<String>) -> Self {
        Self::Style(ViewStyleModifier::inline_css(source))
    }
}

fn collect_text_control_inputs<'a>(expr: &'a ViewExpr, inputs: &mut Vec<&'a EntityRefSyntax>) {
    match expr {
        ViewExpr::Fragment(children) => {
            for child in children {
                collect_text_control_inputs(child, inputs);
            }
        }
        ViewExpr::Element(element) => {
            for child in element.children() {
                collect_text_control_inputs(child, inputs);
            }
        }
        ViewExpr::TextField(field) => {
            if let Some(input) = field.input() {
                inputs.push(input);
            }
        }
        ViewExpr::ComponentCall(_)
        | ViewExpr::Text(_)
        | ViewExpr::Image(_)
        | ViewExpr::Button(_)
        | ViewExpr::If(_)
        | ViewExpr::Match(_)
        | ViewExpr::ForEach(_)
        | ViewExpr::Await(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => {}
    }
}
