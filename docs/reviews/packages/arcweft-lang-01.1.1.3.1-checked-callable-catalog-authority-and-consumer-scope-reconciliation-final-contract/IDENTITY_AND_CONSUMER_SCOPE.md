# Identity and consumer scope

## 1. Identity classes

Arcweft uses three deliberately different identity classes. They are not interchangeable and do not form fallback tiers.

| Class | Purpose | Examples | Persistence |
|---|---|---|---|
| structural declaration identity | names one declaration independent of one check generation | `CallableDeclarationKey`, `EnvironmentCallableId`, `StandardCallableDeclarationId` | canonically serialized/digested only at the durable boundaries specified below |
| checked identity | binds a structural/context-local declaration to exact semantic inputs | `CheckedCallableId`, `CheckedClosureId`, `TraitMethodConformanceId` | live checked/compiler/LSP state only; never interface-summary identity |
| downstream projection identity | identifies an emitted runtime or durable read-only projection | `RuntimeCallableId`, structural Agent symbol ID, persistent declaration object/digest | one-way; never resolves back to sema records |

A structural identity is not proof that a checked record exists. A checked identity is not a durable public artifact identity. A downstream projection is not accepted by the resolver.

## 2. Final structural identities

### 2.1 Project callables

`CallableDeclarationKey` is the sole project callable key:

```rust
pub enum CallableDeclarationKey {
    Existing(CallableDeclarationId),
    TraitRequirement(TraitMethodRequirementId),
    ImplMethod(ImplMethodDeclarationId),
}
```

The existing HIR owner enum is extended directly:

```rust
pub enum CallableDeclarationOwner {
    Function,
    ExternCapability,
    View,
    Predicate,
    Proof,
    TraitRequirement,
    TraitImplementation,
    InherentMethod,
}
```

The original inherent impl receives exhaustive `as_str`, runtime/logical/proof predicates, `is_method`, `is_dispatch_contract`, and canonical digest tags. No separate helper owns this behavior.

New-variant behavior:

| Owner | runtime callable | method | dispatch contract |
|---|---:|---:|---:|
| `TraitRequirement` | no | yes | yes |
| `TraitImplementation` | yes | yes | no |
| `InherentMethod` | yes | yes | no |

### 2.2 Environment and standard callables

Accepted environment callables retain `EnvironmentCallableId`. The ID's existing owner/kind/lookup-key/overload components are the structural identity. The correction adds its use in `CheckedCallableDeclaration::Environment`; it does not introduce a string wrapper.

Standard methods use:

```rust
pub struct StandardCallableDeclarationId {
    owner: CallableDeclarationOwner,
    catalog_ordinal: u32,
}
```

The ordinal belongs to one `StandardTraitCatalogVersion`; reorder/change requires a version increment.

### 2.3 Detached callables

```rust
pub struct DetachedCallableDeclarationId {
    owner: CallableDeclarationOwner,
    source_ordinal: u32,
}
```

The ordinal is assigned from the unified deterministic source order of admitted detached callables. The exact `SourceDocumentIdentity` is in checked context. No project package or canonical module is fabricated.

## 3. Checked identity

```rust
pub enum CheckedCallableDeclaration {
    Project(CallableDeclarationKey),
    Detached(DetachedCallableDeclarationId),
    Environment(EnvironmentCallableId),
    Standard(StandardCallableDeclarationId),
}

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

All fields are private. Owner-specific crate-visible constructors validate exact pairs. Read-only accessors and `Ord`/`Hash` are public where sema/compiler/LSP need them. There is no public raw constructor, string parser, serde identity input, or local-index conversion.

Closures remain:

```rust
pub struct CheckedClosureId {
    owner: CheckedCallableId,
    expression: SourceSpan,
}

pub enum CheckedEffectCallableId {
    Declaration(CheckedCallableId),
    Closure(CheckedClosureId),
}
```

The closure span must belong to the owner's accepted source generation. A source-less/foreign closure is rejected.

## 4. Durable structural digest

Project durable consumers use a digest of `CallableDeclarationKey`, not `CheckedCallableId`.

```rust
pub struct CallableDeclarationDigest([u8; 32]);

