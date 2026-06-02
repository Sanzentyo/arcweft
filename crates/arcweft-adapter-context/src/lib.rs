//! Semantic context contributed by host adapters.
//!
//! The language checker stays adapter-agnostic. Adapter runners, CLIs, LSP
//! profiles, and tests opt into one of these contexts when a host surface
//! injects runtime bindings such as HTTP request data or Rust adapter exports.

use arcweft_lang_sema::env::{FunctionParam, FunctionSignature, TypeCheckEnv};
use arcweft_lang_sema::types::TypeKind;
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustParam, ArcweftRustTypeDecl,
    ArcweftRustTypeRef,
};

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

/// A Rust function export injected into type checking and LSP tooling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterRustFunction {
    name: String,
    rust_path: String,
    signature: FunctionSignature,
}

/// A Rust ADT export injected into tooling metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterRustType {
    decl: ArcweftRustTypeDecl,
}

/// Type-checking facts supplied by a host adapter profile.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdapterTypecheckContext {
    symbols: Vec<AdapterSymbol>,
    methods: Vec<AdapterMethod>,
    rust_functions: Vec<AdapterRustFunction>,
    rust_types: Vec<AdapterRustType>,
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

impl AdapterRustFunction {
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
    /// Exported Rust ADT declaration.
    pub const fn decl(&self) -> &ArcweftRustTypeDecl {
        &self.decl
    }
}

impl AdapterTypecheckContext {
    /// Creates an empty adapter context.
    pub fn new() -> Self {
        Self::default()
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

    /// Adds Rust ABI metadata exported by an adapter crate.
    #[must_use]
    pub fn with_rust_manifest(mut self, manifest: &ArcweftRustManifest) -> Self {
        self.rust_types.extend(
            manifest
                .types
                .iter()
                .cloned()
                .map(|decl| AdapterRustType { decl }),
        );
        self.rust_functions
            .extend(manifest.functions.iter().map(adapter_rust_function));
        self
    }

    /// Applies this adapter context to an existing checker environment.
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
        self.rust_functions.iter().fold(env, |env, function| {
            env.with_function_signature(function.name.clone(), function.signature.clone())
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

    /// Rust functions exported by adapter metadata.
    pub fn rust_functions(&self) -> &[AdapterRustFunction] {
        &self.rust_functions
    }

    /// Rust types exported by adapter metadata.
    pub fn rust_types(&self) -> &[AdapterRustType] {
        &self.rust_types
    }
}

/// Semantic context used by the built-in native HTTP server adapter.
pub fn native_http_server_context() -> AdapterTypecheckContext {
    AdapterTypecheckContext::new()
        .with_symbol("request", TypeKind::Named("HttpRequestContext".to_owned()))
}

/// Semantic context for the optional forward-inference tensor adapter.
pub fn inference_tensor_context() -> AdapterTypecheckContext {
    let tensor = TypeKind::Named("TensorF32".to_owned());
    AdapterTypecheckContext::new()
        .with_symbol("conv2d", TypeKind::Named("Conv2dApi".to_owned()))
        .with_symbol("infer", TypeKind::Named("InferApi".to_owned()))
        .with_method(
            TypeKind::Named("Conv2dApi".to_owned()),
            "valid_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "matmul_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "add_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "bias_add_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "relu_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "max_pool2d_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "softmax_last_dim_f32",
            tensor.clone(),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "argmax_last_dim_f32",
            TypeKind::Seq(Box::new(TypeKind::USize)),
        )
        .with_method(
            TypeKind::Named("InferApi".to_owned()),
            "flatten_outer_f32",
            tensor,
        )
}

fn adapter_rust_function(function: &ArcweftRustFunction) -> AdapterRustFunction {
    AdapterRustFunction {
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
    fn inference_tensor_context_injects_namespaced_methods_without_core_prelude() {
        let context = inference_tensor_context();
        let tensor = TypeKind::Named("TensorF32".to_owned());
        let env = context.apply_to_env(TypeCheckEnv::new());

        assert_eq!(
            context.symbols(),
            &[
                AdapterSymbol::new("conv2d", TypeKind::Named("Conv2dApi".to_owned())),
                AdapterSymbol::new("infer", TypeKind::Named("InferApi".to_owned()))
            ]
        );
        assert!(context.methods().iter().any(|method| {
            method.receiver() == &TypeKind::Named("Conv2dApi".to_owned())
                && method.name() == "valid_f32"
                && method.signature().return_type() == &tensor
        }));
        assert!(context.methods().iter().any(|method| {
            method.receiver() == &TypeKind::Named("InferApi".to_owned())
                && method.name() == "argmax_last_dim_f32"
                && method.signature().return_type() == &TypeKind::Seq(Box::new(TypeKind::USize))
        }));
        assert_eq!(env, context.clone().apply_to_env(TypeCheckEnv::new()));
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

        let context = AdapterTypecheckContext::new().with_rust_manifest(&manifest);
        let env = context.apply_to_env(TypeCheckEnv::new());

        assert_eq!(context.rust_functions().len(), 1);
        assert_eq!(context.rust_functions()[0].signature().params().len(), 1);
        assert_eq!(
            env,
            TypeCheckEnv::new().with_function_signature(
                "mini_games.truck.score_to_rank",
                FunctionSignature::new(
                    TypeKind::Named("Rank".to_owned()),
                    [FunctionParam::required("score", TypeKind::I32)]
                )
            )
        );
    }
}
