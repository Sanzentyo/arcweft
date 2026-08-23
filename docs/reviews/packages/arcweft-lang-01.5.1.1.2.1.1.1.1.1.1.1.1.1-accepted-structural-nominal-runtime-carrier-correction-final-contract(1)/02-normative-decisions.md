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