impl CallableDeclarationKey {
    pub fn semantic_digest(&self) -> CallableDeclarationDigest;
}
```

The method is implemented in the original HIR identity owner. It hashes:

```text
"arcweft.callable-declaration.v1\0"
variant-tag:u8
variant-payload
```

Tags/payloads:

1. `0 Existing`: package, canonical module segment vector, `CallableDeclarationOwner` tag, owner-path vector, name.
2. `1 TraitRequirement`: trait package, trait canonical module segment vector, trait name, method.
3. `2 ImplMethod`: implementation package, canonical module segment vector, source ordinal `u32`, `ImplMethodKind` tag, method.

String/vector lengths are `u32` little-endian. No source revision, display qualification, `Debug` text, source span, or checked generation enters this digest.

Environment durable consumers use an inherent canonical digest on `EnvironmentCallableId` with domain `arcweft.environment-callable-id.v1\0`. Standard durable consumers use standard version plus standard declaration ID where a durable standard projection is required.

## 5. Project semantic index types

### 5.1 Map and symbol

The exact storage is:

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
```

The map key is structural. `ProjectCallableSymbol::checked` is the exact revision-bound join and must be `CheckedCallableDeclaration::Project(declaration.clone())`. The symbol does not store signature, source, docs, access, provider, publication, or effect row.

`ProjectCallableKind` is extended in its original enum:

```rust
pub enum ProjectCallableKind {
    Function,
    View,
    TraitRequirement,
    TraitImplementation,
    InherentMethod,
}
```

Its inherent `as_str` values are exactly:

- `function`
- `view`
- `trait_requirement`
- `trait_implementation`
- `inherent_method`

### 5.2 Environment lowering projection

```rust
pub struct EnvironmentCallableLowering {
    checked: CheckedCallableId,
    lowering: CallableLowering,
}
```

This stores only exact identity and an execution/lowering projection. It has no signature/effect metadata and cannot answer semantic queries. The old name-keyed `CallableSymbol { signature, effects, ... }` authority and `ProjectSemanticIndex::typecheck_env()` reconstruction are deleted. Environment metadata is read from the checked catalog's exact record.

### 5.3 Graph references and relations

```rust
pub enum ProjectGraphSymbolRef {
    Entity(PublicId),
    Callable(CheckedCallableId),
}

pub struct ProjectGraphDependencyRelation {
    from: ProjectGraphSymbolRef,
    to: ProjectGraphSymbolRef,
    edge_kind: ProjectGraphDependencyRelationKind,
}
```

Project call relations use checked IDs because the graph is one immutable checked snapshot. This is not a durable wire identity. Agent projection maps each checked ID to its structural declaration only after generation validation.

No `Callable(QualifiedName)` variant remains. No relation builder scans a callee expression's name or raw HIR to choose a callable. Relations are built from the typed call target/conformance facts already committed by sema.

### 5.4 Lookup API

Allowed:

```rust
pub fn project_callable_by_declaration(
    &self,
    declaration: &CallableDeclarationKey,
) -> Option<&ProjectCallableSymbol>;

pub fn checked_callable(
    &self,
    id: &CheckedCallableId,
) -> Result<&CheckedCallableFacts, CheckedCallableLookupError>;

pub fn environment_lowering(
    &self,
    id: &EnvironmentCallableId,
) -> Option<&EnvironmentCallableLowering>;
```

Deleted:

- `project_callable(&QualifiedName)`;
- linear scan by `qualified_name()`;
- fallback to a name when a checked record is absent;
- reconstruction of `FunctionSignature`/effect row from project-index copies; and
- any use of project index as a replacement type-check environment.

## 6. Source-to-record resolution

### 6.1 Declaration locations

The frozen checked catalog has an ID-only source index. The key is a validated source identity plus exact source category/range, not a line/column or name:

```rust
pub struct CheckedCallableSourceKey {
    source: SourceDocumentIdentity,
    category: CheckedCallableSourceCategory,
    range: TextRange,
}
```

Categories distinguish declaration, name, signature, effect contract, and body. The index is derived from `CallableRecord::source()` and points to `CheckedCallableId`. It does not own another `CallableSource`.

