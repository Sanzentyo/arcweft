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
    range: TextRange,
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
    OnEvent { name: String, body: Expr },
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
            range,
        }
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
