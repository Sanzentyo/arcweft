# Decision 07 — runtime-plan root facts, coordinates, and project-capable errors

## Semantic bridge owner

Owner: new responsibility module `crates/arcweft-runtime-plan/src/semantic_facts/runtime_roots.rs` (not a new `mod.rs`). Facts are accepted-world products and are never Serde.

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeSemanticFactSource {
    ProjectSite {
        site: RuntimePlanTypedSite,
        source: Option<SourceSpan>,
    },
    StandardDialogueRole {
        role: CharacterDialogueRuntimeRole,
        source: Option<SourceSpan>,
    },
    CharacterDialogueCustomField {
        field: CharacterDialogueCustomFieldId,
        source: SourceSpan,
    },
    RegisteredValue {
        value: RuntimeRegisteredValueId,
        source: Option<SourceSpan>,
    },
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeProducerRootCoordinate {
    Generic(RuntimeProducerRootId),
    CharacterDialogueRole(CharacterDialogueRuntimeRole),
    CharacterDialogueCustomField(CharacterDialogueCustomFieldId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProjectRootFact {
    site: RuntimePlanTypedSite,
    semantic_type: RuntimeSemanticTypeId,
    checked_type: RuntimeCheckedType,
    source: RuntimeSemanticFactSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProducerRootFact {
    coordinate: RuntimeProducerRootCoordinate,
    semantic_type: RuntimeSemanticTypeId,
    checked_type: RuntimeCheckedType,
    source: RuntimeSemanticFactSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeProducerFact {
    producer: RuntimeOpaqueTypeProducerId,
    roots: Box<[RuntimeProducerRootFact]>,
    source: RuntimeProducerFactSource,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeProducerFactSource {
    StandardDialogue,
    AcceptedOpaqueProducer { declaration: Option<SourceSpan> },
}
```

Each struct has a crate-private `try_new` that receives the fields in declaration order, validates source/coordinate consistency and checked-type projection, and exposes const/reference accessors for every field. `RuntimeProducerFact::try_new` sorts a candidate copy by `RuntimeProducerRootCoordinate`, rejects duplicates, and publishes the boxed slice atomically. A root fact may share a `RuntimeSemanticTypeId` with another site/coordinate; the semantic root declaration is deduplicated by root ID while each source use remains retained.

## Core raw declaration errors

Owner: new responsibility module `crates/arcweft-core/src/plan/project_root.rs`; producer-only errors remain in `producer_contract.rs`.

```rust
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum RuntimeProjectRootError {
    #[error("runtime generation contract has no project roots")]
    Empty,
    #[error("project root count {observed} exceeds {maximum}")]
    Limit { observed: usize, maximum: usize },
    #[error("duplicate project root {root:?}")]
    Duplicate { root: RuntimeProjectRootId },
    #[error("project root {root:?} is out of canonical order after {previous:?}")]
    NonCanonicalOrder {
        previous: RuntimeProjectRootId,
        root: RuntimeProjectRootId,
    },
    #[error("project root {root:?} has an unresolved checked type")]
    UnresolvedCheckedType { root: RuntimeProjectRootId },
    #[error("project root {root:?} has conflicting checked types")]
    ConflictingCheckedType { root: RuntimeProjectRootId },
    #[error("project root {root:?} nominal lookup failed")]
    NominalLookup {
        root: RuntimeProjectRootId,
        source: RuntimeNominalCatalogLookupError,
    },
    #[error("project root source generation differs from the accepted world")]
    SourceGenerationMismatch {
        root: RuntimeProjectRootId,
        expected: RuntimeGenerationIdentity,
        actual: RuntimeGenerationIdentity,
    },
    #[error("typed RuntimePlan site is missing from the root-use table")]
    SiteMissing { site: RuntimePlanTypedSite },
    #[error("typed RuntimePlan site occurs more than once in the root-use table")]
    SiteDuplicate { site: RuntimePlanTypedSite },
    #[error("typed RuntimePlan site resolves to the wrong semantic root")]
    RootMismatch {
        site: RuntimePlanTypedSite,
        expected: RuntimeProjectRootId,
        actual: RuntimeProjectRootId,
    },
}
```

`RuntimeProjectRootDeclaration::try_from_checked_projection` and all project-root array validation return `RuntimeProjectRootError`. They do not reuse the producer-only error owner. Generation admission preserves the distinction:

```rust
pub enum RuntimeGenerationContractError {
    ProjectRoot { source: RuntimeProjectRootError },
    ProducerRoot {
        producer: RuntimeOpaqueTypeProducerId,
        source: RuntimeProducerRootError,
    },
    // retained parent variants
}
```

This directly resolves the maintained request clarification: every duplicate/order/unresolved/lookup failure carries `RuntimeProjectRootId`, while producer errors carry `RuntimeProducerRootId` and producer identity.
