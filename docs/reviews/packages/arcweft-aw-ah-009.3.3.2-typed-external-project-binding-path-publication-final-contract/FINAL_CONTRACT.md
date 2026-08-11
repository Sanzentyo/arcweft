# Final contract

## 1. Scope and preserved substrate

This correction owns only the missing typed evidence for source-visible project bindings. The following current production substrate is accepted and must not be redesigned:

- `CallableName`, `CallablePath`, and `ProjectCallablePath`;
- `ProjectNameBinding` and every callable catalog record/container type;
- project and environment callable IDs, schemas, limits, diagnostics, and work accounting;
- the shared callable resolver and its project-before-environment shadow behavior;
- `ProjectSymbolTable` as the only project-symbol resolver;
- `ExternalDeclarationSeed::canonical_path` as the opaque external canonical identity;
- the current registered-world construction and accepted-world publication transaction;
- AW-AH-009.3.1 call ranges and AW-AH-009.3.2 request leasing/lifecycle;
- current import visibility, ambiguity, alias limits, fixed-point behavior, and diagnostic ordering.

The only demonstrated defect is the current `add_project_bindings` branch that calls `CallableName::try_new(spelling)` on a complete scope spelling and executes `continue` when the spelling contains `.`. This drops a valid qualified external binding from `ProjectCallableCatalog`.

## 2. Selected ownership model

### 2.1 Sole project-binding path owner

