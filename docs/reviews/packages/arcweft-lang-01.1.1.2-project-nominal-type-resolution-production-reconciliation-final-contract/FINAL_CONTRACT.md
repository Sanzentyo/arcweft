# FINAL CONTRACT

## 0. Normative status

This document is normative. `MUST`, `MUST NOT`, `SHALL`, `SHALL NOT`,
`SHOULD`, and `MAY` have their usual requirements-language meanings. Where an
example conflicts with a type definition or an explicit rule, the type
definition and explicit rule win.

The final implementation has exactly two cooperating authorities:

1. `arcweft_lang_hir::symbol::ProjectSymbolTable` is the sole authority for
   project declaration identity, module topology, visibility, imports,
   aliases created by `use`, globs, and re-exports.
2. `arcweft_lang_sema::nominal::resolve_type_ref` is the sole recursive
   authority that converts one authored type reference into checked semantic
   type facts by consuming the immutable project table and the accepted
   environment.

The HIR table never depends on sema. Sema depends on HIR, syntax, source, and
the existing domain crates. `arcweft-core` is not involved. Syntax remains
parser/syntax-only. HIR and data-format layers remain Sans I/O.

A project name is never accepted merely because context-free conversion can
construct `TypeKind::Named`. A project alias is never recognized by spelling.
`Unknown` and `ArcResult` have no special behavior.

## 1. Preserved substrate and permitted corrections

The implementation SHALL preserve the already selected Lang-01.1.1.1 Try,
Await, checked-return-boundary, nearest-boundary, operand-success recovery,
anonymous-choice, and callable-catalog contracts. It SHALL NOT redesign Try,
Await, callable identity, direct suspension, Stream generators, runtime
lowering, AWBC, save/load, host wire, cancellation, rendering, CSS, or Takumi.

The following are concrete defects and therefore are the only substrate
corrections authorized by this contract:

1. authored type paths and generic heads are string-valued and lack an exact
   source map;
2. type aliases lose generic parameters and exact target source evidence;
3. enum payloads are retained as strings and reparsed by entry checking;
4. `ProjectSymbolTargetId` lacks source nominal declarations;
5. the normal checker can successfully fall through to
   `TypeKind::from(&TypeRef)` and arbitrary `TypeKind::Named`;
6. the normal checker owns spelling-keyed alias maps;
7. the entry checker reconstructs project/import/alias resolution;
8. the entry canonicalizer has an `ArcResult` constructor branch; and
9. the project linker omits unresolved imports instead of classifying unknown
   versus unanchored import cycles.

Every replaced field or successful reader is removed in the same compile-clean
migration cut. There are no compatibility fields, aliases, extension-trait
shims, dual readers, source gates, or spelling deny lists.

## 2. Typed authored type syntax and exact source evidence

### 2.1 Owning module and final shapes

`arcweft-lang-syntax::types` SHALL own these final public types:

```rust
use crate::ast::{
    common::TextRange,
    module_path::ModuleSegment,
    symbol_path::ProjectSymbolPath,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypePath(ProjectSymbolPath);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeRecoveryId(u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeRef {
    Never,
    ConstInt(usize),
    Path(TypePath),
    Tuple(Vec<TypeRef>),
    Function {
        params: Vec<TypeRef>,
        return_type: Box<TypeRef>,
        effects: Option<TypeEffectRow>,
    },
    Choice(Vec<TypeRef>),
    Generic {
        base: TypePath,
        args: Vec<TypeRef>,
    },
    TraitBound(TraitBound),
    Projection {
        subject: Box<TypeRef>,
        assoc: ModuleSegment,
    },
    Reference(ReferenceType),
    Slice(Box<TypeRef>),
    Recovery(TypeRecoveryId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssociatedTypeBinding {
    name: ModuleSegment,
    value: TypeRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraitBound {
    path: TypePath,
    args: Vec<TypeRef>,
    associated: Vec<AssociatedTypeBinding>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypeRefNodePath(Box<[TypeRefNodeStep]>);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeRefNodeStep {
    TupleItem(u16),
    FunctionParameter(u16),
    FunctionReturn,
    ChoiceAlternative(u16),
    GenericArgument(u16),
    TraitArgument(u16),
    AssociatedBinding(u16),
    ProjectionSubject,
    ReferenceReferent,
    SliceItem,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeRefHeadKind {
    Never,
    ConstInt,
    Path,
    Constructor,
    Trait,
    ProjectionMember,
    Recovery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefHeadSource<R> {
    kind: TypeRefHeadKind,
    range: R,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefNodeSource<R> {
    whole: R,
    head: Option<TypeRefHeadSource<R>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefSourceMap<R> {
    nodes: Box<[(TypeRefNodePath, TypeRefNodeSource<R>)]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoredTypeRef {
    value: TypeRef,
    source: TypeRefSourceMap<TextRange>,
}
```

`TypePath` SHALL expose `path(&self) -> &ProjectSymbolPath`,
`root()`, `segments()`, and `canonical_string()` for presentation only.
Construction is parser-owned (`pub(crate)`); consumers do not construct
identity by parsing a display string.

`AuthoredTypeRef` SHALL expose `value()`, `source()`, `root_source()`, and
`source_at(&TypeRefNodePath)`. Its validating constructor is `pub(crate)` to
the syntax crate. There is one public `parse_type_ref` and it returns
`AuthoredTypeRef`. The unspanned value parser is private. This is a direct API
switch, not a second reader.

### 2.2 Source-map invariant

For every `TypeRef` node, the source map contains exactly one entry with the
same structural node path. It contains no extra entry. Every `whole` and every
present `head.range` is a UTF-8 byte range, a head range is contained by its
`whole`, and each child `whole` is contained by its parent `whole`. Structural
nodes without one diagnostic head (tuple, function, choice, reference, slice)
use `head = None`; path, constructor, trait, projection-member, const, never,
and recovery nodes use `Some`. Sibling ranges may touch
but do not overlap except where the grammar explicitly makes a parenthesized
wrapper the parent node.

The root path is the empty `TypeRefNodePath`. Index-bearing steps use `u16`;
the parser rejects a type node with more than 256 generic arguments and more
than 4,096 total nodes before conversion, so index overflow is impossible.

Reference-token ranges already owned by `ReferenceType` remain authoritative
for `&`, lifetime, `mut`, and referent grouping. The new map must agree with
those ranges; disagreement is an internal parser invariant failure, not a
recovery path.

### 2.3 Authored owners migrated atomically

Every authored type position SHALL store `AuthoredTypeRef`, including:

