use super::identity::EnvironmentBindingId;
use super::{
    effects::EffectCapability,
    enums::{EnumVariantPayload, normalize_enum_variant_payload},
    nominal::{
        AcceptedEnvironmentRecordSemantics, AcceptedNominalCatalog, AcceptedNominalOrigin,
        AcceptedNominalRecord, AcceptedOpaqueRuntimeCarrier, standard_environment_record,
        standard_exact_record, standard_runtime_environment_record,
    },
};
use crate::callable::{
    CallableArgumentPolicy, CallableEffectSchema, CallableEvaluatedEffect,
    CallableExtensionReceiver, CallableGenericParameterIssuer, CallableGroupIndex,
    CallableGroupKind, CallableLogLevel, CallableName, CallableOverloadIndex, CallableParameter,
    CallableParameterAdmission, CallableParameterGroup, CallableParameterIndex,
    CallableParameterPassing, CallableParameterPresence, CallablePath, CallableSignatureSchema,
    CallableValidator, DropCallableId, PRODUCTION_CALLABLE_LIMITS, SpreadArgumentPolicy,
    StandardMapFamily, UnknownNamedArgumentPolicy, ViewModifierId,
};
use crate::dialogue_view::{
    DIALOGUE_ACTION_TYPE, DIALOGUE_CHARACTER_TYPE, DIALOGUE_CONTENT_TYPE,
    DIALOGUE_OCCURRENCE_ID_TYPE, DIALOGUE_REVEAL_TYPE, DIALOGUE_STAGE_TYPE,
    DialogueCharacterProjection, DialogueProjectionCoordinate, DialogueRuntimeValueRole,
    DialogueViewModelRegistry, STANDARD_DIALOGUE_VIEW_TYPE,
};
use crate::effect_row::EffectRow;
use crate::types::{
    CharacterNominalType, EntityType, GenericParameterOwnerId, GenericTypeParameterId,
    LanguageIntrinsicGenericOwner, TypeKind, direct_type_name,
};
use arcweft_data::DataFormat;
use arcweft_lang_syntax::{
    ast::{
        module_path::ModulePathRoot,
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment},
    },
    types::FnParamKind,
};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Function or method signature tracked by the semantic environment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionSignature {
    pub(crate) return_type: TypeKind,
    pub(crate) params: Vec<FunctionParam>,
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
    pub(crate) overload: CallableOverloadIndex,
    pub(crate) schema: CallableSignatureSchema,
}

/// Typed standard-environment method input retained until accepted-world publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StandardEnvironmentMethod {
    pub(crate) receiver: TypeKind,
    pub(crate) member: CallableName,
    pub(crate) schema: CallableSignatureSchema,
}

/// Closed standard-registry role for one typed receiver method.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StandardEnvironmentMethodRole {
    Ordinary,
    ViewModifier(ViewModifierId),
}

