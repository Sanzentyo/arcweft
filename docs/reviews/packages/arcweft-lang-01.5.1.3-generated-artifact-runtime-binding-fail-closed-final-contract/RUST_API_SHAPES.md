# Normative Rust API shapes

These shapes close result-changing API decisions. Private helper names and import grouping may change mechanically, but owners, fields, variants, validation points, dependency direction, and public behavior must not change without a new accepted contract. All public serializable key/product types implement both `Serialize` and `Deserialize`; catalog/binding instances do not implement serialization.

## 1. `arcweft-id`

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GeneratedArtifactBindingId(u32);

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum GeneratedArtifactBindingIdError {
    #[error("generated artifact binding requirement count exceeds u32")]
    Overflow,
}

impl GeneratedArtifactBindingId {
    pub fn try_from_index(index: usize) -> Result<Self, GeneratedArtifactBindingIdError>;
    pub const fn get(self) -> u32;
}
```

The field remains private. Callers convert `get()` to `usize` with checked conversion at vector boundaries. The type is product-local; no parser from a callable string is provided.

## 2. `arcweft-adapter-metadata` owner behavior

Extend the existing exact-marker macro and `AdapterTarget` implementation rather than creating an extension trait:

```rust
impl RustAbi {
    pub const fn as_str(self) -> &'static str;
}
impl WasmAbi {
    pub const fn as_str(self) -> &'static str;
}
impl ProcessAbi {
    pub const fn as_str(self) -> &'static str;
}
impl ProcessTransport {
    pub const fn as_str(self) -> &'static str;
}

impl AdapterTarget {
    pub const fn family(&self) -> AdapterFamily;
    pub const fn abi_str(&self) -> &'static str;
}
```

`family()` matches the owner enum. `abi_str()` delegates to its exact ABI marker. Family-specific detail remains available from the existing target variant.

## 3. Exact key types in `arcweft-runtime-binding`

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GeneratedArtifactAbi(Box<str>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GeneratedArtifactTransport(Box<str>);

impl GeneratedArtifactAbi {
    pub fn try_new(value: impl Into<Box<str>>) -> Result<Self, GeneratedArtifactIdentityError>;
    pub fn as_str(&self) -> &str;
}
impl GeneratedArtifactTransport {
    pub fn try_new(value: impl Into<Box<str>>) -> Result<Self, GeneratedArtifactIdentityError>;
    pub fn as_str(&self) -> &str;
}
```

The constructor accepts a non-empty bounded visible ASCII identifier, rejects whitespace/control characters and non-canonical spellings, and preserves exact case-sensitive identity. The production constants satisfy the same validator.

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedArtifactTopologyIdentity {
    profile: ProfileId,
    source_set_revision: SourceSetRevision,
}

impl GeneratedArtifactTopologyIdentity {
    pub const fn new(profile: ProfileId, source_set_revision: SourceSetRevision) -> Self;
    pub const fn profile(&self) -> &ProfileId;
    pub const fn source_set_revision(&self) -> SourceSetRevision;
}
```

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedArtifactImportIdentity {
    id: ExternalModuleImportId,
    mount: ModuleMountPath,
    metadata_path: NormalizedProjectPath,
    metadata_raw_hash: RawDigest,
    visibility: ManifestVisibility,
    demand: DependencyDemand,
}

impl GeneratedArtifactImportIdentity {
    pub fn new(
        id: ExternalModuleImportId,
        mount: ModuleMountPath,
        metadata_path: NormalizedProjectPath,
        metadata_raw_hash: RawDigest,
        visibility: ManifestVisibility,
        demand: DependencyDemand,
    ) -> Self;
    // one getter per field
}
```

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedArtifactMetadataIdentity {
    document: SourceDocumentIdentity,
    abi_hash: SemanticDigest,
    payload_hash: SemanticDigest,
}

