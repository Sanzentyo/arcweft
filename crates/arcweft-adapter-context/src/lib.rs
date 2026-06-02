//! Semantic context contributed by host adapters.
//!
//! The language checker stays adapter-agnostic. Adapter runners, CLIs, LSP
//! profiles, and tests opt into one of these contexts when a host surface
//! injects runtime bindings such as HTTP request data.

use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::types::TypeKind;

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
    return_type: TypeKind,
}

/// Type-checking facts supplied by a host adapter profile.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdapterTypecheckContext {
    symbols: Vec<AdapterSymbol>,
    methods: Vec<AdapterMethod>,
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
            return_type,
        });
        self
    }

    /// Applies this adapter context to an existing checker environment.
    pub fn apply_to_env(&self, env: TypeCheckEnv) -> TypeCheckEnv {
        let env = self.symbols.iter().fold(env, |env, symbol| {
            env.with_symbol(symbol.name(), symbol.ty().clone())
        });
        self.methods.iter().fold(env, |env, method| {
            env.with_method(
                method.receiver.clone(),
                method.name.clone(),
                method.return_type.clone(),
            )
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

#[cfg(test)]
mod tests {
    use super::*;

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
            method.receiver == TypeKind::Named("Conv2dApi".to_owned())
                && method.name == "valid_f32"
                && method.return_type == tensor
        }));
        assert!(context.methods().iter().any(|method| {
            method.receiver == TypeKind::Named("InferApi".to_owned())
                && method.name == "argmax_last_dim_f32"
                && method.return_type == TypeKind::Seq(Box::new(TypeKind::USize))
        }));
        assert_eq!(env, context.clone().apply_to_env(TypeCheckEnv::new()));
    }
}
