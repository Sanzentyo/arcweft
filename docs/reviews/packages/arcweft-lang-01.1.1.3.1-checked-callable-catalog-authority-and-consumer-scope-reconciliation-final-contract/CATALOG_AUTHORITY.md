# Callable catalog authority

## 1. Authority statement

Arcweft has exactly two layers with non-overlapping responsibilities:

1. **accepted record authority** — `RegisteredCallableCatalog` and exact `Arc<CallableRecord>` values;
2. **checked-context authority** — one immutable `Arc<CheckedCallableCatalog>` for one successful check generation.

The accepted record authority is retained. It is not wrapped by a copied public record model and is not atomically replaced by a new catalog. The checked layer stores exact accepted record Arcs and adds only facts that require checking.

This division is normative:

| Fact | Sole owner |
|---|---|
| structural candidate/declaration ID | `CallableRecord::id` and its owning structural ID type |
| lookup key and precedence rank | `CallableRecord` / accepted catalog indexes |
| signature schema | `CallableRecord::schema` |
| exact source | `CallableRecord::source` |
| documentation | `CallableRecord::documentation` |
| declaration access classification | `CallableRecord::access` |
| provider/Rust/publication provenance | `CallableRecord` |
| fixed environment/standard row | `CallableRecord::schema().effects().Fixed` |
| checked ID/generation | `CheckedCallableCatalog` |
| body effect contract/inferred row | `CheckedCallableFacts::effects` |
| bodyless requirement contract | `CheckedCallableFacts::effects` |
| execution role | `CheckedCallableFacts::execution` |
| conformance/substitution | `CheckedCallableCatalog::conformances` |
| closure row | `CheckedCallableCatalog::closure_rows` |
| source/candidate lookup index | derived ID-only indices in `CheckedCallableCatalog` |
| interface digest | private derived value frozen with checked facts |

No row or metadata fact has two mutable/authoritative homes.

## 2. Accepted catalog changes

### 2.1 `CallableRecord`

The final record is:

```rust
pub struct CallableRecord {
    id: CallableCandidateId,
    key: CallableLookupKey,
    authority: CallableAuthorityRank,
    provider: CallableProviderId,
    access: CallableAccess,
    schema: Arc<CallableSignatureSchema>,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    rust: Option<RustCallableProvenance>,
    publication_digest: Option<EnvironmentCallablePublicationDigest>,
    declaration_order: EnvironmentDeclarationOrdinal,
}
```

Visibility and constructors:

- type and read-only accessors stay `pub` because sema/compiler/LSP consumers need them;
- fields stay private;
- `try_new` changes from public to `pub(crate)`;
- no `Default`, struct update, setter, mutation, or field-specific replacement API exists;
- only `RegisteredCallableCatalogBuilder`, accepted environment publication projection, and the private detached checked builder create records;
- every builder creates the final `Arc<CallableRecord>` once.

`CallableRecord` gains inherent accessors for `access`, `provider`, `publication_digest`, and a complete immutable metadata view. Callers do not pattern-match private fields through helper traits.

### 2.2 Candidate identity

The current `CallableCandidateId` enum is extended in place:

```rust
pub enum CallableCandidateId {
    Project(CallableDeclarationKey),
    Detached(DetachedCallableDeclarationId),
    Environment(EnvironmentCallableId),
    Standard(StandardCallableDeclarationId),
    /* retained existing non-source candidate families */
}
```

`TraitMethod(TraitCallableId)` is deleted. The original enum's inherent `owner`, ordering/canonical encoding, display-label, and family predicates receive exhaustive arms. No converter reconstructs a candidate from trait/method strings.

### 2.3 Project registration

`RegisteredCallableCatalogBuilder::add_project` consumes the HIR project's one ordered callable-source inventory. That inventory contains:

- ordinary functions;
- views already admitted as callables;
- extern capability functions;
- predicates/proofs already in the accepted callable family;
- trait method requirements;
- trait implementation methods; and
- inherent methods.