The existing syntax-owned types remain authoritative and unchanged:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectSymbolSegment(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectSymbolPath {
    root: ModulePathRoot,
    segments: Vec<ProjectSymbolSegment>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectSymbolPathError {
    #[error("project symbol path is empty")]
    Empty,
    #[error("project symbol segment is empty")]
    EmptySegment,
    #[error("invalid project symbol segment `{segment}`")]
    InvalidSegment { segment: String },
    #[error("implicit project symbol path has invalid first segment `{segment}`")]
    InvalidImplicitRoot { segment: String },
}
```

The current constructors and read-only accessors remain the only project-path construction API:

```rust
impl ProjectSymbolSegment {
    pub fn try_new(value: impl Into<String>) -> Result<Self, ProjectSymbolPathError>;
    pub fn as_str(&self) -> &str;
    pub fn try_as_module_segment(&self) -> Result<ModuleSegment, ModulePathError>;
}

impl ProjectSymbolPath {
    pub fn new(
        root: ModulePathRoot,
        segments: impl IntoIterator<Item = ProjectSymbolSegment>,
    ) -> Result<Self, ProjectSymbolPathError>;
    pub const fn root(&self) -> ModulePathRoot;
    pub fn segments(&self) -> &[ProjectSymbolSegment];
    pub fn last_segment(&self) -> &ProjectSymbolSegment;
}
```

`ProjectSymbolSegment` accepts a non-empty sequence of letters, numbers, `_`, or `-`. An implicit path's first segment must begin with a letter or `_`. Therefore its grammar is a strict subset of `CallableName`: every valid project segment is convertible to one callable segment, including a segment containing `-`, while `.`, `:`, `/`, `\\`, controls, and empty segments are rejected before HIR linking.

`SymbolPath` is not the source-visible binding-path owner. It remains the resolution/canonical carrier that deliberately permits an opaque external leaf.

### 2.2 Scope-local root invariant

Every binding installed in one `ProjectSymbolTable` scope has an implicit path. The containing `CanonicalModulePath` identifies the scope; a `crate`, `self`, or `super` root would duplicate or contradict that owner. Source references may use explicit roots, but linking resolves those references and installs an implicit destination binding.

## 3. Exact HIR public model

### 3.1 `ProjectDirectBinding`

The current string field and string constructor are directly replaced by:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectDirectBinding {
    module: CanonicalModulePath,
    path: ProjectSymbolPath,
    visibility: Option<Visibility>,
    source: SourceSpan,
    authored_alias: bool,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectDirectBindingError {
    #[error(
        "direct project binding path must use the implicit project root, found {root:?}"
    )]
    ExplicitRoot { root: ModulePathRoot },
}

impl ProjectDirectBinding {
    pub fn try_new(
        module: CanonicalModulePath,
        path: ProjectSymbolPath,
        visibility: Option<Visibility>,
        source: SourceSpan,
        authored_alias: bool,
    ) -> Result<Self, ProjectDirectBindingError>;

    pub const fn module(&self) -> &CanonicalModulePath;
    pub const fn path(&self) -> &ProjectSymbolPath;
    pub const fn visibility(&self) -> Option<Visibility>;
    pub const fn source(&self) -> &SourceSpan;
    pub const fn authored_alias(&self) -> bool;
}
```

Constructor behavior is exact:

```rust
pub fn try_new(
    module: CanonicalModulePath,
    path: ProjectSymbolPath,
    visibility: Option<Visibility>,
    source: SourceSpan,
    authored_alias: bool,
) -> Result<Self, ProjectDirectBindingError> {
    if path.root() != ModulePathRoot::ImplicitCrate {
        return Err(ProjectDirectBindingError::ExplicitRoot { root: path.root() });
    }
    Ok(Self {
        module,
        path,
        visibility,
        source,
        authored_alias,
    })
}
```

There is no `name: String`, `name()`, string-taking overload, `From<&str>`, dotted parser, deprecated wrapper, or compatibility constructor.

`ProjectDirectBindingError` is re-exported beside `ProjectDirectBinding` from `arcweft_lang_hir::symbol`.

### 3.2 Canonical identity remains separate

`ExternalDeclarationSeed` remains:

```rust
pub struct ExternalDeclarationSeed {
    canonical_path: SymbolPath,
    visibility: Option<Visibility>,
    declaration: SourceSpan,
    direct_bindings: Vec<ProjectDirectBinding>,
}
```

Its canonical path and each source-visible binding path have different roles:

- `canonical_path` identifies the external declaration independently of aliases and remains an opaque `SymbolPath` when the external domain requires one;
- each `ProjectDirectBinding::path` is a validated, segmented spelling installed into one source scope;
- two or more binding paths may target the same external declaration;
- an authored alias never replaces, normalizes, or mutates the canonical path;
- equality or hashing of the canonical path is never used as a substitute for binding-path equality.

For character `character.akane`, canonical identity may remain one opaque leaf while direct binding paths are `['character', 'akane']`, `['akane']`, and optionally `['hero']`.

## 4. Exact private HIR linker model

### 4.1 Scope row

`ScopeBinding` remains private to HIR and gains the exact typed path:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScopeBinding {
    pub(super) path: ProjectSymbolPath,
    pub(super) target: ProjectSymbolTargetId,
    pub(super) visibility: Option<Visibility>,
    pub(super) owner: CanonicalModulePath,
    pub(super) sites: Vec<SourceSpan>,
}
```

Its exact private constructors are:

```rust
impl ScopeBinding {
    fn new(
        path: ProjectSymbolPath,
        target: ProjectSymbolTargetId,
        visibility: Option<Visibility>,
        owner: CanonicalModulePath,
        sites: impl IntoIterator<Item = SourceSpan>,
    ) -> Self;

    fn rebound(
        &self,
        path: ProjectSymbolPath,
        owner: &CanonicalModulePath,
        visibility: Option<Visibility>,
        site: SourceSpan,
    ) -> Self;
}
```

`new` asserts the internal invariant `path.root() == ModulePathRoot::ImplicitCrate`, sorts/deduplicates sites with the existing span ordering, and stores the path without rendering or reparsing it. `rebound` keeps the target, receives the exact destination path, applies the existing requested visibility and owner, and retains the import site.

The table may retain its private lookup accelerator:

```rust
scopes: BTreeMap<CanonicalModulePath, BTreeMap<String, Vec<ScopeBinding>>>
```

The `String` key is not identity. `insert_scope_binding` computes it exactly once as `binding.path.to_string()` after the typed path exists. Nothing reads that string to reconstruct segments. This preserves current root-scope lookup behavior for an opaque external spelling without making it public evidence.

### 4.2 Insertion and coalescing

The exact private insertion API becomes:

```rust
fn insert_scope_binding(
    &mut self,
    module: &CanonicalModulePath,
    binding: ScopeBinding,
) -> bool;
```

The previous separate `name: String` parameter is deleted.

Rows coalesce only when all of these are equal:

1. `path`;
2. `target`;
3. `visibility`;
4. `owner`.

Coalescing merges, sorts, and deduplicates `sites`. A different path that targets the same declaration remains a separate binding. A different target at the same path remains a collision/ambiguity candidate.

After insertion or coalescing, each per-key vector is sorted by the typed tuple:

```text
(path, target, visibility, owner)
```

`Option<Visibility>` uses its existing derived `Ord`; site order is already canonical and does not distinguish rows after the four-field coalescing key.

### 4.3 Typed collision evidence

The public collision projection is directly replaced by:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSymbolBindingCollision {
    module: CanonicalModulePath,
    path: ProjectSymbolPath,
    expected: ProjectSymbolTargetId,
    conflicting: Vec<ProjectSymbolTargetId>,
    expected_sites: Vec<SourceSpan>,
    conflicting_sites: Vec<SourceSpan>,
}

impl ProjectSymbolBindingCollision {
    pub const fn module(&self) -> &CanonicalModulePath;
    pub const fn path(&self) -> &ProjectSymbolPath;
    pub const fn expected(&self) -> &ProjectSymbolTargetId;
    pub fn conflicting(&self) -> &[ProjectSymbolTargetId];
    pub fn expected_sites(&self) -> &[SourceSpan];
    pub fn conflicting_sites(&self) -> &[SourceSpan];
}
```

`spelling: String` and `spelling()` are deleted. Diagnostic rendering may format `path`, but no diagnostic or registrar parses that formatted value.

## 5. Exact typed import carrier

The current private `link_path(&ProjectSymbolPath) -> SymbolPath` helper is replaced by a carrier that retains both already typed meanings:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
struct LinkedProjectSymbolPath {
    reference: SymbolPath,
    unaliased_binding: ProjectSymbolPath,
}

impl LinkedProjectSymbolPath {
    fn try_new(path: &ProjectSymbolPath) -> Result<Self, ImportResolutionError>;
    const fn reference(&self) -> &SymbolPath;
    const fn unaliased_binding(&self) -> &ProjectSymbolPath;
}
```

`try_new` performs no string split:

1. Convert the typed source path with the existing `SymbolPath::try_from(path)` for resolution.
2. Inspect the already typed non-leaf `ProjectSymbolSegment` values with `try_as_module_segment()`.
3. When every non-leaf segment is a valid module segment, set `unaliased_binding` to an implicit one-segment path containing `path.last_segment()`; this preserves current ordinary import behavior.
4. When an implicit path contains an external-only qualifier such as `hero-pack`, retain all original typed segments as the unaliased binding; this matches the current opaque external-root lookup without parsing `reference.leaf()`.
5. An explicit-root path with an external-only qualifier continues to fail with the existing `InvalidPath(ModulePathError)` behavior.

This carrier is not a resolver. `targets_for_symbol_path` remains the only HIR resolution algorithm and receives `reference()` exactly as before.

## 6. Exact linker propagation rules

Every path installed in a scope is constructed from typed input:

- module declaration binding: implicit one segment converted from the module's validated `ModuleSegment`;
- source callable binding: implicit one segment constructed from the already validated declaration name;
- external direct binding: exact `ProjectDirectBinding::path().clone()`;
- unaliased path import: `LinkedProjectSymbolPath::unaliased_binding().clone()`;
- explicit `as` alias: implicit one segment converted from the typed `UseAlias::name()`;
- grouped selected name: implicit one segment cloned from typed `UseName::name()`;
- grouped explicit alias: implicit one segment converted from typed `UseAlias::name()`;
- glob import/re-export: exact `candidate.path.clone()`;
- fixed-point rebind: exact destination path received by `rebound`.

`import_bindings` directly returns `Vec<ScopeBinding>` rather than `Vec<(String, ScopeBinding)>`. The fixed-point loop passes each row to `insert_scope_binding` with no spelling parameter.

No source import visibility, ambiguity, visibility escalation, inaccessible import, unknown import, alias-budget, work-limit, or fixed-point stopping behavior changes.

## 7. Deterministic typed iterator

The current string iterator is directly replaced by exactly one public iterator:

```rust
impl ProjectSymbolTable {
    /// Every typed binding installed in a project module scope.
    ///
    /// Order is module, rendered private lookup key, then typed scope-row order.
    /// Same-target source sites remain coalesced in HIR.
    pub fn scope_bindings(
        &self,
    ) -> impl Iterator<
        Item = (
            &CanonicalModulePath,
            &ProjectSymbolPath,
            &ProjectSymbolTargetId,
        ),
    >;
}
```

Order is exact:

1. module order from the outer `BTreeMap`;
2. private lookup-key order from the inner `BTreeMap`;
3. `(path, target, visibility, owner)` order inside each vector.

There is no parallel `scope_binding_spellings`, old `&str` iterator, extension trait, secondary resolver view, or sema-side path discovery API.

## 8. Character producer contract

Character fact construction uses the existing validated `CharacterId::compact_segments()` iterator. It must not split or strip `CharacterId::as_str()`.

For owner `character.akane`, the producer constructs:

```rust
let compact_segments = owner
    .compact_segments()
    .map(|segment| ProjectSymbolSegment::try_new(segment.to_owned()))
    .collect::<Result<Vec<_>, ProjectSymbolPathError>>()?;

let qualified = ProjectSymbolPath::new(
    ModulePathRoot::ImplicitCrate,
    std::iter::once(ProjectSymbolSegment::try_new("character")?)
        .chain(compact_segments.iter().cloned()),
)?;

let compact = ProjectSymbolPath::new(
    ModulePathRoot::ImplicitCrate,
    compact_segments,
)?;
```

The producer then creates two direct bindings, both targeting the same seed. An explicitly authored alias such as `hero` is a third independent one-segment `ProjectSymbolPath` with `authored_alias = true`.

The canonical seed remains:

```rust
SymbolPath::try_new(
    ModulePathRoot::ImplicitCrate,
    Vec::new(),
    owner.as_str(),
)?
```

This formatting writes the already validated canonical `CharacterId` into the deliberately opaque canonical leaf. It is never read back to discover binding segments.

`ProjectRegistrationLoadError` adds direct typed propagation:

```rust
#[error(transparent)]
ProjectSymbolPath(#[from] ProjectSymbolPathError),
#[error(transparent)]
ProjectDirectBinding(#[from] ProjectDirectBindingError),
```

The existing `SymbolPath` variant remains for canonical external identity.

Every registrar audit that currently uses `strip_prefix("character.")`, a dotted spelling, or `ProjectSymbolBindingCollision::spelling()` is migrated to the same typed construction and `collision.path()` API. Formatting a typed path for a diagnostic is permitted; parsing it is not.

## 9. Adapter producer contract

### 9.1 Language-free typed identity

The base adapter manifest layer remains usable without the `sema` feature. It gains a producer-local typed symbol identity in a new `arcweft-adapter-context/src/symbol.rs` module:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterSymbolSegment(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterSymbolPath(Vec<AdapterSymbolSegment>);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdapterSymbolPathError {
    #[error("adapter symbol path must contain at least one typed segment")]
    Empty,
    #[error("adapter symbol segment must not be empty")]
    EmptySegment,
    #[error("invalid adapter symbol segment `{segment}`")]
    InvalidSegment { segment: String },
    #[error("adapter symbol path has invalid first segment `{segment}`")]
    InvalidImplicitRoot { segment: String },
}

impl AdapterSymbolSegment {
    pub fn try_new(value: impl Into<String>) -> Result<Self, AdapterSymbolPathError>;
    pub fn as_str(&self) -> &str;
}

impl AdapterSymbolPath {
    pub fn try_new(
        segments: impl IntoIterator<Item = AdapterSymbolSegment>,
    ) -> Result<Self, AdapterSymbolPathError>;
    pub fn segments(&self) -> &[AdapterSymbolSegment];
    pub fn last_segment(&self) -> &AdapterSymbolSegment;
}
```

Validation is intentionally identical to an implicit `ProjectSymbolPath`: each segment is non-empty and contains only letters, numbers, `_`, or `-`; the first segment begins with a letter or `_`. `Display` joins the already typed segments with `.`. There is no public `FromStr`, dotted-string constructor, serde implementation, or alternate untyped storage.

`lib.rs` adds `pub mod symbol;`. `manifest.rs` publicly re-exports `AdapterSymbolPath`, `AdapterSymbolPathError`, and `AdapterSymbolSegment` beside its current callable re-exports.

### 9.2 `AdapterSymbol` direct replacement

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterSymbol {
    path: AdapterSymbolPath,
    ty: AdapterTypeKind,
}

impl AdapterSymbol {
    pub fn new(path: AdapterSymbolPath, ty: AdapterTypeKind) -> Self;
    pub const fn path(&self) -> &AdapterSymbolPath;
    pub const fn ty(&self) -> &AdapterTypeKind;
}

impl AdapterManifest {
    #[must_use]
    pub fn with_symbol(mut self, symbol: AdapterSymbol) -> Self;
}
```

The old `AdapterSymbol { name: String, .. }`, `AdapterSymbol::new(name, ty)`, `name()`, and `AdapterManifest::with_symbol(name, ty)` are deleted in the same cut.

### 9.3 File codec boundary

Schema version 1 and its single `symbols[].name` source field remain unchanged. This is not a compatibility reader: the existing field is decoded directly into the final typed model, with no retained untyped manifest object.

`AdapterManifestCodecError` adds:

```rust
#[error(transparent)]
SymbolPath(#[from] AdapterSymbolPathError),
```

A private codec function is the only adapter dotted-field split:

```rust
fn symbol_path_from_file(
    path: &str,
) -> Result<AdapterSymbolPath, AdapterSymbolPathError>;
```

It rejects an empty path or empty dotted component, constructs every `AdapterSymbolSegment`, and then constructs `AdapterSymbolPath`. It is private to `codec.rs`; there is no public dotted constructor and no second file field or version.

`AdapterManifestFile::into_manifest` becomes:

```rust
manifest = manifest.with_symbol(AdapterSymbol::new(
    symbol_path_from_file(&symbol.name)?,
    parse_adapter_type_kind_label(&symbol.ty),
));
```

### 9.4 Registration-fact publication

`source_backed_registration_facts` sorts by `symbol.path()` and type evidence. For each symbol it:

1. renders the already typed path only to write deterministic generated source and the opaque canonical external identifier;
2. converts each `AdapterSymbolSegment` independently to `ProjectSymbolSegment`;
3. constructs one implicit `ProjectSymbolPath` from those typed segments;
4. passes that path to `ProjectDirectBinding::try_new`;
5. uses the rendered typed path for the existing `EnvironmentBindingId`, never as a segment source.

`AdapterRegistrationFactsError` adds:

```rust
#[error(transparent)]
ProjectSymbolPath(#[from] ProjectSymbolPathError),
#[error(transparent)]
ProjectDirectBinding(#[from] ProjectDirectBindingError),
```

The existing `SymbolPath` error remains for the opaque canonical identity.

`AdapterSymbolPath` is producer-local evidence, not a second project path authority. After conversion, `ProjectSymbolPath` is the sole binding identity consumed by HIR and sema.

## 10. Catalog publication contract

### 10.1 Existing catalog types remain unchanged

These implemented types are retained exactly:

```rust
pub struct ProjectCallablePath {
    package: CallablePackageId,
    module: CanonicalModulePath,
    path: CallablePath,
}

pub enum ProjectNameBinding {
    Callable(CallableDeclarationId),
    NonCallable {
        path: ProjectCallablePath,
        ty: TypeKind,
    },
}
```

No new binding map, record, resolver, or adapter-owned sema type is added.

### 10.2 Exact segment conversion

`RegisteredCallableCatalogBuilder::add_project_bindings` consumes the typed iterator. For each row it charges the existing binding work unit plus one work unit per typed segment, then constructs `CallablePath` directly from segments:

```rust
let segment_count = binding_path.segments().len();
self.work.charge(1)?;
self.work.charge(
    u64::try_from(segment_count)
        .map_err(|_| CallableCatalogBuildError::WorkOverflow)?,
)?;

let segments = binding_path.segments().iter().map(|segment| {
    CallableName::try_new(segment.as_str())
        .expect("ProjectSymbolSegment grammar is a strict subset of CallableName")
});

let callable_path = match CallablePath::try_new_with_limits(segments, &self.limits) {
    Ok(path) => path,
    Err(CallablePathError::TooManySegments { actual, limit }) => {
        return Err(CallableBuildLimitError::PathSegments { actual, limit }.into());
    }
    Err(CallablePathError::Empty) => {
        unreachable!("ProjectSymbolPath is non-empty by construction")
    }
};

let path = ProjectCallablePath::new(
    project.package().clone(),
    module.clone(),
    callable_path,
);
```

There is no `CallableName::try_new(complete_spelling)`, no `continue`, no split, and no fallback to `SymbolPath::leaf()`.

Target mapping remains exact:

```rust
let binding = match target {
    ProjectSymbolTargetId::Callable(declaration) => {
        ProjectNameBinding::Callable(declaration.clone())
    }
    ProjectSymbolTargetId::External(_) | ProjectSymbolTargetId::Module(_) => {
        ProjectNameBinding::NonCallable {
            path: path.clone(),
            ty: non_callable_type(target).ok_or_else(|| {
                CallableCatalogBuildError::MissingProjectBindingType {
                    target: target.clone(),
                }
            })?,
        }
    }
};
```

### 10.3 `TypeKind` ownership and dependency direction

HIR stores only `ProjectSymbolPath` plus `ProjectSymbolTargetId`. It does not store `TypeKind` and does not depend on sema or adapter-context.

The existing sema registration closure remains the sole type projection:

- character external: `TypeKind::Ref(EntityType::new(EntityKind::Character, None))`;
- environment external: the exact `TypeKind` from `request.base.environment_binding(id)`;
- module: current `TypeKind::Named("Module".to_owned())` representation;
- callable: no non-callable type.

Sema therefore depends only on HIR target kinds and its own registered owner/type facts. It does not import `AdapterManifest`, `AdapterSymbolPath`, adapter Rust metadata, or adapter type enums.

## 11. Duplicate, ambiguity, inaccessible, and invalid behavior

### 11.1 Valid duplicates

- Exact duplicate `ProjectDirectBinding` values continue to sort/deduplicate in `ExternalDeclarationSeed`.
- Exact scope rows with the same path, target, visibility, and owner coalesce sites.
- Multiple rows that produce the same `ProjectCallablePath` and identical `ProjectNameBinding` remain harmless exact duplicates in `finish_project`.

### 11.2 Collisions and ambiguity

- Same scope path with different targets remains visible as HIR ambiguity/collision evidence.
- A callable-catalog path receiving unequal `ProjectNameBinding` values fails with the existing `CallableCatalogBuildError::ProjectBindingCollision`.
- The deterministic typed iterator fixes which typed binding is reported as `first` and `second`; input fact order cannot change it.
- No collision is resolved by source order, display spelling, canonical external identity, type compatibility, or environment fallback.

### 11.3 Imports

- Unknown imports retain current omission behavior.
- Inaccessible imports retain the existing `InaccessibleImport` diagnostic and are not installed.
- Visibility escalation and ambiguous imports retain their existing typed errors.
- Explicit aliases and re-exports do not inherit or overwrite canonical external identity.
- Alias limits, import limits, diagnostic limits, work limits, and fixed-point termination are unchanged.

### 11.4 Invalid paths

- Empty/control/separator components fail in `ProjectSymbolSegment::try_new` or `AdapterSymbolSegment::try_new`.
- Empty paths and invalid implicit roots fail in the owning path constructor.
- A direct binding with a non-implicit root fails in `ProjectDirectBinding::try_new` before linking.
- A valid `-` in an external segment remains a typed segment and is never mistaken for a `ModuleSegment`.
- No valid current character or adapter registration is rejected merely because its binding is qualified.

## 12. Resolver and shadowing behavior

The shared resolver remains unchanged. It continues to consult project bindings before environment callables. A `ProjectNameBinding::NonCallable` is a terminal project shadow and prevents same-spelled environment fallback.

Once every typed path is published:

- `character.akane` blocks an environment callable with path `['character', 'akane']`;
- authored alias `hero` blocks an environment callable with path `['hero']`;
- compact `akane` remains independent and blocks only `['akane']`;
- each path still resolves to the same external declaration in `ProjectSymbolTable`.

No second project-symbol resolver or catalog-side reinterpretation is introduced.

## 13. Accepted-world atomicity

The current transaction order is preserved:

1. link project symbols;
2. validate external owners and character facts;
3. build all project and environment callable publications;
4. finish the complete callable catalog;
5. construct the candidate registered environment and definition index;
6. return the complete `RegisteredSemanticWorld`;
7. let the existing caller publish the accepted pointer only after success.

A malformed typed path fails at producer construction before a candidate world exists. A binding collision fails catalog construction before `RegisteredSemanticWorld` is returned. Both outcomes leave the previous accepted world pointer, generation, symbols, external owners, character facts, callable catalog, and caches unchanged.

No world-only publication, catalog-only publication, partial mutation, source gate, or rollback shim is added.

## 14. Public visibility summary

Public syntax API, unchanged:

- `ProjectSymbolSegment` and its validating constructor/accessor;
- `ProjectSymbolPath` and its validating constructor/accessors;
- `ProjectSymbolPathError`.

Public HIR API after the cut:

- `ProjectDirectBinding` with typed constructor/accessors;
- `ProjectDirectBindingError`;
- `ProjectSymbolBindingCollision::path()`;
- the read-only typed `ProjectSymbolTable::scope_bindings()` iterator.

Private to HIR:

- `ScopeBinding`;
- `LinkedProjectSymbolPath`;
- rendered lookup keys;
- insertion, rebind, sort, and coalescing machinery.

Public adapter-context API after the cut:

- `AdapterSymbolSegment`, `AdapterSymbolPath`, and `AdapterSymbolPathError`;
- typed `AdapterSymbol::new`/`path`/`ty`;
- `AdapterManifest::with_symbol(AdapterSymbol)`.

Private to adapter codec/publication:

- dotted source-field parsing;
- adapter-to-project segment conversion;
- generated source rendering.

Private to sema:

- `RegisteredCallableCatalogBuilder` mutation;
- typed segment conversion to `CallableName`;
- target-to-`TypeKind` projection.

## 15. Final invariants

Implementation is conforming only when all are true:

1. `ProjectSymbolPath` is the sole project-binding path authority.
2. Every scope row retains its exact ordered segments.
3. Canonical external identity and source-visible aliases remain distinct.
4. Character and adapter producers construct paths before HIR linking.
5. Sema and the callable catalog never split a string to obtain binding identity.
6. Every callable and non-callable scope binding is published or causes a typed failure; none is silently omitted.
7. Project non-callable qualified and compact paths terminate environment fallback.
8. `-` remains valid in external segments without becoming a module identifier.
9. Import visibility, ambiguity, limits, and fixed-point semantics are unchanged.
10. Catalog and accepted-world failure remain atomic.
11. The string-only direct-binding constructor, string scope iterator, invalid-name skip, and adapter symbol string model are deleted in one coherent cut.
12. No compatibility shim, dual reader, deprecated wrapper, extension trait, source gate, CSS/Takumi path, display parser, or second resolver exists.
13. Existing callable identity, schema, catalog, resolver, accepted source identity, call-range, and request-lifecycle substrate is not redesigned.
