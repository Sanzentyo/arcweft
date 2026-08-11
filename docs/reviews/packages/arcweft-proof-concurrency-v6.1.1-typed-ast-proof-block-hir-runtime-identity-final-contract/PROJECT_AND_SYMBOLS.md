# Module-preserving project and unified symbol boundary

## 1. Required end state

A project is an ordered set of immutable `Arc<HirModule>` snapshots. It never concatenates module bodies, rebases IDs, clones arenas, or invents a synthetic linked module. Every item, scope, local, expression, statement, type, pattern, and capture keeps the `HirModuleId` allocated by its `HirDatabase`.

The existing `ProjectSymbolTable` is the only declaration/import authority. Predicate and proof registration extends its repository-owned callable enum and table implementation.

## 2. Exact project types

```rust
#[derive(Clone)]
pub struct HirProjectModule {
    package: CallablePackageId,
    path: CanonicalModulePath,
    source_document: SourceDocumentId,
    module: Arc<HirModule>,
}

impl HirProjectModule {
    pub fn try_new(
        package: CallablePackageId,
        path: CanonicalModulePath,
        source_document: SourceDocumentId,
        module: Arc<HirModule>,
    ) -> Result<Self, HirProjectError>;

    pub fn package(&self) -> &CallablePackageId;
    pub fn path(&self) -> &CanonicalModulePath;
    pub fn source_document(&self) -> &SourceDocumentId;
    pub fn module(&self) -> &Arc<HirModule>;
}

#[derive(Clone)]
pub struct HirProject {
    package: CallablePackageId,
    modules: BTreeMap<CanonicalModulePath, HirProjectModule>,
    source_index: BTreeMap<SourceDocumentId, CanonicalModulePath>,
}

impl HirProject {
    pub fn try_new(
        package: CallablePackageId,
        modules: impl IntoIterator<Item = HirProjectModule>,
    ) -> Result<Self, HirProjectError>;

    pub fn package(&self) -> &CallablePackageId;
    pub fn module(&self, path: &CanonicalModulePath) -> Option<&HirProjectModule>;
    pub fn view(&self) -> HirProjectView<'_>;
    pub fn executable_view(&self) -> Result<HirProjectView<'_>, HirProjectError>;
}

#[derive(Clone, Copy)]
pub struct HirProjectView<'a> {
    package: &'a CallablePackageId,
    modules: &'a BTreeMap<CanonicalModulePath, HirProjectModule>,
}

impl<'a> HirProjectView<'a> {
    pub fn package(self) -> &'a CallablePackageId;
    pub fn modules(
        self,
    ) -> impl ExactSizeIterator<Item = (&'a CanonicalModulePath, &'a Arc<HirModule>)>;
    pub fn module(
        self,
        path: &CanonicalModulePath,
    ) -> Option<&'a Arc<HirModule>>;
    pub fn items(self) -> impl Iterator<Item = HirProjectItemRef<'a>>;
}

pub struct HirProjectItemRef<'a> {
    module_path: &'a CanonicalModulePath,
    module: &'a HirModule,
    id: ItemId,
    item: &'a HirItem,
}

impl<'a> HirProjectItemRef<'a> {
    pub fn module_path(&self) -> &'a CanonicalModulePath;
    pub fn module(&self) -> &'a HirModule;
    pub fn id(&self) -> ItemId;
    pub fn item(&self) -> &'a HirItem;
}
```

Project order is canonical module-path order. Within a module, explicit source-order item arrays determine item order; arena slot order is not a project ordering authority.

## 3. Checked construction

`HirProjectModule::try_new` verifies all of the following without mutation or panic:

- supplied package equals `module.key().package()`;
- supplied path equals `module.key().path()`;
- supplied source document equals `module.key().document()`;
- the module snapshot's exact `SourceDocumentIdentity::id()` matches the source document;
- `module.snapshot_id().module()` belongs to the module's owning HIR database and is internally consistent.

`HirProject::try_new` verifies:

