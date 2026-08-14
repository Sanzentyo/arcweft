# Exact admission, pair, catalog, driver, hot-swap, and restore API

## Core admitted wrappers

Owners:

- `arcweft_core::plan::generation_admission`;
- `arcweft_core::plan::admission`;
- `arcweft_core::awbc::admission`;
- `arcweft_core::awbc::product`.

```rust
#[derive(Clone, Debug)]
pub struct AdmittedRuntimePlan {
    plan: RuntimePlan,
    generation: AdmittedRuntimeGeneration,
    resolved_sites: std::collections::BTreeMap<
        RuntimePlanTypedSite,
        RuntimeResolvedType,
    >,
}

#[derive(Clone, Debug)]
pub struct AdmittedAwbcProduct {
    program: AwbcProgram,
    generation: AdmittedRuntimeGeneration,
    resolved_sites: std::collections::BTreeMap<
        AwbcTypedSite,
        RuntimeResolvedType,
    >,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeResolvedType {
    semantic_identity: RuntimeSemanticTypeId,
    kind: RuntimeResolvedTypeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeResolvedTypeKind {
    Checked {
        checked_type: RuntimeCheckedType,
        authority: RuntimeResolvedTypeAuthority,
    },
    Operational(RuntimeOperationalType),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeResolvedTypeAuthority {
    Project { root: RuntimeProjectRootId },
    Producer {
        producer: RuntimeOpaqueTypeProducerId,
        root: RuntimeProducerRootId,
    },
}

impl AdmittedRuntimeGeneration {
    pub fn try_admit_plan(
        &self,
        plan: RuntimePlan,
    ) -> Result<AdmittedRuntimePlan, RuntimePlanAdmissionError>;

    pub fn try_admit_awbc(
        &self,
        program: AwbcProgram,
    ) -> Result<AdmittedAwbcProduct, AwbcAdmissionError>;
}

impl AdmittedRuntimePlan {
    pub const fn plan(&self) -> &RuntimePlan;
    pub const fn generation(&self) -> &AdmittedRuntimeGeneration;

    pub fn try_admit_awbc(
        self,
        program: AwbcProgram,
    ) -> Result<AdmittedRuntimeProduct, RuntimeProductAdmissionError>;

    pub(crate) fn resolved_type(
        &self,
        site: &RuntimePlanTypedSite,
    ) -> Result<&RuntimeResolvedType, RuntimePlanSiteResolutionError>;
}

impl AdmittedAwbcProduct {
    pub const fn program(&self) -> &AwbcProgram;
    pub const fn generation(&self) -> &AdmittedRuntimeGeneration;

    pub(crate) fn resolved_type(
        &self,
        site: &AwbcTypedSite,
    ) -> Result<&RuntimeResolvedType, AwbcTypedSiteResolutionError>;
}
```

There is no method on `RuntimePlan` or `AwbcProgram` that returns an admitted
wrapper. The raw generation declaration is checked as step 2, after raw limits
and before type declaration/site resolution. It is never used to build or
modify the admitted parent.

## Pair correlation

```rust
#[derive(Clone, Debug)]
pub struct AdmittedRuntimeProduct {
    plan: AdmittedRuntimePlan,
    awbc: AdmittedAwbcProduct,
    correlation: RuntimePlanAwbcCorrelation,
}

#[derive(Clone, Debug)]
struct RuntimePlanAwbcCorrelation {
    rows: Box<[RuntimePlanAwbcCorrelationRow]>,
}

#[derive(Clone, Debug)]
struct RuntimePlanAwbcCorrelationRow {
    plan_site: RuntimePlanTypedSite,
    awbc_sites: Box<[AwbcTypedSite]>,
}

impl AdmittedRuntimeProduct {
    pub const fn plan(&self) -> &AdmittedRuntimePlan;
    pub const fn awbc(&self) -> &AdmittedAwbcProduct;
    pub const fn generation(&self) -> &AdmittedRuntimeGeneration;

    pub fn nominal_record_domain(
        &self,
        origin: &AwbcTypedOrigin,
        domain: AwbcNominalRecordDomainId,
    ) -> Result<RuntimeNominalRecordAdmissionDomain<'_>,
                RuntimeProductAdmissionError>;

    pub fn checked_value_context(
        &self,
        origin: &AwbcTypedOrigin,
        limits: RuntimeCheckedValueLimits,
    ) -> Result<RuntimeCheckedValueContext<'_>,
                RuntimeProductAdmissionError>;
}
```

