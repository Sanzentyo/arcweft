use crate::expr::Expr;
use crate::types::{FnSignature, TypeRef, WhereClause};

use super::common::{DocBlock, ModuleDecl, TextRange, UseItem, Visibility};
use super::dialogue::DialogueDefaultsItem;
use super::flow::{AuthoredExpr, ContractClause, Flow, FlowItem, Stmt};
use super::ids::{EntityRef, WikiLink};
use super::proof::{BenchItem, ProofItem, TestItem};
use super::source::SourceItem;
use super::style::StyleDecl;
use super::view::ViewBody;

/// Typed syntax view of an `.arcw` source with module/use headers and items.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedSyntaxTree {
    source: String,
    attrs: Vec<Attribute>,
    module: Option<ModuleDecl>,
    uses: Vec<UseItem>,
    items: Vec<Item>,
    wiki_links: Vec<WikiLink>,
}

/// Top-level syntax item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Flow(Flow),
    Function(FunctionItem),
    Agent(AgentItem),
    Callable(CallableItem),
    State(StateItem),
    Trait(TraitItem),
    Impl(ImplItem),
    Enum(EnumItem),
    Struct(StructItem),
    TypeAlias(TypeAliasItem),
    EntityDecl(EntityDeclItem),
    Entry(EntryDeclItem),
    ExternCapability(ExternCapabilityItem),
    ExternMod(ExternModItem),
    DialogueDefaults(DialogueDefaultsItem),
    Proof(ProofItem),
    Test(TestItem),
    Bench(BenchItem),
    Source(SourceItem),
    Style(StyleDecl),
    FlowItem(Box<FlowItem>),
    Raw(RawItem),
}

/// Raw top-level item preserved for grammar families not lowered yet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawItem {
    head: String,
    body: Option<String>,
    range: TextRange,
}

/// Recovery-only syntax that did not match a typed grammar family.
///
/// `RawSyntax` is not executable syntax. Parser recovery uses it to keep source
/// text plus a best-effort family/span so HIR, diagnostics, verifier, and LSP
/// tooling can report the unsupported construct without reparsing strings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawSyntax {
    family: RawSyntaxFamily,
    source: String,
    range: Option<TextRange>,
}

/// Grammar family where a recovery node was produced.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawSyntaxFamily {
    FlowItem,
    ChoiceItem,
    ChoicePlanItem,
    LinePlanItem,
    Stmt,
}

/// Attribute syntax such as `#[derive(...)]`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attribute {
    name: String,
    args: Option<String>,
    range: TextRange,
}

impl TypedSyntaxTree {
    pub(crate) fn new(
        source: String,
        attrs: Vec<Attribute>,
        module: Option<ModuleDecl>,
        uses: Vec<UseItem>,
        items: Vec<Item>,
        wiki_links: Vec<WikiLink>,
    ) -> Self {
        Self {
            source,
            attrs,
            module,
            uses,
            items,
            wiki_links,
        }
    }

    /// Original source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Inner attributes attached to the whole source file.
    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    /// Optional module declaration.
    pub const fn module(&self) -> Option<&ModuleDecl> {
        self.module.as_ref()
    }

    /// Parsed use declarations.
    pub fn uses(&self) -> &[UseItem] {
        &self.uses
    }

    /// Parsed top-level items.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// Wiki links discovered in comments.
    pub fn wiki_links(&self) -> &[WikiLink] {
        &self.wiki_links
    }
}

impl Item {
    /// Returns the authoritative top-level source range for item-boundary tooling.
    ///
    /// Flow-body items do not own top-level declaration boundaries, so they are
    /// deliberately excluded from insertion-point inventories.
    pub const fn range(&self) -> Option<TextRange> {
        match self {
            Self::Flow(item) => Some(*item.range()),
            Self::Function(item) => Some(*item.range()),
            Self::Agent(item) => Some(*item.range()),
            Self::Callable(item) => Some(*item.range()),
            Self::State(item) => Some(*item.range()),
            Self::Trait(item) => Some(*item.range()),
            Self::Impl(item) => Some(*item.range()),
            Self::Enum(item) => Some(*item.range()),
            Self::Struct(item) => Some(*item.range()),
            Self::TypeAlias(item) => Some(*item.range()),
            Self::EntityDecl(item) => Some(*item.range()),
            Self::Entry(item) => Some(*item.range()),
            Self::ExternCapability(item) => Some(*item.range()),
            Self::ExternMod(item) => Some(*item.range()),
            Self::DialogueDefaults(item) => Some(*item.range()),
            Self::Proof(item) => Some(*item.range()),
            Self::Test(item) => Some(*item.range()),
            Self::Bench(item) => Some(*item.range()),
            Self::Source(item) => Some(*item.range()),
            Self::Style(item) => Some(*item.range()),
            Self::Raw(item) => Some(*item.range()),
            Self::FlowItem(_) => None,
        }
    }
}

