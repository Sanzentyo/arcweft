use crate::types::{EntityKind, EntityType, TypeKind};
use arcweft_character::manifest::CharacterManifest;
use arcweft_lang_syntax::types::FnParamKind;
use std::collections::{HashMap, HashSet};

/// Agent debug path family used to type `state(...)` and `observation(...)`
/// probes from a project semantic index.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DebugPathKind {
    State,
    Observation,
}

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
    pub(crate) enum_variants: HashMap<TypeKind, HashSet<String>>,
    pub(crate) functions: HashMap<String, TypeKind>,
    pub(crate) function_signatures: HashMap<String, FunctionSignature>,
    pub(crate) function_effects: HashMap<String, Vec<EffectCapability>>,
    pub(crate) methods: HashMap<(TypeKind, String), MethodSignature>,
    pub(crate) indexes: HashMap<TypeKind, TypeKind>,
    pub(crate) agent_actions: HashMap<String, Vec<AgentActionEnvSignature>>,
    pub(crate) debug_paths: HashMap<(DebugPathKind, String), TypeKind>,
    pub(crate) capabilities: HashSet<EffectCapability>,
    pub(crate) available_effects: Option<HashSet<EffectCapability>>,
    pub(crate) rust_packages: HashMap<String, RustPackageExports>,
}

impl FunctionSignature {
    /// Creates a fixed-arity function signature.
    pub fn new(return_type: TypeKind, params: impl IntoIterator<Item = FunctionParam>) -> Self {
        Self {
            return_type: normalize_type_kind(return_type),
            params: params.into_iter().map(normalize_function_param).collect(),
            checks_args: true,
        }
    }

    /// Creates a return-only signature for adapter surfaces whose parameter
    /// model is supplied by a later typed metadata pass.
    pub fn return_only(return_type: TypeKind) -> Self {
        Self {
            return_type: normalize_type_kind(return_type),
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
            ty: normalize_type_kind(ty),
            kind: FnParamKind::Fixed,
            has_default: false,
        }
    }

    /// Creates a fixed positional/named parameter with a source-level default.
    pub fn defaulted(name: impl Into<String>, ty: TypeKind) -> Self {
        Self {
            name: Some(name.into()),
            ty: normalize_type_kind(ty),
            kind: FnParamKind::Fixed,
            has_default: true,
        }
    }

