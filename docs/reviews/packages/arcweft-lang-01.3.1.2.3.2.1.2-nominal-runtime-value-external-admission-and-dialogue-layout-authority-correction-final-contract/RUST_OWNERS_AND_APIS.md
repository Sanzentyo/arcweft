# Exact Rust-shaped owners and APIs

The declarations below are normative target shapes. They are design text, not a
production overlay. Existing unrelated variants/methods remain unchanged.
Private algorithmic decomposition may vary only when visibility, authority,
error typing, precedence, and observable behavior remain exactly equivalent.

## 1. Core catalog declaration

Owner: `arcweft_core::value::nominal_record`, re-exported from
`arcweft_core::value`.

```rust
#[derive(
    Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
pub struct RuntimeNominalRecordCatalogKey {
    nominal: RuntimeNominalTypeId,
    semantic_identity: RuntimeSemanticTypeId,
    layout: TypeLayoutHash,
}

impl RuntimeNominalRecordCatalogKey {
    #[must_use]
    pub fn from_layout(layout: &RuntimeNominalRecordLayout) -> Self;
    pub const fn nominal(&self) -> &RuntimeNominalTypeId;
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn layout(&self) -> TypeLayoutHash;
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeNominalRecordProducerDeclaration {
    producer: RuntimeOpaqueTypeProducerId,
    records: Box<[RuntimeNominalRecordCatalogKey]>,
}

impl RuntimeNominalRecordProducerDeclaration {
    pub fn try_from_checked_projection(
        producer: RuntimeOpaqueTypeProducerId,
        records: impl IntoIterator<Item = RuntimeNominalRecordCatalogKey>,
    ) -> Result<Self, RuntimeNominalRecordCatalogError>;

    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId;
    pub const fn records(&self) -> &[RuntimeNominalRecordCatalogKey];
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeNominalRecordCatalogDeclaration {
    layouts: Box<[Arc<RuntimeNominalRecordLayout>]>,
    producers: Box<[RuntimeNominalRecordProducerDeclaration]>,
}

impl RuntimeNominalRecordCatalogDeclaration {
    pub fn try_from_checked_projection(
        layouts: impl IntoIterator<Item = Arc<RuntimeNominalRecordLayout>>,
        producers: impl IntoIterator<Item = RuntimeNominalRecordProducerDeclaration>,
    ) -> Result<Self, RuntimeNominalRecordCatalogError>;

    pub const fn layouts(&self) -> &[Arc<RuntimeNominalRecordLayout>];
    pub const fn producers(&self) -> &[RuntimeNominalRecordProducerDeclaration];
}
```

The two `try_from_checked_projection` methods are plan-construction APIs, not
runtime value constructors. They canonicalize ordering/duplicates and cannot
issue operational handles. The compiler/runtime-plan bridge is their intended
caller. Raw declarations are untrusted until `RuntimePlan::try_admit`.

## 2. Operational catalog and handles

Owner: same module.

