# Combined final contract

This file is a review convenience. The split normative files remain authoritative and are indexed by `README.md`.


<!-- BEGIN README.md -->

# Accepted structural/nominal runtime carrier — final design contract

This ZIP is a **design-only** return for `2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction(1).md`. It contains no production overlay and makes no commit to the repository.

## Fixed evidence basis

- Repository: `Sanzentyo/arcweft`
- Basis ref: `origin/main`
- Complete Git SHA actually used: `UNAVAILABLE`
- Git decorations: `UNAVAILABLE`
- Working tree status after checkout: `(clean/no status output)`
- Repository acquired successfully: `false`
- Root/latest-main AGENTS files read in full: (none found / repository unavailable)
- Request SHA-256: `e9ead183b2bfd4d3019e8c3e51da79136bdae64d38aa5fe63ec4c92c1c948269`
- Premise SHA-256: `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1`
- Rust Skill SHA-256: `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`


## Normative result

The design introduces one runtime authority, `AcceptedRuntimeCarrier`, represented by an enum owned by the crate that already owns the runtime value/carrier enum. If current source already has that enum under another name, the implementation **extends that enum and its inherent `impl`**; it must not add an ad-hoc wrapper, extension trait, or side table merely to avoid editing the owner.

The carrier has exactly two semantic classes:

1. `Structural`: carries the canonical structural shape identity required by checked matching.
2. `Nominal`: carries the canonical nominal instance identity **and** its validated structural representation identity.

Nominal identity is never reconstructed from layout. Structural access to nominal representation is allowed only when checked lowering emitted an explicit projection witness. The same checked constraint is consumed by runtime execution, coverage closure, transcript production, persistence, and restore.

## Package map

- `01-evidence-basis.md` — current-main SHA, AGENTS scope, source anchors, and input hashes.
- `02-normative-decisions.md` — decisions D1–D24 and rejection of competing designs.
- `03-rust-api-and-owner-map.md` — concrete Rust types, inherent methods, owner/module map, and error taxonomy.
- `04-match-admission-and-coverage.md` — complete structural/nominal matrix and transcript/coverage closure.
- `05-persistence-byte-grammar-and-restore.md` — canonical grammar and two-phase restore.
- `06-runtime-task-awbc-integration.md` — task, Need, handle-batch, and allocation integration.
- `07-test-matrix.md` — executable test rows T1–T32.
- `08-implementation-sequence.md` — file-level change order and admission gates.
- `09-requirement-traceability.md` — request rows mapped 1:1 to concrete decisions/tests.
- `10-verification-boundary.md` — what was and was not actually run.
- `api-sketches/*.rs.txt` — non-production API sketches.
- `evidence/` — source search results, AGENTS copies, acquisition log, and validation logs.

## Closure state

`OPEN_QUESTIONS = 0`. Any name whose exact current-main spelling could not be proven is explicitly labeled **proposed**, while its semantic owner and migration rule are fixed. There are no generic `CLOSED` placeholders.


<!-- END README.md -->


<!-- BEGIN 01-evidence-basis.md -->

# 01. Evidence basis

- Repository: `Sanzentyo/arcweft`
- Basis ref: `origin/main`
- Complete Git SHA actually used: `UNAVAILABLE`
- Git decorations: `UNAVAILABLE`
- Working tree status after checkout: `(clean/no status output)`
- Repository acquired successfully: `false`
- Root/latest-main AGENTS files read in full: (none found / repository unavailable)
- Request SHA-256: `e9ead183b2bfd4d3019e8c3e51da79136bdae64d38aa5fe63ec4c92c1c948269`
- Premise SHA-256: `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1`
- Rust Skill SHA-256: `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`

## Current-main owner anchors

| Concern | Current source anchor selected by symbol search | Status |
|---|---|---|
| runtime value/carrier owner | `crates/<runtime-owner>/src/value.rs (new module only if no owning enum exists)` | observed candidate or explicit proposed fallback |
| checked match executor/plan | `crates/<runtime-owner>/src/match_exec.rs` | observed candidate or explicit proposed fallback |
| checked type/nominal owner | `crates/<language-owner>/src/checked/type.rs` | observed candidate or explicit proposed fallback |
| snapshot/restore owner | `crates/<runtime-owner>/src/snapshot.rs` | observed candidate or explicit proposed fallback |
| task/Need/handle owner | `crates/<runtime-owner>/src/task.rs` | observed candidate or explicit proposed fallback |

