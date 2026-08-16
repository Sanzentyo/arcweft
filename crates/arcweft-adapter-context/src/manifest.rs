//! Typed adapter manifest model shared by product adapters, CLI, LSP, and semantic checking.

use arcweft_rust_abi::{
    ArcweftRustAbiLimits, ArcweftRustFunction, ArcweftRustManifest,
    ArcweftRustOpaqueTypeProducerId, ArcweftRustPackage, ArcweftRustPackageId, ArcweftRustParam,
    ArcweftRustPurity, ArcweftRustStructShape, ArcweftRustTypeDecl, ArcweftRustTypeKind,
    ArcweftRustTypeRef, ArcweftRustVariantPayload,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
mod nominal;
mod registry;

pub use crate::callable::{
    AdapterCallableGroupIndex, AdapterCallableModelError, AdapterCallableName,
    AdapterCallableOverloadIndex, AdapterCallableParameterIndex, AdapterCallablePath,
    AdapterFreeCallableKind, AdapterFunctionParam, AdapterFunctionSignature, AdapterParameterGroup,
    AdapterParameterPassing, AdapterParameterPresence, AdapterToolingDoc,
    AdapterToolingParameterDoc, AdapterToolingSubject,
};
pub use crate::symbol::{AdapterSymbolPath, AdapterSymbolPathError, AdapterSymbolSegment};
pub use arcweft_manifest_model::{AdapterOpaqueTypeProducerId, AdapterOpaqueTypeProducerIdError};
pub use nominal::{
    AdapterEnvironmentOwnerId, AdapterNominalDeclaration, AdapterNominalOwner, AdapterNominalPath,
    AdapterNominalPathError, AdapterNominalPathPrefix, AdapterNominalPathSegment,
    AdapterNominalTypeRef, AdapterNominalVisibility, AdapterRustPackageMountTable,
    AdapterTypeModelError,
};
pub use registry::{AdapterRegistry, AdapterRegistryError};

/// Stable adapter identifier used by launch profiles and tooling.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterId(String);

/// Language-free type shape used by adapter manifests.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AdapterTypeKind {
    /// Unit value.
    Unit,
    /// Boolean value.
    Bool,
    /// Signed 8-bit integer.
    I8,
    /// Signed 16-bit integer.
    I16,
    /// Signed 32-bit integer.
    I32,
    /// Signed 64-bit integer.
    I64,
    /// Signed 128-bit integer.
    I128,
    /// Pointer-sized signed integer.
    ISize,
    /// Unsigned 8-bit integer.
    U8,
    /// Unsigned 16-bit integer.
    U16,
    /// Unsigned 32-bit integer.
    U32,
    /// Unsigned 64-bit integer.
    U64,
    /// Unsigned 128-bit integer.
    U128,
    /// Pointer-sized unsigned integer.
    USize,
    /// 32-bit floating point value.
    F32,
    /// 64-bit floating point value.
    F64,
    /// UTF-8 string value.
    String,
    /// Unicode scalar value.
    Char,
    /// Owned vector.
    Vec { item: Box<AdapterTypeKind> },
    /// Deterministic sequence.
    Seq { item: Box<AdapterTypeKind> },
    /// Optional value.
    Option { item: Box<AdapterTypeKind> },
    /// Fallible value.
    Result {
        /// Success payload.
        ok: Box<AdapterTypeKind>,
        /// Error payload.
        error: Box<AdapterTypeKind>,
    },
    /// Tuple value.
    Tuple { items: Box<[AdapterTypeKind]> },
    /// Incremental asynchronous value used by the existing await contract.
    Need {
        /// Ready payload.
        ready: Box<AdapterTypeKind>,
        /// Failure payload.
        error: Box<AdapterTypeKind>,
    },
    /// Exact adapter or Rust-package nominal type.
    Nominal { nominal: AdapterNominalTypeRef },
}

/// A symbol injected by a host adapter into the checked source environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterSymbol {
    path: AdapterSymbolPath,
    ty: AdapterTypeKind,
}

/// A method injected by a host adapter for a receiver type it contributes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterMethod {
    receiver: AdapterTypeKind,
    name: AdapterCallableName,
    overload: AdapterCallableOverloadIndex,
    signature: AdapterFunctionSignature,
    effects: Vec<AdapterEffectCapability>,
}

