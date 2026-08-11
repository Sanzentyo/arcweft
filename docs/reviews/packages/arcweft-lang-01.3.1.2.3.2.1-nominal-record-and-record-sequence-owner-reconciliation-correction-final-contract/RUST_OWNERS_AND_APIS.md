# Rust owners and APIs

The declarations below are normative Rust-shaped targets. They describe the
final surface; they are not a production overlay.

## 1. Core nominal layout owner

Owner: `arcweft_core::value::nominal_record`, re-exported from
`arcweft_core::value`.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeNominalRecordLayout {
    nominal: RuntimeNominalTypeId,
    semantic_identity: RuntimeSemanticTypeId,
    layout: TypeLayoutHash,
    fields: Box<[RuntimeNominalRecordLayoutField]>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeNominalRecordLayoutField {
    name: String,
    checked_type: RuntimeCheckedType,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalRecordLayoutError {
    #[error("nominal record layout has {actual} fields, exceeding the {maximum}-field identity space")]
    TooManyFields { actual: usize, maximum: u32 },

    #[error("nominal record layout contains duplicate field `{name}`")]
    DuplicateFieldName { name: String },

    #[error("nominal record layout field {ordinal} (`{name}`) has invalid identity")]
    InvalidFieldIdentity {
        ordinal: usize,
        name: String,
        source: RuntimeRecordFieldIdError,
    },
}

impl RuntimeNominalRecordLayout {
    pub fn try_from_checked_projection(
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
        fields_in_layout_order: Vec<(String, RuntimeCheckedType)>,
    ) -> Result<Self, RuntimeNominalRecordLayoutError>;

    #[must_use]
    pub const fn nominal(&self) -> &RuntimeNominalTypeId;

    #[must_use]
    pub const fn semantic_identity(&self) -> RuntimeSemanticTypeId;

    #[must_use]
    pub const fn layout(&self) -> TypeLayoutHash;

    #[must_use]
    pub fn fields(&self) -> &[RuntimeNominalRecordLayoutField];

    #[must_use]
    pub fn len(&self) -> usize;

    #[must_use]
    pub fn is_empty(&self) -> bool;

    #[must_use]
    pub fn field_id(&self, zero_based_ordinal: usize)
        -> Option<RuntimeRecordFieldId>;

    #[must_use]
    pub fn field_by_id(
        &self,
        field: RuntimeRecordFieldId,
    ) -> Option<&RuntimeNominalRecordLayoutField>;

    #[must_use]
    pub fn field_by_name(
        &self,
        name: &str,
    ) -> Option<(RuntimeRecordFieldId, &RuntimeNominalRecordLayoutField)>;

    #[must_use]
    pub fn checked_type(&self) -> RuntimeCheckedType;
}

impl RuntimeNominalRecordLayoutField {
    #[must_use]
    pub fn name(&self) -> &str;

    #[must_use]
    pub const fn checked_type(&self) -> &RuntimeCheckedType;
}
```

The layout-field constructor is private. The only public construction entry is
`try_from_checked_projection`, which validates the complete ordered aggregate.
The constructor does not recompute `TypeLayoutHash`; it receives the result of
the existing `RuntimeTypeSchema::try_layout_hash` canonicalization performed by
the compiler projection owner. This keeps the core value module below the
entry-schema module and prevents a second hash grammar.

## 2. Canonical schema-to-layout projection

Owner: the existing compiler `RuntimeSchemaProjection` implementation in
`arcweft-compiler/src/project/entry_runtime.rs`, extended on that original impl
rather than wrapped by an ad-hoc helper.

```rust
pub(super) enum EntryRuntimeProjectionError {
    // existing variants unchanged

    #[error("runtime schema for nominal `{nominal}` cannot be canonically encoded")]
    NominalLayoutHash {
        nominal: String,
        #[source]
        source: RuntimeSchemaError,
    },

    #[error("checked nominal schema digest for `{nominal}` differs from the projected runtime schema hash")]
    NominalSchemaDigestMismatch {
        nominal: String,
        checked: [u8; 32],
        projected: TypeLayoutHash,
    },
}

impl RuntimeSchemaProjection {
    fn schema(shape: &TypeShape) -> RuntimeTypeSchema;

    fn layout_hash(
        nominal: &str,
        schema: &RuntimeTypeSchema,
    ) -> Result<TypeLayoutHash, EntryRuntimeProjectionError>;

    fn nominal(
        checked: &CheckedNominalRole,
    ) -> Result<RuntimeNominalRole, EntryRuntimeProjectionError>;
}
```

`layout_hash` is only an error-context adapter around
`RuntimeTypeSchema::try_layout_hash`; it does not hash bytes itself. `nominal`
projects the schema once, obtains the hash through `layout_hash`, compares its
32 bytes with `checked.schema_digest().as_bytes()`, and only then constructs
`RuntimeNominalRole { identity, layout, schema }`. The accepted nominal-record
projection uses the same `schema` plus `layout_hash` methods and discards the
transient schema after the descriptor is built. No parallel projection-error
enum is introduced.

The canonical domain/version/tag/order remain exactly the existing
`arcweft.nominal-schema\0`, version 1 encoding. This correction neither adds a
`TypeLayoutHash` constructor nor changes its ordering, Serde, or fixed bytes.

## 3. Checked type correction

Owner: existing `arcweft_core::pattern::RuntimeCheckedType`.

```rust
pub enum RuntimeCheckedType {
    // existing non-nominal variants unchanged
    Nominal {
        nominal: RuntimeNominalTypeId,
        semantic_identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    },
    // existing remaining variants unchanged
}

impl RuntimeCheckedType {
    #[must_use]
    pub fn accepts_value(&self, value: &RuntimeValue) -> bool;
}
```

`accepts_value` subsumes and deletes the private free function
`runtime_value_matches_pattern_type`. Its private recursive implementation
retains the existing nesting limit. The nominal branch accepts a
`RuntimeValue::NominalRecord` only when both `type_id` and `layout` match. It
cannot compare `semantic_identity`, because that identity is compiler
projection provenance and is intentionally absent from runtime values.

All Arcweft-owned consumers call this inherent method; no extension trait or
ad-hoc helper is added.

## 4. Runtime-plan nominal facts

Owner: `arcweft_runtime_plan::semantic_facts`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedNominal {
    declaration: ProjectNominalDeclarationId,
    owner: ItemId,
    identity: RuntimeSemanticTypeId,
    layout: TypeLayoutHash,
}

impl RuntimeResolvedNominal {
    pub const fn new(
        declaration: ProjectNominalDeclarationId,
        owner: ItemId,
        identity: RuntimeSemanticTypeId,
        layout: TypeLayoutHash,
    ) -> Self;

    pub const fn layout(&self) -> TypeLayoutHash;

    #[must_use]
    pub fn checked_type(&self) -> RuntimeCheckedType;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeResolvedNominalRecord {
    nominal: RuntimeResolvedNominal,
    layout: Arc<RuntimeNominalRecordLayout>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalRecordFactError {
    #[error("nominal record fact has runtime identity {actual:?}, expected {expected:?}")]
    NominalIdentity {
        expected: RuntimeNominalTypeId,
        actual: RuntimeNominalTypeId,
    },

    #[error("nominal record fact has a different semantic identity")]
    SemanticIdentity {
        expected: RuntimeSemanticTypeId,
        actual: RuntimeSemanticTypeId,
    },

    #[error("nominal record fact has a different layout identity")]
    LayoutIdentity {
        expected: TypeLayoutHash,
        actual: TypeLayoutHash,
    },
}

impl RuntimeResolvedNominalRecord {
    pub fn try_new(
        nominal: RuntimeResolvedNominal,
        layout: Arc<RuntimeNominalRecordLayout>,
    ) -> Result<Self, RuntimeNominalRecordFactError>;

    pub const fn nominal(&self) -> &RuntimeResolvedNominal;
    pub const fn layout(&self) -> &Arc<RuntimeNominalRecordLayout>;
    pub fn checked_type(&self) -> RuntimeCheckedType;
}
```

`RuntimeTypeShape::Named` gains a `layout: TypeLayoutHash` field.
`RuntimeTypeShape::ProjectNominal` uses `RuntimeResolvedNominal::layout`.
`RuntimeNormalizedType::checked_type` remains an inherent recursive projection
and supplies layout to every nominal checked type.

The fact input/set replacement is exact:

```rust
pub struct RuntimePlanSemanticFactInput {
    // ...
    nominal_records: Vec<(ExprId, RuntimeResolvedNominalRecord)>,
    pattern_nominal_records: Vec<(PatternId, RuntimeResolvedNominalRecord)>,
    // no nominals/pattern_nominals record maps
}

impl RuntimePlanSemanticFactInput {
    pub fn push_nominal_record(
        &mut self,
        owner: ExprId,
        nominal: RuntimeResolvedNominalRecord,
    );

    pub fn push_pattern_nominal_record(
        &mut self,
        owner: PatternId,
        nominal: RuntimeResolvedNominalRecord,
    );
}

impl RuntimePlanSemanticFacts {
    pub fn nominal_record(
        &self,
        expression: ExprId,
    ) -> Option<&RuntimeResolvedNominalRecord>;

    pub fn pattern_nominal_record(
        &self,
        pattern: PatternId,
    ) -> Option<&RuntimeResolvedNominalRecord>;
}
```

`RuntimeSemanticFactsError` gains these publication variants:

```rust
WrongNominalRecordItemFamily { item: ItemId },
NominalRecordFieldCount {
    item: ItemId,
    expected: usize,
    actual: usize,
},
NominalRecordFieldName {
    item: ItemId,
    ordinal: usize,
    expected: String,
    actual: String,
},
ConflictingNominalRecordLayout {
    nominal: RuntimeNominalTypeId,
    semantic_identity: RuntimeSemanticTypeId,
    layout: TypeLayoutHash,
},
UnresolvedNominalLayout {
    item: ItemId,
    nominal: RuntimeNominalTypeId,
},
UnrepresentableNominalRecordField {
    item: ItemId,
    ordinal: usize,
    name: String,
},
```

`UnresolvedNominalLayout` is emitted only when a nested nominal checked type
cannot resolve its already projected layout in the generation-local nominal
catalog. It forbids semantic-digest substitution, nominal-ID-only lowering,
and local hashing. Canonical schema encoding failures and checked-digest drift
remain compiler-owned `EntryRuntimeProjectionError` variants and occur before fact publication.

## 5. Nominal expression carrier

Owner: existing `arcweft_core::value` expression model.

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeNominalRecordExpr {
    layout: Arc<RuntimeNominalRecordLayout>,
    initializers: Box<[RuntimeNominalRecordFieldExpr]>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeNominalRecordFieldExpr {
    field: RuntimeRecordFieldId,
    name: String,
    value: RuntimeExpr,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeNominalRecordInitializerError {
    #[error("nominal record initializer has {actual} fields, exceeding the {maximum}-field identity space")]
    TooManyFields { actual: usize, maximum: u32 },

    #[error("nominal record initializer contains duplicate field `{name}`")]
    DuplicateName { name: String },

    #[error("nominal record initializer contains unknown field `{name}`")]
    UnknownField { name: String },

    #[error("nominal record initializer is missing field {field:?} (`{name}`)")]
    MissingField {
        field: RuntimeRecordFieldId,
        name: String,
    },

    #[error("nominal record initializer field {ordinal} (`{name}`) has invalid identity")]
    InvalidFieldIdentity {
        ordinal: usize,
        name: String,
        source: RuntimeRecordFieldIdError,
    },

    #[error("nominal record initializer `{name}` carries field {actual:?}, expected {expected:?}")]
    FieldIdentityMismatch {
        name: String,
        expected: RuntimeRecordFieldId,
        actual: RuntimeRecordFieldId,
    },
}

impl RuntimeNominalRecordExpr {
    pub fn try_from_checked_initializers(
        layout: Arc<RuntimeNominalRecordLayout>,
        initializers_in_authored_order: Vec<(String, RuntimeExpr)>,
    ) -> Result<Self, RuntimeNominalRecordInitializerError>;

    pub fn validate(&self) -> Result<(), RuntimeNominalRecordInitializerError>;

    pub const fn layout(&self) -> &Arc<RuntimeNominalRecordLayout>;
    pub fn initializers(&self) -> &[RuntimeNominalRecordFieldExpr];
}

impl RuntimeNominalRecordFieldExpr {
    pub const fn field(&self) -> RuntimeRecordFieldId;
    pub fn name(&self) -> &str;
    pub const fn value(&self) -> &RuntimeExpr;
}

pub enum RuntimeExpr {
    // existing variants
    NominalRecord(RuntimeNominalRecordExpr),
    // existing variants
}
```

The field-expression constructor is private. Validation of a deserialized
carrier repeats count, duplicate-name, authoritative name-to-ID, and missing
field checks before plan publication.

## 6. Nominal runtime value

Owner: existing `arcweft_core::value::RuntimeNominalRecordValue`. Its stored
fields remain unchanged.

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeNominalRecordValue {
    type_id: RuntimeNominalTypeId,
    layout: TypeLayoutHash,
    fields: Vec<RuntimeValue>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum RuntimeNominalRecordError {
    #[error("expected nominal type `{expected:?}`, found `{actual:?}`")]
    Type {
        expected: RuntimeNominalTypeId,
        actual: RuntimeNominalTypeId,
    },

    #[error("nominal record layout does not match the expected layout")]
    Layout {
        expected: TypeLayoutHash,
        actual: TypeLayoutHash,
    },

    #[error("nominal record has {actual} fields, expected {expected}")]
    FieldCount { expected: usize, actual: usize },

    #[error("nominal record layout ordinal {ordinal} has invalid field identity")]
    InvalidFieldIdentity {
        ordinal: usize,
        source: RuntimeRecordFieldIdError,
    },

    #[error("nominal record field {field:?} (`{name}`) does not satisfy {expected:?}")]
    FieldType {
        field: RuntimeRecordFieldId,
        name: String,
        expected: RuntimeCheckedType,
    },
}

impl RuntimeNominalRecordValue {
    pub(crate) fn try_from_accepted_layout(
        layout: &RuntimeNominalRecordLayout,
        fields_in_layout_order: Vec<RuntimeValue>,
    ) -> Result<Self, RuntimeNominalRecordError>;

    pub fn validate_against_layout(
        &self,
        layout: &RuntimeNominalRecordLayout,
    ) -> Result<(), RuntimeNominalRecordError>;

    pub const fn type_id(&self) -> &RuntimeNominalTypeId;
    pub const fn layout(&self) -> TypeLayoutHash;
    pub fn fields(&self) -> &[RuntimeValue];
    pub fn into_fields(self) -> Vec<RuntimeValue>;

    pub fn field_id(
        &self,
        zero_based_ordinal: usize,
    ) -> Option<RuntimeRecordFieldId>;

    pub fn field(
        &self,
        field: RuntimeRecordFieldId,
    ) -> Option<&RuntimeValue>;
}
```

Deleted in this cut:

```rust
pub const fn RuntimeNominalRecordValue::new(...);
pub fn RuntimeNominalRecordValue::validate_shape(...);
```

No replacement compatibility constructor is retained.

## 6. Pattern owner

The existing record pattern changes in place:

```rust
pub enum RuntimePattern {
    // ...
    Record {
        nominal_layout: Option<Arc<RuntimeNominalRecordLayout>>,
        fields: Vec<RuntimeRecordPatternField>,
        rest: bool,
    },
    // ...
}
```

No new record value enum or parallel pattern schema is introduced.

## 7. Anonymous carrier and admission

These are the accepted parent declarations, with interim `Clone`/Serde derives
retained only while the enclosing live value model requires them:

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RuntimeFieldValue {
    field: RuntimeRecordFieldId,
    name: String,
    value: RuntimeValue,
}

impl RuntimeFieldValue {
    pub(crate) fn new_accepted(
        field: RuntimeRecordFieldId,
        name: String,
        value: RuntimeValue,
    ) -> Self;

    pub const fn field(&self) -> RuntimeRecordFieldId;
    pub fn name(&self) -> &str;
    pub fn value(&self) -> &RuntimeValue;
    pub(crate) fn value_mut(&mut self) -> &mut RuntimeValue;
    pub(crate) fn into_value(self) -> RuntimeValue;
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeRecordAdmissionError {
    #[error("runtime record has duplicate field name `{name}`")]
    DuplicateName { name: String },

    #[error("runtime record has too many fields")]
    TooManyFields,

    #[error("runtime record field `{name}` has invalid identity")]
    InvalidFieldIdentity {
        name: String,
        source: RuntimeRecordFieldIdError,
    },
}

impl RuntimeValue {
    pub(crate) fn try_record(
        fields_in_authored_order: Vec<(String, RuntimeValue)>,
    ) -> Result<Self, RuntimeRecordAdmissionError>;
}
```

Anonymous admission preflights count, rejects the first duplicate name in
authored order, then assigns contiguous one-based IDs and publishes atomically.

## 8. Record sequence carrier and sole error owner

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RecordSeqField {
    field: RuntimeRecordFieldId,
    name: String,
    values: RuntimeSeq,
}

impl RecordSeqField {
    pub(crate) fn new_accepted(
        field: RuntimeRecordFieldId,
        name: String,
        values: RuntimeSeq,
    ) -> Self;

    pub const fn field(&self) -> RuntimeRecordFieldId;
    pub fn name(&self) -> &str;
    pub fn values(&self) -> &RuntimeSeq;
    pub(crate) fn into_values(self) -> RuntimeSeq;
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeSeqError {
    #[error("sequence column {ordinal} length {actual} does not match expected length {expected}")]
    ColumnLength {
        ordinal: usize,
        expected: usize,
        actual: usize,
    },

    #[error("record sequence contains duplicate field `{field}`")]
    DuplicateRecordField { field: String },

    #[error("record sequence has {actual} fields, exceeding the {maximum}-field identity space")]
    TooManyRecordFields { actual: usize, maximum: u32 },

    #[error("record sequence field {ordinal} (`{field}`) has invalid identity")]
    InvalidRecordFieldIdentity {
        ordinal: usize,
        field: String,
        source: RuntimeRecordFieldIdError,
    },
}

impl RecordSeq {
    pub(crate) fn try_from_accepted_fields(
        rows: usize,
        fields_in_accepted_order: Vec<(String, RuntimeSeq)>,
    ) -> Result<Self, RuntimeSeqError>;
}

impl RuntimeSeq {
    pub fn record_columns(
        rows: usize,
        fields_in_accepted_order: Vec<(String, RuntimeSeq)>,
    ) -> Result<Self, RuntimeSeqError>;
}
```

Deleted in the same cut:

```rust
pub fn RecordSeq::new(len: usize, fields: Vec<RecordSeqField>) -> ...;
pub fn RuntimeSeq::record_columns(len: usize, fields: Vec<RecordSeqField>) -> ...;
```

There is no `RecordSeqError` declaration or alias.