The exact grep rows are retained in `evidence/source-search-results.md`; this table does not silently promote a proposed fallback into an observed path.

## Workspace packages observed

(cargo metadata unavailable; see validation logs)

## Request headings read

- Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1 — accepted structural nominal runtime carrier correction
-   Parent, split reason, and precedence
-   Mandatory redispatch inputs and repository preflight
-   Decisions required
-   Consumers to inventory
-   Non-goals
-   Required implementation order
-   Required tests
-   Required returned archive

## Evidence discipline

The request document is a requirement source, not evidence that current source already implements it. Source observations in this package come only from the checked-out SHA and command logs. Proposed API names are marked as such.


<!-- END 01-evidence-basis.md -->


<!-- BEGIN 02-normative-decisions.md -->

# 02. Normative decisions

## Runtime carrier identity

**D1 — One authority.** There is exactly one accepted runtime carrier enum in the runtime-value owner crate. Existing owner enum: modify it directly. No mirror enum, extension trait, `HashMap<Handle, TypeFact>`, or match-local reconstruction is permitted.

**D2 — Closed semantic variants.** The enum has two semantic variants, `Structural` and `Nominal`. Built-ins, tuples, records, sequences, views, and other runtime-representable values are classified through their checked structural shape; language-declared/newtype/class-like identities use `Nominal` when the checked program requires nominal identity.

**D3 — Structural payload.** A structural carrier stores a canonical runtime-interned structural shape key and the payload handle/reference already owned by the runtime value system. Field names/order/kinds are not recopied into each value.

**D4 — Nominal payload.** A nominal carrier stores a canonical nominal instance key: stable declaration identity, defining catalog digest/domain, and canonical generic arguments. It also stores the already-validated structural representation shape key needed for destructuring. Two nominal instances with identical representation remain unequal.

**D5 — No layout inference.** Runtime code may not infer nominal identity from a shape, field list, Rust type, discriminant, vtable address, allocation address, or debug name.

**D6 — Seal-on-construction.** Construction validates all cross-links and returns a sealed immutable carrier. Mutation after publication is impossible; sharing uses the repository's existing immutable/arena handle model.

## Admission and projection

**D7 — Checked constraint authority.** Checked lowering emits `AcceptedCarrierConstraint` for every runtime match root. Runtime matching consumes it and performs no semantic type checking.

**D8 — Structural-on-structural.** Admission requires canonical shape compatibility defined by the checked plan. It is not ad-hoc Rust structural equality.

**D9 — Nominal-on-nominal.** Admission requires exact canonical nominal instance equality, including generic arguments and catalog domain. Representation equality alone is insufficient.

**D10 — Structural projection of nominal.** A structural pattern may inspect a nominal representation only when the checked plan contains a `StructuralProjectionWitness` naming the accepted source nominal instance, target shape, projection mode, and validation digest. Absence of the witness is a deterministic rejection, not a fallback attempt.

**D11 — Nominal pattern never accepts structural-only carrier.** A structural carrier has no authority to synthesize a nominal identity, even if its shape is byte-for-byte equal to the nominal representation.

**D12 — Shared plan for execution and proof.** Coverage closure, complete transcript generation, arm selection, and runtime execution use the same normalized constraint/projection records. A second independently-normalized domain is forbidden.

## Persistence and restore

**D13 — Stable keys on wire.** Persistence encodes stable declaration/type/shape keys and canonical argument encodings. Raw process-local interner indices, arena slots, pointers, and hash-map iteration order never enter bytes.

