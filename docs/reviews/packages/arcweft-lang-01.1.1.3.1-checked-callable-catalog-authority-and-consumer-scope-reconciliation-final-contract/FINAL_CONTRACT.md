# Final normative correction

## 1. Scope and precedence

This document is the normative Lang-01.1.1.3.1 correction. It closes the ownership and consumer-scope gap in the returned Lang-01.1.1.3 package.

The following parent decisions remain unchanged and are incorporated into this package:

1. parent E017 is `SUPERSEDED_FOR_LANG_01_1_1`; dynamic trait objects remain future work;
2. E017S is the supported static-witness row;
3. omitted bodyless trait effects are the real closed empty row;
4. body effect inference uses the existing fixed-point traversal and existing `EffectRow`, `EffectRowTail`, `EffectVar`, and `EffectSubstitution` types;
5. authored rows, inferred rows, actual/exposed row distinction, curried latent effects, static witnesses, and typed substitutions follow the parent contract;
6. E015, E016, E022, and E023 remain the typed diagnostic variants/codes/range rules selected by the parent;
7. `TraitCallableId`, synthesized empty resolver rows, copied requirement-as-implementation records, string/source callable effect IDs, project method-value rejection, generic `AWF-EFX-001` / `UpperBoundExceeded`, local-index/string runtime trait identity, and `(usize, String)` witness-method lookup remain mandatory deletions; and
8. the parent one-way checked-ID-to-`RuntimeCallableId` projection, conformance-keyed compiler lowering index, direct runtime method IDs, and no-runtime-source/row-resolution rule remain normative.

This correction replaces these parent clauses:

| Parent clause | Corrected clause |
|---|---|
| `CheckedCallableFacts` owns `signature`, `source`, and `access` | facts retain the exact `Arc<CallableRecord>` and delegate all record metadata |
| `CheckedCallableEffects::ExternalOrStandard { exposed }` owns a row copy | `CheckedCallableEffects::RecordFixed` stores no row and delegates to the record schema |
| `CallableEffectSchema::Project { declaration, declared }` | ID-only source-callable schema; the checked contract/row exists only in checked facts |
| `ProjectCallableSymbol` retains signature/source/hash copies | symbol retains structural key, checked ID, kind, and derived interface digest only |
| `ProjectGraphSymbolRef::Callable(QualifiedName)` or name reconstruction | `ProjectGraphSymbolRef::Callable(CheckedCallableId)` |
| Agent `owner:name` identity | canonical structural declaration digest |
| persistent interface signatures rebuilt from HIR | structural identity plus accepted-record signature/interface digests |
| checked context omits accepted environment catalog identity | project/environment contexts include exact accepted catalog digest; environment declarations are typed |

Where any wording in the parent archive conflicts with this table or the Rust-shaped declarations below, this package wins. No other parent decision is reopened.

## 2. Selected authority model

The final model is parent option 1: an effect-checking layer retaining the exact accepted record.

`RegisteredCallableCatalog` remains the sole accepted metadata catalog. `CallableRecord` remains the sole record owner for:

- structural candidate identity;
- lookup key and resolver authority rank;
- signature schema;
- exact declaration/name/result/parameter source;
- documentation;
- declaration access classification;
- provider provenance;
- environment publication provenance/digest;
- Rust provenance;
- declaration order; and
- fixed environment/standard effect rows.

`CheckedCallableCatalog` is not a replacement metadata catalog. It is the sole checked-context catalog. It binds accepted records to exact checked generations and owns only facts that do not exist before checking:

- revision-bound `CheckedCallableId`;
- checked execution role;
- body-bearing effect contract and final inferred row;
- bodyless requirement contract;
- trait method conformance and substitution;
- closure rows;
- exact source-to-ID and candidate-to-ID indices derived at freeze; and
- deterministic interface digests derived from the retained record and final checked row.

No field-by-field synchronization exists. A registered checked fact is valid only when its retained record is the same allocation stored by the accepted catalog.

```rust
Arc::ptr_eq(
    checked_facts.record(),
    registered_catalog.record(checked_facts.record().id())?,
)
```

The relationship is validated by the private builder and revalidated at accepted compiler/LSP publication boundaries.

## 3. Structural declaration identity

The parent project structural identities remain in `arcweft-lang-hir::symbol::identity`:

