# Generic match complete transcript and coverage closure — final design contract

- Request: `2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure-correction(1).md`
- Package: `arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure-correction-final-contract.zip`
- Mode: **design-only**; no production source is modified by this package.
- Contract status: **ACCEPTED-DESIGN / IMPLEMENTABLE**
- `OPEN_QUESTIONS=0`
- Repository baseline used: `UNMATERIALIZED`
- Observed `origin/main`: `UNMATERIALIZED`
- Verification tier: `V3_REQUEST_AND_CONTRACT_ONLY`
- Generated at: `2026-08-22T19:26:31.522700+00:00`

## 1. Outcome

A generic `match` is admitted only when the checked owner has produced one immutable, complete, schema-versioned transcript and the same sealing operation has proven coverage closed for the exact generic universe. Partial rows, inferred reconstruction in HIR/runtime, unresolved substitutions, unknown guards used as coverage, legacy empty transcripts, and restore-time publication before validation are all impossible through the public type boundary.

The design is deliberately capability-typed: an open builder cannot be passed to lowering; an open or poisoned coverage result cannot be embedded in the complete carrier; a decoded restore candidate cannot be published until owner, universe, references, canonical order, closure digest, and full transcript digest are verified.

## 2. Normative scope

Included:

- complete source-arm and normalized-alternative transcript production;
- generic substitution and constructor-universe binding;
- guard-aware coverage closure and structured witnesses;
- canonical Need/runtime identity carriage;
- deterministic digest grammar and cache invalidation;
- checked → HIR → runtime → persistence/restore admission;
- diagnostics, migration, performance constraints, and exact test closure.

Excluded:

- changing match surface syntax;
- changing the semantic meaning of existing patterns;
- runtime recomputation of exhaustiveness;
- production patch contents in this design-only return.

## 3. Verified source baseline and evidence

### 3.1 Baseline

- Local Git worktree available: `False`
- Local source tree available: `False`
- `HEAD`: `not materialized`
- `origin/main`: `not materialized`
- Working tree clean: `False`
- AGENTS files read, root-to-leaf: not materialized locally

### 3.2 Highest-relevance source evidence

- No local repository source materialization was available; see verification tier and request evidence.

Exact line-numbered excerpts are in `evidence/source-excerpts.md`; the complete machine inventory is in `machine/source-inventory.json`. Existing owner paths below are marked separately from proposed modules, so an unverified guess can never masquerade as source evidence.

## 4. Owner and integration mapping

| Concern | Existing owner path | Evidence status | Required implementation placement |
|---|---|---|---|
| checked generic-match certification owner | `(not materialized)` | NOT_MATERIALIZED | extend the existing match checker/certifier owner; no sidecar helper trait |
| coverage algebra and closure owner | `(not materialized)` | NOT_MATERIALIZED | add closure variants/methods to the original coverage enum/impl |
| HIR carrier owner | `(not materialized)` | NOT_MATERIALIZED | carry only CompleteMatchTranscriptId/ClosedCoverageId after admission |
| runtime match admission owner | `(not materialized)` | NOT_MATERIALIZED | reject plans without a complete sealed transcript before execution/persistence |
| Need identity mapping owner | `(not materialized)` | NOT_MATERIALIZED | reuse existing Need identity type; transcript stores identity, never an ad-hoc integer |

Placement rule: when the current source already owns the relevant enum or identity, add ordering/encoding/validation behavior to that enum’s original inherent `impl`. Do not introduce an extension trait, duplicate “wire” enum, call-site switch, or unrelated helper merely because the current impl lacks a method.

## 5. Exact decisions

### D01: Single authoritative owner

The checked generic-match certifier is the only producer of a complete transcript. Parser, HIR lowering, runtime planner, persistence, and restore may carry or verify the sealed artifact but may not reconstruct it.

### D02: No partial transcript crosses the certification boundary

Construction uses `MatchTranscriptBuilder`; only `seal(self, ...) -> Result<CompleteMatchTranscript, MatchClosureError>` creates the admitted type. The builder and all open coverage states remain crate-private.

### D03: Every source arm has exactly one transcript row

Rows are keyed by stable `MatchArmId` derived from source owner plus arm ordinal. Wildcards, guarded arms, syntactically unreachable arms, and recovery arms are present; omission is a hard internal error.

### D04: Every normalized pattern alternative has a row

An arm row contains a non-empty ordered `PatternAlternativeTranscript` list. Or-pattern expansion is represented explicitly and retains a reversible source path; alternatives are never silently coalesced.

### D05: Generic substitutions are closed before coverage

Coverage is evaluated against a `GenericMatchUniverseKey` containing the checked scrutinee type and canonical substitutions. Unresolved inference variables make sealing fail with `UnresolvedGenericUniverse`.

### D06: Coverage closure is a typed result, not a boolean

`CoverageClosure` distinguishes `Closed`, `Open`, and `Poisoned`. Only `ClosedCoverage` can be embedded in `CompleteMatchTranscript`; gaps and redundancy witnesses remain structured diagnostics.

### D07: Guards do not falsely close coverage

Only a statically proven always-true guard contributes unconditional coverage. Unknown/runtime guards retain reachability metadata but contribute no closing coverage. Always-false guards are represented and diagnosed.

### D08: Runtime Need identity is preserved exactly

A pattern observation that references a Need/runtime value records the existing canonical Need identity and view/instance discriminator. Equality and serialization delegate to the owning enum/type impl, not a helper trait or ad-hoc encoding.

### D09: Deterministic ordering is normative

Arms use source order; alternatives use normalized left-to-right order; constructor atoms use the checked type owner’s canonical order; maps are sorted before serialization. Hash iteration order is forbidden in digest input.

### D10: Digest grammar is versioned and domain-separated

Transcript and closure digests use the byte grammar in this package with explicit tags, lengths, enum discriminants, and schema version. Debug text, Rust layout, serde defaults, and pointer identity are forbidden inputs.

