# Independent generation projection and issuance API

## Core generation contract

Owners:

- `crates/arcweft-core/src/plan/generation_contract.rs` for scalar newtypes and
  `RuntimeGenerationContractDeclaration`;
- `crates/arcweft-core/src/plan/generation_admission.rs` for projection and
  admitted generation.

```rust
pub const RUNTIME_GENERATION_CONTRACT_VERSION: u16 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RuntimeGenerationContractDeclaration {
    contract_version: u16,
    identity: RuntimeGenerationIdentity,
    nominal_catalog: RuntimeNominalCatalogDigest,
    character_catalog: RuntimeCharacterCatalogDigest,
    view_catalog: RuntimeViewCatalogDigest,
    dialogue_custom_fields: CharacterDialogueRuntimeCustomFieldDigest,
}

impl RuntimeGenerationContractDeclaration {
    pub const fn contract_version(&self) -> u16;
    pub const fn identity(&self) -> RuntimeGenerationIdentity;
    pub const fn nominal_catalog(&self) -> RuntimeNominalCatalogDigest;
    pub const fn character_catalog(&self) -> RuntimeCharacterCatalogDigest;
    pub const fn view_catalog(&self) -> RuntimeViewCatalogDigest;
    pub const fn dialogue_custom_fields(
        &self,
    ) -> CharacterDialogueRuntimeCustomFieldDigest;
}
```

The declaration constructor is private to successful generation issuance.
Its custom `Deserialize` validates version exactly `1`, but decoding a
declaration never issues a generation.

