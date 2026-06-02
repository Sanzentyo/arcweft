use crate::types::TypeKind;
use arcweft_lang_syntax::types::FnParamKind;
use std::collections::{HashMap, HashSet};

/// Function or method signature tracked by the semantic environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSignature {
    pub(crate) return_type: TypeKind,
    pub(crate) params: Vec<FunctionParam>,
    pub(crate) checks_args: bool,
}

/// One function or method parameter in a semantic environment signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionParam {
    pub(crate) name: Option<String>,
    pub(crate) ty: TypeKind,
    pub(crate) kind: FnParamKind,
    pub(crate) has_default: bool,
}

/// Method signature tracked by the lightweight semantic environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodSignature {
    pub(crate) signature: FunctionSignature,
}

/// Rust exports contributed by one adapter crate metadata manifest.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RustPackageExports {
    pub(crate) functions: HashMap<String, FunctionSignature>,
    pub(crate) types: HashSet<String>,
}

/// Small, explicit environment used to validate that HIR can feed type checking.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeCheckEnv {
    pub(crate) symbols: HashMap<String, TypeKind>,
    pub(crate) functions: HashMap<String, TypeKind>,
    pub(crate) function_signatures: HashMap<String, FunctionSignature>,
    pub(crate) function_effects: HashMap<String, Vec<String>>,
    pub(crate) methods: HashMap<(TypeKind, String), MethodSignature>,
    pub(crate) indexes: HashMap<TypeKind, TypeKind>,
    pub(crate) capabilities: HashSet<String>,
    pub(crate) rust_packages: HashMap<String, RustPackageExports>,
}

impl FunctionSignature {
    /// Creates a fixed-arity function signature.
    pub fn new(return_type: TypeKind, params: impl IntoIterator<Item = FunctionParam>) -> Self {
        Self {
            return_type,
            params: params.into_iter().collect(),
            checks_args: true,
        }
    }

    /// Creates a return-only signature for adapter surfaces whose parameter
    /// model is supplied by a later typed metadata pass.
    pub fn return_only(return_type: TypeKind) -> Self {
        Self {
            return_type,
            params: Vec::new(),
            checks_args: false,
        }
    }

    /// Return type produced by the callable.
    pub const fn return_type(&self) -> &TypeKind {
        &self.return_type
    }

    /// Ordered parameters accepted by the callable.
    pub fn params(&self) -> &[FunctionParam] {
        &self.params
    }

    /// Whether this signature has enough parameter information for arg checks.
    pub const fn checks_args(&self) -> bool {
        self.checks_args
    }
}

impl FunctionParam {
    /// Creates a required positional/named parameter.
    pub fn required(name: impl Into<String>, ty: TypeKind) -> Self {
        Self {
            name: Some(name.into()),
            ty,
            kind: FnParamKind::Fixed,
            has_default: false,
        }
    }

    /// Creates a rest parameter.
    pub fn rest(name: impl Into<String>, ty: TypeKind) -> Self {
        Self {
            name: Some(name.into()),
            ty,
            kind: FnParamKind::Rest,
            has_default: false,
        }
    }

    /// Parameter name when one is visible to tooling.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Parameter type.
    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    /// Surface parameter kind.
    pub const fn kind(&self) -> FnParamKind {
        self.kind
    }

    /// Whether the parameter has a default value.
    pub const fn has_default(&self) -> bool {
        self.has_default
    }

    pub(crate) const fn is_rest(&self) -> bool {
        matches!(self.kind, FnParamKind::Rest)
    }
}

impl TypeCheckEnv {
    /// Creates an empty type-checking environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a variable, constant, or resolved path.
    #[must_use]
    pub fn with_symbol(mut self, name: impl Into<String>, ty: TypeKind) -> Self {
        self.symbols.insert(name.into(), ty);
        self
    }

    /// Registers a free function return type.
    #[must_use]
    pub fn with_function(mut self, name: impl Into<String>, return_type: TypeKind) -> Self {
        self.functions.insert(name.into(), return_type);
        self
    }

