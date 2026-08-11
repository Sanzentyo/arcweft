# Exact Rust-shaped owners and APIs

All snippets in this document are normative Rust shapes, not a production
overlay. Names, fields, visibility, derives, constructors, accessors, and error
variants are fixed. Private algorithmic organization may vary only when the
observable contract is unchanged.

## 1. Core opaque identities

Owner: `arcweft_core::pattern` (the existing checked-type owner module).

```rust
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
pub struct RuntimeOpaqueTypeProducerId(RuntimeNominalTypeId);

impl RuntimeOpaqueTypeProducerId {
    pub fn try_new(
        value: impl Into<String>,
    ) -> Result<Self, RuntimeIdentityError>;

    pub const fn from_nominal(value: RuntimeNominalTypeId) -> Self;
    pub const fn nominal(&self) -> &RuntimeNominalTypeId;
    pub fn as_str(&self) -> &str;
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[repr(u8)]
pub enum RuntimeOpaqueTypeAdmission {
    ExactIdentity = 0,
    ProducerWide = 1,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeOpaqueTypeOwner {
    producer: RuntimeOpaqueTypeProducerId,
    semantic_identity: RuntimeSemanticTypeId,
    admission: RuntimeOpaqueTypeAdmission,
}

impl RuntimeOpaqueTypeOwner {
    pub const fn exact(
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Self;

    pub const fn producer_wide(
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Self;

    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId;
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn admission(&self) -> RuntimeOpaqueTypeAdmission;

    /// Static assignability: exact equality, or expected producer-wide from
    /// an exact actual row owned by the same producer.
    pub fn accepts_owner(&self, actual: &Self) -> bool;

    /// Runtime acceptance against exact evidence carried by a value.
    pub fn accepts_opaque_value(&self, actual: &RuntimeOpaqueValue) -> bool;

    /// Only `ExactIdentity` can create a concrete value.
    pub fn try_wrap(
        &self,
        payload: RuntimeValue,
    ) -> Result<RuntimeValue, RuntimeOpaqueValueError>;
}
```

`RuntimeOpaqueTypeProducerId` deliberately reuses validation from
`RuntimeNominalTypeId` but is a distinct type so opaque producer identity cannot
be passed accidentally as nominal layout identity.

## 2. Concrete opaque runtime value

Owner: `arcweft_core::value`.

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeOpaqueValue {
    producer: RuntimeOpaqueTypeProducerId,
    semantic_identity: RuntimeSemanticTypeId,
    payload: Box<RuntimeValue>,
}

impl RuntimeOpaqueValue {
    pub const fn producer(&self) -> &RuntimeOpaqueTypeProducerId;
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;
    pub const fn payload(&self) -> &RuntimeValue;
    pub fn into_payload(self) -> RuntimeValue;
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeOpaqueValueError {
    #[error("producer-wide opaque type is not a concrete runtime value owner")]
    NonConcreteOwner {
        producer: RuntimeOpaqueTypeProducerId,
        semantic_identity: RuntimeSemanticTypeId,
    },
}
```

`RuntimeOpaqueValue` has no public constructor. It is created only by
`RuntimeOpaqueTypeOwner::try_wrap`, which rejects `ProducerWide`. A concrete
value therefore always carries exact producer/semantic evidence. Producer
crates validate their domain payload before calling `try_wrap`.

## 3. Existing enums extended in place

```rust
pub enum RuntimeCheckedType {
    // retained variants
    Nominal {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash, // retained parent correction
    },
    Opaque {
        owner: RuntimeOpaqueTypeOwner,
    },
    // retained variants
}

impl RuntimeCheckedType {
    pub fn accepts_value(&self, value: &RuntimeValue) -> bool;
    pub fn variant_identity(&self) -> Option<RuntimeVariantIdentity>;
    pub fn variant_case(&self, ordinal: u32) -> Option<RuntimeCheckedVariantCase>;
}

pub enum RuntimeValue {
    // retained variants
    Opaque(RuntimeOpaqueValue),
}
```

`variant_case` returns an owned descriptor because Result/Option cases are
synthesized by the inherent implementation while nominal cases are cloned from
the retained vector. `accepts_variant_case` is deleted.

## 4. Acceptance algorithm

```rust
impl RuntimeOpaqueTypeOwner {
    pub fn accepts_owner(&self, actual: &Self) -> bool {
        self == actual
            || (self.admission == RuntimeOpaqueTypeAdmission::ProducerWide
                && actual.admission == RuntimeOpaqueTypeAdmission::ExactIdentity
                && self.producer == actual.producer)
    }

