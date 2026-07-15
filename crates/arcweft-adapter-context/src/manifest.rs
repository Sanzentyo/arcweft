//! Typed adapter manifest model shared by product adapters, CLI, LSP, and semantic checking.

#[cfg(feature = "sema")]
use arcweft_lang_hir::symbol::{
    ExternalDeclarationSeed, ExternalDeclarationSeedError, ProjectDirectBinding,
};
#[cfg(feature = "sema")]
use arcweft_lang_sema::env::{EffectCapability, FunctionParam, FunctionSignature, TypeCheckEnv};
#[cfg(feature = "sema")]
use arcweft_lang_sema::registration::{
    EnvironmentBindingId, EnvironmentBindingIdError, ExternalRegistrationFact,
    RegisteredExternalOwner,
};
#[cfg(feature = "sema")]
use arcweft_lang_sema::types::TypeKind;
#[cfg(feature = "sema")]
use arcweft_lang_syntax::ast::{
    common::Visibility,
    module_path::{CanonicalModulePath, ModulePathRoot},
    symbol_path::{SymbolPath, SymbolPathError},
};
#[cfg(feature = "sema")]
use arcweft_rust_abi::ArcweftRustTypeKind;
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustParam, ArcweftRustTypeDecl,
    ArcweftRustTypeRef,
};
#[cfg(feature = "sema")]
use arcweft_source::{
    SourceDocument, SourceDocumentError, SourceDocumentId, SourceDocumentIdError, SourceName,
    SourceRange, SourceSpanError,
};
#[cfg(feature = "sema")]
use std::{fmt::Write as _, sync::Arc};
#[cfg(feature = "sema")]
use thiserror::Error;

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

/// One adapter callable parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFunctionParam {
    name: String,
    ty: AdapterTypeKind,
}

/// Adapter callable signature independent of Arcweft semantic internals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFunctionSignature {
    return_type: AdapterTypeKind,
    params: Vec<AdapterFunctionParam>,
}

/// A symbol injected by a host adapter into the checked source environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterSymbol {
    name: String,
    ty: AdapterTypeKind,
}

/// A method injected by a host adapter for a receiver type it contributes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterMethod {
    receiver: AdapterTypeKind,
    name: String,
    signature: AdapterFunctionSignature,
}

/// A free function injected by a host adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFunction {
    name: String,
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

/// Tooling-facing docs supplied by an adapter manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterToolingDoc {
    subject: String,
    docs: String,
}

/// A Rust function export injected into type checking and LSP tooling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterRustFunction {
    package: String,
    name: String,
    rust_path: String,
    signature: AdapterFunctionSignature,
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

/// One adapter's deterministic generated source and typed external contributions.
#[cfg(feature = "sema")]
#[derive(Clone, Debug)]
pub struct SourceBackedAdapterRegistrationFacts {
    document: Arc<SourceDocument>,
    externals: Vec<ExternalRegistrationFact>,
}