/// A free function injected by a host adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFunction {
    path: AdapterCallablePath,
    overload: AdapterCallableOverloadIndex,
    signature: AdapterFunctionSignature,
    effects: Vec<AdapterEffectCapability>,
}

/// Effect capability granted or required by an adapter surface.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterEffectCapability {
    id: String,
}

/// Runtime host call exported by an adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterHostCall {
    id: AdapterHostCallId,
    signature: AdapterFunctionSignature,
    domain_error: Option<AdapterTypeKind>,
    effects: Vec<AdapterEffectCapability>,
}

/// Stable runtime host-call identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterHostCallId(String);

/// A Rust function export injected into type checking and LSP tooling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterRustFunction {
    package: ArcweftRustPackage,
    path: AdapterCallablePath,
    overload: AdapterCallableOverloadIndex,
    rust_path: String,
    signature: AdapterFunctionSignature,
    purity: ArcweftRustPurity,
    effects: Vec<AdapterEffectCapability>,
}

/// A Rust ADT export injected into tooling metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterRustType {
    package: ArcweftRustPackage,
    accepted_path: AdapterNominalPath,
    decl: ArcweftRustTypeDecl,
}

/// Complete typed facts supplied by one host adapter profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterManifest {
    id: AdapterId,
    display_name: String,
    nominal_declarations: Vec<AdapterNominalDeclaration>,
    rust_package_mounts: AdapterRustPackageMountTable,
    rust_packages: BTreeMap<ArcweftRustPackageId, ArcweftRustPackage>,
    symbols: Vec<AdapterSymbol>,
    methods: Vec<AdapterMethod>,
    functions: Vec<AdapterFunction>,
    effects: Vec<AdapterEffectCapability>,
    host_calls: Vec<AdapterHostCall>,
    rust_functions: Vec<AdapterRustFunction>,
    rust_types: Vec<AdapterRustType>,
    tooling_docs: Vec<AdapterToolingDoc>,
}

