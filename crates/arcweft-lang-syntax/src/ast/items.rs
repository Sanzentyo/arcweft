use crate::expr::Expr;
use crate::types::{FnSignature, TypeRef};

use super::common::{DocBlock, ModuleDecl, TextRange, UseItem, Visibility};
use super::dialogue::DialogueDefaultsItem;
use super::flow::{ContractClause, Flow, FlowItem, Stmt};
use super::ids::{EntityRef, WikiLink};
use super::proof::{BenchItem, ProofItem, TestItem, TrustedAxiomItem};
use super::source::SourceItem;

/// Typed syntax view of an `.arcw` source with module/use headers and items.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedSyntaxTree {
    source: String,
    module: Option<ModuleDecl>,
    uses: Vec<UseItem>,
    items: Vec<Item>,
    wiki_links: Vec<WikiLink>,
}

/// Top-level syntax item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Attribute(Attribute),
    Flow(Flow),
    Function(FunctionItem),
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
    Hook(HookItem),
    DialogueDefaults(DialogueDefaultsItem),
    MemoFn(MemoFn),
    Proof(ProofItem),
    TrustedAxiom(TrustedAxiomItem),
    Test(TestItem),
    Bench(BenchItem),
    Parser(ParserItem),
    Source(SourceItem),
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
        module: Option<ModuleDecl>,
        uses: Vec<UseItem>,
        items: Vec<Item>,
        wiki_links: Vec<WikiLink>,
    ) -> Self {
        Self {
            source,
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
    doc: Option<DocBlock>,
    kind: FunctionKind,
    visibility: Option<Visibility>,
    signature: FnSignature,
    signature_text: String,
    contracts: Vec<ContractClause>,
    body: String,
    body_statements: Vec<Stmt>,
    body_value: Option<Expr>,
    range: TextRange,
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
    Character,
    Component,
    Activity,
    Signal,
    Metric,
    Layer,
    Textbox,
    Voice,
    Se,
    Bgm,
    AudioBus,
    MixerSnapshot,
    Ducking,
    Motion,
    Rig,
}

/// Top-level entity declaration such as `character`, `component`, `activity`,
/// `signal`, or `layer`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntityDeclItem {
    kind: EntityDeclKind,
    visibility: Option<Visibility>,
    id: EntityRef,
    name: Option<String>,
    surface_alias: Option<String>,
    signature_tail: String,
    body: Option<String>,
    range: TextRange,
}

/// Program entry declaration such as `entry game @entry.main { start(@flow.opening) }`.
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
    Start(EntityRef),
    Run(EntityRef),
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
    body: String,
    range: TextRange,
}

/// Host capability declaration such as `extern capability cli { fn stdout(...) }`.
///
/// Capability declarations describe adapter-provided functions for checking and
/// tooling. They do not import host code into Sans I/O core crates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternCapabilityItem {
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
    pub(crate) doc: Option<DocBlock>,
    pub(crate) kind: FunctionKind,
    pub(crate) visibility: Option<Visibility>,
    pub(crate) signature: FnSignature,
    pub(crate) signature_text: String,
    pub(crate) contracts: Vec<ContractClause>,
    pub(crate) body: String,
    pub(crate) body_statements: Vec<Stmt>,
    pub(crate) body_value: Option<Expr>,
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
        body_value: Option<Expr>,
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
    members: Vec<ImplMember>,
    body: String,
    range: TextRange,
}

/// Top-level algebraic data type declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumItem {
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
    visibility: Option<Visibility>,
    name: String,
    target: TypeRef,
    where_clauses: Vec<Expr>,
    range: TextRange,
}

/// Hook item syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookItem {
    visibility: Option<Visibility>,
    id: EntityRef,
    target: String,
    phase: String,
    when: Option<Expr>,
    priority: Option<i64>,
    once: bool,
    effects: Vec<Expr>,
    body: String,
    body_statements: Vec<Stmt>,
    range: TextRange,
}

/// Internal initializer for hook syntax.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HookInit {
    pub(crate) visibility: Option<Visibility>,
    pub(crate) id: EntityRef,
    pub(crate) target: String,
    pub(crate) phase: String,
    pub(crate) when: Option<Expr>,
    pub(crate) priority: Option<i64>,
    pub(crate) once: bool,
    pub(crate) effects: Vec<Expr>,
    pub(crate) body: String,
    pub(crate) body_statements: Vec<Stmt>,
    pub(crate) range: TextRange,
}

