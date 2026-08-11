# Public AST and HIR migration

## 1. Public syntax authority

After the private inventory gate and broader Proof public-switch prerequisites are satisfied, `ParsedSource` is the sole source-backed parse authority. The final public item enum contains explicit variants:

```rust
pub enum Item {
    // existing non-retained typed variants ...
    Resource(ResourceDeclaration),
    Character(CharacterDeclaration),
    View(ViewDeclaration),
    Action(ActionDeclaration),
    Activity(ActivityDeclaration),
    Signal(SignalDeclaration),
    Metric(MetricDeclaration),
    Layer(LayerDeclaration),
    Error(ErrorItem),
}
```

There is no `Item::Asset`, generic retained/entity variant, or detached source-backed item.

Each declaration wrapper is a cloneable, snapshot-bound handle with a private attached node:

```rust
pub struct CharacterDeclaration(AstNode<CharacterDeclarationKind>);
pub struct ViewDeclaration(AstNode<ViewDeclarationKind>);
pub struct ActionDeclaration(AstNode<ActionDeclarationKind>);
pub struct ActivityDeclaration(AstNode<ActivityDeclarationKind>);
pub struct SignalDeclaration(AstNode<SignalDeclarationKind>);
pub struct MetricDeclaration(AstNode<MetricDeclarationKind>);
pub struct LayerDeclaration(AstNode<LayerDeclarationKind>);
```

Constructors are crate-private to attachment. Equality and hashing are exact attached identity (database, lineage, snapshot, node), not structural text equality. Cloning preserves the immutable snapshot handle and cannot forge IDs.

## 2. Public accessors

Every wrapper exposes read-only structural access:

```rust
fn syntax(&self) -> SyntaxNodeHandle;
fn id(&self) -> SyntaxNodeId;
fn range(&self) -> SourceRange;
fn header(&self) -> Result<RetainedDeclarationHeader, SyntaxAccessError>;
fn status(&self) -> DeclarationStatus;
```

Family accessors return exact attached child handles:

```rust
CharacterDeclaration::surface_alias() -> Result<Option<SurfaceAliasNode>, SyntaxAccessError>
CharacterDeclaration::body() -> Result<CharacterBodyNode, SyntaxAccessError>

ViewDeclaration::parameters() -> Result<FixedParameterGroupNode, SyntaxAccessError>
ViewDeclaration::exports() -> Result<impl Iterator<Item = ViewExportNode>, SyntaxAccessError>
ViewDeclaration::fragment() -> Result<ViewFragmentNode, SyntaxAccessError>

ActionDeclaration::signature() -> Result<ActionSignatureNode, SyntaxAccessError>
ActivityDeclaration::body() -> Result<ActivityBodyNode, SyntaxAccessError>
SignalDeclaration::observable_type() -> Result<TypeNode, SyntaxAccessError>
MetricDeclaration::kind() -> Result<MetricKindNode, SyntaxAccessError>
MetricDeclaration::value_type() -> Result<TypeNode, SyntaxAccessError>
MetricDeclaration::body() -> Result<MetricBodyNode, SyntaxAccessError>
LayerDeclaration::kind() -> Result<LayerKindNode, SyntaxAccessError>
LayerDeclaration::body() -> Result<LayerBodyNode, SyntaxAccessError>
```

`RetainedDeclarationHeader` is a borrowed typed view, not a body-erasing stored declaration:

```rust
fn family(&self) -> RetainedIdentityFamily;
fn documentation(&self) -> Result<Option<DocBlockNode>, SyntaxAccessError>;
fn attributes(&self) -> Result<impl Iterator<Item = OuterAttributeNode>, SyntaxAccessError>;
fn visibility(&self) -> Result<Option<VisibilityNode>, SyntaxAccessError>;
fn explicit_public_id(&self) -> Result<Option<DeclarationPublicIdNode>, SyntaxAccessError>;
fn name(&self) -> Result<NameNode, SyntaxAccessError>;
fn semantic_public_id(&self) -> Result<PublicId, RetainedHeaderError>;
fn range(&self) -> SourceRange;
fn core_range(&self) -> Result<SourceRange, SyntaxAccessError>;
```

No accessor returns `signature_tail`, raw body text, or copied source for reparsing.

## 3. Rowan round trip and errors

`ParsedSource` resolves exact handles in both directions:

- typed declaration/child to exact `SyntaxNodeHandle`;
- exact attached Rowan node to its concrete typed wrapper when the kind matches;
- handle to current range through the immutable snapshot.

Wrong database/lineage/snapshot, stale generation, retired ID, and concrete-kind mismatch are typed errors. An unbound fragment type cannot be passed to source-backed HIR lowering; it requires the accepted explicit attachment operation against a `SourceDocument` and syntax database transaction.

## 4. HIR identity extension

This slice uses the accepted `HirDatabase`, module, revision, immutable arena, liveness, and transaction design. It adds payload/child kinds, not a parallel database.

The existing raw-ID vocabulary is extended in its owning module with:

```rust
pub struct ParameterId(RawHirId);
pub struct DeclarationMemberId(RawHirId);

pub enum HirIdKind {
    Item,
    Scope,
    Local,
    Expr,
    Stmt,
    Type,
    Pattern,
    Parameter,
    DeclarationMember,
    Capture,
}
```

The corresponding `HirLimit` variants use inclusive module limits of 65,536 parameters and 65,536 declaration members, still bounded by the accepted total-slot limit. The enum's owned `as_str`/limit behavior is updated in the original implementation; no external match helper is introduced.

Each arena slot has source metadata separate from semantic payload:

```rust
pub struct HirSourceSlot {
    syntax: SyntaxNodeId,
    span: SourceSpan,
    born: HirRevision,
    last_live: Option<HirRevision>,
}
```