- every module has the project package;
- canonical module paths are unique;
- source document IDs are unique within the project;
- no module path/source pair conflicts;
- the canonical crate-root module is present; and
- no module object is silently rebound or rewritten.

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirProjectError {
    PackageMismatch {
        expected: CallablePackageId,
        actual: CallablePackageId,
    },
    ModulePathMismatch {
        expected: CanonicalModulePath,
        actual: CanonicalModulePath,
    },
    SourceDocumentMismatch {
        expected: SourceDocumentId,
        actual: SourceDocumentId,
    },
    DuplicateModulePath { path: CanonicalModulePath },
    DuplicateSourceDocument { document: SourceDocumentId },
    MissingRootModule,
    RecoveredModule { path: CanonicalModulePath, snapshot: HirSnapshotId },
    InvalidModuleSnapshot { snapshot: HirSnapshotId },
}
```

`view()` includes clean and recovered modules for tooling. `executable_view()` returns the first `RecoveredModule` in canonical path order and never exposes a partially executable project.

## 4. No flattening APIs

The following are deleted, not deprecated:

- `HirProject::linked_module`;
- `HirModule::append_module_body`;
- compiler `CompiledProject::linked_hir` field and accessor;
- any helper that rebases IDs or package/module ownership;
- any semantic, verifier, style, runtime-plan, CLI, LSP, tooling, cache, or test entrypoint that accepts the flattened module.

There is no compatibility wrapper that internally links modules.

## 5. Per-module aggregation

### 5.1 Exported parts

```rust
pub struct ProjectExportedPartRef<'a> {
    module_path: &'a CanonicalModulePath,
    item: ItemId,
    part: &'a HirExportedPart,
}

impl<'a> ProjectExportedPartRef<'a> {
    pub fn module_path(&self) -> &'a CanonicalModulePath;
    pub fn item(&self) -> ItemId;
    pub fn part(&self) -> &'a HirExportedPart;
}

pub fn exported_parts(
    project: HirProjectView<'_>,
) -> impl Iterator<Item = ProjectExportedPartRef<'_>>;
```

The iterator uses canonical module order then each module's authored exported-part order. IDs are never rewritten.

### 5.2 Style records

```rust
pub struct ProjectStyleRef<'a> {
    module_path: &'a CanonicalModulePath,
    item: ItemId,
    style: &'a HirStyleItem,
}

impl<'a> ProjectStyleRef<'a> {
    pub fn module_path(&self) -> &'a CanonicalModulePath;
    pub fn item(&self) -> ItemId;
    pub fn style(&self) -> &'a HirStyleItem;
}

pub fn styles(
    project: HirProjectView<'_>,
) -> impl Iterator<Item = ProjectStyleRef<'_>>;
```

Style compilation, duplicate checks, and exported-part/style joins use `(module_path, ItemId)` and project symbols. They do not depend on linked item indices.

### 5.3 Sema and runtime-plan

Project entrypoints become:

```rust
pub fn resolve_project(
    project: HirProjectView<'_>,
    symbols: &ProjectSymbolTable,
) -> ProjectResolution;

pub fn type_check_project(
    project: HirProjectView<'_>,
    symbols: &ProjectSymbolTable,
    resolution: &ProjectResolution,
) -> ProjectTypeCheck;

pub fn lower_runtime_project(
    project: HirProjectView<'_>,
    symbols: &ProjectSymbolTable,
    sema: &ProjectSemanticResult,
    profile: RuntimeBuildProfile,
) -> Result<RuntimeProjectPlan, RuntimePlanError>;
```

Every result keeps module-qualified IDs.

## 6. Unified callable registration

The repository-owned enum is the one callable owner vocabulary:

```rust
pub enum CallableDeclarationOwner {
    Function,
    Predicate,
    Proof,
}
```

Its inherent implementation is extended:

```rust
impl CallableDeclarationOwner {
    pub const fn is_runtime_callable(self) -> bool;
    pub const fn is_logical_callable(self) -> bool;
    pub const fn permits_proof_statement_call(self) -> bool;
}
```

Exact behavior:

| Owner | runtime callable | logical callable | proof-statement target |
|---|---:|---:|---:|
| Function | yes under existing readiness | no unless current purity rules classify call expression | no |
| Predicate | no standalone runtime entry | yes, returns Bool | no |
| Proof | no | no value expression | yes |

No extension trait or separate `ProofCallableKind` is introduced.

## 7. Callable symbol record

```rust
pub struct CallableSymbol {
    declaration: CallableDeclarationId,
    visibility: Option<Visibility>,
    signature: HirCallableSignature,
    source: CallableSymbolSource,
    executable: bool,
}

