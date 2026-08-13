# Exact Rust-shaped owners, visibility, Serde, and errors

These declarations are normative target shapes. They are design text, not a
production overlay. Existing unrelated variants and methods remain unchanged.
Private decomposition may vary only when the same owner, visibility, serialized
shape, authority, error typing, precedence, and observable behavior are
preserved.

## 1. Shared typed CharacterDialogue role coordinate

Owner:
`arcweft_interaction_model::dialogue::CharacterDialogueRuntimeRole`.

```rust
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[repr(u8)]
#[serde(rename_all = "snake_case")]
pub enum CharacterDialogueRuntimeRole {
    Stage = 0,
    Portrait = 1,
    Focus = 2,
    Cleanup = 3,
    Hook = 4,
    Style = 5,
    RichText = 6,
}
```

The enum is the only role vocabulary. No role string parser exists. The
`repr(u8)` values are the canonical role-order tags.

`CharacterDialogueCustomFieldId` is likewise owned by the lower
`arcweft_interaction_model::dialogue` module and re-exported by sema/dialogue.
Core therefore uses the same typed ID without depending upward. Accepted View
IDs are projected to core-owned `RuntimeViewId`; a display/source View name is
never stored as runtime authority.

## 2. Semantic accepted role owner

Owner:
`arcweft_lang_sema::character_dialogue::runtime_types`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedCharacterDialogueRuntimeRoleType {
    role: CharacterDialogueRuntimeRole,
    semantic_type: TypeId,
    closed_type: TypeKind,
    source: SourceRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedCharacterDialogueRuntimeTypes {
    world: AcceptedNominalWorldStamp,
    stage: AcceptedCharacterDialogueRuntimeRoleType,
    portrait: AcceptedCharacterDialogueRuntimeRoleType,
    focus: AcceptedCharacterDialogueRuntimeRoleType,
    cleanup: AcceptedCharacterDialogueRuntimeRoleType,
    hook: AcceptedCharacterDialogueRuntimeRoleType,
    rich_text: AcceptedCharacterDialogueRuntimeRoleType,
    style: TypeKind,
}

impl AcceptedCharacterDialogueRuntimeTypes {
    pub(crate) fn try_accept(
        world: AcceptedNominalWorldStamp,
        declarations: impl IntoIterator<Item = CharacterDialogueRuntimeRoleDeclaration>,
    ) -> Result<Self, CharacterDialogueRuntimeTypeError>;

    pub const fn world(&self) -> AcceptedNominalWorldStamp;
    pub const fn get(
        &self,
        role: CharacterDialogueRuntimeRole,
    ) -> &TypeKind;
    pub const fn source(
        &self,
        role: CharacterDialogueRuntimeRole,
    ) -> SourceRange;
}
```

`try_accept` requires exactly one typed declaration for Stage, Portrait, Focus,
Cleanup, Hook, and RichText. It rejects Style as an authored base declaration
and derives it as the source-ordered semantic choice
`Choice([EntityRef, RichText])`.

The existing owning `TypeKind` enum gains:

```rust
CharacterDialogueRole(CharacterDialogueRuntimeRole),
```

Its original normalization/acceptance implementation substitutes the
corresponding accepted `closed_type`. A leaked role coordinate or `Named` type
at runtime projection is an error. No helper trait or spelling table is added.

## 3. Generation identity and typed root IDs

Owner: `arcweft_core::plan::generation_contract`.

```rust
#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct RuntimeGenerationIdentity([u8; 32]);

impl RuntimeGenerationIdentity {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self;
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32];
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct RuntimeProjectRootId([u8; 32]);

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct RuntimeProducerRootId([u8; 32]);

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct RuntimeViewId([u8; 32]);

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct CharacterDialogueRuntimeCustomFieldDigest([u8; 32]);

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct RuntimeCharacterCatalogDigest([u8; 32]);

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[repr(transparent)]
#[serde(transparent)]
pub struct RuntimeViewCatalogDigest([u8; 32]);

impl RuntimeProjectRootId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self;
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32];
}