```rust
pub struct TraitDeclarationId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    name: ModuleSegment,
}

pub struct ImplDeclarationId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    source_ordinal: u32,
}

pub struct TraitMethodRequirementId {
    trait_declaration: TraitDeclarationId,
    method: ModuleSegment,
}

pub enum ImplMethodKind {
    Trait,
    Inherent,
}

pub struct ImplMethodDeclarationId {
    implementation: ImplDeclarationId,
    kind: ImplMethodKind,
    method: ModuleSegment,
}

pub enum CallableDeclarationKey {
    Existing(CallableDeclarationId),
    TraitRequirement(TraitMethodRequirementId),
    ImplMethod(ImplMethodDeclarationId),
}
```

`ProjectDeclarationId::Callable` is changed in place from `CallableDeclarationId` to `CallableDeclarationKey`. Trait requirements, trait implementation methods, and inherent methods are registered through that same callable declaration path. They are associated with their containing trait/impl IDs but are not inserted into module value scope.

Accepted environment callables keep the already accepted structural `EnvironmentCallableId`. Standard trait callables use the parent's `StandardCallableDeclarationId`. Detached declarations use `DetachedCallableDeclarationId` and never fabricate a project package.

The final checked declaration enum in `arcweft-lang-sema::callable::identity` is:

```rust
pub enum CheckedCallableDeclaration {
    Project(CallableDeclarationKey),
    Detached(DetachedCallableDeclarationId),
    Environment(EnvironmentCallableId),
    Standard(StandardCallableDeclarationId),
}
```

The new `Environment` variant is mandatory. Registered environment records are not keyed by a spelling, provider string, or reconstructed signature.

## 4. Exact checked identity and generation

### 4.1 Context

```rust
pub enum CheckedCallableContext {
    Project {
        world: ProjectSymbolWorldId,
        revision: ProjectSymbolRevision,
        catalog: RegisteredCallableCatalogDigest,
        standard: StandardTraitCatalogVersion,
    },
    Detached {
        source: SourceDocumentIdentity,
        standard: StandardTraitCatalogVersion,
    },
    Environment {
        catalog: RegisteredCallableCatalogDigest,
    },
    Standard {
        version: StandardTraitCatalogVersion,
    },
}

pub struct CheckedCallableId {
    context: CheckedCallableContext,
    declaration: CheckedCallableDeclaration,
}
```

Fields are private. There is no raw public constructor. The only crate-visible constructors are:

```rust
CheckedCallableId::for_project(...)
CheckedCallableId::for_detached(...)
CheckedCallableId::for_environment(...)
CheckedCallableId::for_standard(...)
```

Admissible pairs are closed:

| Constructor | Context | Declaration |
|---|---|---|
| `for_project` | `Project` | `Project` |
| `for_detached` | `Detached` | `Detached` |
| `for_environment` | `Environment` | `Environment` |
| `for_standard` | `Standard` | `Standard` |

Any other pair returns `CheckedCallableIdentityError::ContextMismatch`. No constructor changes a key, coerces a context, or derives identity from display text.

### 4.2 Catalog generation

```rust
pub struct CheckedCallableCatalogGeneration {
    origin: CheckedCallableCatalogOrigin,
    standard: StandardTraitCatalogVersion,
}

pub enum CheckedCallableCatalogOrigin {
    RegisteredProject {
        world: ProjectSymbolWorldId,
        revision: ProjectSymbolRevision,
        catalog: RegisteredCallableCatalogDigest,
    },
    Detached {
        source: SourceDocumentIdentity,
    },
}
```

A registered-project catalog contains project, environment, and standard IDs. A detached catalog contains detached and standard IDs. Every insertion and lookup validates the appropriate part of `generation`; a catalog never accepts a project ID from another world/revision, an environment ID from another catalog digest, a standard ID from another standard version, or a detached ID from another document identity.

Public lookups return typed failure, not `Option` alone:

```rust
pub fn callable(
    &self,
    id: &CheckedCallableId,
) -> Result<&CheckedCallableFacts, CheckedCallableLookupError>;
```

`CheckedCallableLookupError` distinguishes missing, wrong family, foreign world, stale project revision, foreign catalog digest, stale standard version, and foreign detached source. Every failure publishes no row and authorizes no fallback.

### 4.3 Checked digest amendment