Each source entry already carries `CallableDeclarationKey`, exact signature/source spans, access classification inputs, and structural owner. Registration performs nominal signature resolution once and constructs one record.

Final project storage:

```rust
pub struct ProjectCallableCatalog {
    modules: BTreeMap<CanonicalModulePath, RegisteredProjectModuleCallables>,
    by_declaration: BTreeMap<CallableDeclarationKey, Arc<CallableRecord>>,
    bindings: BTreeMap<ProjectCallablePath, ProjectNameBinding>,
}
```

`by_declaration` changes from `CallableDeclarationId` to `CallableDeclarationKey`. Free/source-visible callables enter `bindings`. Method records do not enter module value bindings:

- trait requirements are found through typed trait resolution;
- trait implementations are found only through conformance/witness selection;
- inherent methods are found through typed receiver/method indexes.

Those indexes contain IDs/Arcs to the same records; they do not own copied schemas.

### 2.4 Environment and standard registration

`EnvironmentCallablePublicationRecord` continues to be a validated pre-freeze input. During `finish_environment`, its schema/documentation/source/Rust provenance are moved into one `Arc<CallableRecord>` with `CallableCandidateId::Environment` or `Standard`. The final accepted catalog keeps that Arc in all lookup indexes.

Programmatic standard trait requirements/implementations are appended by one `StandardTraitCatalogBuilder`. The builder assigns `StandardCallableDeclarationId` in one deterministic order and increments `STANDARD_TRAIT_CATALOG_VERSION` for any semantic insertion, deletion, reorder, signature/effect change, or witness remap.

After publication, `StandardTraitCatalog` retains exactly structural requirement/implementation IDs and witness relations. It owns no signature or row; all schema/source/access/fixed-row reads use the accepted record.

### 2.5 Access and bindings

`CallableAccess` is record metadata. `ProjectDirectBinding.visibility` remains an edge property for a specific source alias/reexport. These are not interchangeable:

- declaration access says what the declaration permits;
- binding visibility says where one binding edge is visible.

The accepted resolver checks both. It does not copy declaration access into checked facts or tooling symbols.

### 2.6 Catalog digest

`RegisteredCallableCatalogDigest` remains the digest of the final immutable accepted catalog and is expanded automatically by the record/candidate/access changes. Its canonical encoder includes:

- accepted nominal world stamp;
- project module/source inventory;
- record candidate ID, key, rank, provider, access, signature digest, docs, source, Rust provenance, publication digest, declaration order;
- binding targets;
- environment indexes/publications; and
- standard catalog version/publication.

The checked layer binds project/environment identities to this exact digest. There is no separate checked signature digest synchronized with it.

## 3. Checked builder and immutable catalog

### 3.1 Private builder shape

The implementation uses a private state machine in `arcweft-lang-sema::checker` or a responsibility sibling:

```rust
pub(crate) struct CheckedCallableCatalogBuilder {
    generation: CheckedCallableCatalogGeneration,
    registered: Option<Arc<RegisteredCallableCatalog>>,
    state: CheckedCatalogBuildState,
    pending: BTreeMap<CheckedCallableId, PendingCheckedCallable>,
    checked_by_candidate: BTreeMap<CallableCandidateId, CheckedCallableId>,
    conformances: BTreeMap<TraitMethodConformanceId, TraitMethodConformance>,
    closure_rows: BTreeMap<CheckedClosureId, EffectRow>,
    source_index: BTreeMap<CheckedCallableSourceKey, CheckedCallableId>,
    journal: Vec<CheckedCatalogMutation>,
    effect_state: EffectInferenceState,
}

pub(crate) enum CheckedCatalogBuildState {
    Collecting,
    Inferring,
    Validating,
    Finished,
    Poisoned,
}
```

These types are crate-private. There is no public pending lookup, snapshot, or partial catalog reader.

