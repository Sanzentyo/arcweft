//! Typed adapter manifest model shared by CLI, LSP, and semantic checking.

use arcweft_lang_sema::env::{FunctionParam, FunctionSignature, TypeCheckEnv};
use arcweft_lang_sema::types::TypeKind;
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustParam, ArcweftRustTypeDecl,
    ArcweftRustTypeRef,
};

/// Stable adapter identifier used by launch profiles and tooling.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterId(String);

/// A symbol injected by a host adapter into the checked source environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterSymbol {
    name: String,
    ty: TypeKind,
}

/// A method injected by a host adapter for a receiver type it contributes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterMethod {
    receiver: TypeKind,
    name: String,
    signature: FunctionSignature,
}

/// A free function injected by a host adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterFunction {
    name: String,
    signature: FunctionSignature,
    effects: Vec<AdapterEffectCapability>,
}

/// Effect capability granted or required by an adapter surface.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AdapterEffectCapability {
    name: String,
}

/// Runtime host call exported by an adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterHostCall {
    id: AdapterHostCallId,
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
    signature: FunctionSignature,
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

impl AdapterSymbol {
    /// Creates a typed adapter symbol.
    pub fn new(name: impl Into<String>, ty: TypeKind) -> Self {
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
    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }
}

impl AdapterMethod {
    /// Receiver type this method is attached to.
    pub const fn receiver(&self) -> &TypeKind {
        &self.receiver
    }

    /// Arcweft-visible method name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Method signature.
    pub const fn signature(&self) -> &FunctionSignature {
        &self.signature
    }
}

impl AdapterFunction {
    /// Arcweft-visible function name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Full callable signature.
    pub const fn signature(&self) -> &FunctionSignature {
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
        Self { name: name.into() }
    }