- function, method, flow, extern-function, and closure annotations;
- generic bounds and all typed where predicates;
- struct fields and enum payloads;
- type-alias targets;
- trait associated-type defaults and bounds;
- impl target, trait reference, associated-type values, and where predicates;
- entry state and event declarations;
- references, slices, tuples, function types, choices, projections, trait
  bounds, and associated bindings nested inside any of those positions.

`TypeAliasItem` gains `name_range`, `generic_params`, a typed target, typed
where predicates, and exact ranges. `EnumVariant.payload` is replaced by
`Option<AuthoredTypeRef>` and exact variant/name/payload ranges. It is never
reparsed. Struct fields gain whole/name ranges. No old string payload or
unspanned alias target remains.

Project nominal declarations in this contract accept type generic parameters
only. A lifetime parameter on a struct, enum, or type alias is syntactically
retained but rejected during project publication as an invalid nominal
declaration. Function and method lifetime generics remain governed by the
existing borrow/type rules. This avoids inventing an unrelated lifetime
application grammar while leaving no arity decision open.

## 3. Project nominal identity and source records

### 3.1 Owner

`arcweft-lang-hir::symbol::nominal` SHALL own project nominal identity,
declaration records, bound type references, and declaration source records.
They are not re-exported from the `arcweft-lang_hir` crate root.

### 3.2 Exact identity

```rust
use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModuleSegment},
};
use arcweft_source::SourceSpan;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProjectNominalDeclarationKind {
    Struct,
    Enum,
    TypeAlias,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectNominalDeclarationId {
    world: ProjectSymbolWorldId,
    revision: ProjectSymbolRevision,
    module: CanonicalModulePath,
    kind: ProjectNominalDeclarationKind,
    owner_path: Box<[ModuleSegment]>,
    name: ModuleSegment,
}
```

The identity tuple is:

```text
(
  world.package,
  world.root_document,
  world.profile,
  revision,
  canonical_module,
  declaration_family,
  owner_path,
  local_name
)
```

All fields participate in equality, hashing, and ordering. The constructor is
`pub(crate)` to the HIR symbol linker and validates that the module is present
in the same world/revision. Public accessors expose each field by reference or
copy. `qualified_name()` is display output only and is never parsed back.

Top-level declarations have an empty owner path. The field is retained as an
identity component so a future owner-supported declaration does not need a new
identity schema; this contract does not add nested nominal syntax.

Qualified, parent/child-relative, imported, aliased, globbed, and re-exported
spellings resolve to this original declaration ID. An alias declaration has
its own `ProjectNominalDeclarationId` with kind `TypeAlias`; it remains
distinct from every declaration reached by normalization.

