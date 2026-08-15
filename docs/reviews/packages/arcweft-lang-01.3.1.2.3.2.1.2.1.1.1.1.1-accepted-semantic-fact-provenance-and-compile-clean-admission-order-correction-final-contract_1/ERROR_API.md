# Exact new error owners and precedence

The enums below are the complete new observable error families introduced by
this correction. Existing child errors such as `RuntimePlanTypeTableError`,
`RuntimePatternBindingPathError`, checked-value errors, catalog errors, and
bundle envelope/signature errors are retained and nested without string
flattening.

## Core path, builder, and generation errors

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeIndexPathError {
    #[error("runtime index path must contain root index 0")]
    Empty,
    #[error("runtime index path root must be 0")]
    RootNotZero { actual: u32 },
    #[error("runtime index path exceeds depth {limit}")]
    TooDeep { depth: usize, limit: usize },
    #[error("runtime child ordinal does not fit u32")]
    OrdinalOverflow { ordinal: usize },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimePlanBuildError {
    #[error(transparent)]
    IndexPath(#[from] RuntimeIndexPathError),
    #[error("typed node path is duplicated")]
    DuplicateNodePath { path: RuntimeIndexPath },
    #[error("runtime node has no accepted type row")]
    MissingNodePath { path: RuntimeIndexPath },
    #[error("accepted type row has no runtime node")]
    ExtraNodePath { path: RuntimeIndexPath },
    #[error(transparent)]
    TypeTable(#[from] RuntimePlanTypeTableError),
    #[error("pattern binding references an unknown runtime local")]
    UnknownRuntimeLocal { local: RuntimeLocalDeclarationId },
    #[error("pattern binding coordinate is duplicated")]
    DuplicateBindingCoordinate { coordinate: RuntimePatternBindingCoordinate },
    #[error("pattern binding path does not select the declared node type")]
    BindingTypeMismatch { coordinate: RuntimePatternBindingCoordinate },
    #[error("typed publication site is duplicated")]
    DuplicateTypedSite { site: RuntimePlanTypedSite },
    #[error("typed publication site cannot resolve its referenced node")]
    UnresolvedTypedSite { site: RuntimePlanTypedSite },
    #[error("runtime plan collection ID space is exhausted")]
    CollectionIdExhausted { collection: RuntimePlanCollectionKind },
    #[error("runtime plan finish found an unbound required typed site")]
    MissingRequiredTypedSite { site: RuntimePlanTypedSite },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeGenerationFactError {
    #[error("project root checked type does not match its semantic identity")]
    ProjectRootTypeMismatch { semantic: RuntimeSemanticTypeId },
    #[error("producer fact owner does not match its producer")]
    ProducerOwnerMismatch { producer: RuntimeOpaqueTypeProducerId },
    #[error("canonical fact key is duplicated with unequal payload")]
    ConflictingDuplicate { table: RuntimeGenerationFactTable, key: RuntimeSemanticTypeId },
    #[error("generation fact table exceeds its fixed limit")]
    LimitExceeded { table: RuntimeGenerationFactTable, actual: usize, limit: usize },
    #[error(transparent)]
    Catalog(#[from] RuntimeCatalogFactError),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimePlanAdmissionError {
    #[error("runtime plan generation declaration differs from the independent parent")]
    GenerationMismatch,
    #[error("runtime plan root fact is absent from the independent generation")]
    UnknownRoot { site: RuntimePlanTypedSite },
    #[error("runtime plan typed site differs from the independently admitted root type")]
    TypedSiteMismatch { site: RuntimePlanTypedSite },
    #[error("runtime plan type ID is unresolved")]
    UnknownType { ty: RuntimePlanTypeId },
    #[error("runtime plan local declaration is unresolved")]
    UnknownLocal { local: RuntimeLocalDeclarationId },
    #[error("runtime plan violates a final structural invariant")]
    Structural { kind: RuntimePlanStructuralErrorKind },
}
```

## Synthetic fact errors

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeSyntheticFactError {
    #[error("synthetic fact belongs to a stale HIR snapshot")]
    StaleSnapshot,
    #[error("synthetic fact belongs to the wrong semantic world")]
    WrongWorld,
    #[error("synthetic fact owner family is unresolved or wrong")]
    WrongOwnerFamily,
    #[error("synthetic fact is duplicated")]
    Duplicate { site: RuntimeSyntheticSite },
    #[error("required synthetic fact is missing")]
    Missing { site: RuntimeSyntheticSite },
    #[error("unexpected synthetic fact was submitted")]
    Unexpected { site: RuntimeSyntheticSite },
    #[error("synthetic fact type differs from the accepted owner projection")]
    TypeMismatch { site: RuntimeSyntheticSite },
    #[error("accepted synthetic type has no runtime representation")]
    UnsupportedRuntimeShape { site: RuntimeSyntheticSite },
}
```

## AWBC, product, and publication errors

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AwbcProgramBuildError {
    #[error("AWBC header is not canonical version 1")]
    InvalidHeader,
    #[error("nominal-record domain table exceeds 262144 rows")]
    NominalRecordDomainLimit { actual: usize },
    #[error("one nominal-record domain origin was submitted with different types")]
    ConflictingNominalRecordDomain { origin: AwbcNominalRecordDomainOrigin },
    #[error("AWBC table ID space is exhausted")]
    TableIdExhausted { table: AwbcTableKind },
    #[error("AWBC draft refers to an unknown nominal-record domain handle")]
    UnknownNominalRecordDomainHandle { handle: AwbcNominalRecordDomainHandle },
    #[error("record field count differs from the selected construction type")]
    RecordFieldCountMismatch { expected: usize, actual: usize },
    #[error("AWBC final rewrite failed to resolve a staged handle")]
    UnresolvedDraftHandle { handle: AwbcNominalRecordDomainHandle },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AwbcAdmissionError {
    #[error("AWBC generation parent differs from the admitted plan parent")]
    ParentMismatch,
    #[error("AWBC plan admission key differs from the admitted plan")]
    PlanKeyMismatch,
    #[error("AWBC nominal-record domain table is not in canonical encoded order")]
    NonCanonicalNominalRecordDomainOrder,
    #[error("AWBC nominal-record domain is duplicated")]
    DuplicateNominalRecordDomain,
    #[error("AWBC nominal-record domain origin cannot be resolved")]
    UnresolvedNominalRecordDomain { id: AwbcNominalRecordDomainId },
    #[error("AWBC nominal-record domain root does not correlate with the parent")]
    NominalRecordDomainRootMismatch { id: AwbcNominalRecordDomainId },
    #[error("AWBC nominal-record domain type is not the exact admitted nominal record")]
    NominalRecordDomainTypeMismatch { id: AwbcNominalRecordDomainId },
    #[error("AWBC record operand refers to an unknown domain")]
    UnknownNominalRecordDomain { id: AwbcNominalRecordDomainId },
    #[error("AWBC structural verifier rejected the program")]
    Structural { kind: AwbcStructuralErrorKind },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProductAdmissionError {
    #[error("plan and AWBC do not share the exact Arc generation parent")]
    ParentMismatch,
    #[error("plan and AWBC do not share the exact Arc plan admission key")]
    PlanKeyMismatch,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeBundleEvidenceError {
    #[error("publication policy requires trusted Ed25519 authentication")]
    TrustedSignatureRequired,
    #[error("verified generation identity differs from the admitted product parent")]
    GenerationIdentityMismatch {
        evidence: RuntimeGenerationIdentity,
        product: RuntimeGenerationIdentity,
    },
    #[error("verified plan digest differs from the admitted plan bytes")]
    PlanDigestMismatch { evidence: RuntimeDigest, actual: RuntimeDigest },
    #[error("verified AWBC digest differs from the admitted AWBC bytes")]
    AwbcDigestMismatch { evidence: RuntimeDigest, actual: RuntimeDigest },
    #[error("verified container digest differs from the retained bundle bytes")]
    ContainerDigestMismatch { evidence: RuntimeDigest, actual: RuntimeDigest },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimePublicationError {
    #[error(transparent)]
    Evidence(#[from] RuntimeBundleEvidenceError),
    #[error("verified product correlation failed")]
    ProductCorrelation,
    #[error("required canonical catalog or resource section is unavailable")]
    MissingExternalSection { section: RuntimeExternalSectionKind },
    #[error("host binding does not satisfy an admitted producer/effect capability")]
    HostBinding { binding: RuntimeHostBindingKind },
    #[error("selected execution backend cannot represent an admitted operational type")]
    UnsupportedBackendType { site: RuntimePlanTypedSite },
    #[error("publication limit is exceeded")]
    Limit { kind: RuntimePublicationLimitKind },
    #[error("publication generation changed before atomic commit")]
    StalePublication,
}
```

## Fixed precedence

Plan construction checks path form, complete path-set equality, type projection,
type-table conflicts/capacity, bindings, typed-site uniqueness, required sites,
then commits. Generation admission checks canonical table structure, duplicates,
limits, roots, catalogs, then identity. AWBC decode follows the exact order in
`AWBC_V1_WIRE_GRAMMAR.md`; product pairing checks `Arc::ptr_eq` parent before
plan-key equality. Publication, hot swap, restore, and replay use the order in
`TRUST_PROVENANCE_AND_PUBLICATION.md`. No child error is converted to a string.
