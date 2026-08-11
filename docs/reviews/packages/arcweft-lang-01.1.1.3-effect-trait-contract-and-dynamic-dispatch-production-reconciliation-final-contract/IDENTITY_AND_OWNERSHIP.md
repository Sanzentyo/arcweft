# Identity and ownership contract

## 1. Identity hierarchy

The accepted top-level `CallableDeclarationId` remains the structural identity
for its existing declaration families. Lang-01.1.1.3 adds typed structural keys
for traits and impl methods and then binds every structural key to the exact
checked context.

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraitDeclarationId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    name: ModuleSegment,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImplDeclarationId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    source_ordinal: u32,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraitMethodRequirementId {
    trait_declaration: TraitDeclarationId,
    method: ModuleSegment,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ImplMethodKind {
    Trait,
    Inherent,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ImplMethodDeclarationId {
    implementation: ImplDeclarationId,
    kind: ImplMethodKind,
    method: ModuleSegment,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableDeclarationKey {
    Existing(CallableDeclarationId),
    TraitRequirement(TraitMethodRequirementId),
    ImplMethod(ImplMethodDeclarationId),
}
```

These project structural types live in
`arcweft-lang-hir::symbol::identity`; HIR project publication owns their
construction. `source_ordinal` on `ImplDeclarationId` is assigned in source
order among admitted `HirTopLevelDecl::Impl` declarations in the same canonical
project module.

Detached and standard identities are checked-context identities rather than
project symbols and therefore live in `arcweft-lang-sema::callable::identity`:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DetachedCallableDeclarationId {
    owner: CallableDeclarationOwner,
    source_ordinal: u32,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StandardCallableDeclarationId {
    owner: CallableDeclarationOwner,
    catalog_ordinal: u32,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCallableDeclaration {
    Project(CallableDeclarationKey),
    Detached(DetachedCallableDeclarationId),
    Standard(StandardCallableDeclarationId),
}
```

Detached checking has no fabricated package. Its one unified callable ordinal
is assigned in sema after HIR lowering by sorting every admitted source callable
(ordinary function, trait requirement, trait impl method, and inherent method)
by `(whole_range.start, whole_range.end, owner stable tag, member source order)`
and numbering from zero. The exact `SourceDocumentIdentity` in its checked
context makes the ordinal revision-safe. Source-less detached HIR does not
receive a synthetic ID: `validate_typecheck_ready` records the existing
`TypeCheckReadinessError` message
`checked callable catalog requires an exact source document identity`; the
invalid report publishes no checked catalog and compiler/project-index
construction does not run.

One sema-owned `StandardTraitCatalogBuilder` assigns `catalog_ordinal` from zero
in its single append order across all installed requirements and
implementations. The ordinal is not reused within a catalog version; any
insertion, deletion, reorder, or semantic change increments
`STANDARD_TRAIT_CATALOG_VERSION` in the same commit. No project package or
source identity is fabricated for standard callables.

## 2. Revision-bound checked identity

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedCallableContext {
    Project {
        world: ProjectSymbolWorldId,
        revision: ProjectSymbolRevision,
    },
    Detached {
        source: SourceDocumentIdentity,
    },
    Standard {
        version: StandardTraitCatalogVersion,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedCallableId {
    context: CheckedCallableContext,
    declaration: CheckedCallableDeclaration,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedClosureId {
    owner: CheckedCallableId,
    expression: SourceSpan,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedEffectCallableId {
    Declaration(CheckedCallableId),
    Closure(CheckedClosureId),
}
```

`CheckedCallableDeclaration`, `CheckedCallableContext`, `CheckedCallableId`,
`CheckedClosureId`, `CheckedEffectCallableId`, `CheckedCallableDigest`, the
detached/standard declaration IDs, and the standard catalog version live in
`arcweft-lang-sema::callable::identity`. HIR owns project structural identity;
sema alone binds project or context-local declarations to a checked context.

The standard context is exact and versioned:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StandardTraitCatalogVersion(u32);

pub const STANDARD_TRAIT_CATALOG_VERSION: StandardTraitCatalogVersion =
    StandardTraitCatalogVersion(1);
```

The field is private; the owning module exposes `as_u32()` and no raw public
constructor. Every semantic change to an installed standard trait declaration,
method signature, effect contract, or witness mapping increments the constant
in the same commit. Programmatically installed standard methods use the same
checked catalog and row model and carry structured `StandardCallableSource`;
they never use a fake source span or display-name identity.

### Constructor visibility

- Identity fields are private.
- ID types and read-only accessors are `pub` where a downstream compiler or
  tooling crate must compare/query them.
- All source-binding constructors are `pub(crate)` in `arcweft-lang-hir` or
  `arcweft-lang-sema` builders.
- There is no public unchecked `new` accepting raw strings or local indices.
- `ModuleSegment::new`, `CallablePackageId`, canonical module paths, project
  world/revision, and source identity validation are reused.

The only constructors are owner-specific:

```rust
TraitDeclarationId::from_linked_trait(...)
ImplDeclarationId::from_linked_impl(...)
TraitMethodRequirementId::from_linked_member(...)
ImplMethodDeclarationId::from_linked_member(...)
DetachedCallableDeclarationId::from_source_order(...)
StandardCallableDeclarationId::from_catalog_append(...)
CheckedCallableId::for_project(...)
CheckedCallableId::for_detached(...)
CheckedCallableId::for_standard(...)
CheckedClosureId::from_checked_expression(...)
```

They validate owner/context consistency and exact source membership.
Admissible key/context pairs are closed:

| Constructor | Accepted declaration key |
|---|---|
| `for_project` | `CheckedCallableDeclaration::Project`; every contained package/module must match the project world |
| `for_detached` | `CheckedCallableDeclaration::Detached`; source identity must equal the bound HIR source |
| `for_standard` | `CheckedCallableDeclaration::Standard`; version must equal the installed standard catalog |

Every other pair returns `CheckedCallableIdentityError::ContextMismatch`; no
constructor coerces or rekeys a declaration.

## 3. Existing owner enum extension

`CallableDeclarationOwner` remains the owner of declaration-family behavior and
adds:

```rust
TraitRequirement,
TraitImplementation,
InherentMethod,
```

Its existing inherent `as_str`, `is_runtime_callable`,
`is_logical_callable`, and `permits_proof_statement_call` matches are updated in
the original impl. It additionally owns:

```rust
pub const fn is_method(self) -> bool;
pub const fn is_dispatch_contract(self) -> bool;
```

`CallableDeclarationKey` itself owns the sole family projection in its original
inherent impl:

```rust
impl CallableDeclarationKey {
    pub const fn owner(&self) -> CallableDeclarationOwner {
        match self {
            Self::Existing(id) => id.owner(),
            Self::TraitRequirement(_) => CallableDeclarationOwner::TraitRequirement,
            Self::ImplMethod(id) => match id.kind() {
                ImplMethodKind::Trait => CallableDeclarationOwner::TraitImplementation,
                ImplMethodKind::Inherent => CallableDeclarationOwner::InherentMethod,
            },
        }
    }
}

impl CheckedCallableDeclaration {
    pub const fn owner(&self) -> CallableDeclarationOwner {
        match self {
            Self::Project(key) => key.owner(),
            Self::Detached(id) => id.owner(),
            Self::Standard(id) => id.owner(),
        }
    }
}
```

Each match is implemented on its original owning enum. No caller repeats either
match, and no extension trait/helper owns them.

New-variant semantics are exact:

| Variant | runtime callable | method | dispatch contract |
|---|---:|---:|---:|
| `TraitRequirement` | no | yes | yes |
| `TraitImplementation` | yes | yes | no |
| `InherentMethod` | yes | yes | no |

Existing variants retain current behavior. No extension trait or duplicated
match helper is allowed.

## 4. Project symbol ownership

The project symbol table adds typed trait and impl records:

```rust
pub enum ProjectDeclarationId {
    Callable(CallableDeclarationKey),
    Trait(TraitDeclarationId),
    Impl(ImplDeclarationId),
    External(ExternalDeclarationId),
    Nominal(ProjectNominalDeclarationId),
}
```

Existing ordinary/project callables are wrapped as
`CallableDeclarationKey::Existing`; requirement and impl/inherent methods use
their structural key variants. All are stored through the one
`ProjectDeclarationId::Callable` symbol path. Method callable declarations are
associated with their containing trait/impl IDs but are not inserted into
module value scope. There is no dedicated parallel method-callable map.
Detached and standard checked declarations are not `ProjectDeclarationId`s and
cannot enter this constructor by type.

A trait declaration is available to the trait-bound resolver through the same
project scope/import/reexport binding infrastructure. A trait alias stores the
original `TraitDeclarationId`; it does not make a new trait or method ID. An
impl has no source-visible name and is stored by `ImplDeclarationId` only.

### Visibility

```rust
pub enum CallableAccess {
    Direct(Option<Visibility>),
    TraitRequirement {
        trait_declaration: TraitDeclarationId,
        trait_visibility: Option<Visibility>,
    },
    TraitImplementation,
    InherentMethod {
        owner_module: CanonicalModulePath,
    },
    Standard,
}
```

- Trait requirement access follows the trait declaration.
- A trait impl method has no independent source-visible access. Resolution can
  reach it only through a selected `TraitMethodConformance`; that conformance's
  requirement access and target-type/witness validity are checked before the
  implementation candidate is committed. This remains correct when one impl
  method satisfies several inherited requirements.
- Inherent methods follow the current owner-module rule. The currently rejected
  `pub impl` surface is not restored.
- Standard methods use the standard access policy.

## 5. Sole checked record owner

```rust
pub struct CheckedCallableCatalog {
    records: BTreeMap<CheckedCallableId, CheckedCallableFacts>,
    conformances: BTreeMap<TraitMethodConformanceId, TraitMethodConformance>,
    closure_rows: BTreeMap<CheckedClosureId, EffectRow>,
}

pub struct CheckedCallableFacts {
    id: CheckedCallableId,
    signature: CallableSignatureSchema,
    source: CallableSource,
    access: CallableAccess,
    execution: CheckedCallableExecution,
    effects: CheckedCallableEffects,
}

pub enum CheckedCallableExecution {
    Runtime(CallableExecutionMode),
    DispatchContract,
}

pub enum CheckedCallableEffects {
    Body {
        contract: CallableEffectContract,
        inferred: EffectRow,
    },
    BodylessTraitRequirement {
        contract: CallableEffectContract,
    },
    ExternalOrStandard {
        exposed: EffectRow,
    },
}
```

Fields are private. Final-record constructors are `pub(crate)` and callable only
from the checked-catalog builder after source, signature, and row validation.
The final catalog is immutable and shared as `Arc<CheckedCallableCatalog>`.

### Required inherent accessors

```rust
CheckedCallableFacts::id()
CheckedCallableFacts::signature()
CheckedCallableFacts::source()
CheckedCallableFacts::access()
CheckedCallableFacts::execution()
CheckedCallableFacts::actual_row() -> Option<&EffectRow>
CheckedCallableFacts::exposed_row() -> &EffectRow
CheckedCallableCatalog::callable(&CheckedCallableId)
CheckedCallableCatalog::conformance(&TraitMethodConformanceId)
CheckedCallableCatalog::method_dispatch_row(...)
```

`exposed_row()` is computed by the record; callers do not reproduce its branch.

## 6. Source owner

```rust
pub enum CallableSource {
    Authored(CallableAuthoredSource),
    Standard(StandardCallableSource),
}

pub struct CallableAuthoredSource {
    declaration: SourceSpan,
    name: SourceSpan,
    signature: SourceSpan,
    effect_contract: EffectContractSource,
    body: Option<SourceSpan>,
}
```

Every authored span is tied to the exact `SourceDocumentIdentity`. Source
conversion happens once when syntax/HIR ranges join a project source document.
Sema, diagnostics, project index, CLI, and LSP do not recreate spans from
line/column or text.

## 7. Identity preservation by call form

| Form | Identity retained | Row source |
|---|---|---|
| direct ordinary call | project/detached/standard checked declaration in `CheckedCallableId` | target record exposed row |
| concrete trait call | impl method checked ID + conformance ID | impl record exposed row |
| inherent call | inherent method checked ID | inherent record exposed row |
| generic static witness call | original requirement checked ID + witness | requirement record exposed row after substitution |
| trait alias/reexport | original target checked ID | same record |
| inherited requirement | original declaring requirement checked ID | same requirement record |
| bound method value | `BoundMethodTarget` checked IDs | one latent substituted row |
| non-final curry | same checked ID and next group | latent row only |
| final curry | same checked ID | latent exposed row applied |
| closure | typed closure ID with source span | closure row in same catalog |
| compiler trait-method lowering | `CheckedCallableId`/conformance in compiler-only typed key | catalog row already consumed before lowering |
| runtime trait-method execution | opaque general `RuntimeCallableId` projection + plan-local `RuntimeTraitMethodId` | no runtime row lookup or source resolution |

## 8. Trait catalog changes

`TraitCatalog` retains its compact vectors and local handles as private
storage, but changes its indices and resolution results:

- `by_name: BTreeMap<String, TraitId>` is not public authority. Project-aware
  lookup resolves a typed `TraitDeclarationId` first, then maps it to `TraitId`.
- `TraitDecl` stores its `TraitDeclarationId`.
- `TraitImpl` stores its `ImplDeclarationId`.
- `TraitMethodRequirement` and `TraitMethodImpl` store checked callable IDs.
- `TraitMethodResolution` returns IDs/references; it does not clone a method
  record as an effect owner.
- `TraitMethodCandidate` contains typed declaration IDs so two same-named traits
  remain distinguishable.

## 9. Resolver/schema changes

The current shared callable identity/schema owners migrate as follows:

| Current owner | Final owner |
|---|---|
| `TraitCallableId { trait_name, method, implementation, source }` | deleted; use `CheckedCallableId` plus conformance/witness |
| `CallableCandidateId::TraitMethod(TraitCallableId)` | ID-bearing source/method candidate using `CheckedCallableId` |
| `CallableValidator::Trait(TraitCallableId)` | typed method validation payload containing conformance/witness IDs only |
| `CallableEffectSchema::Project { declaration, declared }` | ID-only checked schema; row queried from catalog |
| resolver-created empty effect row | deleted |
| requirement synthesized as `TraitMethodImpl` | deleted |

## 10. Project index and tooling ownership

`ProjectCallableSymbol` changes its declaration field to `CheckedCallableId`
(project context only). It retains signature/source/hash metadata and gains
method kinds, but no row field. `ProjectSemanticIndex` and `TypeCheckReport`
share the same checked catalog.

A tooling query is:

```text
request snapshot
  -> project/source revision validation
  -> CheckedCallableId
  -> CheckedCallableCatalog record
  -> signature / exposed row / source / conformance
  -> hover, signature help, navigation, diagnostic
```

There is no spelling-based method reconstruction or effect copy.

## 11. Compiler/runtime projection is one-way and non-authoritative

The existing general `arcweft_core::entry::RuntimeCallableId` is reused. No
trait-only runtime string ID is added. A checked method becomes a runtime ID by
one domain-separated digest projection:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedCallableDigest([u8; 32]);

impl CheckedCallableDigest {
    pub const fn as_bytes(&self) -> &[u8; 32];
    pub const fn into_bytes(self) -> [u8; 32];
}

impl CheckedCallableId {
    pub fn semantic_digest(&self) -> CheckedCallableDigest;
}

impl RuntimeCallableId {
    pub fn from_checked_digest(digest: [u8; 32]) -> Self;
}
```

Both methods are implemented on the original owning types. The runtime value's
canonical spelling is exactly
`arcweft.checked.v1.<64-lowercase-hex-digest>`. The existing runtime identity
validator remains the storage validator; runtime does not parse the spelling
back into declaration components.

`CheckedCallableId::semantic_digest()` is BLAKE3 over these exact bytes:

```text
"arcweft.checked-callable.v1\0"
context-tag:u8
context-payload
declaration-tag:u8
declaration-payload
```

Encoding rules are fixed: UTF-8 strings and segment vectors use a `u32` little-
endian byte/count prefix; integers are little-endian; enum tags use the order
listed below; source and source-set revisions contribute their raw 32 bytes.

Context tags/payloads:

1. `0 Project`: package string, root document ID string, profile string,
   `ProjectSymbolRevision` raw 32-byte source-set revision.
2. `1 Detached`: document ID string, source revision raw 32 bytes,
   `source_len: u64`.
3. `2 Standard`: `StandardTraitCatalogVersion: u32`.

Declaration tags/payloads for `CheckedCallableDeclaration`:

1. `0 ProjectExisting`: package, canonical module segment vector, explicit
   `CallableDeclarationOwner` tag, owner-path segment vector, name.
2. `1 ProjectTraitRequirement`: trait package/canonical-module-segments/name,
   method.
3. `2 ProjectImplMethod`: impl package/canonical-module-segments/
   `source_ordinal: u32`, `ImplMethodKind` tag (`0 Trait`, `1 Inherent`), method.
4. `3 Detached`: `CallableDeclarationOwner` tag and detached unified
   `source_ordinal: u32`; the detached source identity is encoded by context.
5. `4 Standard`: `CallableDeclarationOwner` tag and standard
   `catalog_ordinal: u32`; the catalog version is encoded by context.

`CallableDeclarationOwner` digest tags are explicit and frozen in its inherent
impl: `0 Function`, `1 ExternCapability`, `2 View`, `3 Predicate`, `4 Proof`,
`5 TraitRequirement`, `6 TraitImplementation`, `7 InherentMethod`.
`CanonicalModulePath` contributes only its validated segment vector; no source
spelling or `Debug` text enters the digest.

The final runtime-plan types are:

```rust
pub struct RuntimeTraitMethodIdentity {
    implementation: RuntimeCallableId,
    requirement: Option<RuntimeCallableId>,
}

pub struct RuntimeTraitMethodInventory {
    methods: Vec<RuntimeTraitMethod>,
}

pub(crate) struct RuntimeTraitMethodLoweringIndex {
    by_conformance: BTreeMap<TraitMethodConformanceId, RuntimeTraitMethodId>,
    by_inherent: BTreeMap<CheckedCallableId, RuntimeTraitMethodId>,
}

pub enum ForIterationEvidenceFamily {
    Builtin(StandardIteratorFamily),
    Witness {
        into_iterator: TraitMethodConformanceId,
        iterator: TraitMethodConformanceId,
    },
    IteratorWitness {
        iterator: TraitMethodConformanceId,
    },
    WitnessUnsupported {
        reason: String,
    },
}
```

The standard trait catalog publishes typed requirement IDs for
`IntoIterator::into_iter` and `Iterator::next`; `checker/iterator.rs` resolves
those requirements to method conformance IDs while building iteration evidence.
It never stores only a trait witness and later supplies a method-name string.
The conformance ID pair is available after signature/witness resolution; if its
effect subset later fails, the report is invalid and compiler lowering is not
entered.

The compiler sorts lower inputs by the typed tuple
`(implementation CheckedCallableId, requirement Option<CheckedCallableId>)`
before assigning plan-local `RuntimeTraitMethodId`s. One conformance produces
one runtime method body in this cut; generic/self/effect substitutions are
applied before lowering and are not reconstructed at runtime. The compiler-only
index translates exact sema evidence to direct emitted IDs and is discarded
after plan construction.

Current consumers migrate exactly:

| Current consumer | Final contract |
|---|---|
| `arcweft-lang-sema/src/checker/iterator.rs` / `ForIterationEvidenceFamily` | retain exact method conformance IDs instead of trait-witness IDs that require later method-name lookup |
| `arcweft-compiler/src/trait_methods.rs::trait_method_input` | consumes `TraitMethodConformanceId`/checked method IDs and builds the two runtime projections; no `format!("{:?}")`, witness index, or method-name identity |
| `lower_runtime_trait_methods_from_typecheck` | returns inventory plus compiler-only typed lowering index |
| `runtime_witness_evidence` / `runtime_iterator_identity_witness_evidence` | query the typed lowering index and place direct `RuntimeTraitMethodId`s in runtime evidence |
| `arcweft-runtime-plan/src/trait_methods.rs::RuntimeTraitMethodInventory` | method vector only; no `(usize, String)` lookup map |
| `arcweft-core/src/plan.rs::RuntimeTraitMethodIdentity` | implementation projection plus optional requirement projection only |

When a compiler/runtime log requests a display label, the compiler derives it
from the checked catalog. It is not serialized as identity and is never
accepted by a resolver. The directly owned runtime-plan schema/fingerprint is
updated atomically for the new shape; no old identity reader or compatibility
decoder remains.

## 12. Missing, stale, private, ambiguous, and rollback behavior

- **Missing:** typed declaration lookup returns `Missing`; no effect edge.
- **Ambiguous:** all typed candidates are returned in deterministic ID order;
  no first-name fallback.
- **Private:** visibility failure precedes candidate commit; no row is exposed.
- **Stale:** context mismatch (`world`, revision, document identity, or standard
  version) returns `StaleCallableIdentity`; a row is never read.
- **Rollback:** builder journals keyed records, inferred tails, edges, and
  conformances. Rollback removes all post-checkpoint mutations and restores the
  previous current callable and effect-var supply. Only the frozen committed
  catalog is published.

These behaviors are tested through typed APIs and transaction outcomes, not by
scanning source files.