impl RuntimeProducerRootId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self;
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32];
}

impl RuntimeViewId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self;
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32];
}

impl RuntimeCharacterCatalogDigest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self;
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32];
}

impl RuntimeViewCatalogDigest {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self;
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32];
}
```

None implements `Default`. Raw scalar construction is not operational
authority. `RuntimeGenerationIdentity` is accepted only after recomputation
from the canonical declaration body. The custom digest is constructed by its
catalog owner and exposes only `as_bytes`.

## 4. Closed project and generic producer roots

Owner: `arcweft_core::plan::producer_contract`.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProjectRootDeclaration {
    id: RuntimeProjectRootId,
    checked_type: RuntimeCheckedType,
}

impl RuntimeProjectRootDeclaration {
    pub fn try_from_checked_projection(
        id: RuntimeProjectRootId,
        checked_type: RuntimeCheckedType,
    ) -> Result<Self, RuntimeProducerRootError>;

    pub const fn id(&self) -> RuntimeProjectRootId;
    pub const fn checked_type(&self) -> &RuntimeCheckedType;
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProducerCheckedRootDeclaration {
    id: RuntimeProducerRootId,
    checked_type: RuntimeCheckedType,
}

impl RuntimeProducerCheckedRootDeclaration {
    pub fn try_from_checked_projection(
        id: RuntimeProducerRootId,
        checked_type: RuntimeCheckedType,
    ) -> Result<Self, RuntimeProducerRootError>;

    pub const fn id(&self) -> RuntimeProducerRootId;
    pub const fn checked_type(&self) -> &RuntimeCheckedType;
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeCheckedProducerRootSet {
    roots: Box<[RuntimeProducerCheckedRootDeclaration]>,
}

impl RuntimeCheckedProducerRootSet {
    pub fn try_from_checked_projection(
        roots: impl IntoIterator<Item = RuntimeProducerCheckedRootDeclaration>,
    ) -> Result<Self, RuntimeProducerRootError>;

    pub const fn roots(&self) -> &[RuntimeProducerCheckedRootDeclaration];
}
```

Constructors sort by typed ID and reject duplicates. A raw root remains
forgeable data; only whole-generation admission gives it operational effect.

## 5. CharacterDialogue role declaration in the generation contract

Owner: same core producer-contract module.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterDialogueRuntimeRoleTypeDeclaration {
    stage: RuntimeCheckedType,
    portrait: RuntimeCheckedType,
    focus: RuntimeCheckedType,
    cleanup: RuntimeCheckedType,
    hook: RuntimeCheckedType,
    style: RuntimeCheckedType,
    rich_text: RuntimeCheckedType,
}

impl CharacterDialogueRuntimeRoleTypeDeclaration {
    pub fn try_from_checked_projection(
        stage: RuntimeCheckedType,
        portrait: RuntimeCheckedType,
        focus: RuntimeCheckedType,
        cleanup: RuntimeCheckedType,
        hook: RuntimeCheckedType,
        rich_text: RuntimeCheckedType,
    ) -> Result<Self, CharacterDialogueRuntimeRoleTypeError>;

