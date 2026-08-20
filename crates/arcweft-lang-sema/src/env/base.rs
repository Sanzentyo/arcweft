use super::identity::EnvironmentBindingId;
use super::{
    effects::EffectCapability,
    enums::{EnumVariantPayload, normalize_enum_variant_payload},
    nominal::{AcceptedNominalCatalog, AcceptedNominalOrigin, standard_exact_record},
};
use crate::callable::{
    CallableEvaluatedEffect, CallableLogLevel, CallableName, CallablePath, CallableValidator,
};
use crate::dialogue_view::{
    DIALOGUE_ACTION_TYPE, DIALOGUE_CHARACTER_TYPE, DIALOGUE_CONTENT_TYPE,
    DIALOGUE_OCCURRENCE_ID_TYPE, DIALOGUE_REVEAL_TYPE, DIALOGUE_STAGE_TYPE,
    DialogueCharacterProjection, DialogueProjectionCoordinate, DialogueViewModelRegistry,
    STANDARD_DIALOGUE_VIEW_TYPE,
};
use crate::effect_row::EffectRow;
use crate::types::{CharacterNominalType, EntityType, TypeKind};
use arcweft_data::DataFormat;
use arcweft_lang_syntax::types::FnParamKind;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

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

/// Typed standard-environment free-callable input retained until accepted-world publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StandardEnvironmentFunction {
    pub(crate) path: CallablePath,
    pub(crate) signature: FunctionSignature,
    pub(crate) effects: Vec<EffectCapability>,
    pub(crate) validator: CallableValidator,
    pub(crate) evaluated_effect: Option<CallableEvaluatedEffect>,
}

/// Typed standard-environment method input retained until accepted-world publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StandardEnvironmentMethod {
    pub(crate) receiver: TypeKind,
    pub(crate) member: CallableName,
    pub(crate) signature: FunctionSignature,
}

/// One source-ordered case owned by a closed base-environment enum schema.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EnvironmentEnumVariant {
    name: String,
    payload: EnumVariantPayload,
}

impl EnvironmentEnumVariant {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) const fn payload(&self) -> &EnumVariantPayload {
        &self.payload
    }
}

/// Sole ordered owner for one closed enum supplied by the base environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EnvironmentEnumSchema {
    owner: EnvironmentBindingId,
    variants: Vec<EnvironmentEnumVariant>,
}

impl EnvironmentEnumSchema {
    pub(crate) const fn owner(&self) -> &EnvironmentBindingId {
        &self.owner
    }

    pub(crate) fn variants(&self) -> &[EnvironmentEnumVariant] {
        &self.variants
    }
}

/// Small, explicit environment used to validate that HIR can feed type checking.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeCheckEnv {
    pub(crate) nominal_catalog: AcceptedNominalCatalog,
    pub(crate) symbols: HashMap<String, TypeKind>,
    closed_enums: HashMap<TypeKind, EnvironmentEnumSchema>,
    pub(crate) standard_functions: Vec<StandardEnvironmentFunction>,
    pub(crate) standard_methods: Vec<StandardEnvironmentMethod>,
    pub(crate) capabilities: HashSet<EffectCapability>,
    pub(crate) available_effects: Option<HashSet<EffectCapability>>,
    pub(crate) nominal_records: HashMap<String, HashMap<String, TypeKind>>,
    pub(crate) dialogue_view_models: DialogueViewModelRegistry,
}