### D11: HIR and runtime admission are capability-typed

Post-check HIR stores `CompleteMatchTranscriptId` and `ClosedCoverageId`; runtime plan construction requires `&CompleteMatchTranscript`. APIs accepting optional/bare transcript data are removed or kept only as private migration shims.

### D12: Persistence is two-phase

Restore decodes a candidate, validates schema/digest/references/row completeness, rechecks closure identity, and only then publishes the runtime handle. No partially restored match task becomes observable.

### D13: Diagnostics are lossless and stable

Coverage gaps, redundant alternatives, guard dispositions, unresolved generic substitutions, row omissions, and digest mismatch each have distinct error variants with source anchors and stable diagnostic codes.

### D14: Existing enums own new behavior

When `Coverage`, `NeedId`, runtime handle, or match-result enums already exist in arcweft, canonical ordering, encoding, and validation methods are added to their original `impl` blocks. No extension trait, duplicate helper enum, or switch-at-call-site is admitted.

### D15: Transcript completeness is checked in O(rows + atoms)

Sealing performs one pass over source arms, one pass over normalized alternatives, and set-algebra operations over canonical constructor indices. The runtime hot path only dereferences a sealed ID and never recomputes coverage.

### D16: Cache keys include semantic closure

Any generic match cache key includes owner identity, canonical substitutions, scrutinee checked type digest, transcript schema version, and closed-coverage digest. Source span and debug text are excluded.

### D17: Invalidation is explicit

A change to pattern normalization, constructor universe, generic substitutions, guard proof, Need identity encoding, or schema version invalidates the transcript/cache entry. Pure source relocation does not, unless source identity is part of the repository’s existing stable owner key.

### D18: Recovery cannot be admitted

Error-recovery patterns may produce a poisoned builder for diagnostics, but `PoisonedCoverage` has no conversion to `ClosedCoverage`; lowering/runtime construction must stop before plan emission.

### D19: Test closure is row-addressable

Every normative decision maps to named unit/integration/golden/property rows. Golden transcript tests assert all fields and byte digests, not only pass/fail diagnostics.

### D20: Migration is fail-closed

Old snapshots/cache entries without schema-versioned complete transcripts are rejected and regenerated. No default empty transcript, inferred wildcard closure, or legacy “assume exhaustive” compatibility path is allowed.

### D21: Typed Need producer ABI is transcript-bound

An alternative that invokes or observes a typed Need producer records the checked producer ID, exact output checked-type digest, canonical Need identity, runtime instance identity, runtime view identity, and AWBC allocation binding as one indivisible row. Runtime code may not re-infer any member.

### D22: AWBC allocation authority remains canonical

The transcript stores the existing `AwbcAllocationId`, generation, lane, and storage class issued by the canonical allocator. Encoding/order methods are added to the original AWBC enum/ID impl. Match lowering and restore validate the binding; they never allocate a substitute slot.

### D23: Need instance and view are distinct identities

`NeedIdentity`, `RuntimeNeedInstanceIdentity`, and `RuntimeNeedViewIdentity` are carried and checked separately. Equality of one does not imply equality of the others; transcript verification rejects mixed-instance or wrong-view bindings.

### D24: Nominal and structural pattern carriers remain distinguishable

Coverage atoms preserve the checked carrier kind and owner identity. A structural shape cannot close a nominal constructor universe merely because fields match, and nominal tags are never reconstructed from runtime layout.

### D25: Semantic provenance is complete

Each row retains source arm/alternative identity, normalized pattern ID/path, checked type, generic universe, binding slots, producer/Need/AWBC identities, guard proof, reachability, and coverage atoms. Any consumer needing another semantic fact extends the owning row/schema rather than opening a side channel.

### D26: Snapshot isomorphism includes transcript-bound runtime identities

Compile → persist → restore yields the same transcript digest, closed-coverage digest, Need/instance/view identities, producer binding, and AWBC allocation binding. Restore remapping is allowed only through the repository’s explicit canonical relocation table and is included in verification.

### D27: Coverage certification is authoritative

Runtime dispatch, task coordination, and restore never perform a fallback exhaustiveness check or assume wildcard closure. They verify the checked certificate identity and fail closed on absence/mismatch.

### D28: Independent generations cannot alias

Generation identity participates in producer/AWBC bindings and cache validation. Two independently generated plans with equal source syntax cannot share mutable/runtime allocation state unless the existing catalog explicitly proves the complete semantic key identical.


## 6. Concrete Rust API

The type names below are normative contract names. During implementation, repository-native checked IDs/digests replace the descriptive aliases one-for-one according to the owner table; their semantic fields may not be dropped.