    pub const fn get(
        &self,
        role: CharacterDialogueRuntimeRole,
    ) -> &RuntimeCheckedType;
}
```

The constructor computes `style` as
`Choice([EntityRef, rich_text.clone()])`. Raw Serde carries all seven
fields so tampering with `style` is diagnosed; admission recomputes it and
requires equality. There is no seven-argument arbitrary operational
constructor.

## 6. CharacterDialogue custom-field declaration

Owner: same core producer-contract module; `arcweft-dialogue` re-exports the
raw descriptor names it needs without depending on an upper crate.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterDialogueRuntimeCustomFieldDescriptorDeclaration {
    id: CharacterDialogueCustomFieldId,
    checked_type: RuntimeCheckedType,
    clearable: bool,
    accepted_views: Box<[RuntimeViewId]>,
}

impl CharacterDialogueRuntimeCustomFieldDescriptorDeclaration {
    pub fn try_from_checked_projection(
        id: CharacterDialogueCustomFieldId,
        checked_type: RuntimeCheckedType,
        clearable: bool,
        accepted_views: impl IntoIterator<Item = RuntimeViewId>,
    ) -> Result<Self, CharacterDialogueRuntimeCustomFieldError>;

    pub const fn id(&self) -> &CharacterDialogueCustomFieldId;
    pub const fn checked_type(&self) -> &RuntimeCheckedType;
    pub const fn clearable(&self) -> bool;
    pub const fn accepted_views(&self) -> &[RuntimeViewId];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterDialogueRuntimeCustomFieldCatalogDeclaration {
    digest: CharacterDialogueRuntimeCustomFieldDigest,
    fields: Box<[CharacterDialogueRuntimeCustomFieldDescriptorDeclaration]>,
}

impl CharacterDialogueRuntimeCustomFieldCatalogDeclaration {
    pub fn try_from_checked_projection(
        fields: impl IntoIterator<
            Item = CharacterDialogueRuntimeCustomFieldDescriptorDeclaration,
        >,
    ) -> Result<Self, CharacterDialogueRuntimeCustomFieldError>;

    pub const fn digest(&self) -> CharacterDialogueRuntimeCustomFieldDigest;
    pub const fn fields(
        &self,
    ) -> &[CharacterDialogueRuntimeCustomFieldDescriptorDeclaration];

    pub fn recompute_digest(
        &self,
    ) -> Result<CharacterDialogueRuntimeCustomFieldDigest,
               CharacterDialogueRuntimeCustomFieldError>;
}
```

`try_from_checked_projection` sorts fields and View IDs and computes the digest.
There is no constructor taking `digest`.

## 7. CharacterDialogue producer payload

Owner: same core module.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CharacterDialogueRuntimeProducerDeclaration {
    roles: CharacterDialogueRuntimeRoleTypeDeclaration,
    custom_fields: CharacterDialogueRuntimeCustomFieldCatalogDeclaration,
    character_catalog: RuntimeCharacterCatalogDigest,
    view_catalog: RuntimeViewCatalogDigest,
}

impl CharacterDialogueRuntimeProducerDeclaration {
    pub fn try_from_checked_projection(
        roles: CharacterDialogueRuntimeRoleTypeDeclaration,
        custom_fields: CharacterDialogueRuntimeCustomFieldCatalogDeclaration,
        character_catalog: RuntimeCharacterCatalogDigest,
        view_catalog: RuntimeViewCatalogDigest,
    ) -> Result<Self, CharacterDialogueRuntimeProducerError>;

    pub const fn roles(&self) -> &CharacterDialogueRuntimeRoleTypeDeclaration;
    pub const fn custom_fields(
        &self,
    ) -> &CharacterDialogueRuntimeCustomFieldCatalogDeclaration;
    pub const fn character_catalog(&self) -> RuntimeCharacterCatalogDigest;
    pub const fn view_catalog(&self) -> RuntimeViewCatalogDigest;
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "roots", rename_all = "snake_case")]
pub enum RuntimeProducerPayloadRootSet {
    Checked(RuntimeCheckedProducerRootSet),
    CharacterDialogue(CharacterDialogueRuntimeProducerDeclaration),
}
```

For `CharacterDialogue`, roots are derived from every role checked type and
every custom checked type. They are not duplicated as generic roots.

## 8. Producer payload contract and claimed closure

Owner: same core module.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeProducerPayloadContractDeclaration {
    producer: RuntimeOpaqueTypeProducerId,
    payload: RuntimeProducerPayloadRootSet,
    authorized_records: Box<[RuntimeNominalRecordCatalogKey]>,
}

impl RuntimeProducerPayloadContractDeclaration {
    pub fn try_from_checked_projection(
        producer: RuntimeOpaqueTypeProducerId,
        payload: RuntimeProducerPayloadRootSet,
        authorized_records: impl IntoIterator<Item = RuntimeNominalRecordCatalogKey>,
    ) -> Result<Self, RuntimeProducerContractError>;

    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId;
    pub const fn payload(&self) -> &RuntimeProducerPayloadRootSet;
    pub const fn authorized_records(&self) -> &[RuntimeNominalRecordCatalogKey];
}
```

