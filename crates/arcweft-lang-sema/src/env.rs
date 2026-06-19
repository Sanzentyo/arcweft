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

/// Agent-visible semantic action attached to one project entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentActionEnvSignature {
    action: String,
    params: Vec<AgentActionEnvParam>,
    return_type: TypeKind,
}

/// Named payload parameter for an Agent action visible to the checker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentActionEnvParam {
    name: String,
    ty: TypeKind,
    has_default: bool,
}

/// Canonical effect capability label tracked by semantic environments.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EffectCapability {
    label: String,
}

/// Parsed components of a canonical effect capability label.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectCapabilityParts {
    family: String,
    operation: String,
    scope: Option<String>,
}

/// Small, explicit environment used to validate that HIR can feed type checking.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeCheckEnv {
    pub(crate) symbols: HashMap<String, TypeKind>,
    pub(crate) functions: HashMap<String, TypeKind>,
    pub(crate) function_signatures: HashMap<String, FunctionSignature>,
    pub(crate) function_effects: HashMap<String, Vec<EffectCapability>>,
    pub(crate) methods: HashMap<(TypeKind, String), MethodSignature>,
    pub(crate) indexes: HashMap<TypeKind, TypeKind>,
    pub(crate) agent_actions: HashMap<String, Vec<AgentActionEnvSignature>>,
    pub(crate) capabilities: HashSet<EffectCapability>,
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

    /// Creates a fixed positional/named parameter with a source-level default.
    pub fn defaulted(name: impl Into<String>, ty: TypeKind) -> Self {
        Self {
            name: Some(name.into()),
            ty,
            kind: FnParamKind::Fixed,
            has_default: true,
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

impl EffectCapability {
    /// Creates a canonical effect capability label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }

    /// Source-level capability label.
    pub fn as_str(&self) -> &str {
        &self.label
    }

    /// Returns the parsed family/operation/scope shape.
    pub fn parts(&self) -> EffectCapabilityParts {
        parse_effect_capability_parts(&self.label)
    }
}

impl EffectCapabilityParts {
    /// Capability namespace such as `fs` or `system`.
    pub fn family(&self) -> &str {
        &self.family
    }

    /// Operation such as `read` or `write`.
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// Optional scoped selector from labels such as `state.write(flow)`.
    pub fn scope(&self) -> Option<&str> {
        self.scope.as_deref()
    }
}

impl AgentActionEnvSignature {
    /// Creates a semantic action signature for one project entity.
    pub fn new(
        action: impl Into<String>,
        params: impl IntoIterator<Item = AgentActionEnvParam>,
        return_type: TypeKind,
    ) -> Self {
        Self {
            action: action.into(),
            params: params.into_iter().collect(),
            return_type,
        }
    }

    /// Canonical action name such as `advance` or `dialogue.skip`.
    pub fn action(&self) -> &str {
        &self.action
    }

    /// Named payload parameters accepted by the action contract.
    pub fn params(&self) -> &[AgentActionEnvParam] {
        &self.params
    }

    /// Type returned by this action.
    pub const fn return_type(&self) -> &TypeKind {
        &self.return_type
    }
}

impl AgentActionEnvParam {
    /// Creates a checker-visible Agent action payload parameter.
    pub fn new(name: impl Into<String>, ty: TypeKind, has_default: bool) -> Self {
        Self {
            name: name.into(),
            ty,
            has_default,
        }
    }

    /// Source-visible payload key.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Expected payload value type.
    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    /// Whether this payload key can be omitted.
    pub const fn has_default(&self) -> bool {
        self.has_default
    }
}

impl From<&str> for EffectCapability {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for EffectCapability {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl TypeCheckEnv {
    /// Creates an empty type-checking environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates the standard source type-checking environment.
    pub fn standard() -> Self {
        Self::new().with_standard_builtins()
    }

    /// Registers builtins that are available to ordinary Arcweft source files.
    #[must_use]
    pub fn with_standard_builtins(self) -> Self {
        self.with_function("fmt", TypeKind::DisplayText)
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
    pub fn with_function_effects<I, E>(mut self, name: impl Into<String>, effects: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<EffectCapability>,
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

    /// Registers one semantic Agent action exported by a project entity.
    #[must_use]
    pub fn with_agent_action(
        mut self,
        target: impl Into<String>,
        action: AgentActionEnvSignature,
    ) -> Self {
        self.agent_actions
            .entry(target.into())
            .or_default()
            .push(action);
        self
    }

    /// Registers a checker capability such as `state.write(flow)`.
    #[must_use]
    pub fn with_capability(mut self, capability: impl Into<EffectCapability>) -> Self {
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
    pub fn function_effects(&self, name: &str) -> Option<&[EffectCapability]> {
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

    pub(crate) fn agent_actions(&self, target: &str) -> Option<&[AgentActionEnvSignature]> {
        self.agent_actions.get(target).map(Vec::as_slice)
    }

    /// Returns whether the environment grants a named effect or state capability.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities
            .contains(&EffectCapability::new(capability))
    }

    pub(crate) fn rust_package(&self, package: &str) -> Option<&RustPackageExports> {
        self.rust_packages.get(package)
    }
}

fn parse_effect_capability_parts(label: &str) -> EffectCapabilityParts {
    let (body, scope) = label
        .strip_suffix(')')
        .and_then(|value| value.rsplit_once('('))
        .map_or((label, None), |(body, scope)| {
            (body, Some(scope.to_owned()))
        });
    let (family, operation) = body
        .split_once('.')
        .map_or((body, ""), |(family, operation)| (family, operation));
    EffectCapabilityParts {
        family: family.to_owned(),
        operation: operation.to_owned(),
        scope,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_capability_parses_family_operation_and_scope() {
        let capability = EffectCapability::new("state.write(flow)");
        let parts = capability.parts();

        assert_eq!(capability.as_str(), "state.write(flow)");
        assert_eq!(parts.family(), "state");
        assert_eq!(parts.operation(), "write");
        assert_eq!(parts.scope(), Some("flow"));
    }

    #[test]
    fn typecheck_env_stores_capabilities_as_typed_ids() {
        let env = TypeCheckEnv::new()
            .with_capability(EffectCapability::new("fs.read"))
            .with_function_effects("adapter.read", [EffectCapability::new("fs.read")]);

        assert!(env.has_capability("fs.read"));
        assert_eq!(
            env.function_effects("adapter.read").map(|effects| {
                effects
                    .iter()
                    .map(EffectCapability::as_str)
                    .collect::<Vec<_>>()
            }),
            Some(vec!["fs.read"])
        );
    }

    #[test]
    fn standard_env_contains_dialogue_fmt_builtin() {
        assert_eq!(
            TypeCheckEnv::standard().function_type("fmt"),
            Some(&TypeKind::DisplayText)
        );
    }
}