The parent's `CheckedCallableId::semantic_digest()` remains the one-way runtime projection input, but its canonical encoding is amended so the accepted catalog generation and environment declaration family are represented.

```text
"arcweft.checked-callable.v2\0"
context-tag:u8
context-payload
declaration-tag:u8
declaration-payload
```

Context tags and payloads:

1. `0 Project`: project world package, root document ID, profile, raw project source-set revision, raw 32-byte registered catalog digest, standard catalog version `u32`.
2. `1 Detached`: document ID, raw source revision, source length `u64`, standard catalog version `u32`.
3. `2 Environment`: raw 32-byte registered catalog digest.
4. `3 Standard`: standard catalog version `u32`.

Declaration tags:

1. `0 ProjectExisting`;
2. `1 ProjectTraitRequirement`;
3. `2 ProjectImplMethod`;
4. `3 Detached`;
5. `4 Environment` using the accepted canonical `EnvironmentCallableId` encoder;
6. `5 Standard`.

Strings and vectors use `u32` little-endian lengths; integers are little-endian; revisions and digests contribute raw bytes; no `Debug` text, source spelling, pointer value, local index, or unordered-map iteration enters the digest. The runtime spelling is versioned with the new domain: `arcweft.checked.v2.<64-lowercase-hex>`.

## 5. Accepted record authority

### 5.1 Record shape

`CallableRecord` remains in `arcweft-lang-sema::callable::catalog` and is extended in place:

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

All fields remain private. `CallableRecord::try_new` becomes `pub(crate)` and is callable only from the accepted catalog builders and the private detached checked builder. Downstream crates receive read-only accessors; no public constructor can manufacture a record or replace one field.

`CallableCandidateId` is extended in its original enum implementation:

```rust
Project(CallableDeclarationKey),
Detached(DetachedCallableDeclarationId),
Environment(EnvironmentCallableId),
Standard(StandardCallableDeclarationId),
```

The legacy `TraitMethod(TraitCallableId)` variant is deleted.

### 5.2 Access owner

```rust
pub enum CallableAccess {
    Direct {
        declaration_visibility: Option<Visibility>,
    },
    TraitRequirement {
        trait_declaration: TraitDeclarationId,
        trait_visibility: Option<Visibility>,
    },
    TraitImplementation,
    InherentMethod {
        owner_module: CanonicalModulePath,
    },
    Environment,
    Standard,
    Detached,
}
```

The enum lives with `CallableRecord`; fields are private and behavior is implemented in its original inherent impl. The record owns declaration-level access classification. Project import/reexport bindings retain their own binding visibility because that is edge-specific access, not a duplicate declaration property. No checked fact, project callable symbol, Agent record, or persistent object owns another declaration-access authority.

- trait requirement access follows the original trait declaration;
- trait implementation methods are reachable only through a validated conformance and never by direct module name;
- inherent methods follow the owner-module rule and do not restore `pub impl`;
- environment access is determined by the accepted publication and resolver index retained in the same catalog;
- standard and detached access use their closed policies.

### 5.3 Signature effect schema

The source-callable effect schema becomes ID-only:

```rust
pub enum CallableEffectSchema {
    Project {
        declaration: CallableDeclarationKey,
    },
    Detached {
        declaration: DetachedCallableDeclarationId,
    },
    Fixed(EffectRow),
}
```

`Project { declared }` is deleted. `Fixed` remains the authoritative row for accepted environment/standard records. Source body/contract rows are absent from the accepted record and are owned only by checked facts after inference.

### 5.4 Trait records

The compact `TraitCatalog` remains private resolution storage, not a signature/effect catalog. Its method records are reduced to identity and trait/body metadata:

```rust
pub struct TraitMethodRequirement {
    declaration: CheckedCallableId,
    trait_id: TraitId,
    self_parameter: GenericTypeParameterId,
}

pub struct TraitMethodImpl {
    declaration: CheckedCallableId,
    trait_id: Option<TraitId>,
    body: Option<TraitMethodBody>,
}
```

The copied `FnSignature`, parameter groups, return type, source, and effect row are removed. Signature and source queries use `CheckedCallableFacts::record()`. `TraitMethodResolution` returns checked IDs and conformance/witness evidence; it does not clone method records. Inherited requirements retain the original declaring requirement ID.

## 6. Checked catalog authority

### 6.1 Final immutable shape