### 3.3 Source-backed records

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBackedTypeRef {
    authored: AuthoredTypeRef,
    spans: TypeRefSourceMap<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalTypeParameterSource {
    whole: SourceSpan,
    name: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalTypeParameter {
    ordinal: u16,
    name: ModuleSegment,
    bounds: Box<[SourceBackedTypeRef]>,
    source: ProjectNominalTypeParameterSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBackedWherePredicate {
    subject: SourceBackedTypeRef,
    bounds: Box<[SourceBackedTypeRef]>,
    whole: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalFieldSource {
    whole: SourceSpan,
    name: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalField {
    name: ModuleSegment,
    ty: SourceBackedTypeRef,
    source: ProjectNominalFieldSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalVariantSource {
    whole: SourceSpan,
    name: SourceSpan,
    payload: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalVariant {
    name: ModuleSegment,
    payload: Option<SourceBackedTypeRef>,
    source: ProjectNominalVariantSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectNominalBody {
    Struct {
        fields: Box<[ProjectNominalField]>,
    },
    Enum {
        variants: Box<[ProjectNominalVariant]>,
    },
    TypeAlias {
        target: SourceBackedTypeRef,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalDeclarationSource {
    whole: SourceSpan,
    name: SourceSpan,
    generics: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNominalDeclaration {
    id: ProjectNominalDeclarationId,
    visibility: Option<Visibility>,
    type_parameters: Box<[ProjectNominalTypeParameter]>,
    where_predicates: Box<[SourceBackedWherePredicate]>,
    body: ProjectNominalBody,
    source: ProjectNominalDeclarationSource,
}
```

`SourceBackedTypeRef` keeps both local syntax ranges and exact project
`SourceSpan`s. Its HIR-only constructor maps every local range through the
actual module `SourceDocument`, validates one-to-one node paths and UTF-8
boundaries, and rejects a mismatch. It never creates a document identity.

Records expose read-only accessors. Mutation occurs only in the bounded linker
builder. Declaration IDs and records are immutable after publication.

The exact construction error is:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectNominalDeclarationError {
    InvalidName {
        source: SourceSpan,
        reason: ModulePathError,
    },
    UnsupportedLifetimeParameter {
        source: SourceSpan,
    },
    DuplicateTypeParameter {
        name: ModuleSegment,
        first: SourceSpan,
        duplicate: SourceSpan,
    },
    SourceMapMismatch {
        source: SourceSpan,
        reason: ProjectNominalSourceError,
    },
    Limit {
        kind: ProjectSymbolLimitKind,
        observed: u64,
        maximum: u64,
        source: SourceSpan,
    },
}
```

`ProjectSymbolLinkError` gains
`InvalidNominalDeclaration { source, reason }`; its code is
`aw.project.symbol.invalid_nominal_declaration`.

The HIR binding error is:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectNominalSourceError {
    Structure(TypeRefSourceMapError),
    OutOfBounds {
        range: TextRange,
        source_len: u32,
    },
    NotUtf8Boundary {
        byte: u32,
    },
    WrongDocument {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
}
```

The syntax source-map error itself is closed:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeRefSourceMapError {
    MissingRoot,
    MissingNode(TypeRefNodePath),
    ExtraNode(TypeRefNodePath),
    DuplicateNode(TypeRefNodePath),
    HeadOutsideWhole(TypeRefNodePath),
    ChildOutsideParent(TypeRefNodePath),
    IndexOverflow(TypeRefNodePath),
}
```

Binding local ranges to a `SourceDocument` maps structural, UTF-8,
out-of-bounds, and wrong-document failures to
`ProjectNominalDeclarationError::SourceMapMismatch`; no range is clamped or
fabricated.

## 4. Unified project-symbol publication

### 4.1 Enum extensions in the owner

The existing enums SHALL be extended directly in
`arcweft-lang-hir::symbol`; no helper enum or extension trait is allowed:

```rust
pub enum ProjectDeclarationId {
    Callable(CallableDeclarationId),
    External(ExternalDeclarationId),
    Nominal(ProjectNominalDeclarationId),
}

pub enum ProjectSymbol {
    Callable(CallableSymbol),
    External(ExternalSymbol),
    Nominal(ProjectNominalDeclaration),
}

pub enum ProjectSymbolTargetId {
    Callable(CallableDeclarationId),
    External(ExternalDeclarationId),
    Nominal(ProjectNominalDeclarationId),
    Module(CanonicalModulePath),
}

pub enum ResolvedProjectSymbol<'a> {
    Callable(&'a CallableSymbol),
    External(&'a ExternalSymbol),
    Nominal(&'a ProjectNominalDeclaration),
    Module(&'a CanonicalModulePath),
}
```

Every existing exhaustive inherent method and match is updated in its owner.
No broad root re-export is added.

### 4.2 Type-target lookup

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectTypeCandidate {
    target: ProjectSymbolTargetId,
    declaration: Option<SourceSpan>,
    binding_sites: Box<[SourceSpan]>,
}

#[derive(Debug)]
pub enum ProjectTypeTarget<'a> {
    Nominal(&'a ProjectNominalDeclaration),
    External(&'a ExternalSymbol),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectTypeLookupError {
    Unknown {
        module: CanonicalModulePath,
        reference: TypePath,
        source: SourceSpan,
    },
    Ambiguous {
        module: CanonicalModulePath,
        reference: TypePath,
        source: SourceSpan,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    Inaccessible {
        module: CanonicalModulePath,
        reference: TypePath,
        source: SourceSpan,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    WrongKind {
        reference: TypePath,
        source: SourceSpan,
        actual: ProjectTypeCandidate,
    },
    InvalidPath {
        source: SourceSpan,
        reason: ModulePathError,
    },
}
```

The table exposes only these intentional APIs:

```rust
impl ProjectSymbolTable {
    pub fn nominal(
        &self,
        id: &ProjectNominalDeclarationId,
    ) -> Option<&ProjectNominalDeclaration>;

    pub fn nominal_symbols(
        &self,
    ) -> impl ExactSizeIterator<Item = &ProjectNominalDeclaration>;

    pub fn resolve_type_target(
        &self,
        module: &CanonicalModulePath,
        path: &TypePath,
        source: SourceSpan,
    ) -> Result<ProjectTypeTarget<'_>, ProjectTypeLookupError>;

    pub fn visible_type_bindings(
        &self,
        module: &CanonicalModulePath,
    ) -> impl Iterator<Item = VisibleProjectTypeBinding<'_>>;
}
```

`visible_type_bindings` is the completion API; it returns typed spelling,
target, visibility, and source sites. It does not return rendered strings that
must be reparsed.

### 4.3 Atomic link transaction

`ProjectSymbolTable::link(project, externals)` keeps its existing signature and
transaction. It SHALL perform these phases in deterministic order:

1. validate world/revision/source inventory;
2. insert module bindings;
3. collect and insert callable declarations;
4. collect and insert nominal declarations;
5. insert accepted external declarations;
6. validate direct-name collisions and all collection limits;
7. resolve imports/re-exports by the existing bounded fixed point;
8. classify every unresolved import;
9. sort/deduplicate/cap diagnostics;
10. publish only if the diagnostic set is empty.

Nominals are collected from module-preserving `HirProject::modules()`, never
from `linked_module()`. Module order, declaration order within a module,
candidate order, and diagnostic order are canonicalized through `BTreeMap` /
`BTreeSet` plus source ordering.

The current accepted project model binds one `SourceDocumentIdentity` to one
canonical module entry. A project may contain many modules in many documents.
Two documents claiming the same canonical module are rejected by project/HIR
construction before symbol publication; the nominal linker never merges their
declarations or invents a composite document identity.

A direct declaration name is one project-symbol namespace. Two distinct direct
targets with the same module-local binding are a duplicate, including
struct/enum/alias cross-family collisions and nominal/callable/external/module
collisions. The later declaration name is primary and the first declaration
name is secondary.

The same target reaching a scope through multiple imports or re-exports is
coalesced while preserving every source site. Different targets introduced by
multiple globs remain a deterministic ambiguous binding; publication may
succeed and an authored type use produces `sema.nominal.ambiguous_type`.
An explicit named import that itself denotes multiple targets is a link error
`aw.project.symbol.ambiguous_import`.

The existing visibility semantics are frozen:

- a declaration is visible in its owner module;
- `pub` and `crate` are visible throughout the accepted project;
- `super` is visible in the parent module subtree;
- a private declaration is not visible outside its owner;
- a re-export can never widen target visibility.

An inaccessible target is not downgraded to unknown. Visibility escalation is
a link error.

The import fixed point SHALL build an unresolved import graph. An unresolved
strongly connected component with no path to an anchored declaration is
`aw.project.symbol.cyclic_import`; an unresolved acyclic import or unknown
module is `aw.project.symbol.unknown_import`. A cycle that reaches an anchored
declaration and resolves within limits is valid. The current silent omission
of `ImportResolutionError::Unknown` is deleted.

Language-reserved built-in type names cannot be direct project declaration or
external binding names. Publication reports
`aw.project.symbol.reserved_type_name`. This prevents an unreachable
declaration behind built-in precedence.

## 5. Accepted environment and explicit open-name policy

### 5.1 Ownership

`arcweft-lang-sema::env::nominal` SHALL own exact accepted nominal records,
open-name rules, identities, collision validation, and the catalog digest.
`EnvironmentBindingId` moves destructively from registration internals to
`arcweft_lang_sema::env::identity`; imports are updated and no compatibility
re-export remains.

### 5.2 Exact record types

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RustPackageId(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedNominalOwnerId {
    Standard,
    Environment(EnvironmentBindingId),
    RustPackage(RustPackageId),
    Character(CharacterId),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedNominalId {
    owner: AcceptedNominalOwnerId,
    canonical_path: TypePath,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedNominalOrigin {
    Standard,
    Domain,
    NominalRecord,
    EnumInventory,
    RustExport,
    Character,
    Adapter,
    Test,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptedNominalSemantics {
    Exact(TypeKind),
    Opaque,
    Character(CharacterNominalType),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedNominalRecord {
    id: AcceptedNominalId,
    arity: u16,
    semantics: AcceptedNominalSemantics,
    origin: AcceptedNominalOrigin,
    source: Option<SourceSpan>,
}
```

`Exact` and `Character` require arity zero. `Opaque` yields an
`AcceptedNominalType` with checked arguments. There are no defaults,
variadics, or higher-kinded parameters.

### 5.3 Open rules

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpenNominalRuleId {
    owner: EnvironmentBindingId,
    ordinal: u32,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OpenNominalScope {
    AcceptedWorld,
    ExactModule(CanonicalModulePath),
    ModuleSubtree(CanonicalModulePath),
    DetachedOnly,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OpenNominalPattern {
    Exact(TypePath),
    Namespace {
        prefix: TypePath,
        min_tail_segments: u16,
        max_tail_segments: u16,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OpenNominalArity {
    Exact(u16),
    Inclusive { minimum: u16, maximum: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenNominalRule {
    id: OpenNominalRuleId,
    scope: OpenNominalScope,
    pattern: OpenNominalPattern,
    arity: OpenNominalArity,
    source: Option<SourceSpan>,
}
```

There is no global wildcard. A namespace prefix is nonempty, is not a reserved
built-in, and requires at least one unmatched tail segment. Its maximum tail is
16. Rule arity is bounded by 256. Rules whose module scopes, path patterns,
and arity ranges overlap are rejected atomically at environment construction.
Therefore open resolution is single-valued and requires no tie breaker.

`AcceptedWorld` rules are legal only in a `RegisteredTypeCheckEnv`;
`DetachedOnly` rules are legal only in a detached `TypeCheckEnv`. Exact and
module-scoped rules preserve adapter/domain/test use without validating an
arbitrary spelling.

### 5.4 Catalog and migration

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedNominalCatalogDigest([u8; 32]);

#[derive(Clone, Debug)]
pub struct AcceptedNominalCatalog {
    exact: BTreeMap<TypePath, AcceptedNominalRecord>,
    open: BTreeMap<OpenNominalRuleId, OpenNominalRule>,
    digest: AcceptedNominalCatalogDigest,
}
```

`TypeCheckEnv` owns one catalog and exposes
`try_with_nominal_record`, `try_with_open_nominal_rule`, and
`nominal_catalog`. Every standard/domain name, nominal record, enum inventory,
Rust export, character type, adapter type, and test type is projected into
this catalog by its owner.

The existing generic `symbols`, `nominal_records`, enum inventory, and
`rust_packages` storage may remain for their non-type semantic duties, but
authored type resolution SHALL NOT consult those legacy maps. Any map whose
only purpose was type-name acceptance is removed in the same cut. There is no
dual read.

Source-backed external declarations remain `ExternalDeclarationId`s in the
unified project table. `RegisteredExternalOwner` maps them to a current
environment or character owner. Sema then returns an external result without
creating a fake `ProjectNominalDeclarationId`.

Catalog construction is fallible and atomic:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedNominalCatalogLimitKind {
    ExactRecords,
    OpenRules,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenNominalPatternError {
    EmptyNamespacePrefix,
    ReservedPath,
    ZeroTail,
    InvertedTailRange,
    TailMaximumExceeded { maximum: u16, allowed: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptedNominalCatalogError {
    DuplicateExactPath {
        path: TypePath,
        first: AcceptedNominalId,
        duplicate: AcceptedNominalId,
    },
    ReservedPath {
        path: TypePath,
    },
    InvalidArity {
        source: Option<SourceSpan>,
        minimum: u16,
        maximum: u16,
    },
    InvalidOpenPattern {
        rule: OpenNominalRuleId,
        reason: OpenNominalPatternError,
    },
    OverlappingOpenRules {
        first: OpenNominalRuleId,
        second: OpenNominalRuleId,
    },
    InvalidScope {
        rule: OpenNominalRuleId,
        scope: OpenNominalScope,
    },
    Limit {
        kind: AcceptedNominalCatalogLimitKind,
        observed: u64,
        maximum: u64,
    },
}
```

The environment builder returns no updated `TypeCheckEnv` or
`RegisteredTypeCheckEnv` on error. Exact accepted records and open rules also
reject reserved built-in paths.

## 6. Semantic type carriers

`arcweft-lang-sema::types` SHALL own these carriers because `TypeKind` uses
them directly:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypePoisonId(u32);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GenericTypeOwnerId {
    Callable(CallableDeclarationId),
    Nominal(ProjectNominalDeclarationId),
    AcceptedSource(SourceSpan),
    Detached(DetachedTypeOwnerId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DetachedTypeOwnerId(u64);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GenericTypeParameterId {
    owner: GenericTypeOwnerId,
    ordinal: u16,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProjectNominalType {
    declaration: ProjectNominalDeclarationId,
    arguments: Box<[TypeKind]>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AcceptedNominalType {
    declaration: AcceptedNominalId,
    arguments: Box<[TypeKind]>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct OpenNominalType {
    rule: OpenNominalRuleId,
    path: TypePath,
    arguments: Box<[TypeKind]>,
}
```

`TypeKind` is extended in its owner enum:

```rust
pub enum TypeKind {
    // existing variants, updated recursively
    GenericParam(GenericTypeParameterId),
    ProjectNominal(ProjectNominalType),
    AcceptedNominal(AcceptedNominalType),
    OpenNominal(OpenNominalType),
    Error(TypePoisonId),
    Named(String),
    // remaining existing variants
}
```

The string form of `GenericParam` is removed. `Named(String)` remains only for
internal or host-produced semantic values that have not originated from an
authored `TypeRef`; the authored resolver never constructs it as a fallback.

Every inherent recursive `TypeKind` operation—source labels, ordering,
compatibility, mismatch paths, substitution, effect-row traversal, choice
normalization, registration integrity, and any exhaustive visitor—is updated
in the owner implementation. `Error` is recovery-compatible with any type for
the sole purpose of suppressing cascades. It is never serializable, lowerable,
exportable, callable-schema-valid, or accepted as a final compilation type.

A project type alias does not produce a `ProjectNominal` wrapper. Its distinct
declaration ID is retained in `ResolvedAliasReference` and
`AliasExpansionFact`, while compatibility and lowering use the normalized
target. Thus alias identity remains observable without making aliases runtime
nominal types.

The successful `impl From<&TypeRef> for TypeKind` is deleted. A private helper
may convert already-resolved built-in structure only if its argument type makes
unresolved names unrepresentable; it may not accept `TypeRef`.

## 7. One bounded recursive resolution operation

### 7.1 Inputs

`arcweft-lang-sema::nominal` SHALL own:

```rust
#[derive(Clone, Debug)]
pub struct GenericTypeBinding {
    id: GenericTypeParameterId,
    name: ModuleSegment,
    source: TypeSourceEvidence,
}

#[derive(Clone, Debug)]
pub struct GenericTypeScope {
    bindings: BTreeMap<ModuleSegment, GenericTypeBinding>,
    fingerprint: GenericTypeScopeFingerprint,
}

#[derive(Clone, Debug)]
pub enum SelfTypeScope {
    Absent,
    Known(TypeKind),
    Poisoned(TypePoisonId),
}

pub enum TypeResolutionWorld<'a> {
    Accepted {
        symbols: &'a ProjectSymbolTable,
        environment: &'a RegisteredTypeCheckEnv,
    },
    Detached {
        environment: &'a TypeCheckEnv,
    },
}

pub enum AuthoredTypeInput<'a> {
    Accepted(&'a SourceBackedTypeRef),
    Detached(&'a AuthoredTypeRef),
}

pub struct TypeResolutionInput<'a> {
    authored: AuthoredTypeInput<'a>,
    current_module: Option<&'a CanonicalModulePath>,
    world: TypeResolutionWorld<'a>,
    generics: &'a GenericTypeScope,
    self_scope: SelfTypeScope,
    limits: NominalResolutionLimits,
}
```

Fields are private. Constructors are:

```rust
impl<'a> TypeResolutionInput<'a> {
    pub fn accepted(
        authored: &'a SourceBackedTypeRef,
        current_module: &'a CanonicalModulePath,
        symbols: &'a ProjectSymbolTable,
        environment: &'a RegisteredTypeCheckEnv,
        generics: &'a GenericTypeScope,
        self_scope: SelfTypeScope,
        limits: NominalResolutionLimits,
    ) -> Result<Self, TypeResolutionInputError>;

    pub fn detached(
        authored: &'a AuthoredTypeRef,
        current_module: Option<&'a CanonicalModulePath>,
        environment: &'a TypeCheckEnv,
        generics: &'a GenericTypeScope,
        self_scope: SelfTypeScope,
        limits: NominalResolutionLimits,
    ) -> Self;
}
```

The accepted constructor proves:

- exact symbol/environment world equality;
- exact symbol/environment revision equality;
- current module presence in the table;
- module source identity equality with every root/source-map span;
- no stale document revision; and
- production limits not exceeding the compiled resolver schema.

Failure is `TypeResolutionInputError` and no type diagnostic is fabricated.
Compiler/LSP rebuild the accepted snapshot.

### 7.2 Product

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeSourceEvidence {
    local: TextRange,
    project: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalNominalResolution {
    Accepted {
        external: ExternalDeclarationId,
        nominal: AcceptedNominalType,
    },
    Exact {
        external: ExternalDeclarationId,
        ty: TypeKind,
        accepted: AcceptedNominalId,
    },
    Character {
        external: ExternalDeclarationId,
        nominal: CharacterNominalType,
        accepted: AcceptedNominalId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOpenNominal {
    rule: OpenNominalRuleId,
    path: TypePath,
    arguments: Box<[TypeKind]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAliasReference {
    declaration: ProjectNominalDeclarationId,
    arguments: Box<[TypeKind]>,
    normalized: TypeKind,
    use_source: TypeSourceEvidence,
    declaration_source: SourceSpan,
    target_source: TypeSourceEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeNameResolution {
    Builtin(BuiltinTypeConstructor),
    Generic(GenericTypeParameterId),
    SelfType(TypeKind),
    TraitHead(TypePath),
    Projection,
    Project(ProjectNominalType),
    Alias(ResolvedAliasReference),
    External(ExternalNominalResolution),
    Accepted(AcceptedNominalType),
    Open(ResolvedOpenNominal),
    Failed(TypeResolutionFailure),
    Poisoned(TypePoisonId),
    DetachedUnavailable(DetachedNominalEvidence),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeNode {
    node: TypeRefNodePath,
    source: TypeSourceEvidence,
    outcome: TypeNameResolution,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AliasExpansionFact {
    alias: ProjectNominalDeclarationId,
    arguments: Box<[TypeKind]>,
    substitution: Box<[(GenericTypeParameterId, TypeKind)]>,
    normalized: TypeKind,
    use_source: TypeSourceEvidence,
    declaration_source: SourceSpan,
    target_source: TypeSourceEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeProduct {
    recovered: TypeKind,
    nodes: Box<[ResolvedTypeNode]>,
    aliases: Box<[AliasExpansionFact]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoisonedTypeRef {
    product: ResolvedTypeProduct,
    causes: Box<[TypePoisonId]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetachedTypeRef {
    product: ResolvedTypeProduct,
    unavailable: Box<[TypeRefNodePath]>,
    causes: Box<[TypePoisonId]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedTypeRefOutcome {
    Complete(ResolvedTypeProduct),
    Poisoned(PoisonedTypeRef),
    Detached(DetachedTypeRef),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeResolutionReport {
    outcome: ResolvedTypeRefOutcome,
    diagnostics: Box<[NominalTypeDiagnostic]>,
    poisons: Box<[TypePoisonRecord]>,
    omitted_diagnostics: u64,
    work_charged: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NominalResolutionLimitKind {
    TypeNodesPerReference,
    RecursiveTypeDepth,
    GenericArgumentsPerApplication,
    AliasExpansionDepth,
    AliasExpansionNodes,
    DiagnosticsPerTypeReference,
    RelatedLabelsPerDiagnostic,
    WorkPerReference,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NominalResolutionLimits {
    type_nodes_per_reference: u64,
    recursive_type_depth: u16,
    generic_arguments_per_application: u16,
    alias_expansion_depth: u16,
    alias_expansion_nodes: u64,
    diagnostics_per_type_reference: u16,
    related_labels_per_diagnostic: u16,
    work_per_reference: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NominalAggregationLimits {
    diagnostics_per_document: u16,
    diagnostics_per_project: u16,
    work_per_project: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct AcceptedNominalCatalogLimits {
    exact_records: u16,
    open_rules: u16,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeArityTarget {
    Builtin(BuiltinTypeConstructor),
    Project(ProjectNominalDeclarationId),
    Accepted(AcceptedNominalId),
    Open(OpenNominalRuleId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypeArityExpectation {
    Exact(u16),
    Inclusive { minimum: u16, maximum: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetachedNominalEvidence {
    path: TypePath,
    source: TypeSourceEvidence,
    reason: DetachedNominalReason,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DetachedNominalReason {
    ProjectWorldUnavailable,
    ModuleUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeResolutionFailure {
    Unknown {
        path: TypePath,
    },
    Ambiguous {
        path: TypePath,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    Inaccessible {
        path: TypePath,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    WrongKind {
        path: TypePath,
        actual: ProjectTypeCandidate,
    },
    WrongArity {
        target: TypeArityTarget,
        expected: TypeArityExpectation,
        actual: u16,
    },
    CyclicAlias {
        cycle: Box<[ProjectNominalDeclarationId]>,
    },
    SelfUnavailable,
    Limit {
        kind: NominalResolutionLimitKind,
        observed: u64,
        maximum: u64,
    },
    WorkOverflow {
        attempted: u64,
        maximum: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NominalResolutionLimitsError {
    Zero {
        kind: NominalResolutionLimitKind,
    },
    AboveHardCeiling {
        kind: NominalResolutionLimitKind,
        value: u64,
        ceiling: u64,
    },
    DiagnosticWorkInconsistent {
        diagnostics: u16,
        related_labels: u16,
        work: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeResolutionInputError {
    StaleWorld {
        symbol_world: ProjectSymbolWorldId,
        environment_world: ProjectSymbolWorldId,
    },
    StaleRevision {
        symbol_revision: ProjectSymbolRevision,
        environment_revision: ProjectSymbolRevision,
    },
    UnknownModule {
        module: CanonicalModulePath,
    },
    SourceMismatch {
        module: CanonicalModulePath,
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    RegisteredEnvironmentIntegrity {
        external: ExternalDeclarationId,
        reason: ExternalOwnerLookupError,
    },
    InvalidLimits {
        reason: NominalResolutionLimitsError,
    },
}
```

All fields have const/read-only accessors.
`NominalResolutionLimits::PRODUCTION`,
`NominalAggregationLimits::PRODUCTION`, and
`AcceptedNominalCatalogLimits::PRODUCTION` are the exact values in section 13.
Their `try_new` constructors reject zero maxima, values above parser/project
hard ceilings, and inconsistent diagnostic/work caps. Resolver and report
constructors are `pub(crate)`; consumers receive immutable reports.

The one public operation is:

```rust
pub fn resolve_type_ref(
    input: TypeResolutionInput<'_>,
) -> Result<TypeResolutionReport, TypeResolutionInputError>;
```

No other public function successfully resolves a project type path.

### 7.3 Name precedence

For every path or generic head, the resolver applies this order:

1. `TypeRef::Recovery` reuses its syntax-origin poison and emits no nominal
   diagnostic.
2. Exact one-segment `Self` uses `SelfTypeScope`.
3. An unqualified one-segment generic name uses the nearest
   `GenericTypeScope` binding.
4. A reserved language built-in uses `BuiltinTypeConstructor`.
5. `ProjectSymbolTable::resolve_type_target` is queried.
   - nominal: project struct/enum/alias;
   - external: registered external owner lookup;
   - ambiguous/inaccessible/wrong-kind: authoritative failure with no fallback;
   - unknown only: continue.
6. Exact `AcceptedNominalCatalog` lookup.
7. One explicitly matching open rule.
8. In an accepted world, authoritative unknown.
9. In a detached world, `DetachedUnavailable` with no fabricated project
   diagnostic.

Qualified names never resolve as a generic, `Self`, or built-in. Generics
shadow project/environment/open single-segment names. Built-ins are reserved,
so publication prevents project/environment collisions. Project ambiguity,
inaccessibility, and wrong-kind never fall through to environment or open
evidence.

### 7.4 Built-ins

`BuiltinTypeConstructor` is a closed owner enum. The final table is:

| spelling | arity | semantic result |
|---|---:|---|
| `bool`, integer/floating primitives, `String`, `char`, `Bytes`, `Unit`, `Never` | 0 | existing atomic `TypeKind` |
| `Vec`, `Slice`, `Seq`, `Option`, `Probe`, `ThreadHandle`, `Shared` | 1 | existing constructor |
| `Array` | 2 | existing item/const-length constructor |
| `OrderedMap`, `SortedMap`, `BTreeMap`, `Result`, `Need`, `Stream`, `Source` | 2 | existing constructor |
| `Speaker`, `SpeakerPreset` | 1 | existing entity-family constructor |

Array argument 1 must be a `ConstInt`; otherwise wrong-kind is reported at the
argument. Domain atoms such as `ArcError`, `Duration`, `DataFormat`, character
families, reducer/agent errors, and adapter-specific names are exact accepted
records, not language built-ins. `ArcResult` is not a built-in.

## 8. Recursive traversal and accumulation

The resolver walks in source structural order and resolves every child in:

- tuples;
- function parameters and return type;
- choices;
- generic arguments;
- every type argument and associated binding value nested in a trait bound;
- projection subjects;
- references and slices.

Effect-row labels remain owned by the existing effect-row authority. The
nominal walker charges the function type node and recurses into its types but
does not reinterpret effect labels.

Every declaration/checking owner listed in section 2.3 calls the same operation.
No owner is allowed to call `TypeKind::from`, inspect a display label, or scan
source text.

Independent sibling failures accumulate. A failed node becomes
`TypeKind::Error(poison_id)`, while the outer tuple/function/choice/container
shape remains available for recovery. Parent nodes carry the sorted union of
child poison IDs. A poisoned child suppresses only diagnostics whose proof
depends on that child; unrelated siblings and declarations continue.

The nominal resolver records a trait-bound head as `TraitHead(TypePath)` and
resolves every type argument and associated binding value. Selection of the
trait declaration itself remains with the existing trait authority; it is not
a nominal declaration and is not a second nominal resolver. An unknown trait
head uses that authority's existing unknown-trait diagnostic. Unknown type
paths nested in the bound use this contract's nominal diagnostic.

Projection resolution resolves the subject recursively and records the
associated member token. The nominal authority does not prove that the member
exists; the existing trait/projection checker does so after subject resolution.
A poisoned subject suppresses projection-member follow-ons. The member spelling
is never treated as a nominal path.

## 9. Alias arity, substitution, chains, and cycles

Project struct, enum, alias, accepted, open, and built-in applications use
exact arity. Project nominal parameters have no defaults, variadics, const
parameters, or higher-kinded parameters.

A path without `<...>` is an application with zero arguments. The resolver
always resolves every authored argument first, even when arity is wrong, so
independent child errors are retained. Wrong arity prevents target expansion
and poisons the application node.

Alias substitution is capture-free and keyed by
`GenericTypeParameterId`, never by a string. The alias target is interpreted
in the alias declaration module with an alias-owned generic scope. Use-site
arguments retain use-site node facts. Declaration-owned target nodes retain
declaration source spans.

Chains resolve argument children, bind parameters, substitute, and normalize
repeatedly. The resolver retains one `AliasExpansionFact` per step in use-to-
target order. Imported and re-exported spellings select the same alias ID.

Cycle detection uses the alias declaration-ID stack, not names or instantiated
argument labels. Re-entering an ID is cyclic even when arguments differ. The
cycle payload is rotated to the lexicographically smallest declaration ID and
then follows expansion order, making diagnostics insertion-order independent.
The primary label is the closing target reference. Secondary labels are each
alias name and target reference in cycle order, capped deterministically.

Alias target validation is eager during declaration checking. An unknown or
cyclic target therefore produces one declaration-owned diagnostic even when
the alias is unused. A use report may relate the use site but deduplication is
by alias ID and target node, so repeated uses do not multiply the root error.

Anonymous-choice duplicate detection runs only after complete alias
normalization and substitution. Alternatives containing `TypeKind::Error` are
excluded from duplicate comparison. Existing choice diagnostics and wording
remain authoritative. TM-074 therefore emits only the prior choice duplicate
error.

## 10. Structured diagnostics

### 10.1 Project-link codes

Existing codes remain. The owner enum gains:

```text
aw.project.symbol.unknown_import
aw.project.symbol.cyclic_import
aw.project.symbol.reserved_type_name
aw.project.symbol.invalid_nominal_declaration
```

`DuplicateDeclaration` covers nominal cross-family collisions and keeps its
existing stable code.

### 10.2 Semantic nominal codes

```text
sema.nominal.unknown_type
sema.nominal.ambiguous_type
sema.nominal.inaccessible_type
sema.nominal.wrong_kind
sema.nominal.wrong_arity
sema.nominal.cyclic_alias
sema.nominal.self_unavailable
sema.nominal.limit
sema.nominal.work_overflow
```

Exact payloads:

```rust
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum NominalTypeDiagnosticKind {
    Unknown {
        path: TypePath,
    },
    Ambiguous {
        path: TypePath,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    Inaccessible {
        path: TypePath,
        candidates: Box<[ProjectTypeCandidate]>,
    },
    WrongKind {
        path: TypePath,
        actual: ProjectTypeCandidate,
    },
    WrongArity {
        target: TypeArityTarget,
        expected: TypeArityExpectation,
        actual: u16,
    },
    CyclicAlias {
        cycle: Box<[ProjectNominalDeclarationId]>,
    },
    SelfUnavailable,
    Limit {
        kind: NominalResolutionLimitKind,
        observed: u64,
        maximum: u64,
    },
    WorkOverflow {
        attempted: u64,
        maximum: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalDiagnosticRelated {
    source: TypeSourceEvidence,
    message: NominalRelatedMessage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalTypeDiagnostic {
    poison: TypePoisonId,
    kind: NominalTypeDiagnosticKind,
    primary: TypeSourceEvidence,
    secondary: Box<[NominalDiagnosticRelated]>,
}
```

`NominalTypeDiagnostic::to_source_diagnostic` is available only when the
primary has a `SourceSpan`. It builds the existing `arcweft_source::Diagnostic`
and exact primary/secondary labels. Detached diagnostics retain local ranges
but are not projected as project diagnostics.

Label rules:

- unknown: exact path/constructor head;
- ambiguous: exact head; each candidate declaration and binding site;
- inaccessible: exact head; hidden declaration and blocking import/binding;
- wrong kind: exact head; actual declaration or module/import site;
- wrong arity: exact constructor head; declaration generic-list or whole
  declaration when expected arity is zero;
- cyclic alias: closing alias-target head; cycle declaration and target sites;
- self unavailable: exact `Self` head;
- limit/work: smallest affected node head, with limit facts in payload.

Diagnostics sort by `(document id, source revision, byte range, code, typed
subject IDs)`. Detached diagnostics sort by local range, code, and typed
subject. Deduplication key is `(code, primary identity-or-local range, typed
subject/candidate IDs)`. Related labels sort by source identity and range and
are deduplicated.

Caps retain omitted counts and never choose rows by insertion order.

## 11. Poison discipline and checked return boundaries

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TypePoisonOrigin {
    SyntaxTypeDiagnostic,
    NominalTypeDiagnostic,
    UpstreamTypeDiagnostic,
    DetachedUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypePoisonRecord {
    id: TypePoisonId,
    origin: TypePoisonOrigin,
    primary: TypeSourceEvidence,
    authoritative_for_annotation: bool,
}
```

`CheckedReturnTarget` is not redesigned. The checker owns a side record:

```rust
pub struct CheckedReturnTargetEvidence {
    target: CheckedReturnTarget,
    annotation_source: Option<TypeSourceEvidence>,
    poison_causes: Box<[TypePoisonId]>,
}
```

`CheckedReturnTarget::Unresolved` may be constructed only when a return
annotation report is poisoned by at least one record with
`authoritative_for_annotation = true`. The report and diagnostic are stored
before body checking. A detached-unavailable node uses `TypeKind::Error` with
`TypePoisonOrigin::DetachedUnavailable` and
`authoritative_for_annotation = false`. It cannot produce `Unresolved`;
compiler and accepted LSP paths must first build an accepted world.

When a nearest return boundary is `Unresolved`, Try and propagating Await:

- still check the operand;
- retain the operand success type for expression recovery;
- do not emit target-missing, non-result-boundary, or error-mismatch diagnostics
  whose proof needs the boundary type;
- do not alter nearest-boundary selection; and
- do not suppress unrelated operand diagnostics.

Thus `fn f() -> Unknown { result_t_e? }` emits only
`sema.nominal.unknown_type`, stores an unresolved checked boundary, retains
the operand success type, and emits no `sema.try.*` cascade. Prefix Try,
postfix Try, and propagating Await use the same gate.

A known boundary behind any generic result alias is determined from the alias
declaration ID, arity check, substitution, and normalized target. No alias
spelling participates.

## 12. Entry resolver reconciliation

The final entry path consumes the same `ProjectSymbolTable`,
`ResolvedTypeRefOutcome`, project nominal records, and alias facts as normal
checking.

The following successful responsibilities are deleted from
`NominalSchemaResolver`:

- project struct/enum/alias inventory;
- local/qualified/parent/child name selection;
- import, glob, alias, and re-export reconstruction;
- visibility selection;
- alias-target lookup;
- alias recursion detection;
- enum payload string parsing;
- context-free `TypeKind::from` fallback.

`EntryContractBuilder::canonical_type_ref` no longer performs project or alias
lookup. `canonical_constructor("ArcResult")` is deleted. Entry canonical
construction consumes a resolved semantic type and alias trace.

The entry-specific responsibilities that remain are:

- state/event role admissibility policy;
- schema-shape expansion from a selected struct/enum declaration record;
- entry-specific cycle/shape rules for recursive schemas;
- canonical entry contract comparison;
- entry role, effect, and callable contract diagnostics.

During migration, shared project facts land first. Entry lookup moves next and
the old successful methods are deleted in the same cut. There is never a
release or commit state with two successful project nominal resolvers.

## 13. Limits and deterministic work

The existing `ProjectSymbolLimits::PRODUCTION` values remain:

```text
aliases_per_module = 256
aliases_per_world  = 8,192
imports            = 32,768
diagnostics        = 128
work               = 262,144
```

The owner type gains:

```text
nominal_declarations_per_module = 1,024
nominal_declarations_per_world  = 16,384
nominal_members_per_declaration = 4,096
nominal_type_parameters         = 64
nominal_type_nodes_per_declaration = 16,384
```

`NominalResolutionLimits::PRODUCTION` is:

```text
type_nodes_per_reference       = 4,096
recursive_type_depth           = 256
generic_arguments_per_application = 256
alias_expansion_depth          = 64
alias_expansion_nodes          = 16,384
diagnostics_per_type_reference = 32
diagnostics_per_document       = 128
diagnostics_per_project        = 512
related_labels_per_diagnostic  = 32
work_per_reference             = 65,536
work_per_project               = 1,048,576
accepted_exact_nominals        = 4,096
open_nominal_rules             = 1,024
```

One work unit is charged for each visited type node, project candidate
inspected, exact/open catalog row inspected, alias parameter binding,
substituted target node, produced diagnostic, and produced related label.
Arithmetic uses checked addition. First limit crossing poisons the smallest
affected node; project-link limit crossing aborts publication. Work and
diagnostic totals are retained in reports.

## 14. Revision and caching

A project nominal ID contains the exact world and source-set revision. It
cannot compare equal across accepted revisions.

The accepted resolver rejects a stale symbol/environment/source combination
with `TypeResolutionInputError::StaleWorld`, `StaleRevision`, or
`SourceMismatch`. These are infrastructure errors, not unknown-name
diagnostics.

A checked type-reference cache key is exactly:

```text
(
  ProjectSymbolWorldId,
  ProjectSymbolRevision,
  current CanonicalModulePath,
  authored root SourceSpan,
  AuthoredTypeRef structural digest,
  GenericTypeScope fingerprint,
  SelfTypeScope fingerprint,
  AcceptedNominalCatalogDigest,
  nominal resolver schema version,
  NominalResolutionLimits value
)
```

The cache stores the complete report, including diagnostics, poison records,
omitted count, and work. No fact is reused across any differing component.
Alias normalization may have a subordinate key
`(alias ID, normalized argument semantic hashes, catalog digest, limits
version)` within the same world/revision.

Detached results are cacheable only inside the same detached HIR arena,
authored structural digest, generic/Self fingerprint, and environment digest.
They can never be promoted to accepted project proof.

Detached validation explicitly cannot prove project declaration existence,
module/import/re-export selection, visibility, cross-document collisions,
project alias targets, project definition/rename locations, accepted
world/revision freshness, or related labels in another document. Those nodes
are `DetachedUnavailable`; callers must not reinterpret that state as either
known or unknown.

Compiler and LSP publish/discard all project nominal caches atomically with the
accepted project snapshot.

## 15. Consumers and minimum APIs

### Sema

`TypeCheckReport` gains one `NominalResolutionIndex` keyed by exact authored
root and node source evidence. Normal function, method, flow, closure,
struct/enum/alias, trait/impl, extern, and entry checks use it. No display
string lookup is exposed.

### Entry checking

Entry receives `ResolvedTypeRefOutcome` and project declaration records. It
keeps only schema/role policy described in section 12.

### Compiler

The compiler constructs one accepted registered world, invokes checked
resolution before body propagation checks, projects typed nominal diagnostics,
and rejects any final `TypeKind::Error`. Stale input fails the accepted
transaction. No runtime or wire schema changes are part of this contract.

### Project semantic index

The internal project semantic index replaces string-keyed project nominal
projection with:

```rust
BTreeMap<ProjectNominalDeclarationId, ProjectNominalIndexRecord>
Box<[ProjectNominalReferenceEdge]>
```

Records and edges use typed IDs and source spans. Existing non-project,
agent-prelude type entries may remain exact accepted records. If the internal
schema version is persisted, it switches directly to the next version; no dual
reader or compatibility field is added.

### LSP

The accepted snapshot retains the exact world/revision
`NominalResolutionIndex`, declaration records, and reference edges.

- diagnostics use structured nominal diagnostics;
- hover shows selected kind, generic arguments, and alias expansion trace;
- definition goes to the original struct/enum/alias or source-backed external
  declaration;
- completion uses `visible_type_bindings` plus accepted/open catalog facts;
- rename is by project declaration ID and exact resolved reference edges;
- alias rename targets the alias ID, not its normalized target;
- built-in, generic, open, character-without-source, and environment-only
  accepted names are not project rename targets.

No LSP-only resolver, source scan, display parse, or guessed definition is
allowed.

### Tests

Tests may construct typed catalogs and worlds through intentional test
constructors behind `cfg(test)`. A test-only open rule must still carry an
`EnvironmentBindingId`, scope, pattern, and arity. Tests may not make arbitrary
`Named` fallback valid.

## 16. Completion invariants

The implementation is complete only when all of the following hold:

1. every authored type owner uses `AuthoredTypeRef`;
2. every accepted project nominal declaration is in the unified table;
3. every project type lookup goes through `resolve_type_target`;
4. every recursive authored type check goes through `resolve_type_ref`;
5. aliases are selected by ID and substituted by typed parameter ID;
6. the normal checker has no successful context-free `TypeRef` conversion;
7. entry has no successful project/import/alias resolver;
8. `ArcResult` and `Unknown` have no spelling branch;
9. an unresolved checked return target has prior poison evidence;
10. project and environment worlds/revisions match exactly;
11. detached HIR never fabricates source identity;
12. diagnostics and limits are deterministic;
13. compiler, project index, and LSP consume typed facts; and
14. every row of `TEST_MATRIX.csv` passes.
