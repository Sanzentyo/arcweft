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