    /// Creates a rest parameter.
    pub fn rest(name: impl Into<String>, ty: TypeKind) -> Self {
        Self {
            name: Some(name.into()),
            ty: normalize_type_kind(ty),
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
            params: params
                .into_iter()
                .map(normalize_agent_action_param)
                .collect(),
            return_type: normalize_type_kind(return_type),
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
            ty: normalize_type_kind(ty),
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
            .with_symbol("data", TypeKind::Named("DataNamespace".to_owned()))
            .with_symbol("content", TypeKind::Named("ContentNamespace".to_owned()))
            .with_data_format_symbols()
            .with_content_functions()
            .with_function_signature(
                "component",
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required(
                        "component",
                        TypeKind::entity_ref(crate::types::EntityKind::Component),
                    )],
                ),
            )
            .with_enum_variants(TypeKind::DataFormat, data_format_variant_names())
            .with_function_signature(
                "data.encode",
                FunctionSignature::new(
                    TypeKind::Bytes,
                    [
                        FunctionParam::required("value", TypeKind::Named("_".to_owned())),
                        FunctionParam::required("format", TypeKind::DataFormat),
                    ],
                ),
            )
            .with_function_signature(
                "data.decode",
                FunctionSignature::new(
                    TypeKind::AgentValue,
                    [
                        FunctionParam::required("bytes", TypeKind::Bytes),
                        FunctionParam::required("format", TypeKind::DataFormat),
                        FunctionParam::defaulted("shape", TypeKind::DataShape),
                    ],
                ),
            )
            .with_function_signature(
                "data.shape",
                FunctionSignature::new(
                    TypeKind::DataShape,
                    [FunctionParam::required(
                        "value",
                        TypeKind::Named("_".to_owned()),
                    )],
                ),
            )
    }

    #[must_use]
    fn with_content_functions(self) -> Self {
        let content_ref = TypeKind::entity_ref(crate::types::EntityKind::Content);
        self.with_function_signature(
            "content.prefetch",
            FunctionSignature::new(
                TypeKind::Unit,
                [FunctionParam::required("unit", content_ref.clone())],
            ),
        )
        .with_function_effects("content.prefetch", ["content.load"])
        .with_function_signature(
            "content.ensure",
            FunctionSignature::new(
                TypeKind::Need {
                    ready: Box::new(TypeKind::Unit),
                    error: Box::new(TypeKind::Named("ContentLoadError".to_owned())),
                },
                [FunctionParam::required("unit", content_ref.clone())],
            ),
        )
        .with_function_effects("content.ensure", ["content.load"])
        .with_function_signature(
            "content.release",
            FunctionSignature::new(
                TypeKind::Unit,
                [FunctionParam::required("unit", content_ref)],
            ),
        )
        .with_function_effects("content.release", ["content.release"])
    }

    #[must_use]
    fn with_data_format_symbols(self) -> Self {
        data_format_variant_names()
            .into_iter()
            .fold(self, |env, variant| {
                env.with_symbol(format!("DataFormat.{variant}"), TypeKind::DataFormat)
            })
    }

    /// Registers the unit variants available for an enum-like type.
    #[must_use]
    pub fn with_enum_variants(
        mut self,
        ty: TypeKind,
        variants: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.enum_variants
            .entry(normalize_type_kind(ty))
            .or_default()
            .extend(variants.into_iter().map(Into::into));
        self
    }

    /// Registers look, part, and per-part variant enums from one validated manifest.
    #[must_use]
    pub fn with_character_manifest(mut self, manifest: &CharacterManifest) -> Self {
        let character = manifest.character().as_str();
        self = self.with_symbol(character, TypeKind::entity_ref(EntityKind::Character));
        if let Some(compact_name) = character.strip_prefix("character.")
            && !compact_name.is_empty()
        {
            self = self.with_symbol(compact_name, TypeKind::entity_ref(EntityKind::Character));
        }
        self = self.with_enum_variants(
            TypeKind::character_look(character),
            manifest.looks().iter().map(|look| look.id().as_str()),
        );
        self = self.with_enum_variants(
            TypeKind::character_part(character),
            manifest.parts().iter().map(|part| part.id().as_str()),
        );
        for part in manifest.parts() {
            self = self.with_enum_variants(
                TypeKind::character_variant(character, part.id().as_str()),
                part.variants().iter().map(|variant| variant.id().as_str()),
            );
        }
        self
    }

    /// Returns the registered look enum type when character metadata is loaded.
    pub fn character_look_type(&self, character: &str) -> Option<TypeKind> {
        let ty = TypeKind::character_look(character);
        self.enum_variants.contains_key(&ty).then_some(ty)
    }

    /// Returns sorted manifest-declared looks for tooling and tests.
    pub fn character_look_variants(&self, character: &str) -> Option<Vec<String>> {
        let ty = TypeKind::character_look(character);
        self.enum_variants.get(&ty).map(|variants| {
            let mut variants = variants.iter().cloned().collect::<Vec<_>>();
            variants.sort();
            variants
        })
    }

    /// Returns sorted manifest-declared part ids for tooling and tests.
    pub fn character_part_variants(&self, character: &str) -> Option<Vec<String>> {
        let ty = TypeKind::character_part(character);
        self.enum_variants.get(&ty).map(|variants| {
            let mut variants = variants.iter().cloned().collect::<Vec<_>>();
            variants.sort();
            variants
        })
    }

    /// Returns sorted manifest-declared variants for one character part.
    pub fn character_variant_variants(&self, character: &str, part: &str) -> Option<Vec<String>> {
        let ty = TypeKind::character_variant(character, part);
        self.enum_variants.get(&ty).map(|variants| {
            let mut variants = variants.iter().cloned().collect::<Vec<_>>();
            variants.sort();
            variants
        })
    }

    /// Returns registered enum-like unit variants grouped by semantic type in
    /// deterministic order for tooling surfaces such as LSP completion.
    pub fn enum_variant_sets(&self) -> Vec<(TypeKind, Vec<String>)> {
        let mut sets = self
            .enum_variants
            .iter()
            .map(|(ty, variants)| {
                let mut variants = variants.iter().cloned().collect::<Vec<_>>();
                variants.sort();
                (format!("{ty:?}"), ty.clone(), variants)
            })
            .collect::<Vec<_>>();
        sets.sort_by(|left, right| left.0.cmp(&right.0));
        sets.into_iter()
            .map(|(_, ty, variants)| (ty, variants))
            .collect()
    }

    /// Registers a variable, constant, or resolved path.
    #[must_use]
    pub fn with_symbol(mut self, name: impl Into<String>, ty: TypeKind) -> Self {
        self.symbols.insert(name.into(), normalize_type_kind(ty));
        self
    }

    /// Registers a free function return type.
    #[must_use]
    pub fn with_function(mut self, name: impl Into<String>, return_type: TypeKind) -> Self {
        self.functions
            .insert(name.into(), normalize_type_kind(return_type));
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
        let signature = normalize_function_signature(signature);
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
        let receiver = normalize_type_kind(receiver);
        let return_type = normalize_type_kind(return_type);
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
        let receiver = normalize_type_kind(receiver);
        let signature = normalize_function_signature(signature);
        self.methods
            .insert((receiver, method.into()), MethodSignature { signature });
        self
    }

    /// Registers index result type for a collection-like type.
    #[must_use]
    pub fn with_index(mut self, target: TypeKind, return_type: TypeKind) -> Self {
        self.indexes.insert(
            normalize_type_kind(target),
            normalize_type_kind(return_type),
        );
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
            .push(normalize_agent_action(action));
        self
    }

    /// Registers one typed Agent Debug Bus path.
    #[must_use]
    pub fn with_debug_path(
        mut self,
        kind: DebugPathKind,
        path: impl Into<String>,
        value_type: TypeKind,
    ) -> Self {
        self.debug_paths
            .insert((kind, path.into()), normalize_type_kind(value_type));
        self
    }

    /// Registers a checker capability such as `state.write(flow)`.
    #[must_use]
    pub fn with_capability(mut self, capability: impl Into<EffectCapability>) -> Self {
        self.capabilities.insert(capability.into());
        self
    }

    /// Sets the target environment effects available to the checked program.
    ///
    /// This is deliberately separate from `with_capability`: a checker
    /// capability can discharge semantic operations such as registry writes,
    /// while target availability states which runtime effects the selected
    /// adapter/runner can actually provide.
    #[must_use]
    pub fn with_available_effects<I, E>(mut self, effects: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<EffectCapability>,
    {
        self.available_effects = Some(effects.into_iter().map(Into::into).collect());
        self
    }

    /// Adds one target environment effect while preserving existing
    /// availability state.
    #[must_use]
    pub fn with_available_effect(mut self, effect: impl Into<EffectCapability>) -> Self {
        self.available_effects
            .get_or_insert_with(HashSet::new)
            .insert(effect.into());
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
        let signature = normalize_function_signature(signature);
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

    pub(crate) fn enum_has_variant(&self, ty: &TypeKind, variant: &str) -> bool {
        self.enum_variants
            .get(ty)
            .is_some_and(|variants| variants.contains(variant))
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

    pub(crate) fn debug_path_type(&self, kind: DebugPathKind, path: &str) -> Option<&TypeKind> {
        self.debug_paths.get(&(kind, path.to_owned()))
    }

    /// Returns whether the environment grants a named effect or state capability.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities
            .contains(&EffectCapability::new(capability))
    }

    /// Returns the target environment effect set, when target availability is
    /// being enforced.
    pub fn available_effects(&self) -> Option<&HashSet<EffectCapability>> {
        self.available_effects.as_ref()
    }

    pub(crate) fn rust_package(&self, package: &str) -> Option<&RustPackageExports> {
        self.rust_packages.get(package)
    }
}

fn data_format_variant_names() -> [&'static str; 10] {
    [
        "Json",
        "Toml",
        "Yaml",
        "MessagePack",
        "Cbor",
        "Avro",
        "Csv",
        "ArrowIpc",
        "Parquet",
        "ArcweftBinary",
    ]
}

fn normalize_function_signature(mut signature: FunctionSignature) -> FunctionSignature {
    signature.return_type = normalize_type_kind(signature.return_type);
    signature.params = signature
        .params
        .into_iter()
        .map(normalize_function_param)
        .collect();
    signature
}

fn normalize_function_param(mut param: FunctionParam) -> FunctionParam {
    param.ty = normalize_type_kind(param.ty);
    param
}

fn normalize_agent_action(mut action: AgentActionEnvSignature) -> AgentActionEnvSignature {
    action.params = action
        .params
        .into_iter()
        .map(normalize_agent_action_param)
        .collect();
    action.return_type = normalize_type_kind(action.return_type);
    action
}

fn normalize_agent_action_param(mut param: AgentActionEnvParam) -> AgentActionEnvParam {
    param.ty = normalize_type_kind(param.ty);
    param
}

fn normalize_type_kind(ty: TypeKind) -> TypeKind {
    match ty {
        TypeKind::Named(name) => TypeKind::primitive_name(&name).unwrap_or(TypeKind::Named(name)),
        TypeKind::Ref(entity) => TypeKind::Ref(EntityType::new(
            entity.kind().clone(),
            entity.value().cloned().map(normalize_type_kind),
        )),
        TypeKind::Probe(inner) => TypeKind::Probe(Box::new(normalize_type_kind(*inner))),
        TypeKind::Vec(inner) => TypeKind::Vec(Box::new(normalize_type_kind(*inner))),
        TypeKind::Array { item, len } => TypeKind::Array {
            item: Box::new(normalize_type_kind(*item)),
            len,
        },
        TypeKind::Slice(inner) => TypeKind::Slice(Box::new(normalize_type_kind(*inner))),
        TypeKind::Seq(inner) => TypeKind::Seq(Box::new(normalize_type_kind(*inner))),
        TypeKind::Map { kind, key, value } => TypeKind::Map {
            kind,
            key: Box::new(normalize_type_kind(*key)),
            value: Box::new(normalize_type_kind(*value)),
        },
        TypeKind::BorrowRef { lifetime, inner } => TypeKind::BorrowRef {
            lifetime,
            inner: Box::new(normalize_type_kind(*inner)),
        },
        TypeKind::Need { ready, error } => TypeKind::Need {
            ready: Box::new(normalize_type_kind(*ready)),
            error: Box::new(normalize_type_kind(*error)),
        },
        TypeKind::Stream { item, error } => TypeKind::Stream {
            item: Box::new(normalize_type_kind(*item)),
            error: Box::new(normalize_type_kind(*error)),
        },
        TypeKind::Source { item, error } => TypeKind::Source {
            item: Box::new(normalize_type_kind(*item)),
            error: Box::new(normalize_type_kind(*error)),
        },
        TypeKind::Result { ok, error } => TypeKind::Result {
            ok: Box::new(normalize_type_kind(*ok)),
            error: Box::new(normalize_type_kind(*error)),
        },
        TypeKind::Option(inner) => TypeKind::Option(Box::new(normalize_type_kind(*inner))),
        TypeKind::Handle {
            name,
            lifetime,
            state,
            must_drop,
        } => TypeKind::Handle {
            name,
            lifetime,
            state,
            must_drop,
        },
        TypeKind::ThreadHandle(inner) => {
            TypeKind::ThreadHandle(Box::new(normalize_type_kind(*inner)))
        }
        TypeKind::Shared(inner) => TypeKind::Shared(Box::new(normalize_type_kind(*inner))),
        TypeKind::Function { return_type } => TypeKind::Function {
            return_type: Box::new(normalize_type_kind(*return_type)),
        },
        TypeKind::Projection {
            subject,
            trait_name,
            assoc,
        } => TypeKind::Projection {
            subject: Box::new(normalize_type_kind(*subject)),
            trait_name,
            assoc,
        },
        TypeKind::Tuple(items) => {
            TypeKind::Tuple(items.into_iter().map(normalize_type_kind).collect())
        }
        TypeKind::Choice(alternatives) => {
            TypeKind::Choice(alternatives.into_iter().map(normalize_type_kind).collect())
        }
        other => other,
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
