# Decision 11 — layer-correct generation catalog bridge

## Owners and dependency direction

The concrete Character/View/custom bridge belongs to `arcweft-dialogue`, not runtime-driver:

- owner: `crates/arcweft-dialogue/src/character_dialogue/catalog_admission.rs`;
- narrow re-export: `arcweft_dialogue::character_dialogue::CharacterDialogueGenerationCatalogs`;
- core owns only an opaque digest-comparison provenance token;
- runtime-driver depends on dialogue and may construct/pass the same bridge;
- dialogue already depends on core, character, and View, so no forbidden lower-to-upper dependency is added.

## Core provenance token

```rust
#[derive(Clone, Copy, Debug)]
pub struct RuntimeGenerationCatalogAdmission<'generation> {
    generation: &'generation AdmittedRuntimeGeneration,
    character_digest: RuntimeCharacterCatalogDigest,
    view_digest: RuntimeViewCatalogDigest,
    custom_digest: CharacterDialogueRuntimeCustomFieldDigest,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeGenerationCatalogError {
    #[error("Character catalog digest differs")]
    CharacterDigest {
        expected: RuntimeCharacterCatalogDigest,
        actual: RuntimeCharacterCatalogDigest,
    },
    #[error("View catalog digest differs")]
    ViewDigest {
        expected: RuntimeViewCatalogDigest,
        actual: RuntimeViewCatalogDigest,
    },
    #[error("CharacterDialogue custom catalog digest differs")]
    CustomDigest {
        expected: CharacterDialogueRuntimeCustomFieldDigest,
        actual: CharacterDialogueRuntimeCustomFieldDigest,
    },
}

impl AdmittedRuntimeGeneration {
    pub fn admit_dialogue_catalog_digests(
        &self,
        character: RuntimeCharacterCatalogDigest,
        views: RuntimeViewCatalogDigest,
        custom: CharacterDialogueRuntimeCustomFieldDigest,
    ) -> Result<RuntimeGenerationCatalogAdmission<'_>, RuntimeGenerationCatalogError>;
}
```

The token's constructor is private to `AdmittedRuntimeGeneration`; fields are private; it has no Serde, Default, `Deref`, `into_inner`, owned generation clone, or constructor from a generation scalar. Its generation identity is provenance from the borrow, not a caller claim.

## Dialogue bridge

```rust
#[derive(Clone, Copy, Debug)]
pub struct CharacterDialogueGenerationCatalogs<'generation> {
    provenance: RuntimeGenerationCatalogAdmission<'generation>,
    characters: &'generation CharacterCatalog,
    views: &'generation ViewRegistry,
    custom_fields: &'generation CharacterDialogueRuntimeCustomFieldCatalog,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CharacterDialogueCatalogAdmissionError {
    #[error(transparent)]
    Character(#[from] CharacterCatalogRuntimeDigestError),
    #[error(transparent)]
    View(#[from] ViewRegistryRuntimeDigestError),
    #[error(transparent)]
    Custom(#[from] CharacterDialogueRuntimeCustomFieldDigestError),
    #[error(transparent)]
    Generation(#[from] RuntimeGenerationCatalogError),
    #[error("custom field {field} accepts missing View {view}")]
    MissingCustomFieldView {
        field: CharacterDialogueCustomFieldId,
        view: ViewId,
    },
}

impl<'generation> CharacterDialogueGenerationCatalogs<'generation> {
    pub fn try_admit(
        generation: &'generation AdmittedRuntimeGeneration,
        characters: &'generation CharacterCatalog,
        views: &'generation ViewRegistry,
        custom_fields: &'generation CharacterDialogueRuntimeCustomFieldCatalog,
    ) -> Result<Self, CharacterDialogueCatalogAdmissionError>;

    #[must_use] pub const fn generation(&self) -> &'generation AdmittedRuntimeGeneration;
    #[must_use] pub const fn characters(&self) -> &'generation CharacterCatalog;
    #[must_use] pub const fn views(&self) -> &'generation ViewRegistry;
    #[must_use] pub const fn custom_fields(&self) -> &'generation CharacterDialogueRuntimeCustomFieldCatalog;
}
```

Admission order is local Character digest, local View digest, local custom digest, core comparison with the borrowed generation, then every custom-field accepted View ID against the exact View registry. All candidates publish in one return expression.

## Relationship correction

Current `CharacterCatalog` stores validated manifests keyed by `CharacterId`; it does not own a Character-to-View relationship. Therefore the retry's Character-to-View scan and `MissingCharacterView` error are removed. No relationship is inferred from labels, look IDs, default views, or presentation metadata.

The legitimate View relationship is `CharacterDialogueRuntimeCustomFieldDescriptor.accepted_views`, projected from the accepted sema custom-field registry. Those IDs are validated against this bridge's View registry. Role View constraints, when present in their exact admitted declarations, are likewise validated from that typed owner, not from CharacterCatalog.

`target_generation: RuntimeGenerationIdentity` and `GenerationMismatch { expected, actual }` disappear because no free scalar enters the API. A bridge borrowed from one admitted generation cannot be re-bound to another.
