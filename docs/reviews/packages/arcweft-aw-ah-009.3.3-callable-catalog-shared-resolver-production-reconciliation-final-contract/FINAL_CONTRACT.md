# Final contract

## 1. Scope, ownership, and dependency direction

This file is normative. It reconciles AW-AH-009.3 with production at
`9fd6ee8fb2814ff04dc7a3e4ef413b86b7f4ac4d` without selecting the call-range
carrier owned by AW-AH-009.3.1 or the accepted-HIR lease owned by
AW-AH-009.3.2.

The implementation has one ownership direction:

```text
arcweft-lang-hir
    publishes typed project declaration/signature source records
        |
        v
arcweft-lang-sema
    owns callable IDs, schemas, catalogs, resolver, argument mapping,
    target facts, public semantic signature results, limits, and errors
        ^
        |
arcweft-adapter-context
    normalizes accepted manifests and typed Rust metadata into sema-owned
    publication records; sema never depends on adapter-context
        |
        v
arcweft-lang-sema::registration
    validates and atomically publishes one RegisteredCallableCatalog inside
    RegisteredTypeCheckEnv
        |
        v
checker and signature query
    invoke the same resolver and the same argument checker exactly once
```

`arcweft-lang-sema` adds `callable.rs` with children
`callable/{identity,schema,catalog,publication,resolver,arguments,facts,presentation,dialogue,limits,error}.rs`.
It does not add `callable/mod.rs`. Public exports are deliberate re-exports from
`arcweft-lang-sema::callable`; catalog builders and checker adapters remain
`pub(crate)`.

Existing imported types used below are `CanonicalModulePath`, `CallablePackageId`,
`SymbolPath`, `CallableDeclarationId`, `ProjectSymbolWorldId`,
`ProjectSymbolRevision`, `HirProject`, `FnSignature`, `DocBlock`, `CharacterId`,
`CharacterPartId`, `CharacterManifest`, `CharacterNominalType`, `TypeKind`,
`TypeCheckEnv`, `EffectRow`, `CallableId`, `SourceSpan`,
`SourceDocumentIdentity`, `Expr`, `TypeExpressionId`, `TypeCheckReport`,
`TypeChecker`, `ProjectSymbolTable`, `RegisteredSemanticWorld`, `TraitCatalog`,
`SemanticScopeId`, `CharacterInventoryDescriptorV1`,
`CharacterInventoryDigest`, `CharacterInventoryRevision`, and the current
registration-private `ExternalOwnerRegistry`. This contract does not redefine
any of them; private existing fields appear only when fixing the exact target
shape of their owning registration type.

No new dependency is added from HIR to sema, from sema to adapter-context, from
core/runtime crates to syntax/HIR/sema, or from an adapter crate to LSP.

## 2. Validated scalar identities

All fields are private. These declarations and methods are exact.

```rust
use std::{num::NonZeroU32, sync::Arc};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableName(Arc<str>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterPackageId(Arc<str>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RustItemPath(Arc<str>);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableGroupIndex(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableParameterIndex(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableOverloadIndex(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableArgumentIndex(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableArgumentSlotIndex(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LexicalBindingIndex(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionValueOrdinal(u32);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CallableScalarError {
    Empty { kind: CallableScalarKind },
    Control { kind: CallableScalarKind, byte: usize },
    ContainsSeparator {
        kind: CallableScalarKind,
        byte: usize,
        separator: char,
    },
    IndexOverflow {
        kind: CallableIndexKind,
        value: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableScalarKind {
    CallableName,
    AdapterPackageId,
    RustItemPath,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CallableIndexKind {
    Group,
    Parameter,
    Overload,
    Argument,
    ArgumentSlot,
    LexicalBinding,
    FunctionValue,
}
```

`CallableName::try_new` rejects empty strings, control characters, `.`, `:`,
`/`, `\\`, `(`, `)`, `[`, `]`, `{`, and `}`. A path is never reconstructed by
splitting a display string. `AdapterPackageId::try_new` rejects empty strings,
control characters, whitespace, `/`, `\\`, `:`, and `@`; it otherwise retains
the exact accepted manifest `id`. `RustItemPath::try_new` rejects empty strings
and control characters but intentionally permits Rust `::`, generic punctuation,
and spaces because it is provenance only, never a lookup key.

```rust
impl CallableName {
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, CallableScalarError>;
    pub fn as_str(&self) -> &str;
}

impl AdapterPackageId {
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, CallableScalarError>;
    pub fn as_str(&self) -> &str;
}

impl RustItemPath {
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, CallableScalarError>;
    pub fn as_str(&self) -> &str;
}

macro_rules! index_api {
    ($ty:ident, $kind:expr) => {
        impl $ty {
            pub fn try_from_usize(value: usize) -> Result<Self, CallableScalarError>;
            pub const fn get(self) -> usize;
        }
    };
}
```

The implementation writes the seven inherent `impl` blocks directly; the macro
above is specification shorthand only and is not added to production. Each
constructor uses `u16::try_from` or `u32::try_from`; there is no truncating cast
and no raw public constructor.