```rust
pub struct CheckedCallableCatalog {
    generation: CheckedCallableCatalogGeneration,
    registered: Option<Arc<RegisteredCallableCatalog>>,
    records: BTreeMap<CheckedCallableId, CheckedCallableFacts>,
    checked_by_candidate: BTreeMap<CallableCandidateId, CheckedCallableId>,
    conformances: BTreeMap<TraitMethodConformanceId, TraitMethodConformance>,
    closure_rows: BTreeMap<CheckedClosureId, EffectRow>,
    source_index: BTreeMap<CheckedCallableSourceKey, CheckedCallableId>,
}

pub struct CheckedCallableFacts {
    id: CheckedCallableId,
    record: Arc<CallableRecord>,
    execution: CheckedCallableExecution,
    effects: CheckedCallableEffects,
    interface_digest: CallableInterfaceDigest,
}

pub enum CheckedCallableEffects {
    Body {
        contract: CallableEffectContract,
        inferred: EffectRow,
    },
    BodylessTraitRequirement {
        contract: CallableEffectContract,
    },
    RecordFixed,
}
```

All fields are private. Final constructors are `pub(crate)` and only the consuming `CheckedCallableCatalogBuilder::finish` can call them. `checked_by_candidate` and `source_index` are immutable derived indices inside the same catalog, not alternate records. They contain IDs only.

For registered-project checking, `registered` is `Some(exact_arc)`. Every project/environment/standard fact is built from an exact record returned by that Arc. For detached checking, `registered` is `None`; the private builder creates each detached/standard record exactly once and moves the same Arc into the pending shell/final fact. No public detached record collection exists.

### 6.2 Inherent API

The required API is:

```rust
impl CheckedCallableFacts {
    pub const fn id(&self) -> &CheckedCallableId;
    pub const fn record(&self) -> &Arc<CallableRecord>;
    pub fn signature(&self) -> &CallableSignatureSchema;
    pub fn source(&self) -> Option<&CallableSource>;
    pub fn documentation(&self) -> &CallableDocumentation;
    pub fn access(&self) -> &CallableAccess;
    pub fn provider(&self) -> &CallableProviderId;
    pub fn publication_digest(&self) -> Option<EnvironmentCallablePublicationDigest>;
    pub const fn execution(&self) -> CheckedCallableExecution;
    pub fn actual_row(&self) -> Option<&EffectRow>;
    pub fn exposed_row(&self) -> &EffectRow;
    pub const fn interface_digest(&self) -> CallableInterfaceDigest;
}

impl CheckedCallableCatalog {
    pub const fn generation(&self) -> &CheckedCallableCatalogGeneration;
    pub const fn registered_catalog(&self) -> Option<&Arc<RegisteredCallableCatalog>>;
    pub fn callable(&self, id: &CheckedCallableId)
        -> Result<&CheckedCallableFacts, CheckedCallableLookupError>;
    pub fn checked_for_candidate(&self, id: &CallableCandidateId)
        -> Result<&CheckedCallableId, CheckedCallableLookupError>;
    pub fn callable_at_source(&self, key: &CheckedCallableSourceKey)
        -> Result<&CheckedCallableId, CheckedCallableLookupError>;
    pub fn conformance(&self, id: &TraitMethodConformanceId)
        -> Result<&TraitMethodConformance, CheckedCallableLookupError>;
    pub fn closure_row(&self, id: &CheckedClosureId)
        -> Result<&EffectRow, CheckedCallableLookupError>;
}
```

`signature`, `source`, `documentation`, `access`, `provider`, and `publication_digest` delegate directly to `record`. They are not fields on checked facts.

`actual_row()` / `exposed_row()` are exact:

- `Body`: actual is `inferred`; exposed is the authored bounded row when present, otherwise `inferred`;
- `BodylessTraitRequirement`: actual is absent; exposed is the bounded requirement contract, including the real closed empty row for omission;
- `RecordFixed`: actual is absent; exposed is the `EffectRow` held by `record.schema().effects().Fixed`; construction rejects any other schema.

No consumer reproduces this match.

### 6.3 Interface digest

`CallableInterfaceDigest([u8; 32])` is a derived, non-authoritative projection created only at freeze. For project declarations it is BLAKE3 over:

