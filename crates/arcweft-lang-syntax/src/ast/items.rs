use super::{
    BenchItem, CallableItem, DialogueDefaultsItem, EntityDeclItem, EnumItem, ExternModItem, Flow,
    FlowItem, FunctionItem, HookItem, ImplItem, MemoFn, ModuleDecl, ParserItem, ProofItem,
    SourceItem, StateItem, StructItem, TestItem, TextRange, TraitItem, TrustedAxiomItem,
    TypeAliasItem, UseItem, WikiLink,
};

/// Typed syntax view of an `.awft` source with module/use headers and items.
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