impl GeneratedArtifactMetadataIdentity {
    pub const fn new(
        document: SourceDocumentIdentity,
        abi_hash: SemanticDigest,
        payload_hash: SemanticDigest,
    ) -> Self;
    // getters
}
```

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum GeneratedArtifactTargetDetail {
    Rust { target_triple: TargetTriple },
    Wasm { world: WitWorldId },
    Process { transport: GeneratedArtifactTransport },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedArtifactTargetIdentity {
    family: AdapterFamily,
    abi: GeneratedArtifactAbi,
    detail: GeneratedArtifactTargetDetail,
}

impl GeneratedArtifactTargetIdentity {
    pub fn try_new(
        family: AdapterFamily,
        abi: GeneratedArtifactAbi,
        detail: GeneratedArtifactTargetDetail,
    ) -> Result<Self, GeneratedArtifactTargetIdentityError>;

    pub fn try_from_accepted(
        target: &AdapterTarget,
    ) -> Result<Self, GeneratedArtifactIdentityError>;

    pub const fn family(&self) -> AdapterFamily;
    pub const fn abi(&self) -> &GeneratedArtifactAbi;
    pub const fn detail(&self) -> &GeneratedArtifactTargetDetail;

    pub(crate) fn validate_accepted_markers(
        &self,
    ) -> Result<(), GeneratedArtifactTargetIdentityError>;
}
```

`try_new` is the host-claim constructor: it requires family/detail agreement and checked ABI/transport syntax, but it may represent a wrong yet well-formed claim for precise mismatch reporting. `try_from_accepted` is the only loader projection constructor and uses `AdapterTarget`'s inherent `family()` / `abi_str()` behavior. Manual `Deserialize` enforces family/detail agreement. Product construction and product decode additionally call `validate_accepted_markers()`, which requires the exact current owner tuple: Rust + `RustAbi`, WASM + `WasmAbi`, or process + `ProcessAbi` + `ProcessTransport`. No extension trait is introduced.

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum GeneratedArtifactExportIdentity {
    Function {
        export: AdapterFunctionExport,
    },
    Activity {
        abstract_activity: ActivityId,
        implementation: ActivityImplementationId,
        export: AdapterActivityExport,
    },
}

impl GeneratedArtifactExportIdentity {
    pub fn try_activity(
        abstract_activity: ActivityId,
        implementation: ActivityImplementationId,
        export: AdapterActivityExport,
    ) -> Result<Self, GeneratedArtifactExportIdentityError>;
    pub const fn kind(&self) -> GeneratedArtifactBindingKind;
}
```

`try_activity` requires `abstract_activity == export.activity_id`. `ActivityImplementationId` is not inferred from module/export later. `Deserialize` is manual and routes Activity values through `try_activity`, so public deserialization cannot bypass the invariant.

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedArtifactBindingKey {
    topology: GeneratedArtifactTopologyIdentity,
    import: GeneratedArtifactImportIdentity,
    metadata: GeneratedArtifactMetadataIdentity,
    target: GeneratedArtifactTargetIdentity,
    package: AdapterPackage,
    module: AdapterModule,
    artifact: AdapterArtifact,
    export: GeneratedArtifactExportIdentity,
}

impl GeneratedArtifactBindingKey {
    pub fn try_new(
        topology: GeneratedArtifactTopologyIdentity,
        import: GeneratedArtifactImportIdentity,
        metadata: GeneratedArtifactMetadataIdentity,
        target: GeneratedArtifactTargetIdentity,
        package: AdapterPackage,
        module: AdapterModule,
        artifact: AdapterArtifact,
        export: GeneratedArtifactExportIdentity,
    ) -> Result<Self, GeneratedArtifactBindingProductError>;

    pub const fn topology(&self) -> &GeneratedArtifactTopologyIdentity;
    pub const fn import(&self) -> &GeneratedArtifactImportIdentity;
    pub const fn metadata(&self) -> &GeneratedArtifactMetadataIdentity;
    pub const fn target(&self) -> &GeneratedArtifactTargetIdentity;
    pub const fn package(&self) -> &AdapterPackage;
    pub const fn module(&self) -> &AdapterModule;
    pub const fn artifact(&self) -> &AdapterArtifact;
    pub const fn export(&self) -> &GeneratedArtifactExportIdentity;

    pub fn correlate(
        &self,
        claimed: &Self,
    ) -> Result<(), GeneratedArtifactBindingCorrelationError>;
}
```

`try_new` validates target/export internal invariants. Projection additionally asserts the already accepted import expectations equal package/module/family/metadata ABI hash; that check remains in project-loader admission and is not duplicated as a second wire authority.

