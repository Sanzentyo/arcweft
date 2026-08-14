# Decision 03 — catalog recomputation and generation-bound wrappers

## Dependency-correct owner

Owner: new responsibility module `crates/arcweft-runtime-driver/src/generation_catalogs.rs`. This owner already sits above `arcweft-core`, `arcweft-character`, and `arcweft-view`; lower catalog crates remain independent of core admission types.

## Generation-bound admission API

```rust
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum GenerationCatalogAdmissionError {
    #[error(transparent)]
    Character(#[from] CharacterCatalogRuntimeDigestError),
    #[error(transparent)]
    View(#[from] ViewRegistryRuntimeDigestError),
    #[error("character catalog digest differs from the admitted generation declaration")]
    CharacterDigestMismatch {
        declared: RuntimeCharacterCatalogDigest,
        actual: RuntimeCharacterCatalogDigest,
    },
    #[error("View registry digest differs from the admitted generation declaration")]
    ViewDigestMismatch {
        declared: RuntimeViewCatalogDigest,
        actual: RuntimeViewCatalogDigest,
    },
    #[error("catalog target generation differs from admitted generation")]
    GenerationMismatch {
        expected: RuntimeGenerationIdentity,
        actual: RuntimeGenerationIdentity,
    },
    #[error("character {character} refers to missing View {view}")]
    MissingCharacterView { character: CharacterId, view: RuntimeViewId },
    #[error("CharacterDialogue custom field {field} refers to missing View {view}")]
    MissingCustomFieldView {
        field: CharacterDialogueCustomFieldId,
        view: RuntimeViewId,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct AdmittedCharacterCatalog<'generation> {
    generation: &'generation AdmittedRuntimeGeneration,
    catalog: &'generation CharacterCatalog,
    digest: RuntimeCharacterCatalogDigest,
}

#[derive(Clone, Copy, Debug)]
pub struct AdmittedViewRegistry<'generation> {
    generation: &'generation AdmittedRuntimeGeneration,
    registry: &'generation ViewRegistry,
    digest: RuntimeViewCatalogDigest,
}

#[derive(Clone, Copy, Debug)]
pub struct AdmittedGenerationCatalogs<'generation> {
    generation: &'generation AdmittedRuntimeGeneration,
    character: AdmittedCharacterCatalog<'generation>,
    views: AdmittedViewRegistry<'generation>,
}

impl<'generation> AdmittedGenerationCatalogs<'generation> {
    pub fn try_admit(
        generation: &'generation AdmittedRuntimeGeneration,
        character: &'generation CharacterCatalog,
        views: &'generation ViewRegistry,
        target_generation: RuntimeGenerationIdentity,
    ) -> Result<Self, GenerationCatalogAdmissionError>;

    #[must_use]
    pub const fn generation(&self) -> &'generation AdmittedRuntimeGeneration;
    #[must_use]
    pub const fn character(&self) -> AdmittedCharacterCatalog<'generation>;
    #[must_use]
    pub const fn views(&self) -> AdmittedViewRegistry<'generation>;
}

impl AdmittedCharacterCatalog<'_> {
    #[must_use]
    pub const fn catalog(&self) -> &CharacterCatalog;
    #[must_use]
    pub const fn digest(&self) -> RuntimeCharacterCatalogDigest;
    #[must_use]
    pub const fn generation_identity(&self) -> RuntimeGenerationIdentity;
}

impl AdmittedViewRegistry<'_> {
    #[must_use]
    pub const fn registry(&self) -> &ViewRegistry;
    #[must_use]
    pub const fn digest(&self) -> RuntimeViewCatalogDigest;
    #[must_use]
    pub const fn generation_identity(&self) -> RuntimeGenerationIdentity;
    pub fn resolve_runtime_view(
        &self,
        id: RuntimeViewId,
    ) -> Result<&ViewDescriptor, GenerationCatalogAdmissionError>;
}
```

Fields and constructors are private. These wrappers derive neither Serde nor Default and expose no `Deref`, `into_inner`, owned clone of either catalog, raw-byte constructor, or generation-erasing conversion.

## Exact admission precedence

1. `CharacterCatalog::runtime_digest_v1`: catalog-local syntax/structure, key equality, duplicates, limits, canonical transcript.
2. `ViewRegistry::runtime_digest_v1`: identity/slot structure, canonical order/duplicates, limits, canonical transcript.
3. Losslessly project both local digest bytes into current core digest scalar types.
4. Compare actual Character digest to `generation.declaration().character_catalog_digest()`.
5. Compare actual View digest to `generation.declaration().view_catalog_digest()`.
6. Compare `target_generation` with `generation.identity()`.
7. Resolve every CharacterDialogue role/custom-field accepted View reference and every character-to-View relationship against the admitted View registry.
8. Construct all three wrappers in one return expression. No wrapper is published on failure.

Digest mismatch precedes generation mismatch because the request's catalog-admission order places declared-versus-actual digest comparison before generation comparison. A generation mismatch can never mask malformed local catalog structure or a stale digest.

## Use by CharacterDialogue

`CharacterDialogueRuntimeSchema::try_from_admitted_generation` receives one `AdmittedGenerationCatalogs<'generation>` plus the admitted dialogue producer view from the same `AdmittedRuntimeGeneration`. It compares identity/digests and role/custom roots before any encode, decode, canonical-byte, digest, patch, restore, or View activation operation. Dialogue itself does not depend on runtime-driver; runtime-driver constructs the lower-layer schema input from admitted borrows.