pub struct CallableSymbolSource {
    snapshot: HirSnapshotId,
    item: ItemId,
    declaration_span: SourceSpan,
    name_span: SourceSpan,
}

pub struct HirCallableSignature {
    generic_parameters: Box<[HirGenericParameter]>,
    parameters: Box<[HirCallableParameter]>,
    return_type: TypeId,
    where_predicates: Box<[HirWherePredicate]>,
    requires: Box<[ExprId]>,
    ensures: Box<[ExprId]>,
}

impl CallableSymbol {
    pub fn declaration(&self) -> &CallableDeclarationId;
    pub fn owner(&self) -> CallableDeclarationOwner; // delegates to declaration().owner()
    pub fn visibility(&self) -> Option<Visibility>;
    pub fn signature(&self) -> &HirCallableSignature;
    pub fn source(&self) -> &CallableSymbolSource;
    pub fn is_executable(&self) -> bool;
}

impl CallableSymbolSource {
    pub fn snapshot(&self) -> HirSnapshotId;
    pub fn item(&self) -> ItemId;
    pub fn declaration_span(&self) -> &SourceSpan;
    pub fn name_span(&self) -> &SourceSpan;
}

impl HirCallableSignature {
    pub fn generic_parameters(&self) -> &[HirGenericParameter];
    pub fn parameters(&self) -> &[HirCallableParameter];
    pub fn return_type(&self) -> TypeId;
    pub fn where_predicates(&self) -> &[HirWherePredicate];
    pub fn requires(&self) -> &[ExprId];
    pub fn ensures(&self) -> &[ExprId];
}
```

`CallableSymbol::owner()` delegates to `CallableDeclarationId::owner()`; no second stored owner field can diverge. Source provenance uses the exact HIR snapshot/item and revision-bound spans. It does not recreate a source-name/span authority.

`None` in `CallableSymbol::visibility` is module-private; `Some(Visibility::Public | Crate | Super)` preserves the existing syntax authority. No `Private` compatibility variant is added.

Recovered declarations may be registered for tooling with `executable = false`; executable semantic entrypoints reject them and do not cache them.

## 8. `ProjectSymbolTable` API

```rust
impl ProjectSymbolTable {
    pub fn link(
        project: HirProjectView<'_>,
        externals: &ProjectExternalDeclarations,
    ) -> Result<ProjectSymbolLinkOutput, ProjectSymbolLinkReport>;

    pub fn callable(
        &self,
        id: &CallableDeclarationId,
    ) -> Option<&CallableSymbol>;

    pub fn resolve_callable(
        &self,
        from: &CanonicalModulePath,
        path: &SymbolPath,
        source: &SourceSpan,
    ) -> Result<&CallableSymbol, ProjectSymbolResolutionError>;

    pub fn proof_artifact(
        &self,
        project: HirProjectView<'_>,
        id: &CallableDeclarationId,
    ) -> Result<ProofArtifactId, ProofArtifactIdentityError>;

    pub fn revision(&self) -> &ProjectSymbolRevision;
}