impl Attribute {
    pub(crate) const fn new(name: String, args: Option<String>, range: TextRange) -> Self {
        Self { name, args, range }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn args(&self) -> Option<&str> {
        self.args.as_deref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl RawItem {
    pub(crate) const fn new(head: String, body: Option<String>, range: TextRange) -> Self {
        Self { head, body, range }
    }

    pub fn head(&self) -> &str {
        &self.head
    }

    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl RawSyntax {
    pub(crate) fn new(
        family: RawSyntaxFamily,
        source: impl Into<String>,
        range: Option<TextRange>,
    ) -> Self {
        Self {
            family,
            source: source.into(),
            range,
        }
    }

    pub(crate) fn flow_item(source: impl Into<String>, range: Option<TextRange>) -> Self {
        Self::new(RawSyntaxFamily::FlowItem, source, range)
    }

    pub(crate) fn choice_item(source: impl Into<String>, range: Option<TextRange>) -> Self {
        Self::new(RawSyntaxFamily::ChoiceItem, source, range)
    }

    pub(crate) fn choice_plan_item(source: impl Into<String>, range: Option<TextRange>) -> Self {
        Self::new(RawSyntaxFamily::ChoicePlanItem, source, range)
    }

    pub(crate) fn line_plan_item(source: impl Into<String>, range: Option<TextRange>) -> Self {
        Self::new(RawSyntaxFamily::LinePlanItem, source, range)
    }

    pub(crate) fn stmt(source: impl Into<String>, range: Option<TextRange>) -> Self {
        Self::new(RawSyntaxFamily::Stmt, source, range)
    }

    pub const fn family(&self) -> RawSyntaxFamily {
        self.family
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub const fn range(&self) -> Option<TextRange> {
        self.range
    }
}

impl core::fmt::Display for RawSyntax {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.source)
    }
}

/// Top-level function item with parsed signature head and contract clauses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionItem {
    attrs: Vec<Attribute>,
    doc: Option<DocBlock>,
    kind: FunctionKind,
    visibility: Option<Visibility>,
    signature: FnSignature,
    signature_text: String,
    contracts: Vec<ContractClause>,
    body: String,
    body_statements: Vec<Stmt>,
    body_value: Option<AuthoredExpr>,
    range: TextRange,
}

/// Agent controller entry point declared in an Agent dialect source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentItem {
    attrs: Vec<Attribute>,
    doc: Option<DocBlock>,
    visibility: Option<Visibility>,
    id: Option<EntityRef>,
    name: String,
    signature: Option<FnSignature>,
    signature_text: Option<String>,
    contracts: Vec<ContractClause>,
    body: String,
    body_statements: Vec<Stmt>,
    body_value: Option<AuthoredExpr>,
    range: TextRange,
}

/// Internal initializer for an agent controller item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AgentItemInit {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) doc: Option<DocBlock>,
    pub(crate) visibility: Option<Visibility>,
    pub(crate) id: Option<EntityRef>,
    pub(crate) name: String,
    pub(crate) signature: Option<FnSignature>,
    pub(crate) signature_text: Option<String>,
    pub(crate) contracts: Vec<ContractClause>,
    pub(crate) body: String,
    pub(crate) body_statements: Vec<Stmt>,
    pub(crate) body_value: Option<AuthoredExpr>,
    pub(crate) range: TextRange,
}

/// Top-level function category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionKind {
    /// Ordinary synchronous function.
    Function,
    /// Background task function that may await non-visible work.
    Task,
    /// Dialogue-safe function callable from dialogue content tags.
    Dialogue,
    /// Generator-like function that yields a stream/source of values.
    Stream,
}

/// Top-level entity declaration family with runtime-specific body preserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntityDeclKind {
    Asset,
    Image,
    Character,
    View,
    Action,
    Activity,
    Content,
    Signal,
    Metric,
    Layer,
    Voice,
    Se,
    Bgm,
    AudioBus,
    MixerSnapshot,
    Ducking,
    Motion,
    Rig,
}