```text
"arcweft.callable-interface.v1\0"
CallableDeclarationKey canonical structural bytes
CallableSignatureSchema::semantic_digest raw bytes
CallableAccess canonical bytes
CallableProviderId canonical bytes
publication-digest option tag and bytes
CheckedCallableExecution tag
exposed EffectRow canonical digest
```

Environment and standard records replace the project structural bytes with the accepted canonical environment/standard declaration bytes. Source spans, documentation prose, pointer addresses, checked generation, and display names do not enter the interface digest. Documentation/source changes remain detected by the accepted catalog digest and source revision; signature/access/provider/exposed contract changes affect the durable interface digest.

## 7. Construction, transaction, and publication order

The order is mandatory:

1. HIR publishes exact structural `CallableDeclarationKey` values for every source callable, including trait requirements, trait implementation methods, and inherent methods.
2. `RegisteredCallableCatalogBuilder` registers all project records. Method records enter the same by-declaration storage but not module value bindings.
3. The same builder consumes every accepted environment publication and installed standard publication. Each publication record is moved once into one `Arc<CallableRecord>`.
4. `RegisteredCallableCatalogBuilder::finish` freezes one immutable `Arc<RegisteredCallableCatalog>`. `RegisteredTypeCheckEnv` and `RegisteredSemanticWorld` retain that Arc.
5. `CheckedCallableCatalogBuilder::for_registered` receives that exact Arc plus world, revision, catalog digest, and standard version. Detached checking uses `for_detached` with exact source identity and standard version.
6. The private builder creates one pending shell for every admitted callable. Registered shells retain an `Arc::clone` of the exact accepted record. Pending shells are not publicly queryable.
7. The existing checker traversal and effect fixed point populate body facts, closure rows, call edges, and final inferred rows. Trait signature compatibility and effect subset validation create conformances only after both exact checked IDs and substitutions are known.
8. Any diagnostic, missing/stale record, source mismatch, work-limit failure, unresolved row, or conformance failure aborts the transaction. No public catalog is produced.
9. `finish` validates every shell, candidate index, source index, conformance, fixed-row binding, and exact registered pointer; then consumes the builder and returns one `Arc<CheckedCallableCatalog>`.
10. `TypeCheckReport` is constructed with that Arc. It has no separate public callable effect map or callable-execution vector authority.
11. `ProjectSemanticIndex` is constructed from the successful report and retains `Arc::clone` of the same checked catalog. Its constructor validates `Arc::ptr_eq` and the generation.
12. `CompiledProject` retains the accepted registered world and the report. Compiler lowerers borrow the report's checked catalog; they do not receive a second catalog parameter.
13. LSP `AcceptedProfileCandidate` validates registered world/HIR/source identities and additionally validates the checked catalog generation and accepted registered catalog Arc before atomic publication.
14. Agent graph/function payloads and persistent interface summaries are generated only from the accepted `ProjectSemanticIndex` / checked catalog. Failure returns no partial payload.

### Rollback

The private builder journal covers:

- pending shell insertion;
- checked candidate and source index insertion;
- current callable stack;
- effect variable allocation/substitution;
- call/effect graph edges;
- closure rows;
- trait conformances and substitutions;
- inferred/final row assignment; and
- derived interface digest staging.

Rollback removes all mutations after the checkpoint and restores allocation counters/current callable. A rolled-back ID is never inserted into a final catalog, project relation, Agent payload, runtime lowering index, or persistent object. The accepted `RegisteredCallableCatalog` is immutable and never rolled back or mutated by checking.

## 8. TypeCheckReport and project index

### 8.1 TypeCheckReport

`TypeCheckReport` gains:

```rust
checked_callables: Arc<CheckedCallableCatalog>,
```

and exposes:

```rust
pub const fn checked_callables(&self) -> &Arc<CheckedCallableCatalog>;
```

The old separate public authorities are deleted or made private builder state:

- standalone public `EffectAnalysisReport` row lookup for declarations;
- `callable_executions: Vec<CheckedCallableExecution>`;
- public trait-method copied signature/effect records; and
- string/local-index closure/callable effect lookup.

Non-callable type-check facts remain in the report. Typed diagnostics retain the parent path.

### 8.2 ProjectSemanticIndex