```rust
/// Existing checked owner identity; map to the repository's canonical owner key.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GenericMatchOwnerId {
    pub checked_module: CheckedModuleId,
    pub checked_item: CheckedItemId,
    pub match_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct MatchArmId {
    pub owner: GenericMatchOwnerId,
    pub source_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PatternAlternativeId {
    pub arm: MatchArmId,
    pub normalized_ordinal: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenericMatchUniverseKey {
    pub owner: GenericMatchOwnerId,
    pub scrutinee_checked_type: CheckedTypeDigest,
    pub substitutions: CanonicalGenericSubstitutions,
    pub constructor_universe: ConstructorUniverseDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum GuardDisposition {
    Absent = 0,
    ProvenTrue = 1,
    ProvenFalse = 2,
    RuntimeUnknown = 3,
}

impl GuardDisposition {
    /// Only these dispositions contribute unconditional coverage.
    pub const fn closes_coverage(self) -> bool {
        matches!(self, Self::Absent | Self::ProvenTrue)
    }

    pub const fn stable_tag(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoverageAtom {
    Constructor {
        owner: CheckedTypeOwnerId,
        constructor_index: u32,
        fields: Box<[CoverageAtom]>,
    },
    IntegerRange {
        signed: bool,
        width: u16,
        lo_inclusive: CanonicalInteger,
        hi_inclusive: CanonicalInteger,
    },
    TextLiteral(CanonicalTextId),
    Nominal {
        nominal: NominalTypeId,
        fields: Box<[CoverageAtom]>,
    },
    Wildcard,
    RuntimeNeed {
        need: NeedIdentity,
        view: RuntimeNeedViewIdentity,
    },
}

impl CoverageAtom {
    /// Add these methods to the original enum impl if this enum already exists.
    pub const fn stable_tag(&self) -> u8 {
        match self {
            Self::Constructor { .. } => 0x20,
            Self::IntegerRange { .. } => 0x21,
            Self::TextLiteral(_) => 0x22,
            Self::Nominal { .. } => 0x23,
            Self::Wildcard => 0x24,
            Self::RuntimeNeed { .. } => 0x25,
        }
    }

    pub fn canonical_cmp(&self, other: &Self) -> core::cmp::Ordering {
        use core::cmp::Ordering;
        let tag_order = self.stable_tag().cmp(&other.stable_tag());
        if tag_order != Ordering::Equal {
            return tag_order;
        }
        match (self, other) {
            (
                Self::Constructor { owner: ao, constructor_index: ai, fields: af },
                Self::Constructor { owner: bo, constructor_index: bi, fields: bf },
            ) => ao.cmp(bo).then(ai.cmp(bi)).then_with(|| af.as_ref().cmp(bf.as_ref())),
            (
                Self::IntegerRange { signed: asg, width: aw, lo_inclusive: al, hi_inclusive: ah },
                Self::IntegerRange { signed: bsg, width: bw, lo_inclusive: bl, hi_inclusive: bh },
            ) => asg.cmp(bsg).then(aw.cmp(bw)).then(al.cmp(bl)).then(ah.cmp(bh)),
            (Self::TextLiteral(a), Self::TextLiteral(b)) => a.cmp(b),
            (Self::Nominal { nominal: an, fields: af }, Self::Nominal { nominal: bn, fields: bf }) => {
                an.cmp(bn).then_with(|| af.as_ref().cmp(bf.as_ref()))
            }
            (Self::Wildcard, Self::Wildcard) => Ordering::Equal,
            (Self::RuntimeNeed { need: an, view: av }, Self::RuntimeNeed { need: bn, view: bv }) => {
                an.cmp(bn).then(av.cmp(bv))
            }
            _ => unreachable!("equal stable tags imply equal CoverageAtom variants"),
        }
    }

    pub fn encode_stable(&self, out: &mut StableEncoder) {
        out.u8(self.stable_tag());
        match self {
            Self::Constructor { owner, constructor_index, fields } => {
                owner.encode_stable(out);
                out.uleb128(u64::from(*constructor_index));
                out.uleb128(fields.len() as u64);
                for field in fields { field.encode_stable(out); }
            }
            Self::IntegerRange { signed, width, lo_inclusive, hi_inclusive } => {
                out.u8(u8::from(*signed));
                out.u16_le(*width);
                lo_inclusive.encode_stable(out);
                hi_inclusive.encode_stable(out);
            }
            Self::TextLiteral(text) => text.encode_stable(out),
            Self::Nominal { nominal, fields } => {
                nominal.encode_stable(out);
                out.uleb128(fields.len() as u64);
                for field in fields { field.encode_stable(out); }
            }
            Self::Wildcard => {}
            Self::RuntimeNeed { need, view } => {
                need.encode_stable(out);
                view.encode_stable(out);
            }
        }
    }
}

impl Ord for CoverageAtom {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering { self.canonical_cmp(other) }
}

impl PartialOrd for CoverageAtom {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> { Some(self.cmp(other)) }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AwbcAllocationBinding {
    pub allocation: AwbcAllocationId,
    pub generation: GenerationId,
    pub lane: u32,
    pub storage_class: AwbcStorageClass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedNeedProducerBinding {
    pub producer: CheckedNeedProducerId,
    pub output_checked_type: CheckedTypeDigest,
    pub need: NeedIdentity,
    pub instance: RuntimeNeedInstanceIdentity,
    pub view: RuntimeNeedViewIdentity,
    pub awbc: AwbcAllocationBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternAlternativeTranscript {
    pub id: PatternAlternativeId,
    pub source_pattern_path: CanonicalPatternPath,
    pub normalized_pattern: CheckedPatternId,
    pub coverage_atoms: Box<[CoverageAtom]>,
    pub guard: GuardDisposition,
    pub reachability: AlternativeReachability,
    pub bound_slots: Box<[CheckedBindingSlotId]>,
    pub typed_need_producer: Option<TypedNeedProducerBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchArmTranscript {
    pub id: MatchArmId,
    pub source_span: SourceSpanId,
    pub alternatives: Box<[PatternAlternativeTranscript]>,
    pub result_checked_type: CheckedTypeDigest,
    pub runtime_need_uses: Box<[NeedIdentity]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageGapWitness {
    pub uncovered: Box<[CoverageAtom]>,
    pub rendered_example: CanonicalWitnessText,
    pub source_anchor: SourceSpanId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedundancyWitness {
    pub alternative: PatternAlternativeId,
    pub subsumed_by: Box<[PatternAlternativeId]>,
    pub source_anchor: SourceSpanId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosedCoverage {
    pub universe: GenericMatchUniverseKey,
    pub covered_constructor_bits: CanonicalBitSet,
    pub redundancy: Box<[RedundancyWitness]>,
    pub digest: ClosedCoverageDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCoverage {
    pub universe: GenericMatchUniverseKey,
    pub gaps: Box<[CoverageGapWitness]>,
    pub redundancy: Box<[RedundancyWitness]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoverageClosure {
    Closed(ClosedCoverage),
    Open(OpenCoverage),
    Poisoned(MatchPoison),
}

impl CoverageClosure {
    pub fn require_closed(self) -> Result<ClosedCoverage, MatchClosureError> {
        match self {
            Self::Closed(closed) => Ok(closed),
            Self::Open(open) => Err(MatchClosureError::NonExhaustive(open)),
            Self::Poisoned(poison) => Err(MatchClosureError::Poisoned(poison)),
        }
    }
}

#[derive(Debug)]
pub struct MatchTranscriptBuilder {
    owner: GenericMatchOwnerId,
    universe: GenericMatchUniverseKey,
    expected_arm_count: u32,
    rows: Vec<Option<MatchArmTranscript>>,
    poisoned: Option<MatchPoison>,
}

impl MatchTranscriptBuilder {
    pub fn new(
        owner: GenericMatchOwnerId,
        universe: GenericMatchUniverseKey,
        expected_arm_count: u32,
    ) -> Result<Self, MatchClosureError>;

    pub fn record_arm(
        &mut self,
        row: MatchArmTranscript,
    ) -> Result<(), MatchClosureError>;

    pub fn poison(&mut self, poison: MatchPoison);

    /// Consumes all open state. This is the sole constructor of the complete carrier.
    pub fn seal(
        self,
        coverage_engine: &mut CoverageEngine,
    ) -> Result<CompleteMatchTranscript, MatchClosureError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteMatchTranscript {
    owner: GenericMatchOwnerId,
    universe: GenericMatchUniverseKey,
    arms: Box<[MatchArmTranscript]>,
    closed_coverage: ClosedCoverage,
    schema_version: MatchTranscriptSchemaVersion,
    digest: MatchTranscriptDigest,
}

impl CompleteMatchTranscript {
    pub fn id(&self) -> CompleteMatchTranscriptId;
    pub fn owner(&self) -> GenericMatchOwnerId;
    pub fn universe(&self) -> &GenericMatchUniverseKey;
    pub fn arms(&self) -> &[MatchArmTranscript];
    pub fn closed_coverage(&self) -> &ClosedCoverage;
    pub fn digest(&self) -> MatchTranscriptDigest;

    pub fn verify_for(
        &self,
        expected_owner: GenericMatchOwnerId,
        expected_universe: &GenericMatchUniverseKey,
    ) -> Result<(), MatchTranscriptVerificationError>;

    pub fn encode_stable(&self, out: &mut StableEncoder);
    pub fn decode_and_verify(
        input: &mut StableDecoder<'_>,
        catalog: &CheckedCatalog,
    ) -> Result<Self, MatchTranscriptDecodeError>;
}

/// Post-check HIR/runtime plan constructors accept the admitted capability.
pub fn lower_checked_generic_match(
    checked: &CheckedGenericMatch,
    transcript: &CompleteMatchTranscript,
    hir: &mut HirBuilder,
) -> Result<HirMatchId, HirLowerError>;

pub fn build_runtime_match_plan(
    hir_match: &HirMatch,
    transcript: &CompleteMatchTranscript,
    runtime_catalog: &RuntimeCatalog,
) -> Result<RuntimeMatchPlan, RuntimePlanError>;
```


