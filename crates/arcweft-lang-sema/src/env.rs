use crate::types::TypeKind;
use std::collections::{HashMap, HashSet};

/// Method signature tracked by the lightweight semantic environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodSignature {
    pub(crate) return_type: TypeKind,
}

/// Small, explicit environment used to validate that HIR can feed type checking.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeCheckEnv {
    pub(crate) symbols: HashMap<String, TypeKind>,
    pub(crate) functions: HashMap<String, TypeKind>,
    pub(crate) methods: HashMap<(TypeKind, String), MethodSignature>,
    pub(crate) indexes: HashMap<TypeKind, TypeKind>,
    pub(crate) capabilities: HashSet<String>,
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

    /// Registers a method return type for a receiver type.
    #[must_use]
    pub fn with_method(
        mut self,
        receiver: TypeKind,
        method: impl Into<String>,
        return_type: TypeKind,
    ) -> Self {
        self.methods
            .insert((receiver, method.into()), MethodSignature { return_type });
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

    pub(crate) fn symbol_type(&self, name: &str) -> Option<&TypeKind> {
        self.symbols.get(name)
    }

    pub(crate) fn function_type(&self, name: &str) -> Option<&TypeKind> {
        self.functions.get(name)
    }

    pub(crate) fn method_type(&self, receiver: &TypeKind, method: &str) -> Option<&TypeKind> {
        self.methods
            .get(&(receiver.clone(), method.to_owned()))
            .map(|signature| &signature.return_type)
    }

    pub(crate) fn index_type(&self, target: &TypeKind) -> Option<&TypeKind> {
        self.indexes.get(target)
    }

    /// Returns whether the environment grants a named effect or state capability.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }
}