The constructor only canonicalizes raw data. Whole-generation admission derives
the closure from `payload` and requires exact set equality with
`authorized_records`.

For the `CharacterDialogue` payload, `producer` must be exactly
`std.character_dialogue`. Other producer IDs fail before root traversal.

## 9. Nominal catalog declaration corrected from `.1.2`

Owner: existing `arcweft_core::value::nominal_record`.

```rust
#[derive(
    Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct RuntimeNominalRecordCatalogKey {
    nominal: RuntimeNominalTypeId,
    semantic_identity: RuntimeSemanticTypeId,
    layout: TypeLayoutHash,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeNominalRecordCatalogDeclaration {
    layouts: Box<[Arc<RuntimeNominalRecordLayout>]>,
}

impl RuntimeNominalRecordCatalogDeclaration {
    pub fn try_from_checked_projection(
        layouts: impl IntoIterator<Item = Arc<RuntimeNominalRecordLayout>>,
    ) -> Result<Self, RuntimeNominalRecordCatalogError>;

    pub const fn layouts(&self) -> &[Arc<RuntimeNominalRecordLayout>];
}
```

The `.1.2` `producers` field and
`RuntimeNominalRecordProducerDeclaration` are deleted. Producer declarations
belong to the generation contract because only there can independent payload
roots be correlated.

## 10. Single raw generation declaration

Owner: `arcweft_core::plan::generation_contract`.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeGenerationContractDeclaration {
    identity: RuntimeGenerationIdentity,
    nominal_records: RuntimeNominalRecordCatalogDeclaration,
    project_roots: Box<[RuntimeProjectRootDeclaration]>,
    producers: Box<[RuntimeProducerPayloadContractDeclaration]>,
}

impl RuntimeGenerationContractDeclaration {
    pub fn try_from_checked_projection(
        nominal_records: RuntimeNominalRecordCatalogDeclaration,
        project_roots: impl IntoIterator<Item = RuntimeProjectRootDeclaration>,
        producers: impl IntoIterator<Item = RuntimeProducerPayloadContractDeclaration>,
    ) -> Result<Self, RuntimeGenerationContractError>;

    pub const fn identity(&self) -> RuntimeGenerationIdentity;
    pub const fn nominal_records(&self) -> &RuntimeNominalRecordCatalogDeclaration;
    pub const fn project_roots(&self) -> &[RuntimeProjectRootDeclaration];
    pub const fn producers(&self) -> &[RuntimeProducerPayloadContractDeclaration];

    pub fn canonical_body_bytes(
        &self,
    ) -> Result<Box<[u8]>, RuntimeGenerationContractError>;

    pub fn recompute_identity(
        &self,
    ) -> Result<RuntimeGenerationIdentity, RuntimeGenerationContractError>;
}
```

The public constructor is a raw bridge API and computes `identity`; it does not
admit or execute anything. Serde can carry a forged identity, which
`try_admit` rejects.

## 11. Runtime-plan semantic projection owner

Owner: existing
`arcweft_runtime_plan::semantic_facts::RuntimePlanSemanticFacts`.

```rust
#[derive(Clone, Debug)]
pub struct RuntimeCharacterDialogueProducerFacts {
    world: AcceptedNominalWorldStamp,
    roles: CharacterDialogueRuntimeRoleTypeDeclaration,
    custom_fields: CharacterDialogueRuntimeCustomFieldCatalogDeclaration,
    character_catalog: RuntimeCharacterCatalogDigest,
    view_catalog: RuntimeViewCatalogDigest,
    sources: CharacterDialogueRuntimeRoleSources,
}

impl RuntimePlanSemanticFacts {
    pub fn try_project_character_dialogue_producer(
        &self,
        accepted: &AcceptedCharacterDialogueRuntimeTypes,
        custom: &CharacterDialogueCustomFieldRegistry,
        character_catalog: RuntimeCharacterCatalogDigest,
        view_catalog: RuntimeViewCatalogDigest,
    ) -> Result<RuntimeCharacterDialogueProducerFacts,
               RuntimeSemanticFactsError>;

