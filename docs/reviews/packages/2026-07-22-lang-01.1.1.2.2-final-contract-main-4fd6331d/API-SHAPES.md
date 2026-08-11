# Exact Rust and API Shapes

The declarations below are normative. Existing fields not shown in an explicitly replaced type are retained unchanged. Constructors validate before storing; downstream code may rely on their invariants.

## 1. `arcweft-rust-abi`: Sans I/O wire model

File: `crates/arcweft-rust-abi/src/lib.rs`

The crate remains data + codecs only. It performs no Cargo discovery, source inspection, filesystem access, or network access.

```rust
#[derive(
    Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct ArcweftRustPackageId(String);

#[derive(
    Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct ArcweftRustTypePathSegment(String);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ArcweftRustTypePath {
    segments: Vec<ArcweftRustTypePathSegment>,
}

#[derive(
    Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct ArcweftRustTypeParameterIndex(u16);

#[derive(
    Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct ArcweftRustTypeParameterName(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustTypeParameter {
    pub index: ArcweftRustTypeParameterIndex,
    pub name: ArcweftRustTypeParameterName,
}
```

Required inherent APIs:

```rust
impl ArcweftRustPackageId {
    pub fn try_new(value: impl Into<String>)
        -> Result<Self, ArcweftRustIdentityError>;
    pub fn as_str(&self) -> &str;
}

impl ArcweftRustTypePathSegment {
    pub fn try_new(value: impl Into<String>)
        -> Result<Self, ArcweftRustIdentityError>;
    pub fn as_str(&self) -> &str;
}

impl ArcweftRustTypePath {
    pub fn try_new(
        segments: impl IntoIterator<Item = ArcweftRustTypePathSegment>,
    ) -> Result<Self, ArcweftRustIdentityError>;
    pub fn segments(&self) -> &[ArcweftRustTypePathSegment];
}

impl ArcweftRustTypeParameterIndex {
    pub fn try_from_usize(value: usize)
        -> Result<Self, ArcweftRustIdentityError>;
    pub const fn get(self) -> usize;
}
```

Identity validation:

- package ID: non-empty, no control character;
- path segment: valid Rust identifier text, excluding raw-prefix spelling in the stored value;
- type path: non-empty; at most 256 segments;
- type parameter indices: contiguous `0..parameters.len()`, maximum 256;
- duplicate package-local type paths: rejected;
- duplicate parameter names or indices: rejected.

The package field is replaced in place:

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustPackage {
    pub id: ArcweftRustPackageId,
    pub version: String,
    #[serde(default)]
    pub metadata_hash: Option<String>,
}
```

The nominal type carrier is replaced in place:

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcweftRustTypeRef {
    Unit,
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    String,
    Char,
    Vec {
        item: Box<ArcweftRustTypeRef>,
    },
    Seq {
        item: Box<ArcweftRustTypeRef>,
    },
    Option {
        item: Box<ArcweftRustTypeRef>,
    },
    Result {
        ok: Box<ArcweftRustTypeRef>,
        error: Box<ArcweftRustTypeRef>,
    },
    Tuple {
        items: Vec<ArcweftRustTypeRef>,
    },
    Nominal {
        package: ArcweftRustPackageId,
        path: ArcweftRustTypePath,
        #[serde(default)]
        arguments: Vec<ArcweftRustTypeRef>,
    },
    TypeParameter {
        index: ArcweftRustTypeParameterIndex,
    },
}
```

`Named` is not present.

