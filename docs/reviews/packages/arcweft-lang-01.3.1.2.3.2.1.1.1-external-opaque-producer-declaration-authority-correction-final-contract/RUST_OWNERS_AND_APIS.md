# Exact Rust-shaped owners and APIs

All declarations are target shapes, not a production overlay. Private internal
helpers may vary only when error precedence and public behavior remain exact.

## 1. Adapter-owned producer ID

Owner: `crates/arcweft-adapter-context/src/manifest/nominal.rs`; re-export from
`arcweft_adapter_context::manifest` and the crate's existing public facade.

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AdapterOpaqueTypeProducerId(String);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdapterOpaqueTypeProducerIdError {
    #[error("adapter opaque type producer ID must not be empty")]
    Empty,
    #[error(
        "adapter opaque type producer ID contains a control character at byte {byte}"
    )]
    ControlCharacter { byte: usize },
    #[error(
        "adapter opaque type producer ID `{producer}` uses the reserved `std.` namespace"
    )]
    ReservedStandardNamespace { producer: String },
}

impl AdapterOpaqueTypeProducerId {
    pub fn try_new(
        value: impl Into<String>,
    ) -> Result<Self, AdapterOpaqueTypeProducerIdError>;

    pub fn as_str(&self) -> &str;

    pub(crate) fn validate(
        value: &str,
    ) -> Result<(), AdapterOpaqueTypeProducerIdError>;
}

impl std::fmt::Display for AdapterOpaqueTypeProducerId;
impl std::str::FromStr for AdapterOpaqueTypeProducerId {
    type Err = AdapterOpaqueTypeProducerIdError;
}
impl TryFrom<String> for AdapterOpaqueTypeProducerId {
    type Error = AdapterOpaqueTypeProducerIdError;
}
impl<'de> Deserialize<'de> for AdapterOpaqueTypeProducerId;
```

Validation is exactly:

```rust
if value.is_empty() { return Err(Empty); }
if let Some((byte, _)) = value.char_indices().find(|(_, ch)| ch.is_control()) {
    return Err(ControlCharacter { byte });
}
if value.starts_with("std.") {
    return Err(ReservedStandardNamespace { producer: value.to_owned() });
}
```

No `Copy`, `Default`, unchecked constructor, public inner-string accessor,
normalization, trimming, or semantic maximum is added.

## 2. Rust-ABI-owned producer ID

Owner: new `crates/arcweft-rust-abi/src/producer.rs`; `lib.rs` contains
`mod producer;` and publicly re-exports the two symbols.

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ArcweftRustOpaqueTypeProducerId(String);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ArcweftRustOpaqueTypeProducerIdError {
    #[error("Rust ABI opaque type producer ID must not be empty")]
    Empty,
    #[error(
        "Rust ABI opaque type producer ID contains a control character at byte {byte}"
    )]
    ControlCharacter { byte: usize },
    #[error(
        "Rust ABI opaque type producer ID `{producer}` uses the reserved `std.` namespace"
    )]
    ReservedStandardNamespace { producer: String },
}

impl ArcweftRustOpaqueTypeProducerId {
    pub fn try_new(
        value: impl Into<String>,
    ) -> Result<Self, ArcweftRustOpaqueTypeProducerIdError>;
    pub fn as_str(&self) -> &str;
    pub(crate) fn validate(
        value: &str,
    ) -> Result<(), ArcweftRustOpaqueTypeProducerIdError>;
}

impl std::fmt::Display for ArcweftRustOpaqueTypeProducerId;
impl std::str::FromStr for ArcweftRustOpaqueTypeProducerId {
    type Err = ArcweftRustOpaqueTypeProducerIdError;
}
impl TryFrom<String> for ArcweftRustOpaqueTypeProducerId {
    type Error = ArcweftRustOpaqueTypeProducerIdError;
}
impl<'de> Deserialize<'de> for ArcweftRustOpaqueTypeProducerId;
```

The implementation is locally duplicated rather than introducing an upward or
cross-sibling dependency. The observable grammar and tests must stay identical.

## 3. Adapter nominal declaration

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AdapterNominalDeclaration {
    path: AdapterNominalPath,
    arity: u16,
    opaque_producer: AdapterOpaqueTypeProducerId,
    visibility: AdapterNominalVisibility,
    source_label: String,
}

impl AdapterNominalDeclaration {
    pub fn try_new(
        path: AdapterNominalPath,
        arity: u16,
        opaque_producer: AdapterOpaqueTypeProducerId,
        visibility: AdapterNominalVisibility,
        source_label: impl Into<String>,
    ) -> Result<Self, AdapterTypeModelError>;