    pub fn try_project_generation_contract(
        &self,
        project_roots: impl IntoIterator<Item = RuntimeProjectRootFact>,
        producers: impl IntoIterator<Item = RuntimeProducerFact>,
    ) -> Result<RuntimeGenerationContractDeclaration,
               RuntimeSemanticFactsError>;
}
```

These are inherent owner methods. No new projection trait or name resolver is
introduced.

## 12. Non-Serde operational aggregate

Owners:
`arcweft_core::plan::generation_contract` and existing nominal-record module.

```rust
#[derive(Clone, Debug)]
pub struct AdmittedRuntimeGeneration {
    inner: Arc<AdmittedRuntimeGenerationInner>,
}

#[derive(Debug)]
struct AdmittedRuntimeGenerationInner {
    identity: RuntimeGenerationIdentity,
    canonical_contract: Arc<[u8]>,
    nominal_records: RuntimeNominalRecordCatalog,
    project_records: BTreeSet<RuntimeNominalRecordCatalogKey>,
    producers: BTreeMap<
        RuntimeOpaqueTypeProducerId,
        AdmittedRuntimeProducerContract,
    >,
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeNominalRecordProducerShape<'generation> {
    generation: RuntimeGenerationIdentity,
    producer: &'generation RuntimeOpaqueTypeProducerId,
    records: &'generation BTreeSet<RuntimeNominalRecordCatalogKey>,
    catalog: &'generation RuntimeNominalRecordCatalog,
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeNominalRecordShape<'generation> {
    generation: RuntimeGenerationIdentity,
    domain: RuntimeNominalRecordAdmissionDomain<'generation>,
    layout: &'generation RuntimeNominalRecordLayout,
}

impl AdmittedRuntimeGeneration {
    pub const fn identity(&self) -> RuntimeGenerationIdentity;

    pub fn producer_shape(
        &self,
        producer: &RuntimeOpaqueTypeProducerId,
    ) -> Result<RuntimeNominalRecordProducerShape<'_>,
               RuntimeNominalRecordLookupError>;

    pub fn character_dialogue(
        &self,
    ) -> Result<RuntimeCharacterDialogueProducerShape<'_>,
               RuntimeCharacterDialogueProducerShapeError>;

    pub(crate) fn canonical_contract(&self) -> &[u8];
}

impl RuntimeNominalRecordProducerShape<'_> {
    pub const fn generation(&self) -> RuntimeGenerationIdentity;
    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId;

    pub fn require(
        &self,
        nominal: &RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    ) -> Result<RuntimeNominalRecordShape<'_>,
               RuntimeNominalRecordLookupError>;

    pub fn preflight_checked_type(
        &self,
        expected: &RuntimeCheckedType,
    ) -> Result<(), RuntimeNominalRecordLookupError>;

    pub fn validate_value(
        &self,
        expected: &RuntimeCheckedType,
        value: &RuntimeValue,
    ) -> Result<(), RuntimeNominalRecordTreeError>;
}