## 4. Product shapes

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeneratedArtifactBindingKind {
    Function,
    Activity,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedArtifactBindingRequirement {
    id: GeneratedArtifactBindingId,
    key: GeneratedArtifactBindingKey,
}

impl GeneratedArtifactBindingRequirement {
    pub const fn id(&self) -> GeneratedArtifactBindingId;
    pub const fn key(&self) -> &GeneratedArtifactBindingKey;
    pub const fn kind(&self) -> GeneratedArtifactBindingKind;
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedArtifactActivitySelection {
    activity: ActivityId,
    implementation: ActivityImplementationId,
    binding: GeneratedArtifactBindingId,
}

impl GeneratedArtifactActivitySelection {
    pub const fn activity(&self) -> &ActivityId;
    pub const fn implementation(&self) -> &ActivityImplementationId;
    pub const fn binding(&self) -> GeneratedArtifactBindingId;
}
```

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneratedArtifactBindingLaunchProduct {
    topology: GeneratedArtifactTopologyIdentity,
    requirements: Box<[GeneratedArtifactBindingRequirement]>,
    activity_selections: Box<[GeneratedArtifactActivitySelection]>,
}

impl GeneratedArtifactBindingLaunchProduct {
    pub fn try_from_keys(
        topology: GeneratedArtifactTopologyIdentity,
        keys: impl IntoIterator<Item = GeneratedArtifactBindingKey>,
    ) -> Result<Self, GeneratedArtifactBindingProductError>;

    pub const fn topology(&self) -> &GeneratedArtifactTopologyIdentity;
    pub fn requirements(&self) -> &[GeneratedArtifactBindingRequirement];
    pub fn requirement(
        &self,
        id: GeneratedArtifactBindingId,
    ) -> Option<&GeneratedArtifactBindingRequirement>;
    pub fn activity_selections(&self) -> &[GeneratedArtifactActivitySelection];
    pub fn activity_selection(
        &self,
        activity: &ActivityId,
    ) -> Option<&GeneratedArtifactActivitySelection>;
    pub fn verify(&self) -> Result<(), GeneratedArtifactBindingProductError>;
}
```

`try_from_keys` validates that every target has exact currently accepted ABI/transport markers, sorts requirements, assigns IDs, and derives `activity_selections` from Activity requirements. Selections are canonical by typed `ActivityId`; each points to exactly one Activity requirement whose abstract Activity and `ActivityImplementationId` match, and every Activity requirement has exactly one selection. Duplicate Activity selections are product errors.

There is deliberately no `empty(topology)` constructor for a no-profile compile. A selected profile with no generated requirements uses `try_from_keys(real_topology, empty_iterator)` and produces an empty **selected** product. No accepted launch profile is represented by `None` at the compiler owner.

Manual `Serialize`/`Deserialize` supplies the exact format/schema envelope and sends decode through strict validation. Internal construction may sort keys and derive selections; decoding must require already canonical requirements and selections and must not repair either.

## 5. Adapter origin

In `arcweft-adapter-context`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterFunctionOrigin {
    HostAdapter,
    GeneratedArtifact(GeneratedArtifactBindingId),
}

pub struct AdapterFunction {
    path: AdapterCallablePath,
    overload: AdapterCallableOverloadIndex,
    signature: AdapterFunctionSignature,
    effects: Vec<AdapterEffectCapability>,
    origin: AdapterFunctionOrigin,
}

impl AdapterFunction {
    pub const fn origin(&self) -> AdapterFunctionOrigin;
}

impl AdapterManifest {
    // Existing method remains the host-adapter constructor and sets HostAdapter.
    pub fn with_function_signature(/* current args */) -> Self;

    pub fn with_generated_function_signature(
        self,
        path: AdapterCallablePath,
        overload: AdapterCallableOverloadIndex,
        signature: AdapterFunctionSignature,
        effects: impl IntoIterator<Item = AdapterEffectCapability>,
        binding: GeneratedArtifactBindingId,
    ) -> Self;
}
```

Both constructors use one private insertion path owned by `AdapterManifest`; no external helper or side table.

## 6. Runtime identity variants

In `arcweft-core::value`:

```rust
pub enum RuntimeCallTarget {
    Intrinsic(RuntimeIntrinsic),
    Named(String),
    GeneratedArtifact(GeneratedArtifactBindingId),
}

impl RuntimeCallTarget {
    pub const fn generated_artifact(id: GeneratedArtifactBindingId) -> Self;
    pub const fn generated_artifact_id(&self) -> Option<GeneratedArtifactBindingId>;
    pub fn named_label(&self) -> Option<&str>;
}

pub enum RuntimeFunctionBody {
    Expr(Box<RuntimeExpr>),
    Awbc(AwbcFunctionId),
    GeneratedArtifact(GeneratedArtifactBindingId),
}

impl RuntimeFunctionValue {
    pub fn new_generated_artifact(
        params: Vec<String>,
        binding: GeneratedArtifactBindingId,
        captures: Vec<RuntimeBinding>,
    ) -> Self;
}
```

`Display for RuntimeCallTarget` matches variants directly. `as_label() -> &str` is deleted because a generated target has no authoritative string label.

In the existing `RuntimeTypedLoweringEvidenceKind`, replace string-only evidence for generated callables with ID-bearing variants rather than adding a parallel map. The final minimum shape is:

```rust
pub enum RuntimeTypedCallableOrigin {
    Named(String),
    GeneratedArtifact(GeneratedArtifactBindingId),
}

pub enum RuntimeTypedLoweringEvidenceKind {
    // existing variants retained as applicable
    FunctionValueCall {
        callee: Option<RuntimeTypedCallableOrigin>,
        arg_count: usize,
        partial: bool,
    },
    FunctionValueReference {
        callee: RuntimeTypedCallableOrigin,
    },
    SignaturePartialCall {
        callee: RuntimeTypedCallableOrigin,
        arg_count: usize,
    },
    FunctionEffectCallable {
        callable: RuntimeTypedCallableOrigin,
    },
    // existing non-callable evidence variants
}
```

The owner enum is the single behavior point; do not add separate optional binding fields beside string fields.

## 7. Unified loader product

In `arcweft-project-loader`:

```rust
pub struct SelectedExternalModuleProjection {
    adapter: AdapterManifest,
    bindings: Arc<GeneratedArtifactBindingLaunchProduct>,
}

// `bindings.activity_selections()` is the exact selected Activity implementation
// projection; no second ActivityId-to-binding side map is authoritative.

pub(super) fn project_selected_external_modules(
    adapter: AdapterManifest,
    profile_id: &ProfileId,
    profile: &ResolvedLaunchProfile,
    source_set_revision: SourceSetRevision,
    modules: &[LoadedExternalModuleMetadata],
) -> Result<SelectedExternalModuleProjection, ProfileTopologyLoadError>;
```

`LoadedProfileTopology` gains:

```rust
generated_artifact_bindings: Arc<GeneratedArtifactBindingLaunchProduct>
```

and a getter with the same immutable `Arc` type.

## 8. Compiler carriers and verifier

`AcceptedLaunchProfileInput` gains one mandatory field and constructor argument:

```rust
pub struct AcceptedLaunchProfileInput {
    // existing manifest/profile/revision/resource fields remain
    generated_artifact_bindings: Arc<GeneratedArtifactBindingLaunchProduct>,
}

impl AcceptedLaunchProfileInput {
    pub const fn generated_artifact_bindings(
        &self,
    ) -> &Arc<GeneratedArtifactBindingLaunchProduct>;
}
```

The existing `ProjectCompilationContext::accepted_launch_profile: Option<AcceptedLaunchProfileInput>` remains the only optional boundary.

The current `CompiledProject` owner gains:

```rust
pub struct CompiledProject {
    // existing fields remain
    generated_artifact_bindings: Option<Arc<GeneratedArtifactBindingLaunchProduct>>,
}

impl CompiledProject {
    pub const fn generated_artifact_bindings(
        &self,
    ) -> Option<&Arc<GeneratedArtifactBindingLaunchProduct>>;
}
```

Its constructor copies `context.accepted_launch_profile().map(|input| Arc::clone(input.generated_artifact_bindings()))` exactly. It must not synthesize a `ProfileId`, topology, or empty product for `None`.

The compiler invokes a named verifier after `RuntimePlan::verify()`:

```rust
pub fn verify_generated_artifact_bindings(
    plan: &RuntimePlan,
    product: Option<&GeneratedArtifactBindingLaunchProduct>,
) -> Result<(), GeneratedArtifactBindingPlanError>;
```

Rules:

- `None` plus no generated function IDs and no generated Activity launch selections is valid;
- `None` plus any generated ID/selection is product-invalid;
- `Some(product)` validates the product, every function ID, and every Activity selection;
- a selected profile with zero requirements remains `Some(product)`, not `None`.

This is a legitimate cross-boundary verifier owned by the compiler/runtime-plan integration, not an extension trait or callable-name helper. Any compiled-project/bundle codec that carries this field must round-trip `None` and `Some` distinctly and must not treat a missing legacy field as `None` through a compatibility default.

## 9. Catalog

```rust
pub struct GeneratedArtifactBindingCatalogBuilder<F, A> {
    product: Arc<GeneratedArtifactBindingLaunchProduct>,
    slots: Vec<GeneratedArtifactBindingSlot<F, A>>,
}

pub struct GeneratedArtifactBindingCatalog<F, A> {
    product: Arc<GeneratedArtifactBindingLaunchProduct>,
    slots: Box<[GeneratedArtifactBindingSlot<F, A>]>,
}

enum GeneratedArtifactBindingSlot<F, A> {
    Function(Option<F>),
    Activity(Option<A>),
}

impl<F, A> GeneratedArtifactBindingCatalogBuilder<F, A> {
    pub fn new(product: Arc<GeneratedArtifactBindingLaunchProduct>) -> Self;

    pub fn register_function(
        &mut self,
        id: GeneratedArtifactBindingId,
        claimed_key: GeneratedArtifactBindingKey,
        binding: F,
    ) -> Result<(), GeneratedArtifactBindingRegistrationError>;

    pub fn register_activity(
        &mut self,
        id: GeneratedArtifactBindingId,
        claimed_key: GeneratedArtifactBindingKey,
        binding: A,
    ) -> Result<(), GeneratedArtifactBindingRegistrationError>;

    pub fn freeze(self) -> GeneratedArtifactBindingCatalog<F, A>;
}

impl<F, A> GeneratedArtifactBindingCatalog<F, A> {
    pub const fn product(&self) -> &Arc<GeneratedArtifactBindingLaunchProduct>;

    pub fn resolve_function(
        &self,
        active: &GeneratedArtifactTopologyIdentity,
        id: GeneratedArtifactBindingId,
    ) -> Result<&F, GeneratedArtifactBindingResolveError>;

    pub fn resolve_activity(
        &self,
        active: &GeneratedArtifactTopologyIdentity,
        id: GeneratedArtifactBindingId,
    ) -> Result<&A, GeneratedArtifactBindingResolveError>;
}
```

No trait bounds are placed on `F` or `A` by the shared catalog. No `register_by_*` or `resolve_by_*` overload exists for a spelling, path, digest, profile, Activity ID, mount, or adapter ID.

## 10. Error shapes

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneratedArtifactRuntimeBindingCode {
    Missing,
    Stale,
    Mismatch,
    Unselected,
    KindMismatch,
    Duplicate,
    ProductInvalid,
}

impl GeneratedArtifactRuntimeBindingCode {
    pub const fn as_str(self) -> &'static str;
}
```

The three public error families are:

```rust
pub enum GeneratedArtifactBindingProductError { /* strict schema, accepted-marker, Activity-selection, and plan/product invariant cases */ }
pub enum GeneratedArtifactBindingRegistrationError {
    Stale(GeneratedArtifactBindingStale),
    Unselected { id: GeneratedArtifactBindingId },
    KindMismatch { id: GeneratedArtifactBindingId, expected: GeneratedArtifactBindingKind, actual: GeneratedArtifactBindingKind },
    Mismatch { id: GeneratedArtifactBindingId, source: GeneratedArtifactBindingKeyMismatch },
    Duplicate { id: GeneratedArtifactBindingId },
}
pub enum GeneratedArtifactBindingResolveError {
    Stale(GeneratedArtifactBindingStale),
    Unselected { id: GeneratedArtifactBindingId },
    KindMismatch { id: GeneratedArtifactBindingId, expected: GeneratedArtifactBindingKind, actual: GeneratedArtifactBindingKind },
    Missing { requirement: GeneratedArtifactBindingRequirement },
}
```

Each has an inherent `code()` method. The complete typed mismatch variants and precedence are in `ERRORS_AND_LIFETIME.md`.