### 6.1 Constructor visibility

- All fields of `CompleteMatchTranscript` are private to its owning crate.
- There is no `Default`, unchecked `From`, public struct literal, or deserializer that bypasses verification.
- `MatchTranscriptBuilder::seal` is the sole non-restore constructor.
- Restore uses `decode_and_verify`; raw decode returns a private candidate, not `Self`.
- `ClosedCoverage` has no constructor from a boolean and no `unwrap_or_default` path.

### 6.2 Exact row-completeness algorithm

1. Allocate `rows` with exactly `expected_arm_count` slots.
2. For each checked source arm, derive `MatchArmId { owner, source_ordinal }`.
3. Reject an ordinal outside the allocated range or a second write to the same slot.
4. Normalize that arm into one or more alternatives in left-to-right source order.
5. Require a non-empty alternative array. Each alternative records source pattern path, checked pattern ID, canonical atoms, guard disposition, reachability, and binding slots.
6. On `seal`, reject any `None` slot before invoking coverage.
7. Compute coverage against the exact `GenericMatchUniverseKey`.
8. Require `CoverageClosure::Closed`; retain redundancy witnesses in the closed artifact.
9. Encode canonical bytes, compute the domain-separated digest, and immediately self-verify.
10. Intern the complete artifact and issue the only ID accepted by HIR/runtime plan construction.

### 6.3 Coverage algebra

For finite constructor universes, coverage uses canonical constructor indices and a bitset. Product/record patterns recurse field-wise; ranges use canonical disjoint interval sets; literal domains use sorted canonical IDs; wildcard denotes the current universe, not a global untyped top. A runtime/unknown guard contributes zero unconditional coverage. Redundancy is calculated against coverage accumulated before the alternative, while reachability and source row retention remain independent.

Generic substitutions are applied before constructor enumeration. The universe digest changes when nominal owner, checked scrutinee representation, substitutions, constructor set/order, or schema changes. Two generic instantiations cannot share a closure merely because their source syntax is identical.

## 7. State machine


| State | Owner | Observable outside checker? | Allowed operation | Exit |
|---|---|---:|---|---|
| `Allocated` | checked match certifier | No | create builder with owner/universe/expected count | `Collecting` |
| `Collecting` | checked match certifier | No | record each source arm exactly once; poison on recovery | `Collected` or `Poisoned` |
| `Collected` | coverage engine | No | normalize/order atoms, compute closure and redundancy | `Closed` or `Open` |
| `Open` | diagnostic owner | No | emit structured gaps/redundancy; stop lowering | terminal error |
| `Poisoned` | diagnostic owner | No | preserve prior diagnostics; stop lowering | terminal error |
| `Closed` | transcript sealer | No | encode canonical body, compute digest, self-verify | `Complete` |
| `Complete` | checked catalog | Yes, immutable | intern by digest and issue `CompleteMatchTranscriptId` | `Admitted` |
| `Admitted` | HIR/runtime plan | Yes | lower/build/persist using capability-typed reference | runtime/persistence |
| `RestoreCandidate` | restore coordinator | No | decode, validate references/digests/closure | `Admitted` or reject |