/// Failure while binding adapter facts to one generated source revision.
#[cfg(feature = "sema")]
#[derive(Debug, Error)]
pub enum AdapterRegistrationFactsError {
    #[error(transparent)]
    DocumentId(#[from] SourceDocumentIdError),
    #[error(transparent)]
    Document(#[from] SourceDocumentError),
    #[error(transparent)]
    Span(#[from] SourceSpanError),
    #[error(transparent)]
    SymbolPath(#[from] SymbolPathError),
    #[error(transparent)]
    ExternalDeclaration(#[from] ExternalDeclarationSeedError),
    #[error(transparent)]
    EnvironmentBinding(#[from] EnvironmentBindingIdError),
}

/// Collection used to resolve launch-profile adapter ids.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdapterRegistry {
    manifests: Vec<AdapterManifest>,
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

impl AdapterFunctionParam {
    /// Creates a required adapter function parameter.
    pub fn required(name: impl Into<String>, ty: AdapterTypeKind) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }

    /// Parameter name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Parameter type.
    pub const fn ty(&self) -> &AdapterTypeKind {
        &self.ty
    }
}

impl AdapterFunctionSignature {
    /// Creates a signature from a return type and ordered parameters.
    pub fn new(
        return_type: AdapterTypeKind,
        params: impl IntoIterator<Item = AdapterFunctionParam>,
    ) -> Self {
        Self {
            return_type,
            params: params.into_iter().collect(),
        }
    }

    /// Creates a signature without parameters.
    pub fn return_only(return_type: AdapterTypeKind) -> Self {
        Self::new(return_type, [])
    }

    /// Return type.
    pub const fn return_type(&self) -> &AdapterTypeKind {
        &self.return_type
    }

    /// Ordered parameters.
    pub fn params(&self) -> &[AdapterFunctionParam] {
        &self.params
    }
}

impl AdapterSymbol {
    /// Creates a typed adapter symbol.
    pub fn new(name: impl Into<String>, ty: AdapterTypeKind) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }

    /// Symbol name visible to Arcweft source.
    pub fn name(&self) -> &str {
        &self.name
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
        &self.name
    }

    /// Method signature.
    pub const fn signature(&self) -> &AdapterFunctionSignature {
        &self.signature
    }
}

impl AdapterFunction {
    /// Arcweft-visible function name.
    pub fn name(&self) -> &str {
        &self.name
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
            signature: AdapterFunctionSignature::return_only(AdapterTypeKind::Unit),
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

impl AdapterToolingDoc {
    /// Creates one tooling documentation entry.
    pub fn new(subject: impl Into<String>, docs: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            docs: docs.into(),
        }
    }

    /// Documented adapter item.
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Documentation body.
    pub fn docs(&self) -> &str {
        &self.docs
    }
}

impl AdapterRustFunction {
    /// Rust adapter package that exported this function.
    pub fn package(&self) -> &str {
        &self.package
    }

    /// Arcweft-visible function path.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Rust path recorded in the adapter metadata.
    pub fn rust_path(&self) -> &str {
        &self.rust_path
    }

    /// Full callable signature.
    pub const fn signature(&self) -> &AdapterFunctionSignature {
        &self.signature
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

    /// Binds every registration-visible base fact to one deterministic generated document.
    #[cfg(feature = "sema")]
    pub fn source_backed_registration_facts(
        &self,
        ordinal: u64,
    ) -> Result<SourceBackedAdapterRegistrationFacts, AdapterRegistrationFactsError> {
        let mut source = String::new();
        writeln!(&mut source, "adapter-manifest-v1 {self:#?}")
            .expect("writing adapter facts to a String cannot fail");
        let mut symbols = self.symbols.iter().collect::<Vec<_>>();
        symbols.sort_by(|left, right| {
            left.name()
                .cmp(right.name())
                .then_with(|| format!("{:?}", left.ty()).cmp(&format!("{:?}", right.ty())))
        });
        let mut symbol_ranges = Vec::with_capacity(symbols.len());
        for symbol in symbols {
            source.push_str("symbol ");
            let start = source.len();
            source.push_str(symbol.name());
            let end = source.len();
            source.push('\n');
            symbol_ranges.push((symbol.name().to_owned(), SourceRange::new(start, end)));
        }

        let document = Arc::new(SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-generated://adapter-context/{ordinal}"))?,
            SourceName::Generated,
            source,
        )?);
        let mut externals = Vec::with_capacity(symbol_ranges.len());
        for (name, range) in symbol_ranges {
            let declaration = document.span(range)?;
            let canonical_path =
                SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), name.clone())?;
            let direct_binding = ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                name.clone(),
                Some(Visibility::Public),
                declaration.clone(),
                false,
            )?;
            let seed = ExternalDeclarationSeed::try_new(
                canonical_path,
                Some(Visibility::Public),
                declaration.clone(),
                vec![direct_binding],
            )?;
            externals.push(ExternalRegistrationFact::new(
                seed,
                RegisteredExternalOwner::Environment(EnvironmentBindingId::try_new(name)?),
                declaration,
            ));
        }
        Ok(SourceBackedAdapterRegistrationFacts {
            document,
            externals,
        })
    }

    /// Adds one injected symbol.
    #[must_use]
    pub fn with_symbol(mut self, name: impl Into<String>, ty: AdapterTypeKind) -> Self {
        self.symbols.push(AdapterSymbol::new(name, ty));
        self
    }

    /// Adds one injected method.
    #[must_use]
    pub fn with_method(
        mut self,
        receiver: AdapterTypeKind,
        name: impl Into<String>,
        return_type: AdapterTypeKind,
    ) -> Self {
        self.methods.push(AdapterMethod {
            receiver,
            name: name.into(),
            signature: AdapterFunctionSignature::return_only(return_type),
        });
        self
    }

    /// Adds one injected method with full parameter signature.
    #[must_use]
    pub fn with_method_signature(
        mut self,
        receiver: AdapterTypeKind,
        name: impl Into<String>,
        signature: AdapterFunctionSignature,
    ) -> Self {
        self.methods.push(AdapterMethod {
            receiver,
            name: name.into(),
            signature,
        });
        self
    }

    /// Adds one injected free function with typed effects.
    #[must_use]
    pub fn with_function_signature(
        mut self,
        name: impl Into<String>,
        signature: AdapterFunctionSignature,
        effects: impl IntoIterator<Item = AdapterEffectCapability>,
    ) -> Self {
        self.functions.push(AdapterFunction {
            name: name.into(),
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
    #[must_use]
    pub fn with_rust_manifest(mut self, manifest: &ArcweftRustManifest) -> Self {
        let package = manifest.package.name.clone();
        self.rust_types
            .extend(manifest.types.iter().cloned().map(|decl| AdapterRustType {
                package: package.clone(),
                decl,
            }));
        self.rust_functions.extend(
            manifest
                .functions
                .iter()
                .map(|function| adapter_rust_function(manifest.package.name.clone(), function)),
        );
        self
    }

    /// Applies this adapter manifest to an existing checker environment.
    #[cfg(feature = "sema")]
    pub fn apply_to_env(&self, env: TypeCheckEnv) -> TypeCheckEnv {
        let env = self.symbols.iter().fold(env, |env, symbol| {
            env.with_symbol(symbol.name(), symbol.ty().to_sema_type_kind())
        });
        let env = self.methods.iter().fold(env, |env, method| {
            env.with_method_signature(
                method.receiver.to_sema_type_kind(),
                method.name.clone(),
                method.signature.to_sema_function_signature(),
            )
        });
        let env = self.functions.iter().fold(env, |env, function| {
            let effects = function
                .effects()
                .iter()
                .map(AdapterEffectCapability::to_sema_effect_capability)
                .collect::<Vec<_>>();
            let signature = function.signature.to_sema_function_signature();
            env.with_function_signature(function.name.clone(), signature.clone())
                .with_function_effects(function.name.clone(), effects)
        });
        let env = self.effects.iter().fold(env, |env, effect| {
            env.with_capability(effect.to_sema_effect_capability())
        });
        let env = self.rust_functions.iter().fold(env, |env, function| {
            let signature = function.signature.to_sema_function_signature();
            env.with_function_signature(function.name.clone(), signature.clone())
                .with_rust_function_export(
                    function.package.clone(),
                    function.name.clone(),
                    signature,
                )
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

#[cfg(feature = "sema")]
impl SourceBackedAdapterRegistrationFacts {
    pub fn document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    pub fn externals(&self) -> &[ExternalRegistrationFact] {
        &self.externals
    }

    pub fn into_parts(self) -> (Arc<SourceDocument>, Vec<ExternalRegistrationFact>) {
        (self.document, self.externals)
    }
}

impl AdapterRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry from typed manifests.
    pub fn from_manifests(manifests: impl IntoIterator<Item = AdapterManifest>) -> Self {
        Self {
            manifests: manifests.into_iter().collect(),
        }
    }

    /// Adds one manifest.
    #[must_use]
    pub fn with_manifest(mut self, manifest: AdapterManifest) -> Self {
        self.manifests.push(manifest);
        self
    }

    /// Looks up one manifest by id.
    pub fn get(&self, id: &str) -> Option<&AdapterManifest> {
        self.manifests
            .iter()
            .find(|manifest| manifest.id().as_str() == id)
    }

    /// Known adapter ids.
    pub fn adapter_ids(&self) -> Vec<&str> {
        self.manifests
            .iter()
            .map(|manifest| manifest.id().as_str())
            .collect()
    }

    /// All registered manifests.
    pub fn manifests(&self) -> &[AdapterManifest] {
        &self.manifests
    }
}

fn adapter_rust_function(
    package: impl Into<String>,
    function: &ArcweftRustFunction,
) -> AdapterRustFunction {
    AdapterRustFunction {
        package: package.into(),
        name: function.name.clone(),
        rust_path: function.rust_path.clone(),
        signature: AdapterFunctionSignature::new(
            rust_type_ref_to_adapter_type_kind(&function.return_type),
            function.params.iter().map(adapter_function_param),
        ),
    }
}

fn adapter_function_param(param: &ArcweftRustParam) -> AdapterFunctionParam {
    AdapterFunctionParam::required(
        param.name.clone(),
        rust_type_ref_to_adapter_type_kind(&param.ty),
    )
}

#[cfg(feature = "sema")]
fn apply_rust_type_to_env(env: TypeCheckEnv, ty: &AdapterRustType) -> TypeCheckEnv {
    let type_kind = TypeKind::Named(ty.decl.name.clone());
    let env = env.with_rust_type_export(ty.package.clone(), ty.decl.name.clone());
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
impl AdapterFunctionParam {
    /// Converts this parameter into the semantic checker parameter model.
    pub fn to_sema_function_param(&self) -> FunctionParam {
        FunctionParam::required(self.name.clone(), self.ty.to_sema_type_kind())
    }
}

#[cfg(feature = "sema")]
impl AdapterFunctionSignature {
    /// Converts this signature into the semantic checker signature model.
    pub fn to_sema_function_signature(&self) -> FunctionSignature {
        FunctionSignature::new(
            self.return_type.to_sema_type_kind(),
            self.params
                .iter()
                .map(AdapterFunctionParam::to_sema_function_param),
        )
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
mod tests {
    use super::*;
    use arcweft_rust_abi::{
        ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustParam,
        ArcweftRustPurity, ArcweftRustTypeDecl, ArcweftRustTypeKind, ArcweftRustTypeRef,
        ArcweftRustVariant,
    };

    #[cfg(feature = "sema")]
    #[test]
    fn adapter_manifest_applies_effect_capabilities_and_function_effects() {
        let manifest = AdapterManifest::new("fixture", "Fixture")
            .with_effect(AdapterEffectCapability::new("fs.read"))
            .with_function_signature(
                "adapter.read_text",
                AdapterFunctionSignature::new(
                    AdapterTypeKind::String,
                    [AdapterFunctionParam::required(
                        "path",
                        AdapterTypeKind::String,
                    )],
                ),
                [AdapterEffectCapability::new("fs.read")],
            );
        let env = manifest.apply_to_env(TypeCheckEnv::new());

        assert!(env.has_capability("fs.read"));
        assert_eq!(
            env.function_effects("adapter.read_text").map(|effects| {
                effects
                    .iter()
                    .map(EffectCapability::as_str)
                    .collect::<Vec<_>>()
            }),
            Some(vec!["fs.read"])
        );
        assert!(
            env.available_effects().is_none(),
            "surface application must not select target availability"
        );

        let target_env = manifest.apply_to_target_env(TypeCheckEnv::new());
        assert!(
            target_env
                .available_effects()
                .is_some_and(|effects| effects.contains(&EffectCapability::new("fs.read")))
        );
    }

    #[cfg(feature = "sema")]
    #[test]
    fn source_backed_adapter_facts_bind_exact_environment_keys_and_base_revision() {
        let first_manifest = AdapterManifest::new("fixture", "Fixture")
            .with_symbol("adapter.viewport", AdapterTypeKind::I32);
        let changed_manifest = AdapterManifest::new("fixture", "Fixture")
            .with_symbol("adapter.viewport", AdapterTypeKind::I64);
        let first = first_manifest
            .source_backed_registration_facts(7)
            .expect("first source-backed facts");
        let changed = changed_manifest
            .source_backed_registration_facts(7)
            .expect("changed source-backed facts");

        assert_eq!(
            first.document().identity().id().as_str(),
            "arcweft-generated://adapter-context/7"
        );
        assert_ne!(
            first.document().identity().revision(),
            changed.document().identity().revision(),
            "a base-environment type change must change the complete fact revision"
        );
        assert_eq!(first.externals().len(), 1);
        assert!(matches!(
            first.externals()[0].target(),
            RegisteredExternalOwner::Environment(id)
                if id.as_str() == "adapter.viewport"
        ));
        let base = first_manifest.apply_to_env(TypeCheckEnv::new());
        let RegisteredExternalOwner::Environment(id) = first.externals()[0].target() else {
            panic!("adapter symbol must register an environment owner");
        };
        assert_eq!(base.environment_binding(id), Some(&TypeKind::I32));
    }

    #[test]
    fn rust_manifest_injects_full_function_signature() {
        let manifest = ArcweftRustManifest::new(ArcweftRustPackage {
            name: "truck_game".to_owned(),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
        })
        .with_type(ArcweftRustTypeDecl {
            name: "Rank".to_owned(),
            rust_path: "truck_game::Rank".to_owned(),
            kind: ArcweftRustTypeKind::Enum {
                variants: vec![
                    ArcweftRustVariant {
                        name: "Bronze".to_owned(),
                        fields: Vec::new(),
                    },
                    ArcweftRustVariant {
                        name: "Custom".to_owned(),
                        fields: vec![arcweft_rust_abi::ArcweftRustField {
                            name: "label".to_owned(),
                            ty: ArcweftRustTypeRef::String,
                        }],
                    },
                ],
            },
        })
        .with_function(ArcweftRustFunction {
            name: "mini_games.truck.score_to_rank".to_owned(),
            rust_path: "truck_game::score_to_rank".to_owned(),
            params: vec![ArcweftRustParam {
                name: "score".to_owned(),
                ty: ArcweftRustTypeRef::I32,
            }],
            return_type: ArcweftRustTypeRef::Named {
                name: "Rank".to_owned(),
            },
            purity: ArcweftRustPurity::Pure,
            effects: Vec::new(),
        });

        let context = AdapterManifest::new("fixture", "Fixture").with_rust_manifest(&manifest);

        assert_eq!(context.rust_functions().len(), 1);
        assert_eq!(context.rust_functions()[0].signature().params().len(), 1);
        assert_eq!(
            context.rust_functions()[0].signature(),
            &AdapterFunctionSignature::new(
                AdapterTypeKind::Named("Rank".to_owned()),
                [AdapterFunctionParam::required(
                    "score",
                    AdapterTypeKind::I32
                )]
            )
        );
    }

    #[cfg(feature = "sema")]
    #[test]
    fn rust_manifest_applies_to_semantic_env_when_enabled() {
        let manifest = ArcweftRustManifest::new(ArcweftRustPackage {
            name: "truck_game".to_owned(),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
        })
        .with_type(ArcweftRustTypeDecl {
            name: "Rank".to_owned(),
            rust_path: "truck_game::Rank".to_owned(),
            kind: ArcweftRustTypeKind::Enum {
                variants: vec![
                    ArcweftRustVariant {
                        name: "Bronze".to_owned(),
                        fields: Vec::new(),
                    },
                    ArcweftRustVariant {
                        name: "Custom".to_owned(),
                        fields: vec![arcweft_rust_abi::ArcweftRustField {
                            name: "label".to_owned(),
                            ty: ArcweftRustTypeRef::String,
                        }],
                    },
                ],
            },
        })
        .with_function(ArcweftRustFunction {
            name: "mini_games.truck.score_to_rank".to_owned(),
            rust_path: "truck_game::score_to_rank".to_owned(),
            params: vec![ArcweftRustParam {
                name: "score".to_owned(),
                ty: ArcweftRustTypeRef::I32,
            }],
            return_type: ArcweftRustTypeRef::Named {
                name: "Rank".to_owned(),
            },
            purity: ArcweftRustPurity::Pure,
            effects: Vec::new(),
        });

        let env = AdapterManifest::new("fixture", "Fixture")
            .with_rust_manifest(&manifest)
            .apply_to_env(TypeCheckEnv::new());

        assert_eq!(
            env,
            TypeCheckEnv::new()
                .with_function_signature(
                    "mini_games.truck.score_to_rank",
                    FunctionSignature::new(
                        TypeKind::Named("Rank".to_owned()),
                        [FunctionParam::required("score", TypeKind::I32)]
                    )
                )
                .with_rust_function_export(
                    "truck_game",
                    "mini_games.truck.score_to_rank",
                    FunctionSignature::new(
                        TypeKind::Named("Rank".to_owned()),
                        [FunctionParam::required("score", TypeKind::I32)]
                    )
                )
                .with_rust_type_export("truck_game", "Rank")
                .try_with_enum_variants(TypeKind::Named("Rank".to_owned()), ["Bronze"])
                .expect("non-character Rust enum variants are accepted")
        );
    }
}