/// Invalid construction of a base semantic environment.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TypeCheckEnvBuildError {
    #[error("character nominal inventories are owned by CharacterRegistrar: {nominal:?}")]
    ReservedCharacterNominal { nominal: CharacterNominalType },
    #[error(
        "base-environment enum owner {owner} is already assigned to {existing:?}, not {requested:?}"
    )]
    ConflictingEnumOwner {
        owner: EnvironmentBindingId,
        existing: Box<TypeKind>,
        requested: Box<TypeKind>,
    },
    #[error(
        "base-environment enum type {ty:?} is already assigned to owner {existing}, not {requested}"
    )]
    ConflictingEnumTypeOwner {
        ty: Box<TypeKind>,
        existing: EnvironmentBindingId,
        requested: EnvironmentBindingId,
    },
    #[error("base-environment enum owner {owner} contains duplicate case {variant}")]
    DuplicateEnumVariant {
        owner: EnvironmentBindingId,
        variant: String,
    },
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
    fn with_standard_builtins(self) -> Self {
        self.with_standard_accepted_nominals()
            .with_standard_presentation_nominals()
            .with_standard_dialogue_view_types()
            .with_standard_presentation_lifetimes()
            .with_standard_agent_enums()
            .with_standard_function(
                ["fmt"],
                FunctionSignature::return_only(TypeKind::DisplayText),
            )
            .with_symbol("data", TypeKind::Named("DataNamespace".to_owned()))
            .with_symbol("content", TypeKind::Named("ContentNamespace".to_owned()))
            .with_data_format_builtins()
            .with_content_functions()
            .with_standard_function(
                ["view"],
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
            .with_standard_function(
                ["data", "encode"],
                FunctionSignature::new(
                    TypeKind::Bytes,
                    [
                        FunctionParam::required("value", TypeKind::Named("_".to_owned())),
                        FunctionParam::required("format", TypeKind::DataFormat),
                    ],
                ),
            )
            .with_standard_function(
                ["data", "decode"],
                FunctionSignature::new(
                    TypeKind::AgentValue,
                    [
                        FunctionParam::required("bytes", TypeKind::Bytes),
                        FunctionParam::required("format", TypeKind::DataFormat),
                        FunctionParam::defaulted("shape", TypeKind::DataShape),
                    ],
                ),
            )
            .with_standard_function(
                ["data", "shape"],
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
    fn with_standard_agent_enums(self) -> Self {
        [
            (
                "CaptureFormat",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::CaptureFormat),
                &["png", "raw_rgba"][..],
            ),
            (
                "CaptureKind",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::CaptureKind),
                &["color", "mask"][..],
            ),
            (
                "PointerButton",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::PointerButton),
                &["primary", "secondary", "middle"][..],
            ),
        ]
        .into_iter()
        .fold(self, |environment, (owner, ty, variants)| {
            environment
                .try_with_enum_variants(
                    EnvironmentBindingId::try_new(owner)
                        .expect("Agent enum owner identity is valid"),
                    ty,
                    variants.iter().copied(),
                )
                .expect("Agent enum inventories have distinct typed owners")
        })
    }

    #[must_use]
    fn with_standard_presentation_nominals(self) -> Self {
        let environment = [
            ("Fx", TypeKind::Named("Fx".to_owned())),
            ("Color", TypeKind::Named("Color".to_owned())),
            ("Length", TypeKind::Named("Length".to_owned())),
            ("Angle", TypeKind::Named("Angle".to_owned())),
            ("AudioLevel", TypeKind::Named("AudioLevel".to_owned())),
            ("Tempo", TypeKind::Named("Tempo".to_owned())),
            ("Rgba8", TypeKind::Named("Rgba8".to_owned())),
            (
                "FxSampleContext",
                TypeKind::Named("FxSampleContext".to_owned()),
            ),
        ]
        .into_iter()
        .fold(self, |environment, (name, semantics)| {
            environment
                .try_with_nominal_record(
                    standard_exact_record(name, semantics, AcceptedNominalOrigin::Domain)
                        .expect("standard presentation atoms have valid typed identities"),
                )
                .expect("standard presentation atoms have distinct paths")
        });

        environment.with_standard_nominal_record(
            "Transform2D",
            [
                (
                    "translate_x".to_owned(),
                    TypeKind::Named("Length".to_owned()),
                ),
                (
                    "translate_y".to_owned(),
                    TypeKind::Named("Length".to_owned()),
                ),
                ("scale_x".to_owned(), TypeKind::F32),
                ("scale_y".to_owned(), TypeKind::F32),
                ("skew_x".to_owned(), TypeKind::Named("Angle".to_owned())),
                ("skew_y".to_owned(), TypeKind::Named("Angle".to_owned())),
                ("rotation".to_owned(), TypeKind::Named("Angle".to_owned())),
                ("origin_x".to_owned(), TypeKind::Named("Length".to_owned())),
                ("origin_y".to_owned(), TypeKind::Named("Length".to_owned())),
                ("opacity".to_owned(), TypeKind::F32),
            ],
        )
    }

    #[must_use]
    fn with_standard_presentation_lifetimes(self) -> Self {
        self.try_with_enum_variants(
            EnvironmentBindingId::try_new("PresentationLifetime")
                .expect("presentation lifetime owner identity is valid"),
            TypeKind::Named("PresentationLifetime".to_owned()),
            [
                "frame",
                "tick",
                "cue",
                "line",
                "scene",
                "flow",
                "session",
                "global",
                "persistent",
            ],
        )
        .expect("presentation lifetime inventory is not character nominal")
    }

    /// Installs the finite source-visible runtime callable surface.
    ///
    /// These records are the single source for both standalone type checking
    /// and the immutable core publication accepted by a registered world.
    #[must_use]
    fn with_standard_runtime_callables(self) -> Self {
        let unit_callables = [
            standard_callable_path(["drop"]),
            standard_callable_path(["drop_optional"]),
            standard_callable_path(["on_drop"]),
            standard_callable_path(["adapter", "events"]),
            standard_callable_path(["event", "emit"]),
            standard_callable_path(["scene", "show"]),
            standard_callable_path(["scene", "clear"]),
            standard_callable_path(["progress", "set"]),
            standard_callable_path(["meter", "show"]),
            standard_callable_path(["text", "show"]),
            standard_callable_path(["text", "flush"]),
            standard_callable_path(["voice", "stop"]),
            standard_callable_path(["cues", "stop"]),
        ];
        let env = unit_callables.into_iter().fold(self, |env, path| {
            env.with_typed_standard_function(
                path,
                FunctionSignature::return_only(TypeKind::Unit),
                std::iter::empty::<EffectCapability>(),
            )
        });
        let env = [
            (
                standard_callable_path(["log", "trace"]),
                TypeKind::Unit,
                CallableEvaluatedEffect::Log(CallableLogLevel::Trace),
            ),
            (
                standard_callable_path(["log", "debug"]),
                TypeKind::Unit,
                CallableEvaluatedEffect::Log(CallableLogLevel::Debug),
            ),
            (
                standard_callable_path(["log", "info"]),
                TypeKind::Unit,
                CallableEvaluatedEffect::Log(CallableLogLevel::Info),
            ),
            (
                standard_callable_path(["log", "warn"]),
                TypeKind::Unit,
                CallableEvaluatedEffect::Log(CallableLogLevel::Warn),
            ),
            (
                standard_callable_path(["log", "error"]),
                TypeKind::Unit,
                CallableEvaluatedEffect::Log(CallableLogLevel::Error),
            ),
            (
                standard_callable_path(["signal", "set"]),
                TypeKind::Unit,
                CallableEvaluatedEffect::SignalWrite,
            ),
            (
                standard_callable_path(["metric", "set"]),
                TypeKind::Unit,
                CallableEvaluatedEffect::MetricWrite,
            ),
            (
                standard_callable_path(["ensure"]),
                TypeKind::Unit,
                CallableEvaluatedEffect::Ensure,
            ),
            (
                standard_callable_path(["panic"]),
                TypeKind::Never,
                CallableEvaluatedEffect::Panic,
            ),
            (
                standard_callable_path(["fail"]),
                TypeKind::Never,
                CallableEvaluatedEffect::Fail,
            ),
            (
                standard_callable_path(["bail"]),
                TypeKind::Never,
                CallableEvaluatedEffect::Bail,
            ),
        ]
        .into_iter()
        .fold(env, |env, (path, result, effect)| {
            env.with_typed_standard_evaluated_effect_function(
                path,
                FunctionSignature::return_only(result),
                std::iter::empty::<EffectCapability>(),
                effect,
            )
        });
        [
            (
                standard_callable_path(["load_bg"]),
                TypeKind::Need(Box::new(TypeKind::Result {
                    ok: Box::new(TypeKind::Named("ImageHandle".to_owned())),
                    error: Box::new(TypeKind::Named("ArcError".to_owned())),
                })),
            ),
            (
                standard_callable_path(["asset", "image"]),
                TypeKind::Need(Box::new(TypeKind::Result {
                    ok: Box::new(TypeKind::Named("ImageHandle".to_owned())),
                    error: Box::new(TypeKind::Named("AssetError".to_owned())),
                })),
            ),
            (
                standard_callable_path(["voice", "load"]),
                TypeKind::Need(Box::new(TypeKind::Result {
                    ok: Box::new(TypeKind::Named("VoiceHandle".to_owned())),
                    error: Box::new(TypeKind::Named("VoiceError".to_owned())),
                })),
            ),
            (standard_callable_path(["len"]), TypeKind::I64),
        ]
        .into_iter()
        .fold(env, |env, (path, result)| {
            env.with_typed_standard_function(
                path,
                FunctionSignature::return_only(result),
                std::iter::empty::<EffectCapability>(),
            )
        })
        .with_standard_method(
            TypeKind::Named("VoiceHandle".to_owned()),
            "stop",
            FunctionSignature::return_only(TypeKind::Unit),
        )
        .with_standard_method(
            TypeKind::Named("DialogueText".to_owned()),
            "flush",
            FunctionSignature::return_only(TypeKind::Unit),
        )
    }

    #[must_use]
    fn with_standard_dialogue_view_types(self) -> Self {
        self.with_standard_nominal_record(
            DIALOGUE_CONTENT_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
        )
        .with_standard_nominal_record(
            DIALOGUE_OCCURRENCE_ID_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
        )
        .with_standard_nominal_record(
            DIALOGUE_STAGE_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
        )
        .with_standard_nominal_record(
            DIALOGUE_REVEAL_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
        )
        .with_standard_nominal_record(
            DIALOGUE_ACTION_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
        )
        .with_standard_nominal_record(
            DIALOGUE_CHARACTER_TYPE,
            [
                (
                    DialogueCharacterProjection::Id.field().to_owned(),
                    DialogueCharacterProjection::Id.value_type(),
                ),
                (
                    DialogueCharacterProjection::DisplayName.field().to_owned(),
                    DialogueCharacterProjection::DisplayName.value_type(),
                ),
            ],
        )
        .with_standard_nominal_record(
            STANDARD_DIALOGUE_VIEW_TYPE,
            [
                (
                    "character".to_owned(),
                    TypeKind::Named(DIALOGUE_CHARACTER_TYPE.to_owned()),
                ),
                (
                    DialogueProjectionCoordinate::Content.field().to_owned(),
                    TypeKind::Named(DIALOGUE_CONTENT_TYPE.to_owned()),
                ),
                (
                    DialogueProjectionCoordinate::Occurrence.field().to_owned(),
                    TypeKind::Named(DIALOGUE_OCCURRENCE_ID_TYPE.to_owned()),
                ),
                (
                    DialogueProjectionCoordinate::Stage.field().to_owned(),
                    TypeKind::Named(DIALOGUE_STAGE_TYPE.to_owned()),
                ),
                (
                    DialogueProjectionCoordinate::Reveal.field().to_owned(),
                    TypeKind::Named(DIALOGUE_REVEAL_TYPE.to_owned()),
                ),
                (
                    DialogueProjectionCoordinate::PrimaryAction
                        .field()
                        .to_owned(),
                    TypeKind::Named(DIALOGUE_ACTION_TYPE.to_owned()),
                ),
            ],
        )
        .with_dialogue_view_models(DialogueViewModelRegistry::standard())
    }

    /// Registers one standard nominal record and its typed fields.
    #[must_use]
    fn with_standard_nominal_record(
        mut self,
        name: impl Into<String>,
        fields: impl IntoIterator<Item = (String, TypeKind)>,
    ) -> Self {
        self.insert_standard_nominal_record(name.into(), fields);
        self
    }

    fn insert_standard_nominal_record(
        &mut self,
        name: String,
        fields: impl IntoIterator<Item = (String, TypeKind)>,
    ) {
        let accepted = standard_exact_record(
            &name,
            TypeKind::Named(name.clone()),
            AcceptedNominalOrigin::NominalRecord,
        )
        .expect("nominal record names are valid non-reserved type paths");
        self.nominal_catalog = self
            .nominal_catalog
            .try_with_record(
                accepted,
                crate::nominal::AcceptedNominalCatalogLimits::PRODUCTION,
            )
            .expect("nominal record paths are unique in one semantic environment");
        self.nominal_records.insert(
            name,
            fields
                .into_iter()
                .map(|(name, ty)| (name, normalize_type_kind(ty)))
                .collect(),
        );
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
        self.with_standard_function_effects(
            ["content", "prefetch"],
            FunctionSignature::new(
                TypeKind::Unit,
                [FunctionParam::required("unit", content_ref.clone())],
            ),
            ["content.load"],
        )
        .with_standard_function_effects(
            ["content", "ensure"],
            FunctionSignature::new(
                TypeKind::Need(Box::new(TypeKind::Result {
                    ok: Box::new(TypeKind::Unit),
                    error: Box::new(TypeKind::Named("ContentLoadError".to_owned())),
                })),
                [FunctionParam::required("unit", content_ref.clone())],
            ),
            ["content.load"],
        )
        .with_standard_function_effects(
            ["content", "release"],
            FunctionSignature::new(
                TypeKind::Unit,
                [FunctionParam::required("unit", content_ref)],
            ),
            ["content.release"],
        )
    }

    /// Registers qualified values and expected-type shorthand from the owning
    /// data-format inventory.
    #[must_use]
    fn with_data_format_builtins(self) -> Self {
        self.try_with_enum_variants(
            EnvironmentBindingId::try_new("DataFormat")
                .expect("data-format owner identity is valid"),
            TypeKind::DataFormat,
            DataFormat::ALL.map(DataFormat::variant_name),
        )
        .expect("data-format inventory is not character nominal")
    }

    /// Registers the unit variants available for an enum-like type.
    pub fn try_with_enum_variants(
        mut self,
        owner: EnvironmentBindingId,
        ty: TypeKind,
        variants: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, TypeCheckEnvBuildError> {
        let ty = normalize_type_kind(ty);
        if let TypeKind::CharacterNominal(nominal) = &ty {
            return Err(TypeCheckEnvBuildError::ReservedCharacterNominal {
                nominal: nominal.clone(),
            });
        }
        self.insert_enum_variants(owner, &ty, variants)?;
        Ok(self)
    }

    pub(crate) fn insert_enum_variants(
        &mut self,
        owner: EnvironmentBindingId,
        ty: &TypeKind,
        variants: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<(), TypeCheckEnvBuildError> {
        let schema = self.ensure_closed_enum(owner, ty)?;
        for variant in variants {
            let name = variant.into();
            if schema.variants.iter().any(|variant| variant.name == name) {
                return Err(TypeCheckEnvBuildError::DuplicateEnumVariant {
                    owner: schema.owner.clone(),
                    variant: name,
                });
            }
            schema.variants.push(EnvironmentEnumVariant {
                name,
                payload: EnumVariantPayload::Unit,
            });
        }
        Ok(())
    }

    /// Registers one enum-like variant with its payload contract.
    pub fn try_with_enum_variant_payload(
        mut self,
        owner: EnvironmentBindingId,
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
        let schema = self.ensure_closed_enum(owner, &ty)?;
        let payload = normalize_enum_variant_payload(payload);
        if schema
            .variants
            .iter()
            .any(|existing| existing.name == variant)
        {
            return Err(TypeCheckEnvBuildError::DuplicateEnumVariant {
                owner: schema.owner.clone(),
                variant,
            });
        }
        schema.variants.push(EnvironmentEnumVariant {
            name: variant,
            payload,
        });
        Ok(self)
    }

    fn ensure_closed_enum(
        &mut self,
        owner: EnvironmentBindingId,
        ty: &TypeKind,
    ) -> Result<&mut EnvironmentEnumSchema, TypeCheckEnvBuildError> {
        if let Some((existing, _)) = self
            .closed_enums
            .iter()
            .find(|(_, schema)| schema.owner == owner)
            && existing != ty
        {
            return Err(TypeCheckEnvBuildError::ConflictingEnumOwner {
                owner,
                existing: Box::new(existing.clone()),
                requested: Box::new(ty.clone()),
            });
        }
        let schema = self
            .closed_enums
            .entry(ty.clone())
            .or_insert_with(|| EnvironmentEnumSchema {
                owner: owner.clone(),
                variants: Vec::new(),
            });
        if schema.owner != owner {
            return Err(TypeCheckEnvBuildError::ConflictingEnumTypeOwner {
                ty: Box::new(ty.clone()),
                existing: schema.owner.clone(),
                requested: owner,
            });
        }
        Ok(schema)
    }

    /// Looks up one exact source-visible base-environment binding identity.
    pub fn environment_binding(&self, id: &EnvironmentBindingId) -> Option<&TypeKind> {
        self.symbols.get(id.as_str())
    }

    /// Returns registered enum-like unit variants grouped by semantic type in
    /// deterministic order for tooling surfaces such as LSP completion.
    pub fn enum_variant_sets(&self) -> Vec<(TypeKind, Vec<String>)> {
        let mut sets = self
            .closed_enums
            .iter()
            .map(|(ty, schema)| {
                let variants = schema
                    .variants()
                    .iter()
                    .map(|variant| variant.name().to_owned())
                    .collect::<Vec<_>>();
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

    #[must_use]
    fn with_standard_function<const N: usize>(
        self,
        path: [&str; N],
        signature: FunctionSignature,
    ) -> Self {
        self.with_standard_function_effects(path, signature, std::iter::empty::<EffectCapability>())
    }

    #[must_use]
    fn with_standard_function_effects<const N: usize, I, E>(
        self,
        path: [&str; N],
        signature: FunctionSignature,
        effects: I,
    ) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<EffectCapability>,
    {
        self.with_typed_standard_function(standard_callable_path(path), signature, effects)
    }

    #[must_use]
    fn with_typed_standard_function<I, E>(
        mut self,
        path: CallablePath,
        signature: FunctionSignature,
        effects: I,
    ) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<EffectCapability>,
    {
        let validator = if signature.checks_args() {
            CallableValidator::Ordinary
        } else {
            CallableValidator::Untyped
        };
        assert!(
            self.standard_functions
                .iter()
                .all(|function| function.path != path),
            "standard callable paths are unique"
        );
        self.standard_functions.push(StandardEnvironmentFunction {
            path,
            signature: normalize_function_signature(signature),
            effects: effects.into_iter().map(Into::into).collect(),
            validator,
            evaluated_effect: None,
        });
        self
    }

    #[must_use]
    fn with_typed_standard_evaluated_effect_function<I, E>(
        mut self,
        path: CallablePath,
        signature: FunctionSignature,
        effects: I,
        evaluated_effect: CallableEvaluatedEffect,
    ) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<EffectCapability>,
    {
        let validator = if signature.checks_args() {
            CallableValidator::Ordinary
        } else {
            CallableValidator::Untyped
        };
        assert!(
            self.standard_functions
                .iter()
                .all(|function| function.path != path),
            "standard callable paths are unique"
        );
        self.standard_functions.push(StandardEnvironmentFunction {
            path,
            signature: normalize_function_signature(signature),
            effects: effects.into_iter().map(Into::into).collect(),
            validator,
            evaluated_effect: Some(evaluated_effect),
        });
        self
    }

    #[must_use]
    fn with_standard_method(
        mut self,
        receiver: TypeKind,
        member: &str,
        signature: FunctionSignature,
    ) -> Self {
        let receiver = normalize_type_kind(receiver);
        let member = CallableName::try_new(member)
            .expect("standard method members are valid typed callable names");
        assert!(
            self.standard_methods
                .iter()
                .all(|method| { method.receiver != receiver || method.member != member }),
            "standard method keys are unique"
        );
        self.standard_methods.push(StandardEnvironmentMethod {
            receiver,
            member,
            signature: normalize_function_signature(signature),
        });
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

    pub(crate) fn closed_enum(&self, ty: &TypeKind) -> Option<&EnvironmentEnumSchema> {
        self.closed_enums.get(ty)
    }

    pub(crate) fn closed_enum_by_owner(
        &self,
        owner: &str,
    ) -> Option<(&TypeKind, &EnvironmentEnumSchema)> {
        self.closed_enums
            .iter()
            .find(|(_, schema)| schema.owner().as_str() == owner)
    }

    pub(crate) fn standard_functions(&self) -> &[StandardEnvironmentFunction] {
        &self.standard_functions
    }

    pub(crate) fn standard_methods(&self) -> &[StandardEnvironmentMethod] {
        &self.standard_methods
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

fn standard_callable_path<const N: usize>(segments: [&str; N]) -> CallablePath {
    CallablePath::try_new(
        segments
            .into_iter()
            .map(|segment| {
                CallableName::try_new(segment)
                    .expect("standard callable path segments are valid typed names")
            })
            .collect::<Vec<_>>(),
    )
    .expect("standard callable paths are non-empty and within production limits")
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
        TypeKind::Need(item) => TypeKind::Need(Box::new(normalize_type_kind(*item))),
        TypeKind::Stream { item, error } => TypeKind::Stream {
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