impl RuntimeNominalRecordShape<'_> {
    pub const fn generation(&self) -> RuntimeGenerationIdentity;
    pub const fn layout(&self) -> &RuntimeNominalRecordLayout;

    pub fn try_construct(
        &self,
        fields_in_layout_order: Vec<RuntimeValue>,
    ) -> Result<RuntimeNominalRecordValue, RuntimeNominalRecordError>;

    pub fn validate(
        &self,
        value: &RuntimeNominalRecordValue,
    ) -> Result<(), RuntimeNominalRecordError>;
}
```

No operational type implements `Serialize`, `Deserialize`, `Default`, public
field access, public constructor, `Deref`, or generation-erasing `into_inner`.
Only `RuntimeNominalRecordShape::try_construct` reaches crate-private
`try_from_accepted_layout`.

## 13. RuntimePlan raw field and admitted wrapper

Owner: existing `arcweft_core::plan`. Errors remain in the original
`plan::entry_inventory::RuntimePlanError` enum.

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimePlan {
    // current fields unchanged
    generation_contract: RuntimeGenerationContractDeclaration,
}

#[derive(Clone, Debug)]
pub struct AdmittedRuntimePlan {
    plan: RuntimePlan,
    generation: AdmittedRuntimeGeneration,
}

impl RuntimePlan {
    pub fn try_with_generation_contract(
        self,
        declaration: RuntimeGenerationContractDeclaration,
    ) -> Result<Self, RuntimePlanError>;

    pub fn try_admit(self) -> Result<AdmittedRuntimePlan, RuntimePlanError>;
}

impl AdmittedRuntimePlan {
    pub const fn plan(&self) -> &RuntimePlan;
    pub const fn generation(&self) -> &AdmittedRuntimeGeneration;

    pub fn try_admit_awbc(
        &self,
        program: AwbcProgram,
    ) -> Result<AdmittedAwbcProduct, AwbcAdmissionError>;
}
```

There is no `Deref<Target = RuntimePlan>`, `into_runtime_plan`, or constructor
that accepts an already built catalog.

## 14. Raw AWBC field and admitted wrapper

Owners:
`arcweft_core::awbc::{schema, admission}`.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AwbcProgram {
    pub header: AwbcHeader,
    generation_contract: RuntimeGenerationContractDeclaration,
    // all current tables remain in current order
}

impl AwbcProgram {
    pub fn try_with_generation_contract(
        self,
        declaration: RuntimeGenerationContractDeclaration,
    ) -> Result<Self, AwbcAdmissionError>;

    pub const fn generation_contract(
        &self,
    ) -> &RuntimeGenerationContractDeclaration;

    pub fn try_admit(self) -> Result<AdmittedAwbcProduct, AwbcAdmissionError>;
}

#[derive(Clone, Debug)]
pub struct AdmittedAwbcProduct {
    program: AwbcProgram,
    generation: AdmittedRuntimeGeneration,
}

impl AdmittedAwbcProduct {
    pub const fn program(&self) -> &AwbcProgram;
    pub const fn generation(&self) -> &AdmittedRuntimeGeneration;
    pub const fn identity(&self) -> RuntimeGenerationIdentity;
}
```

There is no Serde, `Deref`, raw `into_program`, or raw replacement method on
`AdmittedAwbcProduct`.

## 15. CharacterDialogue operational producer shape and role/custom views

Owner:
`arcweft_core::plan::producer_contract`.

Core owns these views because `AdmittedRuntimeGeneration` must issue them
without depending upward on `arcweft-dialogue`. The dialogue crate consumes the
core view; it may re-export the role/custom view names but does not own or
construct them.

```rust
#[derive(Clone, Copy, Debug)]
pub struct RuntimeCharacterDialogueProducerShape<'generation> {
    generation: RuntimeGenerationIdentity,
    producer: RuntimeNominalRecordProducerShape<'generation>,
    declaration: &'generation CharacterDialogueRuntimeProducerDeclaration,
}

#[derive(Clone, Copy, Debug)]
pub struct CharacterDialogueRuntimeRoleTypes<'generation> {
    generation: RuntimeGenerationIdentity,
    roles: &'generation CharacterDialogueRuntimeRoleTypeDeclaration,
}

#[derive(Clone, Copy, Debug)]
pub struct CharacterDialogueRuntimeCustomFieldCatalog<'generation> {
    generation: RuntimeGenerationIdentity,
    declaration:
        &'generation CharacterDialogueRuntimeCustomFieldCatalogDeclaration,
}

impl RuntimeCharacterDialogueProducerShape<'_> {
    pub const fn generation(&self) -> RuntimeGenerationIdentity;
    pub const fn producer(&self) -> RuntimeNominalRecordProducerShape<'_>;
    pub const fn role_types(&self) -> CharacterDialogueRuntimeRoleTypes<'_>;
    pub const fn custom_fields(
        &self,
    ) -> CharacterDialogueRuntimeCustomFieldCatalog<'_>;
    pub const fn character_catalog_digest(&self) -> RuntimeCharacterCatalogDigest;
    pub const fn view_catalog_digest(&self) -> RuntimeViewCatalogDigest;
}