`AdmittedRuntimePlan::try_admit_awbc` first calls
`generation.try_admit_awbc(program)`, then calls
`generation.require_same_parent(awbc.generation())`, then validates every
sorted unique coordinate-only origin. `require_same_parent` is implemented by
`Arc::ptr_eq` on the private inner Arcs. Checked
rows require exact semantic identity, checked type, and authority equality.
Operational plan node rows cannot occur in `AwbcTypedOrigin`; an origin naming
one is `OperationalOriginForbidden`. Correlation rows store coordinates only.

## Catalog ownership

Owner: `arcweft_dialogue::character_dialogue::catalog_admission`.

```rust
#[derive(Clone, Copy, Debug)]
pub struct CharacterDialogueGenerationCatalogs<'generation> {
    generation: &'generation AdmittedRuntimeGeneration,
    characters: &'generation CharacterCatalog,
    views: &'generation ViewRegistry,
    custom_fields: &'generation CharacterDialogueRuntimeCustomFieldCatalog,
}

#[derive(Clone, Debug)]
pub struct AdmittedCharacterDialogueCatalogs {
    generation: AdmittedRuntimeGeneration,
    characters: std::sync::Arc<CharacterCatalog>,
    views: std::sync::Arc<ViewRegistry>,
    custom_fields: std::sync::Arc<CharacterDialogueRuntimeCustomFieldCatalog>,
}

impl AdmittedCharacterDialogueCatalogs {
    pub fn try_admit(
        generation: &AdmittedRuntimeGeneration,
        characters: std::sync::Arc<CharacterCatalog>,
        views: std::sync::Arc<ViewRegistry>,
        custom_fields: std::sync::Arc<CharacterDialogueRuntimeCustomFieldCatalog>,
    ) -> Result<Self, CharacterDialogueCatalogAdmissionError>;

    pub const fn generation(&self) -> &AdmittedRuntimeGeneration;
    pub const fn characters(&self) -> &CharacterCatalog;
    pub const fn views(&self) -> &ViewRegistry;
    pub const fn custom_fields(
        &self,
    ) -> &CharacterDialogueRuntimeCustomFieldCatalog;

    pub fn as_borrowed(&self) -> CharacterDialogueGenerationCatalogs<'_>;
}
```

The owned wrapper validates the existing canonical Character/View/custom
digests against the borrowed generation and validates every custom-field
accepted View ID. It clones the exact generation Arc parent only after all
checks pass. Neither catalog wrapper has Serde, `Default`, public fields, or a
free generation identity constructor.

## Runtime-driver generation

Owner: `arcweft_runtime_driver::generation_runtime`.

```rust
#[derive(Clone, Debug)]
pub struct RuntimeDriverGeneration {
    product: AdmittedRuntimeProduct,
    catalogs: AdmittedCharacterDialogueCatalogs,
}

impl RuntimeDriverGeneration {
    pub fn try_new(
        product: AdmittedRuntimeProduct,
        catalogs: AdmittedCharacterDialogueCatalogs,
    ) -> Result<Self, RuntimeDriverGenerationError>;

    pub const fn product(&self) -> &AdmittedRuntimeProduct;
    pub const fn catalogs(&self) -> &AdmittedCharacterDialogueCatalogs;
    pub const fn generation(&self) -> &AdmittedRuntimeGeneration;

    pub fn for_entry(
        &self,
        entry: AwbcEntryId,
        budget_quantum: u64,
    ) -> Result<AwbcProductStepExecutor, AwbcProductStepBuildError>;
}
```

`try_new` calls the public non-forgeable
`product.generation().require_same_parent(catalogs.generation())` before any
catalog or executable state is published. It owns no raw program replacement
path.

## Product-step executor

Owner: `arcweft_core::awbc::product_step`.