impl EntityDeclKind {
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Asset => "asset",
            Self::Image => "image",
            Self::Character => "character",
            Self::View => "view",
            Self::Action => "action",
            Self::Activity => "activity",
            Self::Content => "content",
            Self::Signal => "signal",
            Self::Metric => "metric",
            Self::Layer => "layer",
            Self::Voice => "voice",
            Self::Se => "se",
            Self::Bgm => "bgm",
            Self::AudioBus => "audio bus",
            Self::MixerSnapshot => "mixer snapshot",
            Self::Ducking => "ducking",
            Self::Motion => "motion",
            Self::Rig => "rig",
        }
    }
}

/// Top-level entity declaration such as `character`, `view`, `activity`,
/// `signal`, or `layer`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityDeclItem {
    attrs: Vec<Attribute>,
    kind: EntityDeclKind,
    visibility: Option<Visibility>,
    id: EntityRef,
    name: Option<String>,
    surface_alias: Option<String>,
    signature_tail: String,
    body: Option<String>,
    structured_body: Option<EntityDeclBody>,
    body_range: Option<TextRange>,
    range: TextRange,
}

/// Typed entity declaration body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntityDeclBody {
    Content(ContentDeclBody),
    Image(ImageDeclBody),
    View(Box<ViewDeclBody>),
}

/// Structured retained View body for `view ...` declarations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewDeclBody {
    view: Option<ViewBody>,
}

/// Content availability unit body declared with explicit root IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContentDeclBody {
    roots: Vec<EntityRef>,
}

/// Image presentation-object declaration body with parsed assignment fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageDeclBody {
    fields: Vec<ImageDeclField>,
}

/// One flat image declaration field such as `asset = @asset:.bg.room` or `transform.tx = 24px`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageDeclField {
    name: String,
    value_source: String,
    value: Expr,
}

/// Program entry declaration such as `entry game @entry.main { goto @flow.opening }`.
///
/// Entries are launch manifests in source form. They select an executable flow
/// or adapter route without making the first flow in a file special.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryDeclItem {
    kind: EntryKind,
    visibility: Option<Visibility>,
    id: EntityRef,
    items: Vec<EntryItem>,
    range: TextRange,
}

/// Entry adapter family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryKind {
    Game,
    Cli,
    Server,
    Activity,
    Test,
    Bench,
    Custom(String),
}

/// Structured item inside an entry block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryItem {
    Goto(EntityRef),
    Route {
        method: String,
        path: String,
        target: EntityRef,
        bindings: Vec<EntryRouteBinding>,
    },
    Option {
        name: String,
        value: Expr,
    },
    Raw(String),
}

/// Explicit route-to-flow argument binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryRouteBinding {
    name: String,
    source: EntryRouteBindingSource,
}

/// Adapter route value source used by an entry route binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntryRouteBindingSource {
    PathParam(String),
}

/// External module import declaration such as
/// `extern rust mod path from crate "name" { ... }`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternModItem {
    abi: String,
    path: String,
    source: Option<String>,
    members: Vec<ExternModMember>,
    body: String,
    range: TextRange,
}

/// Structured member declared inside an `extern rust mod` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternModMember {
    Type(ExternModType),
    Function(ExternModFunction),
    Activity(ExternModActivity),
    Raw(String),
}

/// Rust type-like export declared by an external module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternModType {
    visibility: Option<Visibility>,
    kind: ExternModTypeKind,
    name: String,
}

/// Type-like external export category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternModTypeKind {
    Type,
    Event,
}

/// Rust function export declared by an external module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternModFunction {
    visibility: Option<Visibility>,
    signature: FnSignature,
}

/// Runtime activity export declared by an external module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternModActivity {
    visibility: Option<Visibility>,
    name: String,
    ty: TypeRef,
}

/// Host capability declaration such as `extern capability cli { fn stdout(...) }`.
///
/// Capability declarations describe adapter-provided functions for checking and
/// tooling. They do not import host code into Sans I/O core crates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternCapabilityItem {
    attrs: Vec<Attribute>,
    visibility: Option<Visibility>,
    id: String,
    functions: Vec<CapabilityFn>,
    body: String,
    range: TextRange,
}

/// One function exported by an `extern capability` block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityFn {
    signature: FnSignature,
    effects: Vec<Expr>,
}