## 3. Typed paths and lookup keys

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallablePath {
    segments: Arc<[CallableName]>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ReceiverMethodKey {
    receiver: TypeKind,
    method: CallableName,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CallableLookupKey {
    Free(CallablePath),
    Method(ReceiverMethodKey),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectCallablePath {
    package: CallablePackageId,
    module: CanonicalModulePath,
    path: CallablePath,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum ProjectNameBinding {
    Callable(CallableDeclarationId),
    NonCallable {
        path: ProjectCallablePath,
        ty: TypeKind,
    },
}
```

```rust
impl CallablePath {
    pub fn try_new(
        segments: impl IntoIterator<Item = CallableName>,
    ) -> Result<Self, CallablePathError>;
    pub fn segments(&self) -> &[CallableName];
    pub fn leaf(&self) -> &CallableName;
    pub fn len(&self) -> usize;
}

impl ReceiverMethodKey {
    pub fn new(receiver: TypeKind, method: CallableName) -> Self;
    pub const fn receiver(&self) -> &TypeKind;
    pub const fn method(&self) -> &CallableName;
}

impl ProjectCallablePath {
    pub fn new(
        package: CallablePackageId,
        module: CanonicalModulePath,
        path: CallablePath,
    ) -> Self;
    pub const fn package(&self) -> &CallablePackageId;
    pub const fn module(&self) -> &CanonicalModulePath;
    pub const fn path(&self) -> &CallablePath;
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CallablePathError {
    Empty,
    TooManySegments { actual: usize, limit: usize },
}
```

A method key uses `TypeKind` equality and hashing. `TypeKind` is not made
`Ord`. Deterministic method publication order is established by the typed
record order key in section 10, never by formatting `TypeKind`.

`ProjectNameBinding` is produced by `ProjectSymbolTable` during registration.
It is the sole authority for project non-callable shadowing; the resolver does
not infer shadowing from a failed callable lookup.

## 4. Package, provider, authority, and declaration identity

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StandardEnvironmentId {
    Core,
    SansIo,
    NativeHttp,
    InferenceTensor,
    SystemInfo,
    NativeFile,
    Math,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentCallableOwner {
    Standard(StandardEnvironmentId),
    Adapter(AdapterPackageId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentCallableKind {
    Function,
    Method,
    UntypedMethodFallback,
    RustFunction,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EnvironmentCallableId {
    owner: EnvironmentCallableOwner,
    kind: EnvironmentCallableKind,
    key: CallableLookupKey,
    overload: CallableOverloadIndex,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableAuthorityRank {
    Project,
    Standard,
    Adapter,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CallableProviderId {
    Project(CallablePackageId),
    Standard(StandardEnvironmentId),
    Adapter(AdapterPackageId),
}
```

```rust
impl EnvironmentCallableId {
    pub fn new(
        owner: EnvironmentCallableOwner,
        kind: EnvironmentCallableKind,
        key: CallableLookupKey,
        overload: CallableOverloadIndex,
    ) -> Self;
    pub const fn owner(&self) -> &EnvironmentCallableOwner;
    pub const fn kind(&self) -> EnvironmentCallableKind;
    pub const fn key(&self) -> &CallableLookupKey;
    pub const fn overload(&self) -> CallableOverloadIndex;
}

impl EnvironmentCallableOwner {
    pub const fn authority(&self) -> CallableAuthorityRank;
    pub fn provider(&self) -> CallableProviderId;
}
```

`CallableAuthorityRank` is a semantic precedence, not an enum-discriminant sort:
`Project` shadows environment records, and between equally viable environment
candidates `Standard` wins over `Adapter`. Project candidates are selected by
project/lexical name resolution before environment candidates rather than being
mixed into one environment overload tie.

The exact accepted adapter manifest field forming `AdapterPackageId` is
`AdapterManifest::id` and no other field. Display name, filesystem path, Rust
package, Rust item path, documentation, and selected profile name do not
participate. A selected manifest whose `id` is one of the six standard manifest
IDs is owned by the matching `StandardEnvironmentId`; another selected adapter
may not claim a standard ID. `Core` owns language-provided environment records
that are not supplied by a manifest.

## 5. Documentation, Rust provenance, and source evidence

Documentation and source evidence are typed payloads and are never lookup keys.
Rust provenance retains both the accepted adapter owner and the complete typed
`ArcweftRustPackage` identity (`name`, `version`, and optional
`metadata_hash`); no Rust package field is reconstructed from `rust_path`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDocumentation {
    summary: Option<Arc<str>>,
    details: Option<Arc<str>>,
    parameters: Arc<[CallableParameterDocumentation]>,
    provenance: DocumentationProvenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameterDocumentation {
    group: CallableGroupIndex,
    parameter: CallableParameterIndex,
    text: Arc<str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentationProvenance {
    Missing,
    ProjectSource { declaration: CallableDeclarationId },
    AdapterTooling { package: AdapterPackageId },
    RustMetadata {
        adapter: AdapterPackageId,
        package: RustPackageProvenance,
        item: RustItemPath,
    },
    Language { family: LanguageDocumentationFamily },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanguageDocumentationFamily {
    Builtin,
    Fx,
    Agent,
    Presentation,
    Dialogue,
    Collection,
    Domain,
    Integer,
    Capacity,
    Trait,
    Constructor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustPackageProvenance {
    name: Arc<str>,
    version: Arc<str>,
    metadata_hash: Option<Arc<str>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustCallablePurity {
    External,
    Pure,
    Task,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustCallableProvenance {
    adapter: AdapterPackageId,
    package: RustPackageProvenance,
    rust_path: RustItemPath,
    purity: RustCallablePurity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSource {
    declaration: Option<CallableDeclarationId>,
    signature: Option<SourceSpan>,
    name: Option<SourceSpan>,
    result: Option<SourceSpan>,
    parameters: Arc<[CallableParameterSource]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameterSource {
    group: CallableGroupIndex,
    parameter: CallableParameterIndex,
    whole: SourceSpan,
    name: Option<SourceSpan>,
    ty: Option<SourceSpan>,
    default: Option<SourceSpan>,
}
```

```rust
impl CallableDocumentation {
    pub fn try_new(
        summary: Option<Arc<str>>,
        details: Option<Arc<str>>,
        parameters: Vec<CallableParameterDocumentation>,
        provenance: DocumentationProvenance,
    ) -> Result<Self, CallableDocumentationError>;
    pub fn missing() -> Self;
    pub fn summary(&self) -> Option<&str>;
    pub fn details(&self) -> Option<&str>;
    pub fn parameters(&self) -> &[CallableParameterDocumentation];
    pub const fn provenance(&self) -> &DocumentationProvenance;
    pub fn parameter(
        &self,
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    ) -> Option<&str>;
}

impl CallableParameterDocumentation {
    pub fn try_new(
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
        text: impl Into<Arc<str>>,
    ) -> Result<Self, CallableDocumentationError>;
    pub const fn group(&self) -> CallableGroupIndex;
    pub const fn parameter(&self) -> CallableParameterIndex;
    pub fn text(&self) -> &str;
}

impl CallableParameterSource {
    pub fn try_new(
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
        whole: SourceSpan,
        name: Option<SourceSpan>,
        ty: Option<SourceSpan>,
        default: Option<SourceSpan>,
    ) -> Result<Self, CallableSourceError>;
    pub const fn group(&self) -> CallableGroupIndex;
    pub const fn parameter(&self) -> CallableParameterIndex;
    pub const fn whole(&self) -> &SourceSpan;
    pub const fn name(&self) -> Option<&SourceSpan>;
    pub const fn ty(&self) -> Option<&SourceSpan>;
    pub const fn default(&self) -> Option<&SourceSpan>;
}

impl RustPackageProvenance {
    pub fn try_new(
        name: impl Into<Arc<str>>,
        version: impl Into<Arc<str>>,
        metadata_hash: Option<Arc<str>>,
    ) -> Result<Self, RustProvenanceError>;
    pub fn name(&self) -> &str;
    pub fn version(&self) -> &str;
    pub fn metadata_hash(&self) -> Option<&str>;
}

impl RustCallableProvenance {
    pub fn try_new(
        adapter: AdapterPackageId,
        package: RustPackageProvenance,
        rust_path: RustItemPath,
        purity: RustCallablePurity,
    ) -> Result<Self, RustProvenanceError>;
    pub const fn adapter(&self) -> &AdapterPackageId;
    pub const fn package(&self) -> &RustPackageProvenance;
    pub const fn rust_path(&self) -> &RustItemPath;
    pub const fn purity(&self) -> RustCallablePurity;
}

impl CallableSource {
    pub fn try_new(
        declaration: Option<CallableDeclarationId>,
        signature: Option<SourceSpan>,
        name: Option<SourceSpan>,
        result: Option<SourceSpan>,
        parameters: Vec<CallableParameterSource>,
    ) -> Result<Self, CallableSourceError>;
    pub const fn declaration(&self) -> Option<&CallableDeclarationId>;
    pub const fn signature(&self) -> Option<&SourceSpan>;
    pub const fn name(&self) -> Option<&SourceSpan>;
    pub const fn result(&self) -> Option<&SourceSpan>;
    pub fn parameters(&self) -> &[CallableParameterSource];
    pub fn parameter(
        &self,
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    ) -> Option<&CallableParameterSource>;
}
```

Constructors reject duplicate parameter coordinates, coordinates outside the
schema supplied to final record construction, spans from a different
`SourceDocumentIdentity` than the declaration signature, and a child span not
contained by the signature span. Documentation may be absent. Missing
documentation is represented only by `DocumentationProvenance::Missing`, not by
an empty fabricated string.

When a standard record and an adapter record are exact semantic duplicates,
the standard record remains primary. The adapter documentation and Rust
provenance are retained in the candidate's `equivalent_sources`; they are not
silently discarded or merged into one string.

## 6. Signature origin

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureOrigin {
    Project {
        declaration: CallableDeclarationId,
        path: ProjectCallablePath,
    },
    Standard {
        owner: StandardEnvironmentId,
        id: EnvironmentCallableId,
    },
    Adapter {
        package: AdapterPackageId,
        id: EnvironmentCallableId,
    },
    Language {
        family: LanguageCallableFamily,
    },
    Trait {
        id: TraitCallableId,
    },
    Lexical {
        id: LocalCallableId,
    },
    FunctionValue {
        id: FunctionValueSignatureId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LanguageCallableFamily {
    Fx,
    EnumConstructor,
    ResultConstructor,
    OptionConstructor,
    Builtin,
    Agent,
    Presentation,
    Dialogue,
    CollectionMethod,
    PresentationHandleMethod,
    IntegerMethod,
    DomainMethod,
    CapacityMethod,
    DataLast,
    Drop,
    Promote,
    Assume,
    Speaker,
}
```

`SignatureOrigin` is presentation/provenance data. Candidate identity is always
`CallableCandidateId` from sections 7 and 8. No consumer compares formatted
origins.

## 7. Free-call candidate identities

All formerly opaque IDs are fixed here. Public constructors validate owned
fields; enum constructors and accessors are inherent on these Arcweft-owned
types.

### 7.1 Builtins

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BuiltinCallableId {
    InlineFailureFallback,
    Panic,
    Fail,
    Bail,
    Ensure,
    Assert,
    DebugAssert,
    Rgb,
    Sin,
    Cos,
    Vector { dimensions: VectorDimensions },
    Math(MathCallableId),
    StdFloat(StdFloatCallableId),
    Capability(CapabilityCallableId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VectorDimensions {
    Two,
    Three,
    Four,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MathCallableId {
    MatMulF32,
    MatrixAddF32,
    MatMulF64,
    MatrixAddF64,
    TensorAddF32,
    TensorAddF64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FloatWidth {
    F32,
    F64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StdFloatOperation {
    Abs,
    Floor,
    Ceil,
    Round,
    Trunc,
    Fract,
    Sqrt,
    Sin,
    Cos,
    Tan,
    Exp,
    Exp2,
    Ln,
    Log2,
    Log10,
    Powf,
    Atan2,
    MulAdd,
    IsNan,
    IsInfinite,
    IsFinite,
    IsSignPositive,
    IsSignNegative,
    ToBits,
    FromBits,
    ToF32,
    ToF64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StdFloatCallableId {
    width: FloatWidth,
    operation: StdFloatOperation,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CapabilityCallableId {
    EventEmit,
}
```

```rust
impl StdFloatCallableId {
    pub fn try_new(
        width: FloatWidth,
        operation: StdFloatOperation,
    ) -> Result<Self, BuiltinIdentityError>;
    pub const fn width(self) -> FloatWidth;
    pub const fn operation(self) -> StdFloatOperation;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuiltinIdentityError {
    UnsupportedConversion {
        width: FloatWidth,
        operation: StdFloatOperation,
    },
}
```

`ToF64` is valid only for `F32`; `ToF32` is valid only for `F64`. All other
operation/width pairs follow the current production inventory. `resolve` and
`signature_schema` are inherent methods on `BuiltinCallableId`:

```rust
impl BuiltinCallableId {
    pub fn resolve(path: &CallablePath) -> Option<Self>;
    pub fn signature_schema(&self) -> CallableSignatureSchema;
}
```

`resolve` compares typed segments; it does not split a display label.

### 7.2 Enum, Result, and Option constructors

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProjectNominalTypeId {
    package: CallablePackageId,
    module: CanonicalModulePath,
    name: CallableName,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct EnumVariantSignatureId {
    owner: ProjectNominalTypeId,
    variant: CallableName,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ResultConstructorKind {
    Ok,
    Err,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OptionConstructorKind {
    Some,
}
```

```rust
impl ProjectNominalTypeId {
    pub fn new(
        package: CallablePackageId,
        module: CanonicalModulePath,
        name: CallableName,
    ) -> Self;
    pub const fn package(&self) -> &CallablePackageId;
    pub const fn module(&self) -> &CanonicalModulePath;
    pub const fn name(&self) -> &CallableName;
}

impl EnumVariantSignatureId {
    pub fn new(owner: ProjectNominalTypeId, variant: CallableName) -> Self;
    pub const fn owner(&self) -> &ProjectNominalTypeId;
    pub const fn variant(&self) -> &CallableName;
}
```

The ID never embeds the query's expected type. Expected-type disambiguation is
recorded in `ResolvedCallable::instantiation`. `Ok`, `Err`, and `Some` use their
separate closed candidate variants and are retained in
`CallableInstantiation::Result` or `Option`; they are not represented as
project enum IDs.

### 7.3 FX

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FxCallableSignatureId {
    Style,
    Text,
    Color,
    Transform,
    Mask,
    Filter,
    Shader,
    Transition,
    Conditional,
    Stack,
}
```

```rust
impl FxCallableSignatureId {
    pub fn resolve(path: &CallablePath) -> FxResolution;
    pub fn signature_schema(self) -> CallableSignatureSchema;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FxResolution {
    NotFx,
    Known(FxCallableSignatureId),
    UnknownMember { member: CallableName },
    InvalidNestedPath { path: CallablePath },
}
```

An unknown direct `Fx.<member>` remains a resolved poisoned FX family so the
checker emits the existing unknown-constructor diagnostic, checks every
argument for recovery, and returns `Fx`. It must not fall through to project or
environment resolution.

### 7.4 Agent intrinsics

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AgentIntrinsicSignatureId {
    Expect,
    Deny,
    Checkpoint,
    Note,
    Attach,
    ChoiceAction,
    Viewport,
    Layer,
    Object,
    Capture,
    ReadResource,
    EntityMeta,
    ProjectNeighbors,
    Signal,
    Metric,
    StatePath,
    ObservationPath,
    State,
    Observation,
    Diagnostics,
    Exists,
    ActionEnabled,
    All,
    Any,
    Not,
    Wait,
    AdvanceText,
    ViewportPoint,
    PointerClick,
    Invoke,
    RagQuery,
}
```

```rust
impl AgentIntrinsicSignatureId {
    pub fn resolve(path: &CallablePath) -> Option<Self>;
    pub fn signature_schema(self) -> CallableSignatureSchema;
}
```

The two-segment identities are `pointer.click` and `rag.query`. They are stored
as two typed path segments, not as a dotted `CallableName`.

### 7.5 Presentation calls

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationCallableId {
    View,
    Menu,
    Overlay,
    Background,
    Image,
    PlayerViewport,
    Show,
    RefBackground,
    RefShow,
    ClearBackground,
    Hide,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationSchemaContext<'a> {
    pub owner: Option<&'a ResolvedCharacterOwner>,
    pub environment: &'a RegisteredTypeCheckEnv,
}

impl PresentationCallableId {
    pub fn resolve(path: &CallablePath) -> Option<Self>;
    pub fn signature_schema(
        self,
        context: PresentationSchemaContext<'_>,
    ) -> Result<CallableSignatureSchema, CallableSchemaError>;
}
```

Typed paths are:

| ID | Typed path |
|---|---|
| `View` | `view` |
| `Menu` | `menu` |
| `Overlay` | `overlay` |
| `Background` | `bg` |
| `Image` | `image` |
| `PlayerViewport` | `player_viewport` |
| `Show` | `show` |
| `RefBackground` | `ref.bg` |
| `RefShow` | `ref.show` |
| `ClearBackground` | `clear.bg` |
| `Hide` | `hide` |

### 7.6 Dialogue calls

Dialogue call identity is separate from presentation identity because syntax,
argument storage, result type, and owner acquisition differ.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueCallableId {
    SpeakerLine,
    ContentCall,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogueCalleeIdentity {
    Speaker {
        character: CharacterId,
    },
    SpeakerPreset {
        character: CharacterId,
    },
    Content {
        path: CallablePath,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DialogueSchemaContext<'a> {
    pub callee: &'a DialogueCalleeIdentity,
    pub environment: &'a RegisteredTypeCheckEnv,
}

impl DialogueCallableId {
    pub fn resolve(callee: &DialogueCalleeIdentity) -> Self;
    pub fn signature_schema(
        self,
        context: DialogueSchemaContext<'_>,
    ) -> Result<CallableSignatureSchema, CallableSchemaError>;
}
```

`SpeakerLine` owns colon-style speaker lines and the `speaker[...]` shorthand.
`ContentCall` owns canonical `speaker.say(...)[...]` and other accepted content
call heads. The parser/HIR supplies `DialogueCalleeIdentity`; sema does not
parse the authored callee string.

### 7.7 Project, environment, lexical, and function values

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LocalCallableId {
    scope: SemanticScopeId,
    binding: LexicalBindingIndex,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FunctionValueSignatureId {
    expression: TypeExpressionId,
    ordinal: FunctionValueOrdinal,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CurriedCallableId {
    base: Box<CallableCandidateId>,
    next_group: CallableGroupIndex,
}

impl LocalCallableId {
    pub(crate) fn new(scope: SemanticScopeId, binding: LexicalBindingIndex) -> Self;
    pub const fn scope(&self) -> &SemanticScopeId;
    pub const fn binding(&self) -> LexicalBindingIndex;
}

impl FunctionValueSignatureId {
    pub(crate) fn new(
        expression: TypeExpressionId,
        ordinal: FunctionValueOrdinal,
    ) -> Self;
    pub const fn expression(&self) -> TypeExpressionId;
    pub const fn ordinal(&self) -> FunctionValueOrdinal;
}

impl CurriedCallableId {
    pub fn try_new(
        base: CallableCandidateId,
        next_group: CallableGroupIndex,
    ) -> Result<Self, CallableIdentityError>;
    pub const fn base(&self) -> &CallableCandidateId;
    pub const fn next_group(&self) -> CallableGroupIndex;
}
```

`FunctionValueSignatureId` is allocated in deterministic expression traversal
order inside one checker invocation. It is never persisted, serialized, or used
as an accepted-world cache key. `LocalCallableId` is valid only in the lexical
scope snapshot borrowed by the resolver. `CurriedCallableId` is rejected if
`next_group` is not a group of the base schema.

## 8. Selected-call and remaining candidate identities

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CollectionMethodId {
    Len,
    Map,
    Filter,
    Sum,
    Contains,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationHandleMethodId {
    Show,
    Hide,
    Unmount,
    Release,
    Destroy,
    OverlayPop,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntegerMethodId {
    Clamp,
    Min,
    Max,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProbeComparisonId {
    Eq,
    Ne,
    NotEq,
    Gt,
    Greater,
    Ge,
    GreaterOrEqual,
    Lt,
    Less,
    Le,
    LessOrEqual,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum DomainMethodId {
    Traverse,
    Parallel,
    FxSampleOrdinalPhase,
    ObservedObjectRequireRole,
    MapGet { key: TypeKind, value: TypeKind },
    ProbeCompare { value: TypeKind, operation: ProbeComparisonId },
    DiagnosticsHasError,
    RagContextPackSummary,
    Context,
    WithContext,
    CharacterFace { character: Option<CharacterId> },
    CharacterSay { character: Option<CharacterId> },
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CapacityMethodId {
    receiver: TypeKind,
    method: CallableName,
    arity: u16,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraitCallableId {
    trait_name: CallablePath,
    method: CallableName,
    implementation: TraitImplementationIndex,
    source: TraitCallableSource,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraitImplementationIndex(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TraitCallableSource {
    Inherent,
    Predicate,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DataLastCallableId {
    callable: Box<CallableCandidateId>,
    receiver_parameter: CallableParameterIndex,
    receiver_group: CallableGroupIndex,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DropCallableId {
    Drop,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PromotionCallableId {
    Promote,
    PromoteUnchecked,
    Assume,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SpeakerCallableId {
    character: Option<CharacterId>,
    preset: bool,
}
```

```rust
impl CapacityMethodId {
    pub fn try_new(
        receiver: TypeKind,
        method: CallableName,
        arity: usize,
    ) -> Result<Self, CallableIdentityError>;
    pub const fn receiver(&self) -> &TypeKind;
    pub const fn method(&self) -> &CallableName;
    pub const fn arity(&self) -> usize;
}

impl TraitImplementationIndex {
    pub(crate) fn try_from_usize(value: usize) -> Result<Self, CallableScalarError>;
    pub const fn get(self) -> usize;
}

impl TraitCallableId {
    pub fn new(
        trait_name: CallablePath,
        method: CallableName,
        implementation: TraitImplementationIndex,
        source: TraitCallableSource,
    ) -> Self;
    pub const fn trait_name(&self) -> &CallablePath;
    pub const fn method(&self) -> &CallableName;
    pub const fn implementation(&self) -> TraitImplementationIndex;
    pub const fn source(&self) -> TraitCallableSource;
}

impl DataLastCallableId {
    pub fn try_new(
        callable: CallableCandidateId,
        receiver_group: CallableGroupIndex,
        receiver_parameter: CallableParameterIndex,
        schema: &CallableSignatureSchema,
    ) -> Result<Self, CallableIdentityError>;
    pub const fn callable(&self) -> &CallableCandidateId;
    pub const fn receiver_group(&self) -> CallableGroupIndex;
    pub const fn receiver_parameter(&self) -> CallableParameterIndex;
}

impl SpeakerCallableId {
    pub fn new(character: Option<CharacterId>, preset: bool) -> Self;
    pub const fn character(&self) -> Option<&CharacterId>;
    pub const fn is_preset(&self) -> bool;
}
```

The data-last constructor requires the selected parameter to be the final
parameter of its group or the sole parameter of the next curried group, exactly
matching production's two accepted fallback shapes. It rejects rest parameters
and rejects a base candidate outside `Project`, `Environment`, or `Local`.
`CurriedCallableId::try_new` likewise requires a non-`Curried`, non-`DataLast`
base candidate and stores the original base plus the next group, preventing
recursive wrapper identity growth.

## 9. Complete candidate ID

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CallableCandidateId {
    Fx(FxCallableSignatureId),
    EnumVariant(EnumVariantSignatureId),
    Result(ResultConstructorKind),
    Option(OptionConstructorKind),
    Builtin(BuiltinCallableId),
    Agent(AgentIntrinsicSignatureId),
    Presentation(PresentationCallableId),
    Dialogue(DialogueCallableId),
    Project(CallableDeclarationId),
    Environment(EnvironmentCallableId),
    Local(LocalCallableId),
    FunctionValue(FunctionValueSignatureId),
    Curried(CurriedCallableId),
    CollectionMethod(CollectionMethodId),
    PresentationHandleMethod(PresentationHandleMethodId),
    IntegerMethod(IntegerMethodId),
    DomainMethod(DomainMethodId),
    TraitMethod(TraitCallableId),
    DataLast(DataLastCallableId),
    CapacityMethod(CapacityMethodId),
    Drop(DropCallableId),
    Promotion(PromotionCallableId),
    Speaker(SpeakerCallableId),
}
```

Every successful call or selected call has exactly one primary
`CallableCandidateId`. Exact standard/adapter duplicates additionally expose
ordered equivalent IDs; they do not create a second successful resolution.

The implementation does not define an extension trait for this enum. Family,
origin, and schema behavior are added in its inherent implementation:

```rust
impl CallableCandidateId {
    pub const fn family(&self) -> CallableFamily;
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableFamily {
    Fx,
    EnumConstructor,
    ResultConstructor,
    OptionConstructor,
    Builtin,
    Agent,
    Presentation,
    Dialogue,
    Project,
    Environment,
    Lexical,
    FunctionValue,
    CollectionMethod,
    PresentationHandleMethod,
    IntegerMethod,
    DomainMethod,
    TraitMethod,
    DataLast,
    CapacityMethod,
    Drop,
    Promotion,
    Speaker,
}
```

`CallableCandidateId::Result` and `Option` report their distinct constructor
families. `Curried` reports the base candidate's family; it never creates a
second semantic family. Every other variant maps one-to-one to the enum above.

## 10. Complete callable schema model

A catalog record stores a fully instantiated schema. Language families build
the same schema type at resolution time. Checker and signature help consume no
parallel signature representation.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableSignatureSchema {
    groups: Arc<[CallableParameterGroup]>,
    result: TypeKind,
    effects: CallableEffectSchema,
    argument_policy: CallableArgumentPolicy,
    validator: CallableValidator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableEffectSchema {
    Fixed(EffectRow),
    Project {
        declaration: CallableDeclarationId,
        declared: EffectRow,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameterGroup {
    index: CallableGroupIndex,
    kind: CallableGroupKind,
    parameters: Arc<[CallableParameter]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableGroupKind {
    Initial,
    Curried,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableParameter {
    index: CallableParameterIndex,
    name: Option<CallableName>,
    ty: CallableParameterType,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    documentation: Option<Arc<str>>,
    source: Option<CallableParameterSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableParameterType {
    Exact(TypeKind),
    Unchecked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableParameterPassing {
    PositionalOnly,
    PositionalOrNamed,
    NamedOnly,
    RestPositional,
    RestNamed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableParameterPresence {
    Required,
    Optional,
    Defaulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallableArgumentPolicy {
    unknown_named: UnknownNamedArgumentPolicy,
    spread: SpreadArgumentPolicy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownNamedArgumentPolicy {
    Reject,
    OpenChecked,
    OpenUnchecked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpreadArgumentPolicy {
    Reject,
    FixedLiteralOnly,
    TypedRest,
    Unchecked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableValidator {
    Ordinary,
    Untyped,
    Fx(FxCallableSignatureId),
    UnknownFxMember { member: CallableName },
    EnumConstructor(EnumVariantSignatureId),
    ResultConstructor(ResultConstructorKind),
    OptionConstructor(OptionConstructorKind),
    Builtin(BuiltinCallableId),
    Agent(AgentIntrinsicSignatureId),
    Presentation(PresentationCallableId),
    Dialogue(DialogueCallableId),
    Collection(CollectionMethodId),
    PresentationHandle(PresentationHandleMethodId),
    Integer(IntegerMethodId),
    Domain(DomainMethodId),
    Trait(TraitCallableId),
    DataLast(DataLastCallableId),
    Capacity(CapacityMethodId),
    Drop,
    Promotion(PromotionCallableId),
    Speaker,
}
```

```rust
impl CallableSignatureSchema {
    pub fn try_new(
        groups: Vec<CallableParameterGroup>,
        result: TypeKind,
        effects: CallableEffectSchema,
        argument_policy: CallableArgumentPolicy,
        validator: CallableValidator,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError>;
    pub fn groups(&self) -> &[CallableParameterGroup];
    pub const fn result(&self) -> &TypeKind;
    pub const fn effects(&self) -> &CallableEffectSchema;
    pub const fn argument_policy(&self) -> CallableArgumentPolicy;
    pub const fn validator(&self) -> &CallableValidator;
    pub fn group(&self, index: CallableGroupIndex) -> Option<&CallableParameterGroup>;
    pub fn total_parameters(&self) -> usize;
    pub fn semantic_eq(&self, other: &Self) -> bool;
}

impl CallableEffectSchema {
    pub fn fixed(row: EffectRow) -> Self;
    pub fn project(
        declaration: CallableDeclarationId,
        declared: EffectRow,
    ) -> Self;
    pub const fn declared(&self) -> &EffectRow;
    pub const fn project_declaration(&self) -> Option<&CallableDeclarationId>;
}

impl CallableParameterGroup {
    pub fn try_new(
        index: CallableGroupIndex,
        kind: CallableGroupKind,
        parameters: Vec<CallableParameter>,
        limits: &CallableLimits,
    ) -> Result<Self, CallableSchemaError>;
    pub const fn index(&self) -> CallableGroupIndex;
    pub const fn kind(&self) -> CallableGroupKind;
    pub fn parameters(&self) -> &[CallableParameter];
    pub fn parameter(&self, index: CallableParameterIndex) -> Option<&CallableParameter>;
}

impl CallableParameter {
    pub fn try_new(
        index: CallableParameterIndex,
        name: Option<CallableName>,
        ty: CallableParameterType,
        passing: CallableParameterPassing,
        presence: CallableParameterPresence,
        documentation: Option<Arc<str>>,
        source: Option<CallableParameterSource>,
    ) -> Result<Self, CallableSchemaError>;
    pub const fn index(&self) -> CallableParameterIndex;
    pub fn name(&self) -> Option<&CallableName>;
    pub const fn ty(&self) -> &CallableParameterType;
    pub const fn passing(&self) -> CallableParameterPassing;
    pub const fn presence(&self) -> CallableParameterPresence;
    pub fn documentation(&self) -> Option<&str>;
    pub const fn source(&self) -> Option<&CallableParameterSource>;
}

impl CallableArgumentPolicy {
    pub const fn new(
        unknown_named: UnknownNamedArgumentPolicy,
        spread: SpreadArgumentPolicy,
    ) -> Self;
    pub const fn unknown_named(self) -> UnknownNamedArgumentPolicy;
    pub const fn spread(self) -> SpreadArgumentPolicy;
}
```

Schema invariants enforced by `try_new`:

1. at least one parameter group exists; a zero-argument callable has one empty
   initial group rather than zero groups;
2. group indices are exactly `0..groups.len()`;
3. group 0 is `Initial`; every later group is `Curried`;
4. parameter indices restart at zero and are contiguous within each group;
5. names are unique within a group;
6. at most one `RestPositional` and one `RestNamed` exist per group, and each is
   the final parameter of its passing class;
7. a rest parameter is never `Defaulted`;
8. a `NamedOnly` or `RestNamed` parameter has a name;
9. source coordinates equal the owning group/parameter coordinates;
10. total groups and parameters are within `CallableLimits`;
11. `Ordinary` validation may use `Exact` or `Unchecked`; family validators may
    impose stricter rules in their inherent schema constructors.

A default expression is not retained in the registered catalog. `Defaulted`
records semantic presence, while `CallableParameterSource::default` points to
the authored project default when one exists. Adapter default values stay in
the adapter-owned runtime metadata; sema stores only the typed default-presence
fact required for argument mapping and signature help.

`semantic_eq` compares groups, names, types, passing, presence, result, the
complete `CallableEffectSchema`, argument policy, and validator. It excludes parameter documentation and source
spans. It performs structural equality, not hash equality.

## 11. Immutable catalog record and order

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableRecord {
    id: CallableCandidateId,
    key: CallableLookupKey,
    authority: CallableAuthorityRank,
    provider: CallableProviderId,
    schema: Arc<CallableSignatureSchema>,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    rust: Option<RustCallableProvenance>,
    declaration_order: EnvironmentDeclarationOrdinal,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EnvironmentDeclarationOrdinal(u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EquivalentCallableSource {
    id: CallableCandidateId,
    origin: SignatureOrigin,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    rust: Option<RustCallableProvenance>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogCallableEntry {
    primary: Arc<CallableRecord>,
    equivalent_sources: Arc<[EquivalentCallableSource]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredCallableCatalog {
    project: ProjectCallableCatalog,
    environment: EnvironmentCallableCatalog,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCallableCatalog {
    modules: Arc<[RegisteredProjectModuleCallables]>,
    by_declaration: std::collections::HashMap<CallableDeclarationId, Arc<CallableRecord>>,
    bindings: std::collections::HashMap<ProjectCallablePath, ProjectNameBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisteredProjectModuleCallables {
    module: CanonicalModulePath,
    source: SourceDocumentIdentity,
    declarations: Arc<[CallableDeclarationId]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallableCatalog {
    free: std::collections::HashMap<CallablePath, NonEmptyCallableSet>,
    methods: std::collections::HashMap<ReceiverMethodKey, NonEmptyCallableSet>,
    by_id: std::collections::HashMap<EnvironmentCallableId, Arc<CallableRecord>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonEmptyCallableSet {
    entries: Arc<[CatalogCallableEntry]>,
}
```

```rust
impl EnvironmentDeclarationOrdinal {
    pub fn try_from_usize(value: usize) -> Result<Self, CallableScalarError>;
    pub const fn get(self) -> usize;
}

impl CallableRecord {
    pub fn try_new(
        id: CallableCandidateId,
        key: CallableLookupKey,
        authority: CallableAuthorityRank,
        provider: CallableProviderId,
        schema: Arc<CallableSignatureSchema>,
        documentation: CallableDocumentation,
        source: Option<CallableSource>,
        rust: Option<RustCallableProvenance>,
        declaration_order: EnvironmentDeclarationOrdinal,
    ) -> Result<Self, CallableCatalogError>;
    pub const fn id(&self) -> &CallableCandidateId;
    pub const fn key(&self) -> &CallableLookupKey;
    pub const fn authority(&self) -> CallableAuthorityRank;
    pub const fn provider(&self) -> &CallableProviderId;
    pub const fn schema(&self) -> &CallableSignatureSchema;
    pub const fn documentation(&self) -> &CallableDocumentation;
    pub const fn source(&self) -> Option<&CallableSource>;
    pub const fn rust(&self) -> Option<&RustCallableProvenance>;
    pub const fn declaration_order(&self) -> EnvironmentDeclarationOrdinal;
}

impl EquivalentCallableSource {
    pub fn new(
        id: CallableCandidateId,
        origin: SignatureOrigin,
        documentation: CallableDocumentation,
        source: Option<CallableSource>,
        rust: Option<RustCallableProvenance>,
    ) -> Self;
    pub const fn id(&self) -> &CallableCandidateId;
    pub const fn origin(&self) -> &SignatureOrigin;
    pub const fn documentation(&self) -> &CallableDocumentation;
    pub const fn source(&self) -> Option<&CallableSource>;
    pub const fn rust(&self) -> Option<&RustCallableProvenance>;
}

impl CatalogCallableEntry {
    pub(crate) fn try_new(
        primary: Arc<CallableRecord>,
        equivalent_sources: Vec<EquivalentCallableSource>,
        limits: &CallableLimits,
    ) -> Result<Self, CallableCatalogError>;
    pub const fn primary(&self) -> &Arc<CallableRecord>;
    pub fn equivalent_sources(&self) -> &[EquivalentCallableSource];
}

impl NonEmptyCallableSet {
    pub(crate) fn try_new(
        entries: Vec<CatalogCallableEntry>,
        limits: &CallableLimits,
    ) -> Result<Self, CallableCatalogError>;
    pub fn first(&self) -> &CatalogCallableEntry;
    pub fn as_slice(&self) -> &[CatalogCallableEntry];
    pub fn len(&self) -> NonZeroU32;
}

impl RegisteredProjectModuleCallables {
    pub(crate) fn new(
        module: CanonicalModulePath,
        source: SourceDocumentIdentity,
        declarations: Vec<CallableDeclarationId>,
    ) -> Self;
    pub const fn module(&self) -> &CanonicalModulePath;
    pub const fn source(&self) -> &SourceDocumentIdentity;
    pub fn declarations(&self) -> &[CallableDeclarationId];
}

impl ProjectCallableCatalog {
    pub fn modules(&self) -> &[RegisteredProjectModuleCallables];
    pub fn record(
        &self,
        id: &CallableDeclarationId,
    ) -> Option<&Arc<CallableRecord>>;
    pub fn binding(&self, key: &ProjectCallablePath) -> Option<&ProjectNameBinding>;
}

impl EnvironmentCallableCatalog {
    pub fn free(&self, path: &CallablePath) -> Option<&NonEmptyCallableSet>;
    pub fn method(&self, key: &ReceiverMethodKey) -> Option<&NonEmptyCallableSet>;
    pub fn record(
        &self,
        id: &EnvironmentCallableId,
    ) -> Option<&Arc<CallableRecord>>;
}

impl RegisteredCallableCatalog {
    pub const fn project(&self) -> &ProjectCallableCatalog;
    pub const fn environment(&self) -> &EnvironmentCallableCatalog;
    pub fn project_binding(&self, key: &ProjectCallablePath) -> Option<&ProjectNameBinding>;
    pub fn project_record(
        &self,
        id: &CallableDeclarationId,
    ) -> Option<&Arc<CallableRecord>>;
    pub fn free(&self, path: &CallablePath) -> Option<&NonEmptyCallableSet>;
    pub fn method(&self, key: &ReceiverMethodKey) -> Option<&NonEmptyCallableSet>;
    pub fn environment_record(
        &self,
        id: &EnvironmentCallableId,
    ) -> Option<&Arc<CallableRecord>>;
}
```

`CallableRecord::try_new` accepts only `Project` or `Environment` candidate IDs.
It requires the ID owner, authority, provider, kind, and lookup key to agree.
Project records require source declaration identity and forbid Rust provenance.
`RustFunction` environment records require Rust provenance. `Function`,
`Method`, and `UntypedMethodFallback` records may retain Rust provenance when
typed Rust metadata supplied it, but do not require it. A fallback record must
use a method lookup key and `CallableValidator::Untyped`; an ordinary `Method`
record may itself have unchecked parameters but remains in the earlier
method-signature phase.

The deterministic environment order key is the tuple:

```text
(authority: Standard before Adapter,
 provider: StandardEnvironmentId or AdapterPackageId,
 kind: Function before Method before UntypedMethodFallback before RustFunction,
 overload index,
 declaration ordinal,
 candidate ID's typed fields)
```

Project record order is declaration order from `HirProject` and is not mixed
with environment order. `NonEmptyCallableSet::try_new` sorts entries by each
primary record's environment order key, rejects project primaries, rejects
primary or equivalent duplicate IDs, enforces the overload limit, and asserts
that every primary has the same lookup key. `EnvironmentCallableCatalog::by_id`
retains every original standard and adapter record, including records coalesced
as equivalents, while lookup sets expose one `CatalogCallableEntry` per semantic
candidate.

A `HashMap` iteration order is never observable. Reversing publication input
must produce byte-for-byte equal ordered candidate ID slices.

## 12. Project callable publication

### 12.1 HIR-owned source record

`arcweft-lang-hir` adds this public immutable record because HIR owns the
source signature and declaration identity. It does not depend on sema types.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCallableSignatureSource {
    declaration: CallableDeclarationId,
    package: CallablePackageId,
    module: CanonicalModulePath,
    path: SymbolPath,
    signature: FnSignature,
    documentation: Option<DocBlock>,
    declaration_span: SourceSpan,
    name_span: SourceSpan,
    signature_span: SourceSpan,
    result_span: Option<SourceSpan>,
    parameter_spans: Arc<[HirCallableParameterSource]>,
    effects: HirCallableEffects,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCallableParameterSource {
    group: u16,
    parameter: u16,
    whole: SourceSpan,
    name: Option<SourceSpan>,
    ty: Option<SourceSpan>,
    default: Option<SourceSpan>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirEffectName(Arc<str>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCallableEffects {
    declared: Arc<[HirEffectName]>,
}

impl HirEffectName {
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, HirEffectNameError>;
    pub fn as_str(&self) -> &str;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirEffectNameError {
    Empty,
    Control { byte: usize },
}
```

The existing HIR types `FnSignature` and `DocBlock` remain authoritative; this
record references their typed values rather than serializing a signature label.
Construction is `pub(crate)` in HIR lowering. The exact read API is:

```rust
impl HirCallableSignatureSource {
    pub const fn declaration(&self) -> &CallableDeclarationId;
    pub const fn package(&self) -> &CallablePackageId;
    pub const fn module(&self) -> &CanonicalModulePath;
    pub const fn path(&self) -> &SymbolPath;
    pub const fn signature(&self) -> &FnSignature;
    pub const fn documentation(&self) -> Option<&DocBlock>;
    pub const fn declaration_span(&self) -> &SourceSpan;
    pub const fn name_span(&self) -> &SourceSpan;
    pub const fn signature_span(&self) -> &SourceSpan;
    pub const fn result_span(&self) -> Option<&SourceSpan>;
    pub fn parameter_spans(&self) -> &[HirCallableParameterSource];
    pub const fn effects(&self) -> &HirCallableEffects;
}

impl HirCallableParameterSource {
    pub const fn group(&self) -> u16;
    pub const fn parameter(&self) -> u16;
    pub const fn whole(&self) -> &SourceSpan;
    pub const fn name(&self) -> Option<&SourceSpan>;
    pub const fn ty(&self) -> Option<&SourceSpan>;
    pub const fn default(&self) -> Option<&SourceSpan>;
}

impl HirCallableEffects {
    pub fn declared(&self) -> &[HirEffectName];
}

impl HirProject {
    pub fn callable_signature_sources(
        &self,
    ) -> impl ExactSizeIterator<Item = &HirCallableSignatureSource>;

    pub fn module_callable_signature_sources(
        &self,
        module: &CanonicalModulePath,
    ) -> Option<&[HirCallableSignatureSource]>;
}
```

The iterator order is `HirProject::modules()` order followed by source
declaration order. Every module has an entry, including a zero-length slice.
`CallableDeclarationId` is reused directly; no sema-only project callable ID is
invented.

The HIR publication includes source functions and every other current
`CallableDeclarationId` owner whose production checker already treats it as
callable. It does **not** synthesize source `impl` methods. Current source
`impl` declarations remain available only to the existing trait catalog until
a separate canonical project-method catalog is proven.

### 12.2 Registration conversion

Sema converts each HIR source record by:

1. validating package/module/declaration agreement;
2. converting typed path segments to `CallablePath` without reading a display
   label;
3. converting every `FnSignature::param_groups()` group in order, preserving
   empty groups, names, positional/named/rest kinds, default presence, curried
   groups, parameter docs, and exact spans;
4. resolving parameter and result `TypeKind` through the same nominal/generic
   type conversion used by the checker;
5. converting declared effects to an `EffectRow` and storing
   `CallableEffectSchema::Project { declaration, declared }`; the accepted
   catalog does not allocate checker-local effect variables or claim a final
   inferred row;
6. retaining docs as `DocumentationProvenance::ProjectSource`;
7. constructing `CallableCandidateId::Project(declaration.clone())` and a
   `CallableRecord` with authority/provider `Project`;
8. adding the record only after all modules and records validate.

`ProjectSymbolTable` is traversed in the same transaction to create a complete
`ProjectNameBinding` map for callable and non-callable names. A project
non-callable binding is retained even when there is no callable record with the
same path. A module with no callables still publishes its module/source row and
all non-callable shadow bindings.

Any HIR/source identity mismatch, duplicate `CallableDeclarationId`, duplicate
project path in a resolution scope, invalid type conversion, source-span
mismatch, or limit failure rejects the entire registered-world candidate. No
partially built project catalog is observable.

## 13. Standard and adapter publication

### 13.1 Sema-owned publication input

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallablePublication {
    owner: EnvironmentCallableOwner,
    records: Vec<EnvironmentCallablePublicationRecord>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvironmentCallablePublicationRecord {
    kind: EnvironmentCallableKind,
    key: CallableLookupKey,
    overload: CallableOverloadIndex,
    schema: CallableSignatureSchema,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    rust: Option<RustCallableProvenance>,
    declaration_order: EnvironmentDeclarationOrdinal,
}
```

Every field has a same-named accessor. Constructors are public and validating:

```rust
impl EnvironmentCallablePublication {
    pub fn try_new(
        owner: EnvironmentCallableOwner,
        records: Vec<EnvironmentCallablePublicationRecord>,
        limits: &CallableLimits,
    ) -> Result<Self, CallablePublicationError>;
    pub const fn owner(&self) -> &EnvironmentCallableOwner;
    pub fn records(&self) -> &[EnvironmentCallablePublicationRecord];
}

impl EnvironmentCallablePublicationRecord {
    pub fn try_new(
        kind: EnvironmentCallableKind,
        key: CallableLookupKey,
        overload: CallableOverloadIndex,
        schema: CallableSignatureSchema,
        documentation: CallableDocumentation,
        source: Option<CallableSource>,
        rust: Option<RustCallableProvenance>,
        declaration_order: EnvironmentDeclarationOrdinal,
    ) -> Result<Self, CallablePublicationError>;
    pub const fn kind(&self) -> EnvironmentCallableKind;
    pub const fn key(&self) -> &CallableLookupKey;
    pub const fn overload(&self) -> CallableOverloadIndex;
    pub const fn schema(&self) -> &CallableSignatureSchema;
    pub const fn documentation(&self) -> &CallableDocumentation;
    pub const fn source(&self) -> Option<&CallableSource>;
    pub const fn rust(&self) -> Option<&RustCallableProvenance>;
    pub const fn declaration_order(&self) -> EnvironmentDeclarationOrdinal;
}
```

`EnvironmentCallablePublication::try_new` accepts zero records; empty
publications retain provider ownership and allow manifests that contribute only
non-callable symbols. Record, group, parameter, and work limits are still
checked before construction.

`arcweft-adapter-context` changes manifest normalization to produce this type.
It depends on sema; sema does not import `AdapterManifest` or Rust ABI types.
The old infallible callable mutation path into `TypeCheckEnv` is deleted after
all callers use publication records.

### 13.2 Adapter identity and metadata preservation

The current string fields are replaced directly by a typed adapter-side callable
model. This is not a compatibility layer. `arcweft-adapter-context` owns these
language-free types so manifests are typed before sema normalization:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterCallableName(String);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterCallablePath(Vec<AdapterCallableName>);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterCallableOverloadIndex(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterCallableGroupIndex(u16);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterCallableParameterIndex(u16);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFunctionSignature {
    groups: Vec<AdapterParameterGroup>,
    return_type: AdapterTypeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterParameterGroup {
    index: AdapterCallableGroupIndex,
    parameters: Vec<AdapterFunctionParam>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFunctionParam {
    index: AdapterCallableParameterIndex,
    name: Option<AdapterCallableName>,
    ty: AdapterTypeKind,
    passing: AdapterParameterPassing,
    presence: AdapterParameterPresence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterParameterPassing {
    PositionalOrNamed,
    PositionalOnly,
    NamedOnly,
    RestPositional,
    RestNamed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterParameterPresence {
    Required,
    Defaulted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterFreeCallableKind {
    Function,
    RustFunction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterToolingSubject {
    Free {
        kind: AdapterFreeCallableKind,
        path: AdapterCallablePath,
        overload: AdapterCallableOverloadIndex,
    },
    Method {
        receiver: AdapterTypeKind,
        name: AdapterCallableName,
        overload: AdapterCallableOverloadIndex,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterToolingParameterDoc {
    group: AdapterCallableGroupIndex,
    parameter: AdapterCallableParameterIndex,
    text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterToolingDoc {
    subject: AdapterToolingSubject,
    summary: Option<String>,
    details: Option<String>,
    parameters: Vec<AdapterToolingParameterDoc>,
}
```

The callable-bearing manifest records become the following exact target fields;
non-callable manifest fields are unchanged:

```rust
pub struct AdapterMethod {
    receiver: AdapterTypeKind,
    name: AdapterCallableName,
    overload: AdapterCallableOverloadIndex,
    signature: AdapterFunctionSignature,
    effects: Vec<AdapterEffectCapability>,
}

pub struct AdapterFunction {
    path: AdapterCallablePath,
    overload: AdapterCallableOverloadIndex,
    signature: AdapterFunctionSignature,
    effects: Vec<AdapterEffectCapability>,
}

pub struct AdapterRustFunction {
    package: ArcweftRustPackage,
    path: AdapterCallablePath,
    overload: AdapterCallableOverloadIndex,
    rust_path: String,
    signature: AdapterFunctionSignature,
    purity: ArcweftRustPurity,
    effects: Vec<AdapterEffectCapability>,
}
```

`AdapterCallableName::try_new` applies the same segment rules as sema
`CallableName`: non-empty, no controls, and no path or grouping separators.
`AdapterCallablePath::try_new` accepts a non-empty iterator of already typed
segments and never accepts a dotted display string. `AdapterCallablePath::single`
constructs the one-segment path used by `ArcweftRustFunction::name`; the Rust
`rust_path` field remains provenance only and is never split. All index
constructors are checked `usize -> u16` conversions.

The exact validating API is:

```rust
impl AdapterCallableName {
    pub fn try_new(value: impl Into<String>) -> Result<Self, AdapterCallableModelError>;
    pub fn as_str(&self) -> &str;
}

impl AdapterCallablePath {
    pub fn try_new(
        segments: impl IntoIterator<Item = AdapterCallableName>,
    ) -> Result<Self, AdapterCallableModelError>;
    pub fn single(segment: AdapterCallableName) -> Self;
    pub fn segments(&self) -> &[AdapterCallableName];
}

impl AdapterCallableOverloadIndex {
    pub fn try_from_usize(value: usize) -> Result<Self, AdapterCallableModelError>;
    pub const fn get(self) -> usize;
}

impl AdapterCallableGroupIndex {
    pub fn try_from_usize(value: usize) -> Result<Self, AdapterCallableModelError>;
    pub const fn get(self) -> usize;
}

impl AdapterCallableParameterIndex {
    pub fn try_from_usize(value: usize) -> Result<Self, AdapterCallableModelError>;
    pub const fn get(self) -> usize;
}

impl AdapterFunctionParam {
    pub fn try_new(
        index: AdapterCallableParameterIndex,
        name: Option<AdapterCallableName>,
        ty: AdapterTypeKind,
        passing: AdapterParameterPassing,
        presence: AdapterParameterPresence,
    ) -> Result<Self, AdapterCallableModelError>;
    pub const fn index(&self) -> AdapterCallableParameterIndex;
    pub const fn name(&self) -> Option<&AdapterCallableName>;
    pub const fn ty(&self) -> &AdapterTypeKind;
    pub const fn passing(&self) -> AdapterParameterPassing;
    pub const fn presence(&self) -> AdapterParameterPresence;
}

impl AdapterParameterGroup {
    pub fn try_new(
        index: AdapterCallableGroupIndex,
        parameters: Vec<AdapterFunctionParam>,
    ) -> Result<Self, AdapterCallableModelError>;
    pub const fn index(&self) -> AdapterCallableGroupIndex;
    pub fn parameters(&self) -> &[AdapterFunctionParam];
}

impl AdapterFunctionSignature {
    pub fn try_new(
        groups: Vec<AdapterParameterGroup>,
        return_type: AdapterTypeKind,
    ) -> Result<Self, AdapterCallableModelError>;
    pub fn groups(&self) -> &[AdapterParameterGroup];
    pub const fn return_type(&self) -> &AdapterTypeKind;
}

impl AdapterToolingParameterDoc {
    pub fn try_new(
        group: AdapterCallableGroupIndex,
        parameter: AdapterCallableParameterIndex,
        text: impl Into<String>,
    ) -> Result<Self, AdapterCallableModelError>;
    pub const fn group(&self) -> AdapterCallableGroupIndex;
    pub const fn parameter(&self) -> AdapterCallableParameterIndex;
    pub fn text(&self) -> &str;
}

impl AdapterToolingDoc {
    pub fn try_new(
        subject: AdapterToolingSubject,
        summary: Option<String>,
        details: Option<String>,
        parameters: Vec<AdapterToolingParameterDoc>,
    ) -> Result<Self, AdapterCallableModelError>;
    pub const fn subject(&self) -> &AdapterToolingSubject;
    pub fn summary(&self) -> Option<&str>;
    pub fn details(&self) -> Option<&str>;
    pub fn parameters(&self) -> &[AdapterToolingParameterDoc];
}
```

The model requires at least one group (group zero may be empty for a no-argument
callable) and rejects non-contiguous group/parameter indices, duplicate parameter
names in one group, a defaulted rest parameter, a nameless named/rest-named
parameter, more than one rest parameter of either class, or a rest parameter
that is not last in its passing class. A tooling parameter coordinate must exist
in the identified signature. Two tooling rows for the same typed subject are a
manifest-construction error rather than last-wins data.

The current manifest builders are replaced, not wrapped:

```rust
impl AdapterManifest {
    #[must_use]
    pub fn with_function_signature(
        self,
        path: AdapterCallablePath,
        overload: AdapterCallableOverloadIndex,
        signature: AdapterFunctionSignature,
        effects: impl IntoIterator<Item = AdapterEffectCapability>,
    ) -> Self;

    #[must_use]
    pub fn with_method_signature(
        self,
        receiver: AdapterTypeKind,
        name: AdapterCallableName,
        overload: AdapterCallableOverloadIndex,
        signature: AdapterFunctionSignature,
        effects: impl IntoIterator<Item = AdapterEffectCapability>,
    ) -> Self;

    #[must_use]
    pub fn with_tooling_doc(self, doc: AdapterToolingDoc) -> Self;
}
```

`with_rust_manifest` copies the exact `ArcweftRustPackage` (`name`, `version`,
`metadata_hash`), validates `ArcweftRustFunction::name` as one callable segment,
preserves `rust_path` only as provenance, converts every Rust parameter into the
single initial adapter group, and preserves `purity` plus every declared Rust
effect. It never parses `rust_path`, a rendered Rust signature, or prose.

`ResolvedLaunchProfile` and its deterministic profile-selection policy decide
which adapter manifest files are admitted before this boundary. Profile ID,
profile selection order, adapter-manifest path, and the optional launch
`adapter` spelling are not callable owner identity. Once decoded, only the
accepted manifest's own typed `AdapterManifest::id` forms `AdapterPackageId`.
The catalog canonicalizes publications by typed provider/key/order, so reversing
the admitted manifest iteration order cannot alter candidate order or selection.

For each accepted `AdapterManifest`:

- `AdapterManifest::id` is validated exactly once into `AdapterPackageId`;
- registration passes an explicit typed `AdapterManifestSource` proving whether
  the manifest came from a standard slot or the selected adapter set;
- a standard source requires the exact reserved ID for its
  `StandardEnvironmentId` and maps to `EnvironmentCallableOwner::Standard`;
- a selected-adapter source rejects every reserved standard ID and otherwise
  maps to `EnvironmentCallableOwner::Adapter`;
- typed path/name segments convert one-for-one into sema `CallablePath` and
  `CallableName` without splitting text;
- methods preserve structural `AdapterTypeKind` receiver identity;
- function, method, and Rust-function overload indices and declaration ordinals
  are retained exactly;
- typed Rust metadata supplies `RustCallableProvenance` using the accepted
  manifest ID, complete Rust package record, exact Rust item path, exported
  typed callable path, and purity; declared effects are preserved in the fixed schema row and no second
  metadata side input exists;
- typed tooling and parameter documentation map by `AdapterToolingSubject`; no
  prose is parsed to discover a candidate or parameter;
- effects convert to `CallableEffectSchema::Fixed(EffectRow)`;
- default/rest/named/curried behavior comes only from typed parameter groups.

The adapter-context publication API is:

```rust
#[cfg(feature = "sema")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterManifestSource {
    Standard(StandardEnvironmentId),
    SelectedAdapter,
}

#[cfg(feature = "sema")]
impl AdapterManifest {
    pub fn try_callable_publication(
        &self,
        source: AdapterManifestSource,
        limits: &CallableLimits,
    ) -> Result<EnvironmentCallablePublication, AdapterCallablePublicationError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterCallableModelError {
    EmptyName,
    Control { byte: usize },
    Separator { byte: usize, separator: char },
    EmptyPath,
    IndexOverflow { value: usize },
    EmptyGroups,
    NonContiguousGroup { expected: usize, actual: usize },
    NonContiguousParameter { group: usize, expected: usize, actual: usize },
    DuplicateParameterName { group: usize, name: String },
    MissingParameterName { group: usize, parameter: usize },
    DefaultedRest { group: usize, parameter: usize },
    DuplicateRest { group: usize, passing: AdapterParameterPassing },
    RestNotFinal { group: usize, parameter: usize },
    EmptyDocumentation,
    DuplicateToolingSubject { subject: AdapterToolingSubject },
    ToolingParameterOutOfBounds {
        subject: AdapterToolingSubject,
        group: usize,
        parameter: usize,
    },
}

#[cfg(feature = "sema")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterCallablePublicationError {
    InvalidPackageId(CallableScalarError),
    InvalidModel(AdapterCallableModelError),
    StandardIdMismatch {
        source: StandardEnvironmentId,
        actual: AdapterPackageId,
    },
    ReservedStandardIdClaimed {
        actual: AdapterPackageId,
    },
    DuplicateToolingSubject {
        subject: AdapterToolingSubject,
    },
    MissingToolingTarget {
        subject: AdapterToolingSubject,
    },
    InvalidReceiverType,
    InvalidSignature(CallablePublicationError),
    InvalidRustProvenance(RustProvenanceError),
    RustMetadataOwnerMismatch {
        package: AdapterPackageId,
    },
}
```

This is an inherent method on the Arcweft-owned `AdapterManifest`; no extension
trait or compatibility wrapper is introduced. The callable-writing portion of
`apply_to_env` is deleted when all callers use publication records. Non-callable
manifest symbols remain on their existing typed registration route.

### 13.3 Core standard publication

Language/runtime environment functions and methods currently installed by
`TypeCheckEnv` builders publish through an inherent method on that owning type:

```rust
impl TypeCheckEnv {
    pub(crate) fn standard_callable_publication(
        &self,
        limits: &CallableLimits,
    ) -> Result<EnvironmentCallablePublication, CallablePublicationError>;
}
```

The publication owner is `Standard(Core)`. A current `method_signature` row
becomes `Method`. A `method_type` row becomes `UntypedMethodFallback` only when
no method-signature row owns the same typed receiver/name; overlapping storage
is normalized to the earlier `Method` record rather than producing two
candidates. The existing enum/table owners add inherent schema constructors;
the implementation must not copy their matches into an adapter-only helper.

The six accepted standard adapter manifests are passed through the same adapter
normalizer with `AdapterManifestSource::Standard(id)`. Selected adapters use
`SelectedAdapter`. There is no post-normalization authority rewrite and no
opportunity for a selected adapter to acquire Standard rank by choosing a
reserved ID. A manifest with zero callable records produces a valid empty
publication so a non-callable-only adapter remains accepted.

## 14. Catalog builder, coalescing, and collision errors

```rust
pub(crate) struct RegisteredCallableCatalogBuilder {
    limits: CallableLimits,
    project_modules: Vec<RegisteredProjectModuleCallables>,
    project_records: Vec<Arc<CallableRecord>>,
    project_bindings: Vec<(ProjectCallablePath, ProjectNameBinding)>,
    environment_publications: Vec<EnvironmentCallablePublication>,
    work: CatalogBuildWork,
}

impl RegisteredCallableCatalogBuilder {
    pub(crate) fn new(limits: CallableLimits) -> Self;
    pub(crate) fn add_project(
        &mut self,
        project: &HirProject,
        symbols: &ProjectSymbolTable,
    ) -> Result<(), CallableCatalogBuildError>;
    pub(crate) fn add_environment(
        &mut self,
        publication: EnvironmentCallablePublication,
    ) -> Result<(), CallableCatalogBuildError>;
    pub(crate) fn finish(self) -> Result<RegisteredCallableCatalog, CallableCatalogBuildError>;
}
```

`finish` performs these deterministic phases:

1. validate complete module/source coverage;
2. index project bindings and project records;
3. reject duplicate typed project and environment IDs;
4. group environment records by `CallableLookupKey` and provider;
5. reject two distinct providers at the same authority rank for the same key,
   even when their signatures differ;
6. permit multiple overloads for one provider/key, identified by distinct
   contiguous overload indices;
7. compare cross-rank Standard/Adapter records structurally;
8. coalesce exact semantic duplicates into one primary standard candidate with
   ordered `EquivalentCallableSource` entries;
9. retain non-equal Standard/Adapter schemas as ordered overload candidates;
10. sort every set by the order key, freeze all vectors into `Arc` slices, and
    build maps;
11. return the immutable catalog only after all phases succeed.

A duplicate typed ID is never coalesced. Coalescing requires different typed
IDs, the same key, and `schema.semantic_eq`. Documentation and source differences
are retained as equivalents and do not prevent coalescing.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableCatalogBuildError {
    DuplicateTypedId {
        id: CallableCandidateId,
    },
    SameRankCollision {
        key: CallableLookupKey,
        rank: CallableAuthorityRank,
        first: CallableProviderId,
        second: CallableProviderId,
    },
    DuplicateProviderOverload {
        key: CallableLookupKey,
        provider: CallableProviderId,
        overload: CallableOverloadIndex,
    },
    NonContiguousOverloads {
        key: CallableLookupKey,
        provider: CallableProviderId,
        expected: CallableOverloadIndex,
        actual: CallableOverloadIndex,
    },
    ProjectBindingCollision {
        path: ProjectCallablePath,
        first: ProjectNameBinding,
        second: ProjectNameBinding,
    },
    MissingProjectModuleSource {
        module: CanonicalModulePath,
    },
    ProjectIdentityMismatch {
        declaration: CallableDeclarationId,
    },
    InvalidRecord(CallableCatalogError),
    InvalidPublication(CallablePublicationError),
    InvalidSchema(CallableSchemaError),
    Limit(CallableBuildLimitError),
    WorkOverflow,
}
```

Every variant has a stable diagnostic code through
`CallableCatalogBuildError::code() -> CallableDiagnosticCode`; formatted error
text is presentation only.

## 15. Atomic `RegisteredTypeCheckEnv` publication

`RegisteredTypeCheckEnv` keeps its current character, owner, descriptor, digest,
and revision fields and gains exactly one accepted callable field. The target
shape is the current production struct plus the marked addition:

```rust
pub struct RegisteredTypeCheckEnv {
    pub(crate) base: Arc<TypeCheckEnv>,
    pub(crate) characters: BTreeMap<CharacterId, CharacterManifest>,
    pub(crate) character_variants: BTreeMap<CharacterNominalType, BTreeSet<String>>,
    pub(crate) external_owners: ExternalOwnerRegistry,
    pub(crate) callables: Arc<RegisteredCallableCatalog>, // new
    pub(crate) world: ProjectSymbolWorldId,
    pub(crate) symbol_revision: ProjectSymbolRevision,
    pub(crate) character_descriptor: CharacterInventoryDescriptorV1,
    pub(crate) character_digest: CharacterInventoryDigest,
    pub(crate) character_revision: CharacterInventoryRevision,
}

impl RegisteredTypeCheckEnv {
    pub fn callables(&self) -> &RegisteredCallableCatalog;
}
```

All existing accessors, including `typecheck_env`, `world`, `symbol_revision`,
`character_enum_variants`, `character_manifest`, and `characters`, remain on
the original type. No parallel `RegisteredCharacterFacts` or
`RegisteredExternalOwnerFacts` wrapper is introduced. Presentation and
dialogue schema contexts borrow this exact accepted environment.

`CharacterRegistrar::register` builds character facts, external owners, project
symbols, callable publications, and the callable catalog in the same
fail-closed transaction, then constructs `RegisteredSemanticWorld`. Catalog
construction failure returns the existing registration error path before a new
world is returned, so the LSP/profile owner preserves its previous accepted
world pointer, generation, caches, character facts, and callable catalog. No
world-only or catalog-only publication API is added.

`TypeCheckEnv` may retain private low-level type/symbol storage during migration,
but after the last callable family moves it no longer exposes successful
callable lookup used by the checker. `RegisteredTypeCheckEnv::callables()` is
the accepted callable authority.

## 16. Resolver input

The shared resolver is checker-owned sema code. It borrows all mutable query
state explicitly and never mutates accepted-world state.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CallCallee<'a> {
    Free {
        path: &'a CallablePath,
    },
    Selected {
        receiver_expression: TypeExpressionId,
        receiver_type: &'a TypeKind,
        method: &'a CallableName,
    },
    Dialogue {
        id: DialogueCallableId,
        callee: &'a DialogueCalleeIdentity,
    },
    FunctionValue {
        value: &'a ResolvedFunctionValueSeed,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedFunctionValueSeed {
    id: FunctionValueSignatureId,
    ty: TypeKind,
    schema: CallableSignatureSchema,
    effect_callable: Option<CallableId>,
    source_candidate: Option<CallableCandidateId>,
    next_group: CallableGroupIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CallSourceContext<'a> {
    document: &'a SourceDocumentIdentity,
    call_span: Option<&'a SourceSpan>,
    callee_span: Option<&'a SourceSpan>,
}

pub(crate) struct CallResolverRequest<'a> {
    callee: CallCallee<'a>,
    lexical: &'a LexicalCallableScope,
    expected: Option<&'a TypeKind>,
    current_module: &'a CanonicalModulePath,
    symbols: &'a ProjectSymbolTable,
    world: &'a RegisteredSemanticWorld,
    traits: &'a TraitCatalog,
    source: CallSourceContext<'a>,
    call_group: CallableGroupIndex,
    expression: TypeExpressionId,
    cancellation: &'a std::sync::atomic::AtomicBool,
    work: &'a mut ResolverWork,
    limits: &'a CallableLimits,
}
```

Every field has a same-named accessor. Construction is validating and
`pub(crate)`:

```rust
impl<'a> CallResolverRequest<'a> {
    pub(crate) fn try_new(
        callee: CallCallee<'a>,
        lexical: &'a LexicalCallableScope,
        expected: Option<&'a TypeKind>,
        current_module: &'a CanonicalModulePath,
        symbols: &'a ProjectSymbolTable,
        world: &'a RegisteredSemanticWorld,
        traits: &'a TraitCatalog,
        source: CallSourceContext<'a>,
        call_group: CallableGroupIndex,
        expression: TypeExpressionId,
        cancellation: &'a std::sync::atomic::AtomicBool,
        work: &'a mut ResolverWork,
        limits: &'a CallableLimits,
    ) -> Result<Self, ResolveCallError>;
}
```

Validation requires `world.symbols()` to have the same world/revision as
`symbols`, `world.environment().callables()` to have been published in that
world, `source.document` to equal the accepted document supplied by the query,
and every supplied span to belong to and be contained by that document. The
constructor does not require call spans when AW-AH-009.3.1 is not yet connected.

`LexicalCallableScope` is a checker snapshot, not accepted state:

```rust
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct LexicalCallableScope {
    bindings: std::collections::HashMap<CallableName, LexicalCallBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LexicalCallBinding {
    Callable {
        id: LocalCallableId,
        schema: Arc<CallableSignatureSchema>,
        effects: EffectRow,
    },
    FunctionValue(ResolvedFunctionValueSeed),
    NonCallable {
        ty: TypeKind,
    },
}

impl LexicalCallableScope {
    pub(crate) fn binding(&self, name: &CallableName) -> Option<&LexicalCallBinding>;
}
```

The checker constructs this snapshot from existing local scopes. It does not
expose mutable insertion to signature-query callers.

## 17. Resolved products

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCallable {
    id: CallableCandidateId,
    origin: SignatureOrigin,
    schema: Arc<CallableSignatureSchema>,
    instantiation: CallableInstantiation,
    equivalent_sources: Arc<[EquivalentCallableSource]>,
    authority: Option<CallableAuthorityRank>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableInstantiation {
    None,
    ExpectedEnum {
        expected: TypeKind,
    },
    Result {
        kind: ResultConstructorKind,
        expected: Option<TypeKind>,
    },
    Option {
        expected: Option<TypeKind>,
    },
    Character {
        owner: ResolvedCharacterOwner,
    },
    Receiver {
        receiver: TypeKind,
    },
    Curried {
        base: CallableCandidateId,
        group: CallableGroupIndex,
    },
    DataLast {
        receiver: TypeKind,
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionValue {
    id: FunctionValueSignatureId,
    callable: ResolvedCallable,
    function_type: TypeKind,
    effect_callable: Option<CallableId>,
    source_candidate: Option<CallableCandidateId>,
    current_group: CallableGroupIndex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedCallTarget {
    Candidates(NonEmptyResolvedCandidates),
    FunctionValue(ResolvedFunctionValue),
    NonCallable(ResolvedNonCallableTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonEmptyResolvedCandidates {
    candidates: Arc<[ResolvedCallable]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedNonCallableTarget {
    source: NonCallableSource,
    ty: TypeKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NonCallableSource {
    Lexical { name: CallableName },
    Project { path: ProjectCallablePath },
    EvaluatedExpression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolveCallOutcome {
    Resolved(ResolvedCallTarget),
    Missing(UnknownCallTarget),
    Rejected(ResolveCallError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnknownCallTarget {
    kind: UnknownCallKind,
    path: Option<CallablePath>,
    receiver: Option<TypeKind>,
    method: Option<CallableName>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownCallKind {
    Free,
    Method,
    Dialogue,
}
```

```rust
impl ResolvedCallable {
    pub fn try_new(
        id: CallableCandidateId,
        origin: SignatureOrigin,
        schema: Arc<CallableSignatureSchema>,
        instantiation: CallableInstantiation,
        equivalent_sources: Vec<EquivalentCallableSource>,
        authority: Option<CallableAuthorityRank>,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError>;
    pub const fn id(&self) -> &CallableCandidateId;
    pub const fn origin(&self) -> &SignatureOrigin;
    pub const fn schema(&self) -> &CallableSignatureSchema;
    pub const fn instantiation(&self) -> &CallableInstantiation;
    pub fn equivalent_sources(&self) -> &[EquivalentCallableSource];
    pub const fn authority(&self) -> Option<CallableAuthorityRank>;
}

impl ResolvedFunctionValue {
    pub fn try_new(
        id: FunctionValueSignatureId,
        callable: ResolvedCallable,
        function_type: TypeKind,
        effect_callable: Option<CallableId>,
        source_candidate: Option<CallableCandidateId>,
        current_group: CallableGroupIndex,
    ) -> Result<Self, ResolveCallError>;
    pub const fn id(&self) -> &FunctionValueSignatureId;
    pub const fn callable(&self) -> &ResolvedCallable;
    pub const fn function_type(&self) -> &TypeKind;
    pub const fn effect_callable(&self) -> Option<&CallableId>;
    pub const fn source_candidate(&self) -> Option<&CallableCandidateId>;
    pub const fn current_group(&self) -> CallableGroupIndex;
}

impl NonEmptyResolvedCandidates {
    pub(crate) fn try_new(
        candidates: Vec<ResolvedCallable>,
        limits: &CallableLimits,
    ) -> Result<Self, ResolveCallError>;
    pub fn first(&self) -> &ResolvedCallable;
    pub fn as_slice(&self) -> &[ResolvedCallable];
    pub fn len(&self) -> NonZeroU32;
}

impl ResolvedNonCallableTarget {
    pub fn new(source: NonCallableSource, ty: TypeKind) -> Self;
    pub const fn source(&self) -> &NonCallableSource;
    pub const fn ty(&self) -> &TypeKind;
}

impl UnknownCallTarget {
    pub fn new(
        kind: UnknownCallKind,
        path: Option<CallablePath>,
        receiver: Option<TypeKind>,
        method: Option<CallableName>,
    ) -> Self;
    pub const fn kind(&self) -> UnknownCallKind;
    pub const fn path(&self) -> Option<&CallablePath>;
    pub const fn receiver(&self) -> Option<&TypeKind>;
    pub const fn method(&self) -> Option<&CallableName>;
}
```

`ResolvedCallable::try_new` checks origin/ID/family agreement, authority
agreement, instantiation agreement, equivalent-source uniqueness, and schema
limits. `ResolvedFunctionValue::try_new` requires a `TypeKind::Function`, a
`FunctionValue` primary ID, and a valid current group. `Candidates` cannot be
empty by construction. Missing and non-callable outcomes are not represented
as empty candidate lists.

## 18. Character owner acquisition

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCharacterOwner {
    character: CharacterId,
    source: CharacterOwnerSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterOwnerSource {
    EntityReference,
    LexicalBinding { name: CallableName },
    ProjectBinding { path: ProjectCallablePath },
    ExternalOwner,
    SpeakerValue,
    SpeakerPresetValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CharacterOwnerResolution {
    Known(ResolvedCharacterOwner),
    Missing,
    NonCharacter { actual: TypeKind },
    UnknownExternalOwner,
    UnknownPart { part: CharacterPartId },
    Poisoned,
}

impl ResolvedCharacterOwner {
    pub fn new(character: CharacterId, source: CharacterOwnerSource) -> Self;
    pub const fn character(&self) -> &CharacterId;
    pub const fn source(&self) -> &CharacterOwnerSource;
}
```

Owner acquisition uses existing typed expression judgment, entity identity,
project symbol identity, lexical binding facts, and registered external-owner
facts. It never parses canonical/compact/qualified/alias spelling. Aliases only
change display text supplied by the caller; all four spellings resolve to the
same `CharacterId`.

For `show`, `ref.show`, `hide`, and dialogue speaker/content calls, the checker
resolves the character argument or speaker target before constructing the
schema. `show.look` and dialogue `look` receive exactly:

```rust
TypeKind::CharacterNominal(CharacterNominalType::Look {
    character: owner.character().clone(),
})
```

No local look spelling is used in identity. A `Part` or `Variant` expectation
is constructed only for a parameter explicitly defined by its family schema;
this contract does not reinterpret `look` as part/variant.

On `Missing`, `NonCharacter`, `UnknownExternalOwner`, or `UnknownPart`, the
schema uses `CallableParameterType::Unchecked` for the affected dynamic
parameter, the checker still checks the expression once without an expected
type, records a typed character-owner diagnostic and poison, and retains the
family target. It does not guess an owner, use another character with the same
local name, or fall through to another resolver.

## 19. Shared resolver entry point and free-call precedence

```rust
pub(crate) fn resolve_call_target(
    request: CallResolverRequest<'_>,
) -> ResolveCallOutcome;
```

Before every family probe and every candidate append, the resolver checks
cancellation and charges work. A cancellation returns
`ResolveCallError::Cancelled`; it never returns a partial candidate set.

Free-call precedence is fixed to current production behavior:

1. observe user FX-definition validation for the path; this stages validation
   metadata but does not itself select a target;
2. closed `Fx.<member>` resolution, including poisoned unknown-member recovery;
3. expected-type project enum constructor, then closed `Result::{Ok,Err}` and
   `Option::Some` constructors;
4. closed builtin and capability functions;
5. closed Agent intrinsic functions;
6. closed presentation functions;
7. exact lexical binding for a one-segment path;
8. exact project symbol binding in the current module/import scope;
9. accepted environment free functions: Standard candidates before Adapter
   candidates after viability/specificity comparison;
10. well-known virtual runtime paths currently accepted by production;
11. speaker/speaker-preset callable values and ordinary evaluated function
   values;
12. unknown or non-callable outcome.

Steps 2 through 6 are language-owned reserved call families and preserve their
current priority over same-spelling project/environment names. A lexical or
project non-callable binding at steps 7 or 8 stops steps 9 through 12 and
returns `ResolvedCallTarget::NonCallable`. It does not shadow closed language
families selected at steps 2 through 6.

Expected enum resolution uses the exact expected `TypeKind` and project nominal
type identity. Without a qualifying expected type, short project enum variants
do not fabricate a candidate. `Ok`, `Err`, and `Some` retain current partial
placeholder result behavior when no expected type is available, but the
placeholder is a typed schema instantiation and poison state, not an opaque
signature ID.

Virtual-path validation remains checker validation attached to the resolved
ordinary environment/project candidate. OS-absolute or otherwise prohibited
paths produce their existing diagnostic and poison; they do not trigger a
second name resolver.

## 20. Selected-call precedence

For `CallCallee::Selected`, `resolve_call_target` uses this exact sequence:

1. drop-name special form;
2. Arcweft domain `traverse` and `parallel`;
3. accepted environment method-signature catalog records (`Method`), including
   schemas whose parameter checks are intentionally unchecked;
4. builtin collection methods `len`, `map`, `filter`, `sum`, `contains`;
5. presentation-handle lifecycle methods and `Overlay.pop`;
6. integer `clamp`, `min`, `max`;
7. remaining Arcweft domain methods;
8. well-known untyped capacity methods;
9. visible trait method resolution;
10. data-last callable fallback;
11. normalized legacy map-only environment methods
    (`UntypedMethodFallback`);
12. unknown method.

This ordering preserves the current implementation's non-obvious property that
an accepted environment **method-signature** record shadows builtin
collection/domain/capacity and trait/data-last methods of the same
receiver/name. `traverse` and `parallel` remain ahead of those records because
they are currently probed first. A legacy map-only method type remains after
data-last and therefore cannot be promoted to the earlier phase merely by
normalization. The resolver records an inherent/data-last shadow warning candidate when
steps 2 through 9 select a method while a viable data-last callable is visible;
the warning is emitted by checker commit, not by a second resolver.

The remaining domain inventory is exact:

- `FxSampleContext.ordinal_phase`;
- `Vec<ObservedObject>.require_role`;
- `Map<K,V>.get`;
- `Probe<T>.eq`, `ne`, `not_eq`, `gt`, `greater`, `ge`,
  `greater_or_equal`, `lt`, `less`, `le`, `less_or_equal`;
- `Diagnostics.has_error`;
- `RagContextPack.summary`;
- `Need`, `Option`, and `Result` `context` and `with_context`;
- character speaker `face` and `say`.

Trait resolution preserves the existing `Missing`, inherent, unique visible,
and ambiguous outcomes. Inherent/unique results become one `TraitMethod`
candidate. Ambiguity returns `ResolveCallError::AmbiguousTraitMethod`, checks
arguments once in poisoned/untyped recovery, and does not continue to data-last
or capacity fallback.

Data-last visibility is lexical, then project, then Standard, then Adapter.
Exact duplicate callables coalesce before applicability. Multiple viable
same-rank sources remain an ambiguity. The receiver may satisfy only the final
parameter in the current group or the sole parameter in the next curried group.
A successful direct/inherent/trait/capacity method shadows data-last; an
incompatible direct candidate does not consume the fallback unless current
production already treats the name as a closed method family. The family
inventory in `SURFACE_INVENTORY.md` records each closed case.

There is no final direct read of an old `TypeCheckEnv::method_type` map. During
migration, every legacy map-only entry is normalized to a catalog record whose
kind is `UntypedMethodFallback` and whose validator is `Untyped`; the shared
resolver probes those records only at step 11. Once that cut compiles, the old
map read and final checker branch are deleted without changing precedence.

## 21. Presentation schema decisions

All presentation schemas use the common records from section 10. Their return
types are exact:

| Call | Result |
|---|---|
| `view` | `Handle<View>` |
| `menu` | `Handle<Menu>` |
| `overlay` | `Handle<Overlay>` |
| `bg` | `Handle<BackgroundSurface>` |
| `image` | `Handle<ImageSurface>` |
| `player_viewport` | `Handle<Viewport>` |
| `show` | `Handle<CharacterSurface>` |
| `ref.bg` | `SlotRef<BackgroundSurface>` |
| `ref.show` | `SlotRef<CharacterSurface>` |
| `clear.bg` | `Option<BackgroundSurface>` |
| `hide` | `Option<CharacterSurface>` |

`view`, `menu`, and `overlay` have a required positional-or-named `view` entity
parameter and known named parameters `lifetime`, `target`, `layer`, `id`,
`handle`, `key`, `mount`, `depth`, `visible`, and `enabled`. Unknown named
arguments are `OpenUnchecked`, matching current behavior. `depth` is `I32`;
`visible`/`enabled` are `Bool`; entity parameters retain their exact entity
`TypeKind`.

`show` has group 0:

```text
0 character: Character entity, required, positional-or-named
1 look: CharacterNominalType::Look { character }, optional, positional-or-named
2 target: Target entity, optional, named-only
3 slot: character Slot entity, optional, named-only
4 scope: scope entity, optional, named-only
```

Unknown named arguments are `OpenUnchecked`, preserving current presentation
openness. `look` may be supplied as the second positional argument or by name;
argument mapping treats those as the same parameter and diagnoses a duplicate.
`ref.show` and `hide` use the same typed character owner acquisition. `ref.show`
does not currently accept a look parameter, so it does not invent one.

`bg`, `image`, and `player_viewport` use closed named schemas. Their existing
family validator preserves all current typed special cases: asset/image source,
lifetime enum spellings, target/layer IDs or public IDs, alignment/playback
ratio forms, opacity, dimensions, transform components, booleans, custom
`param.*` and `proxy.param.*` values, and all current unknown-name diagnostics.
The resolver supplies identity and schema; the presentation validator remains
responsible for these value-shape checks.

`ref.bg` and `clear.bg` retain open presentation named behavior and background
slot semantics. `clear.bg` and `hide` retain the existing checker side effect on
`active_presentation_defaults`; that effect occurs only after candidate commit
and is not part of resolver state.

## 22. Dialogue schema decisions

`DialogueCallableId::signature_schema` maps typed `LineOptions` fields into one
common schema. The parameter order is:

```text
0 id
1 text_key
2 voice
3 look
4 stage
5 portrait
6 focus
7 cleanup
8 view
9 source_locale
10 hooks (rest named/sequence policy owned by dialogue validator)
11 style
12 rich_text
13+ authored LineArg entries in original order
```

The exact parser/HIR carrier for argument ranges is owned by AW-AH-009.3.1.
This contract fixes semantic mapping only.

For `DialogueCalleeIdentity::Speaker` and `SpeakerPreset`, `look` is
`CharacterNominalType::Look { character }`. `ContentCall` receives that same
expectation only when typed callee resolution yields a character speaker or
speaker preset. Otherwise `look` is `Unchecked`, a typed
`DialogueLookOwnerUnavailable` diagnostic is recorded, and the expression is
checked once without an expected type.

`id`, `text_key`, `view`, and other existing special fields retain their current
family validation. User `LineArg` names are open named parameters except that a
name colliding with a reserved `LineOptions` field maps to the reserved field;
a second occurrence is a duplicate. Open names preserve authored order for
presentation but never enter callable identity.

Dialogue content token checking, wait/speed tag validation, mark collection,
FX span validation, inline failure policy, source-map projection, and line-plan
checking remain outside callable argument mapping. The committed dialogue call
candidate only supplies option-expression expected types, result/effects, and
target facts.

Unknown/missing/non-character owner diagnostics are stable typed codes:

- `CharacterOwnerMissing`;
- `CharacterOwnerTypeMismatch`;
- `CharacterOwnerUnknownExternal`;
- `CharacterOwnerUnknownPart`;
- `PresentationLookOwnerUnavailable`;
- `DialogueLookOwnerUnavailable`.

They never include a parsed alias as identity. Diagnostic presentation may
include the authored spelling and canonical display label as separate fields.

## 23. Argument mapping and overload selection

There is one argument engine:

```rust
pub(crate) fn check_resolved_call(
    target: ResolvedCallTarget,
    input: CallArgumentInput<'_>,
    checker: &mut TypeChecker<'_>,
) -> CheckedCallTarget;

pub(crate) struct CallArgumentInput<'a> {
    arguments: &'a [CallArgumentRef<'a>],
    receiver: Option<CallReceiverRef<'a>>,
    group: CallableGroupIndex,
    expected_result: Option<&'a TypeKind>,
    expression: TypeExpressionId,
    cancellation: &'a std::sync::atomic::AtomicBool,
    work: &'a mut ResolverWork,
    limits: &'a CallableLimits,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CallReceiverRef<'a> {
    expression: TypeExpressionId,
    value: &'a Expr,
    ty: &'a TypeKind,
    source: Option<&'a SourceSpan>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CallArgumentRef<'a> {
    index: CallableArgumentIndex,
    value: &'a Expr,
    name: Option<&'a CallableName>,
    spread: bool,
    source: Option<&'a SourceSpan>,
}
```

`CallArgumentRef` and `CallReceiverRef` are borrowed adapters over the accepted
typed syntax/HIR carrier. They retain the exact `&Expr` that the ordinary
checker must evaluate and contain no source string. `CallArgumentRef` does not
pretend that a child `TypeExpressionId` exists before traversal: the ordinary
checker allocates expression IDs when each mapped value or fixed-spread element
is actually checked, and the argument engine stores those IDs in committed slot
facts. The receiver ID is available because selected-call resolution checks the
receiver before resolving its method.

Fixed literal spread is expanded by the common mapper from the typed expression,
using the existing `BracketSeq`/`NumericBracketSeq` variants rather than
reparsing. Authored argument indices are contiguous and checked. Their fields
have accessors and their constructors are `pub(crate)`. `CallArgumentInput`
borrows the same cancellation flag, `ResolverWork`, and limits used by target
resolution; candidate checking cannot start a fresh budget.

For each candidate, the engine creates a checker transaction checkpoint and:

1. maps positional, named, reordered, defaulted, optional, rest, and spread
   arguments to parameters;
2. detects duplicate names/slots, unknown names, missing required parameters,
   positional overflow, unsupported spread, and invalid group;
3. checks every mapped expression once with its `Exact` expected type or once
   without an expected type for `Unchecked`;
4. runs the candidate's family validator for value-shape behavior not expressible
   as ordinary type compatibility;
5. records inferred argument types, diagnostics, effects, higher-order effect
   connections, return type, current/next curried group, poison, and work;
6. rolls back the checkpoint before evaluating another candidate.

Candidate comparison is deterministic:

1. non-poisoned viable beats poisoned recovery;
2. fewer hard argument errors wins;
3. more exact typed parameter matches wins;
4. fewer unchecked/open matches wins;
5. fewer default/optional omissions wins;
6. for an otherwise equal Standard/Adapter tie, Standard wins;
7. an otherwise equal overload tie within one provider is
   `AmbiguousOverload`;
8. any remaining equal candidates at the same authority are ambiguity, never
   insertion-order selection.

Only the selected transaction is replayed/committed once. Diagnostics, effects,
borrow evidence, higher-order effect edges, presentation-default mutations, and
target facts from rejected candidates are discarded. This is not duplicate
successful resolution: name/target resolution happens once, while bounded
candidate viability is transactional argument checking.

Family-specific validation retained after resolution is limited to:

- FX closed property/list/conditional/shader rules;
- Agent intrinsic argument-shape and semantic resource/entity rules;
- presentation and dialogue special value forms and state mutation;
- enum/Result/Option expected-type payload recovery;
- builtin numeric/assert/capability exact rules;
- collection higher-order mapping and return inference;
- domain `traverse`, `parallel`, context, probe, map, role, speaker behavior;
- trait projection and trait diagnostics already supplied by trait resolution;
- data-last receiver injection and effect connection;
- capacity and legacy-untyped argument checking;
- virtual-path policy, partial/curried function behavior, and higher-order
  effect callable propagation.

These validators receive the already selected typed candidate and may not look
up a name or method again.

## 24. Checker target-fact mode

The checker records call facts during the same invocation that selects and
checks the target. Signature help projects those facts; it never calls the
resolver again.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallTargetFactMode {
    Disabled,
    Focused { expression: TypeExpressionId },
    All,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedCallTarget {
    target: CallTargetFact,
    result: Option<TypeKind>,
    arguments: Arc<[CheckedCallArgumentFact]>,
    effects: EffectRow,
    current_group: CallableGroupIndex,
    next_group: Option<CallableGroupIndex>,
    function_value_type: Option<TypeKind>,
    poison: CallPoison,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallTargetFacts {
    expression: TypeExpressionId,
    document: SourceDocumentIdentity,
    call_span: Option<SourceSpan>,
    target: CallTargetFact,
    arguments: Arc<[CheckedCallArgumentFact]>,
    result: Option<TypeKind>,
    effects: EffectRow,
    current_group: CallableGroupIndex,
    next_group: Option<CallableGroupIndex>,
    function_value_type: Option<TypeKind>,
    poison: CallPoison,
    diagnostics: Arc<[CallableDiagnostic]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallTargetFact {
    Selected {
        primary: CallableCandidateId,
        equivalent: Arc<[CallableCandidateId]>,
        considered: Arc<[CallableCandidateId]>,
        origin: SignatureOrigin,
        schema: Arc<CallableSignatureSchema>,
    },
    Ambiguous {
        candidates: Arc<[CallableCandidateId]>,
    },
    NonCallable {
        source: NonCallableSource,
        ty: TypeKind,
    },
    Missing {
        kind: UnknownCallKind,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallArgumentFact {
    index: CallableArgumentIndex,
    source: Option<SourceSpan>,
    authored_name: Option<CallableName>,
    spread: bool,
    slots: Arc<[CheckedCallArgumentSlotFact]>,
    poison: CallPoison,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCallArgumentSlotFact {
    slot: CallableArgumentSlotIndex,
    expression: TypeExpressionId,
    source: Option<SourceSpan>,
    mapped: Option<CallableParameterCoordinate>,
    inferred: Option<TypeKind>,
    expected: Option<TypeKind>,
    poison: CallPoison,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableParameterCoordinate {
    group: CallableGroupIndex,
    parameter: CallableParameterIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallPoison {
    Clean,
    Recovered,
    Rejected,
}
```

The exact read API is:

```rust
impl CallTargetFacts {
    pub const fn expression(&self) -> TypeExpressionId;
    pub const fn document(&self) -> &SourceDocumentIdentity;
    pub const fn call_span(&self) -> Option<&SourceSpan>;
    pub const fn target(&self) -> &CallTargetFact;
    pub fn arguments(&self) -> &[CheckedCallArgumentFact];
    pub const fn result(&self) -> Option<&TypeKind>;
    pub const fn effects(&self) -> &EffectRow;
    pub const fn current_group(&self) -> CallableGroupIndex;
    pub const fn next_group(&self) -> Option<CallableGroupIndex>;
    pub const fn function_value_type(&self) -> Option<&TypeKind>;
    pub const fn poison(&self) -> CallPoison;
    pub fn diagnostics(&self) -> &[CallableDiagnostic];
}

impl CheckedCallArgumentFact {
    pub const fn index(&self) -> CallableArgumentIndex;
    pub const fn source(&self) -> Option<&SourceSpan>;
    pub const fn authored_name(&self) -> Option<&CallableName>;
    pub const fn spread(&self) -> bool;
    pub fn slots(&self) -> &[CheckedCallArgumentSlotFact];
    pub const fn poison(&self) -> CallPoison;
}

impl CheckedCallArgumentSlotFact {
    pub const fn slot(&self) -> CallableArgumentSlotIndex;
    pub const fn expression(&self) -> TypeExpressionId;
    pub const fn source(&self) -> Option<&SourceSpan>;
    pub const fn mapped(&self) -> Option<CallableParameterCoordinate>;
    pub const fn inferred(&self) -> Option<&TypeKind>;
    pub const fn expected(&self) -> Option<&TypeKind>;
    pub const fn poison(&self) -> CallPoison;
}

impl CallableParameterCoordinate {
    pub const fn new(
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    ) -> Self;
    pub const fn group(self) -> CallableGroupIndex;
    pub const fn parameter(self) -> CallableParameterIndex;
}

impl CallPoison {
    pub const fn merge(self, other: Self) -> Self;
}
```

Fact constructors are `pub(crate)`. Argument and slot indices are contiguous;
an ordinary argument has exactly one slot, a fixed literal spread has one slot
per expanded typed element, and an unsupported/open spread has exactly one
unmapped recovered slot. The slot source is the element range when available;
the argument source remains the complete authored argument. `CallableParameterCoordinate::new`
is public, but existence in a particular schema is checked by every result
constructor. `CallPoison` is monotonic: `Clean < Recovered < Rejected`.

`TypeChecker` gains a fact mode and a private recorder. `TypeCheckReport` gains:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallTargetFactError {
    FocusedTargetMissing { expression: TypeExpressionId },
    FocusedTargetDuplicate { expression: TypeExpressionId },
    DuplicateExpression { expression: TypeExpressionId },
}

impl TypeCheckReport {
    pub fn call_target_facts(
        &self,
        expression: TypeExpressionId,
    ) -> Result<Option<&CallTargetFacts>, CallTargetFactError>;

    pub fn focused_call_target_facts(
        &self,
    ) -> Result<&CallTargetFacts, CallTargetFactError>;
}
```

`Focused` records exactly one matching expression and returns
`FocusedTargetMissing` or `FocusedTargetDuplicate` if traversal does not
encounter exactly one. `All` uses a map keyed by `TypeExpressionId` and returns
`DuplicateExpression` rather than replacing a prior fact. `Disabled`
does not allocate fact vectors. None of the modes mutates
`RegisteredSemanticWorld`, `RegisteredTypeCheckEnv`, catalogs, source identity,
or accepted caches.

The committed primary candidate ID in `CallTargetFacts` is the candidate ID
used for checker effects and the candidate ID projected into
`SemanticSignature`. A direct test compares them for every family.

## 25. Public semantic signature results

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticSignatureIndex(u16);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticParameter {
    coordinate: CallableParameterCoordinate,
    label: Arc<str>,
    name: Option<CallableName>,
    ty: CallableParameterType,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    documentation: Option<Arc<str>>,
    source: Option<CallableParameterSource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSignature {
    candidate: CallableCandidateId,
    equivalent: Arc<[CallableCandidateId]>,
    origin: SignatureOrigin,
    label: Arc<str>,
    groups: Arc<[SemanticParameterGroup]>,
    result: TypeKind,
    effects: EffectRow,
    documentation: CallableDocumentation,
    source: Option<CallableSource>,
    current_group: CallableGroupIndex,
    poison: CallPoison,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticParameterGroup {
    index: CallableGroupIndex,
    kind: CallableGroupKind,
    parameters: Arc<[SemanticParameter]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSignatureHelp {
    document: SourceDocumentIdentity,
    call_span: SourceSpan,
    signatures: Arc<[SemanticSignature]>,
    active_signature: SemanticSignatureIndex,
    active_parameter: Option<CallableParameterCoordinate>,
    diagnostics: Arc<[CallableDiagnostic]>,
    work: SignatureWorkReport,
}
```

```rust
impl SemanticSignatureIndex {
    pub fn try_from_usize(value: usize) -> Result<Self, SemanticSignatureError>;
    pub const fn get(self) -> usize;
}

impl SemanticParameter {
    pub fn try_new(
        coordinate: CallableParameterCoordinate,
        label: impl Into<Arc<str>>,
        name: Option<CallableName>,
        ty: CallableParameterType,
        passing: CallableParameterPassing,
        presence: CallableParameterPresence,
        documentation: Option<Arc<str>>,
        source: Option<CallableParameterSource>,
    ) -> Result<Self, SemanticSignatureError>;
    pub const fn coordinate(&self) -> CallableParameterCoordinate;
    pub fn label(&self) -> &str;
    pub const fn name(&self) -> Option<&CallableName>;
    pub const fn ty(&self) -> &CallableParameterType;
    pub const fn passing(&self) -> CallableParameterPassing;
    pub const fn presence(&self) -> CallableParameterPresence;
    pub fn documentation(&self) -> Option<&str>;
    pub const fn source(&self) -> Option<&CallableParameterSource>;
}

impl SemanticParameterGroup {
    pub fn try_new(
        index: CallableGroupIndex,
        kind: CallableGroupKind,
        parameters: Vec<SemanticParameter>,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError>;
    pub const fn index(&self) -> CallableGroupIndex;
    pub const fn kind(&self) -> CallableGroupKind;
    pub fn parameters(&self) -> &[SemanticParameter];
}

impl SemanticSignature {
    pub fn try_new(
        candidate: CallableCandidateId,
        equivalent: Vec<CallableCandidateId>,
        origin: SignatureOrigin,
        label: Arc<str>,
        groups: Vec<SemanticParameterGroup>,
        result: TypeKind,
        effects: EffectRow,
        documentation: CallableDocumentation,
        source: Option<CallableSource>,
        current_group: CallableGroupIndex,
        poison: CallPoison,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError>;

    pub const fn candidate(&self) -> &CallableCandidateId;
    pub fn equivalent(&self) -> &[CallableCandidateId];
    pub const fn origin(&self) -> &SignatureOrigin;
    pub fn label(&self) -> &str;
    pub fn groups(&self) -> &[SemanticParameterGroup];
    pub const fn result(&self) -> &TypeKind;
    pub const fn effects(&self) -> &EffectRow;
    pub const fn documentation(&self) -> &CallableDocumentation;
    pub const fn source(&self) -> Option<&CallableSource>;
    pub const fn current_group(&self) -> CallableGroupIndex;
    pub const fn poison(&self) -> CallPoison;
}

impl SemanticSignatureHelp {
    pub fn try_new(
        document: SourceDocumentIdentity,
        call_span: SourceSpan,
        signatures: Vec<SemanticSignature>,
        active_signature: SemanticSignatureIndex,
        active_parameter: Option<CallableParameterCoordinate>,
        diagnostics: Vec<CallableDiagnostic>,
        work: SignatureWorkReport,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError>;

    pub const fn document(&self) -> &SourceDocumentIdentity;
    pub const fn call_span(&self) -> &SourceSpan;
    pub fn signatures(&self) -> &[SemanticSignature];
    pub const fn active_signature(&self) -> SemanticSignatureIndex;
    pub const fn active_parameter(&self) -> Option<CallableParameterCoordinate>;
    pub fn diagnostics(&self) -> &[CallableDiagnostic];
    pub const fn work(&self) -> SignatureWorkReport;
}
```

Construction invariants:

- signatures are non-empty and within the candidate limit;
- `active_signature.get() < signatures.len()`;
- every signature candidate is unique after exact-duplicate coalescing;
- every equivalent candidate is unique and differs from its primary;
- group and parameter coordinates are contiguous and within limits;
- `current_group` exists in every signature shown for the selected call;
- an active parameter, when present, exists in the active signature's current
  group;
- `call_span` and every retained source/diagnostic span belong to `document` and
  are contained by its accepted byte length;
- signature labels and parameter labels are presentation strings produced from
  typed schema fields only; parsing them is forbidden;
- diagnostics and work are within limits.

When target facts are ambiguous, the help result contains every considered
viable semantic signature in deterministic order and a typed ambiguity
diagnostic; `active_signature` is the first deterministic candidate solely for
UI focus, not a checker selection. A non-callable, missing, stale, cancelled,
or limit-exhausted query returns the typed query outcome fixed by AW-AH-009.3;
it does not fabricate an empty `SemanticSignatureHelp`.

## 26. Typed diagnostics

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CallableDiagnosticCode {
    UnknownCallable,
    UnknownMethod,
    NonCallableTarget,
    UnknownFxConstructor,
    InvalidFxPath,
    AmbiguousOverload,
    AmbiguousTraitMethod,
    DuplicateArgument,
    UnknownNamedArgument,
    MissingArgument,
    TooManyPositionalArguments,
    UnsupportedSpread,
    InvalidCallGroup,
    ArgumentTypeMismatch,
    ResultConstructorExpectedType,
    EnumConstructorExpectedType,
    CharacterOwnerMissing,
    CharacterOwnerTypeMismatch,
    CharacterOwnerUnknownExternal,
    CharacterOwnerUnknownPart,
    PresentationLookOwnerUnavailable,
    DialogueLookOwnerUnavailable,
    DataLastAmbiguity,
    DataLastShadowed,
    VirtualPathRejected,
    CorruptCallableCatalog,
    WorldMismatch,
    SourceIdentityMismatch,
    Cancelled,
    ResourceExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDiagnostic {
    code: CallableDiagnosticCode,
    severity: CallableDiagnosticSeverity,
    span: Option<SourceSpan>,
    subject: CallableDiagnosticSubject,
    related: Arc<[CallableDiagnosticRelated]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableDiagnosticSeverity {
    Error,
    Warning,
    Information,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableDiagnosticSubject {
    Candidate(CallableCandidateId),
    Parameter(CallableParameterCoordinate),
    Argument(TypeExpressionId),
    Path(CallablePath),
    Method { receiver: TypeKind, name: CallableName },
    Character(CharacterId),
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallableDiagnosticRelated {
    subject: CallableDiagnosticSubject,
    span: Option<SourceSpan>,
}
```

```rust
impl CallableDiagnosticRelated {
    pub fn new(
        subject: CallableDiagnosticSubject,
        span: Option<SourceSpan>,
    ) -> Self;
    pub const fn subject(&self) -> &CallableDiagnosticSubject;
    pub const fn span(&self) -> Option<&SourceSpan>;
}

impl CallableDiagnostic {
    pub fn try_new(
        code: CallableDiagnosticCode,
        severity: CallableDiagnosticSeverity,
        span: Option<SourceSpan>,
        subject: CallableDiagnosticSubject,
        related: Vec<CallableDiagnosticRelated>,
        document: Option<&SourceDocumentIdentity>,
        limits: &CallableLimits,
    ) -> Result<Self, SemanticSignatureError>;
    pub const fn code(&self) -> CallableDiagnosticCode;
    pub const fn severity(&self) -> CallableDiagnosticSeverity;
    pub const fn span(&self) -> Option<&SourceSpan>;
    pub const fn subject(&self) -> &CallableDiagnosticSubject;
    pub fn related(&self) -> &[CallableDiagnosticRelated];
}
```

`try_new` validates source identity when a document is provided and enforces the
related-item and diagnostic limits. Error identity is
`CallableDiagnosticCode`; display messages are produced in the existing
diagnostics presentation layer. The resolver and argument engine never use
formatted strings for control flow.

## 27. Errors

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableIdentityError {
    Scalar(CallableScalarError),
    MissingGroup {
        base: Box<CallableCandidateId>,
        group: CallableGroupIndex,
    },
    InvalidCurriedBase {
        base: Box<CallableCandidateId>,
    },
    InvalidDataLastBase {
        base: Box<CallableCandidateId>,
    },
    InvalidDataLastCoordinate {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    DataLastReceiverIsRest {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    DataLastReceiverNotFinal {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RustProvenanceField {
    PackageName,
    PackageVersion,
    MetadataHash,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RustProvenanceError {
    Empty { field: RustProvenanceField },
    Control { field: RustProvenanceField, byte: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolveCallError {
    Cancelled,
    WorldMismatch,
    SourceIdentityMismatch,
    InvalidSourceSpan,
    InvalidCallGroup {
        candidate: CallableCandidateId,
        group: CallableGroupIndex,
    },
    CandidateLimit {
        actual: usize,
        limit: usize,
    },
    AmbiguousOverload {
        candidates: Arc<[CallableCandidateId]>,
    },
    AmbiguousTraitMethod {
        candidates: Arc<[TraitCallableId]>,
    },
    DataLastAmbiguity {
        candidates: Arc<[DataLastCallableId]>,
    },
    CorruptCatalog {
        key: CallableLookupKey,
        reason: CorruptCallableCatalogReason,
    },
    InvalidResolvedCallable,
    Work(CallableQueryLimitError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorruptCallableCatalogReason {
    EmptySet,
    KeyMismatch,
    DuplicateId,
    WrongAuthority,
    MissingRecord,
    InvalidEquivalent,
    Unsorted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableSchemaError {
    EmptyGroups,
    GroupLimit { actual: usize, limit: usize },
    ParameterLimit { actual: usize, limit: usize },
    NonContiguousGroup {
        expected: CallableGroupIndex,
        actual: CallableGroupIndex,
    },
    InvalidGroupKind { group: CallableGroupIndex },
    NonContiguousParameter {
        group: CallableGroupIndex,
        expected: CallableParameterIndex,
        actual: CallableParameterIndex,
    },
    DuplicateParameterName {
        group: CallableGroupIndex,
        name: CallableName,
    },
    MissingParameterName {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    InvalidRestParameter {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    InvalidDefaultedRest {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    SourceCoordinateMismatch {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    FamilyInvariant {
        family: CallableFamily,
        code: CallableFamilyInvariantCode,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallableFamilyInvariantCode {
    InvalidArity,
    InvalidParameterType,
    InvalidResultType,
    InvalidArgumentPolicy,
    InvalidValidator,
    InvalidOwner,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableCatalogError {
    IdKeyMismatch,
    AuthorityProviderMismatch,
    MissingProjectSource,
    UnexpectedProjectRustProvenance,
    MissingRustProvenance,
    EmptyCandidateSet,
    CandidateSetKeyMismatch,
    OverloadLimit { actual: usize, limit: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallablePublicationError {
    OwnerMismatch,
    InvalidOverload,
    InvalidRecord(CallableCatalogError),
    InvalidSchema(CallableSchemaError),
    InvalidRustProvenance(RustProvenanceError),
    Limit(CallableBuildLimitError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableDocumentationError {
    DuplicateParameter {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    EmptyText,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableSourceError {
    DuplicateParameter {
        group: CallableGroupIndex,
        parameter: CallableParameterIndex,
    },
    SourceIdentityMismatch,
    SpanOutsideSignature,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticSignatureError {
    EmptySignatures,
    ActiveSignatureOutOfBounds,
    ActiveParameterOutOfBounds,
    CurrentGroupMissing,
    DuplicateCandidate,
    DuplicateEquivalentCandidate,
    SourceIdentityMismatch,
    InvalidSpan,
    Limit(CallableQueryLimitError),
}
```

Each error enum has an inherent `code()` returning a stable
`CallableDiagnosticCode` or a build-specific typed code. No error enum stores a
preformatted message as its identity.

## 28. Limits and work accounting

The limits are inclusive production constants and are not client-configurable.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallableLimits {
    max_path_segments: usize,
    max_groups_per_callable: usize,
    max_parameters_per_callable: usize,
    max_overloads_per_key: usize,
    max_candidates_per_call: usize,
    max_nested_calls: usize,
    max_recovery_nodes: usize,
    max_diagnostics: usize,
    max_source_bytes: usize,
    max_project_modules: usize,
    max_catalog_records: usize,
    max_catalog_build_work: u64,
    max_query_work: u64,
}

pub const PRODUCTION_CALLABLE_LIMITS: CallableLimits = CallableLimits {
    max_path_segments: 32,
    max_groups_per_callable: 16,
    max_parameters_per_callable: 128,
    max_overloads_per_key: 32,
    max_candidates_per_call: 256,
    max_nested_calls: 32,
    max_recovery_nodes: 256,
    max_diagnostics: 128,
    max_source_bytes: 8_388_608,
    max_project_modules: 4_096,
    max_catalog_records: 262_144,
    max_catalog_build_work: 1_048_576,
    max_query_work: 4_096,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CatalogBuildWork {
    consumed: u64,
    limit: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolverWork {
    consumed: u64,
    limit: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureWorkReport {
    resolver: u64,
    argument_mapping: u64,
    type_checks: u64,
    recovery_nodes: usize,
    diagnostics: usize,
}
```

```rust
impl CallableLimits {
    pub const fn max_path_segments(self) -> usize;
    pub const fn max_groups_per_callable(self) -> usize;
    pub const fn max_parameters_per_callable(self) -> usize;
    pub const fn max_overloads_per_key(self) -> usize;
    pub const fn max_candidates_per_call(self) -> usize;
    pub const fn max_nested_calls(self) -> usize;
    pub const fn max_recovery_nodes(self) -> usize;
    pub const fn max_diagnostics(self) -> usize;
    pub const fn max_source_bytes(self) -> usize;
    pub const fn max_project_modules(self) -> usize;
    pub const fn max_catalog_records(self) -> usize;
    pub const fn max_catalog_build_work(self) -> u64;
    pub const fn max_query_work(self) -> u64;
}

impl CatalogBuildWork {
    pub(crate) const fn new(limit: u64) -> Self;
    pub(crate) fn charge(&mut self, units: u64) -> Result<(), CallableBuildLimitError>;
    pub(crate) const fn consumed(self) -> u64;
    pub(crate) const fn remaining(self) -> u64;
    pub(crate) const fn limit(self) -> u64;
}

impl ResolverWork {
    pub(crate) const fn new(limit: u64) -> Self;
    pub(crate) fn charge(&mut self, units: u64) -> Result<(), CallableQueryLimitError>;
    pub(crate) const fn consumed(self) -> u64;
    pub(crate) const fn remaining(self) -> u64;
    pub(crate) const fn limit(self) -> u64;
}

impl SignatureWorkReport {
    pub fn try_new(
        resolver: u64,
        argument_mapping: u64,
        type_checks: u64,
        recovery_nodes: usize,
        diagnostics: usize,
        limits: &CallableLimits,
    ) -> Result<Self, CallableQueryLimitError>;
    pub const fn resolver(&self) -> u64;
    pub const fn argument_mapping(&self) -> u64;
    pub const fn type_checks(&self) -> u64;
    pub const fn recovery_nodes(&self) -> usize;
    pub const fn diagnostics(&self) -> usize;
    pub fn total_work(&self) -> Result<u64, CallableQueryLimitError>;
}
```

`CallableLimits` exposes exactly the const accessors above and no public
constructor. Tests in the owning crate use `pub(crate) fn for_test(...)` under
`cfg(test)` to exercise small exact/one-over values; production cannot override
the constant. `SignatureWorkReport::try_new` validates recovery and diagnostic
counts, uses checked addition for all three work components, and requires the
sum to be at most `max_query_work`; `total_work` repeats checked addition and
never saturates.

`CatalogBuildWork` and `ResolverWork` have `new(limit)`, `charge(units)`,
`consumed()`, `remaining()`, and `limit()` inherent methods. `charge` uses
`checked_add`; arithmetic overflow is a typed `WorkOverflow`, not saturation.
The exact limit succeeds. A charge that would produce `limit + 1` fails before
mutating `consumed`.

Catalog work charges:

| Operation | Charge |
|---|---:|
| visit project module, including empty | 1 |
| visit project symbol binding | 1 |
| begin callable record | 1 |
| each path segment | 1 |
| each parameter group | 1 |
| each parameter | 1 |
| each documentation parameter row | 1 |
| each environment publication | 1 |
| each environment record | 1 |
| each collision comparison | 1 |
| each exact-duplicate structural schema comparison | 1 plus one per group and parameter |
| each frozen index insertion | 1 |

Query work charges:

| Operation | Charge |
|---|---:|
| enter a call or recovered call node | 1 |
| probe one resolver family | 1 |
| inspect one lexical/project/catalog binding | 1 |
| construct one resolved candidate | 1 |
| inspect one trait candidate | 1 |
| inspect one data-last callable | 1 |
| map one argument | 1 |
| compare one candidate pair | 1 |
| run one family validator branch | 1 |
| stage one diagnostic | 1 |
| project one semantic parameter | 1 |

Type checking an argument charges one `type_checks` report unit and the existing
checker work budget; it also charges one callable query unit so signature-only
requests cannot bypass the callable budget. Nested call depth is checked before
entering the one-over call. Recovery nodes and diagnostics are checked before
staging the one-over item. Source byte size is validated by AW-AH-009.3.1/.2
input construction and rechecked by `SemanticSignatureHelp::try_new`.

Accepted-world build limit failure returns
`CallableCatalogBuildError::Limit`, rejects candidate publication, and preserves
the previous accepted world. Query limit failure returns a typed query error,
discards candidate checkpoints and target facts, and is never cached as
successful signature help. There is no deterministic truncation for candidate,
overload, group, parameter, module, record, recovery, diagnostic, or work limits.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableBuildLimitError {
    PathSegments { actual: usize, limit: usize },
    Groups { actual: usize, limit: usize },
    Parameters { actual: usize, limit: usize },
    Overloads { actual: usize, limit: usize },
    Modules { actual: usize, limit: usize },
    Records { actual: usize, limit: usize },
    Work { requested: u64, consumed: u64, limit: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CallableQueryLimitError {
    Candidates { actual: usize, limit: usize },
    NestedCalls { actual: usize, limit: usize },
    RecoveryNodes { actual: usize, limit: usize },
    Diagnostics { actual: usize, limit: usize },
    SourceBytes { actual: usize, limit: usize },
    Work { requested: u64, consumed: u64, limit: u64 },
    ArithmeticOverflow,
}
```

## 29. Defensive corrupt-world behavior

Although valid worlds are constructed only through the checked builder, public
query code must fail closed if internal corruption is observed. The resolver
validates non-empty sets, matching keys, ordered unique IDs, authority/provider
agreement, equivalent-source uniqueness, and by-ID reachability while reading
a candidate set. A violation returns `ResolveCallError::CorruptCatalog` and no
arguments are checked under a guessed target.

Tests may construct corrupt catalogs only through `cfg(test)` crate-owned
fixtures. No public unchecked constructor, Serde implementation, raw integer ID
constructor, or persisted callable catalog format is added.

## 30. Shared checker/query protocol

The end-to-end protocol is exact:

1. AW-AH-009.3.2 supplies an accepted `HirModule`, exact accepted source, and the
   matching `RegisteredSemanticWorld` lease.
2. AW-AH-009.3.1 identifies the typed call surface, typed callee/method, ordered
   typed arguments, exact ranges, and focused `TypeExpressionId`.
3. The signature query invokes the ordinary `TypeChecker` once with
   `CallTargetFactMode::Focused` and the existing expected-type context.
4. The checker constructs `CallResolverRequest` and calls
   `resolve_call_target` once at the focused call.
5. `check_resolved_call` maps and checks arguments transactionally, commits one
   candidate or one typed ambiguity/rejection, and records `CallTargetFacts`.
6. The query reads `TypeCheckReport::focused_call_target_facts`; it does not
   inspect names, catalog keys, or syntax again.
7. `SemanticSignature` values are projected from the recorded resolved schemas;
   active parameter comes from AW-AH-009.3.1's cursor-to-argument or fixed-spread
   slot result and the corresponding `CheckedCallArgumentSlotFact::mapped`.
   The query indexes committed facts only; it does not remap arguments.
8. `SemanticSignatureHelp::try_new` validates identity and indices.
9. LSP presentation converts the semantic result; it may choose display aliases
   but may not change candidate, owner, type, or source identity.

Ordinary full type checking uses the same steps with `Disabled` or `All` fact
mode. The resolver and argument checker have no feature flag and no
signature-help-specific branch.

## 31. Effects, curried calls, and function values

Environment and language schemas carry `CallableEffectSchema::Fixed`. Project
schemas carry `CallableEffectSchema::Project` with the declaration identity and
its declared row. At candidate checking, the existing effect collector
instantiates that project declaration to the invocation-local `EffectRow`; the
resolved row, not the accepted schema seed, is recorded in `CallTargetFacts` and
`SemanticSignature`. Candidate checking stages effects transactionally and
commits only the selected candidate's effects.

A call whose current group is not the final schema group returns the existing
function `TypeKind` for the remaining groups and records `next_group`. The
selected target remains the base candidate plus
`CallableInstantiation::Curried`; subsequent invocation uses a
`CurriedCallableId` and the same schema. Partial-call acceptance remains
controlled by the existing checker context; the resolver does not infer it from
source position.

Higher-order function arguments retain their exact function `TypeKind` and
existing effect-callable identity. `ResolvedFunctionValue` carries the
function-value type, source candidate when known, effect callable, and current
group. The checker connects higher-order effects only after candidate commit.
A function value with no known source candidate still has a deterministic
`FunctionValueSignatureId` and complete schema derived from its `TypeKind`.
Named arguments and non-fixed spread remain rejected for ordinary function
values exactly as today; fixed literal spread is mapped under
`SpreadArgumentPolicy::FixedLiteralOnly`.

Speaker and speaker-preset callable values remain their own candidate family.
They check each current untyped argument once and return the corresponding
speaker-preset type. They do not masquerade as project functions.

## 32. Standard/adapter precedence and overload behavior

The selected policy is:

- Project and lexical name resolution precede environment records and may
  non-callably shadow them.
- Standard and Adapter records with different schemas coexist as overloads.
- Argument viability and type specificity are compared before authority.
- On an otherwise equal Standard/Adapter tie, Standard wins.
- Exact semantic Standard/Adapter duplicates are coalesced before checking, so
  they cannot be ambiguous.
- Different providers at the same authority rank and key are rejected at world
  construction, not deferred to a query.
- Multiple overloads within one provider are allowed; an otherwise equal viable
  tie is `AmbiguousOverload`.
- Reversed insertion order produces the same typed order and result.

This preserves standard behavior while allowing an adapter-only callable or a
more-specific adapter overload to participate. Adapter metadata never silently
overwrites a standard record.

## 33. Deletion rule and no dual resolver

For each migration cut, the implementation first routes the family through
`resolve_call_target` and `check_resolved_call`, adds direct parity tests, and
uses a crate-owned test hook/counter to prove the old successful branch is not
entered. The old branch is deleted in that same cut before proceeding to the
next family. A dormant fallback, feature-gated fallback, extension trait,
deprecated API, wrapper, or signature-only copy is prohibited.

The final checker contains one call-dispatch entry and one selected-call entry
that both construct `CallResolverRequest`; family validators contain no name
lookup. Production acceptance is proved by public typed behavior and Cargo
metadata, never by tests that scan implementation source for symbol names.

## 34. Serialization and cache boundary

Callable catalogs, resolver candidates, target facts, function-value IDs,
semantic signatures, diagnostics, and work reports are session/accepted-world
memory objects. This contract adds no Serde implementation or persisted wire
format. Existing accepted-generation cache identity from AW-AH-009.3/.2
invalidates the entire catalog and target-fact result on project revision,
manifest, Rust metadata, character facts, profile, or accepted source change.

Display labels, documentation, source spans, equivalent provenance, and Rust
paths may be cached only under that accepted generation. A failed rebuild does
not pair a new catalog or manifest digest with the old accepted world.

## 35. Public visibility summary

Public from `arcweft-lang-sema::callable`:

- validated scalar/path/key types and read-only accessors;
- all candidate IDs and family enums;
- documentation/provenance/source types;
- schema and immutable record types;
- read-only `RegisteredCallableCatalog` access;
- resolved callable/function-value/target fact/result types;
- diagnostics, limits constant/accessors, and typed errors;
- `SemanticSignature` and `SemanticSignatureHelp` validating constructors.

`pub(crate)` only:

- catalog builders and mutable publication transaction internals;
- lexical scope mutation;
- function-value/local ID allocation;
- `CallResolverRequest::try_new`;
- `resolve_call_target`, `check_resolved_call`, candidate checkpoints, and fact
  recorder;
- test-only corrupt builders and small-limit constructors.

Public adapter-context API:

- `AdapterCallableName`, `AdapterCallablePath`, typed overload/group/parameter
  indices, typed parameter groups/passing/presence, and typed tooling subjects;
- the direct typed manifest builder APIs;
- under the existing `sema` feature, `AdapterManifest::try_callable_publication`
  and its typed errors;
- no dotted-string callable constructor and no direct callable mutation of
  `TypeCheckEnv`.

Public HIR API:

- immutable `HirCallableSignatureSource` access and `HirProject` iterators;
- no sema candidate type and no source `impl` method synthesis.

## 36. Final invariants

Implementation is accepted only when all are true:

1. every successful call has one primary typed candidate ID;
2. candidate sets and public signature sets are non-empty by construction;
3. checker and signature help expose the same primary candidate ID;
4. arguments are inferred once in the selected transaction;
5. no family validator re-resolves a name or method;
6. all current free and selected families in `SURFACE_INVENTORY.md` are migrated;
7. exact Standard/Adapter duplicates coalesce and same-rank collisions fail;
8. project non-callable bindings shadow environment callables;
9. `show.look` and dialogue `look` use structural character identity;
10. unknown owner/part recovery is typed and never guesses;
11. accepted-world failures are atomic and query failures are non-cacheable;
12. exact limits succeed and one-over fails before partial publication;
13. no label/source/Rust display string is parsed into identity;
14. no old successful branch, signature-only resolver, extension trait,
    compatibility wrapper, deprecated API, source gate, or source `impl` method
    synthesis remains.