### 3.2 Pending shell

```rust
pub(crate) struct PendingCheckedCallable {
    id: CheckedCallableId,
    record: Arc<CallableRecord>,
    execution: CheckedCallableExecution,
    contract: PendingCallableEffectContract,
    inferred: Option<EffectRow>,
    completion: PendingCallableCompletion,
}
```

A shell is inserted only after:

- checked ID/context validation;
- candidate/record identity equality;
- exact catalog generation validation;
- source membership/range validation;
- access classification validation; and
- `Arc::ptr_eq` validation for registered records.

The shell does not copy signature/source/docs/access/provider/publication. Its contract field is the checked body/requirement contract, not a record metadata duplicate.

### 3.3 Registered pointer validation

For a registered builder:

1. `RegisteredSemanticWorld` yields exact `Arc<RegisteredCallableCatalog>` A.
2. The builder retains `Arc::clone(&A)`.
3. For each candidate, `A.record(candidate)` returns `&Arc<CallableRecord>` R.
4. The shell retains `Arc::clone(R)`.
5. Before freeze, the builder re-queries A by candidate and requires `Arc::ptr_eq` with the shell record.
6. `CheckedCallableFacts` receives the shell Arc unchanged.

A record with equal value but another allocation is rejected as `CheckedCallableBuildError::ForeignRecordArc`. Value equality is not accepted as proof of authority.

### 3.4 Detached record handling

Detached analysis has no accepted project catalog. It therefore uses this closed rule:

- the private checked builder constructs one `Arc<CallableRecord>` for each detached declaration after exact source binding;
- it inserts that Arc directly into the pending shell;
- no `DetachedCallableCatalog`, public map, resolver view, or adapter DTO is created;
- detached candidate/name resolution within the check uses ID-only indices owned by the builder/final checked catalog;
- the record Arc dies with the checked report unless a successful consumer retains the final checked catalog.

Standard records needed by detached checking are created from the exact installed standard version in the same private transaction and use `CallableCandidateId::Standard`. They are fixed-row records and are not copied into `CheckedCallableEffects`.

### 3.5 Fixed rows

`CheckedCallableEffects::RecordFixed` is valid only when:

```rust
matches!(record.schema().effects(), CallableEffectSchema::Fixed(_))
```

It has no payload. `CheckedCallableFacts::exposed_row()` returns the fixed row by reference from the record. This closes the parent package's external/standard row duplication.

Source callables must use `CallableEffectSchema::Project` or `Detached`, and may not use `RecordFixed`. Their contracts/inferred rows are in checked facts only.

### 3.6 Freeze

`finish(self)` consumes the builder. It requires:

- build state `Validating`;
- every pending shell completed exactly once;
- all body rows fully resolved;
- no unknown tail accepted as an open contract;
- all body actual rows validated against authored contracts;
- all trait implementation rows validated against substituted requirement contracts;
- every conformance references existing exact checked IDs;
- every closure row owner exists;
- candidate/source indices are one-to-one where required;
- exact registered Arc/pointer checks pass;
- derived interface digest matches record/final exposed row; and
- deterministic map ordering/work limits pass.

It returns:

```rust
Result<Arc<CheckedCallableCatalog>, CheckedCallableBuildError>
```

No `CheckedCallableCatalog::new` is public. A failed finish consumes/poisons the builder and returns no partial catalog.

## 4. Effect and conformance authority

### 4.1 Effect facts

The accepted effect semantics remain:

- body actual row = final fixed-point result;
- body exposed row = authored bounded row when present, otherwise actual;
- bodyless requirement exposed row = bounded requirement contract;
- omitted bodyless requirement = closed empty row with method-name anchor;
- fixed environment/standard exposed row = accepted record fixed row;
- calls/method values propagate exposed row;
- body validation/conformance uses actual row;
- static witness propagation uses substituted requirement exposed row.

