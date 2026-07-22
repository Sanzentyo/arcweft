//! Typed adapter manifest model shared by product adapters, CLI, LSP, and semantic checking.

#[cfg(feature = "sema")]
use arcweft_lang_sema::env::{EffectCapability, TypeCheckEnv, nominal::RustPackageId};
#[cfg(feature = "sema")]
use arcweft_lang_sema::types::TypeKind;
#[cfg(feature = "sema")]
use arcweft_lang_syntax::{
    ast::{
        module_path::ModulePathRoot,
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment},
    },
    types::TypePath,
};
#[cfg(feature = "sema")]
use arcweft_rust_abi::ArcweftRustTypeKind;
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustParam,
    ArcweftRustPurity, ArcweftRustTypeDecl, ArcweftRustTypeRef,
};
#[cfg(feature = "sema")]
mod registration;
mod registry;

pub use crate::callable::{
    AdapterCallableGroupIndex, AdapterCallableModelError, AdapterCallableName,
    AdapterCallableOverloadIndex, AdapterCallableParameterIndex, AdapterCallablePath,
    AdapterFreeCallableKind, AdapterFunctionParam, AdapterFunctionSignature, AdapterParameterGroup,
    AdapterParameterPassing, AdapterParameterPresence, AdapterToolingDoc,
    AdapterToolingParameterDoc, AdapterToolingSubject,
};
pub use crate::symbol::{AdapterSymbolPath, AdapterSymbolPathError, AdapterSymbolSegment};
#[cfg(feature = "sema")]
pub use registration::{AdapterRegistrationFactsError, SourceBackedAdapterRegistrationFacts};
pub use registry::{AdapterRegistry, AdapterRegistryError};

/// Stable adapter identifier used by launch profiles and tooling.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterId(String);

/// Language-free type shape used by adapter manifests.
#[derive(Clone, Debug, Eq, PartialEq)]
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
    Vec(Box<AdapterTypeKind>),
    /// Deterministic sequence.
    Seq(Box<AdapterTypeKind>),
    /// Optional value.
    Option(Box<AdapterTypeKind>),
    /// Fallible value.
    Result {
        /// Success payload.
        ok: Box<AdapterTypeKind>,
        /// Error payload.
        error: Box<AdapterTypeKind>,
    },
    /// Tuple value.
    Tuple(Vec<AdapterTypeKind>),
    /// Incremental need value.
    Need {
        /// Ready payload.
        ready: Box<AdapterTypeKind>,
        /// Error payload.
        error: Box<AdapterTypeKind>,
    },
    /// Adapter-defined nominal type.
    Named(String),
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
    package: String,
    decl: ArcweftRustTypeDecl,
}

/// Complete typed facts supplied by one host adapter profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterManifest {
    id: AdapterId,
    display_name: String,
    symbols: Vec<AdapterSymbol>,
    methods: Vec<AdapterMethod>,
    functions: Vec<AdapterFunction>,
    effects: Vec<AdapterEffectCapability>,
    host_calls: Vec<AdapterHostCall>,
    rust_functions: Vec<AdapterRustFunction>,
    rust_types: Vec<AdapterRustType>,
    tooling_docs: Vec<AdapterToolingDoc>,
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
            effects: effects.into_iter().collect(),
        }
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
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Exported Rust ADT declaration.
    pub const fn decl(&self) -> &ArcweftRustTypeDecl {
        &self.decl
    }
}