/// Invalid cross-package or nominal facts in one adapter manifest.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdapterManifestModelError {
    #[error(transparent)]
    Callable(#[from] AdapterCallableModelError),
    #[error(transparent)]
    NominalPath(#[from] AdapterNominalPathError),
    #[error(transparent)]
    Type(#[from] AdapterTypeModelError),
    #[error("Rust ABI manifest is invalid: {0}")]
    RustManifest(arcweft_rust_abi::ArcweftRustManifestError),
    #[error("Rust package `{package}` has no adapter nominal mount")]
    MissingRustPackageMount { package: ArcweftRustPackageId },
    #[error("Rust package `{package}` already has an adapter nominal mount")]
    DuplicateRustPackageMount { package: ArcweftRustPackageId },
    #[error("Rust package `{package}` was ingested more than once")]
    DuplicateRustPackageManifest { package: ArcweftRustPackageId },
    #[error("duplicate adapter nominal declaration at `{path}`")]
    DuplicateNominalDeclaration { path: AdapterNominalPath },
    #[error("Rust callable contains declaration-local type parameter {index}")]
    CallableTypeParameter { index: usize },
}

impl AdapterId {
    /// Creates an adapter id.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// String form used in launch profile manifests.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AdapterId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AdapterTypeKind {
    /// Returns a primitive adapter type for a canonical label.
    pub fn primitive_name(name: &str) -> Option<Self> {
        match name {
            "Unit" => Some(Self::Unit),
            "bool" => Some(Self::Bool),
            "i8" => Some(Self::I8),
            "i16" => Some(Self::I16),
            "i32" => Some(Self::I32),
            "i64" => Some(Self::I64),
            "i128" => Some(Self::I128),
            "isize" => Some(Self::ISize),
            "u8" => Some(Self::U8),
            "u16" => Some(Self::U16),
            "u32" => Some(Self::U32),
            "u64" => Some(Self::U64),
            "u128" => Some(Self::U128),
            "usize" => Some(Self::USize),
            "f32" => Some(Self::F32),
            "f64" => Some(Self::F64),
            "String" => Some(Self::String),
            "char" => Some(Self::Char),
            _ => None,
        }
    }
}

impl AdapterSymbol {
    /// Creates a typed adapter symbol.
    pub fn new(path: AdapterSymbolPath, ty: AdapterTypeKind) -> Self {
        Self { path, ty }
    }

    /// Source-visible typed symbol path.
    pub const fn path(&self) -> &AdapterSymbolPath {
        &self.path
    }

    /// Symbol type visible to semantic checking.
    pub const fn ty(&self) -> &AdapterTypeKind {
        &self.ty
    }
}

impl AdapterMethod {
    /// Receiver type this method is attached to.
    pub const fn receiver(&self) -> &AdapterTypeKind {
        &self.receiver
    }

    /// Arcweft-visible method name.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Typed Arcweft-visible method name.
    pub const fn callable_name(&self) -> &AdapterCallableName {
        &self.name
    }

    /// Typed overload position within this provider and method key.
    pub const fn overload(&self) -> AdapterCallableOverloadIndex {
        self.overload
    }

    /// Method signature.
    pub const fn signature(&self) -> &AdapterFunctionSignature {
        &self.signature
    }

    /// Effects required when calling the method.
    pub fn effects(&self) -> &[AdapterEffectCapability] {
        &self.effects
    }
}

impl AdapterFunction {
    /// Arcweft-visible typed function path.
    pub const fn path(&self) -> &AdapterCallablePath {
        &self.path
    }

    /// Typed overload position within this provider and function path.
    pub const fn overload(&self) -> AdapterCallableOverloadIndex {
        self.overload
    }

    /// Full callable signature.
    pub const fn signature(&self) -> &AdapterFunctionSignature {
        &self.signature
    }

    /// Effects required when calling the function.
    pub fn effects(&self) -> &[AdapterEffectCapability] {
        &self.effects
    }
}

impl AdapterEffectCapability {
    /// Creates an effect capability.
    pub fn new(name: impl Into<String>) -> Self {
        Self { id: name.into() }
    }

    /// Canonical capability label.
    pub fn as_str(&self) -> &str {
        self.id.as_str()
    }

    /// Canonical capability label.
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl AdapterHostCall {
    /// Creates a host call with required effects.
    pub fn new(
        id: impl Into<String>,
        effects: impl IntoIterator<Item = AdapterEffectCapability>,
    ) -> Self {
        Self {
            id: AdapterHostCallId::new(id),
            signature: empty_adapter_signature(AdapterTypeKind::Unit),
            domain_error: None,
            effects: effects.into_iter().collect(),
        }
    }

    /// Creates a host call with explicit ABI parameter and result types.
    pub fn with_signature(
        id: impl Into<String>,
        signature: AdapterFunctionSignature,
        effects: impl IntoIterator<Item = AdapterEffectCapability>,
    ) -> Self {
        Self {
            id: AdapterHostCallId::new(id),
            signature,
            domain_error: None,
            effects: effects.into_iter().collect(),
        }
    }

    /// Attaches the typed domain error produced by this host call.
    #[must_use]
    pub fn with_domain_error(mut self, domain_error: AdapterTypeKind) -> Self {
        self.domain_error = Some(domain_error);
        self
    }

    /// Stable runtime host-call id.
    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    /// Effects touched by this host call.
    pub fn effects(&self) -> &[AdapterEffectCapability] {
        &self.effects
    }

    /// Runtime ABI signature used by host adapters to decode payloads.
    pub const fn signature(&self) -> &AdapterFunctionSignature {
        &self.signature
    }

    /// Typed domain error carried by the host call, when one exists.
    pub const fn domain_error(&self) -> Option<&AdapterTypeKind> {
        self.domain_error.as_ref()
    }
}

impl AdapterHostCallId {
    /// Creates a host-call id.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// String form used by runtime task requests.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AdapterRustFunction {
    /// Rust adapter package that exported this function.
    pub const fn package(&self) -> &ArcweftRustPackage {
        &self.package
    }

    /// Arcweft-visible function path.
    pub const fn path(&self) -> &AdapterCallablePath {
        &self.path
    }

    /// Typed overload position within this provider and function path.
    pub const fn overload(&self) -> AdapterCallableOverloadIndex {
        self.overload
    }

    /// Rust path recorded in the adapter metadata.
    pub fn rust_path(&self) -> &str {
        &self.rust_path
    }

    /// Full callable signature.
    pub const fn signature(&self) -> &AdapterFunctionSignature {
        &self.signature
    }

    /// Purity class preserved from Rust ABI metadata.
    pub const fn purity(&self) -> ArcweftRustPurity {
        self.purity
    }

    /// Effects declared by Rust ABI metadata.
    pub fn effects(&self) -> &[AdapterEffectCapability] {
        &self.effects
    }
}

impl AdapterRustType {
    /// Rust adapter package that exported this type.
    pub const fn package(&self) -> &ArcweftRustPackage {
        &self.package
    }

    /// Exact source-visible path after applying the package mount.
    pub const fn accepted_path(&self) -> &AdapterNominalPath {
        &self.accepted_path
    }

    /// Exported Rust ADT declaration.
    pub const fn decl(&self) -> &ArcweftRustTypeDecl {
        &self.decl
    }

    /// Reviewed opaque producer authority retained by the Rust declaration.
    pub const fn opaque_producer(&self) -> &ArcweftRustOpaqueTypeProducerId {
        self.decl.opaque_producer()
    }
}

impl AdapterManifest {
    /// Creates an empty adapter manifest.
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: AdapterId::new(id),
            display_name: display_name.into(),
            nominal_declarations: Vec::new(),
            rust_package_mounts: AdapterRustPackageMountTable::default(),
            rust_packages: BTreeMap::new(),
            symbols: Vec::new(),
            methods: Vec::new(),
            functions: Vec::new(),
            effects: Vec::new(),
            host_calls: Vec::new(),
            rust_functions: Vec::new(),
            rust_types: Vec::new(),
            tooling_docs: Vec::new(),
        }
    }

    /// Adapter id used by profile manifests.
    pub const fn id(&self) -> &AdapterId {
        &self.id
    }

    /// Human-readable adapter name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Adds one injected symbol.
    #[must_use]
    pub fn with_symbol(mut self, symbol: AdapterSymbol) -> Self {
        self.symbols.push(symbol);
        self
    }

    /// Adds one injected method with full parameter signature.
    #[must_use]
    pub fn with_method_signature(
        mut self,
        receiver: AdapterTypeKind,
        name: AdapterCallableName,
        overload: AdapterCallableOverloadIndex,
        signature: AdapterFunctionSignature,
        effects: impl IntoIterator<Item = AdapterEffectCapability>,
    ) -> Self {
        self.methods.push(AdapterMethod {
            receiver,
            name,
            overload,
            signature,
            effects: effects.into_iter().collect(),
        });
        self
    }

    /// Adds one injected free function with typed effects.
    #[must_use]
    pub fn with_function_signature(
        mut self,
        path: AdapterCallablePath,
        overload: AdapterCallableOverloadIndex,
        signature: AdapterFunctionSignature,
        effects: impl IntoIterator<Item = AdapterEffectCapability>,
    ) -> Self {
        self.functions.push(AdapterFunction {
            path,
            overload,
            signature,
            effects: effects.into_iter().collect(),
        });
        self
    }

    /// Grants one effect capability to this adapter environment.
    #[must_use]
    pub fn with_effect(mut self, capability: AdapterEffectCapability) -> Self {
        self.effects.push(capability);
        self
    }

    /// Adds one runtime host-call surface.
    #[must_use]
    pub fn with_host_call(mut self, host_call: AdapterHostCall) -> Self {
        self.host_calls.push(host_call);
        self
    }

    /// Adds one tooling documentation entry.
    #[must_use]
    pub fn with_tooling_doc(mut self, doc: AdapterToolingDoc) -> Self {
        self.tooling_docs.push(doc);
        self
    }

    /// Adds one exact Rust package mount before its metadata is ingested.
    pub fn try_with_rust_package_mount(
        mut self,
        package: ArcweftRustPackageId,
        prefix: AdapterNominalPathPrefix,
    ) -> Result<Self, AdapterManifestModelError> {
        if self.rust_package_mounts.get(&package).is_some() {
            return Err(AdapterManifestModelError::DuplicateRustPackageMount { package });
        }
        self.rust_package_mounts.insert(package, prefix);
        Ok(self)
    }

    /// Adds one adapter-native nominal declaration owned by this manifest.
    pub fn try_with_nominal_declaration(
        mut self,
        declaration: AdapterNominalDeclaration,
    ) -> Result<Self, AdapterManifestModelError> {
        if self
            .nominal_declarations
            .iter()
            .any(|current| current.path() == declaration.path())
        {
            return Err(AdapterManifestModelError::DuplicateNominalDeclaration {
                path: declaration.path().clone(),
            });
        }
        self.nominal_declarations.push(declaration);
        Ok(self)
    }

    /// Adds validated Rust ABI metadata exported by an adapter crate.
    pub fn try_with_rust_manifest(
        mut self,
        manifest: &ArcweftRustManifest,
    ) -> Result<Self, AdapterManifestModelError> {
        manifest
            .validate(ArcweftRustAbiLimits::PRODUCTION)
            .map_err(AdapterManifestModelError::RustManifest)?;
        let package = manifest.package.clone();
        let package_id = package.id.clone();
        let prefix = require_package_mount(&self.rust_package_mounts, &package_id)?.clone();
        if self.rust_packages.contains_key(&package_id) {
            return Err(AdapterManifestModelError::DuplicateRustPackageManifest {
                package: package_id,
            });
        }
        for declaration in &manifest.types {
            require_type_decl_mounts(&self.rust_package_mounts, declaration)?;
        }
        for function in &manifest.functions {
            for parameter in &function.params {
                require_type_ref_mounts(&self.rust_package_mounts, &parameter.ty)?;
            }
            require_type_ref_mounts(&self.rust_package_mounts, &function.return_type)?;
        }
        for decl in manifest.types.iter().cloned() {
            let accepted_path = prefix.join(&decl.path)?;
            self.rust_types.push(AdapterRustType {
                package: package.clone(),
                accepted_path,
                decl,
            });
        }
        for (overload, function) in manifest.functions.iter().enumerate() {
            self.rust_functions.push(adapter_rust_function(
                package.clone(),
                &self.rust_package_mounts,
                overload,
                function,
            )?);
        }
        self.rust_packages.insert(package_id, package);
        Ok(self)
    }

    /// Injected symbols, preserved for tooling and diagnostics.
    pub fn symbols(&self) -> &[AdapterSymbol] {
        &self.symbols
    }

    /// Adapter-native nominal declarations in authored order.
    pub fn nominal_declarations(&self) -> &[AdapterNominalDeclaration] {
        &self.nominal_declarations
    }

    /// Exact Rust package mounts accepted before metadata ingestion.
    pub const fn rust_package_mounts(&self) -> &AdapterRustPackageMountTable {
        &self.rust_package_mounts
    }

    /// Exact Rust package provenance accepted by this manifest.
    pub fn rust_packages(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ArcweftRustPackageId, &ArcweftRustPackage)> {
        self.rust_packages.iter()
    }

    /// Injected methods, preserved for tooling and diagnostics.
    pub fn methods(&self) -> &[AdapterMethod] {
        &self.methods
    }

    /// Injected free functions.
    pub fn functions(&self) -> &[AdapterFunction] {
        &self.functions
    }

    /// Adapter-granted effect capabilities.
    pub fn effects(&self) -> &[AdapterEffectCapability] {
        &self.effects
    }

    /// Runtime host calls exported by this adapter.
    pub fn host_calls(&self) -> &[AdapterHostCall] {
        &self.host_calls
    }

    /// Returns whether this manifest exports a runtime host call.
    pub fn has_host_call(&self, id: &AdapterHostCallId) -> bool {
        self.host_calls.iter().any(|host_call| &host_call.id == id)
    }

    /// Returns whether this manifest grants an effect capability.
    pub fn has_effect(&self, capability: &AdapterEffectCapability) -> bool {
        self.effects.iter().any(|effect| effect == capability)
    }

    /// Rust functions exported by adapter metadata.
    pub fn rust_functions(&self) -> &[AdapterRustFunction] {
        &self.rust_functions
    }

    /// Rust types exported by adapter metadata.
    pub fn rust_types(&self) -> &[AdapterRustType] {
        &self.rust_types
    }

    /// Tooling documentation entries.
    pub fn tooling_docs(&self) -> &[AdapterToolingDoc] {
        &self.tooling_docs
    }
}

fn adapter_rust_function(
    package: ArcweftRustPackage,
    mounts: &AdapterRustPackageMountTable,
    overload: usize,
    function: &ArcweftRustFunction,
) -> Result<AdapterRustFunction, AdapterManifestModelError> {
    Ok(AdapterRustFunction {
        package,
        path: AdapterCallablePath::single(AdapterCallableName::try_new(function.name.clone())?),
        overload: AdapterCallableOverloadIndex::try_from_usize(overload)?,
        rust_path: function.rust_path.clone(),
        signature: adapter_function_signature(
            function
                .params
                .iter()
                .enumerate()
                .map(|(index, parameter)| adapter_function_param(mounts, index, parameter))
                .collect::<Result<Vec<_>, _>>()?,
            rust_type_ref_to_adapter_type_kind(mounts, &function.return_type)?,
        )?,
        purity: function.purity,
        effects: function
            .effects
            .iter()
            .cloned()
            .map(AdapterEffectCapability::new)
            .collect(),
    })
}

fn adapter_function_param(
    mounts: &AdapterRustPackageMountTable,
    index: usize,
    parameter: &ArcweftRustParam,
) -> Result<AdapterFunctionParam, AdapterManifestModelError> {
    Ok(AdapterFunctionParam::try_new(
        AdapterCallableParameterIndex::try_from_usize(index)?,
        Some(AdapterCallableName::try_new(parameter.name.clone())?),
        rust_type_ref_to_adapter_type_kind(mounts, &parameter.ty)?,
        AdapterParameterPassing::PositionalOrNamed,
        AdapterParameterPresence::Required,
    )?)
}

fn adapter_function_signature(
    parameters: Vec<AdapterFunctionParam>,
    return_type: AdapterTypeKind,
) -> Result<AdapterFunctionSignature, AdapterCallableModelError> {
    let group =
        AdapterParameterGroup::try_new(AdapterCallableGroupIndex::try_from_usize(0)?, parameters)?;
    AdapterFunctionSignature::try_new(vec![group], return_type)
}

fn empty_adapter_signature(return_type: AdapterTypeKind) -> AdapterFunctionSignature {
    adapter_function_signature(Vec::new(), return_type)
        .expect("empty initial adapter signature is structurally valid")
}

fn rust_type_ref_to_adapter_type_kind(
    mounts: &AdapterRustPackageMountTable,
    ty: &ArcweftRustTypeRef,
) -> Result<AdapterTypeKind, AdapterManifestModelError> {
    Ok(match ty {
        ArcweftRustTypeRef::Unit => AdapterTypeKind::Unit,
        ArcweftRustTypeRef::Bool => AdapterTypeKind::Bool,
        ArcweftRustTypeRef::I8 => AdapterTypeKind::I8,
        ArcweftRustTypeRef::I16 => AdapterTypeKind::I16,
        ArcweftRustTypeRef::I32 => AdapterTypeKind::I32,
        ArcweftRustTypeRef::I64 => AdapterTypeKind::I64,
        ArcweftRustTypeRef::I128 => AdapterTypeKind::I128,
        ArcweftRustTypeRef::ISize => AdapterTypeKind::ISize,
        ArcweftRustTypeRef::U8 => AdapterTypeKind::U8,
        ArcweftRustTypeRef::U16 => AdapterTypeKind::U16,
        ArcweftRustTypeRef::U32 => AdapterTypeKind::U32,
        ArcweftRustTypeRef::U64 => AdapterTypeKind::U64,
        ArcweftRustTypeRef::U128 => AdapterTypeKind::U128,
        ArcweftRustTypeRef::USize => AdapterTypeKind::USize,
        ArcweftRustTypeRef::F32 => AdapterTypeKind::F32,
        ArcweftRustTypeRef::F64 => AdapterTypeKind::F64,
        ArcweftRustTypeRef::String => AdapterTypeKind::String,
        ArcweftRustTypeRef::Char => AdapterTypeKind::Char,
        ArcweftRustTypeRef::Vec { item } => AdapterTypeKind::Vec {
            item: Box::new(rust_type_ref_to_adapter_type_kind(mounts, item)?),
        },
        ArcweftRustTypeRef::Seq { item } => AdapterTypeKind::Seq {
            item: Box::new(rust_type_ref_to_adapter_type_kind(mounts, item)?),
        },
        ArcweftRustTypeRef::Option { item } => AdapterTypeKind::Option {
            item: Box::new(rust_type_ref_to_adapter_type_kind(mounts, item)?),
        },
        ArcweftRustTypeRef::Result { ok, error } => AdapterTypeKind::Result {
            ok: Box::new(rust_type_ref_to_adapter_type_kind(mounts, ok)?),
            error: Box::new(rust_type_ref_to_adapter_type_kind(mounts, error)?),
        },
        ArcweftRustTypeRef::Tuple { items } => AdapterTypeKind::Tuple {
            items: items
                .iter()
                .map(|item| rust_type_ref_to_adapter_type_kind(mounts, item))
                .collect::<Result<Box<[_]>, _>>()?,
        },
        ArcweftRustTypeRef::Nominal {
            package,
            path,
            arguments,
        } => {
            let prefix = require_package_mount(mounts, package)?;
            let path = prefix.join(path)?;
            let arguments = arguments
                .iter()
                .map(|argument| rust_type_ref_to_adapter_type_kind(mounts, argument))
                .collect::<Result<Vec<_>, _>>()?;
            AdapterTypeKind::Nominal {
                nominal: AdapterNominalTypeRef::try_new(
                    AdapterNominalOwner::RustPackage {
                        package: package.clone(),
                    },
                    path,
                    arguments,
                )?,
            }
        }
        ArcweftRustTypeRef::TypeParameter { index } => {
            return Err(AdapterManifestModelError::CallableTypeParameter { index: index.get() });
        }
    })
}

fn require_package_mount<'a>(
    mounts: &'a AdapterRustPackageMountTable,
    package: &ArcweftRustPackageId,
) -> Result<&'a AdapterNominalPathPrefix, AdapterManifestModelError> {
    mounts
        .get(package)
        .ok_or_else(|| AdapterManifestModelError::MissingRustPackageMount {
            package: package.clone(),
        })
}