    /// Registers a free function with full argument signature.
    #[must_use]
    pub fn with_function_signature(
        mut self,
        name: impl Into<String>,
        signature: FunctionSignature,
    ) -> Self {
        let name = name.into();
        self.functions
            .insert(name.clone(), signature.return_type().clone());
        self.function_signatures.insert(name, signature);
        self
    }

    /// Registers effects required by one free function.
    #[must_use]
    pub fn with_function_effects<I, S>(mut self, name: impl Into<String>, effects: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.function_effects
            .insert(name.into(), effects.into_iter().map(Into::into).collect());
        self
    }

    /// Registers a method return type for a receiver type.
    #[must_use]
    pub fn with_method(
        mut self,
        receiver: TypeKind,
        method: impl Into<String>,
        return_type: TypeKind,
    ) -> Self {
        self.methods.insert(
            (receiver, method.into()),
            MethodSignature {
                signature: FunctionSignature::return_only(return_type),
            },
        );
        self
    }

    /// Registers a method with full argument signature for a receiver type.
    #[must_use]
    pub fn with_method_signature(
        mut self,
        receiver: TypeKind,
        method: impl Into<String>,
        signature: FunctionSignature,
    ) -> Self {
        self.methods
            .insert((receiver, method.into()), MethodSignature { signature });
        self
    }

    /// Registers index result type for a collection-like type.
    #[must_use]
    pub fn with_index(mut self, target: TypeKind, return_type: TypeKind) -> Self {
        self.indexes.insert(target, return_type);
        self
    }

    /// Registers a checker capability such as `state.write(flow)`.
    #[must_use]
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.insert(capability.into());
        self
    }

    /// Registers one Rust function export under the adapter crate package.
    #[must_use]
    pub fn with_rust_function_export(
        mut self,
        package: impl Into<String>,
        name: impl Into<String>,
        signature: FunctionSignature,
    ) -> Self {
        self.rust_packages
            .entry(package.into())
            .or_default()
            .functions
            .insert(name.into(), signature);
        self
    }

    /// Registers one Rust type export under the adapter crate package.
    #[must_use]
    pub fn with_rust_type_export(
        mut self,
        package: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        self.rust_packages
            .entry(package.into())
            .or_default()
            .types
            .insert(name.into());
        self
    }

    pub(crate) fn symbol_type(&self, name: &str) -> Option<&TypeKind> {
        self.symbols.get(name)
    }

    pub(crate) fn function_type(&self, name: &str) -> Option<&TypeKind> {
        self.functions.get(name)
    }

    pub(crate) fn function_signature(&self, name: &str) -> Option<&FunctionSignature> {
        self.function_signatures.get(name)
    }

    /// Returns effects required by a function supplied by the environment.
    pub fn function_effects(&self, name: &str) -> Option<&[String]> {
        self.function_effects.get(name).map(Vec::as_slice)
    }

    pub(crate) fn method_type(&self, receiver: &TypeKind, method: &str) -> Option<&TypeKind> {
        self.methods
            .get(&(receiver.clone(), method.to_owned()))
            .map(|method| method.signature.return_type())
    }

    pub(crate) fn method_signature(
        &self,
        receiver: &TypeKind,
        method: &str,
    ) -> Option<&FunctionSignature> {
        self.methods
            .get(&(receiver.clone(), method.to_owned()))
            .map(|method| &method.signature)
    }

    pub(crate) fn index_type(&self, target: &TypeKind) -> Option<&TypeKind> {
        self.indexes.get(target)
    }

    /// Returns whether the environment grants a named effect or state capability.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }

    pub(crate) fn rust_package(&self, package: &str) -> Option<&RustPackageExports> {
        self.rust_packages.get(package)
    }
}

impl RustPackageExports {
    pub(crate) fn function(&self, name: &str) -> Option<&FunctionSignature> {
        self.functions.get(name)
    }

    pub(crate) fn has_type(&self, name: &str) -> bool {
        self.types.contains(name)
    }
}
