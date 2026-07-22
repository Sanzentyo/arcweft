//! Syntax-level View DSL nodes for Arcweft views.
//!
//! These nodes are intentionally still syntax/HIR-facing. They preserve source
//! structure for diagnostics and lowering without depending on runtime, View, or
//! renderer crates.

use crate::ast::common::TextRange;
use crate::ast::flow::Stmt;
use crate::ast::ids::EntityRefSyntax;
use crate::ast::pattern::Pattern;
use crate::ast::style::StylePatch;
use crate::expr::{CallArg, Expr, Literal, MatchExprArm};

mod fx;
mod part;
mod recovery;

pub use fx::{ViewFxApplication, ViewFxApplicationOrdinal};
pub use part::{ViewPartExportDecl, ViewPartLocalNameSyntax, ViewPartModifier, ViewPartNameSyntax};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewBody {
    locals: Vec<ViewLocalState>,
    stylesheets: Vec<EntityRefSyntax>,
    exports: Vec<ViewPartExportDecl>,
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
    ViewCall(ViewCall),
    Text(ViewText),
    Image(ViewImage),
    TextField(ViewTextField),
    Button(ViewButton),
    Let(ViewLet),
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
pub struct ViewCall {
    view: Expr,
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
    submit_action: Option<ViewAction>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewButton {
    label: ViewButtonLabel,
    args: Vec<ViewArg>,
    id: Option<EntityRefSyntax>,
    enabled: Option<Expr>,
    focusable: bool,
    modifiers: Vec<ViewModifier>,
    activation: Option<ViewAction>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewLet {
    pattern: Pattern,
    value: Expr,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewButtonLabel {
    Literal(String),
    Expr(Box<Expr>),
    Empty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewAction {
    Noop,
    ActionInvoke(ViewActionInvokeAction),
    /// Typed action value projected from a View parameter.
    Projection(Expr),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewActionPayload {
    LiteralString(String),
    TextControlProjection {
        input: String,
        field: ViewTextControlPayloadField,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewTextControlPayloadField {
    Text,
    Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewActionInvokeAction {
    action: EntityRefSyntax,
    payload_name: Option<Box<str>>,
    payload: Option<ViewActionPayload>,
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
    range: TextRange,
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
    kind: ViewAwaitBranchKind,
    pattern: Pattern,
    value: ViewExpr,
    range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewAwaitBranchKind {
    Pending,
    Ready,
    Error,
    Denied,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewModifier {
    Style(ViewStyleModifier),
    /// Applies one typed `#[fx] fn -> Fx` call to the preceding View value.
    Fx(Box<ViewFxApplication>),
    Part(ViewPartModifier),
    Label(Expr),
    AgentTarget(EntityRefSyntax),
    Placeholder(Expr),
    Purpose(Expr),
    EnterKey(Expr),
    Enabled(Expr),
    Focusable(bool),
    Property {
        name: String,
        value: Expr,
    },
    OnEvent {
        name: String,
        body: Expr,
    },
    Environment(Vec<ViewArg>),
    Focus(String),
    Navigation(ViewNavigationModifier),
    Raw(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewNavigationModifier {
    edges: Vec<ViewNavigationEdge>,
    range: TextRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewNavigationEdge {
    direction: ViewNavigationDirection,
    target: ViewNavigationTarget,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewNavigationDirection {
    Up,
    Down,
    Left,
    Right,
    Next,
    Previous,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewNavigationTarget {
    Explicit(EntityRefSyntax),
    Auto,
    None,
    GroupBoundary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewNavigationGroup {
    group: Option<EntityRefSyntax>,
    parent: Option<EntityRefSyntax>,
    axis: ViewNavigationAxis,
    wrap: Option<bool>,
    initial: ViewNavigationInitial,
    trap: ViewNavigationTrap,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewNavigationAxis {
    #[default]
    Auto,
    Horizontal,
    Vertical,
    Grid,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ViewNavigationInitial {
    #[default]
    Auto,
    First,
    Last,
    Explicit(EntityRefSyntax),
    None,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewNavigationTrap {
    #[default]
    Normal,
    Trap,
    Modal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewStyleModifier {
    Named(EntityRefSyntax),
    Inline(StylePatch),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewArg {
    Positional(Expr),
    Named { name: String, value: Expr },
}

impl ViewBody {
    pub const fn new(
        locals: Vec<ViewLocalState>,
        stylesheets: Vec<EntityRefSyntax>,
        exports: Vec<ViewPartExportDecl>,
        value: ViewExpr,
        range: TextRange,
    ) -> Self {
        Self {
            locals,
            stylesheets,
            exports,
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

    pub fn exports(&self) -> &[ViewPartExportDecl] {
        &self.exports
    }

    pub const fn value(&self) -> &ViewExpr {
        &self.value
    }

    pub fn text_control_inputs(&self) -> Vec<&EntityRefSyntax> {
        let mut inputs = Vec::new();
        collect_text_control_inputs(&self.value, &mut inputs);
        inputs
    }

    pub fn action_invokes(&self) -> Vec<ViewActionInvokeAction> {
        let mut invokes = Vec::new();
        collect_action_invokes(&self.value, &mut invokes);
        invokes
    }

    /// Returns typed action-value projections used by View controls.
    pub fn action_projections(&self) -> Vec<&Expr> {
        let mut projections = Vec::new();
        collect_action_projections(&self.value, &mut projections);
        projections
    }

    /// Returns nested View calls in authored depth-first order.
    pub fn view_calls(&self) -> Vec<&ViewCall> {
        let mut calls = Vec::new();
        collect_view_calls(&self.value, &mut calls);
        calls
    }

    /// Returns text leaves in authored depth-first order.
    pub fn text_nodes(&self) -> Vec<&ViewText> {
        let mut text = Vec::new();
        collect_text_nodes(&self.value, &mut text);
        text
    }

    /// Returns View-side Fx applications in authored depth-first order.
    pub fn fx_applications(&self) -> Vec<&ViewFxApplication> {
        let mut applications = Vec::new();
        fx::collect_fx_applications(&self.value, &mut applications);
        applications
    }

    /// Returns inline style patches in authored depth-first order.
    pub fn style_patches(&self) -> Vec<&StylePatch> {
        let mut patches = Vec::new();
        collect_style_patches(&self.value, &mut patches);
        patches
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Whether parser recovery remains anywhere in this retained View tree.
    ///
    /// Recovery nodes are useful to syntax tooling, but they are never an
    /// executable View contract and must not enter an accepted compiler
    /// product.
    pub fn contains_recovered_syntax(&self) -> bool {
        recovery::contains_recovered_syntax(self)
    }
}

impl ViewLocalState {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn ty(&self) -> Option<&str> {
        self.ty.as_deref()
    }

    pub const fn initial(&self) -> &Expr {
        &self.initial
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

    pub fn navigation_group(&self) -> Option<ViewNavigationGroup> {
        ViewNavigationGroup::from_args(&self.args)
    }
}

impl ViewCall {
    pub const fn new(
        view: Expr,
        args: Vec<ViewArg>,
        modifiers: Vec<ViewModifier>,
        range: TextRange,
    ) -> Self {
        Self {
            view,
            args,
            modifiers,
            range,
        }
    }

    pub const fn view(&self) -> &Expr {
        &self.view
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
            submit_action: None,
            range,
        }
    }

    #[must_use]
    pub fn with_input(mut self, input: EntityRefSyntax) -> Self {
        self.input = Some(input);
        self
    }

    #[must_use]
    pub fn with_submit_action(mut self, submit_action: Option<ViewAction>) -> Self {
        self.submit_action = submit_action;
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
    pub const fn submit_action(&self) -> Option<&ViewAction> {
        self.submit_action.as_ref()
    }
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewButton {
    pub const fn new(
        label: ViewButtonLabel,
        args: Vec<ViewArg>,
        modifiers: Vec<ViewModifier>,
        range: TextRange,
    ) -> Self {
        Self {
            label,
            args,
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

    pub fn args(&self) -> &[ViewArg] {
        &self.args
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

impl ViewActionInvokeAction {
    pub fn new(
        action: EntityRefSyntax,
        payload_name: Option<String>,
        payload: Option<ViewActionPayload>,
        range: TextRange,
    ) -> Self {
        Self {
            action,
            payload_name: payload_name.map(String::into_boxed_str),
            payload,
            range,
        }
    }

    pub const fn action(&self) -> &EntityRefSyntax {
        &self.action
    }

    pub fn payload_name(&self) -> Option<&str> {
        self.payload_name.as_deref()
    }

    pub const fn payload(&self) -> Option<&ViewActionPayload> {
        self.payload.as_ref()
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewLet {
    pub(crate) const fn new(pattern: Pattern, value: Expr, range: TextRange) -> Self {
        Self {
            pattern,
            value,
            range,
        }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn value(&self) -> &Expr {
        &self.value
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }

    pub fn input_handle(&self) -> Option<&EntityRefSyntax> {
        input_handle_entity(&self.value)
    }
}

impl ViewIf {
    pub(crate) const fn new(
        condition: Expr,
        then_branch: Box<ViewExpr>,
        else_branch: Option<Box<ViewExpr>>,
        range: TextRange,
    ) -> Self {
        Self {
            condition,
            then_branch,
            else_branch,
            range,
        }
    }

    pub const fn condition(&self) -> &Expr {
        &self.condition
    }

    pub const fn then_branch(&self) -> &ViewExpr {
        &self.then_branch
    }

    pub fn else_branch(&self) -> Option<&ViewExpr> {
        self.else_branch.as_deref()
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewMatch {
    pub(crate) const fn new(scrutinee: Expr, arms: Vec<ViewMatchArm>, range: TextRange) -> Self {
        Self {
            scrutinee,
            arms,
            range,
        }
    }

    pub const fn scrutinee(&self) -> &Expr {
        &self.scrutinee
    }

    pub fn arms(&self) -> &[ViewMatchArm] {
        &self.arms
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewMatchArm {
    pub(crate) const fn new(
        pattern: Pattern,
        guard: Option<Expr>,
        value: ViewExpr,
        range: TextRange,
    ) -> Self {
        Self {
            pattern,
            guard,
            value,
            range,
        }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn guard(&self) -> Option<&Expr> {
        self.guard.as_ref()
    }

    pub const fn value(&self) -> &ViewExpr {
        &self.value
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewForEach {
    pub(crate) const fn new(
        pattern: Pattern,
        source: Expr,
        key: Option<Expr>,
        body: Box<ViewExpr>,
        range: TextRange,
    ) -> Self {
        Self {
            pattern,
            source,
            key,
            body,
            range,
        }
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn source(&self) -> &Expr {
        &self.source
    }

    pub const fn key(&self) -> Option<&Expr> {
        self.key.as_ref()
    }

    pub const fn body(&self) -> &ViewExpr {
        &self.body
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewAwait {
    pub(crate) const fn new(
        source: Expr,
        branches: Vec<ViewAwaitBranch>,
        range: TextRange,
    ) -> Self {
        Self {
            source,
            branches,
            range,
        }
    }

    pub const fn source(&self) -> &Expr {
        &self.source
    }

    pub fn branches(&self) -> &[ViewAwaitBranch] {
        &self.branches
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewAwaitBranch {
    pub(crate) const fn new(
        kind: ViewAwaitBranchKind,
        pattern: Pattern,
        value: ViewExpr,
        range: TextRange,
    ) -> Self {
        Self {
            kind,
            pattern,
            value,
            range,
        }
    }

    pub const fn kind(&self) -> ViewAwaitBranchKind {
        self.kind
    }

    pub const fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    pub const fn value(&self) -> &ViewExpr {
        &self.value
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewStyleModifier {
    pub fn named(name: EntityRefSyntax) -> Self {
        Self::Named(name)
    }
    pub const fn inline(patch: StylePatch) -> Self {
        Self::Inline(patch)
    }
}

impl ViewModifier {
    pub const fn style_inline(patch: StylePatch) -> Self {
        Self::Style(ViewStyleModifier::inline(patch))
    }
}

impl ViewNavigationModifier {
    pub const fn new(edges: Vec<ViewNavigationEdge>, range: TextRange) -> Self {
        Self { edges, range }
    }

    pub fn edges(&self) -> &[ViewNavigationEdge] {
        &self.edges
    }

    pub const fn range(&self) -> TextRange {
        self.range
    }
}

impl ViewNavigationEdge {
    pub const fn new(direction: ViewNavigationDirection, target: ViewNavigationTarget) -> Self {
        Self { direction, target }
    }

    pub const fn direction(&self) -> ViewNavigationDirection {
        self.direction
    }

    pub const fn target(&self) -> &ViewNavigationTarget {
        &self.target
    }
}

impl ViewNavigationGroup {
    pub fn from_args(args: &[ViewArg]) -> Option<Self> {
        let axis = args
            .iter()
            .find_map(|arg| named_raw_arg(arg, "nav"))
            .and_then(parse_axis)?;
        Some(Self {
            group: args.iter().find_map(|arg| named_entity_arg(arg, "group")),
            parent: args.iter().find_map(|arg| named_entity_arg(arg, "parent")),
            axis,
            wrap: args.iter().find_map(|arg| named_bool_arg(arg, "wrap")),
            initial: args
                .iter()
                .find_map(|arg| named_initial_arg(arg, "initial"))
                .unwrap_or_default(),
            trap: args
                .iter()
                .find_map(|arg| named_raw_arg(arg, "trap"))
                .and_then(parse_trap)
                .unwrap_or_default(),
        })
    }

    pub const fn group(&self) -> Option<&EntityRefSyntax> {
        self.group.as_ref()
    }

    pub const fn parent(&self) -> Option<&EntityRefSyntax> {
        self.parent.as_ref()
    }

    pub const fn axis(&self) -> ViewNavigationAxis {
        self.axis
    }

    pub const fn wrap(&self) -> Option<bool> {
        self.wrap
    }

    pub const fn initial(&self) -> &ViewNavigationInitial {
        &self.initial
    }

    pub const fn trap(&self) -> ViewNavigationTrap {
        self.trap
    }
}

fn named_raw_arg<'a>(arg: &'a ViewArg, name: &str) -> Option<&'a str> {
    match arg {
        ViewArg::Named {
            name: actual,
            value: Expr::Raw(value),
        } if actual == name => Some(value.as_str()),
        ViewArg::Named {
            name: actual,
            value: Expr::Path(value),
        } if actual == name => Some(value.as_label()),
        _ => None,
    }
}

fn named_entity_arg(arg: &ViewArg, name: &str) -> Option<EntityRefSyntax> {
    match arg {
        ViewArg::Named {
            name: actual,
            value: Expr::EntityRef(value),
        } if actual == name => Some(value.clone()),
        _ => None,
    }
}

fn named_bool_arg(arg: &ViewArg, name: &str) -> Option<bool> {
    match arg {
        ViewArg::Named {
            name: actual,
            value: Expr::Literal(crate::expr::Literal::Bool(value)),
        } if actual == name => Some(*value),
        _ => None,
    }
}

fn named_initial_arg(arg: &ViewArg, name: &str) -> Option<ViewNavigationInitial> {
    match arg {
        ViewArg::Named {
            name: actual,
            value,
        } if actual == name => match value {
            Expr::EntityRef(value) => Some(ViewNavigationInitial::Explicit(value.clone())),
            Expr::Raw(value) => parse_initial(value),
            Expr::Path(value) => parse_initial(value.as_label()),
            Expr::ShortVariant(value) => parse_initial(value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn parse_axis(value: &str) -> Option<ViewNavigationAxis> {
    match value.trim().trim_start_matches('.') {
        "auto" => Some(ViewNavigationAxis::Auto),
        "horizontal" => Some(ViewNavigationAxis::Horizontal),
        "vertical" => Some(ViewNavigationAxis::Vertical),
        "grid" => Some(ViewNavigationAxis::Grid),
        _ => None,
    }
}

fn parse_initial(value: &str) -> Option<ViewNavigationInitial> {
    match value.trim().trim_start_matches('.') {
        "auto" => Some(ViewNavigationInitial::Auto),
        "first" => Some(ViewNavigationInitial::First),
        "last" => Some(ViewNavigationInitial::Last),
        "none" => Some(ViewNavigationInitial::None),
        _ => None,
    }
}

fn parse_trap(value: &str) -> Option<ViewNavigationTrap> {
    match value.trim().trim_start_matches('.') {
        "normal" => Some(ViewNavigationTrap::Normal),
        "trap" => Some(ViewNavigationTrap::Trap),
        "modal" => Some(ViewNavigationTrap::Modal),
        _ => None,
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
        ViewExpr::Let(view_let) => {
            if let Some(input) = view_let.input_handle() {
                inputs.push(input);
            }
        }
        ViewExpr::If(view_if) => {
            collect_text_control_inputs(view_if.then_branch(), inputs);
            if let Some(else_branch) = view_if.else_branch() {
                collect_text_control_inputs(else_branch, inputs);
            }
        }
        ViewExpr::Match(view_match) => {
            for arm in view_match.arms() {
                collect_text_control_inputs(arm.value(), inputs);
            }
        }
        ViewExpr::ForEach(view_for_each) => {
            collect_text_control_inputs(view_for_each.body(), inputs);
        }
        ViewExpr::Await(view_await) => {
            for branch in view_await.branches() {
                collect_text_control_inputs(branch.value(), inputs);
            }
        }
        ViewExpr::ViewCall(_)
        | ViewExpr::Text(_)
        | ViewExpr::Image(_)
        | ViewExpr::Button(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => {}
    }
}

fn collect_view_calls<'a>(expr: &'a ViewExpr, calls: &mut Vec<&'a ViewCall>) {
    match expr {
        ViewExpr::ViewCall(call) => calls.push(call),
        ViewExpr::Fragment(children) => {
            for child in children {
                collect_view_calls(child, calls);
            }
        }
        ViewExpr::Element(element) => {
            for child in element.children() {
                collect_view_calls(child, calls);
            }
        }
        ViewExpr::If(view_if) => {
            collect_view_calls(view_if.then_branch(), calls);
            if let Some(else_branch) = view_if.else_branch() {
                collect_view_calls(else_branch, calls);
            }
        }
        ViewExpr::Match(view_match) => {
            for arm in view_match.arms() {
                collect_view_calls(arm.value(), calls);
            }
        }
        ViewExpr::ForEach(view_for_each) => collect_view_calls(view_for_each.body(), calls),
        ViewExpr::Await(view_await) => {
            for branch in view_await.branches() {
                collect_view_calls(branch.value(), calls);
            }
        }
        ViewExpr::Text(_)
        | ViewExpr::Image(_)
        | ViewExpr::TextField(_)
        | ViewExpr::Button(_)
        | ViewExpr::Let(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => {}
    }
}

fn collect_style_patches<'a>(expr: &'a ViewExpr, patches: &mut Vec<&'a StylePatch>) {
    let modifiers = match expr {
        ViewExpr::Element(element) => Some(element.modifiers.as_slice()),
        ViewExpr::ViewCall(call) => Some(call.modifiers.as_slice()),
        ViewExpr::Text(text) => Some(text.modifiers.as_slice()),
        ViewExpr::Image(image) => Some(image.modifiers.as_slice()),
        ViewExpr::TextField(field) => Some(field.modifiers.as_slice()),
        ViewExpr::Button(button) => Some(button.modifiers.as_slice()),
        ViewExpr::Fragment(_)
        | ViewExpr::Let(_)
        | ViewExpr::If(_)
        | ViewExpr::Match(_)
        | ViewExpr::ForEach(_)
        | ViewExpr::Await(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => None,
    };
    if let Some(modifiers) = modifiers {
        patches.extend(modifiers.iter().filter_map(|modifier| match modifier {
            ViewModifier::Style(ViewStyleModifier::Inline(patch)) => Some(patch),
            _ => None,
        }));
    }

    match expr {
        ViewExpr::Fragment(children) => {
            for child in children {
                collect_style_patches(child, patches);
            }
        }
        ViewExpr::Element(element) => {
            for child in &element.children {
                collect_style_patches(child, patches);
            }
        }
        ViewExpr::If(view_if) => {
            collect_style_patches(view_if.then_branch(), patches);
            if let Some(else_branch) = view_if.else_branch() {
                collect_style_patches(else_branch, patches);
            }
        }
        ViewExpr::Match(view_match) => {
            for arm in view_match.arms() {
                collect_style_patches(arm.value(), patches);
            }
        }
        ViewExpr::ForEach(view_for_each) => collect_style_patches(view_for_each.body(), patches),
        ViewExpr::Await(view_await) => {
            for branch in view_await.branches() {
                collect_style_patches(branch.value(), patches);
            }
        }
        ViewExpr::ViewCall(_)
        | ViewExpr::Text(_)
        | ViewExpr::Image(_)
        | ViewExpr::TextField(_)
        | ViewExpr::Button(_)
        | ViewExpr::Let(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => {}
    }
}

fn collect_text_nodes<'a>(expr: &'a ViewExpr, text: &mut Vec<&'a ViewText>) {
    match expr {
        ViewExpr::Text(node) => text.push(node),
        ViewExpr::Fragment(children) => {
            for child in children {
                collect_text_nodes(child, text);
            }
        }
        ViewExpr::Element(element) => {
            for child in element.children() {
                collect_text_nodes(child, text);
            }
        }
        ViewExpr::If(view_if) => {
            collect_text_nodes(view_if.then_branch(), text);
            if let Some(else_branch) = view_if.else_branch() {
                collect_text_nodes(else_branch, text);
            }
        }
        ViewExpr::Match(view_match) => {
            for arm in view_match.arms() {
                collect_text_nodes(arm.value(), text);
            }
        }
        ViewExpr::ForEach(view_for_each) => collect_text_nodes(view_for_each.body(), text),
        ViewExpr::Await(view_await) => {
            for branch in view_await.branches() {
                collect_text_nodes(branch.value(), text);
            }
        }
        ViewExpr::ViewCall(_)
        | ViewExpr::Image(_)
        | ViewExpr::TextField(_)
        | ViewExpr::Button(_)
        | ViewExpr::Let(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => {}
    }
}

fn collect_action_invokes(expr: &ViewExpr, invokes: &mut Vec<ViewActionInvokeAction>) {
    match expr {
        ViewExpr::Fragment(children) => {
            for child in children {
                collect_action_invokes(child, invokes);
            }
        }
        ViewExpr::Element(element) => {
            collect_modifier_action_invokes(&element.modifiers, element.range, None, invokes);
            for child in &element.children {
                collect_action_invokes(child, invokes);
            }
        }
        ViewExpr::ViewCall(call) => {
            collect_modifier_action_invokes(&call.modifiers, call.range, None, invokes);
        }
        ViewExpr::Text(text) => {
            collect_modifier_action_invokes(&text.modifiers, text.range, None, invokes);
        }
        ViewExpr::Image(image) => {
            collect_modifier_action_invokes(&image.modifiers, image.range, None, invokes);
        }
        ViewExpr::Button(button) => {
            if let Some(ViewAction::ActionInvoke(action)) = button.activation() {
                invokes.push(action.clone());
            }
            collect_modifier_action_invokes(
                &button.modifiers,
                button.range,
                Some("click"),
                invokes,
            );
        }
        ViewExpr::TextField(field) => {
            if let Some(ViewAction::ActionInvoke(action)) = field.submit_action() {
                invokes.push(action.clone());
            }
            collect_modifier_action_invokes(&field.modifiers, field.range, Some("submit"), invokes);
        }
        ViewExpr::If(view_if) => {
            collect_action_invokes(&view_if.then_branch, invokes);
            if let Some(else_branch) = view_if.else_branch.as_deref() {
                collect_action_invokes(else_branch, invokes);
            }
        }
        ViewExpr::Match(view_match) => {
            for arm in &view_match.arms {
                collect_action_invokes(&arm.value, invokes);
            }
        }
        ViewExpr::ForEach(view_for_each) => collect_action_invokes(&view_for_each.body, invokes),
        ViewExpr::Await(view_await) => {
            for branch in &view_await.branches {
                collect_action_invokes(&branch.value, invokes);
            }
        }
        ViewExpr::Let(_) | ViewExpr::Expr(_) | ViewExpr::Raw(_) => {}
    }
}

fn collect_action_projections<'a>(expr: &'a ViewExpr, projections: &mut Vec<&'a Expr>) {
    match expr {
        ViewExpr::Fragment(children) => {
            for child in children {
                collect_action_projections(child, projections);
            }
        }
        ViewExpr::Element(element) => {
            for child in element.children() {
                collect_action_projections(child, projections);
            }
        }
        ViewExpr::Button(button) => {
            if let Some(ViewAction::Projection(projection)) = button.activation() {
                projections.push(projection);
            }
        }
        ViewExpr::TextField(field) => {
            if let Some(ViewAction::Projection(projection)) = field.submit_action() {
                projections.push(projection);
            }
        }
        ViewExpr::If(view_if) => {
            collect_action_projections(view_if.then_branch(), projections);
            if let Some(else_branch) = view_if.else_branch() {
                collect_action_projections(else_branch, projections);
            }
        }
        ViewExpr::Match(view_match) => {
            for arm in view_match.arms() {
                collect_action_projections(arm.value(), projections);
            }
        }
        ViewExpr::ForEach(view_for_each) => {
            collect_action_projections(view_for_each.body(), projections);
        }
        ViewExpr::Await(view_await) => {
            for branch in view_await.branches() {
                collect_action_projections(branch.value(), projections);
            }
        }
        ViewExpr::ViewCall(_)
        | ViewExpr::Text(_)
        | ViewExpr::Image(_)
        | ViewExpr::Let(_)
        | ViewExpr::Expr(_)
        | ViewExpr::Raw(_) => {}
    }
}

fn collect_modifier_action_invokes(
    modifiers: &[ViewModifier],
    range: TextRange,
    skip_event: Option<&str>,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    for modifier in modifiers {
        let ViewModifier::OnEvent { name, body } = modifier else {
            continue;
        };
        if skip_event.is_some_and(|event| event == name) {
            continue;
        }
        collect_expr_action_invokes(body, range, invokes);
    }
}

fn collect_expr_action_invokes(
    expr: &Expr,
    range: TextRange,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    if let Some(action) = direct_action_invoke(expr, range) {
        invokes.push(action);
        return;
    }

    match expr {
        Expr::Call(call) => {
            collect_expr_action_invokes(call.callee(), range, invokes);
            collect_call_arg_action_invokes(call.args(), range, invokes);
        }
        Expr::Closure { body, .. } => collect_expr_action_invokes(body, range, invokes),
        Expr::Block { statements, value }
        | Expr::ComputationBlock {
            statements, value, ..
        }
        | Expr::NamedBlock {
            statements, value, ..
        } => {
            collect_expr_block_action_invokes(statements, value.as_deref(), range, invokes);
        }
        Expr::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_if_expr_action_invokes(
                condition,
                then_branch,
                else_branch.as_deref(),
                range,
                invokes,
            );
        }
        Expr::IfLet {
            expr,
            guard,
            then_branch,
            else_branch,
            ..
        } => {
            collect_if_let_expr_action_invokes(
                expr,
                guard.as_deref(),
                then_branch,
                else_branch.as_deref(),
                range,
                invokes,
            );
        }
        Expr::Match { scrutinee, arms } => {
            collect_match_expr_action_invokes(scrutinee, arms, range, invokes);
        }
        Expr::Tuple(items) | Expr::BracketSeq(items) => {
            collect_expr_list_action_invokes(items, range, invokes);
        }
        Expr::ArrayRepeat { value, len } => {
            collect_two_expr_action_invokes(value, len, range, invokes);
        }
        Expr::Select(select) => collect_expr_action_invokes(select.target(), range, invokes),
        Expr::Try(try_expr) => {
            collect_expr_action_invokes(try_expr.operand(), range, invokes);
        }
        Expr::Unary { expr: target, .. } => {
            collect_expr_action_invokes(target, range, invokes);
        }
        Expr::Await(awaited) => {
            collect_expr_action_invokes(awaited.operand(), range, invokes);
        }
        Expr::Borrow(borrow) => collect_expr_action_invokes(borrow.operand(), range, invokes),
        Expr::Deref(deref) => collect_expr_action_invokes(deref.operand(), range, invokes),
        Expr::Index { target, index } => {
            collect_expr_action_invokes(target, range, invokes);
            collect_expr_action_invokes(index, range, invokes);
        }
        Expr::Pipe { lhs, rhs } | Expr::Binary { lhs, rhs, .. } => {
            collect_two_expr_action_invokes(lhs, rhs, range, invokes);
        }
        Expr::Record { fields, .. } | Expr::RecordLiteral(fields) => {
            collect_record_field_action_invokes(fields, range, invokes);
        }
        Expr::DialogueCall { callee, .. } => collect_expr_action_invokes(callee, range, invokes),
        Expr::Range { start, end, .. } => {
            if let Some(start) = start.as_deref() {
                collect_expr_action_invokes(start, range, invokes);
            }
            if let Some(end) = end.as_deref() {
                collect_expr_action_invokes(end, range, invokes);
            }
        }
        Expr::Literal(_)
        | Expr::EntityRef(_)
        | Expr::LifetimePath { .. }
        | Expr::Path(_)
        | Expr::ShortVariant(_)
        | Expr::Placeholder(_)
        | Expr::NumericBracketSeq(_)
        | Expr::Thread { .. }
        | Expr::Raw(_) => {}
    }
}

fn direct_action_invoke(expr: &Expr, range: TextRange) -> Option<ViewActionInvokeAction> {
    match expr {
        Expr::Call(call) if is_action_invoke_callee(call.callee()) => {
            action_invoke_call_action(call.args(), range)
        }
        _ => None,
    }
}

fn collect_expr_block_action_invokes(
    statements: &[Stmt],
    value: Option<&Expr>,
    range: TextRange,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    collect_stmt_list_action_invokes(statements, range, invokes);
    if let Some(value) = value {
        collect_expr_action_invokes(value, range, invokes);
    }
}

fn collect_if_expr_action_invokes(
    condition: &Expr,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    range: TextRange,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    collect_expr_action_invokes(condition, range, invokes);
    collect_expr_action_invokes(then_branch, range, invokes);
    if let Some(else_branch) = else_branch {
        collect_expr_action_invokes(else_branch, range, invokes);
    }
}

fn collect_if_let_expr_action_invokes(
    expr: &Expr,
    guard: Option<&Expr>,
    then_branch: &Expr,
    else_branch: Option<&Expr>,
    range: TextRange,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    collect_expr_action_invokes(expr, range, invokes);
    if let Some(guard) = guard {
        collect_expr_action_invokes(guard, range, invokes);
    }
    collect_expr_action_invokes(then_branch, range, invokes);
    if let Some(else_branch) = else_branch {
        collect_expr_action_invokes(else_branch, range, invokes);
    }
}

fn collect_match_expr_action_invokes(
    scrutinee: &Expr,
    arms: &[MatchExprArm],
    range: TextRange,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    collect_expr_action_invokes(scrutinee, range, invokes);
    for arm in arms {
        if let Some(guard) = arm.guard() {
            collect_expr_action_invokes(guard, range, invokes);
        }
        collect_expr_action_invokes(arm.value(), range, invokes);
    }
}

fn collect_expr_list_action_invokes(
    items: &[Expr],
    range: TextRange,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    for item in items {
        collect_expr_action_invokes(item, range, invokes);
    }
}

fn collect_two_expr_action_invokes(
    first: &Expr,
    second: &Expr,
    range: TextRange,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    collect_expr_action_invokes(first, range, invokes);
    collect_expr_action_invokes(second, range, invokes);
}

fn collect_record_field_action_invokes(
    fields: &[(String, Expr)],
    range: TextRange,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    for (_, value) in fields {
        collect_expr_action_invokes(value, range, invokes);
    }
}

fn collect_call_arg_action_invokes(
    args: &[CallArg],
    range: TextRange,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    for arg in args {
        match arg {
            CallArg::Positional(expr) => collect_expr_action_invokes(expr, range, invokes),
            CallArg::Named { value, .. } | CallArg::Spread { value } => {
                collect_expr_action_invokes(value, range, invokes);
            }
        }
    }
}

fn collect_stmt_action_invokes(
    statement: &Stmt,
    range: TextRange,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    match statement {
        Stmt::Assertion(assertion) => {
            for condition in assertion.conditions() {
                collect_expr_action_invokes(condition, range, invokes);
            }
        }
        Stmt::Let { expr, .. } | Stmt::Return { expr, .. } | Stmt::Expr { expr, .. } => {
            collect_expr_action_invokes(expr, range, invokes);
        }
        Stmt::Out { expr, .. } => collect_expr_action_invokes(expr.expr(), range, invokes),
        Stmt::LetActionReceive { action, .. } | Stmt::Defer { expr: action, .. } => {
            collect_expr_action_invokes(action.expr(), range, invokes);
        }
        Stmt::Assign { target, expr }
        | Stmt::Signal {
            target,
            value: expr,
        }
        | Stmt::LifetimeSet { target, expr } => {
            collect_expr_action_invokes(target.expr(), range, invokes);
            collect_expr_action_invokes(expr.expr(), range, invokes);
        }
        Stmt::Goto(expr) | Stmt::Yield(expr) | Stmt::Close(expr) | Stmt::Select(expr) => {
            collect_expr_action_invokes(expr.expr(), range, invokes);
        }
        Stmt::LetElse {
            expr, else_body, ..
        } => {
            collect_expr_action_invokes(expr.expr(), range, invokes);
            collect_stmt_list_action_invokes(else_body, range, invokes);
        }
        Stmt::Wait(target) => match target {
            crate::ast::flow::WaitTarget::Duration(expr)
            | crate::ast::flow::WaitTarget::Expr(expr) => {
                collect_expr_action_invokes(expr.expr(), range, invokes);
            }
        },
        Stmt::DeferBlock { statements, .. }
        | Stmt::On {
            body: statements, ..
        }
        | Stmt::UnsafeLifetime {
            body: statements, ..
        }
        | Stmt::Loop { body: statements } => {
            collect_stmt_list_action_invokes(statements, range, invokes);
        }
        Stmt::If {
            condition,
            body,
            else_body,
        } => {
            collect_expr_action_invokes(condition.expr(), range, invokes);
            collect_stmt_list_action_invokes(body, range, invokes);
            collect_stmt_list_action_invokes(else_body, range, invokes);
        }
        Stmt::While { condition, body } => {
            collect_expr_action_invokes(condition.expr(), range, invokes);
            collect_stmt_list_action_invokes(body, range, invokes);
        }
        Stmt::WhileLet {
            expr, guard, body, ..
        } => {
            collect_expr_action_invokes(expr.expr(), range, invokes);
            if let Some(guard) = guard {
                collect_expr_action_invokes(guard.expr(), range, invokes);
            }
            collect_stmt_list_action_invokes(body, range, invokes);
        }
        Stmt::For { source, body, .. } => {
            collect_expr_action_invokes(source.expr(), range, invokes);
            collect_stmt_list_action_invokes(body, range, invokes);
        }
        Stmt::Match { expr, arms } => {
            collect_expr_action_invokes(expr.expr(), range, invokes);
            for arm in arms {
                if let Some(guard) = arm.guard() {
                    collect_expr_action_invokes(guard, range, invokes);
                }
                collect_stmt_list_action_invokes(arm.body(), range, invokes);
            }
        }
        Stmt::Break { expr, .. } => {
            if let Some(expr) = expr {
                collect_expr_action_invokes(expr.expr(), range, invokes);
            }
        }
        Stmt::LetChoice { .. }
        | Stmt::LetScope { .. }
        | Stmt::LetLoop { .. }
        | Stmt::LetAwait { .. }
        | Stmt::Thread(_)
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => {}
    }
}

fn collect_stmt_list_action_invokes(
    statements: &[Stmt],
    range: TextRange,
    invokes: &mut Vec<ViewActionInvokeAction>,
) {
    for statement in statements {
        collect_stmt_action_invokes(statement, range, invokes);
    }
}

fn is_action_invoke_callee(callee: &Expr) -> bool {
    match callee {
        Expr::Path(path) => path.matches_segments(&["action", "invoke"]),
        Expr::Select(select) => {
            select.member() == "invoke" && expression_path_is(select.target(), &["action"])
        }
        _ => false,
    }
}

fn action_invoke_call_action(args: &[CallArg], range: TextRange) -> Option<ViewActionInvokeAction> {
    let action = args.iter().find_map(|arg| match arg {
        CallArg::Positional(expr) => entity_ref_expr(expr).cloned(),
        CallArg::Named { name, value } if name == "action" => entity_ref_expr(value).cloned(),
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })?;
    let payload = args.iter().find_map(|arg| match arg {
        CallArg::Named { name, value } if name != "action" => {
            action_payload(value).map(|payload| (name.clone(), payload))
        }
        CallArg::Positional(_) | CallArg::Named { .. } | CallArg::Spread { .. } => None,
    });
    Some(ViewActionInvokeAction::new(
        action,
        payload.as_ref().map(|(name, _)| name.clone()),
        payload.map(|(_, payload)| payload),
        range,
    ))
}

fn action_payload(expr: &Expr) -> Option<ViewActionPayload> {
    match expr {
        Expr::Literal(Literal::String(value)) => {
            Some(ViewActionPayload::LiteralString(value.clone()))
        }
        Expr::Select(select) => text_control_payload_target(select.target())
            .zip(text_control_payload_field(select.member().as_str()))
            .map(|(input, field)| ViewActionPayload::TextControlProjection { input, field }),
        _ => None,
    }
}

fn text_control_payload_field(field: &str) -> Option<ViewTextControlPayloadField> {
    match field {
        "text" => Some(ViewTextControlPayloadField::Text),
        "value" => Some(ViewTextControlPayloadField::Value),
        _ => None,
    }
}

fn text_control_payload_target(expr: &Expr) -> Option<String> {
    match expr {
        Expr::EntityRef(reference) => Some(reference.canonical_body()),
        Expr::Path(path) => Some(path.as_label().to_owned()),
        _ => None,
    }
}

fn entity_ref_expr(expr: &Expr) -> Option<&EntityRefSyntax> {
    match expr {
        Expr::EntityRef(reference) => Some(reference),
        _ => None,
    }
}

fn input_handle_entity(expr: &Expr) -> Option<&EntityRefSyntax> {
    match expr {
        Expr::Call(call) if expression_path_is(call.callee(), &["input", "text"]) => {
            first_positional_entity_arg(call.args())
        }
        Expr::Call(call) if expression_path_is(call.callee(), &["input", "secure"]) => {
            first_positional_entity_arg(call.args())
        }
        _ => None,
    }
}

fn expression_path_is(expr: &Expr, segments: &[&str]) -> bool {
    expr.dotted_selector_label().is_some_and(|label| {
        let actual = label.split('.').collect::<Vec<_>>();
        actual == segments
    })
}

fn first_positional_entity_arg(args: &[CallArg]) -> Option<&EntityRefSyntax> {
    args.iter().find_map(|arg| match arg {
        CallArg::Positional(expr) => match expr.as_ref() {
            Expr::EntityRef(reference) => Some(reference),
            _ => None,
        },
        CallArg::Named { .. } | CallArg::Spread { .. } => None,
    })
}