    pub const fn path(&self) -> &AdapterNominalPath;
    pub const fn arity(&self) -> u16;
    pub const fn opaque_producer(&self) -> &AdapterOpaqueTypeProducerId;
    pub const fn visibility(&self) -> AdapterNominalVisibility;
    pub fn source_label(&self) -> &str;
}
```

`AdapterTypeModelError` retains its existing arity/source-label variants.
Producer construction errors are raised before this constructor by codecs and
programmatic callers and are not flattened into `AdapterTypeModelError`.

## 4. Rust ABI model

```rust
pub const ARCWEFT_RUST_ABI_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArcweftRustManifest {
    pub schema_version: u32,
    pub package: ArcweftRustPackage,
    pub types: Vec<ArcweftRustTypeDecl>,
    pub functions: Vec<ArcweftRustFunction>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArcweftRustTypeDecl {
    pub path: ArcweftRustTypePath,
    pub rust_path: String,
    pub opaque_producer: ArcweftRustOpaqueTypeProducerId,
    pub parameters: Vec<ArcweftRustTypeParameter>,
    pub kind: ArcweftRustTypeKind,
}

impl ArcweftRustTypeDecl {
    pub const fn opaque_producer(&self) -> &ArcweftRustOpaqueTypeProducerId;
}
```

The manifest builder's `new` path always writes version 2. The public manifest
root and type declaration do not derive `Deserialize`; decoding uses private
schema-2 DTOs after header preflight. Other independently valid Rust ABI value
models may retain serde where they do not create a manifest/version bypass.

`ArcweftRustManifestError` gains:

```rust
#[error("unsupported Rust ABI schema {found}; expected {expected}")]
UnsupportedSchema { found: u32, expected: u32 },

#[error("Rust type declaration {declaration} has invalid opaque producer: {error}")]
InvalidOpaqueProducer {
    declaration: usize,
    error: ArcweftRustOpaqueTypeProducerIdError,
},
```

`validate` checks schema version, then every producer spelling/reservation in
declaration order, then package identity and the existing structural model.
An empty `types` list does not require any producer.

## 5. Mounted Rust type accessor

`AdapterRustType` retains exactly its existing fields:

```rust
pub struct AdapterRustType {
    package: ArcweftRustPackage,
    accepted_path: AdapterNominalPath,
    decl: ArcweftRustTypeDecl,
}

impl AdapterRustType {
    pub const fn opaque_producer(&self) -> &ArcweftRustOpaqueTypeProducerId {
        self.decl.opaque_producer()
    }
}
```

There is no `opaque_producer` field, setter, `&mut` accessor, constructor
argument, override callback, or map entry on `AdapterRustType`.

## 6. Accepted inventory and catalog APIs

```rust
pub struct AcceptedNominalInventoryInput {
    id: AcceptedNominalId,
    arity: u16,
    runtime_producer: RuntimeOpaqueTypeProducerId,
    visibility: AcceptedNominalVisibility,
    origin: AcceptedNominalOrigin,
    source: SourceSpan,
    item: EnvironmentPublicationItemId,
}

impl AcceptedNominalInventoryInput {
    pub fn new(
        id: AcceptedNominalId,
        arity: u16,
        runtime_producer: RuntimeOpaqueTypeProducerId,
        visibility: AcceptedNominalVisibility,
        origin: AcceptedNominalOrigin,
        source: SourceSpan,
        item: EnvironmentPublicationItemId,
    ) -> Self;

    pub const fn runtime_producer(&self) -> &RuntimeOpaqueTypeProducerId;
}
```

The parent package's final shapes remain:

```rust
pub enum AcceptedNominalSemantics {
    Opaque { producer: RuntimeOpaqueTypeProducerId },
    // retained non-opaque variants
}

impl AcceptedNominalRecord {
    pub fn try_new_opaque(
        id: AcceptedNominalId,
        arity: u16,
        producer: RuntimeOpaqueTypeProducerId,
        origin: AcceptedNominalOrigin,
        source: SourceSpan,
    ) -> Result<Self, AcceptedNominalCatalogError>;
}

pub struct AcceptedNominalType {
    declaration: Arc<AcceptedNominalId>,
    arguments: Box<[TypeKind]>,
    producer: RuntimeOpaqueTypeProducerId,
}

impl AcceptedNominalType {
    pub fn new(
        declaration: Arc<AcceptedNominalId>,
        arguments: impl Into<Box<[TypeKind]>>,
        producer: RuntimeOpaqueTypeProducerId,
    ) -> Self;
    pub const fn runtime_producer(&self) -> &RuntimeOpaqueTypeProducerId;
}
```

Instantiation obtains the producer from the accepted record. Substitution
rebuilds the type with the original `runtime_producer().clone()`; it never
recomputes or validates a new value from the declaration/arguments.

## 7. Adapter-sema sole conversion owner

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalOpaqueProducerSourceKind {
    AdapterNominal,
    RustExport,
}

enum ExternalOpaqueProducer<'a> {
    Adapter(&'a AdapterOpaqueTypeProducerId),
    Rust(&'a ArcweftRustOpaqueTypeProducerId),
}

impl ExternalOpaqueProducer<'_> {
    fn as_str(&self) -> &str;
    const fn source_kind(&self) -> ExternalOpaqueProducerSourceKind;
    fn project(
        &self,
        source: SourceSpan,
    ) -> Result<RuntimeOpaqueTypeProducerId, AdapterRegistrationFactsError>;
}
```

This is an Arcweft-owned enum and receives its behavior in its inherent `impl`.
No extension trait or generic producer trait is introduced.

```rust
#[error("invalid {source_kind:?} opaque producer `{producer}`")]
InvalidOpaqueProducer {
    source_kind: ExternalOpaqueProducerSourceKind,
    producer: String,
    source: SourceSpan,
    #[source]
    error: RuntimeIdentityError,
},

#[error("{source_kind:?} opaque producer `{producer}` uses reserved namespace `std.`")]
ReservedOpaqueProducer {
    source_kind: ExternalOpaqueProducerSourceKind,
    producer: String,
    source: SourceSpan,
},

#[error("duplicate generated opaque-producer source for {item:?}")]
DuplicateOpaqueProducerSource {
    item: Box<EnvironmentPublicationItemId>,
},

#[error("missing generated opaque-producer source for {item:?}")]
MissingOpaqueProducerSource {
    item: Box<EnvironmentPublicationItemId>,
},
```

A test-only private raw-text seam may inject invalid lower-layer bytes to prove
the defensive boundary. There is no public unchecked constructor.