### 7.1 Failure atomicity

All transitions before `Complete` are local and non-observable. Catalog interning, HIR attachment, task publication, snapshot publication, and restore handle insertion happen only after full validation. A failure drops the candidate and preserves structured diagnostics; it never installs a placeholder ID.

## 8. Canonical byte grammar


All integers are unsigned LEB128 unless explicitly fixed-width. Every aggregate begins with a one-byte tag and a length/count; text and opaque digests are length-prefixed. No host-endian or Rust-layout bytes are permitted.

```text
MATCH_TRANSCRIPT :=
    0xA7
    schema_version:u16-le
    domain_len:uleb128 domain:"arcweft.match.transcript"
    owner:OWNER_ID
    universe:UNIVERSE
    arm_count:uleb128 ARM*
    closed_coverage:CLOSED_COVERAGE
    body_digest:[u8;32]

UNIVERSE :=
    0x01
    checked_type_digest:[u8;32]
    substitutions_len:uleb128 substitutions:CANONICAL_SUBSTITUTIONS
    constructor_universe_digest:[u8;32]

ARM :=
    0x10
    source_ordinal:uleb128
    source_span:SOURCE_SPAN_ID
    alternative_count:uleb128 ALTERNATIVE+
    result_checked_type_digest:[u8;32]
    need_use_count:uleb128 NEED_IDENTITY*

ALTERNATIVE :=
    0x11
    normalized_ordinal:uleb128
    source_pattern_path:CANONICAL_PATTERN_PATH
    checked_pattern_id:CHECKED_PATTERN_ID
    guard_tag:u8
    reachability_tag:u8
    atom_count:uleb128 COVERAGE_ATOM+
    binding_count:uleb128 CHECKED_BINDING_SLOT_ID*
    producer_present:u8
    (TYPED_NEED_PRODUCER_BINDING if producer_present == 1)

TYPED_NEED_PRODUCER_BINDING :=
    0x40
    checked_producer_id:CHECKED_NEED_PRODUCER_ID
    output_checked_type_digest:[u8;32]
    need_id:NEED_IDENTITY
    instance_id:RUNTIME_NEED_INSTANCE_IDENTITY
    view_id:RUNTIME_NEED_VIEW_IDENTITY
    allocation_id:AWBC_ALLOCATION_ID
    generation_id:GENERATION_ID
    lane:uleb128
    storage_class_tag:u8

COVERAGE_ATOM :=
      0x20 owner:CHECKED_TYPE_OWNER_ID constructor_index:uleb128 field_count:uleb128 COVERAGE_ATOM*
    | 0x21 signed:u8 width:u16-le lo:CANONICAL_INTEGER hi:CANONICAL_INTEGER
    | 0x22 text_id:CANONICAL_TEXT_ID
    | 0x23 nominal_id:NOMINAL_TYPE_ID field_count:uleb128 COVERAGE_ATOM*
    | 0x24
    | 0x25 need_id:NEED_IDENTITY need_view:RUNTIME_NEED_VIEW_IDENTITY

CLOSED_COVERAGE :=
    0x30
    universe_digest:[u8;32]
    constructor_bit_len:uleb128 constructor_bits:bytes
    redundancy_count:uleb128 REDUNDANCY_WITNESS*
    closure_digest:[u8;32]

REDUNDANCY_WITNESS :=
    0x31 alternative_id:PATTERN_ALTERNATIVE_ID
    subsumer_count:uleb128 PATTERN_ALTERNATIVE_ID*
    source_anchor:SOURCE_SPAN_ID
```

Digest procedure:

1. Encode the body with the digest field absent.
2. Hash `domain || schema_version || encoded_body` using the repository’s existing canonical digest primitive.
3. Encode the 32-byte result as `body_digest`.
4. On decode, reject unknown schema versions, non-canonical ordering, duplicate arm/alternative IDs, row-count mismatch, universe mismatch, missing references, and digest mismatch before publishing the value.


### 8.1 Canonical enum ordering

The stable order is fixed by explicit tags in the original enum impl. Variant declaration order may be reused only when protected by an explicit stable-tag method and golden test. Derived `Ord` is not sufficient if fields include repository types without a normative semantic order. Need/view identities delegate to their current owner’s canonical encoding.

## 9. Diagnostics

Required error variants and stable codes:

| Code | Rust variant | Trigger | Required payload |
|---|---|---|---|
| `GM0001` | `MissingArmRow` | source arm has no transcript row | owner, ordinal, arm span |
| `GM0002` | `DuplicateArmRow` | row recorded twice | arm ID, first/second anchors |
| `GM0003` | `EmptyAlternativeSet` | normalized arm has no alternatives | arm ID, arm span |
| `GM0004` | `DuplicateAlternativeId` | alternative ordinal reused | arm ID, ordinal |
| `GM0005` | `UnresolvedGenericUniverse` | inference/generic value unresolved at seal | owner, unresolved parameter IDs |
| `GM0006` | `NonExhaustive` | closure remains open | ordered `CoverageGapWitness[]` |
| `GM0007` | `Poisoned` | recovery/error pattern participated | prior diagnostic IDs and span |
| `GM0008` | `UniverseMismatch` | transcript and checked catalog disagree | expected/actual universe digests |
| `GM0009` | `DanglingNeedIdentity` | referenced Need/view absent | Need identity and alternative ID |
| `GM0010` | `NonCanonicalOrder` | decoded rows/atoms are not canonical | first offending path |
| `GM0011` | `TranscriptDigestMismatch` | body digest verification fails | expected/actual digest |
| `GM0012` | `UnsupportedTranscriptSchema` | unknown version | observed/supported versions |
| `GM0013` | `ClosedCoverageDigestMismatch` | closure digest/catalog mismatch | expected/actual digest |
| `GM0014` | `LegacyTranscriptMissing` | old snapshot/cache has no complete carrier | snapshot/cache key |

