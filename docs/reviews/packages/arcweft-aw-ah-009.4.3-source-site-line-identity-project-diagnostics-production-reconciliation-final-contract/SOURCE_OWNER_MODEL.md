# Source owner model

## 1. Package-aware module identity

The final public HIR key is:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirModuleKey {
    package: CallablePackageId,
    path: CanonicalModulePath,
    source: SourceDocumentIdentity,
}

impl HirModuleKey {
    pub fn try_new(
        package: CallablePackageId,
        path: CanonicalModulePath,
        document: &SourceDocument,
    ) -> Result<Self, HirModuleKeyError>;

    pub const fn package(&self) -> &CallablePackageId;
    pub const fn path(&self) -> &CanonicalModulePath;
    pub const fn source(&self) -> &SourceDocumentIdentity;
}
```

`try_new` clones `document.identity()`. Raw-field construction is crate-private.
The key has no Serde implementation. `HirSnapshotId` remains the session-local
snapshot identity; `HirModuleKey` is exact content provenance.

`LoweringRequest<'a>` contains:

```rust
pub struct LoweringRequest<'a> {
    key: HirModuleKey,
    syntax: &'a ParsedSource,
    document: &'a SourceDocument,
}
```

Its checked constructor requires:

- `syntax.source_document().identity() == document.identity()`;
- `key.source() == document.identity()`;
- the syntax snapshot and document belong to the same source lineage;
- the requested canonical module agrees with the accepted loader topology; and
- all checked arithmetic needed for source spans succeeds.

A mismatch returns a fatal `HirLowerFatalError::SourceIdentityMismatch`; it does
not allocate an ExprId or publish a HIR snapshot.

## 2. Project module identity

Package-qualified lookup uses:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirPackageModuleKey {
    package: CallablePackageId,
    path: CanonicalModulePath,
}
```

It is projected from `HirModuleKey`; callers cannot supply it independently to
a module snapshot. Final `HirProject` retains one root package and modules keyed
by `HirPackageModuleKey`. The exact source identity remains on each module key.

## 3. Closed line owner

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirDialogueLineSourceOwner {
    Flow(HirDialogueFlowOwner),
    Callable(CallableDeclarationId),
    Ownerless,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirDialogueFlowOwner {
    id: PublicId,
}
```

`HirDialogueFlowOwner::try_new` accepts only a complete `flow.<tail>` PublicId.
It provides `id()`. It does not retain a flow display name, local alias, callee,
or Character.

A callable owner is the existing `CallableDeclarationId`. Candidate
construction checks that its package and module equal the application module
key. `CallableDeclarationOwner`, typed owner path, and name remain intact.

`Ownerless` is used for application expressions not contained by a typed flow
or callable declaration. It is not a fallback for failed owner discovery.

## 4. Named lexical scopes

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDialogueNamedScope {
    scope: ScopeId,
    segment: ModuleSegment,
    declaration: SourceSpan,
}
```

The vector is outermost-first. A scope contributes only when the source scope
has a valid authored name. `ScopeId` remains the lexical identity; the validated
`ModuleSegment` is the ID segment. Unnamed blocks, branches, loops, and closures
retain ordinary HIR scope ownership but do not add line-ID text.

The inclusive count cannot exceed `HirLimit::Scopes.maximum()` (16,384).

## 5. Complete source site

Every module candidate retains:

```rust
pub(crate) struct HirDialogueLineSourceSite {
    application: ExprId,
    owner: HirDialogueLineSourceOwner,
    named_scopes: Arc<[HirDialogueNamedScope]>,
    source_order: DialogueLineSourceOrder,
    application_span: SourceSpan,
    id_coordinate_span: Option<SourceSpan>,
    text_key_coordinate_span: Option<SourceSpan>,
}
```

The application `ExprId` is the source-backed root selected by AW-AH-009.4.2.
Coordinate spans come from its HIR component map, not source slicing. All spans
must share `HirModuleKey.source()`.

`DialogueLineSourceOrder` is a checked `u32` assigned by HIR traversal. It is a
deterministic coordinate, not semantic identity and not persisted.

## 6. Prefix algorithms

### Flow

```text
prefix = "say." + flow_id.as_str()
for scope in remaining_scopes:
    prefix += "." + scope.segment().as_str()
```

No package/module is added because the frozen complete flow ID is already the
owner identity. Cross-package duplicate flow IDs are caught by the project
namespace.

### Callable

```text
prefix segments:
  say
  fn
  callable.package
  callable.module.segments (without `crate`)
  callable.owner.as_str()
  callable.owner_path segments
  callable.name
  remaining named-scope segments
```

Each typed segment is appended with checked length arithmetic. The package is a
validated `CallablePackageId`; module/owner-path segments are existing
`ModuleSegment` values.

## 7. Session and persistence boundary

| Fact | Session-only | Durable/persisted later |
|---|---:|---:|
| `HirSnapshotId`, `ExprId`, `ScopeId` | yes | no |
| `SourceDocumentIdentity`, `SourceSpan` | exact build/tooling provenance | not runtime semantic identity |
| `HirModuleKey`, owner/scopes | HIR/project fact | no direct runtime encoding |
| `DialogueLineId` | no | yes, through checked later codecs |
| `DialogueTextKey` | no | yes, through checked later codecs |
| source order/origin | HIR/tooling | only if a later explicit format selects it |

No source display string, URI basename, alias, or Character label is durable
identity.

## 8. Exact visibility and derive matrix

| Type | Visibility | Derives | Construction and access |
|---|---|---|---|
| `HirModuleKey` | public in `arcweft_lang_hir::module` | `Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd` | public checked `try_new`; public read-only accessors; fields private |
| `HirPackageModuleKey` | public in `arcweft_lang_hir::project` | `Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd` | crate-owned projection from `HirModuleKey`; public accessors |
| `HirDialogueFlowOwner` | public in `arcweft_lang_hir::line_identity` | `Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd` | public checked `try_new`; `id()` accessor |
| `HirDialogueLineSourceOwner` | public in the same module | `Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd` | variants public for inspection; payload fields remain private |
| `HirDialogueNamedScope` | public read-only inspection | `Clone, Debug, Eq, PartialEq` | crate-private checked constructor; `scope()`, `segment()`, `declaration()` |
| `DialogueLineSourceOrder` | public coordinate | `Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd` | crate-private checked constructor; `get() -> u32` |
| `HirDialogueLineSourceSite` | crate-private staged fact | `Clone, Debug, Eq, PartialEq` | candidate builder only |
| `HirDialogueLineCandidates` | crate-private module product | `Clone, Debug, Eq, PartialEq` | module transaction only; immutable slice accessor inside HIR crate |

No type in this table derives `Serialize` or `Deserialize`. The accepted public
inspection record later copies or shares the site facts immutably; it does not
make their constructors public.

## 9. Stored versus projected facts

| Fact | Stored exactly once | Projection rule |
|---|---|---|
| package/module/source | `HirModuleKey` | every site/accepted record references or clones the checked key; it never independently parses values |
| flow identity | `HirDialogueFlowOwner` | projected from the typed flow declaration during lowering |
| callable identity | existing `CallableDeclarationId` | reused by reference/value; package/module equality checked against module key |
| lexical scope identity | HIR arena `ScopeId` | contributing named scopes store the `ScopeId`, checked segment, and declaration span |
| application identity | HIR arena `ExprId` | site stores the source-backed application ID fixed by AW-AH-009.4.2 |
| component ranges | AW-AH-009.4.2 HIR source-component map | exact SourceSpans copied into the site after source-identity validation |
| source order | module lowering traversal | checked monotonic coordinate, not inferred from display order later |