```rust
#[derive(Clone, Debug)]
pub struct RuntimeNominalRecordCatalog {
    entries: BTreeMap<RuntimeNominalRecordCatalogKey, Arc<RuntimeNominalRecordLayout>>,
    producers: BTreeMap<
        RuntimeOpaqueTypeProducerId,
        BTreeSet<RuntimeNominalRecordCatalogKey>,
    >,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeNominalRecordAdmissionDomain<'a> {
    Project,
    OpaquePayload(&'a RuntimeOpaqueTypeProducerId),
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeNominalRecordProducerAdmission<'a> {
    producer: &'a RuntimeOpaqueTypeProducerId,
    catalog: &'a RuntimeNominalRecordCatalog,
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeNominalRecordAdmission<'a> {
    domain: RuntimeNominalRecordAdmissionDomain<'a>,
    layout: &'a RuntimeNominalRecordLayout,
}

impl RuntimeNominalRecordCatalog {
    pub(crate) fn try_admit(
        declaration: RuntimeNominalRecordCatalogDeclaration,
        reachable_project_records: &BTreeSet<RuntimeNominalRecordCatalogKey>,
    ) -> Result<Self, RuntimeNominalRecordCatalogError>;

    pub fn require_project(
        &self,
        nominal: &RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    ) -> Result<RuntimeNominalRecordAdmission<'_>, RuntimeNominalRecordLookupError>;

    pub fn producer(
        &self,
        producer: &RuntimeOpaqueTypeProducerId,
    ) -> Result<RuntimeNominalRecordProducerAdmission<'_>, RuntimeNominalRecordLookupError>;

    pub fn validate_project_value(
        &self,
        expected: &RuntimeCheckedType,
        value: &RuntimeValue,
    ) -> Result<(), RuntimeNominalRecordTreeError>;
}

impl RuntimeNominalRecordProducerAdmission<'_> {
    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId;

    pub fn require(
        &self,
        nominal: &RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    ) -> Result<RuntimeNominalRecordAdmission<'_>, RuntimeNominalRecordLookupError>;

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

impl RuntimeNominalRecordAdmission<'_> {
    pub const fn domain(&self) -> RuntimeNominalRecordAdmissionDomain<'_>;
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

The operational types do not implement `Serialize`, `Deserialize`, `Default`,
or a public constructor. Only `RuntimeNominalRecordAdmission::try_construct`
invokes crate-private `RuntimeNominalRecordValue::try_from_accepted_layout`.

## 3. Core catalog/lookup/tree errors

Owner: same module.

```rust
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalRecordCatalogError {
    #[error("nominal catalog repeats key {key:?}")]
    DuplicateLayout { key: RuntimeNominalRecordCatalogKey },

    #[error("nominal catalog key {key:?} has conflicting defining-order fields")]
    ConflictingLayout { key: RuntimeNominalRecordCatalogKey },

    #[error("nominal catalog descriptor does not match key {key:?}")]
    LayoutKeyMismatch { key: RuntimeNominalRecordCatalogKey },

    #[error("nominal catalog repeats producer {producer:?}")]
    DuplicateProducer { producer: RuntimeOpaqueTypeProducerId },

    #[error("producer {producer:?} repeats nominal catalog key {key:?}")]
    DuplicateProducerRecord {
        producer: RuntimeOpaqueTypeProducerId,
        key: RuntimeNominalRecordCatalogKey,
    },

    #[error("producer {producer:?} references missing nominal catalog key {key:?}")]
    MissingProducerLayout {
        producer: RuntimeOpaqueTypeProducerId,
        key: RuntimeNominalRecordCatalogKey,
    },

    #[error("nominal catalog key {key:?} is unreachable from the admitted plan")]
    UnreachableLayout { key: RuntimeNominalRecordCatalogKey },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalRecordLookupError {
    #[error("opaque producer {producer:?} has no admitted nominal-record domain")]
    ProducerNotAdmitted { producer: RuntimeOpaqueTypeProducerId },

    #[error("active catalog has no nominal record {nominal:?}")]
    Missing { nominal: RuntimeNominalTypeId },

    #[error("active catalog has stale semantic evidence for nominal {nominal:?}")]
    StaleSemanticIdentity {
        nominal: RuntimeNominalTypeId,
        expected: RuntimeSemanticTypeId,
        active: Box<[RuntimeSemanticTypeId]>,
    },

    #[error("active catalog has stale layout evidence for nominal {nominal:?}")]
    StaleLayout {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        expected: TypeLayoutHash,
        active: Box<[TypeLayoutHash]>,
    },

    #[error("producer {producer:?} is not admitted for nominal catalog key {key:?}")]
    WrongProducer {
        producer: RuntimeOpaqueTypeProducerId,
        key: RuntimeNominalRecordCatalogKey,
        admitted: Box<[RuntimeOpaqueTypeProducerId]>,
    },
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeNominalRecordTreeError {
    #[error("nominal descriptor lookup failed at {path:?}: {source}")]
    Lookup {
        path: RuntimeValuePath,
        #[source]
        source: RuntimeNominalRecordLookupError,
    },

    #[error("nominal value validation failed at {path:?}: {source}")]
    Nominal {
        path: RuntimeValuePath,
        #[source]
        source: RuntimeNominalRecordError,
    },

    #[error("runtime value at {path:?} does not satisfy {expected:?}")]
    CheckedType {
        path: RuntimeValuePath,
        expected: RuntimeCheckedType,
    },

    #[error("runtime value nesting failed at {path:?}: {source}")]
    Nesting {
        path: RuntimeValuePath,
        #[source]
        source: RuntimeValueNestingError,
    },
}
```

`RuntimeNominalRecordError` itself is retained unchanged from the parent target.

## 4. Admitted runtime plan

Owner: existing `arcweft_core::plan`; errors stay in the existing
`plan::entry_inventory` `RuntimePlanError` enum.

```rust
#[derive(Clone, Debug)]
pub struct AdmittedRuntimePlan {
    plan: RuntimePlan,
    nominal_records: RuntimeNominalRecordCatalog,
}

impl RuntimePlan {
    pub fn try_with_nominal_record_catalog(
        self,
        declaration: RuntimeNominalRecordCatalogDeclaration,
    ) -> Result<Self, RuntimePlanError>;

    pub fn try_admit(self) -> Result<AdmittedRuntimePlan, RuntimePlanError>;
}

impl AdmittedRuntimePlan {
    pub const fn plan(&self) -> &RuntimePlan;
    pub const fn nominal_records(&self) -> &RuntimeNominalRecordCatalog;
}
```

`RuntimePlan` retains the declaration in a private field. `AdmittedRuntimePlan`
has no Serde implementation, `Deref<Target = RuntimePlan>`, public field, or
unchecked `into_runtime_plan` publication shortcut.

Add to the original `RuntimePlanError`:

```rust
#[error("runtime plan nominal-record catalog is invalid: {source}")]
NominalRecordCatalog {
    #[source]
    source: RuntimeNominalRecordCatalogError,
},
```

All existing plan errors remain in their existing precedence before or after
this variant according to `AUTHORITY_AND_CATALOG.md`.

## 5. Closed variant acceptance

Owner: existing `arcweft_core::pattern::RuntimeCheckedType` impl.

```rust
impl RuntimeCheckedType {
    #[must_use]
    pub fn accepts_value(&self, value: &RuntimeValue) -> bool;
}
```

The original inherent implementation is extended. The `Variant` branch checks
owner, ordinal, exact case name, payload presence, and recursive payload type.
No new trait, free validator, or dialogue-local substitute is added.

## 6. CharacterDialogue role and custom descriptors

Owner: existing `arcweft_dialogue::character_dialogue::schema` and re-exports.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CharacterDialogueRuntimeRole {
    Stage,
    Portrait,
    Focus,
    Cleanup,
    Hook,
    Style,
    RichText,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeRoleTypes {
    stage: RuntimeCheckedType,
    portrait: RuntimeCheckedType,
    focus: RuntimeCheckedType,
    cleanup: RuntimeCheckedType,
    hook: RuntimeCheckedType,
    style: RuntimeCheckedType,
    rich_text: RuntimeCheckedType,
}

impl CharacterDialogueRuntimeRoleTypes {
    pub fn new(
        stage: RuntimeCheckedType,
        portrait: RuntimeCheckedType,
        focus: RuntimeCheckedType,
        cleanup: RuntimeCheckedType,
        hook: RuntimeCheckedType,
        style: RuntimeCheckedType,
        rich_text: RuntimeCheckedType,
    ) -> Self;

    pub const fn get(&self, role: CharacterDialogueRuntimeRole) -> &RuntimeCheckedType;
    pub const fn stage(&self) -> &RuntimeCheckedType;
    pub const fn portrait(&self) -> &RuntimeCheckedType;
    pub const fn focus(&self) -> &RuntimeCheckedType;
    pub const fn cleanup(&self) -> &RuntimeCheckedType;
    pub const fn hook(&self) -> &RuntimeCheckedType;
    pub const fn style(&self) -> &RuntimeCheckedType;
    pub const fn rich_text(&self) -> &RuntimeCheckedType;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CharacterDialogueRuntimeCustomFieldDescriptor {
    id: CharacterDialogueCustomFieldId,
    checked_type: RuntimeCheckedType,
    clearable: bool,
    accepted_views: BTreeSet<ViewId>,
}

impl CharacterDialogueRuntimeCustomFieldDescriptor {
    pub fn new(
        id: CharacterDialogueCustomFieldId,
        checked_type: RuntimeCheckedType,
        clearable: bool,
        accepted_views: BTreeSet<ViewId>,
    ) -> Self;

    pub const fn id(&self) -> &CharacterDialogueCustomFieldId;
    pub const fn checked_type(&self) -> &RuntimeCheckedType;
    pub const fn clearable(&self) -> bool;
    pub const fn accepted_views(&self) -> &BTreeSet<ViewId>;
}
```

`CharacterDialogueRuntimeCustomFieldCatalog` retains its current digest/map
owner and duplicate-ID constructor, but its descriptors use `checked_type` and
have no nominal/layout fields.

## 7. CharacterDialogue runtime schema and value

```rust
pub struct CharacterDialogueRuntimeSchema<'a> {
    character_catalog: &'a CharacterCatalog,
    view_catalog: &'a ViewRegistry,
    custom_fields: &'a CharacterDialogueRuntimeCustomFieldCatalog,
    role_types: &'a CharacterDialogueRuntimeRoleTypes,
    nominal_records: RuntimeNominalRecordProducerAdmission<'a>,
}

#[derive(Clone, Debug)]
pub struct CharacterDialogueValue {
    opaque: RuntimeOpaqueValue,
    dialogue: CharacterDialogue,
}

impl<'a> CharacterDialogueRuntimeSchema<'a> {
    #[must_use]
    pub fn opaque_type_producer() -> RuntimeOpaqueTypeProducerId;

    pub fn try_new(
        character_catalog: &'a CharacterCatalog,
        view_catalog: &'a ViewRegistry,
        custom_fields: &'a CharacterDialogueRuntimeCustomFieldCatalog,
        role_types: &'a CharacterDialogueRuntimeRoleTypes,
        nominal_records: RuntimeNominalRecordProducerAdmission<'a>,
    ) -> Result<Self, CharacterDialogueValueError>;

    pub fn try_encode(
        &self,
        dialogue: &CharacterDialogue,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError>;

    pub fn try_decode_opaque(
        &self,
        value: &RuntimeOpaqueValue,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError>;

    pub fn try_digest(
        &self,
        dialogue: &CharacterDialogue,
    ) -> Result<RuntimeValueDigest, CharacterDialogueValueError>;

    pub fn try_patch(
        &self,
        dialogue: &CharacterDialogue,
        patch: &CharacterDialoguePatch,
    ) -> Result<CharacterDialogue, CharacterDialogueValueError>;

    pub fn try_admit_stage_value(&self, value: RuntimeValue)
        -> Result<CharacterDialogueStageValue, CharacterDialogueValueError>;
    pub fn try_admit_portrait_value(&self, value: RuntimeValue)
        -> Result<CharacterDialoguePortraitValue, CharacterDialogueValueError>;
    pub fn try_admit_focus_value(&self, value: RuntimeValue)
        -> Result<CharacterDialogueFocusValue, CharacterDialogueValueError>;
    pub fn try_admit_cleanup_value(&self, value: RuntimeValue)
        -> Result<CharacterDialogueCleanupValue, CharacterDialogueValueError>;
    pub fn try_admit_hook_value(&self, value: RuntimeValue)
        -> Result<CharacterDialogueHookValue, CharacterDialogueValueError>;
    pub fn try_admit_style_value(&self, value: RuntimeValue)
        -> Result<CharacterDialogueStyleValue, CharacterDialogueValueError>;
    pub fn try_admit_rich_text_value(&self, value: RuntimeValue)
        -> Result<CharacterDialogueRichTextValue, CharacterDialogueValueError>;
    pub fn try_admit_custom_value(
        &self,
        field: &CharacterDialogueCustomFieldId,
        value: RuntimeValue,
    ) -> Result<CharacterDialogueCustomValue, CharacterDialogueValueError>;
}

impl CharacterDialogueValue {
    pub const fn dialogue(&self) -> &CharacterDialogue;
    pub const fn opaque(&self) -> &RuntimeOpaqueValue;
    pub fn into_runtime_value(self) -> RuntimeValue;
}
```

The schema has no root nominal ID/layout accessor. `CharacterDialogueValue`
has no record accessor and no caller-supplied-owner wrapping method.

## 8. CharacterDialogue and live typed wrappers

```rust
#[derive(Clone, Debug)]
pub struct CharacterDialogue {
    character: CharacterId,
    contract: CharacterDialogueContractIdentity,
    config: CharacterDialogueConfig,
}

impl CharacterDialogue {
    pub fn try_new(
        character: CharacterId,
        contract: CharacterDialogueContractIdentity,
        config: CharacterDialogueConfig,
    ) -> Result<Self, CharacterDialogueValueError>;

    pub const fn character(&self) -> &CharacterId;
    pub const fn contract(&self) -> CharacterDialogueContractIdentity;
    pub const fn config(&self) -> &CharacterDialogueConfig;
}

#[derive(Clone, Debug, Serialize)]
pub struct CharacterDialogueTypedValue {
    value: RuntimeValue,
}

impl CharacterDialogueTypedValue {
    pub(crate) fn from_admitted(value: RuntimeValue) -> Self;
    pub const fn value(&self) -> &RuntimeValue;
    pub fn into_value(self) -> RuntimeValue;
}
```

`CharacterDialogueTypedValue` and every role/custom wrapper have no
`Deserialize` and no public constructor. Existing role wrapper accessors may
remain; their `try_new(CharacterDialogueTypedValue)` constructors become
`pub(crate)` or are deleted in favor of the schema admission methods. Equality
and hashing use canonical bytes of the already admitted value and no longer
include nominal/layout side scalars.

## 9. Structured patch

Owner: existing `arcweft_dialogue::character_dialogue::patch`.

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct StructuredPatch<T> {
    clear_all: bool,
    assignments: BTreeMap<RuntimeValuePath, PatchField<RuntimeValue>>,
    marker: PhantomData<fn() -> T>,
}

impl<T> StructuredPatch<T> {
    pub fn try_new(
        clear_all: bool,
        assignments: BTreeMap<RuntimeValuePath, PatchField<RuntimeValue>>,
    ) -> Result<Self, CharacterDialogueValueError>;

    pub const fn assignments(
        &self,
    ) -> &BTreeMap<RuntimeValuePath, PatchField<RuntimeValue>>;
}
```

The dialogue-local `RuntimeFieldPath` declaration and all ordinal-only methods
are deleted.

## 10. Dialogue shape/error declarations

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CharacterDialoguePayloadShape {
    RootTuple18,
    DenseBytes32,
    OptionVariant,
    ValuesSequence,
    CustomEntryTuple2,
    VoiceVariant,
    InlineFailureVariant,
    InlineFallbackVariant,
    FallbackStyleVariant,
    String,
    EntityReference,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum CharacterDialogueValueError {
    // Existing Identity, Locale, Limit, MissingCharacter,
    // CharacterManifestMismatch, MissingLook, MissingView,
    // CustomSchemaMismatch, UnknownCustomField, DuplicateCustomField,
    // NonCanonicalCustomOrder, and CustomFieldView variants remain.

    #[error("opaque CharacterDialogue value uses producer {actual:?}, expected {expected:?}")]
    OpaqueProducer {
        expected: RuntimeOpaqueTypeProducerId,
        actual: RuntimeOpaqueTypeProducerId,
    },

    #[error("opaque CharacterDialogue semantic identity does not match decoded character")]
    OpaqueSemanticIdentity {
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },

    #[error("CharacterDialogue payload at {path:?} does not have shape {expected:?}")]
    PayloadShape {
        path: RuntimeValuePath,
        expected: CharacterDialoguePayloadShape,
    },

    #[error("CharacterDialogue role {role:?} value is invalid: {source}")]
    RoleValue {
        role: CharacterDialogueRuntimeRole,
        #[source]
        source: RuntimeNominalRecordTreeError,
    },

    #[error("CharacterDialogue custom field {field:?} value is invalid: {source}")]
    CustomValue {
        field: CharacterDialogueCustomFieldId,
        #[source]
        source: RuntimeNominalRecordTreeError,
    },

    #[error("CharacterDialogue patch operation {operation} cannot resolve {path:?}: {source}")]
    PatchPath {
        operation: usize,
        path: RuntimeValuePath,
        #[source]
        source: RuntimeValuePathError,
    },

    #[error("CharacterDialogue patch operation {operation} produced an invalid value at {path:?}: {source}")]
    PatchValue {
        operation: usize,
        path: RuntimeValuePath,
        #[source]
        source: RuntimeNominalRecordTreeError,
    },

    #[error(transparent)]
    OpaqueValue(#[from] RuntimeOpaqueValueError),

    #[error(transparent)]
    RuntimeSchema(#[from] RuntimeSchemaError),
}
```

Existing domain variants are retained exactly unless the deletion inventory
explicitly replaces their stringly identity/layout/type use. The old direct
`Nominal(#[from] RuntimeNominalRecordError)` and generic string `Field` mapping
are not used for errors covered by the typed variants above.