impl CharacterDialogueRuntimeRoleTypes<'_> {
    pub const fn generation(&self) -> RuntimeGenerationIdentity;
    pub const fn get(
        &self,
        role: CharacterDialogueRuntimeRole,
    ) -> &RuntimeCheckedType;
}

impl CharacterDialogueRuntimeCustomFieldCatalog<'_> {
    pub const fn generation(&self) -> RuntimeGenerationIdentity;
    pub const fn digest(&self) -> CharacterDialogueRuntimeCustomFieldDigest;
    pub fn get(
        &self,
        id: &CharacterDialogueCustomFieldId,
    ) -> Option<&CharacterDialogueRuntimeCustomFieldDescriptorDeclaration>;
}
```

These views have no public constructors, Serde, `Default`, owned escape, or
independent digest/layout inputs.

## 16. Admitted Character/View catalog wrappers

Owners remain with their current catalog modules in dialogue/View layers.

```rust
#[derive(Clone, Copy, Debug)]
pub struct AdmittedCharacterCatalog<'generation> {
    generation: RuntimeGenerationIdentity,
    digest: RuntimeCharacterCatalogDigest,
    catalog: &'generation CharacterCatalog,
}

#[derive(Clone, Copy, Debug)]
pub struct AdmittedViewRegistry<'generation> {
    generation: RuntimeGenerationIdentity,
    digest: RuntimeViewCatalogDigest,
    registry: &'generation ViewRegistry,
}

impl CharacterCatalog {
    pub fn try_admit_for_generation<'generation>(
        &'generation self,
        generation: &'generation AdmittedRuntimeGeneration,
    ) -> Result<AdmittedCharacterCatalog<'generation>,
               CharacterCatalogAdmissionError>;
}

impl ViewRegistry {
    pub fn try_admit_for_generation<'generation>(
        &'generation self,
        generation: &'generation AdmittedRuntimeGeneration,
    ) -> Result<AdmittedViewRegistry<'generation>,
               ViewCatalogAdmissionError>;
}
```

Each method recomputes the catalog's canonical digest and compares the exact
declared digest before returning the wrapper.

## 17. CharacterDialogue schema construction

Owner: existing dialogue schema module.

```rust
pub struct CharacterDialogueRuntimeSchema<'generation> {
    admission: RuntimeCharacterDialogueProducerShape<'generation>,
    character_catalog: AdmittedCharacterCatalog<'generation>,
    view_catalog: AdmittedViewRegistry<'generation>,
}

impl<'generation> CharacterDialogueRuntimeSchema<'generation> {
    pub fn try_from_generation(
        admission: RuntimeCharacterDialogueProducerShape<'generation>,
        character_catalog: AdmittedCharacterCatalog<'generation>,
        view_catalog: AdmittedViewRegistry<'generation>,
    ) -> Result<Self, CharacterDialogueValueError>;

    pub const fn generation(&self) -> RuntimeGenerationIdentity;
    pub const fn role_types(&self) -> CharacterDialogueRuntimeRoleTypes<'_>;
    pub const fn custom_fields(
        &self,
    ) -> CharacterDialogueRuntimeCustomFieldCatalog<'_>;