Diagnostics render source examples from structured witnesses; rendered strings are never persisted as semantic input.

## 10. HIR, runtime, persistence, and restore

### 10.1 HIR

The HIR match node stores the existing checked owner plus `CompleteMatchTranscriptId` and `ClosedCoverageId` (or one interned transcript ID if closure is structurally contained). It does not store an optional transcript, a raw list of arms as an alternative semantic source, or a “coverage checked” boolean.

### 10.2 Runtime plan

Plan building validates owner/universe once, resolves each transcript alternative to the runtime dispatch representation, and stores only the minimal dispatch data plus the immutable transcript ID for audit/snapshot identity. The hot dispatch loop never visits coverage witnesses and never recomputes generic closure.

### 10.3 Snapshot

Snapshot serialization writes schema version, transcript digest/ID, closed coverage digest, owner, generic universe, and all runtime references required by the existing handle model. A snapshot cannot claim a transcript ID absent from its catalog section.

### 10.4 Two-phase restore

Phase A decodes and validates all transcript candidates in a private restore arena. Phase B resolves cross-references, verifies closure/catalog identity, interns immutable artifacts, then atomically publishes runtime handles/tasks. Any Phase A/B error drops the whole affected unit before observability; no “best effort” arm list is admitted.

## 11. Typed Need producer, instance/view, and AWBC binding

A `PatternAlternativeTranscript` has either no typed producer or exactly one `TypedNeedProducerBinding`. The binding is atomic: producer ID, exact output checked type, Need identity, runtime instance, runtime view, AWBC allocation, generation, lane, and storage class are recorded together and covered by the transcript digest. A call site may not supply some fields from the transcript and recover others from runtime catalogs.

Validation order is fixed: checked producer exists → output type digest matches → Need identity resolves → instance belongs to that Need → view belongs to that instance and checked view contract → AWBC allocation belongs to the same generation and producer site → lane/storage class match → canonical encoding matches. The first failure returns its specific diagnostic; no allocation or handle is published.

Structural and nominal carriers retain separate stable tags and checked owner IDs. Runtime layout similarity is not a proof of checked carrier identity. Any missing canonical operation on an existing Need, view, carrier, or AWBC enum is implemented in that enum’s original inherent `impl`, as required by the repository design rule.

## 12. Cache and invalidation table

| Change | Transcript digest | Closure digest | Cache action |
|---|---:|---:|---|
| source span only | unchanged under existing stable owner policy | unchanged | reuse |
| arm order | changes | may change | invalidate |
| or-pattern alternative order | changes | semantic set may match, transcript changes | invalidate |
| checked scrutinee type | changes | changes | invalidate |
| canonical generic substitution | changes | changes | invalidate |
| constructor catalog/order | changes | changes | invalidate |
| guard proof result | changes | changes when coverage contribution changes | invalidate |
| Need/view identity encoding | changes | changes for affected atoms | invalidate |
| diagnostic wording only | unchanged | unchanged | reuse |
| schema version | changes | changes | reject/regenerate |

## 13. Test closure

| Test ID | Decisions | Layer | Fixture | Exact oracle |
|---|---|---|---|---|
| GM-T001 | D03,D04 | unit | three source arms including an or-pattern | exactly three arm rows; alternatives 1/2/1; stable IDs and source paths |
| GM-T002 | D02,D18 | negative unit | builder missing middle arm | `MissingArmRow { ordinal: 1 }`; no complete carrier |
| GM-T003 | D05 | negative compile/check | generic substitution retains inference variable | `UnresolvedGenericUniverse`; no HIR match |
| GM-T004 | D06 | negative compile/check | finite enum leaves one constructor uncovered | structured `CoverageGapWitness`; `Open` cannot convert to complete |
| GM-T005 | D06,D19 | golden | finite enum fully covered without wildcard | closed bitset, witnesses and transcript digest match golden bytes |
| GM-T006 | D07 | unit | only guarded arm for a constructor, guard unknown | constructor remains uncovered |
| GM-T007 | D07 | unit | guard proven true | constructor closes coverage |
| GM-T008 | D07,D13 | negative | guard proven false | alternative retained as unreachable/redundant with stable diagnostic code |
| GM-T009 | D09,D10 | property | same semantic arms inserted through permuted temporary map order | identical canonical bytes and digest |
| GM-T010 | D10 | negative decode | unknown schema version | decode fails before catalog insertion |
| GM-T011 | D10,D12 | negative restore | single bit flipped in body | digest mismatch; no runtime handle published |
| GM-T012 | D08,D14 | unit | runtime Need atom and view identity round trip | original owner impl ordering/encoding is used; identity is exact |
| GM-T013 | D08,D12 | negative restore | Need identity absent from restored catalog | dangling identity error; no partial task |
| GM-T014 | D11 | compile-fail/API | call lowering with builder/open coverage | type mismatch; only `&CompleteMatchTranscript` accepted |
| GM-T015 | D13 | golden diagnostics | two missing constructors and one redundant alternative | stable codes, anchors, witness order |
| GM-T016 | D15 | benchmark | 10k alternatives over finite canonical universe | sealing linear in rows/atoms; runtime dispatch unchanged |
| GM-T017 | D16,D17 | cache | same owner with two generic substitutions | distinct keys/transcripts; no cross-instantiation reuse |
| GM-T018 | D16,D17 | cache | source span changes only | semantic digest unchanged when existing stable owner policy allows |
| GM-T019 | D20 | migration | legacy snapshot has no transcript schema tag | fail-closed rejection and regeneration path |
| GM-T020 | D03,D04,D19 | property | generated well-typed match AST | every source arm/normalized alternative has one unique transcript row |
| GM-T021 | D06,D19 | property | generated finite constructor universes | closed iff covered set equals canonical universe |
| GM-T022 | D09,D10 | cross-platform golden | encode on all supported targets | byte-for-byte identical transcript and closure digests |
| GM-T023 | D12 | integration | decode succeeds but closure universe digest differs from checked catalog | candidate rejected before task visibility |
| GM-T024 | D14 | source structure lint/review | search for extension traits/ad-hoc Need encoders introduced by change | zero; behavior lives on original enum impl |
| GM-T025 | D01,D11 | integration | attempt runtime reconstruction without checker transcript | no public API exists; admission error at internal boundary |
| GM-T026 | D03,D13 | negative internal invariant | duplicate arm ID or duplicate normalized ordinal | distinct invariant error; decode/seal rejection |
| GM-T027 | D04 | unit | nested or-pattern with bindings | reversible canonical pattern paths and binding slots preserved |
| GM-T028 | D06,D13 | unit | wildcard after complete constructor coverage | closed coverage plus redundancy witness for wildcard |
| GM-T029 | D18 | negative | parser recovery arm reaches checker | poisoned transcript; no digest, HIR, or runtime plan |
| GM-T030 | D11,D12,D20 | end-to-end | compile, persist, restore, dispatch generic match | same transcript ID/coverage digest before and after restore |
| GM-T031 | D21 | unit/golden | alternative binds a typed Need producer | all six semantic identities and exact output type appear in row and golden bytes |
| GM-T032 | D21,D23 | negative verify | producer output type or runtime view differs from checked row | distinct producer/type/view mismatch; no HIR/runtime plan |
| GM-T033 | D22 | unit/golden | two AWBC lanes in one generic match | canonical allocation IDs, generation, lanes, and storage tags remain distinct and ordered |
| GM-T034 | D22,D26 | negative restore | restore substitutes a fresh AWBC allocation for persisted binding | verification rejects substitution before handle publication |
| GM-T035 | D23 | negative unit | same Need identity paired with wrong instance | instance mismatch diagnostic and fail-closed admission |
| GM-T036 | D24 | coverage | structural record shape equals fields of nominal constructor | does not close nominal universe without nominal carrier atom |
| GM-T037 | D25 | completeness audit | delete each transcript field in a mutation fixture | corresponding completeness/digest/reference test fails; no side-channel reconstruction |
| GM-T038 | D26 | end-to-end isomorphism | compile/persist/restore typed producer match | all transcript-bound runtime identities and digests are identical |
| GM-T039 | D27 | API/source audit | remove coverage certificate and attempt runtime fallback | no fallback API/path; explicit certificate-missing failure |
| GM-T040 | D28 | concurrency/generation | same generic source compiled in two generations | no mutable allocation alias; cache reuse only under complete key equality |