    pub fn accepts_opaque_value(&self, actual: &RuntimeOpaqueValue) -> bool {
        self.producer == actual.producer
            && (self.admission == RuntimeOpaqueTypeAdmission::ProducerWide
                || self.semantic_identity == actual.semantic_identity)
    }
}
```

No producer-wide concrete value exists. Two distinct producer-wide rows do not
become assignable merely because their producer matches; exact row equality is
required for wide-to-wide static compatibility.

## 5. Accepted nominal evidence retained in the type

Owner: existing `arcweft_lang_sema::env::nominal` and
`arcweft_lang_sema::types::AcceptedNominalType`.

```rust
pub enum AcceptedNominalSemantics {
    Exact(TypeKind),
    Opaque {
        producer: RuntimeOpaqueTypeProducerId,
    },
    Character(CharacterNominalType),
}

impl AcceptedNominalRecord {
    pub fn try_new_opaque(
        id: AcceptedNominalId,
        arity: u16,
        producer: RuntimeOpaqueTypeProducerId,
        origin: AcceptedNominalOrigin,
        source: Option<SourceSpan>,
    ) -> Result<Self, AcceptedNominalCatalogError>;
}

pub struct AcceptedNominalType {
    declaration: Arc<AcceptedNominalId>,
    arguments: Box<[TypeKind]>,
    producer: RuntimeOpaqueTypeProducerId,
}

impl AcceptedNominalType {
    pub(crate) fn new(
        declaration: AcceptedNominalId,
        arguments: impl Into<Box<[TypeKind]>>,
        producer: RuntimeOpaqueTypeProducerId,
    ) -> Self;

    pub const fn runtime_producer(&self) -> &RuntimeOpaqueTypeProducerId;

    pub fn runtime_opaque_owner(
        &self,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> RuntimeOpaqueTypeOwner;
}
```

There is no `Option<RuntimeOpaqueTypeProducerId>` and no producerless `Opaque`
variant after A1.2.

## 6. Runtime-plan shapes and projection path

```rust
pub enum RuntimeTypeShape {
    // retained shapes
    Opaque {
        producer: RuntimeOpaqueTypeProducerId,
        admission: RuntimeOpaqueTypeAdmission,
    },
    // `Named` deleted
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RuntimeTypeProjectionStep {
    SequenceItem,
    TupleItem(u32),
    ChoiceAlternative(u32),
    ResultOk,
    ResultError,
    OptionItem,
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeTypeProjectionPath(Box<[RuntimeTypeProjectionStep]>);

impl RuntimeTypeProjectionPath {
    pub const fn root() -> Self;
    pub fn pushed(&self, step: RuntimeTypeProjectionStep) -> Self;
    pub const fn steps(&self) -> &[RuntimeTypeProjectionStep];
}

impl RuntimeNormalizedType {
    pub fn checked_type(
        &self,
    ) -> Result<RuntimeCheckedType, RuntimeCheckedTypeProjectionError>;
}
```

Projection is pre-order and left-to-right. Result projects `ok` before `error`;
Option projects its item; tuple/choice use source order; sequence projects its
item once. The first typed error is returned.

## 7. CharacterDialogue producer APIs

Owner behavior is added to the original sema/dialogue types.

```rust
impl CharacterDialogueRuntimeSchema<'_> {
    pub fn opaque_type_producer() -> RuntimeOpaqueTypeProducerId;

    pub fn try_decode_opaque(
        &self,
        value: &RuntimeOpaqueValue,
    ) -> Result<CharacterDialogueValue, CharacterDialogueValueError>;
}

impl CharacterDialogueType {
    pub fn runtime_opaque_owner(
        &self,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> RuntimeOpaqueTypeOwner;
}

impl CharacterDialogueValue {
    pub fn try_into_runtime_value(
        self,
        owner: &RuntimeOpaqueTypeOwner,
    ) -> Result<RuntimeValue, CharacterDialogueValueError>;
}
```

`CharacterDialogueType::Exact(_)` creates `ExactIdentity`;
`CharacterDialogueType::Any` creates `ProducerWide`. `try_into_runtime_value`
requires the canonical producer and an exact owner, then wraps the existing
validated nominal-record payload. `try_decode_opaque` checks producer, decodes
and validates the payload through the existing schema, recomputes the exact
semantic identity from the decoded character type, and requires equality.

## 8. No generic producer trait

No `RuntimeOpaqueProducer` trait, callback table, extension trait, global
registry, or optional predicate is introduced. Each Arcweft producer extends
its existing inherent implementation and calls the core owner/value APIs.
