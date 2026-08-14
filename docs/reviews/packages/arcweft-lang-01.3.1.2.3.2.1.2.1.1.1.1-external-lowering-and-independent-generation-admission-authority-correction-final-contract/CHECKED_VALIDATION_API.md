# Checked-value context, nominal domain, and atomic opaque API

Owner: `crates/arcweft-core/src/pattern.rs` for checked types/validator behavior,
`crates/arcweft-core/src/plan/admission.rs` for context/domain issuance, and
`crates/arcweft-core/src/value/ownership/path.rs` for the canonical physical
value path.

## Context and domain types

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeCheckedValueLimits {
    maximum_depth: u32,
    maximum_work: u64,
}

impl RuntimeCheckedValueLimits {
    pub fn try_new(
        maximum_depth: u32,
        maximum_work: u64,
    ) -> Result<Self, RuntimeCheckedValueLimitError>;
    pub const fn maximum_depth(self) -> u32;
    pub const fn maximum_work(self) -> u64;
}

#[derive(Clone, Copy, Debug)]
pub enum RuntimeNominalRecordAdmissionDomain<'generation> {
    Project {
        generation: &'generation AdmittedRuntimeGeneration,
        root: RuntimeProjectRootId,
    },
    Producer {
        generation: &'generation AdmittedRuntimeGeneration,
        producer: &'generation RuntimeOpaqueTypeProducerId,
        root: RuntimeProducerRootId,
    },
}

pub struct RuntimeCheckedValueContext<'generation> {
    generation: &'generation AdmittedRuntimeGeneration,
    resolved: &'generation RuntimeResolvedType,
    nominal_domain: Option<RuntimeNominalRecordAdmissionDomain<'generation>>,
    limits: RuntimeCheckedValueLimits,
}

impl AdmittedRuntimePlan {
    pub fn checked_value_context(
        &self,
        site: &RuntimePlanTypedSite,
        limits: RuntimeCheckedValueLimits,
    ) -> Result<RuntimeCheckedValueContext<'_>,
                RuntimePlanSiteResolutionError>;
}

impl AdmittedRuntimeProduct {
    pub fn checked_value_context(
        &self,
        origin: &AwbcTypedOrigin,
        limits: RuntimeCheckedValueLimits,
    ) -> Result<RuntimeCheckedValueContext<'_>,
                RuntimeProductAdmissionError>;

    pub fn nominal_record_domain(
        &self,
        origin: &AwbcTypedOrigin,
        domain: AwbcNominalRecordDomainId,
    ) -> Result<RuntimeNominalRecordAdmissionDomain<'_>,
                RuntimeProductAdmissionError>;
}

impl RuntimeCheckedValueContext<'_> {
    pub fn validate(
        &self,
        value: &RuntimeValue,
    ) -> Result<(), RuntimeCheckedValueError>;

    pub fn validate_at(
        &self,
        value: &RuntimeValue,
        value_path: &RuntimeValuePath,
    ) -> Result<(), RuntimeCheckedValueError>;
}
```

Every field is private. `RuntimeCheckedValueContext` and nominal domains have no
Serde, `Default`, `Clone`, public constructor, rebind method, or constructor from
a generation identity. Operational resolved types return
`RuntimePlanSiteResolutionError::OperationalTypeHasNoCheckedContext` and cannot
reach the checked validator.

## Atomic opaque behavior on the original owner

The final behavior is implemented on the existing
`RuntimeOpaqueTypeOwner`/`RuntimeCheckedType` owner; no helper trait is added.

```rust
impl RuntimeOpaqueTypeOwner {
    pub fn accepts_opaque_value(
        &self,
        actual: &RuntimeOpaqueTypeOwner,
    ) -> Result<(), RuntimeOpaqueTypeOwnerMismatch>;
}

impl RuntimeCheckedType {
    pub(crate) fn validate_value_in(
        &self,
        context: &mut RuntimeCheckedValueTraversal<'_>,
        value: &RuntimeValue,
    ) -> Result<(), RuntimeCheckedValueError>;
}
```

The `RuntimeCheckedType::Opaque` arm is exact:

```rust
(RuntimeCheckedType::Opaque { owner: expected }, RuntimeValue::Opaque(actual)) => {
    context.charge_node()?;
    expected
        .accepts_opaque_value(actual.owner())
        .map_err(|source| context.opaque_owner_error(source))?;
    Ok(())
}
```

The common dispatcher performs outer-shape selection before this arm. The arm
must not read `actual.payload()`, call itself recursively, push
`RuntimeCheckedTypePathStep::OpaquePayload`, push
`RuntimeValuePathSegment::OpaquePayload`, increment depth, or charge payload
work.

## Exact opaque error precedence

1. invalid/mismatched admitted generation or site context;
2. global work budget for the current opaque wrapper;
3. outer value-shape mismatch (`expected Opaque`, actual other variant);
4. producer mismatch;
5. exact semantic identity mismatch for `ExactIdentity`;
6. success.

A ProducerWide expected owner skips step 5 after equal producer. A concrete
RuntimeOpaqueValue owner is always ExactIdentity. There is no payload lookup,
payload type, payload path, payload depth, or payload validation error.

## Physical path distinction

`RuntimeValuePathSegment::OpaquePayload` tag `10` remains in ownership, save,
snapshot diagnostics, and explicit physical traversal. Such traversal pushes
one physical segment before visiting the payload and uses its own traversal
limits. It does not imply checked-value recursion. The non-Serde
`RuntimeCheckedTypePath` has no `OpaquePayload` step in the final atomic rule.