The declaration identity and payload shapes are replaced in place:

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustTypeDecl {
    pub path: ArcweftRustTypePath,
    pub rust_path: String,
    #[serde(default)]
    pub parameters: Vec<ArcweftRustTypeParameter>,
    pub kind: ArcweftRustTypeKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcweftRustTypeKind {
    Struct {
        shape: ArcweftRustStructShape,
    },
    Enum {
        variants: Vec<ArcweftRustVariant>,
    },
    Newtype {
        inner: ArcweftRustTypeRef,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcweftRustStructShape {
    Unit,
    Tuple {
        fields: Vec<ArcweftRustTypeRef>,
    },
    Record {
        fields: Vec<ArcweftRustField>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustField {
    pub name: String,
    pub ty: ArcweftRustTypeRef,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ArcweftRustVariant {
    pub name: String,
    pub payload: ArcweftRustVariantPayload,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArcweftRustVariantPayload {
    Unit,
    Tuple {
        fields: Vec<ArcweftRustTypeRef>,
    },
    Record {
        fields: Vec<ArcweftRustField>,
    },
}
```

The manifest validates the complete type graph after deserialization:

```rust
impl ArcweftRustManifest {
    pub fn validate(
        &self,
        limits: ArcweftRustAbiLimits,
    ) -> Result<(), ArcweftRustManifestError>;

    pub fn from_json(
        source: &str,
    ) -> Result<Self, ArcweftRustAbiError>; // decode, schema == 1, validate
}
```

`ArcweftRustAbiLimits::PRODUCTION` uses the accepted nominal production ceilings for type nodes (4,096), depth (256), arguments (256), and parameter count (256). It additionally limits one package-local path to 256 segments. The type graph validator is iterative or bounded recursive; arithmetic uses checked conversion.

`ArcweftType` and `ArcweftTypeMetadata` remain the macro boundary. A derived generic ADT emits a nominal reference whose arguments are the concrete `ArcweftType` arguments:

```rust
impl<T: ArcweftType> ArcweftType for Envelope<T> {
    fn arcweft_type_ref() -> ArcweftRustTypeRef {
        ArcweftRustTypeRef::Nominal {
            package: ArcweftRustPackageId::try_new(env!("CARGO_PKG_NAME"))
                .expect("Cargo package IDs are valid ABI IDs"),
            path: ArcweftRustTypePath::try_new([
                ArcweftRustTypePathSegment::try_new("Envelope")
                    .expect("macro-generated path is valid"),
            ])
            .expect("macro-generated path is non-empty"),
            arguments: vec![T::arcweft_type_ref()],
        }
    }
}
```

The metadata declaration uses `TypeParameter { index }` in its field templates. Lifetime and const generic ADTs remain rejected by the derive macro with typed macro diagnostics. Exported generic functions remain rejected because the checked callable schema has no callable-generic binder.

## 2. `arcweft-adapter-context`: manifest model

File: `crates/arcweft-adapter-context/src/manifest.rs`

```rust
#[derive(
    Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct AdapterEnvironmentOwnerId(String);

#[derive(
    Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct AdapterNominalPathSegment(String);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AdapterNominalPath {
    segments: Box<[AdapterNominalPathSegment]>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AdapterNominalPathPrefix {
    segments: Box<[AdapterNominalPathSegment]>, // empty is legal
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdapterNominalOwner {
    Environment {
        owner: AdapterEnvironmentOwnerId,
    },
    RustPackage {
        package: ArcweftRustPackageId,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct AdapterNominalTypeRef {
    owner: AdapterNominalOwner,
    path: AdapterNominalPath,
    arguments: Box<[AdapterTypeKind]>,
}
```

Required constructors:

```rust
impl AdapterEnvironmentOwnerId {
    pub fn for_adapter(adapter: &AdapterPackageId) -> Self;
    pub fn as_str(&self) -> &str;
}

impl AdapterNominalPath {
    pub fn try_new(
        segments: impl IntoIterator<Item = AdapterNominalPathSegment>,
    ) -> Result<Self, AdapterNominalPathError>;
    pub fn segments(&self) -> &[AdapterNominalPathSegment];
}

impl AdapterNominalPathPrefix {
    pub fn try_new(
        segments: impl IntoIterator<Item = AdapterNominalPathSegment>,
    ) -> Result<Self, AdapterNominalPathError>;
    pub fn join(
        &self,
        local: &ArcweftRustTypePath,
    ) -> Result<AdapterNominalPath, AdapterNominalPathError>;
}

impl AdapterNominalTypeRef {
    pub fn try_new(
        owner: AdapterNominalOwner,
        path: AdapterNominalPath,
        arguments: impl IntoIterator<Item = AdapterTypeKind>,
    ) -> Result<Self, AdapterTypeModelError>;

    pub const fn owner(&self) -> &AdapterNominalOwner;
    pub const fn path(&self) -> &AdapterNominalPath;
    pub fn arguments(&self) -> &[AdapterTypeKind];
}
```

`AdapterTypeKind` is replaced in place:

```rust
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdapterTypeKind {
    Unit,
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    String,
    Char,
    Vec { item: Box<AdapterTypeKind> },
    Seq { item: Box<AdapterTypeKind> },
    Option { item: Box<AdapterTypeKind> },
    Result {
        ok: Box<AdapterTypeKind>,
        error: Box<AdapterTypeKind>,
    },
    Tuple { items: Box<[AdapterTypeKind]> },
    Nominal { nominal: AdapterNominalTypeRef },
}
```

`Named` is not present.

Adapter-native nominal declarations are explicit:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterNominalVisibility {
    Public,
    Private,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AdapterNominalDeclaration {
    path: AdapterNominalPath,
    arity: u16,
    visibility: AdapterNominalVisibility,
    source_label: String,
}
```

The semantic owner for every `AdapterNominalDeclaration` is derived from the containing manifest ID. Callers do not supply another environment owner for declarations.

Rust mounts are stored before manifest ingestion:

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdapterRustPackageMountTable {
    by_package:
        BTreeMap<ArcweftRustPackageId, AdapterNominalPathPrefix>,
}

impl AdapterManifest {
    pub fn try_with_rust_package_mount(
        self,
        package: ArcweftRustPackageId,
        prefix: AdapterNominalPathPrefix,
    ) -> Result<Self, AdapterManifestModelError>;

    pub fn try_with_rust_manifest(
        self,
        manifest: &ArcweftRustManifest,
    ) -> Result<Self, AdapterManifestModelError>;
}
```

`try_with_rust_manifest` requires a mount for the manifest package and for every package referenced by any nested `ArcweftRustTypeRef::Nominal`. It validates package version/hash consistency, maps package-local paths to exact world paths, and stores typed Rust type/function facts. It does not make a semantic `TypeKind`.

`AdapterManifest::with_nominal_declaration` is fallible and detects duplicate local declarations. Standard manifests declare their opaque types explicitly. External module admission creates declarations for public types and inaccessible facts for private types.

## 3. Adapter file carrier

File: `crates/arcweft-adapter-context/src/codec.rs`

The file carrier no longer encodes a type as a string. It uses the recursive tagged `AdapterTypeKind` shape above (or a private isomorphic file type with a total conversion). Example TOML:

```toml
schema_version = 1
id = "inference-tensor"
display_name = "Inference Tensor"

[[nominal_types]]
path = ["TensorF32"]
arity = 1
visibility = "public"
source_label = "TensorF32"

[[rust_package_mounts]]
package = "tensor-runtime"
prefix = ["vendor", "tensor"]

[[functions]]
path = ["infer", "map"]
overload = 0

[[functions.signature.groups]]
index = 0

[[functions.signature.groups.parameters]]
index = 0
name = "value"
passing = "positional_or_named"
presence = "required"
type = { kind = "nominal", nominal = { owner = { kind = "rust_package", package = "tensor-runtime" }, path = ["vendor", "tensor", "TensorF32"], arguments = [{ kind = "f32" }] } }

[functions.signature.result]
kind = "option"
item = { kind = "nominal", nominal = { owner = { kind = "rust_package", package = "tensor-runtime" }, path = ["vendor", "tensor", "TensorF32"], arguments = [{ kind = "f32" }] } }
```

The schema constant stays `1`. Decode is exact and validates constructors. There is no string-type parser in the final codec.

## 4. `arcweft-lang-sema`: accepted nominal lookup and instantiation

Files:

- `crates/arcweft-lang-sema/src/env/nominal.rs`
- `crates/arcweft-lang-sema/src/types/nominal.rs`
- `crates/arcweft-lang-sema/src/nominal/resolver/engine/resolution.rs`

Add `GenericTypeOwnerId::AcceptedNominal` to the existing owner enum, not an extension trait:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GenericTypeOwnerId {
    Callable(CallableDeclarationId),
    Nominal(ProjectNominalDeclarationId),
    AcceptedNominal(AcceptedNominalId),
    AcceptedSource(SourceSpan),
    Detached(DetachedTypeOwnerId),
}
```

Add a world stamp:

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct AcceptedNominalWorldStamp {
    world: ProjectSymbolWorldId,
    revision: ProjectSymbolRevision,
    catalog_digest: AcceptedNominalCatalogDigest,
}

impl AcceptedNominalWorld {
    pub fn stamp(&self) -> AcceptedNominalWorldStamp;
}
```

Add the visibility index:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedNominalVisibilityIndex {
    visible: BTreeMap<AcceptedNominalId, AcceptedNominalSource>,
    inaccessible: BTreeMap<AcceptedNominalId, AcceptedNominalSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedNominalSource {
    declaration: SourceSpan,
    item: EnvironmentPublicationItemId,
}
```

The world owns the index:

```rust
pub struct AcceptedNominalWorld {
    // existing fields
    visibility: Arc<AcceptedNominalVisibilityIndex>,
}
```

Required inherent lookup:

```rust
impl AcceptedNominalWorld {
    pub(crate) fn accepted_record(
        &self,
        requested: &AcceptedNominalId,
    ) -> Result<&AcceptedNominalRecord, AcceptedNominalWorldLookupError>;
}
```

Required inherent instantiation on the original record type:

```rust
impl AcceptedNominalRecord {
    pub(crate) fn try_instantiate(
        &self,
        arguments: impl Into<Box<[TypeKind]>>,
    ) -> Result<TypeKind, AcceptedNominalInstantiationError>;
}
```

Behavior:

- exact owner + exact path;
- arity checked before child instantiation result is committed;
- `Exact` returns the existing exact type;
- `Opaque` returns `TypeKind::AcceptedNominal(AcceptedNominalType::new(...))`;
- `Character` returns the existing character nominal type;
- invalid record semantics are a structured internal contract error;
- no display-name or suffix lookup.

The authored resolver calls these inherent APIs after retaining its existing source lookup, aliases, limits, report, and poison behavior.

## 5. Source-backed environment registration input

New file: `crates/arcweft-lang-sema/src/registration/environment_input.rs`  
Export from the existing `registration.rs`; do not add a directory `mod.rs`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBackedEnvironmentRegistrationInput {
    owner: EnvironmentCallableOwner,
    source: SourceDocumentIdentity,
    manifest_digest: EnvironmentManifestDigest,
    nominal_inventory: Box<[AcceptedNominalInventoryInput]>,
    rust_metadata: Box<[RustTypeMetadataPublicationInput]>,
    callable_records: Box<[EnvironmentCallablePublicationRecordInput]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedNominalInventoryInput {
    id: AcceptedNominalId,
    arity: u16,
    visibility: AcceptedNominalInputVisibility,
    origin: AcceptedNominalOrigin,
    source: SourceSpan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptedNominalInputVisibility {
    Visible,
    Inaccessible,
}
```

The type tree embeds exact source for every node:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentTypeProjectionNode {
    source: SourceSpan,
    kind: EnvironmentTypeProjectionKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentTypeProjectionKind {
    Unit,
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    F32,
    F64,
    String,
    Char,
    Vec(Box<EnvironmentTypeProjectionNode>),
    Seq(Box<EnvironmentTypeProjectionNode>),
    Option(Box<EnvironmentTypeProjectionNode>),
    Result {
        ok: Box<EnvironmentTypeProjectionNode>,
        error: Box<EnvironmentTypeProjectionNode>,
    },
    Tuple(Box<[EnvironmentTypeProjectionNode]>),
    AcceptedNominal {
        id: AcceptedNominalId,
        arguments: Box<[EnvironmentTypeProjectionNode]>,
    },
    TypeParameter {
        index: ArcweftRustTypeParameterIndex,
    },
}
```

Callable record inputs mirror the checked publication record but retain unresolved type nodes:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallablePublicationRecordInput {
    item: EnvironmentPublicationItemId,
    kind: EnvironmentCallableKind,
    key: EnvironmentCallableLookupInput,
    overload: CallableOverloadIndex,
    schema: EnvironmentCallableSignatureInput,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    rust: Option<RustCallableProvenance>,
    declaration_order: EnvironmentDeclarationOrdinal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentCallableLookupInput {
    Free(ProjectCallablePath),
    Method {
        receiver: EnvironmentTypeProjectionNode,
        method: CallableName,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallableSignatureInput {
    groups: Box<[EnvironmentParameterGroupInput]>,
    result: EnvironmentTypeProjectionNode,
    effects: EffectRow,
    call_policy: CallableCallPolicy,
    validator: CallableValidator,
}
```

Parameter/group fields retain the existing checked scalar types and presence/passing enums. Every parameter type is `EnvironmentTypeProjectionNode`.

`SourceBackedAdapterRegistrationFacts::into_parts()` returns:

```rust
pub struct SourceBackedAdapterRegistrationParts {
    pub document: Arc<SourceDocument>,
    pub externals: Box<[ExternalRegistrationFact]>,
    pub environment: SourceBackedEnvironmentRegistrationInput,
}
```

The sema crate never names `AdapterManifest` or depends on adapter-context.

## 6. Projection and final publication

New file: `crates/arcweft-lang-sema/src/callable/projection.rs`

```rust
impl AcceptedNominalWorld {
    pub fn try_project_environment_publication(
        &self,
        input: &BoundEnvironmentRegistrationInput,
        nominal_limits: NominalResolutionLimits,
        aggregation_limits: NominalAggregationLimits,
        callable_limits: &CallableLimits,
    ) -> Result<
        EnvironmentCallablePublication,
        EnvironmentPublicationProjectionReport,
    >;
}
```

`BoundEnvironmentRegistrationInput` is crate-owned registration state after `ProjectRegistrationFacts` binds world/revision/source snapshots. It is not constructible by adapter crates.

The final publication gains stamp and digest:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallablePublication {
    owner: EnvironmentCallableOwner,
    nominal_world: AcceptedNominalWorldStamp,
    records: Box<[EnvironmentCallablePublicationRecord]>,
    digest: EnvironmentCallablePublicationDigest,
}

impl EnvironmentCallablePublication {
    pub const fn owner(&self) -> &EnvironmentCallableOwner;
    pub const fn nominal_world(&self) -> &AcceptedNominalWorldStamp;
    pub fn records(&self) -> &[EnvironmentCallablePublicationRecord];
    pub const fn digest(&self) -> EnvironmentCallablePublicationDigest;

    pub(crate) fn try_new_projected(
        owner: EnvironmentCallableOwner,
        nominal_world: AcceptedNominalWorldStamp,
        records: Vec<EnvironmentCallablePublicationRecord>,
        limits: &CallableLimits,
    ) -> Result<Self, CallablePublicationError>;
}
```

The old public `try_new` without a stamp is removed.

Builder construction becomes world-bound:

```rust
impl RegisteredCallableCatalogBuilder {
    pub fn for_nominal_world(
        world: &AcceptedNominalWorld,
        limits: CallableLimits,
    ) -> Self;

    pub fn add_environment(
        &mut self,
        publication: EnvironmentCallablePublication,
    ) -> Result<(), CallableBuildReport>;
}
```

`add_environment` first compares the full stamp. A mismatch is not downgraded to a catalog miss.

## 7. Rust metadata input and final catalog

New file: `crates/arcweft-lang-sema/src/env/rust_metadata.rs`

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTypeMetadataPublicationInput {
    item: EnvironmentPublicationItemId,
    id: AcceptedNominalId,
    package: RustPackageId,
    rust_item: RustItemPath,
    parameters: Box<[RustTypeParameterPublicationInput]>,
    kind: RustTypeMetadataPublicationKind,
    source: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustTypeParameterPublicationInput {
    index: ArcweftRustTypeParameterIndex,
    name: String,
    source: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustTypeMetadataPublicationKind {
    Struct {
        shape: RustStructMetadataInput,
    },
    Enum {
        variants: Box<[RustVariantMetadataInput]>,
    },
    Newtype {
        inner: EnvironmentTypeProjectionNode,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustStructMetadataInput {
    Unit,
    Tuple(Box<[EnvironmentTypeProjectionNode]>),
    Record(BTreeMap<String, EnvironmentTypeProjectionNode>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustVariantMetadataInput {
    name: String,
    payload: RustVariantPayloadInput,
    source: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustVariantPayloadInput {
    Unit,
    Tuple(Box<[EnvironmentTypeProjectionNode]>),
    Record(BTreeMap<String, EnvironmentTypeProjectionNode>),
}
```

Final immutable forms:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedRustTypeMetadataCatalog {
    by_id: BTreeMap<AcceptedNominalId, AcceptedRustTypeMetadata>,
    digest: AcceptedRustTypeMetadataDigest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedRustTypeMetadata {
    id: AcceptedNominalId,
    package: RustPackageId,
    rust_item: RustItemPath,
    parameters: Box<[GenericTypeParameterId]>,
    kind: AcceptedRustTypeMetadataKind,
    source: SourceSpan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptedRustTypeMetadataKind {
    Struct {
        shape: AcceptedRustStructShape,
    },
    Enum {
        variants: BTreeMap<String, EnumVariantPayload>,
    },
    Newtype {
        inner: TypeKind,
    },
}
```

Required APIs:

```rust
impl AcceptedNominalWorld {
    pub fn try_project_rust_metadata(
        &self,
        inputs: &[RustTypeMetadataPublicationInput],
        nominal_limits: NominalResolutionLimits,
        aggregation_limits: NominalAggregationLimits,
    ) -> Result<
        AcceptedRustTypeMetadataCatalog,
        EnvironmentPublicationProjectionReport,
    >;
}

impl AcceptedRustTypeMetadataCatalog {
    pub fn get(
        &self,
        id: &AcceptedNominalId,
    ) -> Option<&AcceptedRustTypeMetadata>;

    pub fn instantiate(
        &self,
        nominal: &AcceptedNominalType,
    ) -> Result<InstantiatedRustTypeMetadata, RustMetadataInstantiationError>;

    pub const fn digest(&self) -> AcceptedRustTypeMetadataDigest;
}
```

`TypeParameter` is accepted only while projecting metadata with the exact declaration binder. It becomes `TypeKind::GenericParam(GenericTypeParameterId)` owned by that declaration ID. In callable projection it returns `FreeTypeParameterInCallable`.

`RegisteredTypeCheckEnv` gains:

```rust
rust_metadata: Arc<AcceptedRustTypeMetadataCatalog>,
environment_digest: RegisteredEnvironmentDigest,
```

and accessors. Rust-specific variant/payload queries use this catalog. String-based `RustPackageExports` is deleted.

## 8. Stable digest APIs

Add inherent behavior to the owning types:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticTypeDigest([u8; 32]);

impl TypeKind {
    pub fn semantic_identity_digest(&self) -> SemanticTypeDigest;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableSignatureSchemaDigest([u8; 32]);

impl CallableSignatureSchema {
    pub fn semantic_digest(&self) -> CallableSignatureSchemaDigest;
}

impl RegisteredCallableCatalog {
    pub fn digest(&self) -> RegisteredCallableCatalogDigest;
}

impl RegisteredTypeCheckEnv {
    pub const fn environment_digest(&self) -> RegisteredEnvironmentDigest;
}
```

The canonical encoding is specified in `SCHEMA-TOOLING-PERSISTENCE.md`. `std::hash::Hash`, `Debug`, `Display`, source labels, pointer identity, and map iteration order are not digest encodings.

## 9. Deleted or narrowed public API

Delete:

```rust
AdapterTypeKind::to_sema_type_kind()
ArcweftRustTypeRef::Named
AdapterTypeKind::Named
AdapterManifest::try_callable_publication(...)
AdapterManifest::try_rust_callable_publication(...)
EnvironmentCallablePublication::try_new(...) // unstamped form
CharacterRegistrationRequest::with_callable_publication(...)
ProjectCompilationContext::new(..., callable_publications)
RustPackageExports { types: HashSet<String> }
```

`AdapterManifest::try_callable_publication` is replaced by source-backed fact creation. There is no public manifest-to-final-publication shortcut.

Keep, but route through the new boundary:

- `SourceBackedAdapterRegistrationFacts`;
- `AcceptedNominalWorld`;
- `AcceptedNominalCatalog`;
- `AcceptedNominalOwnerId`;
- `RustPackageId`;
- `TypePath`;
- `CallableSignatureSchema`;
- existing callable limits and query budget;
- existing registered-world transaction.

## 10. Serialization derives

All public data carriers in `arcweft-rust-abi` and adapter file carrier types derive both `Serialize` and `Deserialize`. Sema-only semantic identities remain non-serialized unless an existing persistent carrier explicitly owns them. This preserves Sans I/O ownership and avoids making sema implementation types a wire protocol.