**D14 — Versioned canonical grammar.** Carrier bytes have a format version and variant tag. Integer encodings are canonical unsigned LEB128 (or the repository's already-established canonical varint if one exists); map-like collections are sorted by stable key before emission; duplicate entries are rejected.

**D15 — Two-phase restore.** Phase A decodes unresolved stable records and validates local byte invariants. Phase B resolves catalog/type/shape/payload references, validates nominal-to-representation agreement, then atomically publishes. No task sees a partially restored carrier.

**D16 — Snapshot isomorphism.** Live carrier → canonical snapshot → restored live carrier preserves semantic equality and match results. Re-encoding a restored snapshot produces identical bytes for the same format version.

## Task, Need, handles, and allocation

**D17 — Need identity is orthogonal.** `Need`/producer/task identity is not part of type-carrier equality. The task input points to an immutable carrier; multiple Need instances may share it without aliasing identities.

**D18 — Coordinator publication.** Batch/snapshot restore stages all carrier resolutions before the runtime task coordinator publishes handles. A single failed resolution aborts the batch and leaves the prior world unchanged.

**D19 — AWBC/arena ownership.** Metadata is interned once in the repository's canonical metadata owner; payloads remain arena/AWBC-owned through existing handles. The carrier does not clone aggregate payloads merely to carry type facts.

**D20 — Bounded hot path.** Match admission performs variant test plus interned-key comparisons; projection executes precompiled slot steps. There is no field-name lookup, catalog traversal, allocation, or hashing of full shapes in the arm-selection hot path.

## Errors and observability

**D21 — Typed construction/restore errors.** Unknown stable key, catalog mismatch, generic-argument mismatch, representation mismatch, stale projection witness, duplicate encoding, noncanonical varint, unsupported version, and dangling payload reference are distinct typed errors.

**D22 — Match rejection is not restore corruption.** Ordinary constraint mismatch yields a non-error `Rejected` match outcome. Broken sealed invariants or corrupt persisted bytes yield typed errors and never masquerade as an unmatched arm.

**D23 — Transcript completeness.** Each attempted root records carrier class, constraint class, stable diagnostic identity, witness presence/identity, and final outcome. It does not expose process-local addresses.

**D24 — Compatibility.** The format/version gate rejects unknown major versions. Additive optional fields require an explicitly versioned presence bitmap/TLV rule; readers do not silently ignore unknown identity-bearing fields.

## Rejected alternatives

- **Shape-only nominal recovery:** rejected because equal layouts do not imply equal nominal identity.
- **Side table keyed by value handle:** rejected because snapshot/restore and handle reuse can desynchronize it.
- **Extension trait/helper around an arcweft-owned enum:** rejected because behavior belongs on the original enum's inherent implementation.
- **Runtime re-type-checking:** rejected because it duplicates compiler semantics and can diverge from coverage.
- **Serialize process-local IDs:** rejected because it breaks deterministic restore and cross-process snapshots.
- **Publish while resolving:** rejected because failure would expose a partially restored task graph.


<!-- END 02-normative-decisions.md -->


<!-- BEGIN 03-rust-api-and-owner-map.md -->

# 03. Rust API and owner map

## Concrete ownership map

| API/behavior | Owner on current source evidence | Required change |
|---|---|---|
| accepted carrier enum and inherent behavior | `crates/<runtime-owner>/src/value.rs (new module only if no owning enum exists)` | extend the existing owning enum/impl; only create the proposed module when no such owner exists |
| checked carrier constraint emission | `crates/<language-owner>/src/checked/type.rs` | emit normalized stable keys and projection witness |
| runtime match execution | `crates/<runtime-owner>/src/match_exec.rs` | consume the checked constraint; remove shape-to-nominal inference/fallback |
| snapshot codec / restore resolver | `crates/<runtime-owner>/src/snapshot.rs` | add canonical carrier record and two-phase resolution |
| task/Need publication | `crates/<runtime-owner>/src/task.rs` | stage resolved carrier handles and publish atomically |

## Proposed API (normative semantics; spelling may be adapted to an existing owner enum)

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AcceptedRuntimeCarrier {
    Structural(StructuralRuntimeCarrier),
    Nominal(NominalRuntimeCarrier),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StructuralRuntimeCarrier {
    pub shape: RuntimeStructuralShapeId,
    pub payload: RuntimeValueHandle,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct NominalRuntimeCarrier {
    pub instance: RuntimeNominalInstanceId,
    pub representation: RuntimeStructuralShapeId,
    pub payload: RuntimeValueHandle,
}
```

`RuntimeStructuralShapeId`, `RuntimeNominalInstanceId`, and `RuntimeValueHandle` above are semantic roles. Reuse current-main canonical identifiers when they already exist; do not introduce a parallel ID family. `RuntimeNominalInstanceId` interns the tuple `(catalog_domain, stable_nominal_def, canonical_generic_args)`.

## Inherent methods on the owner enum

```rust
impl AcceptedRuntimeCarrier {
    pub fn structural(
        shape: RuntimeStructuralShapeId,
        payload: RuntimeValueHandle,
        catalog: &RuntimeTypeCatalog,
        values: &RuntimeValueArena,
    ) -> Result<Self, CarrierBuildError>;

    pub fn nominal(
        instance: RuntimeNominalInstanceId,
        representation: RuntimeStructuralShapeId,
        payload: RuntimeValueHandle,
        catalog: &RuntimeTypeCatalog,
        values: &RuntimeValueArena,
    ) -> Result<Self, CarrierBuildError>;

    pub fn class(&self) -> AcceptedCarrierClass;
    pub fn payload(&self) -> RuntimeValueHandle;
    pub fn representation_shape(&self) -> RuntimeStructuralShapeId;
    pub fn nominal_instance(&self) -> Option<RuntimeNominalInstanceId>;

    pub fn admit(
        &self,
        constraint: &AcceptedCarrierConstraint,
        catalog: &RuntimeTypeCatalog,
    ) -> Result<CarrierAdmission, CarrierInvariantError>;
}
```

The constructors are the only public route to a sealed carrier. Fields may be `pub(crate)` or private according to the owner crate's conventions. `admit` is inherent behavior, not an extension trait.

## Checked plan

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AcceptedCarrierConstraint {
    Structural {
        target: RuntimeStructuralShapeId,
        projection: StructuralProjectionPolicy,
    },
    Nominal {
        target: RuntimeNominalInstanceId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum StructuralProjectionPolicy {
    Direct,
    FromNominal(StructuralProjectionWitnessId),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StructuralProjectionWitness {
    pub source: RuntimeNominalInstanceId,
    pub target: RuntimeStructuralShapeId,
    pub steps: Box<[ProjectionStep]>,
    pub semantic_digest: ProjectionSemanticDigest,
}

pub enum CarrierAdmission {
    Accepted(AcceptedProjection),
    Rejected(CarrierMismatch),
}
```

`Direct` is valid only for a structural carrier. `FromNominal` is valid only for the exact nominal source named by the witness. `AcceptedProjection` contains prevalidated slot/index operations and never field-name lookup.

## Error taxonomy

```rust
pub enum CarrierBuildError {
    UnknownShape(RuntimeStructuralShapeId),
    UnknownNominalInstance(RuntimeNominalInstanceId),
    PayloadShapeMismatch { expected: RuntimeStructuralShapeId, actual: RuntimeStructuralShapeId },
    NominalRepresentationMismatch { instance: RuntimeNominalInstanceId, declared: RuntimeStructuralShapeId, supplied: RuntimeStructuralShapeId },
    DanglingPayload(RuntimeValueHandle),
}

pub enum CarrierRestoreError {
    UnsupportedVersion(u16),
    NonCanonicalInteger,
    DuplicateField(u32),
    UnknownCatalogDomain(StableCatalogKey),
    UnknownShape(StableShapeKey),
    UnknownNominal(StableNominalKey),
    GenericArgumentMismatch,
    RepresentationMismatch,
    StaleProjectionWitness,
    DanglingPayload(StablePayloadKey),
    TrailingBytes,
}
```

Use existing error aggregation conventions if current source already owns a broader enum; add these variants to that enum's original `impl`/conversion path rather than introducing a private catch-all string error.

## Ownership and borrowing

- Carrier metadata is immutable and interned; clone of a carrier clones small IDs/handles, not payload storage.
- Match execution borrows `&AcceptedRuntimeCarrier` and `&AcceptedCarrierConstraint`.
- Projection returns borrowed/arena handles under the current runtime lifetime model; it never returns a reference manufactured from an unlocked arena.
- Persisted stable keys are separate wire structs, so no `Serialize` derive is placed directly on process-local IDs.
- `unsafe` is neither required nor permitted by this design.


<!-- END 03-rust-api-and-owner-map.md -->


<!-- BEGIN 04-match-admission-and-coverage.md -->

# 04. Match admission and coverage closure

## Complete admission matrix

| Subject carrier | Constraint | Additional checked witness | Result |
|---|---|---|---|
| Structural `S` | Structural `S` | `Direct` | accept, identity projection |
| Structural `S1` | Structural `S2` | any | reject unless checked shape-compatibility canonicalizes both to the same accepted shape |
| Structural `S` | Nominal `N` | none possible | reject; structural data cannot synthesize nominal identity |
| Nominal `N` repr `S` | Nominal same `N` | none | accept |
| Nominal `N<A>` repr `S` | Nominal `N<B>` | none | reject when canonical generic args differ |
| Nominal `N1` repr `S` | Nominal `N2` repr `S` | none | reject even though representation is equal |
| Nominal `N` repr `S` | Structural `S` | valid witness `(N → S)` | accept and execute precompiled projection |
| Nominal `N` repr `S` | Structural `S` | absent, stale, or names another source | reject (stale invariant is an error during plan/restore validation) |
| Nominal `N` repr `S1` | Structural `S2` | witness for `S2` | accept only when the witness was validated against current catalog digest and steps |

## Runtime algorithm

1. Read the sealed carrier variant and checked constraint variant.
2. Compare interned stable semantic IDs; never enumerate fields to decide nominal identity.
3. For direct structural admission, verify the canonical accepted shape ID.
4. For nominal structural projection, resolve the witness ID, verify source/target/digest, and return its projection steps.
5. Return `Rejected` for ordinary domain mismatch; return a typed invariant error only when sealed data is internally inconsistent.
6. Execute arm tests against the admitted projection.
7. Append the deterministic transcript row from stable diagnostic keys and outcome.

## Coverage and transcript closure

The checked compiler creates a single normalized `AcceptedCarrierConstraint` table. Both static coverage and runtime arm selection refer to table indices/digests. The coverage certificate contains:

- subject accepted-domain key,
- ordered arm constraint keys,
- projection-witness keys,
- uncovered-domain proof or exhaustiveness marker,
- semantic digest over the normalized table.

At load/restore, runtime validates that the match plan and coverage/transcript table share the same semantic digest. This prevents a complete static transcript from being paired with a different runtime admission domain.

## Alias/newtype rules

- A transparent type alias is normalized by checked typing before carrier construction; it does not create a fresh nominal identity.
- A nominal/newtype declaration creates a distinct nominal instance even when its representation is identical.
- Representation transparency controls whether a structural projection witness can be emitted; it never controls nominal equality.
- Opaque/external nominal values may carry a nominal identity while refusing structural projection. Their representation shape is the opaque runtime carrier shape required for storage, not permission to destructure.


<!-- END 04-match-admission-and-coverage.md -->


<!-- BEGIN 05-persistence-byte-grammar-and-restore.md -->

# 05. Persistence byte grammar and two-phase restore

## Canonical record

The carrier record is embedded through the repository's existing snapshot framing. The following is the normative semantic grammar; existing canonical integer and digest primitives should be reused verbatim.

```text
accepted_runtime_carrier :=
    format_version:u16le
    variant:u8
    flags:u8
    body_len:canonical_uvarint
    body:bytes[body_len]

variant 0x00 (structural) body :=
    stable_shape_key:stable_key
    stable_payload_key:stable_key

variant 0x01 (nominal) body :=
    stable_catalog_domain:stable_key
    stable_nominal_def:stable_key
    generic_arg_count:canonical_uvarint
    generic_args:stable_type_key[generic_arg_count]
    stable_representation_shape:stable_key
    stable_payload_key:stable_key

stable_key := key_kind:u8 || key_len:canonical_uvarint || key_bytes[key_len]
```

Constraints:

- `format_version = 1` for the first admitted format.
- Reserved flags must be zero; nonzero unknown identity-bearing flags are rejected.
- `body_len` must be minimal/canonical and exactly consumed.
- Generic arguments appear in declaration order, not hash-map order.
- Stable key bytes use the repository's catalog/type digest representation; never debug strings.
- Payload bytes live in the snapshot's payload/value table and are referenced once by stable payload key.
- The carrier record has no raw `Runtime*Id`, arena slot, pointer, `usize`, or platform-endian integer.

## Phase A — decode and local validation

Decode into wire-only records:

```rust
pub enum UnresolvedAcceptedCarrier {
    Structural { shape: StableShapeKey, payload: StablePayloadKey },
    Nominal {
        catalog: StableCatalogKey,
        definition: StableNominalKey,
        generic_args: Box<[StableTypeKey]>,
        representation: StableShapeKey,
        payload: StablePayloadKey,
    },
}
```

Phase A checks version/tag/flags, canonical integers, lengths, duplicates, key syntax, allocation bounds, trailing bytes, and aggregate resource limits. It does not publish runtime handles.

## Phase B — resolve, validate, seal

Resolution order is deterministic:

1. Resolve catalog domain and verify catalog digest/version contract.
2. Resolve every generic type key.
3. Intern/resolve the nominal instance key.
4. Resolve the structural representation shape.
5. Verify the catalog-declared representation of the nominal instance equals the encoded shape.
6. Resolve payload and verify its actual shape/representation.
7. Re-resolve projection witness references used by restored match plans and verify semantic digests.
8. Construct the carrier through the same checked constructor used for live values.
9. Add the sealed carrier to the staged batch.
10. Publish all carrier/value/task handles atomically only after the entire batch succeeds.

## Failure and rollback

A failure drops the unresolved/staged batch. It does not mutate the live interner in an externally visible way, bind any task handle, wake a Need waiter, or emit a successful transcript. Interning implementations that cannot roll back may retain unreachable canonical metadata internally, but publication roots and observable handle tables remain unchanged.

## Isomorphism requirements

For valid `x`:

```text
semantic_eq(resolve(decode(encode(x))), x) == true
encode(resolve(decode(encode(x)))) == encode(x)
match(resolve(decode(encode(x))), plan) == match(x, plan)
```

For invalid/noncanonical bytes, decode or resolve returns a typed error before publication.


<!-- END 05-persistence-byte-grammar-and-restore.md -->


<!-- BEGIN 06-runtime-task-awbc-integration.md -->

# 06. Runtime task, Need, handle-batch, and AWBC integration

## Data flow

```text
checked type + accepted match domain
        │
        ├─ emit nominal/structural stable keys
        ├─ emit optional structural projection witness
        ▼
sealed AcceptedRuntimeCarrier ──► immutable task-plan input
        │                               │
        │                               ├─ runtime match executor
        │                               ├─ transcript/coverage digest check
        │                               └─ snapshot encoder
        ▼
staged restore carrier/value batch ──► coordinator atomic publish ──► Need/task wakeup
```

## Task-plan rules

- A task plan references a carrier-table entry by the plan's existing canonical child/reference mechanism.
- Sealing the plan verifies that every carrier constraint and projection witness is reachable and digest-consistent.
- Semantic child encoding includes stable carrier/witness keys in deterministic order.
- Carrier metadata is not lazily synthesized by the worker that first executes a match.

## Runtime handle/batch rules

- Live handles remain process-local and are allocated only after stable references resolve.
- Batch order does not affect semantic IDs or snapshot bytes.
- A handle batch contains separate staged tables for payload values, carrier metadata, match plans/witnesses, and tasks, with an explicit dependency order.
- The publish barrier installs roots only when all tables pass validation.

## Need rules

- `Need<T>` identity denotes temporal production of `T`; it is not a type identity for `T`.
- A `Need` that yields a nominal value carries that value's sealed nominal carrier when the value becomes available.
- Cancellation/failure of the producer cannot mutate shared carrier metadata.
- Restoring a waiting Need resolves its value/carrier references before registering waiters or wakeups.

## AWBC/allocation rules

- Use the existing AWBC/value arena for payload ownership and current canonical metadata interner for small immutable carrier facts.
- The match hot path performs no allocation.
- Snapshot decode applies explicit count/byte limits before allocating boxed slices.
- Generic arguments and projection steps are allocated once as boxed slices/interned entries, not once per arm attempt.
- Do not box the whole carrier enum solely to suppress enum-size lint; box only genuinely variable/large collections according to measured layout and existing repository policy.

## Concurrency invariants

1. Published carriers are immutable.
2. A payload handle cannot be observed without its validated carrier in the same published generation.
3. Restored plan/witness and carrier catalogs are generation-consistent.
4. Task wakeup happens after the publication release barrier; readers acquire before dereference.
5. Transcript ordering follows the task/match execution authority already established by the coordinator, not hash-map iteration.


<!-- END 06-runtime-task-awbc-integration.md -->


<!-- BEGIN 07-test-matrix.md -->

# 07. Test matrix

Every row names the layer, fixture, expected assertion, and regression caught. Test names are proposed and should be placed next to the actual owner modules discovered in `01-evidence-basis.md`.

| ID | Layer | Fixture / action | Required assertion |
|---|---|---|---|
| T1 | unit/carrier | construct structural carrier with matching shape/payload | sealed carrier, correct class/shape/payload |
| T2 | unit/carrier | structural constructor with mismatched payload shape | exact `PayloadShapeMismatch` |
| T3 | unit/carrier | construct nominal with declared representation | sealed nominal, exact instance and representation |
| T4 | unit/carrier | nominal with equal-looking but undeclared representation | exact `NominalRepresentationMismatch` |
| T5 | unit/carrier | dangling payload handle | exact `DanglingPayload` |
| T6 | match | structural S vs structural S/Direct | accepted identity projection |
| T7 | match | structural S vs nominal N | rejected, no nominal synthesis |
| T8 | match | nominal N vs same N | accepted |
| T9 | match | nominal N1 vs N2 with same shape | rejected |
| T10 | match | nominal N<A> vs N<B> | rejected by canonical generic args |
| T11 | match | nominal N repr S vs structural S with valid witness | accepted, expected projection steps |
| T12 | match | same without witness | rejected |
| T13 | match | witness names another nominal source | typed stale/invariant error at validation, never arm fallback |
| T14 | match | opaque nominal vs structural pattern | rejected unless contract explicitly emits witness |
| T15 | alias | transparent alias and canonical target | same structural/nominal key after checked normalization |
| T16 | newtype | two newtypes with identical representation | distinct nominal instance keys |
| T17 | coverage | static constraint table and runtime plan | identical semantic digest |
| T18 | coverage | mutate one witness/constraint in serialized fixture | load rejects digest mismatch |
| T19 | transcript | every admission matrix row | stable class/constraint/witness/outcome fields, no pointer data |
| T20 | codec | encode structural carrier golden vector | exact canonical bytes |
| T21 | codec | encode generic nominal carrier golden vector | exact canonical bytes and declaration-order args |
| T22 | codec | nonminimal varint / unknown flags / trailing bytes | typed rejection before allocation/publication |
| T23 | codec | duplicate identity-bearing field in extensible framing | `DuplicateField` |
| T24 | restore | unknown catalog/shape/nominal/payload keys | distinct typed errors |
| T25 | restore | representation mismatch after key resolution | reject entire staged batch |
| T26 | restore | valid live→snapshot→restore | semantic equality and identical re-encoding |
| T27 | restore | restored carrier under same match plan | same selected arm/transcript outcome |
| T28 | coordinator | one invalid carrier in multi-task batch | no task/handle/wakeup published |
| T29 | determinism | random insertion and worker scheduling permutations | identical snapshot carrier bytes and plan digest |
| T30 | property/fuzz | arbitrary valid carrier records | decode(encode(x)) semantic round trip; no panic |
| T31 | property/fuzz | arbitrary byte strings under size cap | decoder never panics/over-allocates; canonical errors only |
| T32 | compile/lint | implementation inspection | no extension trait/side-table workaround; owner enum has inherent behavior |

## Required command gates after implementation

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Use the repository's narrower mandated commands from the applicable `AGENTS.md` in addition to, not instead of, these gates. Golden byte tests must pin the format version. Property tests must cap recursion, generic argument count, key length, and aggregate allocation.


<!-- END 07-test-matrix.md -->


<!-- BEGIN 08-implementation-sequence.md -->

# 08. Implementation sequence and admission gates

This is a design-only package. The sequence below is the concrete future production change plan and intentionally contains no patch.

## Phase 0 — owner lock and baseline

1. Checkout `UNAVAILABLE` (or rebase the design against a newer `origin/main` and record the replacement SHA).
2. Re-read every applicable `AGENTS.md` for files to be edited.
3. Confirm the owner anchors in `01-evidence-basis.md` by opening the full files, not only grep snippets.
4. Record baseline `fmt`, `check`, focused tests, workspace tests, and clippy. Do not attribute pre-existing failures to the change.

**Gate G0:** one named owner for carrier, checked plan, codec, and coordinator; no duplicate enum/API family.

## Phase 1 — canonical IDs and owner enum

1. Reuse or extend existing canonical structural-shape and nominal-instance interning.
2. Add the `Structural`/`Nominal` representation to the original runtime carrier/value enum.
3. Add constructors, accessors, invariant validation, equality/hash semantics, and errors to its inherent `impl`.
4. Migrate construction sites; keep any temporary compatibility constructor `pub(crate)` and delete it in Phase 5.

**Gate G1:** T1–T5, T15–T16 pass; no side table, extension trait, or debug-name identity.

## Phase 2 — checked constraint and projection witness

1. Normalize aliases and generic args in checked typing.
2. Emit `AcceptedCarrierConstraint` and, only when legal, `StructuralProjectionWitness`.
3. Include both in semantic child encoding/sealing and the coverage digest.
4. Make invalid/opaque projections a checked diagnostic rather than a runtime guess.

**Gate G2:** T6–T19 pass in focused compiler/runtime test suites.

## Phase 3 — runtime execution and transcript

1. Replace any shape-only/nominal-recovery fallback with `AcceptedRuntimeCarrier::admit`.
2. Execute precompiled projection steps.
3. Emit stable transcript rows from the same constraint table.
4. Measure hot path to ensure no allocation/catalog traversal/full-shape hashing.

**Gate G3:** complete admission matrix; execution and coverage digest remain isomorphic.

## Phase 4 — snapshot codec and two-phase restore

1. Add unresolved wire records and canonical encode/decode.
2. Add resolver validation in dependency order.
3. Stage carriers with payloads/plans/tasks; publish through the coordinator barrier.
4. Add golden vectors, corruption tests, round-trip and deterministic-order property tests.

**Gate G4:** T20–T31 pass, including atomic failure and byte-for-byte re-encoding.

## Phase 5 — closure and cleanup

1. Remove legacy constructors, fallbacks, compatibility aliases, and independently generated coverage/runtime domains.
2. Run `rg` proof searches for forbidden side tables, shape-to-nominal recovery, raw ID serialization, and duplicate carrier enums.
3. Run all command gates and update relevant design/implementation docs with exact SHA and logs.

**Gate G5:** T32 plus workspace format/check/test/clippy; no production TODO/placeholder/Open Question.

## File-level edit map

| Order | Current/proposed owner | Changes |
|---:|---|---|
| 1 | `crates/<language-owner>/src/checked/type.rs` | canonical IDs, normalized accepted constraint, projection witness emission |
| 2 | `crates/<runtime-owner>/src/value.rs (new module only if no owning enum exists)` | carrier variants and inherent construction/admission behavior |
| 3 | `crates/<runtime-owner>/src/match_exec.rs` | sole use of checked constraints; transcript and coverage digest tie |
| 4 | `crates/<runtime-owner>/src/snapshot.rs` | stable wire record, canonical codec, unresolved resolver |
| 5 | `crates/<runtime-owner>/src/task.rs` | staged dependency graph and atomic publication/wakeup |
| 6 | adjacent test modules/fixtures | T1–T32 and golden vectors |

## Migration rule

At no point may old and new carrier authorities both be public. Introduce new internals, migrate all producers/consumers in one gated series, then remove the old path before admission. Compatibility at a persistence boundary must be a versioned decoder path, not a silent semantic fallback.


<!-- END 08-implementation-sequence.md -->


<!-- BEGIN 09-requirement-traceability.md -->

# 09. Request requirement traceability

The request was read in full. The rows below quote/paraphrase extracted numbered requirements and map them to concrete decisions, APIs, and tests. The original unmodified request is in `inputs/REQUEST.md`.

| Request row | Requirement text | Concrete closure in this package | Test/gate |
|---:|---|---|---|
| 8 | the deletion and compile-clean order that replaces the current fail-closed | Trace to the normative invariants and implementation/test rows in this package; no generic `CLOSED` placeholder is used | G0–G5 |

## Closure assertion

Every semantic requirement is owned by one API/decision and at least one test/gate. `OPEN_QUESTIONS = 0`; the implementation must adapt spelling to existing owner types without changing these semantics.


<!-- END 09-requirement-traceability.md -->


<!-- BEGIN 10-verification-boundary.md -->

# 10. Verification boundary

## Actually verified

- All three supplied inputs were read byte-for-byte in full and their SHA-256 values are recorded.
- Repository acquisition/update commands and complete current-main SHA are recorded when access succeeded.
- Every `AGENTS.md` found in that checkout was read in full and copied under `evidence/AGENTS/`.
- Current source was searched for carrier, structural/nominal, match/coverage, snapshot/restore, task/Need/handle, catalog/digest, and AWBC anchors; exact grep rows are retained.
- The design package contains no production source overlay or repository mutation.
- ZIP contents, internal file hashes, and archive readability are verified by the package builder.

## Baseline commands run against unmodified current main

| Command | Exit | Runtime | Log |
|---|---:|---:|---|
| `(repository acquisition)` | 255 | 0 s | `validation/repository_acquisition.log` |

An exit other than zero is not hidden or relabeled. Read the corresponding log before attributing the result. These are **baseline/current-main checks**, not proof of an unimplemented design.

## Not claimed as verified

- The proposed production APIs have not been compiled because this return is design-only and deliberately contains no patch.
- T1–T32 are specified executable test rows, not represented as passing before implementation.
- Performance bounds are architectural (interned key comparisons/no hot-path allocation); no benchmark of unimplemented code is claimed.
- Cross-version compatibility is fixed by the grammar/version rules but requires implementation golden vectors before release.

## Validation classification

- **Source evidence:** verified only when repository acquisition succeeded and a path/line appears in `evidence/source-search-results.md`.
- **Design decision:** normative for the requested correction, but not current implementation evidence.
- **Proposed spelling/path:** may be renamed to current owner conventions; semantic ownership and invariants are not optional.
- **Future gate:** must pass after production implementation.


<!-- END 10-verification-boundary.md -->