impl ProjectSymbolLinkOutput {
    pub fn table(&self) -> &ProjectSymbolTable;
    pub fn into_table(self) -> ProjectSymbolTable;
}
```

`link` extends the existing unified link transaction and performs one canonical pass:

1. register module declarations and explicit source provenance;
2. register every function, predicate, proof, and Character declaration into the existing table;
3. reject same-module ordinary-name duplicates across callable kinds;
4. register explicit imports, grouped imports, globs, and aliases;
5. apply visibility from the declaration's typed visibility;
6. diagnose alias/import collisions and inaccessible targets;
7. mark symbols from recovered modules/declarations non-executable;
8. compute one revision/cache key from the ordered project snapshot set and external symbol revision.

The table is immutable after build. There is no second phase that overlays proof symbols.

## 9. Names and collisions

`CallableDeclarationId` remains package + canonical module + `CallableDeclarationOwner` + ordinary name. The owner field distinguishes declaration identity after the duplicate check; it does not create overload namespaces.

Within one module, these collide:

- function `f` and predicate `f`;
- function `f` and proof `f`;
- predicate `f` and proof `f`;
- two declarations of the same owner/name regardless of signature.

The first declaration in source order remains the lookup target for recovered tooling. Every later declaration receives the same duplicate diagnostic and is non-executable. Project-wide qualification still distinguishes modules.

Imports and aliases bind one ordinary name. A local declaration and an import alias with the same binding name follow the existing collision policy; predicate/proof kinds do not receive special precedence.

## 10. Revision and cache invalidation

```rust
pub struct ProjectSymbolRevision {
    project_package: CallablePackageId,
    module_snapshots: Arc<[(CanonicalModulePath, HirSnapshotId)]>,
    external_revision: ExternalSymbolRevision,
    digest: [u8; 32],
}
```

This type is session-only and non-Serde. The digest is a cache discriminator, not a persisted declaration identity.

A successful HIR commit that changes any callable declaration/signature/visibility/import or executable status invalidates the project symbol revision. Trivia-only changes that preserve all relevant HIR IDs and values may retain semantic table cache entries while source-label caches update by snapshot ID. Failed HIR transactions publish no invalidation.

## 11. Session-only proof artifact identity

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProofArtifactId {
    declaration: CallableDeclarationId,
    snapshot: HirSnapshotId,
    item: ItemId,
}

impl ProofArtifactId {
    pub fn declaration(&self) -> &CallableDeclarationId;
    pub fn snapshot(&self) -> HirSnapshotId;
    pub fn item(&self) -> ItemId;
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProofArtifactIdentityError {
    #[error("callable is not registered in this project symbol table")]
    UnknownDeclaration { declaration: CallableDeclarationId },
    #[error("callable is not a proof declaration")]
    NotProof {
        declaration: CallableDeclarationId,
        actual: CallableDeclarationOwner,
    },
    #[error("proof HIR snapshot is not present in the supplied project view")]
    SnapshotUnavailable { snapshot: HirSnapshotId },
    #[error("registered proof source does not resolve to a proof item")]
    ItemMismatch { snapshot: HirSnapshotId, item: ItemId },
    #[error("registered proof source does not match the table declaration")]
    RegistrationMismatch { declaration: CallableDeclarationId },
}
```

`ProjectSymbolTable::proof_artifact` is the sole constructor and succeeds only when:

- `symbol.owner() == CallableDeclarationOwner::Proof`;
- `symbol.source().item` resolves to `HirItemKind::Proof` in the exact snapshot;
- the symbol is the registered declaration for that ordinary name.

The type has private fields, no raw/text constructor, no `FromStr`, no display codec that can be parsed, and no Serde. It is not authored and not persisted. A new HIR snapshot produces a distinct artifact identity even when the declaration's canonical name is unchanged.

## 12. Compiler product shape

```rust
pub struct CompiledProject {
    hir: HirProject,
    symbols: ProjectSymbolTable,
    resolution: ProjectResolution,
    semantics: ProjectSemanticResult,
    runtime: RuntimeProjectPlan,
    assertion_inventory: RuntimeAssertionInventory,
}
```

There is no `linked_hir`. Accessors expose borrowed project views and module-qualified semantic results. Caches key module facts by `HirSnapshotId` and project facts by `ProjectSymbolRevision`/ordered snapshot set.

## 13. Caller migration

The workspace migration changes all linked-module consumers in one compiling cut:

- HIR project/model tests;
- compiler project assembly, readiness, type checking, style lowering, exported parts, line-task and runtime-plan calls;
- sema checker module/expr/stmt entrypoints and resolution caches;
- verifier contract/proof item iteration;
- runtime-plan expression/flow/project lowering;
- CLI check/run/profile/debug project views;
- LSP diagnostics, hover, definition, references, rename, semantic tokens, document symbols;
- Agent and tooling project summaries;
- formatter/project fixtures;
- cache keys and compile-fail tests.

The linked/append APIs are deleted in the same change that migrates the final caller. No public period exists where both project models are supported.

## 14. Direct tests

Required project/symbol tests are specified in `TEST_MATRIX.md`. They assert:

- original module-qualified IDs survive ordered project iteration;
- mismatched package/path/source construction returns typed errors and never mutates a module;
- one table registers functions, predicates, proofs, and Character declarations;
- duplicate names, visibility, imports, aliases, and invalidation are deterministic;
- recovered modules are visible to tooling but rejected by `executable_view`;
- linked/append APIs and authored proof IDs fail to compile.