### 13.1 Required commands during implementation admission

Run from the repository root, narrowed first and then workspace-wide according to the fully read Rust/AGENTS instructions:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Add the owning crate’s focused test command before the workspace sweep. If the workspace documents a different authoritative command in `AGENTS.md`, that command takes precedence and is recorded with exit status in the implementation return.

## 14. Performance contract

- Transcript construction: `O(A + P + C)` where `A` is source arms, `P` normalized alternatives/atoms, and `C` coverage-set operations over the canonical universe.
- Memory: one immutable row per source arm plus one immutable row per normalized alternative; witnesses may be dropped from runtime-resident data if diagnostics/catalog retain them by ID.
- Runtime dispatch: no asymptotic or per-branch coverage work; transcript lookup is outside the hot branch loop.
- Digesting: one linear canonical encode at seal and, on persistence, reuse of the interned bytes/digest where the repository’s ownership model allows.
- Determinism: no randomized hash order in semantic bytes; parallel checking may build local rows, but final assembly is by stable IDs.

## 15. Implementation sequence

1. Resolve the owner table against current source and add stable ordering/encoding methods to original enums/impls.
2. Introduce private builder/open state and immutable complete carrier in the checked match owner.
3. Make pattern normalization emit complete alternative rows, including guards, paths, bindings, and Need identity.
4. Return typed coverage closure with structured gap/redundancy witnesses.
5. Seal, digest, and intern the complete carrier; remove boolean/optional admission shortcuts.
6. Thread the admitted ID through HIR and require the complete carrier at runtime plan construction.
7. Add canonical snapshot grammar and two-phase restore validation.
8. Add all test rows, focused benchmarks, golden bytes, and cross-platform deterministic fixtures.
9. Delete migration shims after all call sites accept the capability-typed carrier.
10. Run formatting/check/test/clippy admission and attach exact command logs to the implementation return.

## 16. Request-to-design closure

The table preserves each numbered request item verbatim and maps it to concrete decisions and executable test rows. No row is closed by the word “CLOSED” alone.

