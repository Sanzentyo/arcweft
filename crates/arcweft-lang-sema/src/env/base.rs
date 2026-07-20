use super::{
    effects::EffectCapability,
    enums::{EnumVariantPayload, normalize_enum_variant_payload},
};
use crate::dialogue_view::{
    DIALOGUE_ACTION_TYPE, DIALOGUE_CONTENT_TYPE, DIALOGUE_OCCURRENCE_ID_TYPE, DIALOGUE_REVEAL_TYPE,
    DIALOGUE_STAGE_TYPE, DialogueViewModelRegistry, DialogueViewProjection,
    STANDARD_DIALOGUE_VIEW_TYPE,
};
use crate::effect_row::EffectRow;
use crate::registration::EnvironmentBindingId;
use crate::types::{CharacterNominalType, EntityType, TypeKind};
use arcweft_data::DataFormat;
use arcweft_lang_syntax::types::FnParamKind;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

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
    pub(crate) remaining_call_groups: usize,
    pub(crate) remaining_param_groups: Vec<Vec<FunctionParam>>,
}

/// One function or method parameter in a semantic environment signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionParam {
    pub(crate) name: Option<String>,
    pub(crate) ty: TypeKind,
    pub(crate) kind: FnParamKind,
    pub(crate) has_default: bool,
    pub(crate) higher_order_bindings: Vec<FunctionParamHigherOrderBinding>,
}

/// Function-valued binding exposed by one source parameter pattern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionParamHigherOrderBinding {
    pub(crate) name: String,
    pub(crate) ty: TypeKind,
    pub(crate) selector: FunctionParamSelector,
}

/// Location of a binding inside the source argument value supplied for a
/// function parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionParamSelector {
    Root,
    TupleIndex(Vec<usize>),
    Path(Vec<FunctionParamSelectorSegment>),
}

/// One segment inside a parameter argument selector path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FunctionParamSelectorSegment {
    TupleIndex(usize),
    RecordField(String),
    VariantPayload(String),
}

/// Method signature tracked by the lightweight semantic environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodSignature {
    pub(crate) signature: FunctionSignature,
}

/// Rust exports contributed by one adapter crate metadata manifest.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RustPackageExports {
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

/// Small, explicit environment used to validate that HIR can feed type checking.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeCheckEnv {
    pub(crate) symbols: HashMap<String, TypeKind>,
    pub(crate) enum_variants: HashMap<TypeKind, HashSet<String>>,
    pub(crate) enum_variant_payloads: HashMap<TypeKind, HashMap<String, EnumVariantPayload>>,
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
    pub(crate) nominal_records: HashMap<String, HashMap<String, TypeKind>>,
    pub(crate) dialogue_view_models: DialogueViewModelRegistry,
}

/// Invalid construction of a base semantic environment.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TypeCheckEnvBuildError {
    #[error("character nominal inventories are owned by CharacterRegistrar: {nominal:?}")]
    ReservedCharacterNominal { nominal: CharacterNominalType },
}

impl FunctionSignature {
    /// Creates a fixed-arity function signature.
    pub fn new(return_type: TypeKind, params: impl IntoIterator<Item = FunctionParam>) -> Self {
        Self {
            return_type: normalize_type_kind(return_type),
            params: params.into_iter().map(normalize_function_param).collect(),
            checks_args: true,
            remaining_call_groups: 0,
            remaining_param_groups: Vec::new(),
        }
    }

    /// Creates a return-only signature for adapter surfaces whose parameter
    /// model is supplied by a later typed metadata pass.
    pub fn return_only(return_type: TypeKind) -> Self {
        Self {
            return_type: normalize_type_kind(return_type),
            params: Vec::new(),
            checks_args: false,
            remaining_call_groups: 0,
            remaining_param_groups: Vec::new(),
        }
    }

    /// Marks how many declaration call groups remain after this signature's
    /// first group is supplied.
    #[must_use]
    pub fn with_remaining_call_groups(mut self, count: usize) -> Self {
        self.remaining_call_groups = count;
        self
    }

    /// Stores parameter metadata for declaration call groups after the first.
    #[must_use]
    pub fn with_remaining_param_groups(
        mut self,
        groups: impl IntoIterator<Item = impl IntoIterator<Item = FunctionParam>>,
    ) -> Self {
        self.remaining_param_groups = groups
            .into_iter()
            .map(|group| group.into_iter().map(normalize_function_param).collect())
            .collect();
        self.remaining_call_groups = self.remaining_param_groups.len();
        self
    }