### 6.2 Call locations

Direct and bound call facts retain:

```rust
pub struct CheckedCallTarget {
    target: CheckedCallableId,
    conformance: Option<TraitMethodConformanceId>,
    /* retained argument/curry/source facts */
}
```

Concrete trait calls use implementation ID plus conformance. Static witness calls retain the requirement ID plus conformance/witness evidence. Inherent calls use the inherent implementation ID. No call fact stores a signature or effect row.

### 6.3 LSP route

LSP uses one of two exact routes:

- declaration cursor -> source index -> checked ID;
- call cursor -> accepted call-target fact -> checked ID.

It then queries the same catalog Arc. If source/generation validation fails, the result is discarded. It does not search by spelling, rescan HIR, or rebuild a schema.

## 7. Durable Agent identity

### 7.1 Symbol IDs

Project callable:

```text
project:callable:v1:<CallableDeclarationDigest hex>
```

Environment callable:

```text
project:environment-callable:v1:<EnvironmentCallableId digest hex>
```

Entity/action/debug IDs retain their existing families. Callable IDs no longer use `owner:name`, canonical display name, or map-key fallback.

### 7.2 Validation and payload

Before projecting a project callable, Agent conversion validates:

1. the graph ref checked ID belongs to the index catalog generation;
2. the checked fact exists;
3. its record candidate is the expected structural declaration;
4. the project symbol map contains that declaration/checked pair; and
5. the derived interface digest matches.

The protocol receives structural symbol ID, display qualified name, kind, and interface semantic hash. A signature/effect response is rendered directly from the checked fact at request time. Protocol output is not stored back into `ProjectSemanticIndex` and cannot be used as semantic input.

Same-named Function/View/trait/impl/inherent declarations remain distinct because their structural digests differ. A display-name collision changes neither identity nor edge target.

## 8. Persistent interface identity

### 8.1 Serialized structural identity

`PersistentCallableDeclaration` serializes the exact structural project key fields. It is a codec projection owned by the compiler-private persistent schema, not a second HIR identity API. Decode validates every string/segment/tag and recomputes `CallableDeclarationDigest`.

### 8.2 Public callable object

```rust
pub struct PublicCallableObject {
    declaration: PersistentCallableDeclaration,
    declaration_digest: BuildDigest,
    display_name: String,
    kind: PublicCallableKind,
    signature_digest: BuildDigest,
    interface_digest: BuildDigest,
}
```

- `declaration` / `declaration_digest`: durable structural identity;
- `display_name`: non-authoritative presentation;
- `signature_digest`: exact `CallableSignatureSchema::semantic_digest()` from retained record;
- `interface_digest`: derived final checked interface digest.

Not serialized:

- `CheckedCallableId` or checked context;
- `Arc`/pointer identity;
- `CallableSignatureSchema` itself;
- effect row;
- source spans;
- provider/access DTO copies;
- local trait/impl/witness indices.

The object is keyed by compiler identity, source/dependency inputs, registered catalog digest, standard version, and interface digest root. It is not accepted as a source of live source ranges or a replacement checked catalog.

## 9. Runtime identity

The parent runtime projection remains one-way, with the corrected checked digest v2:

```rust
impl CheckedCallableId {
    pub fn semantic_digest(&self) -> CheckedCallableDigest;
}

impl RuntimeCallableId {
    pub fn from_checked_digest(digest: [u8; 32]) -> Self;
}
```

These methods live on the original owner types. Runtime never parses the canonical string back into a checked/structural ID.

Trait-method runtime identity remains:

```rust
pub struct RuntimeTraitMethodIdentity {
    implementation: RuntimeCallableId,
    requirement: Option<RuntimeCallableId>,
}
```

The compiler-only lowering index maps typed conformance/inherent IDs to direct plan-local IDs and is discarded after lowering. Runtime inventory contains method vector only.

## 10. Consumer table