/// Internal initializer for a function item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FunctionInit {
    pub(crate) attrs: Vec<Attribute>,
    pub(crate) doc: Option<DocBlock>,
    pub(crate) kind: FunctionKind,
    pub(crate) visibility: Option<Visibility>,
    pub(crate) signature: FnSignature,
    pub(crate) signature_text: String,
    pub(crate) contracts: Vec<ContractClause>,
    pub(crate) body: String,
    pub(crate) body_statements: Vec<Stmt>,
    pub(crate) body_value: Option<AuthoredExpr>,
    pub(crate) range: TextRange,
}

/// Function-like top-level item such as `reducer` or `view`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableItem {
    kind: CallableKind,
    visibility: Option<Visibility>,
    name: String,
    signature_tail: String,
    contracts: Vec<ContractClause>,
    body: String,
    body_statements: Vec<Stmt>,
    body_value: Option<Expr>,
    range: TextRange,
}

pub(crate) struct CallableItemInit {
    pub(crate) kind: CallableKind,
    pub(crate) visibility: Option<Visibility>,
    pub(crate) name: String,
    pub(crate) signature_tail: String,
    pub(crate) contracts: Vec<ContractClause>,
    pub(crate) body: String,
    pub(crate) body_statements: Vec<Stmt>,
    pub(crate) body_value: Option<Expr>,
    pub(crate) range: TextRange,
}

/// Function-like item category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableKind {
    Reducer,
    View,
}

/// Root state declaration with typed fields and initializer expressions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateItem {
    attrs: Vec<Attribute>,
    visibility: Option<Visibility>,
    name: String,
    fields: Vec<StateField>,
    range: TextRange,
}

/// One state field, optionally public, with its default expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateField {
    doc: Option<DocBlock>,
    visibility: Option<Visibility>,
    name: String,
    ty: TypeRef,
    default: Expr,
}

/// Trait declaration with associated type and function members.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitItem {
    attrs: Vec<Attribute>,
    visibility: Option<Visibility>,
    name: String,
    supertraits: Vec<String>,
    members: Vec<TraitMember>,
    range: TextRange,
}

/// Member allowed inside a trait declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraitMember {
    AssociatedType {
        name: String,
        params: Vec<String>,
        value: Option<TypeRef>,
    },
    Function {
        signature: FnSignature,
        body: Option<String>,
        body_statements: Vec<Stmt>,
        body_value: Option<Box<AuthoredExpr>>,
    },
    Raw(String),
}

/// Member allowed inside an impl declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImplMember {
    AssociatedType {
        name: String,
        params: Vec<String>,
        value: TypeRef,
    },
    Function {
        signature: FnSignature,
        body: String,
        body_statements: Vec<Stmt>,
        body_value: Option<Box<AuthoredExpr>>,
    },
    Raw(String),
}

/// Impl declaration with structured members and original body text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplItem {
    visibility: Option<Visibility>,
    generics: Option<String>,
    trait_name: Option<String>,
    target: String,
    where_clauses: Vec<WhereClause>,
    members: Vec<ImplMember>,
    body: String,
    range: TextRange,
}

/// Internal initializer for an impl declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImplItemInit {
    pub(crate) visibility: Option<Visibility>,
    pub(crate) generics: Option<String>,
    pub(crate) trait_name: Option<String>,
    pub(crate) target: String,
    pub(crate) where_clauses: Vec<WhereClause>,
    pub(crate) members: Vec<ImplMember>,
    pub(crate) body: String,
    pub(crate) range: TextRange,
}

/// Top-level algebraic data type declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumItem {
    attrs: Vec<Attribute>,
    visibility: Option<Visibility>,
    name: String,
    variants: Vec<EnumVariant>,
    range: TextRange,
}

/// One enum variant row, preserving payload syntax for later lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariant {
    doc: Option<DocBlock>,
    name: String,
    payload: Option<String>,
}

/// Top-level struct declaration with typed fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructItem {
    attrs: Vec<Attribute>,
    visibility: Option<Visibility>,
    name: String,
    fields: Vec<StructField>,
    range: TextRange,
}

/// One `name: Type` struct field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructField {
    doc: Option<DocBlock>,
    name: String,
    ty: TypeRef,
}