```rust
pub struct ProjectSemanticIndex {
    checked_callables: Arc<CheckedCallableCatalog>,
    project_callables: BTreeMap<CallableDeclarationKey, ProjectCallableSymbol>,
    environment_lowerings: BTreeMap<EnvironmentCallableId, EnvironmentCallableLowering>,
    relations: Vec<ProjectGraphRelation>,
    dependency_relations: Vec<ProjectGraphDependencyRelation>,
    /* existing non-callable fields */
}

pub struct ProjectCallableSymbol {
    declaration: CallableDeclarationKey,
    checked: CheckedCallableId,
    kind: ProjectCallableKind,
    interface_digest: CallableInterfaceDigest,
}

pub enum ProjectCallableKind {
    Function,
    View,
    TraitRequirement,
    TraitImplementation,
    InherentMethod,
}

pub struct EnvironmentCallableLowering {
    checked: CheckedCallableId,
    lowering: CallableLowering,
}

pub enum ProjectGraphSymbolRef {
    Entity(PublicId),
    Callable(CheckedCallableId),
}
```

`ProjectCallableKind::as_str` and family predicates are implemented on the original enum. `ProjectCallableSymbol` stores no signature, source, documentation, access, provider, publication, or effect row. `environment_lowerings` stores only exact identity and the non-authoritative lowering projection; it is not a metadata registry.

The map key is structural and durable. The stored checked ID is the validated revision-bound join. The constructor proves:

- the checked declaration is `Project(declaration.clone())`;
- the catalog contains that ID;
- the retained record candidate is `Project(declaration.clone())`; and
- the interface digest equals the fact's derived digest.

Required lookups are exact:

```rust
project_callable_by_declaration(&CallableDeclarationKey)
checked_callable(&CheckedCallableId)
environment_lowering(&EnvironmentCallableId)
```

`project_callable(&QualifiedName)` and all name-based uniqueness scans are deleted.

Dependency relations are built from checked call-target facts and conformance IDs, not by walking raw HIR names. A source callable parent is resolved from its structural key to the stored checked ID once. A call edge target is the `CheckedCallableId` already selected by sema. Missing IDs are typed construction errors; no spelling fallback exists.

## 9. LSP, Agent, persistent, and runtime scope

### 9.1 LSP

A declaration query follows:

```text
accepted profile generation
  -> exact open document version/source identity
  -> attached syntax/HIR source index
  -> CallableDeclarationKey
  -> ProjectCallableSymbol.checked
  -> same Arc<CheckedCallableCatalog>
  -> exact CheckedCallableFacts / Arc<CallableRecord>
  -> projected hover/signature/navigation/diagnostic
```

A call-site query uses the accepted call-target fact's `CheckedCallableId` directly. Hover/signature help read record schema/docs plus `exposed_row`; navigation reads record source; diagnostics use parent typed spans. Stale version, world, catalog digest, standard version, source identity, or catalog Arc discards the result. LSP does not try raw HIR, declaration name, callee spelling, or a reconstructed schema.

### 9.2 Agent

Project callable graph identity is structural:

```text
project:callable:v1:<64-lowercase-hex CallableDeclarationKey digest>
```

Environment callable graph identity is:

```text
project:environment-callable:v1:<64-lowercase-hex EnvironmentCallableId digest>
```

`qualified_name` and summaries are display-only. `semantic_hash` is the `CallableInterfaceDigest`. Graph edges originate from validated checked relation refs and are converted to durable IDs; a missing conversion is an error. The current `owner:name` construction and fallback from `ProjectGraphSymbolRef::Callable(QualifiedName)` are deleted.

Agent function/signature/effect protocol payloads contain only the rendered output explicitly requested by the protocol operation. They are one-shot projections from the same checked catalog and are never accepted back as a resolver, cache authority, or type-check input. `ProjectSemanticIndex` does not store a copied Agent signature/effect record.

### 9.3 Persistent interface summary

The compiler-private `.awbo` interface schema is directly replaced and its schema version incremented in the same cut. No old decoder remains.