```rust
pub struct AwbcProductStepExecutor {
    product: AdmittedAwbcProduct,
    // existing fiber/facade/root/session state
}

impl AwbcProductStepExecutor {
    pub fn for_entry(
        product: AdmittedAwbcProduct,
        entry: AwbcEntryId,
        budget_quantum: u64,
    ) -> Result<Self, AwbcProductStepBuildError>;

    pub fn for_function(
        product: AdmittedAwbcProduct,
        entry: AwbcEntryId,
        function: AwbcFunctionId,
        budget_quantum: u64,
    ) -> Result<Self, AwbcProductStepBuildError>;

    pub fn replace_product_preserving_state(
        &mut self,
        candidate: AdmittedAwbcProduct,
    ) -> Result<(), AwbcProductStepBuildError>;

    pub const fn product(&self) -> &AdmittedAwbcProduct;
}
```

Replacement calls `product.generation().require_same_parent(
candidate.generation())` and validates full state compatibility on a candidate
clone before one commit. A failure leaves every field unchanged.
`program()` and raw `replace_program_preserving_state` are deleted.

## Cross-generation hot swap

Owner: `arcweft_runtime_driver::swap`.

```rust
pub struct PreparedRuntimeGenerationSwap {
    expected_current: AdmittedRuntimeGeneration,
    candidate: RuntimeDriverGeneration,
    migration: RuntimeStateMigrationPlan,
}

impl RuntimeSession {
    pub fn prepare_generation_swap(
        &self,
        candidate: RuntimeDriverGeneration,
        policy: RuntimeGenerationSwapPolicy,
    ) -> Result<PreparedRuntimeGenerationSwap, RuntimeGenerationSwapError>;

    pub fn commit_generation_swap(
        &mut self,
        prepared: PreparedRuntimeGenerationSwap,
    ) -> Result<(), RuntimeGenerationSwapError>;
}

#[derive(Debug, Error)]
pub enum RuntimeGenerationSwapError {
    #[error(transparent)]
    Candidate(#[from] RuntimeDriverGenerationError),
    #[error(transparent)]
    Migration(#[from] RuntimeStateMigrationError),
    #[error("prepared runtime-generation swap no longer targets the current issued parent")]
    StalePreparedSwap {
        expected: RuntimeGenerationIdentity,
        actual: RuntimeGenerationIdentity,
    },
}
```

Prepare validates candidate generation/catalogs/product, resource/view/entry
mapping, and a complete state migration without mutating the session. Commit
calls `self.generation().require_same_parent(&prepared.expected_current)`
before rechecking the migration and then swaps all generation-bound state once.
Prepare stores a clone of the exact current admitted parent, not only its
scalar identity, so an ABA replacement or a separately reissued byte-equal
generation is rejected. Same-parent replacement never uses this API;
cross-parent replacement never uses the product-step method.

## Generation-first bundle/load/restore

Owners:

- `arcweft_bundle::product_awbc::runtime_generation` owns verification of the
  product AWFB section set and the non-forgeable owned section token;
- `arcweft_runtime_driver::generation_runtime` owns catalog input and ordered
  generation loading; and
- `arcweft-save` owns fixed save/replay headers.