fn require_type_decl_mounts(
    mounts: &AdapterRustPackageMountTable,
    declaration: &ArcweftRustTypeDecl,
) -> Result<(), AdapterManifestModelError> {
    match &declaration.kind {
        ArcweftRustTypeKind::Struct { shape } => match shape {
            ArcweftRustStructShape::Unit => Ok(()),
            ArcweftRustStructShape::Tuple { fields } => require_type_refs_mounts(mounts, fields),
            ArcweftRustStructShape::Record { fields } => fields
                .iter()
                .try_for_each(|field| require_type_ref_mounts(mounts, &field.ty)),
        },
        ArcweftRustTypeKind::Enum { variants } => {
            variants
                .iter()
                .try_for_each(|variant| match &variant.payload {
                    ArcweftRustVariantPayload::Unit => Ok(()),
                    ArcweftRustVariantPayload::Tuple { fields } => {
                        require_type_refs_mounts(mounts, fields)
                    }
                    ArcweftRustVariantPayload::Record { fields } => fields
                        .iter()
                        .try_for_each(|field| require_type_ref_mounts(mounts, &field.ty)),
                })
        }
        ArcweftRustTypeKind::Newtype { inner } => require_type_ref_mounts(mounts, inner),
    }
}

fn require_type_refs_mounts(
    mounts: &AdapterRustPackageMountTable,
    types: &[ArcweftRustTypeRef],
) -> Result<(), AdapterManifestModelError> {
    types
        .iter()
        .try_for_each(|ty| require_type_ref_mounts(mounts, ty))
}