The source table is the only range/provenance authority. Payloads do not clone AST values or source strings.

## 5. HIR item payloads

```rust
pub enum HirItemKind {
    // existing typed items ...
    Character(HirCharacterDeclaration),
    View(HirViewDeclaration),
    Action(HirActionDeclaration),
    Activity(HirActivityDeclaration),
    Signal(HirSignalDeclaration),
    Metric(HirMetricDeclaration),
    Layer(HirLayerDeclaration),
    Error(HirErrorItem),
}

pub struct HirRetainedHeader {
    family: RetainedIdentityFamily,
    public_id: PublicId,
    public_id_origin: PublicIdOrigin, // Explicit | DerivedFromName
    name: DeclarationName,
    visibility: Option<Visibility>, // None is module-private
    documentation: Option<HirDocumentation>,
    attributes: Box<[HirAttribute]>,
}

pub struct HirCharacterDeclaration {
    header: HirRetainedHeader,
    surface_alias: Option<CharacterSurfaceAlias>,
    display_name: Option<ExprId>,
}

pub struct HirViewDeclaration {
    header: HirRetainedHeader,
    parameters: Box<[ParameterId]>,
    exports: Box<[DeclarationMemberId]>,
    values: Box<[ExprId]>,
}

pub struct HirActionDeclaration {
    header: HirRetainedHeader,
    parameters: Box<[ParameterId]>,
}

pub struct HirActivityDeclaration {
    header: HirRetainedHeader,
    mode: ActivityMode,
    lifecycle: ActivityLifecycle,
    inputs: Box<[DeclarationMemberId]>,
    outputs: Box<[DeclarationMemberId]>,
    requires: Box<[ExprId]>,
    ensures: Box<[ExprId]>,
}

pub struct HirSignalDeclaration {
    header: HirRetainedHeader,
    observable_type: TypeId,
}

pub struct HirMetricDeclaration {
    header: HirRetainedHeader,
    kind: MetricKind,
    value_type: TypeId,
    unit: Option<DeclarationMemberId>,
    labels: Box<[DeclarationMemberId]>,
    buckets: Option<DeclarationMemberId>,
}

pub struct HirLayerDeclaration {
    header: HirRetainedHeader,
    kind: LayerKind,
    members: Box<[DeclarationMemberId]>,
}
```

`HirParameter` owns a source slot, one `PatternId`, one `TypeId`, optional default `ExprId`, and the resulting `LocalId`. Action parameter default is structurally absent by invariant.

`HirDeclarationMember` is a closed enum for View exports, Activity ports, Metric unit/labels/buckets, Character display name when represented as a member source slot, and Layer members. Each variant contains typed IDs/owned enums/references and never raw text.

## 6. References in HIR

A declaration/member reference lowers to a typed unresolved HIR reference product:

```rust
pub struct HirRetainedReference {
    syntax: EntityRefSyntax,
    expected_family: RetainedIdentityFamily,
}
```

Its source is the member's `HirSourceSlot`. Project/sema resolution converts it to the existing checked project symbol identity or a typed resolver cause. Family members never split or inspect the spelling.

## 7. Asset ownership in HIR/project

There is no asset `ItemId` or asset declaration payload. The project snapshot owns catalog symbols:

```rust
pub struct ProjectAssetSymbol {
    id: AssetId,
    virtual_path: AssetVirtualPath,
    digest: BundleDigest,
    media: AssetMediaMetadata,
    inclusion: AssetInclusionProvenance,
}
```

This is project/catalog data, not HIR. HIR entity references may resolve to the catalog symbol through the unified project table. Asset catalog liveness follows project/catalog generation and bundle input changes.

## 8. Lowering transaction

The only source-backed lowering request contains:

- bound `ParsedSource` snapshot;
- exact `SourceDocument` identity;
- canonical package/module identity;
- target HIR database/module transaction.

For every item and child, lowering obtains the attached `SyntaxNodeId`, allocates the typed HIR ID/slot, lowers common typed descendants directly, and records source metadata. It never clones a public AST value into HIR, stores a syntax wrapper in HIR, or parses source/display strings.

A fatal lowering/identity/limit failure commits no revision, slots, liveness changes, diagnostics, project symbols, or caches. A poisoned syntax declaration lowers only an error item/source evidence.

## 9. Project registration

One registration pass over the accepted HIR project generation creates:

- Character symbol and Character alias/registry facet;
- View symbol, View catalog facet, and callable facet from the same `ItemId`;
- Action symbol and typed channel/callable facet from the same `ItemId`;
- Activity abstract interface symbol;
- Signal observable symbol;
- Metric schema symbol;
- Layer presentation symbol;
- Asset catalog symbols from catalog input, without HIR items;
- `res` symbols from their independent typed HIR items.

Imports, re-exports, accessibility, aliases, and collisions all consume this result. LSP borrows it and never creates a second project index.

## 10. Atomic deletion

The attached AST switch deletes, in the same compiling cut:

```text
EntityDeclItem
EntityDeclKind
EntityDeclBody
signature_tail
raw body / structured_body dual storage
source-less public retained constructors
detached retained declarations in TypedSyntaxTree
legacy generic retained parser branches
```

The HIR/project switch deletes, in its accepted atomic cut:

```text
HirTopLevelDecl::EntityDecl(EntityDeclItem)
clone-based retained lowering
raw signature/body parsing in sema/compiler/tooling
family-specific duplicate symbol registries
View callable clone/projection remnants
generic entity match helpers
CLI-local asset ID derivation helpers
Layer free string-to-family match helpers
linked/flattened retained HIR readers when the broader Stage 6 switch removes them
```
