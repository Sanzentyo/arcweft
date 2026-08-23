# Concrete Rust API and owner placement

Baseline: `UNMATERIALIZED`  
Verification tier: `V3_REQUEST_AND_CONTRACT_ONLY`

## Owner table

| Concern | Existing path | Status | Placement |
|---|---|---|---|
| checked generic-match certification owner | `(not materialized)` | NOT_MATERIALIZED | extend the existing match checker/certifier owner; no sidecar helper trait |
| coverage algebra and closure owner | `(not materialized)` | NOT_MATERIALIZED | add closure variants/methods to the original coverage enum/impl |
| HIR carrier owner | `(not materialized)` | NOT_MATERIALIZED | carry only CompleteMatchTranscriptId/ClosedCoverageId after admission |
| runtime match admission owner | `(not materialized)` | NOT_MATERIALIZED | reject plans without a complete sealed transcript before execution/persistence |
| Need identity mapping owner | `(not materialized)` | NOT_MATERIALIZED | reuse existing Need identity type; transcript stores identity, never an ad-hoc integer |

## Normative API


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


## Canonical byte grammar


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