fn require_type_ref_mounts(
    mounts: &AdapterRustPackageMountTable,
    ty: &ArcweftRustTypeRef,
) -> Result<(), AdapterManifestModelError> {
    match ty {
        ArcweftRustTypeRef::Vec { item }
        | ArcweftRustTypeRef::Seq { item }
        | ArcweftRustTypeRef::Option { item } => require_type_ref_mounts(mounts, item),
        ArcweftRustTypeRef::Result { ok, error } => {
            require_type_ref_mounts(mounts, ok)?;
            require_type_ref_mounts(mounts, error)
        }
        ArcweftRustTypeRef::Tuple { items } => require_type_refs_mounts(mounts, items),
        ArcweftRustTypeRef::Nominal {
            package, arguments, ..
        } => {
            require_package_mount(mounts, package)?;
            require_type_refs_mounts(mounts, arguments)
        }
        ArcweftRustTypeRef::Unit
        | ArcweftRustTypeRef::Bool
        | ArcweftRustTypeRef::I8
        | ArcweftRustTypeRef::I16
        | ArcweftRustTypeRef::I32
        | ArcweftRustTypeRef::I64
        | ArcweftRustTypeRef::I128
        | ArcweftRustTypeRef::ISize
        | ArcweftRustTypeRef::U8
        | ArcweftRustTypeRef::U16
        | ArcweftRustTypeRef::U32
        | ArcweftRustTypeRef::U64
        | ArcweftRustTypeRef::U128
        | ArcweftRustTypeRef::USize
        | ArcweftRustTypeRef::F32
        | ArcweftRustTypeRef::F64
        | ArcweftRustTypeRef::String
        | ArcweftRustTypeRef::Char
        | ArcweftRustTypeRef::TypeParameter { .. } => Ok(()),
    }
}

#[cfg(test)]
#[path = "manifest/tests.rs"]
mod tests;