```rust
pub struct VerifiedRuntimeGenerationSections {
    generation_facts: Box<[u8]>,
    plan: Box<[u8]>,
    awbc: Box<[u8]>,
    product_manifest_digest: BundleDigest,
}

pub fn verify_runtime_generation_sections(
    product_awfb: &[u8],
    external_sections: &[ExternalSectionPayload],
    budget: ReadBudget,
) -> Result<VerifiedRuntimeGenerationSections,
            RuntimeGenerationSectionVerificationError>;

impl VerifiedRuntimeGenerationSections {
    pub fn decode_generation_projection(
        &self,
        limits: RuntimeGenerationFactDecodeLimits,
    ) -> Result<RuntimeGenerationAdmissionProjection,
                RuntimeGenerationFactSectionError>;

    pub fn decode_runtime_plan(
        &self,
        limits: RuntimePlanDecodeLimits,
    ) -> Result<RuntimePlan, RuntimePlanWireError>;

    pub fn decode_awbc_program(
        &self,
        limits: AwbcProgramDecodeLimits,
    ) -> Result<AwbcProgram, AwbcProgramWireError>;

    pub const fn product_manifest_digest(&self) -> BundleDigest;
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeGenerationSectionVerificationError {
    #[error(transparent)]
    Container(#[from] ContainerError),
    #[error(transparent)]
    ExternalPayload(#[from] ExternalSectionPayloadError),
    #[error("runtime-generation load requires a Program or AgentController AWFB, got {actual:?}")]
    WrongBundleKind { actual: BundleKind },
    #[error("missing required runtime-generation AWFB section {kind:?}")]
    MissingRequiredSection { kind: BundleSectionKind },
    #[error("duplicate required runtime-generation AWFB section {kind:?}")]
    DuplicateRequiredSection { kind: BundleSectionKind },
    #[error("runtime-generation AWFB section {kind:?} has schema version {actual}, expected 1")]
    WrongSectionSchemaVersion {
        kind: BundleSectionKind,
        actual: u32,
    },
}

pub struct RuntimeGenerationCatalogInput {
    characters: std::sync::Arc<CharacterCatalog>,
    views: std::sync::Arc<ViewRegistry>,
    custom_fields: std::sync::Arc<CharacterDialogueRuntimeCustomFieldCatalog>,
}

impl RuntimeGenerationCatalogInput {
    pub fn new(
        characters: std::sync::Arc<CharacterCatalog>,
        views: std::sync::Arc<ViewRegistry>,
        custom_fields: std::sync::Arc<CharacterDialogueRuntimeCustomFieldCatalog>,
    ) -> Self;
}

pub struct RuntimeGenerationLoadInput {
    sections: VerifiedRuntimeGenerationSections,
    catalogs: RuntimeGenerationCatalogInput,
}

impl RuntimeGenerationLoadInput {
    pub fn new(
        sections: VerifiedRuntimeGenerationSections,
        catalogs: RuntimeGenerationCatalogInput,
    ) -> Self;
}

#[derive(Debug, Error)]
pub enum RuntimeGenerationLoadError {
    #[error(transparent)]
    GenerationFacts(#[from] RuntimeGenerationFactSectionError),
    #[error(transparent)]
    GenerationAdmission(#[from] RuntimeGenerationAdmissionError),
    #[error(transparent)]
    CatalogAdmission(#[from] CharacterDialogueCatalogAdmissionError),
    #[error(transparent)]
    PlanWire(#[from] RuntimePlanWireError),
    #[error(transparent)]
    PlanAdmission(#[from] RuntimePlanAdmissionError),
    #[error(transparent)]
    AwbcWire(#[from] AwbcProgramWireError),
    #[error(transparent)]
    ProductAdmission(#[from] RuntimeProductAdmissionError),
    #[error(transparent)]
    DriverGeneration(#[from] RuntimeDriverGenerationError),
}

pub fn try_load_runtime_generation(
    input: RuntimeGenerationLoadInput,
) -> Result<RuntimeDriverGeneration, RuntimeGenerationLoadError>;

impl RuntimeSession {
    pub fn restore(
        generation: RuntimeDriverGeneration,
        snapshot: &[u8],
    ) -> Result<Self, RuntimeRestoreError>;

    pub fn replay(
        generation: RuntimeDriverGeneration,
        log: &[u8],
    ) -> Result<Self, RuntimeReplayError>;
}
```

`VerifiedRuntimeGenerationSections` has private fields, no Serde, `Clone`,
`Default`, public constructor, raw-byte accessor, or section replacement API.
It owns the three decoded section byte buffers so embedded, compressed, and
externally supplied sections share one lifetime-safe boundary. The sole
verifier delegates container-level magic/version/header/index/bounds/overlap/
stored-content-index-manifest digest and external-payload checks to the
existing `BundleView`/`ContainerError` owners, then requires exactly one
`RuntimeGenerationFacts`, exactly one `RuntimePlan`, and exactly one existing
`ProgramBytecode` section, each with schema version `1`. Its public methods
decode only into checked core DTOs/projection; none returns or accepts a
replaceable byte slice. The generation-fact section has its own private
version-1 wire DTO and is converted into the non-Serde core projection through
checked constructors. It is authored from the accepted world by the compiler
and is never reconstructed from the plan/AWBC sections. The exact section tags
and payload grammar are in `BUNDLE_RUNTIME_GENERATION_SECTIONS.md`; the load
order is in `RESTORE_ORDER.csv`. No plan, AWBC, snapshot/replay value, or
executable payload is decoded before generation issuance.