| Consumer | Sole metadata/row owner | Retained identity/reference | Stored vs projected behavior |
|---|---|---|---|
| registered project callable | exact `Arc<CallableRecord>` in `RegisteredCallableCatalog::project.by_declaration` | `CallableDeclarationKey` / `CallableCandidateId::Project` | stores authoritative record |
| registered environment callable | exact `Arc<CallableRecord>` in environment catalog | `EnvironmentCallableId` / candidate | stores authoritative record and fixed row |
| standard callable | same exact accepted record in registered catalog, or one private detached record | `StandardCallableDeclarationId` + version | stores authoritative fixed record; standard structural catalog stores IDs/witnesses only |
| detached source callable | exact Arc created once by private checked builder | `DetachedCallableDeclarationId` + source context | record exists only inside pending/final checked fact |
| ordinary Function/View symbol | checked catalog + exact record | map key `CallableDeclarationKey`; value `CheckedCallableId` | symbol stores IDs/kind/interface digest only |
| trait requirement | accepted record + checked fact | requirement `CheckedCallableId` | trait catalog stores ID/self parameter; no signature/row copy |
| trait implementation method | accepted record + checked fact | implementation `CheckedCallableId` | trait catalog stores ID/body metadata; conformance references IDs |
| inherent method | accepted record + checked fact | inherent implementation `CheckedCallableId` | receiver index selects ID; no direct name/value binding |
| direct call fact | checked catalog | target `CheckedCallableId` | stores ID/argument/source facts; projects exposed row during check |
| bound method value | checked catalog | target ID plus conformance/witness and curry group | stores typed target; latent row is a typed call fact, not another declaration record |
| effect graph edge | checked catalog builder/final effect evidence | `CheckedEffectCallableId` endpoints | stores typed IDs; rows queried from facts |
| trait conformance | checked catalog | `TraitMethodConformanceId` | stores witness/substitution only |
| `TypeCheckReport` | same checked catalog Arc | `Arc<CheckedCallableCatalog>` | stores one Arc; no public row/execution duplicate |
| `ProjectSemanticIndex` | same checked catalog Arc | structural map key + checked ID | stores identity/kind/digest only |
| accepted LSP profile | compiled report/index with same catalog Arc | accepted environment generation + checked/source IDs | caches projections keyed by generation; no fallback |
| LSP hover/signature/navigation | same checked fact/record | source/call fact -> checked ID | projects response only |
| Agent project graph | same project index/catalog during build | structural declaration/environment digest | wire projection only; names display-only |
| Agent function payload | same checked fact/record | structural symbol ID + interface digest | renders signature/exposed row; no internal copied row record |
| compiler `InterfaceSummary` | same index/catalog during build | serialized structural declaration + digests | durable projection; no checked handle/row/schema |
| compiler trait lowering | same checked catalog | checked implementation/requirement + conformance | projects runtime IDs once |
| runtime trait execution | runtime plan | `RuntimeCallableId`, `RuntimeTraitMethodId` | no source/signature/effect lookup |

## 11. Stale, foreign, missing, ambiguous

- **Structural key found but checked ID missing:** construction/query error; no name lookup.
- **Checked ID from another catalog generation:** typed stale/foreign error.
- **Accepted record value-equal but pointer-distinct:** foreign record error.
- **Project source revision changed:** old project checked IDs and source index are rejected.
- **Environment manifest/catalog changed:** old environment checked IDs are rejected by catalog digest.
- **Standard version changed:** old standard checked IDs are rejected.
- **Detached source changed:** old detached IDs/closure spans are rejected.
- **Same display name:** deterministic distinct structural/checked candidates; ambiguity is reported with typed IDs.
- **Private candidate:** access failure before call/effect edge commit.
- **Missing Agent/persistent projection target:** whole projection fails; no string fallback or partial output.

## 12. Forbidden identity paths

The following are unavailable after the switch:

- `TraitCallableId`;
- `CallableId(String)` / source-string effect IDs;
- `ProjectGraphSymbolRef::Callable(QualifiedName)`;
- `ProjectSemanticIndex::project_callable(name)`;
- Agent `owner:name` callable IDs;
- persistent `decl:{index}:{tag}` callable identity;
- transient checked ID in interface summary;
- local trait/impl/witness indices in runtime identity;
- `(usize, String)` runtime witness/method lookup;
- fallback from any typed miss to raw HIR, source text, name, or reconstructed signature.