    // retained schema-owned encode/decode/digest/patch/admit methods
}
```

The old `try_new(character_catalog, view_catalog, custom_fields, role_types,
producer)` shape is deleted. Schema construction cannot combine independently
built parts.

## 18. Core error owners

Owner modules are the same modules as the corresponding data.

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProducerRootError {
    #[error("root count {actual} exceeds {limit}")]
    Limit { actual: usize, limit: usize },

    #[error("root type exceeds depth {limit} at {path:?}")]
    Depth { path: RuntimeCheckedTypePath, limit: usize },

    #[error("root traversal work exceeds {limit} at {path:?}")]
    Work { path: RuntimeCheckedTypePath, limit: usize },

    #[error("root {id:?} is duplicated")]
    Duplicate { id: RuntimeProducerRootId },

    #[error("root {id:?} is not in canonical order")]
    NonCanonicalOrder { id: RuntimeProducerRootId },

    #[error("root {id:?} contains unresolved semantic type at {path:?}")]
    Unresolved {
        id: RuntimeProducerRootId,
        path: RuntimeCheckedTypePath,
    },

    #[error("root {id:?} references nominal catalog data that is unavailable: {source}")]
    NominalLookup {
        id: RuntimeProducerRootId,
        path: RuntimeCheckedTypePath,
        #[source]
        source: RuntimeNominalRecordLookupError,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProducerContractError {
    #[error("producer {producer:?} is duplicated")]
    DuplicateProducer { producer: RuntimeOpaqueTypeProducerId },

    #[error("producer {producer:?} repeats authorization key {key:?}")]
    DuplicateAuthorization {
        producer: RuntimeOpaqueTypeProducerId,
        key: RuntimeNominalRecordCatalogKey,
    },

    #[error("producer {producer:?} is missing derived authorization key {key:?}")]
    MissingAuthorization {
        producer: RuntimeOpaqueTypeProducerId,
        key: RuntimeNominalRecordCatalogKey,
    },

    #[error("producer {producer:?} claims extra authorization key {key:?}")]
    ExtraAuthorization {
        producer: RuntimeOpaqueTypeProducerId,
        key: RuntimeNominalRecordCatalogKey,
    },

    #[error("CharacterDialogue payload uses producer {actual:?}")]
    CharacterDialogueProducer {
        actual: RuntimeOpaqueTypeProducerId,
    },

    #[error("producer root is invalid: {source}")]
    Root {
        #[source]
        source: RuntimeProducerRootError,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeCharacterDialogueProducerShapeError {
    #[error("the admitted generation has no std.character_dialogue producer")]
    Missing,

    #[error("std.character_dialogue does not carry the CharacterDialogue payload contract")]
    WrongPayloadKind,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeGenerationContractError {
    #[error("generation contract producer/root/custom declaration is invalid: {source}")]
    Producer {
        #[source]
        source: RuntimeProducerContractError,
    },

    #[error("generation contract custom digest is invalid: {source}")]
    Custom {
        #[source]
        source: CharacterDialogueRuntimeCustomFieldError,
    },

    #[error("generation contract nominal catalog is invalid: {source}")]
    Catalog {
        #[source]
        source: RuntimeNominalRecordCatalogError,
    },

    #[error("project root traversal failed at root {root:?}: {source}")]
    ProjectRoot {
        root: RuntimeProjectRootId,
        #[source]
        source: RuntimeProducerRootError,
    },

    #[error("producer root traversal failed for {producer:?}: {source}")]
    ProducerRoot {
        producer: RuntimeOpaqueTypeProducerId,
        #[source]
        source: RuntimeProducerRootError,
    },

    #[error("nominal layout {key:?} is missing from the catalog")]
    MissingLayout { key: RuntimeNominalRecordCatalogKey },

    #[error("nominal layout {key:?} is unreachable")]
    UnreachableLayout { key: RuntimeNominalRecordCatalogKey },

    #[error("generation identity mismatch")]
    Identity {
        expected: RuntimeGenerationIdentity,
        actual: RuntimeGenerationIdentity,
    },

    #[error("equal generation identity has unequal canonical contract bytes")]
    IdentityCollision {
        identity: RuntimeGenerationIdentity,
    },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("runtime generations do not match: expected {expected:?}, actual {actual:?}")]
pub struct RuntimeGenerationMismatch {
    pub expected: RuntimeGenerationIdentity,
    pub actual: RuntimeGenerationIdentity,
}
```

The existing `RuntimePlanError` gains a source-preserving
`GenerationContract` variant. `AwbcAdmissionError` owns header/structural/
generation/join cases and is defined in `AWBC_PRODUCT_ADMISSION_AND_CODEC.md`.
Dialogue, driver, bundle, save, replay, and View errors wrap these sources as
specified by `ERROR_AND_PRECEDENCE.md`.
