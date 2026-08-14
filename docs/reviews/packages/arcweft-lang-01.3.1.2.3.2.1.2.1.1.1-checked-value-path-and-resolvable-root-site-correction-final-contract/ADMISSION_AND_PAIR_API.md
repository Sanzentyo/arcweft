# Exact raw admission, pair-correlation, and execution-wrapper API

This file closes the owner/API surface used by Decisions 04–07. It retains the parent names `AdmittedRuntimePlan` and `AdmittedAwbcProduct`; there is no `AdmittedAwbcProgram` alias.

## One admitted-generation parent

Owner: `arcweft_core::plan::admission`.

```rust
#[derive(Clone, Debug)]
pub struct AdmittedRuntimeGeneration(
    std::sync::Arc<AdmittedRuntimeGenerationInner>,
);

struct AdmittedRuntimeGenerationInner {
    declaration: RuntimeGenerationContractDeclaration,
    project_facts: Box<[RuntimeProjectRootFact]>,
    producer_facts: Box<[RuntimeProducerFact]>,
    nominal_catalog: AdmittedRuntimeNominalCatalog,
}

impl AdmittedRuntimeGeneration {
    pub const fn declaration(&self) -> &RuntimeGenerationContractDeclaration;
    pub const fn identity(&self) -> RuntimeGenerationIdentity;
    pub(crate) fn same_parent(&self, other: &Self) -> bool;
}
```

`same_parent` is `Arc::ptr_eq`; it is not semantic equality reconstructed from a scalar. The sole private admission constructor builds the complete inner value after the retained generation-contract/root/catalog checks. Cloning the handle preserves one immutable parent. It has no Serde, Default, public fields, `Deref`, `into_inner`, constructor from `RuntimeGenerationIdentity`, or mutable catalog access.

## Raw plan and AWBC admission

Owners: `arcweft_core::plan::admission` and `arcweft_core::awbc::admission`.

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
    checked_type: RuntimeCheckedType,
    authority: RuntimeResolvedTypeAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeResolvedTypeAuthority {
    Project { root: RuntimeProjectRootId },
    Producer {
        producer: RuntimeOpaqueTypeProducerId,
        root: RuntimeProducerRootId,
    },
}

impl RuntimePlan {
    pub fn try_admit(
        self,
    ) -> Result<AdmittedRuntimePlan, RuntimePlanAdmissionError>;
}

impl AwbcProgram {
    pub fn try_admit(
        self,
    ) -> Result<AdmittedAwbcProduct, AwbcAdmissionError>;

    pub(crate) fn try_admit_in_generation(
        self,
        generation: &AdmittedRuntimeGeneration,
    ) -> Result<AdmittedAwbcProduct, AwbcAdmissionError>;
}

impl AdmittedRuntimePlan {
    pub const fn plan(&self) -> &RuntimePlan;
    pub const fn generation(&self) -> &AdmittedRuntimeGeneration;

    pub(crate) fn resolved_type(
        &self,
        site: &RuntimePlanTypedSite,
    ) -> Result<&RuntimeResolvedType, RuntimePlanSiteResolutionError>;

    pub fn project_nominal_domain(
        &self,
        site: &RuntimePlanTypedSite,
    ) -> Result<RuntimeNominalRecordAdmissionDomain<'_>,
                RuntimePlanSiteResolutionError>;

    pub fn try_admit_awbc(
        self,
        program: AwbcProgram,
    ) -> Result<AdmittedRuntimeProduct, RuntimeProductAdmissionError>;
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

Public raw `try_admit` constructs the one generation from that artifact's mandatory generation declaration and then delegates to the private in-generation path. Pair admission consumes the already admitted plan and calls only `try_admit_in_generation(plan.generation())`; the AWBC artifact cannot select another parent. `RuntimeResolvedType` rows are derived after actual-owner traversal and are never serialized, deserialized, caller-constructed, or used as raw input.

## Pair wrapper and equality evidence

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
}
```

Construction is private to `AdmittedRuntimePlan::try_admit_awbc`. It first requires `same_parent`, independently resolves all actual plan and AWBC sites, validates the exact direct equality transcript, then builds sorted unique correlation rows. The correlation stores coordinates only; it does not persist a digest, semantic ID, checked type, root map, or optional authority. The wrapper has no Serde, Default, public constructor/fields, `Deref`, plan/AWBC extraction, or raw replacement.

## Product-step execution cut

The current product-step consumer becomes exact retained-parent API:

```rust
pub struct AwbcProductStepExecutor {
    product: AdmittedAwbcProduct,
    // current fiber/facade/root/session state
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

`program()` and raw `replace_program_preserving_state` are deleted. Replacement first checks identical admitted-generation parent/contract and full state compatibility on a candidate clone, then commits once. A failed replacement leaves product, fiber, budget, root, host, and facade state unchanged. Cross-generation replacement is handled only by runtime-driver hot-swap policy and cannot call this method.
