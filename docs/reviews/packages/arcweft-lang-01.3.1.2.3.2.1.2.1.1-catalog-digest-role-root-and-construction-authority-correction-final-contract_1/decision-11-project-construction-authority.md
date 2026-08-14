# Decision 11 — project nominal-record construction authority

## Core owner and lifetime

Owner: `crates/arcweft-core/src/plan/nominal_admission.rs`.

```rust
#[derive(Clone, Copy, Debug)]
pub enum RuntimeNominalRecordAdmissionDomain<'generation> {
    Project(RuntimeNominalRecordProjectDomain<'generation>),
    Producer(RuntimeNominalRecordProducerShape<'generation>),
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeNominalRecordProjectDomain<'generation> {
    generation: &'generation AdmittedRuntimeGeneration,
    site: &'generation RuntimePlanTypedSite,
    root: RuntimeProjectRootId,
    closure: &'generation RuntimeNominalAuthorizationClosure,
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum RuntimeNominalRecordAdmissionDomainError {
    #[error("typed execution site is not present in the admitted plan")]
    UnknownSite { site: RuntimePlanTypedSite },
    #[error("typed execution site has no project root use")]
    MissingRootUse { site: RuntimePlanTypedSite },
    #[error("project root is absent from the admitted generation")]
    UnknownRoot { root: RuntimeProjectRootId },
    #[error("domain belongs to a different generation")]
    GenerationMismatch {
        expected: RuntimeGenerationIdentity,
        actual: RuntimeGenerationIdentity,
    },
    #[error("execution-site checked type differs from its project root")]
    SiteTypeMismatch { site: RuntimePlanTypedSite, root: RuntimeProjectRootId },
    #[error("project root does not authorize nominal catalog key")]
    UnauthorizedCatalogKey {
        root: RuntimeProjectRootId,
        key: RuntimeNominalRecordCatalogKey,
    },
    #[error("authorized nominal catalog layout is missing")]
    MissingCatalogLayout { key: RuntimeNominalRecordCatalogKey },
    #[error("producer domain does not authorize nominal catalog key")]
    WrongProducerDomain {
        producer: RuntimeOpaqueTypeProducerId,
        key: RuntimeNominalRecordCatalogKey,
    },
}

impl<'generation> RuntimeNominalRecordAdmissionDomain<'generation> {
    pub const fn generation_identity(&self) -> RuntimeGenerationIdentity;
    pub fn require(
        &self,
        nominal: &RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    ) -> Result<RuntimeNominalRecordShape<'generation>, RuntimeNominalRecordAdmissionDomainError>;
}

impl RuntimeNominalRecordProjectDomain<'_> {
    pub const fn site(&self) -> &RuntimePlanTypedSite;
    pub const fn root(&self) -> RuntimeProjectRootId;
    pub const fn generation_identity(&self) -> RuntimeGenerationIdentity;
}
```

`RuntimeNominalAuthorizationClosure` is the retained, internal result of traversing one admitted project root through Choice, tuple, sequence, Result/Option, nominal, exact opaque, and exact Variant payloads. It is not serialized and is stored only inside the single `AdmittedRuntimeGeneration`. This is not a second root map: it is the per-root index into the already admitted catalog computed by the parent admission algorithm.

No domain type derives Serde, Default, Eq, Hash, Ord, or owned Clone. Fields and constructors are private. There is no `Deref`, `into_inner`, raw catalog-key accessor, conversion to `'static`, or constructor from root/digest/layout bytes.

## Issuance

```rust
impl AdmittedRuntimePlan {
    pub fn project_nominal_domain(
        &self,
        site: &RuntimePlanTypedSite,
    ) -> Result<RuntimeNominalRecordAdmissionDomain<'_>, RuntimeNominalRecordAdmissionDomainError>;
}

impl AdmittedAwbcProduct {
    pub(crate) fn nominal_record_domain(
        &self,
        domain: AwbcNominalRecordDomainId,
        site: &AwbcInstructionSite,
    ) -> Result<RuntimeNominalRecordAdmissionDomain<'_>, RuntimeNominalRecordAdmissionDomainError>;
}
```

The plan API resolves exactly one retained site-use row, verifies its checked type and generation, then borrows the root closure. AWBC issuance additionally requires the instruction site to reference the selected domain row. Producer selection returns the parent non-exclusive `RuntimeNominalRecordProducerShape<'generation>`; project selection returns the site-scoped project domain. No global project credential exists.

`RuntimeNominalRecordShape::try_construct(fields)` validates exact identity/layout/count/field IDs/field checked values and invokes crate-private `RuntimeNominalRecordValue::try_from_accepted_layout`. A root ID, catalog key, layout descriptor, producer ID, or digest alone cannot invoke that primitive.