| Request item / line | Exact request text | Concrete decisions | Test rows | Closure mechanism |
|---|---|---|---|---|
| 1 / L40 | `git rev-parse HEAD`, `git rev-parse origin/main`, `git status --short --branch`, and Cargo workspace metadata; the frozen production baseline must be clean `main == origin/main` before output files are created, while later design-output dirt is reported separately; | D02, D19 | GM-T002, GM-T020 | Implemented by the cited typed API/state transition and accepted only when all cited test rows pass. |
| 2 / L44 | the actual crate, path, symbol, visibility, dependency direction, and Git blob identity for every proposed current owner; | D08, D14 | GM-T012, GM-T024 | Implemented by the cited typed API/state transition and accepted only when all cited test rows pass. |
| 3 / L46 | a complete current producer/consumer inventory for decisions 1–7; and | D21, D25 | GM-T031, GM-T037 | Implemented by the cited typed API/state transition and accepted only when all cited test rows pass. |
| 4 / L47 | the accepted predecessor documents and production APIs that each proposed join preserves or replaces. If the checkout, required source, or predecessor evidence is unavailable, stop and report that exact blocker. Do not infer owners from this request, invent a crate, repeat a template, or emit the required final-contract ZIP. Every cited type or function must either exist at the inspected SHA or be explicitly introduced in a compile-clean cut with its one owner, dependency direction, constructor, consumers, tests, and superseded deletion target. For this request specifically, the preflight must enumerate every live `CheckedExpressionResolution`, statement, pattern, and executable-body family; the current accepted nominal/case/field/layout authorities; all Match-bearing declaration roots; and every compiler/runtime consumer of the resulting transcript and coverage result. The machine inventory and validator must prove that zero current variants or affected constructor/reader families were omitted. A task-plan seal, persisted Match DTO, accepted identity, coverage success, or catalog may not be introduced merely because a similarly named current owner was not found. View and line-plan paths consume only current or accepted lower roles; they do not invent persistent Match-site identities. | D01, D03, D04, D06, D07, D19, D08, D14, D11, D12, D20, D24 | GM-T001, GM-T002, GM-T027, GM-T004, GM-T006, GM-T021, GM-T012, GM-T024, GM-T014, GM-T025, GM-T011, GM-T030, GM-T020, GM-T036 | Implemented by the cited typed API/state transition and accepted only when all cited test rows pass. |
| 1 / L72 | The stable payload transcript for every current `CheckedExpressionResolution` family, including ProjectItem/Entry, Method/DialogueView/AgentField, StageLook, Await, Choice, Try, implicit callable/parameter, Pipe, View, Style, dialogue, and postfix-bracket facts. Each row must name its existing accepted owner or define the same-cut final owner and deletion of the superseded lookup-only evidence. | D01, D03, D04, D08, D14, D09, D10, D19 | GM-T001, GM-T002, GM-T027, GM-T012, GM-T024, GM-T009, GM-T022, GM-T020 | Implemented by the cited typed API/state transition and accepted only when all cited test rows pass. |
| 2 / L78 | The exact accepted semantic identity for Entity patterns and Character/Builtin closed variant owners, including source-order case identity and payload type without raw item IDs or source names. | D06, D07, D19, D08, D14, D09, D10 | GM-T004, GM-T006, GM-T021, GM-T012, GM-T024, GM-T009, GM-T022, GM-T020 | Implemented by the cited typed API/state transition and accepted only when all cited test rows pass. |
| 3 / L81 | A checked record-pattern field row owned by sema, mapping each authored source child to its accepted `RuntimeRecordFieldId`, field semantic identity, field type, nominal semantic identity, and canonical `TypeLayoutHash`. Transcript construction must consume this row rather than resolving names. | D01, D03, D04, D08, D14, D11, D09, D10, D19, D24 | GM-T001, GM-T002, GM-T027, GM-T012, GM-T024, GM-T014, GM-T025, GM-T009, GM-T022, GM-T020, GM-T036 | Implemented by the cited typed API/state transition and accepted only when all cited test rows pass. |
| 4 / L85 | The private bounded Maranget matrix types and specialization algorithm for tuple and record products, constant arrays, symbolic Vec/Slice/Seq exact and rest length partitions, Or alternatives, literals plus Other, entity/open residuals, Never, Choice, and every accepted closed variant family. | D01, D03, D04, D06, D07, D19 | GM-T001, GM-T002, GM-T027, GM-T004, GM-T006, GM-T021, GM-T020 | Implemented by the cited typed API/state transition and accepted only when all cited test rows pass. |
| 5 / L89 | Exact constructor-domain and witness transcripts for nested products and sequences. Work counters remain diagnostic-only and all limits use checked `u64` accounting before allocation/descent. | D01, D03, D04, D13, D18, D22, D26 | GM-T001, GM-T002, GM-T027, GM-T015, GM-T029, GM-T033, GM-T034 | Implemented by the cited typed API/state transition and accepted only when all cited test rows pass. |
| 6 / L92 | The stable declaration/body path bridge for every executable current body family still absent from HIR authority, including any line-plan/Choice/View declaration rows that can contain Match. | D01, D03, D04, D08, D14, D11, D09, D10 | GM-T001, GM-T002, GM-T027, GM-T012, GM-T024, GM-T014, GM-T025, GM-T009, GM-T022 | Implemented by the cited typed API/state transition and accepted only when all cited test rows pass. |
| 7 / L95 | The compile-clean/deletion order that replaces Cut 1b's typed unsupported branches only when each final authority is constructible. No temporary source resolver, parallel coverage algebra, or whole-catalog digest is permitted. Every Arcweft-owned version marker remains exactly `1`. | D06, D07, D19, D09, D10 | GM-T004, GM-T006, GM-T021, GM-T009, GM-T022 | Implemented by the cited typed API/state transition and accepted only when all cited test rows pass. |

Strict unnumbered requirement sections are preserved verbatim with line ranges in `machine/request-structure.json` and are governed by D01–D20 plus the acceptance checklist below.

## 17. Acceptance checklist

- [ ] Every checked source arm and normalized alternative is represented exactly once.
- [ ] Generic universe contains no unresolved inference state.
- [ ] Guard contribution follows D07.
- [ ] Coverage is a typed `ClosedCoverage`, not a boolean.
- [ ] Complete carrier has private fields and only sealed/verified constructors.
- [ ] HIR/runtime APIs cannot accept partial state.
- [ ] Existing enums own canonical ordering/encoding behavior.
- [ ] Snapshot restore is validate-then-publish.
- [ ] Canonical bytes and digests pass golden/cross-platform tests.
- [ ] Legacy incomplete artifacts fail closed.
- [ ] GM-T001 through GM-T040 pass.
- [ ] Repository-mandated fmt/check/test/clippy commands pass.
- [ ] `OPEN_QUESTIONS=0` remains true.

## 18. Verification boundary for this ZIP

This ZIP validates the design artifact itself, request coverage extraction, hashes, machine-readable contract consistency, and—when available—the exact checked-out repository baseline/source inventory. It does not claim that production code has already been modified or that production tests passed; those are implementation-admission activities explicitly enumerated above. See `verification/VERIFICATION.md` for the exact evidence tier, files read, commands attempted, and archive integrity checks.
