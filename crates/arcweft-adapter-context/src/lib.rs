//! Semantic context contributed by host adapters.
//!
//! The language checker stays adapter-agnostic. Adapter runners, CLIs, LSP
//! profiles, and tests opt into one of these contexts when a host surface
//! injects runtime bindings such as HTTP request data.

use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::types::{MapKind, TypeKind};

/// A symbol injected by a host adapter into the checked source environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdapterSymbol {
    name: String,
    ty: TypeKind,
}

/// Type-checking facts supplied by a host adapter profile.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdapterTypecheckContext {
    symbols: Vec<AdapterSymbol>,
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

    /// Applies this adapter context to an existing checker environment.
    pub fn apply_to_env(&self, env: TypeCheckEnv) -> TypeCheckEnv {
        self.symbols.iter().fold(env, |env, symbol| {
            env.with_symbol(symbol.name(), symbol.ty().clone())
        })
    }

    /// Injected symbols, preserved for tooling and diagnostics.
    pub fn symbols(&self) -> &[AdapterSymbol] {
        &self.symbols
    }
}

/// Semantic context used by the built-in native HTTP server adapter.
pub fn native_http_server_context() -> AdapterTypecheckContext {
    AdapterTypecheckContext::new()
        .with_symbol("request", TypeKind::Named("HttpRequestContext".to_owned()))
        .with_symbol(
            "route_params",
            TypeKind::Map {
                kind: MapKind::BTree,
                key: Box::new(TypeKind::String),
                value: Box::new(TypeKind::String),
            },
        )
}