/// Newtype/type alias declaration with optional `where` contracts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAliasItem {
    attrs: Vec<Attribute>,
    visibility: Option<Visibility>,
    name: String,
    target: TypeRef,
    where_clauses: Vec<Expr>,
    range: TextRange,
}

impl FunctionItem {
    pub(crate) fn new(init: FunctionInit) -> Self {
        Self {
            attrs: init.attrs,
            doc: init.doc,
            kind: init.kind,
            visibility: init.visibility,
            signature: init.signature,
            signature_text: init.signature_text,
            contracts: init.contracts,
            body: init.body,
            body_statements: init.body_statements,
            body_value: init.body_value,
            range: init.range,
        }
    }

    pub const fn kind(&self) -> FunctionKind {
        self.kind
    }

    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn signature(&self) -> &FnSignature {
        &self.signature
    }

    pub fn signature_text(&self) -> &str {
        &self.signature_text
    }

    pub fn contracts(&self) -> &[ContractClause] {
        &self.contracts
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn body_statements(&self) -> &[Stmt] {
        &self.body_statements
    }

    pub const fn body_value(&self) -> Option<&AuthoredExpr> {
        self.body_value.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl AgentItem {
    pub(crate) fn new(init: AgentItemInit) -> Self {
        Self {
            attrs: init.attrs,
            doc: init.doc,
            visibility: init.visibility,
            id: init.id,
            name: init.name,
            signature: init.signature,
            signature_text: init.signature_text,
            contracts: init.contracts,
            body: init.body,
            body_statements: init.body_statements,
            body_value: init.body_value,
            range: init.range,
        }
    }

    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> Option<&EntityRef> {
        self.id.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn signature(&self) -> Option<&FnSignature> {
        self.signature.as_ref()
    }

    pub fn signature_text(&self) -> Option<&str> {
        self.signature_text.as_deref()
    }

    pub fn contracts(&self) -> &[ContractClause] {
        &self.contracts
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn body_statements(&self) -> &[Stmt] {
        &self.body_statements
    }

    pub const fn body_value(&self) -> Option<&AuthoredExpr> {
        self.body_value.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl EntityDeclItem {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        attrs: Vec<Attribute>,
        kind: EntityDeclKind,
        visibility: Option<Visibility>,
        id: EntityRef,
        name: Option<String>,
        surface_alias: Option<String>,
        signature_tail: String,
        body: Option<String>,
        structured_body: Option<EntityDeclBody>,
        body_range: Option<TextRange>,
        range: TextRange,
    ) -> Self {
        Self {
            attrs,
            kind,
            visibility,
            id,
            name,
            surface_alias,
            signature_tail,
            body,
            structured_body,
            body_range,
            range,
        }
    }

    pub const fn kind(&self) -> EntityDeclKind {
        self.kind
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> &EntityRef {
        &self.id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn surface_alias(&self) -> Option<&str> {
        self.surface_alias.as_deref()
    }

    pub fn signature_tail(&self) -> &str {
        &self.signature_tail
    }

    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    pub const fn structured_body(&self) -> Option<&EntityDeclBody> {
        self.structured_body.as_ref()
    }

    pub const fn content_body(&self) -> Option<&ContentDeclBody> {
        match self.structured_body.as_ref() {
            Some(EntityDeclBody::Content(body)) => Some(body),
            Some(EntityDeclBody::Image(_) | EntityDeclBody::View(_)) | None => None,
        }
    }

    pub const fn image_body(&self) -> Option<&ImageDeclBody> {
        match self.structured_body.as_ref() {
            Some(EntityDeclBody::Image(body)) => Some(body),
            Some(EntityDeclBody::Content(_) | EntityDeclBody::View(_)) | None => None,
        }
    }

    pub fn view_body(&self) -> Option<&ViewDeclBody> {
        match self.structured_body.as_ref() {
            Some(EntityDeclBody::View(body)) => Some(body.as_ref()),
            Some(EntityDeclBody::Content(_) | EntityDeclBody::Image(_)) | None => None,
        }
    }

    pub const fn body_range(&self) -> Option<&TextRange> {
        self.body_range.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ContentDeclBody {
    pub(crate) const fn new(roots: Vec<EntityRef>) -> Self {
        Self { roots }
    }

    pub fn roots(&self) -> &[EntityRef] {
        &self.roots
    }
}

impl ViewDeclBody {
    pub(crate) const fn new(view: Option<ViewBody>) -> Self {
        Self { view }
    }

    pub const fn view(&self) -> Option<&ViewBody> {
        self.view.as_ref()
    }
}

impl ImageDeclBody {
    pub(crate) const fn new(fields: Vec<ImageDeclField>) -> Self {
        Self { fields }
    }

    pub fn fields(&self) -> &[ImageDeclField] {
        &self.fields
    }
}

impl ImageDeclField {
    pub(crate) fn new(name: String, value_source: String, value: Expr) -> Self {
        Self {
            name,
            value_source,
            value,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value_source(&self) -> &str {
        &self.value_source
    }

    pub const fn value(&self) -> &Expr {
        &self.value
    }
}

impl EntryDeclItem {
    pub(crate) const fn new(
        kind: EntryKind,
        visibility: Option<Visibility>,
        id: EntityRef,
        items: Vec<EntryItem>,
        range: TextRange,
    ) -> Self {
        Self {
            kind,
            visibility,
            id,
            items,
            range,
        }
    }

    pub const fn kind(&self) -> &EntryKind {
        &self.kind
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> &EntityRef {
        &self.id
    }

    pub fn items(&self) -> &[EntryItem] {
        &self.items
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl EntryKind {
    pub(crate) fn parse(source: &str) -> Self {
        match source {
            "game" => Self::Game,
            "cli" => Self::Cli,
            "server" => Self::Server,
            "activity" => Self::Activity,
            "test" => Self::Test,
            "bench" => Self::Bench,
            custom => Self::Custom(custom.to_owned()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Game => "game",
            Self::Cli => "cli",
            Self::Server => "server",
            Self::Activity => "activity",
            Self::Test => "test",
            Self::Bench => "bench",
            Self::Custom(value) => value,
        }
    }
}

impl EntryRouteBinding {
    pub(crate) fn new(name: impl Into<String>, source: EntryRouteBindingSource) -> Self {
        Self {
            name: name.into(),
            source,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn source(&self) -> &EntryRouteBindingSource {
        &self.source
    }
}

impl EntryRouteBindingSource {
    pub fn path_param(name: impl Into<String>) -> Self {
        Self::PathParam(name.into())
    }

    pub fn name(&self) -> &str {
        match self {
            Self::PathParam(name) => name,
        }
    }
}

impl ExternModItem {
    pub(crate) const fn new(
        abi: String,
        path: String,
        source: Option<String>,
        members: Vec<ExternModMember>,
        body: String,
        range: TextRange,
    ) -> Self {
        Self {
            abi,
            path,
            source,
            members,
            body,
            range,
        }
    }

    pub fn abi(&self) -> &str {
        &self.abi
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    pub fn members(&self) -> &[ExternModMember] {
        &self.members
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ExternModType {
    pub(crate) fn new(
        visibility: Option<Visibility>,
        kind: ExternModTypeKind,
        name: impl Into<String>,
    ) -> Self {
        Self {
            visibility,
            kind,
            name: name.into(),
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn kind(&self) -> ExternModTypeKind {
        self.kind
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl ExternModFunction {
    pub(crate) const fn new(visibility: Option<Visibility>, signature: FnSignature) -> Self {
        Self {
            visibility,
            signature,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn signature(&self) -> &FnSignature {
        &self.signature
    }
}

impl ExternModActivity {
    pub(crate) fn new(
        visibility: Option<Visibility>,
        name: impl Into<String>,
        ty: TypeRef,
    ) -> Self {
        Self {
            visibility,
            name: name.into(),
            ty,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn ty(&self) -> &TypeRef {
        &self.ty
    }
}

impl ExternCapabilityItem {
    pub(crate) const fn new(
        attrs: Vec<Attribute>,
        visibility: Option<Visibility>,
        id: String,
        functions: Vec<CapabilityFn>,
        body: String,
        range: TextRange,
    ) -> Self {
        Self {
            attrs,
            visibility,
            id,
            functions,
            body,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn functions(&self) -> &[CapabilityFn] {
        &self.functions
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl CapabilityFn {
    pub(crate) const fn new(signature: FnSignature, effects: Vec<Expr>) -> Self {
        Self { signature, effects }
    }

    pub const fn signature(&self) -> &FnSignature {
        &self.signature
    }

    pub fn effects(&self) -> &[Expr] {
        &self.effects
    }
}

impl CallableItem {
    pub(crate) fn new(init: CallableItemInit) -> Self {
        Self {
            kind: init.kind,
            visibility: init.visibility,
            name: init.name,
            signature_tail: init.signature_tail,
            contracts: init.contracts,
            body: init.body,
            body_statements: init.body_statements,
            body_value: init.body_value,
            range: init.range,
        }
    }

    pub const fn kind(&self) -> CallableKind {
        self.kind
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn signature_tail(&self) -> &str {
        &self.signature_tail
    }

    pub fn contracts(&self) -> &[ContractClause] {
        &self.contracts
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn body_statements(&self) -> &[Stmt] {
        &self.body_statements
    }

    pub const fn body_value(&self) -> Option<&Expr> {
        self.body_value.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl StateItem {
    pub(crate) const fn new(
        attrs: Vec<Attribute>,
        visibility: Option<Visibility>,
        name: String,
        fields: Vec<StateField>,
        range: TextRange,
    ) -> Self {
        Self {
            attrs,
            visibility,
            name,
            fields,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn fields(&self) -> &[StateField] {
        &self.fields
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl StateField {
    pub(crate) const fn new(
        doc: Option<DocBlock>,
        visibility: Option<Visibility>,
        name: String,
        ty: TypeRef,
        default: Expr,
    ) -> Self {
        Self {
            doc,
            visibility,
            name,
            ty,
            default,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn ty(&self) -> &TypeRef {
        &self.ty
    }

    pub const fn default(&self) -> &Expr {
        &self.default
    }
}

impl TraitItem {
    pub(crate) const fn new(
        attrs: Vec<Attribute>,
        visibility: Option<Visibility>,
        name: String,
        supertraits: Vec<String>,
        members: Vec<TraitMember>,
        range: TextRange,
    ) -> Self {
        Self {
            attrs,
            visibility,
            name,
            supertraits,
            members,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn supertraits(&self) -> &[String] {
        &self.supertraits
    }

    pub fn members(&self) -> &[TraitMember] {
        &self.members
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ImplItem {
    pub(crate) fn new(init: ImplItemInit) -> Self {
        Self {
            visibility: init.visibility,
            generics: init.generics,
            trait_name: init.trait_name,
            target: init.target,
            where_clauses: init.where_clauses,
            members: init.members,
            body: init.body,
            range: init.range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn generics(&self) -> Option<&str> {
        self.generics.as_deref()
    }

    pub fn trait_name(&self) -> Option<&str> {
        self.trait_name.as_deref()
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn where_clauses(&self) -> &[WhereClause] {
        &self.where_clauses
    }

    pub fn members(&self) -> &[ImplMember] {
        &self.members
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl EnumItem {
    pub(crate) const fn new(
        attrs: Vec<Attribute>,
        visibility: Option<Visibility>,
        name: String,
        variants: Vec<EnumVariant>,
        range: TextRange,
    ) -> Self {
        Self {
            attrs,
            visibility,
            name,
            variants,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn variants(&self) -> &[EnumVariant] {
        &self.variants
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl EnumVariant {
    pub(crate) const fn new(doc: Option<DocBlock>, name: String, payload: Option<String>) -> Self {
        Self { doc, name, payload }
    }

    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn payload(&self) -> Option<&str> {
        self.payload.as_deref()
    }
}

impl StructItem {
    pub(crate) const fn new(
        attrs: Vec<Attribute>,
        visibility: Option<Visibility>,
        name: String,
        fields: Vec<StructField>,
        range: TextRange,
    ) -> Self {
        Self {
            attrs,
            visibility,
            name,
            fields,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn fields(&self) -> &[StructField] {
        &self.fields
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl StructField {
    pub(crate) const fn new(doc: Option<DocBlock>, name: String, ty: TypeRef) -> Self {
        Self { doc, name, ty }
    }

    pub const fn doc(&self) -> Option<&DocBlock> {
        self.doc.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn ty(&self) -> &TypeRef {
        &self.ty
    }
}

impl TypeAliasItem {
    pub(crate) const fn new(
        attrs: Vec<Attribute>,
        visibility: Option<Visibility>,
        name: String,
        target: TypeRef,
        where_clauses: Vec<Expr>,
        range: TextRange,
    ) -> Self {
        Self {
            attrs,
            visibility,
            name,
            target,
            where_clauses,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn attrs(&self) -> &[Attribute] {
        &self.attrs
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn target(&self) -> &TypeRef {
        &self.target
    }

    pub fn where_clauses(&self) -> &[Expr] {
        &self.where_clauses
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}