    /// Canonical capability label.
    pub fn as_str(&self) -> &str {
        &self.name
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
    pub const fn signature(&self) -> &FunctionSignature {
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

    /// Adds one injected symbol.
    #[must_use]
    pub fn with_symbol(mut self, name: impl Into<String>, ty: TypeKind) -> Self {
        self.symbols.push(AdapterSymbol::new(name, ty));
        self
    }

    /// Adds one injected method.
    #[must_use]
    pub fn with_method(
        mut self,
        receiver: TypeKind,
        name: impl Into<String>,
        return_type: TypeKind,
    ) -> Self {
        self.methods.push(AdapterMethod {
            receiver,
            name: name.into(),
            signature: FunctionSignature::return_only(return_type),
        });
        self
    }

    /// Adds one injected method with full parameter signature.
    #[must_use]
    pub fn with_method_signature(
        mut self,
        receiver: TypeKind,
        name: impl Into<String>,
        signature: FunctionSignature,
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
        signature: FunctionSignature,
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
    pub fn apply_to_env(&self, env: TypeCheckEnv) -> TypeCheckEnv {
        let env = self.symbols.iter().fold(env, |env, symbol| {
            env.with_symbol(symbol.name(), symbol.ty().clone())
        });
        let env = self.methods.iter().fold(env, |env, method| {
            env.with_method_signature(
                method.receiver.clone(),
                method.name.clone(),
                method.signature.clone(),
            )
        });
        let env = self.functions.iter().fold(env, |env, function| {
            let effects = function
                .effects()
                .iter()
                .map(|effect| effect.as_str().to_owned())
                .collect::<Vec<_>>();
            env.with_function_signature(function.name.clone(), function.signature.clone())
                .with_function_effects(function.name.clone(), effects)
        });
        let env = self.effects.iter().fold(env, |env, effect| {
            env.with_capability(effect.as_str().to_owned())
        });
        let env = self.rust_functions.iter().fold(env, |env, function| {
            env.with_function_signature(function.name.clone(), function.signature.clone())
                .with_rust_function_export(
                    function.package.clone(),
                    function.name.clone(),
                    function.signature.clone(),
                )
        });
        self.rust_types.iter().fold(env, |env, ty| {
            env.with_rust_type_export(ty.package.clone(), ty.decl.name.clone())
        })
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
        signature: FunctionSignature::new(
            rust_type_ref_to_type_kind(&function.return_type),
            function.params.iter().map(adapter_function_param),
        ),
    }
}

fn adapter_function_param(param: &ArcweftRustParam) -> FunctionParam {
    FunctionParam::required(param.name.clone(), rust_type_ref_to_type_kind(&param.ty))
}

fn rust_type_ref_to_type_kind(ty: &ArcweftRustTypeRef) -> TypeKind {
    match ty {
        ArcweftRustTypeRef::Unit => TypeKind::Unit,
        ArcweftRustTypeRef::Bool => TypeKind::Bool,
        ArcweftRustTypeRef::I8 => TypeKind::I8,
        ArcweftRustTypeRef::I16 => TypeKind::I16,
        ArcweftRustTypeRef::I32 => TypeKind::I32,
        ArcweftRustTypeRef::I64 => TypeKind::I64,
        ArcweftRustTypeRef::I128 => TypeKind::I128,
        ArcweftRustTypeRef::ISize => TypeKind::ISize,
        ArcweftRustTypeRef::U8 => TypeKind::U8,
        ArcweftRustTypeRef::U16 => TypeKind::U16,
        ArcweftRustTypeRef::U32 => TypeKind::U32,
        ArcweftRustTypeRef::U64 => TypeKind::U64,
        ArcweftRustTypeRef::U128 => TypeKind::U128,
        ArcweftRustTypeRef::USize => TypeKind::USize,
        ArcweftRustTypeRef::F32 => TypeKind::F32,
        ArcweftRustTypeRef::F64 => TypeKind::F64,
        ArcweftRustTypeRef::String => TypeKind::String,
        ArcweftRustTypeRef::Char => TypeKind::Char,
        ArcweftRustTypeRef::Vec { item } => {
            TypeKind::Vec(Box::new(rust_type_ref_to_type_kind(item)))
        }
        ArcweftRustTypeRef::Seq { item } => {
            TypeKind::Seq(Box::new(rust_type_ref_to_type_kind(item)))
        }
        ArcweftRustTypeRef::Option { item } => {
            TypeKind::Option(Box::new(rust_type_ref_to_type_kind(item)))
        }
        ArcweftRustTypeRef::Result { ok, error } => TypeKind::Result {
            ok: Box::new(rust_type_ref_to_type_kind(ok)),
            error: Box::new(rust_type_ref_to_type_kind(error)),
        },
        ArcweftRustTypeRef::Tuple { items } => {
            TypeKind::Tuple(items.iter().map(rust_type_ref_to_type_kind).collect())
        }
        ArcweftRustTypeRef::Named { name } => TypeKind::Named(name.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_rust_abi::{
        ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustParam,
        ArcweftRustPurity, ArcweftRustTypeRef,
    };

    #[test]
    fn adapter_manifest_applies_effect_capabilities_and_function_effects() {
        let manifest = AdapterManifest::new("fixture", "Fixture")
            .with_effect(AdapterEffectCapability::new("fs.read"))
            .with_function_signature(
                "adapter.read_text",
                FunctionSignature::new(
                    TypeKind::String,
                    [FunctionParam::required("path", TypeKind::String)],
                ),
                [AdapterEffectCapability::new("fs.read")],
            );
        let env = manifest.apply_to_env(TypeCheckEnv::new());

        assert!(env.has_capability("fs.read"));
        assert_eq!(
            env.function_effects("adapter.read_text"),
            Some(["fs.read".to_owned()].as_slice())
        );
    }

    #[test]
    fn rust_manifest_injects_full_function_signature() {
        let manifest = ArcweftRustManifest::new(ArcweftRustPackage {
            name: "truck_game".to_owned(),
            version: "0.1.0".to_owned(),
            metadata_hash: None,
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
        let env = context.apply_to_env(TypeCheckEnv::new());

        assert_eq!(context.rust_functions().len(), 1);
        assert_eq!(context.rust_functions()[0].signature().params().len(), 1);
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
        );
    }
}