No `TraitMethodRequirement`, `TraitMethodImpl`, resolver candidate, call fact, method value, project symbol, Agent record, LSP record, compiler persistent record, or runtime value owns an authoritative row.

### 4.2 Conformance

```rust
pub struct TraitMethodConformanceId {
    implementation: CheckedCallableId,
    requirement: CheckedCallableId,
}

pub struct TraitMethodConformance {
    id: TraitMethodConformanceId,
    witness: TraitWitnessId,
    substitution: TraitMethodSubstitution,
}
```

The checked catalog owns the final map. A conformance stores no signature, source, access, or row. It is constructed only after exact record signature compatibility and `EffectRow::check_subset` succeed. One implementation may have multiple conformance IDs for inherited requirements; each retains the original requirement ID.

## 5. Arc publication graph

The exact Arc graph is:

```text
RegisteredSemanticWorld
  └─ Arc<RegisteredTypeCheckEnv>
       └─ Arc<RegisteredCallableCatalog> A
            ├─ Arc<CallableRecord> R1
            ├─ Arc<CallableRecord> R2
            └─ ...

CheckedCallableCatalog C
  ├─ registered = Some(Arc::clone(A))
  └─ CheckedCallableFacts.record = Arc::clone(Rn)

TypeCheckReport
  └─ Arc<CheckedCallableCatalog> C

ProjectSemanticIndex
  └─ Arc::clone(C)

CompiledProject / AcceptedProfileEnvironment
  └─ report/index retain the same C and validate A
```

Required pointer assertions:

- checked catalog registered Arc vs registered world catalog Arc: `Arc::ptr_eq`;
- checked fact record Arc vs accepted catalog record Arc: `Arc::ptr_eq`;
- typecheck report checked Arc vs project index checked Arc: `Arc::ptr_eq`;
- LSP accepted compiled report Arc vs accepted project semantic Arc: `Arc::ptr_eq`.

Compiler, Agent, and persistent builders borrow C. They do not clone record fields into another in-memory authority.

## 6. Failure and rollback

### 6.1 Checkpoint

A checkpoint captures journal length, effect-variable supply, current callable stack length, and pending completion epoch. It does not copy accepted records.

### 6.2 Journal mutations

```rust
pub(crate) enum CheckedCatalogMutation {
    PendingInserted(CheckedCallableId),
    CandidateIndexed(CallableCandidateId),
    SourceIndexed(CheckedCallableSourceKey),
    EffectVariableAllocated(EffectVar),
    EffectEdgeInserted(CheckedEffectEdgeId),
    ClosureRowInserted(CheckedClosureId),
    ConformanceInserted(TraitMethodConformanceId),
    InferredRowAssigned(CheckedCallableId),
    InterfaceDigestStaged(CheckedCallableId),
}
```

The journal uses these exact mutation variants. Adjacent mutations are not coalesced into an untyped or lossy record; rollback processes them in reverse insertion order.

### 6.3 Rollback outcome

Rollback removes all post-checkpoint checked state, restores counters/stacks, and leaves the immutable accepted catalog unchanged. An ID produced after the checkpoint cannot be observed by any public API because the final catalog has not been published.

Any error after accepted catalog freeze but before checked freeze returns no `TypeCheckReport`; therefore no `ProjectSemanticIndex`, compiler product, LSP accepted generation, Agent payload, or persistent object is built.

## 7. Visibility and dependency direction

- HIR structural identities live in `arcweft-lang-hir`.
- Accepted record/catalog and checked identity/facts live in `arcweft-lang-sema`.
- `TypeCheckReport` and project semantic index retain sema-owned Arcs.
- Compiler consumes sema IDs/catalog and produces one-way runtime/persistent projections.
- LSP consumes accepted compiler/sema snapshots.
- Agent protocol receives projections only and does not become a semantic owner.
- Runtime/core never depend on HIR/sema records or source spans.

No dependency reversal is authorized.