## Owned non-Serde projection rows

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeAcceptedProjectTypeKind {
    Checked(RuntimeCheckedType),
    Operational(RuntimeOperationalType),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectTypeProjection {
    semantic_identity: RuntimeSemanticTypeId,
    kind: RuntimeAcceptedProjectTypeKind,
}

impl RuntimeProjectTypeProjection {
    pub fn checked(
        semantic_identity: RuntimeSemanticTypeId,
        checked_type: RuntimeCheckedType,
    ) -> Self;

    pub fn operational(
        semantic_identity: RuntimeSemanticTypeId,
        shape: RuntimeOperationalType,
    ) -> Self;

    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn kind(&self) -> &RuntimeAcceptedProjectTypeKind;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProducerTypeProjection {
    producer: RuntimeOpaqueTypeProducerId,
    semantic_identity: RuntimeSemanticTypeId,
    owner: RuntimeOpaqueTypeOwner,
}

impl RuntimeProducerTypeProjection {
    pub fn try_exact(
        owner: RuntimeOpaqueTypeOwner,
    ) -> Result<Self, RuntimeGenerationProjectionError>;
    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId;
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn owner(&self) -> &RuntimeOpaqueTypeOwner;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNominalRecordProjection {
    nominal: RuntimeNominalTypeId,
    semantic_identity: RuntimeSemanticTypeId,
    layout: TypeLayoutHash,
    fields: Box<[RuntimeNominalRecordFieldProjection]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeNominalRecordFieldProjection {
    field: RuntimeRecordFieldId,
    ty: RuntimeSemanticTypeId,
}

impl RuntimeNominalRecordProjection {
    pub fn try_new(
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
        fields: impl IntoIterator<Item = RuntimeNominalRecordFieldProjection>,
    ) -> Result<Self, RuntimeGenerationProjectionError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeGenerationCatalogProjection {
    nominal_catalog: RuntimeNominalCatalogDigest,
    character_catalog: RuntimeCharacterCatalogDigest,
    view_catalog: RuntimeViewCatalogDigest,
    dialogue_custom_fields: CharacterDialogueRuntimeCustomFieldDigest,
}

impl RuntimeGenerationCatalogProjection {
    pub fn new(
        nominal_catalog: RuntimeNominalCatalogDigest,
        character_catalog: RuntimeCharacterCatalogDigest,
        view_catalog: RuntimeViewCatalogDigest,
        dialogue_custom_fields: CharacterDialogueRuntimeCustomFieldDigest,
    ) -> Self;
}

pub struct RuntimeGenerationProjectionBuilder {
    catalogs: RuntimeGenerationCatalogProjection,
    project_types: Vec<RuntimeProjectTypeProjection>,
    producer_types: Vec<RuntimeProducerTypeProjection>,
    nominal_records: Vec<RuntimeNominalRecordProjection>,
}

impl RuntimeGenerationProjectionBuilder {
    pub fn new(catalogs: RuntimeGenerationCatalogProjection) -> Self;

    pub fn push_project_type(
        &mut self,
        fact: RuntimeProjectTypeProjection,
    ) -> Result<(), RuntimeGenerationProjectionError>;

    pub fn push_producer_type(
        &mut self,
        fact: RuntimeProducerTypeProjection,
    ) -> Result<(), RuntimeGenerationProjectionError>;

    pub fn push_nominal_record(
        &mut self,
        fact: RuntimeNominalRecordProjection,
    ) -> Result<(), RuntimeGenerationProjectionError>;

    pub fn finish(
        self,
    ) -> Result<RuntimeGenerationAdmissionProjection,
                RuntimeGenerationProjectionError>;
}

pub struct RuntimeGenerationAdmissionProjection {
    catalogs: RuntimeGenerationCatalogProjection,
    project_types: Box<[RuntimeProjectTypeProjection]>,
    producer_types: Box<[RuntimeProducerTypeProjection]>,
    nominal_records: Box<[RuntimeNominalRecordProjection]>,
}
```

`RuntimeGenerationAdmissionProjection` and every projection row have no Serde,
`Default`, public fields, `Deref`, `Clone` on the aggregate, or conversion from
raw plan/AWBC types. `finish` consumes the builder. Row structs may be cloned
for accepted-world staging, but the aggregate is single-use.

## Canonical persisted fact section

Owner: `arcweft_core::plan::generation_admission::wire`; this is a core fact
encoding, not an AWFB/container type and not Serde.

```rust
pub const RUNTIME_GENERATION_FACT_SECTION_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeGenerationFactSection(Box<[u8]>);

impl RuntimeGenerationFactSection {
    pub fn as_bytes(&self) -> &[u8];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeGenerationFactDecodeLimits {
    pub const PRODUCTION: Self = Self {
        maximum_section_bytes: 67_108_864,
        maximum_project_types: 262_144,
        maximum_producer_types: 65_536,
        maximum_nominal_records: 65_536,
        maximum_nominal_fields: 1_048_576,
        maximum_checked_type_nodes: 4_194_304,
    };

    pub fn try_new(
        maximum_section_bytes: u64,
        maximum_project_types: u32,
        maximum_producer_types: u32,
        maximum_nominal_records: u32,
        maximum_nominal_fields: u32,
        maximum_checked_type_nodes: u64,
    ) -> Result<Self, RuntimeGenerationFactLimitError>;
}

pub fn decode_runtime_generation_fact_section(
    bytes: &[u8],
    limits: RuntimeGenerationFactDecodeLimits,
) -> Result<RuntimeGenerationAdmissionProjection,
            RuntimeGenerationFactSectionError>;
```

`PRODUCTION` therefore fixes the six limits at 67,108,864 section bytes,
262,144 project rows, 65,536 producer rows, 65,536 nominal records, 1,048,576
nominal fields, and 4,194,304 checked-type nodes. `try_new` rejects a zero
limit and any field combination whose minimum framing arithmetic overflows;
all per-row and checked-type work debits use checked integer arithmetic.

The only constructor of `RuntimeGenerationFactSection` is successful
`AdmittedRuntimeGeneration::try_issue`. Issuance encodes the independently
accepted projection once, computes the generation identity from the embedded
canonical transcript, and stores those exact bytes beside the admitted facts.
The decoder accepts only the grammar in
`BUNDLE_RUNTIME_GENERATION_SECTIONS.md`, builds rows through the public checked
projection constructors, and calls `RuntimeGenerationProjectionBuilder::finish`.
There is no Serde shape, mutable bytes, unchecked constructor, conversion from
`RuntimePlan`/`AwbcProgram`, or decoder that directly returns an admitted
parent.

## Issuance

```rust
#[derive(Clone, Debug)]
pub struct AdmittedRuntimeGeneration(
    std::sync::Arc<AdmittedRuntimeGenerationInner>,
);

struct AdmittedRuntimeGenerationInner {
    declaration: RuntimeGenerationContractDeclaration,
    fact_section: RuntimeGenerationFactSection,
    project_types: Box<[AdmittedProjectTypeFact]>,
    producer_types: Box<[AdmittedProducerTypeFact]>,
    nominal_records: Box<[AdmittedNominalRecordFact]>,
}

impl AdmittedRuntimeGeneration {
    pub fn try_issue(
        projection: RuntimeGenerationAdmissionProjection,
    ) -> Result<Self, RuntimeGenerationAdmissionError>;

    pub const fn declaration(&self) -> &RuntimeGenerationContractDeclaration;
    pub const fn identity(&self) -> RuntimeGenerationIdentity;
    pub const fn fact_section(&self) -> &RuntimeGenerationFactSection;

    pub fn require_same_parent(
        &self,
        other: &Self,
    ) -> Result<(), RuntimeGenerationParentMismatch>;
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeGenerationParentMismatch {
    #[error("admitted runtime generations do not share one issued parent")]
    DifferentIssuedParent {
        left: RuntimeGenerationIdentity,
        right: RuntimeGenerationIdentity,
    },
}
```

`require_same_parent` is a public comparison capability, not a construction
capability. It performs only `Arc::ptr_eq` on the private inner Arcs and returns
`DifferentIssuedParent` otherwise. It never accepts or reconstructs a parent
from `RuntimeGenerationIdentity`; two separately issued parents fail even when
their canonical identities are byte-equal. This public method is required by
the separate dialogue and runtime-driver crates and replaces the second
friend-crate visibility defect.

`try_issue` performs this exact order:

1. projection count/work limits;
2. strict canonical project row order and duplicate detection;
3. strict canonical producer row order and duplicate detection;
4. strict canonical nominal row/field order and duplicate detection;
5. lossless project root projection from each checked semantic identity;
6. exact producer owner admission (`ExactIdentity` only) and producer root
   projection;
7. checked/operational root-shape consistency;
8. nominal layout/field type reachability against project/producer facts;
9. version-1 canonical generation transcript, fact-section bytes, and identity;
10. immutable Arc publication containing the exact fact section and admitted facts.

The admitted inner stores only core-owned rows plus the core-owned canonical
`RuntimeGenerationFactSection`. No HIR/sema/runtime-plan or AWFB type crosses
the boundary. `fact_section()` is read-only persistence evidence; it does not
construct a second parent.

## Standard accepted-world assembler

Owner: `crates/arcweft-compiler/src/project/runtime_generation.rs`.

```rust
pub(crate) struct ProjectRuntimeGenerationAssembly<'project> {
    semantic_facts: &'project RuntimePlanSemanticFacts,
    nominal_world: &'project AcceptedNominalWorld,
    opaque_producers: &'project AcceptedRuntimeOpaqueProducerRegistry,
    dialogue_roles: &'project CharacterDialogueRuntimeRoleRegistry,
    custom_fields: &'project CharacterDialogueCustomFieldRegistry,
    characters: &'project CharacterCatalog,
    views: &'project ViewRegistry,
}

impl<'project> ProjectRuntimeGenerationAssembly<'project> {
    pub(crate) fn try_new(
        semantic_facts: &'project RuntimePlanSemanticFacts,
        registered: &'project RegisteredTypeCheckEnv,
        characters: &'project CharacterCatalog,
        views: &'project ViewRegistry,
    ) -> Result<Self, ProjectRuntimeGenerationAssemblyError>;

    pub(crate) fn issue(
        self,
    ) -> Result<AdmittedRuntimeGeneration,
                ProjectRuntimeGenerationAssemblyError>;
}
```

`try_new` proves all sema registries have the same accepted nominal-world stamp,
that the semantic facts use the accepted project snapshots, and that catalog
owners are immutable. `issue` projects each accepted semantic type once,
projects exact producer facts and nominal layouts, computes the canonical
catalog digests through their existing owners, feeds the core builder in exact
order, and calls `AdmittedRuntimeGeneration::try_issue`.

No method accepts a `RuntimePlan`, `AwbcProgram`, their type declarations, or
serialized root-use map. The compiler assembly's borrow ends after issuance;
the admitted generation owns all lower-layer facts.

## Canonical generation transcript

The generation identity uses exactly one version-1 transcript:

```text
"arcweft.runtime-generation\0" || u16_le(1)
|| nominal_catalog_digest[32]
|| character_catalog_digest[32]
|| view_catalog_digest[32]
|| dialogue_custom_field_digest[32]
|| u32_le(project_count) || project rows
|| u32_le(producer_count) || producer rows
|| u32_le(nominal_count) || nominal rows
```

Project row:
`semantic_identity[32] || kind_tag:u8 || kind_payload`, where checked payload is
`RuntimeCheckedType` version-1 canonical bytes and operational payload is its
single canonical tag.

Producer row:
`producer_len:u16_le || producer_utf8 || semantic_identity[32] || exact_tag=0`.

Nominal row:
`nominal_len:u16_le || nominal_utf8 || semantic_identity[32] || layout[32] ||
u32_le(field_count) || (field_id:u32_le || field_semantic_identity[32])*`.

Lengths are checked before allocation; text IDs are already validated newtypes;
rows are strictly sorted. This is the generation identity transcript, not a
plan/AWBC correlation digest. The persisted fact-section bytes are exactly
`b"AWGF\r\n\x1a\n" || u32_le(1) || u32_le(transcript_len) || transcript`;
`transcript_len` is at most `u32::MAX`, the payload must end exactly after the
transcript, and the transcript begins with the domain/version bytes above. All
version markers remain `1`.
