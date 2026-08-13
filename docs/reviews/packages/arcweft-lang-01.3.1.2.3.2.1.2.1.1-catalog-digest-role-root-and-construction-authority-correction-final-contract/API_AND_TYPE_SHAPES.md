# Rust-shaped API and ownership design

These are design signatures, not a production overlay. Exact module paths follow the existing owners at the requested commit. Missing behavior on an Arcweft-owned enum is added to that enum's original inherent `impl`.

## 1. Closed role owner

```rust
#[repr(u16)]
pub(crate) enum RuntimeCatalogDigestRole {
    // Exact closed variants listed by FINAL_CONTRACT.md.
}

impl RuntimeCatalogDigestRole {
    pub(crate) const fn stable_ordinal(self) -> u16;
    pub(crate) const fn digest_domain(self) -> &'static [u8];
    pub(crate) const fn is_required(self) -> bool;
    pub(crate) const fn permits_value_construction(self) -> bool;
    pub(crate) fn canonicalize_and_digest(
        self,
        catalog: &RawRoleCatalog,
        budget: &mut AdmissionWorkBudget,
    ) -> Result<DerivedRoleCatalog, CatalogAdmissionError>;
}
```

There is no role extension trait, global side map, string-based dispatch, or helper that repeats these matches.

## 2. Raw versus admitted types

```rust
#[derive(Serialize, Deserialize)]
pub struct RawCatalogDigestAssertions {
    pub role_digests: Vec<RawRoleDigestAssertion>,
    pub role_root: Digest32,
    pub generation: RuntimeGenerationIdentity,
}

pub struct RuntimeCatalogDigestRoleRoot {
    inner: Arc<AdmittedRoleRootInner>,
}

pub struct AdmittedRuntimeGeneration {
    inner: Arc<AdmittedRuntimeGenerationInner>,
}
```

`RuntimeCatalogDigestRoleRoot` and `AdmittedRuntimeGeneration` do not implement `Serialize`, `Deserialize`, `Default`, or raw-parts conversion. Fields and constructors stay private to the admission owner.

## 3. Core admission owner

```rust
impl RuntimeGenerationAdmitter {
    pub fn admit_pair(
        &self,
        raw_plan: RawRuntimePlan,
        raw_awbc: RawAwbcProgram,
        producer_root: AdmittedProducerDeclarationRoot,
        budget: AdmissionWorkBudget,
    ) -> Result<AdmittedRuntimeGeneration, RuntimeGenerationAdmissionError>;
}

impl AdmittedRuntimeGeneration {
    pub fn generation_identity(&self) -> RuntimeGenerationIdentity;
    pub fn role_root_digest(&self) -> Digest32;
    pub fn admitted_plan(&self) -> &AdmittedRuntimePlan;
    pub fn admitted_awbc(&self) -> &AdmittedAwbcProgram;

    pub(crate) fn construction_authority(
        &self,
        role: RuntimeCatalogDigestRole,
        producer: Option<AdmittedProducerId>,
    ) -> Result<RuntimeConstructionAuthority<'_>, ConstructionAuthorityError>;
}
```

Pair admission builds a private candidate aggregate and returns it only after every step succeeds. It does not publish a plan while AWBC validation is pending.

## 4. Scoped construction capability

```rust
pub(crate) struct RuntimeConstructionAuthority<'g> {
    generation: &'g AdmittedRuntimeGenerationInner,
    role: RuntimeCatalogDigestRole,
    producer: Option<AdmittedProducerId>,
    allowed_layouts: &'g AdmittedNominalLayoutClosure,
}

impl<'g> RuntimeConstructionAuthority<'g> {
    pub(crate) fn generation_identity(&self) -> RuntimeGenerationIdentity;
    pub(crate) fn role(&self) -> RuntimeCatalogDigestRole;

    pub(crate) fn construct_nominal(
        &self,
        layout: &AdmittedNominalLayoutHandle<'g>,
        fields: RuntimeNominalFieldValues,
    ) -> Result<RuntimeNominalRecordAdmissionDomain, RuntimeValueConstructionError>;
}
```

The lifetime/object identity ties the capability to the admitted aggregate. Where the runtime needs owned handles, use an opaque `Arc` identity token stored in both handle and active generation and compare identity plus generation; do not reconstruct from digest bytes.

## 5. Original nominal invariant owner

```rust
impl RuntimeNominalRecordAdmissionDomain {
    pub(crate) fn try_from_accepted_layout(
        authority: &RuntimeConstructionAuthority<'_>,
        layout: &AdmittedNominalLayoutHandle<'_>,
        fields: RuntimeNominalFieldValues,
    ) -> Result<Self, RuntimeValueConstructionError>;

    pub fn validate_against_layout(
        &self,
        generation: &AdmittedRuntimeGeneration,
        layout: &AdmittedNominalLayoutHandle<'_>,
    ) -> Result<(), RuntimeValueValidationError>;
}
```

Add needed invariant behavior here, not in a one-off helper or extension trait. The constructor verifies authority role/producer/generation, layout membership, field completeness, exact checked types, nested nominal correlation, and final layout validation.

## 6. External producer façade

```rust
pub struct ExternalProducerValueBuilder<'g> {
    authority: RuntimeConstructionAuthority<'g>,
    declaration: &'g AdmittedExternalProducerDeclaration,
}

impl<'g> ExternalProducerValueBuilder<'g> {
    pub fn construct_declared_value(
        &self,
        slot: AdmittedProducerOutputSlot,
        fields: ExternalFieldValues,
    ) -> Result<RuntimeValueHandle, ExternalValueAdmissionError>;
}
```

The façade resolves the slot to an admitted layout internally. It exposes no raw nominal ID/layout ID/checked type/catalog digest/root digest/generation constructor parameter.

## 7. CharacterDialogue façades

Each admitted CharacterDialogue role has a dedicated typed constructor (speaker/content/voice/choice/custom-entry/inline-failure and every role retained by the parent contract). Generic raw nominal construction is not public. Custom fields are resolved by admitted field ID to exact checked type, clearability, and accepted View closure.

Normalize, clear, and patch APIs accept the active admitted generation or a façade already bound to it. They create a complete candidate value, call the original nominal invariant owner, and swap once. No mutation occurs while path validation is incomplete.

## 8. Forbidden signatures

The final public/reachable API must not contain:

```rust
fn from_digest(...);
fn from_root_bytes(...);
fn new_unchecked(...);
fn construct_nominal(raw_nominal_id, raw_layout_id, fields);
impl Deserialize for AdmittedRuntimeGeneration;
impl Deserialize for RuntimeConstructionAuthority<'_>;
impl From<[u8; 32]> for AdmittedCatalogRoleRoot;
```

Nor may a trait/helper re-expose equivalent behavior under another name.