impl AdapterManifest {
    /// Creates an empty adapter manifest.
    pub fn new(id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            id: AdapterId::new(id),
            display_name: display_name.into(),
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

    /// Adds Rust ABI metadata exported by an adapter crate.
    pub fn try_with_rust_manifest(
        mut self,
        manifest: &ArcweftRustManifest,
    ) -> Result<Self, AdapterCallableModelError> {
        let package = manifest.package.clone();
        self.rust_types
            .extend(manifest.types.iter().cloned().map(|decl| AdapterRustType {
                package: package.name.clone(),
                decl,
            }));
        for (overload, function) in manifest.functions.iter().enumerate() {
            self.rust_functions
                .push(adapter_rust_function(package.clone(), overload, function)?);
        }
        Ok(self)
    }

    /// Applies this adapter manifest to an existing checker environment.
    #[cfg(feature = "sema")]
    pub fn apply_to_env(&self, env: TypeCheckEnv) -> TypeCheckEnv {
        let env = self.symbols.iter().fold(env, |env, symbol| {
            env.with_symbol(symbol.path().to_string(), symbol.ty().to_sema_type_kind())
        });
        let env = self.effects.iter().fold(env, |env, effect| {
            env.with_capability(effect.to_sema_effect_capability())
        });
        self.rust_types.iter().fold(env, apply_rust_type_to_env)
    }

    /// Marks this manifest's effects as provided by the selected target.
    #[cfg(feature = "sema")]
    pub fn grant_effect_availability(&self, env: TypeCheckEnv) -> TypeCheckEnv {
        self.effects.iter().fold(env, |env, effect| {
            env.with_available_effect(effect.to_sema_effect_capability())
        })
    }

    /// Applies this manifest and marks its effects as target-provided.
    #[cfg(feature = "sema")]
    pub fn apply_to_target_env(&self, env: TypeCheckEnv) -> TypeCheckEnv {
        self.grant_effect_availability(self.apply_to_env(env))
    }

    /// Injected symbols, preserved for tooling and diagnostics.
    pub fn symbols(&self) -> &[AdapterSymbol] {
        &self.symbols
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
    overload: usize,
    function: &ArcweftRustFunction,
) -> Result<AdapterRustFunction, AdapterCallableModelError> {
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
                .map(|(index, parameter)| adapter_function_param(index, parameter))
                .collect::<Result<Vec<_>, _>>()?,
            rust_type_ref_to_adapter_type_kind(&function.return_type),
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
    index: usize,
    parameter: &ArcweftRustParam,
) -> Result<AdapterFunctionParam, AdapterCallableModelError> {
    AdapterFunctionParam::try_new(
        AdapterCallableParameterIndex::try_from_usize(index)?,
        Some(AdapterCallableName::try_new(parameter.name.clone())?),
        rust_type_ref_to_adapter_type_kind(&parameter.ty),
        AdapterParameterPassing::PositionalOrNamed,
        AdapterParameterPresence::Required,
    )
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

#[cfg(feature = "sema")]
fn apply_rust_type_to_env(env: TypeCheckEnv, ty: &AdapterRustType) -> TypeCheckEnv {
    let type_kind = TypeKind::Named(ty.decl.name.clone());
    let package = RustPackageId::try_new(ty.package.clone())
        .expect("ingested Rust manifests have valid package identifiers");
    let path = rust_type_path(&ty.decl.name);
    let env = env
        .try_with_rust_type_export(package, path)
        .expect("ingested Rust manifests have unique, valid type exports");
    match &ty.decl.kind {
        ArcweftRustTypeKind::Enum { variants } => env
            .try_with_enum_variants(
                type_kind,
                variants
                    .iter()
                    .filter(|variant| variant.fields.is_empty())
                    .map(|variant| variant.name.clone()),
            )
            .expect("Rust adapter enum types are ordinary nominal types"),
        ArcweftRustTypeKind::Struct { .. } | ArcweftRustTypeKind::Newtype { .. } => env,
    }
}

#[cfg(feature = "sema")]
fn rust_type_path(name: &str) -> TypePath {
    ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        [ProjectSymbolSegment::try_new(name)
            .expect("ingested Rust manifests have valid type identifiers")],
    )
    .expect("one valid segment forms a project-symbol path")
    .into()
}

fn rust_type_ref_to_adapter_type_kind(ty: &ArcweftRustTypeRef) -> AdapterTypeKind {
    match ty {
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
        ArcweftRustTypeRef::Vec { item } => {
            AdapterTypeKind::Vec(Box::new(rust_type_ref_to_adapter_type_kind(item)))
        }
        ArcweftRustTypeRef::Seq { item } => {
            AdapterTypeKind::Seq(Box::new(rust_type_ref_to_adapter_type_kind(item)))
        }
        ArcweftRustTypeRef::Option { item } => {
            AdapterTypeKind::Option(Box::new(rust_type_ref_to_adapter_type_kind(item)))
        }
        ArcweftRustTypeRef::Result { ok, error } => AdapterTypeKind::Result {
            ok: Box::new(rust_type_ref_to_adapter_type_kind(ok)),
            error: Box::new(rust_type_ref_to_adapter_type_kind(error)),
        },
        ArcweftRustTypeRef::Tuple { items } => AdapterTypeKind::Tuple(
            items
                .iter()
                .map(rust_type_ref_to_adapter_type_kind)
                .collect(),
        ),
        ArcweftRustTypeRef::Named { name } => AdapterTypeKind::primitive_name(name)
            .unwrap_or_else(|| AdapterTypeKind::Named(name.clone())),
    }
}

#[cfg(feature = "sema")]
impl AdapterTypeKind {
    /// Converts this adapter type shape into the semantic checker type model.
    pub fn to_sema_type_kind(&self) -> TypeKind {
        match self {
            Self::Unit => TypeKind::Unit,
            Self::Bool => TypeKind::Bool,
            Self::I8 => TypeKind::I8,
            Self::I16 => TypeKind::I16,
            Self::I32 => TypeKind::I32,
            Self::I64 => TypeKind::I64,
            Self::I128 => TypeKind::I128,
            Self::ISize => TypeKind::ISize,
            Self::U8 => TypeKind::U8,
            Self::U16 => TypeKind::U16,
            Self::U32 => TypeKind::U32,
            Self::U64 => TypeKind::U64,
            Self::U128 => TypeKind::U128,
            Self::USize => TypeKind::USize,
            Self::F32 => TypeKind::F32,
            Self::F64 => TypeKind::F64,
            Self::String => TypeKind::String,
            Self::Char => TypeKind::Char,
            Self::Vec(item) => TypeKind::Vec(Box::new(item.to_sema_type_kind())),
            Self::Seq(item) => TypeKind::Seq(Box::new(item.to_sema_type_kind())),
            Self::Option(item) => TypeKind::Option(Box::new(item.to_sema_type_kind())),
            Self::Result { ok, error } => TypeKind::Result {
                ok: Box::new(ok.to_sema_type_kind()),
                error: Box::new(error.to_sema_type_kind()),
            },
            Self::Tuple(items) => TypeKind::Tuple(
                items
                    .iter()
                    .map(AdapterTypeKind::to_sema_type_kind)
                    .collect(),
            ),
            Self::Need { ready, error } => TypeKind::Need {
                ready: Box::new(ready.to_sema_type_kind()),
                error: Box::new(error.to_sema_type_kind()),
            },
            Self::Named(name) => TypeKind::Named(name.clone()),
        }
    }
}

#[cfg(feature = "sema")]
impl AdapterEffectCapability {
    /// Converts this capability into the semantic checker capability model.
    pub fn to_sema_effect_capability(&self) -> EffectCapability {
        EffectCapability::new(self.id.clone())
    }
}

#[cfg(test)]
#[path = "manifest/tests.rs"]
mod tests;