    /// Return type produced by the callable.
    pub const fn return_type(&self) -> &TypeKind {
        &self.return_type
    }

    /// Returns the canonical semantic signature label used in diagnostics.
    pub(crate) fn source_label(&self) -> String {
        let params = self
            .params()
            .iter()
            .map(|param| {
                param.name().map_or_else(
                    || param.ty().source_label(),
                    |name| format!("{name}: {}", param.ty().source_label()),
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("fn({params}) -> {}", self.return_type().source_label())
    }

    /// Return type produced after every declared parameter group has been
    /// applied. Function wrappers introduced by currying are not included.
    pub fn body_return_type(&self) -> &TypeKind {
        let mut ty = &self.return_type;
        for _ in 0..self.remaining_call_groups {
            let TypeKind::Function { return_type, .. } = ty else {
                break;
            };
            ty = return_type;
        }
        ty
    }

    /// Ordered parameters accepted by the callable.
    pub fn params(&self) -> &[FunctionParam] {
        &self.params
    }

    /// Whether this signature has enough parameter information for arg checks.
    pub const fn checks_args(&self) -> bool {
        self.checks_args
    }

    /// Number of source declaration call groups still represented by the
    /// return function type.
    pub const fn remaining_call_groups(&self) -> usize {
        self.remaining_call_groups
    }

    /// Parameter metadata for a remaining declaration call group, where index
    /// 0 is the group immediately after the first source call group.
    pub fn remaining_param_group(&self, index: usize) -> Option<&[FunctionParam]> {
        self.remaining_param_groups.get(index).map(Vec::as_slice)
    }

    /// Type of this callable when referenced as a first-class function value.
    pub fn function_value_type(&self) -> Option<TypeKind> {
        self.checks_args.then(|| {
            TypeKind::function(
                self.params.iter().map(|param| param.ty.clone()),
                self.return_type.clone(),
            )
        })
    }

    /// Type of this callable when referenced as a function value with a known
    /// effect row.
    pub fn function_value_type_with_effects(&self, effects: EffectRow) -> Option<TypeKind> {
        self.checks_args.then(|| {
            let return_type = curried_return_type_with_body_effects(
                self.return_type.clone(),
                self.remaining_call_groups,
                &effects,
            );
            let invocation_effects = if self.remaining_call_groups == 0 {
                effects
            } else {
                EffectRow::closed(crate::effects::EffectSet::new())
            };
            TypeKind::function_with_effects(
                self.params.iter().map(|param| param.ty.clone()),
                return_type,
                invocation_effects,
            )
        })
    }

    pub(crate) fn with_body_effects(mut self, effects: &EffectRow) -> Self {
        self.return_type = curried_return_type_with_body_effects(
            self.return_type,
            self.remaining_call_groups,
            effects,
        );
        self
    }
}

fn curried_return_type_with_body_effects(
    ty: TypeKind,
    remaining_call_groups: usize,
    body_effects: &EffectRow,
) -> TypeKind {
    if remaining_call_groups == 0 {
        return ty;
    }
    let TypeKind::Function {
        params,
        return_type,
        ..
    } = ty
    else {
        return ty;
    };
    let return_type = curried_return_type_with_body_effects(
        *return_type,
        remaining_call_groups.saturating_sub(1),
        body_effects,
    );
    let effects = if remaining_call_groups == 1 {
        body_effects.clone()
    } else {
        EffectRow::closed(crate::effects::EffectSet::new())
    };
    TypeKind::function_with_effects(params, return_type, effects)
}

impl FunctionParam {
    /// Creates a required positional/named parameter.
    pub fn required(name: impl Into<String>, ty: TypeKind) -> Self {
        Self::new(Some(name.into()), ty, FnParamKind::Fixed, false, Vec::new())
    }

    /// Creates a fixed positional/named parameter with a source-level default.
    pub fn defaulted(name: impl Into<String>, ty: TypeKind) -> Self {
        Self::new(Some(name.into()), ty, FnParamKind::Fixed, true, Vec::new())
    }

    /// Creates a rest parameter.
    pub fn rest(name: impl Into<String>, ty: TypeKind) -> Self {
        Self::new(Some(name.into()), ty, FnParamKind::Rest, false, Vec::new())
    }

    pub(crate) fn new(
        name: Option<String>,
        ty: TypeKind,
        kind: FnParamKind,
        has_default: bool,
        higher_order_bindings: impl IntoIterator<Item = FunctionParamHigherOrderBinding>,
    ) -> Self {
        let ty = normalize_type_kind(ty);
        let higher_order_bindings = higher_order_bindings
            .into_iter()
            .map(normalize_function_param_higher_order_binding)
            .collect::<Vec<_>>();
        let higher_order_bindings = if higher_order_bindings.is_empty() {
            root_higher_order_bindings(name.as_ref(), &ty)
        } else {
            higher_order_bindings
        };
        Self {
            name,
            ty,
            kind,
            has_default,
            higher_order_bindings,
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

    /// Function-valued local bindings projected from this source parameter.
    pub fn higher_order_bindings(&self) -> &[FunctionParamHigherOrderBinding] {
        &self.higher_order_bindings
    }

    pub(crate) const fn is_rest(&self) -> bool {
        matches!(self.kind, FnParamKind::Rest)
    }
}

impl FunctionParamHigherOrderBinding {
    pub(crate) fn new(
        name: impl Into<String>,
        ty: TypeKind,
        selector: FunctionParamSelector,
    ) -> Self {
        Self {
            name: name.into(),
            ty,
            selector,
        }
    }

    /// Source binding name visible inside the callee body.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Function type of the binding.
    pub const fn ty(&self) -> &TypeKind {
        &self.ty
    }

    /// Argument selector that yields this binding's source value.
    pub const fn selector(&self) -> &FunctionParamSelector {
        &self.selector
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

impl TypeCheckEnv {
    /// Creates the core source environment with always-available runtime callables.
    pub fn new() -> Self {
        Self::default().with_standard_runtime_callables()
    }

    /// Creates the standard source type-checking environment.
    pub fn standard() -> Self {
        Self::new().with_standard_builtins()
    }

    /// Registers builtins that are available to ordinary Arcweft source files.
    #[must_use]
    pub fn with_standard_builtins(self) -> Self {
        self.with_standard_dialogue_view_types()
            .with_function("fmt", TypeKind::DisplayText)
            .with_function_signature(
                "SpeakerPreset.new",
                FunctionSignature::new(
                    TypeKind::SpeakerPreset(crate::types::EntityKind::Character),
                    [FunctionParam::required(
                        "character",
                        TypeKind::entity_ref(crate::types::EntityKind::Character),
                    )],
                ),
            )
            .with_symbol("data", TypeKind::Named("DataNamespace".to_owned()))
            .with_symbol("content", TypeKind::Named("ContentNamespace".to_owned()))
            .with_data_format_builtins()
            .with_content_functions()
            .with_function_signature(
                "view",
                FunctionSignature::new(
                    TypeKind::presentation_handle("View"),
                    [
                        FunctionParam::required(
                            "view",
                            TypeKind::entity_ref(crate::types::EntityKind::View),
                        ),
                        FunctionParam::defaulted(
                            "lifetime",
                            TypeKind::Named("PresentationLifetime".to_owned()),
                        ),
                    ],
                ),
            )
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

    /// Installs the finite source-visible runtime callable surface.
    ///
    /// These records are the single source for both standalone type checking
    /// and the immutable core publication accepted by a registered world.
    #[must_use]
    fn with_standard_runtime_callables(self) -> Self {
        let unit_callables = [
            "log.trace",
            "log.debug",
            "log.info",
            "log.warn",
            "log.error",
            "drop",
            "drop_optional",
            "on_drop",
            "signal.set",
            "metric.set",
            "event.emit",
            "adapter.events",
            "scene.show",
            "scene.clear",
            "progress.set",
            "meter.show",
            "text.show",
            "text.flush",
            "voice.stop",
            "cues.stop",
            "ensure",
            "assert",
            "debug_assert",
        ];
        let env = unit_callables.into_iter().fold(self, |env, name| {
            env.with_function_signature(name, FunctionSignature::return_only(TypeKind::Unit))
        });
        [
            ("panic", TypeKind::Never),
            ("fail", TypeKind::Never),
            ("bail", TypeKind::Never),
            (
                "load_bg",
                TypeKind::Need {
                    ready: Box::new(TypeKind::Named("ImageHandle".to_owned())),
                    error: Box::new(TypeKind::Named("ArcError".to_owned())),
                },
            ),
            (
                "asset.image",
                TypeKind::Need {
                    ready: Box::new(TypeKind::Named("ImageHandle".to_owned())),
                    error: Box::new(TypeKind::Named("AssetError".to_owned())),
                },
            ),
            (
                "voice.load",
                TypeKind::Need {
                    ready: Box::new(TypeKind::Named("VoiceHandle".to_owned())),
                    error: Box::new(TypeKind::Named("VoiceError".to_owned())),
                },
            ),
            ("len", TypeKind::I64),
        ]
        .into_iter()
        .fold(env, |env, (name, result)| {
            env.with_function_signature(name, FunctionSignature::return_only(result))
        })
        .with_method(
            TypeKind::Named("VoiceHandle".to_owned()),
            "stop",
            TypeKind::Unit,
        )
        .with_method(
            TypeKind::Named("DialogueText".to_owned()),
            "flush",
            TypeKind::Unit,
        )
    }

    #[must_use]
    fn with_standard_dialogue_view_types(self) -> Self {
        self.with_nominal_record(
            DIALOGUE_CONTENT_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
        )
        .with_nominal_record(
            DIALOGUE_OCCURRENCE_ID_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
        )
        .with_nominal_record(
            DIALOGUE_STAGE_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
        )
        .with_nominal_record(
            DIALOGUE_REVEAL_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
        )
        .with_nominal_record(
            DIALOGUE_ACTION_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
        )
        .with_nominal_record(
            STANDARD_DIALOGUE_VIEW_TYPE,
            [
                (
                    DialogueViewProjection::Speaker.field().to_owned(),
                    TypeKind::String,
                ),
                (
                    DialogueViewProjection::Content.field().to_owned(),
                    TypeKind::Named(DIALOGUE_CONTENT_TYPE.to_owned()),
                ),
                (
                    DialogueViewProjection::Occurrence.field().to_owned(),
                    TypeKind::Named(DIALOGUE_OCCURRENCE_ID_TYPE.to_owned()),
                ),
                (
                    DialogueViewProjection::Stage.field().to_owned(),
                    TypeKind::Named(DIALOGUE_STAGE_TYPE.to_owned()),
                ),
                (
                    DialogueViewProjection::Reveal.field().to_owned(),
                    TypeKind::Named(DIALOGUE_REVEAL_TYPE.to_owned()),
                ),
                (
                    DialogueViewProjection::PrimaryAction.field().to_owned(),
                    TypeKind::Named(DIALOGUE_ACTION_TYPE.to_owned()),
                ),
            ],
        )
        .with_dialogue_view_models(DialogueViewModelRegistry::standard())
    }

    /// Registers one nominal record and its typed fields.
    #[must_use]
    pub fn with_nominal_record(
        mut self,
        name: impl Into<String>,
        fields: impl IntoIterator<Item = (String, TypeKind)>,
    ) -> Self {
        self.nominal_records.insert(
            name.into(),
            fields
                .into_iter()
                .map(|(name, ty)| (name, normalize_type_kind(ty)))
                .collect(),
        );
        self
    }

    /// Standard and adapter-provided nominal records visible to source files.
    pub fn nominal_records(&self) -> &HashMap<String, HashMap<String, TypeKind>> {
        &self.nominal_records
    }

    /// Registers the semantic-role inventory used by dialogue View parameters.
    #[must_use]
    pub fn with_dialogue_view_models(mut self, models: DialogueViewModelRegistry) -> Self {
        self.dialogue_view_models = models;
        self
    }

    /// Dialogue View role-bearing nominal records visible to source files.
    pub const fn dialogue_view_models(&self) -> &DialogueViewModelRegistry {
        &self.dialogue_view_models
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

    /// Registers qualified values and expected-type shorthand from the owning
    /// data-format inventory.
    #[must_use]
    fn with_data_format_builtins(self) -> Self {
        let env = self
            .try_with_enum_variants(
                TypeKind::DataFormat,
                DataFormat::ALL.map(DataFormat::variant_name),
            )
            .expect("data-format inventory is not character nominal");
        DataFormat::ALL.into_iter().fold(env, |env, format| {
            env.with_symbol(
                format!("DataFormat.{}", format.variant_name()),
                TypeKind::DataFormat,
            )
        })
    }

    /// Registers the unit variants available for an enum-like type.
    pub fn try_with_enum_variants(
        mut self,
        ty: TypeKind,
        variants: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, TypeCheckEnvBuildError> {
        let ty = normalize_type_kind(ty);
        if let TypeKind::CharacterNominal(nominal) = &ty {
            return Err(TypeCheckEnvBuildError::ReservedCharacterNominal {
                nominal: nominal.clone(),
            });
        }
        self.insert_enum_variants(&ty, variants);
        Ok(self)
    }

    pub(crate) fn insert_enum_variants(
        &mut self,
        ty: &TypeKind,
        variants: impl IntoIterator<Item = impl Into<String>>,
    ) {
        self.enum_variants
            .entry(ty.clone())
            .or_default()
            .extend(variants.into_iter().map(|variant| {
                let variant = variant.into();
                self.enum_variant_payloads
                    .entry(ty.clone())
                    .or_default()
                    .entry(variant.clone())
                    .or_insert(EnumVariantPayload::Unit);
                variant
            }));
    }

    /// Registers one enum-like variant with its payload contract.
    pub fn try_with_enum_variant_payload(
        mut self,
        ty: TypeKind,
        variant: impl Into<String>,
        payload: EnumVariantPayload,
    ) -> Result<Self, TypeCheckEnvBuildError> {
        let ty = normalize_type_kind(ty);
        if let TypeKind::CharacterNominal(nominal) = &ty {
            return Err(TypeCheckEnvBuildError::ReservedCharacterNominal {
                nominal: nominal.clone(),
            });
        }
        let variant = variant.into();
        self.enum_variants
            .entry(ty.clone())
            .or_default()
            .insert(variant.clone());
        self.enum_variant_payloads
            .entry(ty)
            .or_default()
            .insert(variant, normalize_enum_variant_payload(payload));
        Ok(self)
    }

    /// Looks up one exact source-visible base-environment binding identity.
    pub fn environment_binding(&self, id: &EnvironmentBindingId) -> Option<&TypeKind> {
        self.symbols.get(id.as_str())
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
                (ty.source_label(), format!("{ty:?}"), ty.clone(), variants)
            })
            .collect::<Vec<_>>();
        sets.sort_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
        sets.into_iter()
            .map(|(_, _, ty, variants)| (ty, variants))
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

    pub(crate) fn enum_variant_payload(
        &self,
        ty: &TypeKind,
        variant: &str,
    ) -> Option<&EnumVariantPayload> {
        self.enum_variant_payloads
            .get(ty)
            .and_then(|variants| variants.get(variant))
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

fn normalize_function_signature(mut signature: FunctionSignature) -> FunctionSignature {
    signature.return_type = normalize_type_kind(signature.return_type);
    signature.params = signature
        .params
        .into_iter()
        .map(normalize_function_param)
        .collect();
    signature.remaining_param_groups = signature
        .remaining_param_groups
        .into_iter()
        .map(|group| group.into_iter().map(normalize_function_param).collect())
        .collect();
    signature
}

fn normalize_function_param(mut param: FunctionParam) -> FunctionParam {
    param.ty = normalize_type_kind(param.ty);
    param.higher_order_bindings = param
        .higher_order_bindings
        .into_iter()
        .map(normalize_function_param_higher_order_binding)
        .collect();
    if param.higher_order_bindings.is_empty() {
        param.higher_order_bindings = root_higher_order_bindings(param.name.as_ref(), &param.ty);
    }
    param
}

fn normalize_function_param_higher_order_binding(
    mut binding: FunctionParamHigherOrderBinding,
) -> FunctionParamHigherOrderBinding {
    binding.ty = normalize_type_kind(binding.ty);
    binding
}

fn root_higher_order_bindings(
    name: Option<&String>,
    ty: &TypeKind,
) -> Vec<FunctionParamHigherOrderBinding> {
    match (name, ty) {
        (Some(name), TypeKind::Function { .. }) => vec![FunctionParamHigherOrderBinding::new(
            name.clone(),
            ty.clone(),
            FunctionParamSelector::Root,
        )],
        _ => Vec::new(),
    }
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

pub(super) fn normalize_type_kind(ty: TypeKind) -> TypeKind {
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
        TypeKind::BorrowRef {
            kind,
            lifetime,
            inner,
        } => TypeKind::BorrowRef {
            kind,
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
        TypeKind::Function {
            params,
            return_type,
            effects,
        } => TypeKind::function_with_effects(
            params.into_iter().map(normalize_type_kind),
            normalize_type_kind(*return_type),
            effects,
        ),
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

impl RustPackageExports {
    pub(crate) fn has_type(&self, name: &str) -> bool {
        self.types.contains(name)
    }
}