```rust
pub enum PublicSymbolObject {
    Flow(PublicFlowObject),
    Callable(PublicCallableObject),
    Declaration(PublicDeclarationObject),
}

pub struct PublicCallableObject {
    declaration: PersistentCallableDeclaration,
    declaration_digest: BuildDigest,
    display_name: String,
    kind: PublicCallableKind,
    signature_digest: BuildDigest,
    interface_digest: BuildDigest,
}

pub enum PersistentCallableDeclaration {
    Existing {
        package: String,
        module: Vec<String>,
        owner: PersistentCallableOwner,
        owner_path: Vec<String>,
        name: String,
    },
    TraitRequirement {
        trait_package: String,
        trait_module: Vec<String>,
        trait_name: String,
        method: String,
    },
    ImplMethod {
        package: String,
        module: Vec<String>,
        source_ordinal: u32,
        kind: PersistentImplMethodKind,
        method: String,
    },
}
```

The persistent declaration is an exact typed serialization of `CallableDeclarationKey`; `declaration_digest` is recomputed and validated on decode. `signature_digest` is copied only as the required durable digest from `CallableRecord::schema().semantic_digest()`. `interface_digest` is the derived checked interface digest. The object contains no `CheckedCallableId`, source span, effect row, provider DTO, or copied signature schema.

`InterfaceSummaryFactsInput` is changed to accept the exact module, accepted source, `ProjectSemanticIndex`, and its shared checked catalog. Callable entries are selected from project symbols and catalog records. The current HIR `FnSignature` encoder for callable interface authority and fabricated `decl:{index}:{tag}` callable identity are deleted. HIR continues to contribute non-callable flow/declaration facts only; it cannot create callable interface rows.

The persistent key/stage inputs include registered catalog digest, standard catalog version, and checked interface digest root. Missing, foreign, or stale records produce a typed builder error and no object; there is no raw-HIR fallback or soft reconstruction. Ordinary cache corruption/staleness remains a cache miss under the existing private-object policy.

### 9.4 Compiler/runtime

Compiler lowerers borrow `TypeCheckReport::checked_callables()`. Trait implementation inputs are keyed by `TraitMethodConformanceId`; inherent inputs by exact checked ID. The parent runtime shapes remain:

```rust
pub struct RuntimeTraitMethodIdentity {
    implementation: RuntimeCallableId,
    requirement: Option<RuntimeCallableId>,
}

pub(crate) struct RuntimeTraitMethodLoweringIndex {
    by_conformance: BTreeMap<TraitMethodConformanceId, RuntimeTraitMethodId>,
    by_inherent: BTreeMap<CheckedCallableId, RuntimeTraitMethodId>,
}
```

Lower inputs are sorted by typed IDs before plan-local IDs are assigned. The compiler consumes rows/conformance before lowering. Runtime stores no checked catalog, source identity, trait/method display string, effect row, local trait/impl index, or name lookup.

## 10. Fail-closed rules

The following outcomes are mandatory:

- **missing record:** typed error; no checked shell/fact/edge;
- **foreign accepted record:** pointer/candidate mismatch; entire checked transaction fails;
- **stale project world/revision:** no checked record/report/index/LSP/Agent/persistent output;
- **foreign catalog digest:** environment/project record unavailable; no fallback;
- **stale standard version:** standard record unavailable; whole publication fails if referenced or admitted;
- **foreign detached source:** no detached checked ID or record;
- **private/inaccessible declaration:** candidate is rejected before call/effect edge commit;
- **ambiguous declarations:** deterministic typed candidate set; no first-name selection;
- **rollback:** all pending checked state is removed; immutable accepted records remain unchanged;
- **persistent mismatch:** no interface object; no HIR reconstruction;
- **LSP mismatch:** stale response discarded; no stale ranges or text lookup;
- **Agent mismatch:** no partial graph/function payload;
- **runtime mismatch:** plan verification fails; runtime never reconstructs identity.

## 11. Explicit prohibitions

Implementation MUST NOT add or retain:

- `CheckedCallableFacts.signature`, `.source`, `.documentation`, `.access`, `.provider`, `.publication`, or fixed-row copies;
- a second `CheckedCallableCatalog`-like metadata registry;
- a trait-only signature catalog or effect registry;
- a DTO/view synchronized field-by-field with `CallableRecord`;
- name, string, source-text, raw-HIR, or local-index fallback;
- public pending-shell readers;
- compatibility aliases, shims, dual readers, source gates, old schema readers, or V2 compatibility envelopes;
- removed-syntax-only diagnostics;
- CSS or Takumi paths; or
- ad hoc family helpers/extension traits where the original Arcweft enum owns the behavior.

This contract leaves no implementation choice among alternate authorities or fallback paths.