/// Memoized function item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoFn {
    visibility: Option<Visibility>,
    signature: String,
    options: Vec<String>,
    body: String,
    body_statements: Vec<Stmt>,
    body_value: Option<Expr>,
    range: TextRange,
}

/// User-defined parser item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParserItem {
    visibility: Option<Visibility>,
    name: String,
    signature_tail: String,
    body: String,
    body_statements: Vec<Stmt>,
    body_value: Option<Expr>,
    range: TextRange,
}

impl FunctionItem {
    pub(crate) fn new(init: FunctionInit) -> Self {
        Self {
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

    pub const fn body_value(&self) -> Option<&Expr> {
        self.body_value.as_ref()
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl EntityDeclItem {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        kind: EntityDeclKind,
        visibility: Option<Visibility>,
        id: EntityRef,
        name: Option<String>,
        surface_alias: Option<String>,
        signature_tail: String,
        body: Option<String>,
        range: TextRange,
    ) -> Self {
        Self {
            kind,
            visibility,
            id,
            name,
            surface_alias,
            signature_tail,
            body,
            range,
        }
    }

    pub const fn kind(&self) -> EntityDeclKind {
        self.kind
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

    pub const fn range(&self) -> &TextRange {
        &self.range
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
        body: String,
        range: TextRange,
    ) -> Self {
        Self {
            abi,
            path,
            source,
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

    pub fn body(&self) -> &str {
        &self.body
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl ExternCapabilityItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        id: String,
        functions: Vec<CapabilityFn>,
        body: String,
        range: TextRange,
    ) -> Self {
        Self {
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
        visibility: Option<Visibility>,
        name: String,
        fields: Vec<StateField>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name,
            fields,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
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
        visibility: Option<Visibility>,
        name: String,
        supertraits: Vec<String>,
        members: Vec<TraitMember>,
        range: TextRange,
    ) -> Self {
        Self {
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
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        generics: Option<String>,
        trait_name: Option<String>,
        target: String,
        members: Vec<ImplMember>,
        body: String,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            generics,
            trait_name,
            target,
            members,
            body,
            range,
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
        visibility: Option<Visibility>,
        name: String,
        variants: Vec<EnumVariant>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name,
            variants,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
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
        visibility: Option<Visibility>,
        name: String,
        fields: Vec<StructField>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name,
            fields,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
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
        visibility: Option<Visibility>,
        name: String,
        target: TypeRef,
        where_clauses: Vec<Expr>,
        range: TextRange,
    ) -> Self {
        Self {
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

impl HookItem {
    pub(crate) fn new(init: HookInit) -> Self {
        Self {
            visibility: init.visibility,
            id: init.id,
            target: init.target,
            phase: init.phase,
            when: init.when,
            priority: init.priority,
            once: init.once,
            effects: init.effects,
            body: init.body,
            body_statements: init.body_statements,
            range: init.range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub const fn id(&self) -> &EntityRef {
        &self.id
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn phase(&self) -> &str {
        &self.phase
    }

    pub const fn when(&self) -> Option<&Expr> {
        self.when.as_ref()
    }

    pub const fn priority(&self) -> Option<i64> {
        self.priority
    }

    pub const fn once(&self) -> bool {
        self.once
    }

    pub fn effects(&self) -> &[Expr] {
        &self.effects
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn body_statements(&self) -> &[Stmt] {
        &self.body_statements
    }

    pub const fn range(&self) -> &TextRange {
        &self.range
    }
}

impl MemoFn {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        signature: String,
        options: Vec<String>,
        body: String,
        body_statements: Vec<Stmt>,
        body_value: Option<Expr>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            signature,
            options,
            body,
            body_statements,
            body_value,
            range,
        }
    }

    pub const fn visibility(&self) -> Option<Visibility> {
        self.visibility
    }

    pub fn signature(&self) -> &str {
        &self.signature
    }

    pub fn options(&self) -> &[String] {
        &self.options
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

impl ParserItem {
    pub(crate) const fn new(
        visibility: Option<Visibility>,
        name: String,
        signature_tail: String,
        body: String,
        body_statements: Vec<Stmt>,
        body_value: Option<Expr>,
        range: TextRange,
    ) -> Self {
        Self {
            visibility,
            name,
            signature_tail,
            body,
            body_statements,
            body_value,
            range,
        }
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