impl StandardEnvironmentMethodRole {
    pub(crate) const fn validator(self) -> CallableValidator {
        match self {
            Self::Ordinary => CallableValidator::Ordinary,
            Self::ViewModifier(modifier) => CallableValidator::ViewModifier(modifier),
        }
    }
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

/// Closed source-environment value semantics retained with the binding type.
///
/// These are values, not spelling-based compiler intrinsics: resolution keeps
/// the exact environment binding identity and consumers read this payload from
/// the accepted environment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StandardEnvironmentValue {
    DropPolicy(StandardDropPolicyValue),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StandardDropPolicyValue {
    Stop { fade_nanos: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StandardDropPolicyCase {
    Cancel,
    Stop,
    Finish,
    Release,
    Detach,
}

impl StandardEnvironmentValue {
    pub(crate) fn for_binding(id: &EnvironmentBindingId) -> Option<Self> {
        (id == &stop_now_binding()).then_some(Self::DropPolicy(StandardDropPolicyValue::Stop {
            fade_nanos: 0,
        }))
    }
}

impl StandardDropPolicyCase {
    const ALL: [Self; 5] = [
        Self::Cancel,
        Self::Stop,
        Self::Finish,
        Self::Release,
        Self::Detach,
    ];

    pub(crate) fn for_owner_ordinal(owner: &EnvironmentBindingId, ordinal: u32) -> Option<Self> {
        if owner != &drop_policy_owner() {
            return None;
        }
        Self::ALL.get(usize::try_from(ordinal).ok()?).copied()
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Cancel => "Cancel",
            Self::Stop => "Stop",
            Self::Finish => "Finish",
            Self::Release => "Release",
            Self::Detach => "Detach",
        }
    }

    fn payload(self) -> EnumVariantPayload {
        match self {
            Self::Stop => EnumVariantPayload::record([("fade", TypeKind::Duration)]),
            Self::Cancel | Self::Finish | Self::Release | Self::Detach => EnumVariantPayload::Unit,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EnvironmentValueBinding {
    ty: TypeKind,
    standard: Option<StandardEnvironmentValue>,
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
    pub(crate) symbols: HashMap<String, EnvironmentValueBinding>,
    closed_enums: HashMap<TypeKind, EnvironmentEnumSchema>,
    pub(crate) standard_functions: Vec<StandardEnvironmentFunction>,
    pub(crate) standard_methods: Vec<StandardEnvironmentMethod>,
    pub(crate) capabilities: HashSet<EffectCapability>,
    pub(crate) available_effects: Option<HashSet<EffectCapability>>,
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
    pub fn function_value_type(&self) -> TypeKind {
        TypeKind::function(
            self.params.iter().map(|param| param.ty.clone()),
            self.return_type.clone(),
        )
    }

    /// Type of this callable when referenced as a function value with a known
    /// effect row.
    pub fn function_value_type_with_effects(&self, effects: EffectRow) -> TypeKind {
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
        Self::default()
            .with_standard_accepted_nominals()
            .with_standard_runtime_value_enums()
            .with_standard_runtime_callables()
    }

    /// Creates the standard source type-checking environment.
    pub fn standard() -> Self {
        Self::new().with_standard_builtins()
    }

    /// Registers builtins that are available to ordinary Arcweft source files.
    #[must_use]
    fn with_standard_builtins(self) -> Self {
        self.with_standard_presentation_nominals()
            .with_standard_dialogue_view_types()
            .with_standard_presentation_lifetimes()
            .with_standard_dialogue_value_enums()
            .with_standard_agent_enums()
            .with_typed_standard_schema(standard_callable_path(["fmt"]), fmt_schema())
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
                        FunctionParam::required("value", TypeKind::AgentValue),
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
                    [FunctionParam::required("value", TypeKind::AgentValue)],
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
            (
                "AgentBinaryEncoding",
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::AgentBinaryEncoding),
                &["Base64"][..],
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
        .try_with_enum_variant_payload(
            EnvironmentBindingId::try_new("AgentResourceBody")
                .expect("Agent resource body owner identity is valid"),
            TypeKind::AgentResourceBody,
            "Json",
            EnumVariantPayload::Tuple(vec![TypeKind::AgentValue]),
        )
        .and_then(|environment| {
            environment.try_with_enum_variant_payload(
                EnvironmentBindingId::try_new("AgentResourceBody")
                    .expect("Agent resource body owner identity is valid"),
                TypeKind::AgentResourceBody,
                "Text",
                EnumVariantPayload::Tuple(vec![TypeKind::String]),
            )
        })
        .and_then(|environment| {
            environment.try_with_enum_variant_payload(
                EnvironmentBindingId::try_new("AgentResourceBody")
                    .expect("Agent resource body owner identity is valid"),
                TypeKind::AgentResourceBody,
                "BytesBase64",
                EnumVariantPayload::Tuple(vec![TypeKind::AgentBuiltin(
                    crate::types::AgentBuiltinType::AgentBinaryBody,
                )]),
            )
        })
        .expect("Agent resource body variants have one canonical typed schema")
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

    #[must_use]
    fn with_standard_dialogue_value_enums(self) -> Self {
        self.try_with_enum_variants(
            EnvironmentBindingId::try_new("DialogueVoice")
                .expect("DialogueVoice owner identity is valid"),
            TypeKind::Named("DialogueVoice".to_owned()),
            ["auto"],
        )
        .expect("DialogueVoice has one canonical source-visible enum inventory")
    }

    #[must_use]
    fn with_standard_runtime_value_enums(self) -> Self {
        let environment = self
            .try_with_enum_variants(
                EnvironmentBindingId::try_new("TextFlushMode")
                    .expect("text flush mode owner identity is valid"),
                TypeKind::Named("TextFlushMode".to_owned()),
                ["Instant"],
            )
            .expect("TextFlushMode has one canonical source-visible enum inventory")
            .try_with_enum_variants(
                EnvironmentBindingId::try_new("CueStopPolicy")
                    .expect("cue stop policy owner identity is valid"),
                TypeKind::Named("CueStopPolicy".to_owned()),
                ["CancelPending", "CompleteCurrent", "SnapToFinal"],
            )
            .expect("CueStopPolicy has one canonical source-visible enum inventory");
        let environment =
            StandardDropPolicyCase::ALL
                .into_iter()
                .fold(environment, |environment, case| {
                    environment
                        .try_with_enum_variant_payload(
                            drop_policy_owner(),
                            drop_policy_type(),
                            case.name(),
                            case.payload(),
                        )
                        .expect("DropPolicy case inventory is canonical")
                });
        environment
            .with_standard_value(
                "stop_now",
                drop_policy_type(),
                StandardEnvironmentValue::DropPolicy(StandardDropPolicyValue::Stop {
                    fade_nanos: 0,
                }),
            )
            .with_symbol("keep_running", TypeKind::Named("CueStopPolicy".to_owned()))
    }

    /// Installs the finite source-visible runtime callable surface.
    ///
    /// These records are the single source for both standalone type checking
    /// and the immutable core publication accepted by a registered world.
    #[must_use]
    fn with_standard_runtime_callables(self) -> Self {
        let env = StandardMapFamily::PUBLISHED
            .into_iter()
            .fold(self, |environment, family| {
                environment.with_typed_standard_overload_schema(
                    standard_callable_path(["map"]),
                    family.overload(),
                    family.signature_schema(),
                )
            });
        let env = env
            .with_typed_standard_overload_schema(
                standard_callable_path(["drop"]),
                standard_overload(0),
                drop_schema(DropCallableId::Drop),
            )
            .with_typed_standard_overload_schema(
                standard_callable_path(["drop"]),
                standard_overload(1),
                drop_schema(DropCallableId::DropWithPolicy),
            )
            .with_typed_standard_schema(
                standard_callable_path(["drop_optional"]),
                drop_schema(DropCallableId::DropOptional),
            )
            .with_typed_standard_schema(
                standard_callable_path(["on_drop"]),
                drop_schema(DropCallableId::OnDrop),
            )
            .with_standard_function(
                ["scene", "show"],
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required(
                        "scene",
                        TypeKind::entity_ref(crate::types::EntityKind::Scene),
                    )],
                ),
            )
            .with_standard_function(
                ["scene", "clear"],
                FunctionSignature::new(TypeKind::Unit, std::iter::empty::<FunctionParam>()),
            )
            .with_standard_function(
                ["progress", "set"],
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required("ratio", TypeKind::F32)],
                ),
            )
            .with_standard_function(
                ["meter", "show"],
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required(
                        "source",
                        TypeKind::entity_ref_with_value(
                            crate::types::EntityKind::Signal,
                            TypeKind::F32,
                        ),
                    )],
                ),
            )
            .with_standard_function(
                ["text", "show"],
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::required(
                        "text",
                        TypeKind::Choice(vec![TypeKind::String, TypeKind::DisplayText]),
                    )],
                ),
            )
            .with_standard_function(
                ["text", "flush"],
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::defaulted(
                        "mode",
                        TypeKind::Named("TextFlushMode".to_owned()),
                    )],
                ),
            )
            .with_standard_function(
                ["voice", "stop"],
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::defaulted("fade", TypeKind::Duration)],
                ),
            )
            .with_standard_function(
                ["cues", "stop"],
                FunctionSignature::new(
                    TypeKind::Unit,
                    [FunctionParam::defaulted(
                        "policy",
                        TypeKind::Named("CueStopPolicy".to_owned()),
                    )],
                ),
            );
        let env = [
            CallableLogLevel::Trace,
            CallableLogLevel::Debug,
            CallableLogLevel::Info,
            CallableLogLevel::Warn,
            CallableLogLevel::Error,
        ]
        .into_iter()
        .fold(env, |env, level| {
            env.with_typed_standard_schema(
                standard_callable_path(["log", level.as_str()]),
                evaluated_log_schema(level),
            )
        })
        .with_typed_standard_schema(
            standard_callable_path(["signal", "set"]),
            evaluated_entity_write_schema(
                crate::types::EntityKind::Signal,
                crate::types::LanguageIntrinsicGenericOwner::SignalWrite,
                CallableEvaluatedEffect::SignalWrite,
            ),
        )
        .with_typed_standard_schema(
            standard_callable_path(["metric", "set"]),
            evaluated_entity_write_schema(
                crate::types::EntityKind::Metric,
                crate::types::LanguageIntrinsicGenericOwner::MetricWrite,
                CallableEvaluatedEffect::MetricWrite,
            ),
        );
        let env = env.with_typed_standard_function(
            standard_callable_path(["load_bg"]),
            FunctionSignature::new(
                TypeKind::Need(Box::new(TypeKind::Result {
                    ok: Box::new(TypeKind::Named("ImageHandle".to_owned())),
                    error: Box::new(TypeKind::Named("ArcError".to_owned())),
                })),
                std::iter::empty::<FunctionParam>(),
            ),
            std::iter::empty::<EffectCapability>(),
        );
        env.with_standard_function(
            ["asset", "image"],
            FunctionSignature::new(
                TypeKind::Need(Box::new(TypeKind::Result {
                    ok: Box::new(TypeKind::Named("ImageHandle".to_owned())),
                    error: Box::new(TypeKind::Named("AssetError".to_owned())),
                })),
                [FunctionParam::required(
                    "asset",
                    TypeKind::entity_ref(crate::types::EntityKind::Asset),
                )],
            ),
        )
        .with_standard_function(
            ["voice", "load"],
            FunctionSignature::new(
                TypeKind::Need(Box::new(TypeKind::Result {
                    ok: Box::new(TypeKind::VoiceHandle),
                    error: Box::new(TypeKind::Named("VoiceError".to_owned())),
                })),
                [FunctionParam::required(
                    "voice",
                    TypeKind::entity_ref(crate::types::EntityKind::Voice),
                )],
            ),
        )
        .with_standard_method(
            TypeKind::VoiceHandle,
            "stop",
            FunctionSignature::new(
                TypeKind::Unit,
                [FunctionParam::defaulted("fade", TypeKind::Duration)],
            ),
        )
        .with_standard_method(
            TypeKind::Named("DialogueText".to_owned()),
            "flush",
            FunctionSignature::new(
                TypeKind::Unit,
                [FunctionParam::defaulted(
                    "mode",
                    TypeKind::Named("TextFlushMode".to_owned()),
                )],
            ),
        )
        .with_standard_view_modifier(ViewModifierId::OnActivate)
    }

    #[must_use]
    fn with_standard_dialogue_view_types(self) -> Self {
        self.with_standard_runtime_nominal_record(
            DIALOGUE_CONTENT_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
            DialogueRuntimeValueRole::Content,
        )
        .with_standard_runtime_nominal_record(
            DIALOGUE_OCCURRENCE_ID_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
            DialogueRuntimeValueRole::Occurrence,
        )
        .with_standard_runtime_nominal_record(
            DIALOGUE_STAGE_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
            DialogueRuntimeValueRole::Stage,
        )
        .with_standard_runtime_nominal_record(
            DIALOGUE_REVEAL_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
            DialogueRuntimeValueRole::Reveal,
        )
        .with_standard_runtime_nominal_record(
            DIALOGUE_ACTION_TYPE,
            std::iter::empty::<(String, TypeKind)>(),
            DialogueRuntimeValueRole::Action,
        )
        .with_standard_runtime_nominal_record(
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
            DialogueRuntimeValueRole::Character,
        )
        .with_standard_runtime_nominal_record(
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
            DialogueRuntimeValueRole::View,
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

    #[must_use]
    fn with_standard_runtime_nominal_record(
        mut self,
        name: impl Into<String>,
        fields: impl IntoIterator<Item = (String, TypeKind)>,
        role: DialogueRuntimeValueRole,
    ) -> Self {
        let name = name.into();
        let record = standard_runtime_environment_record(
            &name,
            fields,
            AcceptedOpaqueRuntimeCarrier::new(
                role.producer(),
                role.value_class(),
                role.persistence(),
            ),
        )
        .expect("standard runtime environment records are valid");
        self.nominal_catalog = self
            .nominal_catalog
            .try_with_record(
                record,
                crate::nominal::AcceptedNominalCatalogLimits::PRODUCTION,
            )
            .expect("standard runtime environment record identities are unique");
        self
    }

    fn insert_standard_nominal_record(
        &mut self,
        name: String,
        fields: impl IntoIterator<Item = (String, TypeKind)>,
    ) {
        let accepted = standard_environment_record(&name, fields)
            .expect("nominal record names are valid non-reserved type paths");
        self.nominal_catalog = self
            .nominal_catalog
            .try_with_record(
                accepted,
                crate::nominal::AcceptedNominalCatalogLimits::PRODUCTION,
            )
            .expect("nominal record paths are unique in one semantic environment");
    }

    /// Exact accepted environment record selected from the sole nominal
    /// catalog authority.
    pub fn environment_record(&self, name: &str) -> Option<&AcceptedEnvironmentRecordSemantics> {
        self.accepted_environment_record(name)
            .and_then(AcceptedNominalRecord::environment_record)
    }

    /// Exact accepted owner row for one direct environment record name.
    pub(crate) fn accepted_environment_record(&self, name: &str) -> Option<&AcceptedNominalRecord> {
        let segment = ProjectSymbolSegment::try_new(name).ok()?;
        let path = ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, [segment]).ok()?;
        self.nominal_catalog
            .exact(&path.into())
            .filter(|record| record.environment_record().is_some())
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
        self.symbols.get(id.as_str()).map(|binding| &binding.ty)
    }

    pub(crate) fn standard_environment_value(
        &self,
        id: &EnvironmentBindingId,
    ) -> Option<StandardEnvironmentValue> {
        let stored = self.symbols.get(id.as_str())?.standard?;
        (StandardEnvironmentValue::for_binding(id) == Some(stored)).then_some(stored)
    }

    pub(crate) fn standard_drop_policy_case(
        &self,
        owner: &EnvironmentBindingId,
        ordinal: u32,
    ) -> Option<StandardDropPolicyCase> {
        let case = StandardDropPolicyCase::for_owner_ordinal(owner, ordinal)?;
        let schema = self.closed_enums.get(&drop_policy_type())?;
        if schema.owner() != owner {
            return None;
        }
        let variant = schema.variants().get(usize::try_from(ordinal).ok()?)?;
        (variant.name() == case.name() && variant.payload() == &case.payload()).then_some(case)
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
        self.symbols.insert(
            name.into(),
            EnvironmentValueBinding {
                ty: normalize_type_kind(ty),
                standard: None,
            },
        );
        self
    }

    #[must_use]
    fn with_standard_value(
        mut self,
        name: impl Into<String>,
        ty: TypeKind,
        value: StandardEnvironmentValue,
    ) -> Self {
        let previous = self.symbols.insert(
            name.into(),
            EnvironmentValueBinding {
                ty: normalize_type_kind(ty),
                standard: Some(value),
            },
        );
        assert!(
            previous.is_none(),
            "standard environment value identities are unique"
        );
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
        self,
        path: CallablePath,
        signature: FunctionSignature,
        effects: I,
    ) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<EffectCapability>,
    {
        let signature = self.canonical_standard_callable_signature(signature);
        let effects = crate::effects::EffectSet::from_labels(
            effects
                .into_iter()
                .map(Into::into)
                .map(|effect: EffectCapability| effect.as_str().to_owned()),
        )
        .expect("standard callable effect labels are canonical");
        let schema = signature
            .callable_schema(
                EffectRow::closed(effects),
                CallableValidator::Ordinary,
                CallableGenericParameterIssuer::empty(),
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("standard function signature is a valid checked callable schema");
        self.with_typed_standard_schema(path, schema)
    }

    #[must_use]
    fn with_typed_standard_schema(
        self,
        path: CallablePath,
        schema: CallableSignatureSchema,
    ) -> Self {
        self.with_typed_standard_overload_schema(
            path,
            CallableOverloadIndex::try_from_usize(0)
                .expect("zero standard overload is representable"),
            schema,
        )
    }

    #[must_use]
    fn with_typed_standard_overload_schema(
        mut self,
        path: CallablePath,
        overload: CallableOverloadIndex,
        schema: CallableSignatureSchema,
    ) -> Self {
        assert!(
            self.standard_functions
                .iter()
                .all(|function| function.path != path || function.overload != overload),
            "standard callable path/overload identities are unique"
        );
        self.standard_functions.push(StandardEnvironmentFunction {
            path,
            overload,
            schema,
        });
        self
    }

    #[must_use]
    fn with_standard_method(
        self,
        receiver: TypeKind,
        member: &str,
        signature: FunctionSignature,
    ) -> Self {
        self.with_standard_method_role(
            receiver,
            CallableName::try_new(member)
                .expect("standard method members are valid typed callable names"),
            signature,
            StandardEnvironmentMethodRole::Ordinary,
        )
    }

    fn with_standard_method_role(
        mut self,
        receiver: TypeKind,
        member: CallableName,
        signature: FunctionSignature,
        role: StandardEnvironmentMethodRole,
    ) -> Self {
        let receiver = self.canonical_standard_callable_type(receiver);
        let signature = self.canonical_standard_callable_signature(signature);
        let schema = signature
            .callable_schema(
                EffectRow::closed(crate::effects::EffectSet::new()),
                role.validator(),
                CallableGenericParameterIssuer::empty(),
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("standard method signature is a valid checked callable schema");
        assert!(
            self.standard_methods
                .iter()
                .all(|method| { method.receiver != receiver || method.member != member }),
            "standard method keys are unique"
        );
        self.standard_methods.push(StandardEnvironmentMethod {
            receiver,
            member,
            schema,
        });
        self
    }

    #[must_use]
    fn with_standard_view_modifier(self, modifier: ViewModifierId) -> Self {
        self.with_standard_method_role(
            modifier.receiver(),
            modifier.member(),
            modifier.signature(),
            StandardEnvironmentMethodRole::ViewModifier(modifier),
        )
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

    /// Canonicalizes one standard callable signature against this environment's
    /// accepted nominal catalog at publication time.
    ///
    /// Core runtime callables are installed before the complete standard
    /// nominal inventory exists. Publication is therefore the first boundary
    /// where their internal `Named` atoms can be joined to the exact accepted
    /// identities without introducing a second name-based resolver.
    pub(crate) fn canonical_standard_callable_signature(
        &self,
        mut signature: FunctionSignature,
    ) -> FunctionSignature {
        signature.return_type = self.canonical_standard_callable_type(signature.return_type);
        signature.params = signature
            .params
            .into_iter()
            .map(|parameter| self.canonical_standard_callable_parameter(parameter))
            .collect();
        signature.remaining_param_groups = signature
            .remaining_param_groups
            .into_iter()
            .map(|group| {
                group
                    .into_iter()
                    .map(|parameter| self.canonical_standard_callable_parameter(parameter))
                    .collect()
            })
            .collect();
        signature
    }

    pub(crate) fn canonical_standard_callable_type(&self, ty: TypeKind) -> TypeKind {
        map_named_type_kind(ty, &|name| {
            self.nominal_catalog
                .exact_records()
                .find(|record| {
                    direct_type_name(record.id().canonical_path()) == Some(name.as_str())
                })
                .and_then(|record| record.try_instantiate([]).ok())
                .unwrap_or(TypeKind::Named(name))
        })
    }

    fn canonical_standard_callable_parameter(&self, mut parameter: FunctionParam) -> FunctionParam {
        parameter.ty = self.canonical_standard_callable_type(parameter.ty);
        parameter.higher_order_bindings = parameter
            .higher_order_bindings
            .into_iter()
            .map(|mut binding| {
                binding.ty = self.canonical_standard_callable_type(binding.ty);
                binding
            })
            .collect();
        parameter
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

fn fmt_schema() -> CallableSignatureSchema {
    standard_schema(
        vec![vec![standard_parameter(
            0,
            "value",
            CallableParameterAdmission::unchecked_supply(),
            CallableParameterPassing::PositionalOrNamed,
            CallableParameterPresence::Required,
        )]],
        TypeKind::DisplayText,
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::OpenSupply,
            SpreadArgumentPolicy::FixedLiteralOnly,
        ),
        CallableValidator::Ordinary,
        CallableGenericParameterIssuer::empty(),
    )
}

fn evaluated_log_schema(level: CallableLogLevel) -> CallableSignatureSchema {
    standard_schema(
        vec![vec![standard_parameter(
            0,
            "message",
            CallableParameterAdmission::unchecked_supply(),
            CallableParameterPassing::PositionalOrNamed,
            CallableParameterPresence::Required,
        )]],
        TypeKind::Unit,
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::OpenSupply,
            SpreadArgumentPolicy::FixedLiteralOnly,
        ),
        CallableValidator::Ordinary,
        CallableGenericParameterIssuer::empty(),
    )
    .with_evaluated_effect(CallableEvaluatedEffect::Log(level))
}

fn evaluated_entity_write_schema(
    entity: crate::types::EntityKind,
    owner: LanguageIntrinsicGenericOwner,
    effect: CallableEvaluatedEffect,
) -> CallableSignatureSchema {
    let value = language_intrinsic_generic(owner);
    standard_schema(
        vec![vec![
            standard_parameter(
                0,
                "target",
                CallableParameterAdmission::checked(TypeKind::entity_ref_with_value(
                    entity,
                    value.clone(),
                )),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
            ),
            standard_parameter(
                1,
                "value",
                CallableParameterAdmission::checked(value),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
            ),
        ]],
        TypeKind::Unit,
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::FixedLiteralOnly,
        ),
        CallableValidator::Ordinary,
        CallableGenericParameterIssuer::language_intrinsic(owner, 1, 0)
            .expect("entity write generic owner has one typed parameter"),
    )
    .with_evaluated_effect(effect)
}

fn drop_schema(id: DropCallableId) -> CallableSignatureSchema {
    let owner = drop_generic_owner(id);
    let value = language_intrinsic_generic(owner);
    let (groups, result, receiver_group) = match id {
        DropCallableId::Drop => (
            vec![drop_value_group(value)],
            TypeKind::Unit,
            CallableGroupIndex::ZERO,
        ),
        DropCallableId::DropWithPolicy => (
            vec![drop_policy_group(), drop_value_group(value)],
            TypeKind::Unit,
            second_callable_group(),
        ),
        DropCallableId::DropOptional => (
            vec![drop_value_group(TypeKind::Option(Box::new(value)))],
            TypeKind::Unit,
            CallableGroupIndex::ZERO,
        ),
        DropCallableId::OnDrop => (
            vec![drop_policy_group(), drop_value_group(value.clone())],
            value,
            second_callable_group(),
        ),
    };
    let schema = standard_schema(
        groups,
        result,
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::FixedLiteralOnly,
        ),
        CallableValidator::Drop(id),
        CallableGenericParameterIssuer::language_intrinsic(owner, 1, 0)
            .expect("drop generic owner has one typed parameter"),
    )
    .with_extension_receiver(CallableExtensionReceiver::new(
        receiver_group,
        zero_parameter_index(),
    ))
    .expect("drop receiver is a canonical explicit receiver coordinate");
    match id {
        DropCallableId::Drop | DropCallableId::DropWithPolicy | DropCallableId::DropOptional => {
            schema.with_evaluated_effect(CallableEvaluatedEffect::Drop(id))
        }
        DropCallableId::OnDrop => schema,
    }
}

const fn drop_generic_owner(id: DropCallableId) -> LanguageIntrinsicGenericOwner {
    match id {
        DropCallableId::Drop => LanguageIntrinsicGenericOwner::Drop,
        DropCallableId::DropWithPolicy => LanguageIntrinsicGenericOwner::DropWithPolicy,
        DropCallableId::DropOptional => LanguageIntrinsicGenericOwner::DropOptional,
        DropCallableId::OnDrop => LanguageIntrinsicGenericOwner::OnDrop,
    }
}

fn drop_policy_group() -> Vec<CallableParameter> {
    vec![standard_parameter(
        0,
        "policy",
        CallableParameterAdmission::checked(drop_policy_type()),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
    )]
}

fn drop_value_group(value: TypeKind) -> Vec<CallableParameter> {
    vec![standard_parameter(
        0,
        "value",
        CallableParameterAdmission::checked(value),
        CallableParameterPassing::PositionalOnly,
        CallableParameterPresence::Required,
    )]
}

fn second_callable_group() -> CallableGroupIndex {
    CallableGroupIndex::try_from_usize(1).expect("second callable group is representable")
}

fn zero_parameter_index() -> CallableParameterIndex {
    CallableParameterIndex::try_from_usize(0).expect("zero parameter index is representable")
}

fn drop_policy_type() -> TypeKind {
    TypeKind::Named("DropPolicy".to_owned())
}

fn drop_policy_owner() -> EnvironmentBindingId {
    EnvironmentBindingId::try_new("DropPolicy").expect("drop policy owner identity is valid")
}

fn stop_now_binding() -> EnvironmentBindingId {
    EnvironmentBindingId::try_new("stop_now").expect("stop_now binding identity is valid")
}

fn standard_overload(index: usize) -> CallableOverloadIndex {
    CallableOverloadIndex::try_from_usize(index).expect("standard overload index is representable")
}

fn language_intrinsic_generic(owner: LanguageIntrinsicGenericOwner) -> TypeKind {
    TypeKind::GenericParam(GenericTypeParameterId::new(
        GenericParameterOwnerId::LanguageIntrinsic(owner),
        0,
    ))
}

fn standard_schema(
    groups: Vec<Vec<CallableParameter>>,
    result: TypeKind,
    argument_policy: CallableArgumentPolicy,
    validator: CallableValidator,
    generic_issuer: CallableGenericParameterIssuer,
) -> CallableSignatureSchema {
    let groups = groups
        .into_iter()
        .enumerate()
        .map(|(index, parameters)| {
            let index = CallableGroupIndex::try_from_usize(index)
                .expect("standard callable group index is representable");
            CallableParameterGroup::try_new(
                index,
                if index == CallableGroupIndex::ZERO {
                    CallableGroupKind::Initial
                } else {
                    CallableGroupKind::Curried
                },
                parameters,
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("standard callable parameter group is valid")
        })
        .collect();
    CallableSignatureSchema::try_new(
        groups,
        result,
        CallableEffectSchema::fixed(EffectRow::closed(crate::effects::EffectSet::new())),
        argument_policy,
        validator,
        generic_issuer,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("standard callable schema is valid")
}

fn standard_parameter(
    index: usize,
    name: &str,
    admission: CallableParameterAdmission,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
) -> CallableParameter {
    CallableParameter::try_new(
        CallableParameterIndex::try_from_usize(index)
            .expect("standard parameter index is representable"),
        Some(CallableName::try_new(name).expect("standard parameter name is valid")),
        admission,
        passing,
        presence,
        None,
        None,
    )
    .expect("standard parameter schema is valid")
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
    map_named_type_kind(ty, &|name| {
        TypeKind::primitive_name(&name).unwrap_or(TypeKind::Named(name))
    })
}

fn map_named_type_kind(ty: TypeKind, resolve_named: &impl Fn(String) -> TypeKind) -> TypeKind {
    match ty {
        TypeKind::Named(name) => resolve_named(name),
        TypeKind::Ref(entity) => TypeKind::Ref(EntityType::new(
            entity.kind().clone(),
            entity
                .value()
                .cloned()
                .map(|value| map_named_type_kind(value, resolve_named)),
        )),
        TypeKind::Probe(inner) => {
            TypeKind::Probe(Box::new(map_named_type_kind(*inner, resolve_named)))
        }
        TypeKind::Vec(inner) => TypeKind::Vec(Box::new(map_named_type_kind(*inner, resolve_named))),
        TypeKind::Array { item, len } => TypeKind::Array {
            item: Box::new(map_named_type_kind(*item, resolve_named)),
            len,
        },
        TypeKind::Slice(inner) => {
            TypeKind::Slice(Box::new(map_named_type_kind(*inner, resolve_named)))
        }
        TypeKind::Seq(inner) => TypeKind::Seq(Box::new(map_named_type_kind(*inner, resolve_named))),
        TypeKind::Map { kind, key, value } => TypeKind::Map {
            kind,
            key: Box::new(map_named_type_kind(*key, resolve_named)),
            value: Box::new(map_named_type_kind(*value, resolve_named)),
        },
        TypeKind::BorrowRef {
            kind,
            lifetime,
            inner,
        } => TypeKind::BorrowRef {
            kind,
            lifetime,
            inner: Box::new(map_named_type_kind(*inner, resolve_named)),
        },
        TypeKind::Need(item) => TypeKind::Need(Box::new(map_named_type_kind(*item, resolve_named))),
        TypeKind::Stream { item, error } => TypeKind::Stream {
            item: Box::new(map_named_type_kind(*item, resolve_named)),
            error: Box::new(map_named_type_kind(*error, resolve_named)),
        },
        TypeKind::Result { ok, error } => TypeKind::Result {
            ok: Box::new(map_named_type_kind(*ok, resolve_named)),
            error: Box::new(map_named_type_kind(*error, resolve_named)),
        },
        TypeKind::Option(inner) => {
            TypeKind::Option(Box::new(map_named_type_kind(*inner, resolve_named)))
        }
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
            TypeKind::ThreadHandle(Box::new(map_named_type_kind(*inner, resolve_named)))
        }
        TypeKind::Shared(inner) => {
            TypeKind::Shared(Box::new(map_named_type_kind(*inner, resolve_named)))
        }
        TypeKind::Function {
            params,
            return_type,
            effects,
        } => TypeKind::function_with_effects(
            params
                .into_iter()
                .map(|parameter| map_named_type_kind(parameter, resolve_named)),
            map_named_type_kind(*return_type, resolve_named),
            effects,
        ),
        TypeKind::Projection {
            subject,
            trait_name,
            assoc,
        } => TypeKind::Projection {
            subject: Box::new(map_named_type_kind(*subject, resolve_named)),
            trait_name,
            assoc,
        },
        TypeKind::Tuple(items) => TypeKind::Tuple(
            items
                .into_iter()
                .map(|item| map_named_type_kind(item, resolve_named))
                .collect(),
        ),
        TypeKind::Choice(alternatives) => TypeKind::Choice(
            alternatives
                .into_iter()
                .map(|alternative| map_named_type_kind(alternative, resolve_named))
                .collect(),
        ),
        other => other,
    }
}
