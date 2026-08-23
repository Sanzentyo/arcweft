# 03. Rust API and owner map

## Concrete ownership map

| API/behavior | Owner on current source evidence | Required change |
|---|---|---|
| accepted carrier enum and inherent behavior | `crates/<runtime-owner>/src/value.rs (new module only if no owning enum exists)` | extend the existing owning enum/impl; only create the proposed module when no such owner exists |
| checked carrier constraint emission | `crates/<language-owner>/src/checked/type.rs` | emit normalized stable keys and projection witness |
| runtime match execution | `crates/<runtime-owner>/src/match_exec.rs` | consume the checked constraint; remove shape-to-nominal inference/fallback |
| snapshot codec / restore resolver | `crates/<runtime-owner>/src/snapshot.rs` | add canonical carrier record and two-phase resolution |
| task/Need publication | `crates/<runtime-owner>/src/task.rs` | stage resolved carrier handles and publish atomically |

## Proposed API (normative semantics; spelling may be adapted to an existing owner enum)

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AcceptedRuntimeCarrier {
    Structural(StructuralRuntimeCarrier),
    Nominal(NominalRuntimeCarrier),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StructuralRuntimeCarrier {
    pub shape: RuntimeStructuralShapeId,
    pub payload: RuntimeValueHandle,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct NominalRuntimeCarrier {
    pub instance: RuntimeNominalInstanceId,
    pub representation: RuntimeStructuralShapeId,
    pub payload: RuntimeValueHandle,
}
```

`RuntimeStructuralShapeId`, `RuntimeNominalInstanceId`, and `RuntimeValueHandle` above are semantic roles. Reuse current-main canonical identifiers when they already exist; do not introduce a parallel ID family. `RuntimeNominalInstanceId` interns the tuple `(catalog_domain, stable_nominal_def, canonical_generic_args)`.

## Inherent methods on the owner enum

```rust
impl AcceptedRuntimeCarrier {
    pub fn structural(
        shape: RuntimeStructuralShapeId,
        payload: RuntimeValueHandle,
        catalog: &RuntimeTypeCatalog,
        values: &RuntimeValueArena,
    ) -> Result<Self, CarrierBuildError>;

    pub fn nominal(
        instance: RuntimeNominalInstanceId,
        representation: RuntimeStructuralShapeId,
        payload: RuntimeValueHandle,
        catalog: &RuntimeTypeCatalog,
        values: &RuntimeValueArena,
    ) -> Result<Self, CarrierBuildError>;

    pub fn class(&self) -> AcceptedCarrierClass;
    pub fn payload(&self) -> RuntimeValueHandle;
    pub fn representation_shape(&self) -> RuntimeStructuralShapeId;
    pub fn nominal_instance(&self) -> Option<RuntimeNominalInstanceId>;

    pub fn admit(
        &self,
        constraint: &AcceptedCarrierConstraint,
        catalog: &RuntimeTypeCatalog,
    ) -> Result<CarrierAdmission, CarrierInvariantError>;
}
```

The constructors are the only public route to a sealed carrier. Fields may be `pub(crate)` or private according to the owner crate's conventions. `admit` is inherent behavior, not an extension trait.

## Checked plan

```rust
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum AcceptedCarrierConstraint {
    Structural {
        target: RuntimeStructuralShapeId,
        projection: StructuralProjectionPolicy,
    },
    Nominal {
        target: RuntimeNominalInstanceId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum StructuralProjectionPolicy {
    Direct,
    FromNominal(StructuralProjectionWitnessId),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct StructuralProjectionWitness {
    pub source: RuntimeNominalInstanceId,
    pub target: RuntimeStructuralShapeId,
    pub steps: Box<[ProjectionStep]>,
    pub semantic_digest: ProjectionSemanticDigest,
}

pub enum CarrierAdmission {
    Accepted(AcceptedProjection),
    Rejected(CarrierMismatch),
}
```

`Direct` is valid only for a structural carrier. `FromNominal` is valid only for the exact nominal source named by the witness. `AcceptedProjection` contains prevalidated slot/index operations and never field-name lookup.

## Error taxonomy

```rust
pub enum CarrierBuildError {
    UnknownShape(RuntimeStructuralShapeId),
    UnknownNominalInstance(RuntimeNominalInstanceId),
    PayloadShapeMismatch { expected: RuntimeStructuralShapeId, actual: RuntimeStructuralShapeId },
    NominalRepresentationMismatch { instance: RuntimeNominalInstanceId, declared: RuntimeStructuralShapeId, supplied: RuntimeStructuralShapeId },
    DanglingPayload(RuntimeValueHandle),
}

pub enum CarrierRestoreError {
    UnsupportedVersion(u16),
    NonCanonicalInteger,
    DuplicateField(u32),
    UnknownCatalogDomain(StableCatalogKey),
    UnknownShape(StableShapeKey),
    UnknownNominal(StableNominalKey),
    GenericArgumentMismatch,
    RepresentationMismatch,
    StaleProjectionWitness,
    DanglingPayload(StablePayloadKey),
    TrailingBytes,
}
```

Use existing error aggregation conventions if current source already owns a broader enum; add these variants to that enum's original `impl`/conversion path rather than introducing a private catch-all string error.

## Ownership and borrowing

- Carrier metadata is immutable and interned; clone of a carrier clones small IDs/handles, not payload storage.
- Match execution borrows `&AcceptedRuntimeCarrier` and `&AcceptedCarrierConstraint`.
- Projection returns borrowed/arena handles under the current runtime lifetime model; it never returns a reference manufactured from an unlocked arena.
- Persisted stable keys are separate wire structs, so no `Serialize` derive is placed directly on process-local IDs.
- `unsafe` is neither required nor permitted by this design.
