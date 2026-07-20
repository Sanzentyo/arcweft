//! Closed family schemas used by the shared callable resolver.

use arcweft_character::id::CharacterId;
use arcweft_lang_syntax::types::{TypeRef, parse_type_ref};

use crate::{
    effect_row::EffectRow,
    effects::EffectSet,
    env::EnumVariantPayload,
    types::{EntityKind, MapKind, TypeKind},
};

use super::{
    CallableArgumentPolicy, CallableEffectSchema, CallableGroupKind, CallableParameter,
    CallableParameterGroup, CallableParameterPassing, CallableParameterPresence,
    CallableParameterType, CallableSignatureSchema, CallableValidator, SpreadArgumentPolicy,
    UnknownNamedArgumentPolicy,
};
use crate::callable::PromotionCallableId;
use crate::callable::{
    AgentIntrinsicSignatureId, BuiltinCallableId, CallableName, CallableParameterIndex,
    CallableSchemaError, CapabilityCallableId, CapacityMethodId, CollectionMethodId,
    DialogueCallableId, DialogueCalleeIdentity, DomainMethodId, DropCallableId,
    EnumVariantSignatureId, FloatWidth, FxCallableSignatureId, IntegerMethodId, MathCallableId,
    OptionConstructorKind, PRODUCTION_CALLABLE_LIMITS, PresentationArgumentValuePolicy,
    PresentationCallableId, PresentationHandleMethodId, ReductionConstructorKind,
    ResolvedCharacterOwner, ResultConstructorKind, SpeakerCallableId, StdFloatCallableId,
    StdFloatOperation, VectorDimensions,
};

impl BuiltinCallableId {
    pub fn signature_schema(&self) -> CallableSignatureSchema {
        let validator = CallableValidator::Builtin(self.clone());
        match self {
            Self::InlineFailureFallback => {
                variadic_unchecked(TypeKind::Named("InlineFailure".to_owned()), validator, &[])
            }
            Self::Panic | Self::Fail | Self::Bail => {
                variadic_unchecked(TypeKind::Never, validator, &[])
            }
            Self::Ensure | Self::Assert | Self::DebugAssert => schema(
                vec![
                    parameter(
                        0,
                        Some("condition"),
                        CallableParameterType::Exact(TypeKind::Bool),
                        CallableParameterPassing::PositionalOrNamed,
                        CallableParameterPresence::Required,
                    ),
                    parameter(
                        1,
                        Some("details"),
                        CallableParameterType::Unchecked,
                        CallableParameterPassing::RestPositional,
                        CallableParameterPresence::Optional,
                    ),
                ],
                TypeKind::Unit,
                &[],
                open_unchecked(),
                validator,
            ),
            Self::Rgb => homogeneous(1, &TypeKind::String, named("Color"), validator),
            Self::Sin | Self::Cos => homogeneous(1, &TypeKind::F32, TypeKind::F32, validator),
            Self::Vector { dimensions } => {
                let arity = match dimensions {
                    VectorDimensions::Two => 2,
                    VectorDimensions::Three => 3,
                    VectorDimensions::Four => 4,
                };
                homogeneous(
                    arity,
                    &TypeKind::F32,
                    named(&format!("Vec{arity}")),
                    validator,
                )
            }
            Self::Math(id) => {
                let ty = match id {
                    MathCallableId::MatMulF32 | MathCallableId::MatrixAddF32 => "MatrixF32",
                    MathCallableId::MatMulF64 | MathCallableId::MatrixAddF64 => "MatrixF64",
                    MathCallableId::TensorAddF32 => "TensorF32",
                    MathCallableId::TensorAddF64 => "TensorF64",
                };
                let ty = named(ty);
                homogeneous(2, &ty, ty.clone(), validator)
            }
            Self::StdFloat(id) => std_float_schema(*id, validator),
            Self::Capability(CapabilityCallableId::EventEmit) => {
                variadic_unchecked(TypeKind::Unit, validator, &[])
            }
            Self::Reduction(kind) => kind.signature_schema(),
        }
    }
}

fn std_float_schema(
    id: StdFloatCallableId,
    validator: CallableValidator,
) -> CallableSignatureSchema {
    let width = match id.width() {
        FloatWidth::F32 => TypeKind::F32,
        FloatWidth::F64 => TypeKind::F64,
    };
    let bits = match id.width() {
        FloatWidth::F32 => TypeKind::U32,
        FloatWidth::F64 => TypeKind::U64,
    };
    let (arity, input, result) = match id.operation() {
        StdFloatOperation::Powf | StdFloatOperation::Atan2 => (2, width.clone(), width.clone()),
        StdFloatOperation::MulAdd => (3, width.clone(), width.clone()),
        StdFloatOperation::IsNan
        | StdFloatOperation::IsInfinite
        | StdFloatOperation::IsFinite
        | StdFloatOperation::IsSignPositive
        | StdFloatOperation::IsSignNegative => (1, width.clone(), TypeKind::Bool),
        StdFloatOperation::ToBits => (1, width.clone(), bits.clone()),
        StdFloatOperation::FromBits => (1, bits, width.clone()),
        StdFloatOperation::ToF32 => (1, width.clone(), TypeKind::F32),
        StdFloatOperation::ToF64 => (1, width.clone(), TypeKind::F64),
        StdFloatOperation::Abs
        | StdFloatOperation::Floor
        | StdFloatOperation::Ceil
        | StdFloatOperation::Round
        | StdFloatOperation::Trunc
        | StdFloatOperation::Fract
        | StdFloatOperation::Sqrt
        | StdFloatOperation::Sin
        | StdFloatOperation::Cos
        | StdFloatOperation::Tan
        | StdFloatOperation::Exp
        | StdFloatOperation::Exp2
        | StdFloatOperation::Ln
        | StdFloatOperation::Log2
        | StdFloatOperation::Log10 => (1, width.clone(), width),
    };
    homogeneous(arity, &input, result, validator)
}

impl ReductionConstructorKind {
    pub fn signature_schema(self) -> CallableSignatureSchema {
        self.instantiated_signature_schema(None)
    }

    pub(crate) fn instantiated_signature_schema(
        self,
        expected: Option<&TypeKind>,
    ) -> CallableSignatureSchema {
        match self {
            Self::Unchanged => schema(
                vec![parameter(
                    0,
                    Some("state"),
                    CallableParameterType::Unchecked,
                    CallableParameterPassing::PositionalOnly,
                    CallableParameterPresence::Required,
                )],
                expected
                    .filter(|expected| self.state_type(expected).is_some())
                    .cloned()
                    .unwrap_or_else(|| TypeKind::Named("Reduction<_>".to_owned())),
                &[],
                closed(),
                CallableValidator::ReductionConstructor(self),
            ),
        }
    }

    pub(crate) fn state_type(self, ty: &TypeKind) -> Option<TypeKind> {
        match self {
            Self::Unchanged => {
                let TypeKind::Named(label) = ty else {
                    return None;
                };
                let TypeRef::Generic { base, args } = parse_type_ref(label).ok()? else {
                    return None;
                };
                (base == "Reduction" && args.len() == 1).then(|| TypeKind::from(&args[0]))
            }
        }
    }
}

impl EnumVariantSignatureId {
    pub(crate) fn signature_schema(
        &self,
        payload: &EnumVariantPayload,
        expected: TypeKind,
    ) -> CallableSignatureSchema {
        let parameters = match payload {
            EnumVariantPayload::Unit | EnumVariantPayload::Record(_) => Vec::new(),
            EnumVariantPayload::Tuple(items) => items
                .iter()
                .enumerate()
                .map(|(index, ty)| {
                    parameter(
                        index,
                        None,
                        CallableParameterType::Exact(ty.clone()),
                        CallableParameterPassing::PositionalOnly,
                        CallableParameterPresence::Required,
                    )
                })
                .collect(),
        };
        schema(
            parameters,
            expected,
            &[],
            closed(),
            CallableValidator::EnumConstructor(self.clone()),
        )
    }
}

impl ResultConstructorKind {
    pub(crate) fn instantiated_signature_schema(
        self,
        expected: Option<&TypeKind>,
    ) -> CallableSignatureSchema {
        let expected_result = expected.and_then(|expected| match expected {
            TypeKind::Result { ok, error } => Some((expected.clone(), ok.as_ref(), error.as_ref())),
            _ => None,
        });
        let payload = expected_result.as_ref().map(|(_, ok, error)| match self {
            Self::Ok => (*ok).clone(),
            Self::Err => (*error).clone(),
        });
        let result = expected_result.map_or_else(
            || TypeKind::Result {
                ok: Box::new(named("_")),
                error: Box::new(named("_")),
            },
            |(expected, _, _)| expected,
        );
        schema(
            vec![parameter(
                0,
                Some("payload"),
                payload.map_or(
                    CallableParameterType::Unchecked,
                    CallableParameterType::Exact,
                ),
                CallableParameterPassing::PositionalOnly,
                CallableParameterPresence::Required,
            )],
            result,
            &[],
            closed(),
            CallableValidator::ResultConstructor(self),
        )
    }
}

impl OptionConstructorKind {
    pub(crate) fn instantiated_signature_schema(
        self,
        expected: Option<&TypeKind>,
    ) -> CallableSignatureSchema {
        let expected_option = expected.and_then(|expected| match expected {
            TypeKind::Option(item) => Some((expected.clone(), item.as_ref().clone())),
            _ => None,
        });
        let result = expected_option.as_ref().map_or_else(
            || TypeKind::Option(Box::new(named("_"))),
            |(ty, _)| ty.clone(),
        );
        schema(
            vec![parameter(
                0,
                Some("payload"),
                expected_option.map_or(CallableParameterType::Unchecked, |(_, item)| {
                    CallableParameterType::Exact(item)
                }),
                CallableParameterPassing::PositionalOnly,
                CallableParameterPresence::Required,
            )],
            result,
            &[],
            closed(),
            CallableValidator::OptionConstructor(self),
        )
    }
}

impl CollectionMethodId {
    pub(crate) fn signature_schema(self, receiver: &TypeKind) -> CallableSignatureSchema {
        let item = sequence_item(receiver).unwrap_or_else(|| named("_"));
        let validator = CallableValidator::Collection(self);
        match self {
            Self::Len => empty(TypeKind::USize, &[], validator),
            Self::Map => one_positional(
                "mapping",
                TypeKind::function([item], named("_")),
                collection_with_item(receiver, named("_")),
                &[],
                validator,
            ),
            Self::Filter => one_positional(
                "predicate",
                TypeKind::function([item], TypeKind::Bool),
                receiver.clone(),
                &[],
                validator,
            ),
            Self::Sum => empty(TypeKind::I64, &[], validator),
            Self::Contains => one_positional("item", item, TypeKind::Bool, &[], validator),
        }
    }
}

impl PresentationHandleMethodId {
    pub(crate) fn signature_schema(self) -> CallableSignatureSchema {
        empty(
            TypeKind::Unit,
            &[],
            CallableValidator::PresentationHandle(self),
        )
    }
}

impl IntegerMethodId {
    pub(crate) fn signature_schema(self, receiver: &TypeKind) -> CallableSignatureSchema {
        let arity = if self == Self::Clamp { 2 } else { 1 };
        homogeneous(
            arity,
            receiver,
            receiver.clone(),
            CallableValidator::Integer(self),
        )
    }
}

impl DomainMethodId {
    pub(crate) fn signature_schema(&self, receiver: &TypeKind) -> CallableSignatureSchema {
        let validator = CallableValidator::Domain(self.clone());
        match self {
            Self::Traverse => schema(
                vec![parameter(
                    0,
                    Some("task"),
                    CallableParameterType::Unchecked,
                    CallableParameterPassing::PositionalOnly,
                    CallableParameterPresence::Required,
                )],
                named("_"),
                &[],
                closed(),
                validator,
            ),
            Self::Parallel => schema(
                vec![required_named(0, "limit", TypeKind::I64)],
                receiver.clone(),
                &[],
                closed(),
                validator,
            ),
            Self::FxSampleOrdinalPhase => empty(TypeKind::F32, &[], validator),
            Self::ObservedObjectRequireRole => one_positional(
                "role",
                TypeKind::String,
                agent_result(TypeKind::ObservedObject),
                &[],
                validator,
            ),
            Self::MapGet { key, value } => {
                one_positional("key", key.clone(), value.clone(), &[], validator)
            }
            Self::ProbeCompare { value, .. } => one_positional(
                "expected",
                value.clone(),
                TypeKind::Predicate,
                &[],
                validator,
            ),
            Self::DiagnosticsHasError => empty(TypeKind::Predicate, &[], validator),
            Self::RagContextPackSummary => empty(TypeKind::DisplayText, &[], validator),
            Self::Context | Self::WithContext => {
                variadic_unchecked(context_result(receiver), validator, &[])
            }
            Self::CharacterFace { .. } => variadic_unchecked(
                TypeKind::CharacterPatch(EntityKind::Character),
                validator,
                &[],
            ),
            Self::CharacterSay { .. } => variadic_unchecked(
                TypeKind::SpeakerPreset(EntityKind::Character),
                validator,
                &[],
            ),
        }
    }
}

impl CapacityMethodId {
    pub(crate) fn signature_schema(&self, result: TypeKind) -> CallableSignatureSchema {
        homogeneous(
            self.arity(),
            &named("_"),
            result,
            CallableValidator::Capacity(self.clone()),
        )
    }
}

impl DropCallableId {
    #[allow(
        clippy::unused_self,
        reason = "schema construction remains discoverable on every resolved callable identity"
    )]
    pub(crate) fn signature_schema(self) -> CallableSignatureSchema {
        variadic_unchecked(TypeKind::Unit, CallableValidator::Drop, &[])
    }
}

impl PromotionCallableId {
    pub(crate) fn signature_schema(self) -> CallableSignatureSchema {
        let result = match self {
            Self::Promote | Self::PromoteUnchecked => named("Promoted"),
            Self::Assume => TypeKind::Unit,
        };
        variadic_unchecked(result, CallableValidator::Promotion(self), &[])
    }
}

impl SpeakerCallableId {
    #[allow(
        clippy::unused_self,
        reason = "schema construction remains discoverable on every resolved callable identity"
    )]
    pub(crate) fn signature_schema(&self) -> CallableSignatureSchema {
        variadic_unchecked(
            TypeKind::SpeakerPreset(EntityKind::Character),
            CallableValidator::Speaker,
            &[],
        )
    }
}

fn sequence_item(receiver: &TypeKind) -> Option<TypeKind> {
    match receiver {
        TypeKind::Vec(item)
        | TypeKind::Seq(item)
        | TypeKind::Slice(item)
        | TypeKind::Array { item, .. } => Some(item.as_ref().clone()),
        TypeKind::String => Some(TypeKind::TextCluster),
        _ => None,
    }
}

fn collection_with_item(receiver: &TypeKind, item: TypeKind) -> TypeKind {
    match receiver {
        TypeKind::Vec(_) => TypeKind::Vec(Box::new(item)),
        TypeKind::Seq(_) => TypeKind::Seq(Box::new(item)),
        TypeKind::Slice(_) => TypeKind::Slice(Box::new(item)),
        TypeKind::Array { len, .. } => TypeKind::Array {
            item: Box::new(item),
            len: len.clone(),
        },
        _ => named("_"),
    }
}

fn context_result(receiver: &TypeKind) -> TypeKind {
    match receiver {
        TypeKind::Need { .. } => receiver.clone(),
        TypeKind::Option(inner) => TypeKind::Result {
            ok: inner.clone(),
            error: Box::new(named("ArcError")),
        },
        TypeKind::Result { ok, .. } => TypeKind::Result {
            ok: ok.clone(),
            error: Box::new(named("ArcError")),
        },
        _ => named("_"),
    }
}

impl FxCallableSignatureId {
    pub fn signature_schema(self) -> CallableSignatureSchema {
        let validator = CallableValidator::Fx(self);
        let fx = named("Fx");
        match self {
            Self::Conditional => schema(
                vec![
                    required_named(0, "condition", TypeKind::Bool),
                    required_named(1, "then", fx.clone()),
                    required_named(2, "else", fx.clone()),
                ],
                fx,
                &[],
                closed(),
                validator,
            ),
            Self::Stack => schema(
                vec![parameter(
                    0,
                    Some("graphs"),
                    CallableParameterType::Exact(TypeKind::Seq(Box::new(fx.clone()))),
                    CallableParameterPassing::PositionalOnly,
                    CallableParameterPresence::Required,
                )],
                fx,
                &[],
                closed(),
                validator,
            ),
            Self::Shader => schema(
                vec![parameter(
                    0,
                    Some("resource"),
                    CallableParameterType::Unchecked,
                    CallableParameterPassing::PositionalOnly,
                    CallableParameterPresence::Optional,
                )],
                fx,
                &[],
                open_checked(),
                validator,
            ),
            Self::Style
            | Self::Text
            | Self::Color
            | Self::Transform
            | Self::Mask
            | Self::Filter
            | Self::Transition => schema(Vec::new(), fx, &[], open_checked(), validator),
        }
    }
}

impl AgentIntrinsicSignatureId {
    #[allow(
        clippy::too_many_lines,
        reason = "the exhaustive closed intrinsic table keeps ID and schema parity auditable"
    )]
    pub fn signature_schema(self) -> CallableSignatureSchema {
        let validator = CallableValidator::Agent(self);
        match self {
            Self::Expect | Self::Deny => schema(
                vec![
                    required(0, "condition", TypeKind::Bool),
                    optional(1, "message", TypeKind::String),
                ],
                TypeKind::Unit,
                &[],
                closed(),
                validator,
            ),
            Self::Checkpoint => one_positional(
                "name",
                TypeKind::String,
                TypeKind::Unit,
                &["debug.record"],
                validator,
            ),
            Self::Note => one_positional(
                "text",
                TypeKind::DisplayText,
                TypeKind::Unit,
                &["debug.record"],
                validator,
            ),
            Self::Attach => one_positional(
                "resource",
                TypeKind::Choice(vec![TypeKind::CaptureRef, TypeKind::AgentResource]),
                TypeKind::Unit,
                &["debug.record"],
                validator,
            ),
            Self::ChoiceAction => one_positional(
                "choice",
                TypeKind::entity_ref(EntityKind::ChoiceOption),
                TypeKind::ActionTarget,
                &[],
                validator,
            ),
            Self::Viewport => empty(TypeKind::CaptureTarget, &[], validator),
            Self::Layer => one_positional(
                "target",
                TypeKind::entity_ref(EntityKind::Layer),
                TypeKind::CaptureTarget,
                &[],
                validator,
            ),
            Self::Object => one_positional(
                "id",
                named("ObservedObjectId"),
                TypeKind::CaptureTarget,
                &[],
                validator,
            ),
            Self::Capture => schema(
                vec![
                    required(0, "target", TypeKind::CaptureTarget),
                    optional_named(1, "name", TypeKind::String),
                    optional_named_unchecked(2, "format"),
                    optional_named_unchecked(3, "kind"),
                ],
                agent_result(TypeKind::CaptureRef),
                &["agent.capture"],
                closed(),
                validator,
            ),
            Self::ReadResource => schema(
                vec![required(0, "uri", TypeKind::String)],
                agent_result(TypeKind::AgentResource),
                &["agent.resource.read"],
                closed(),
                validator,
            ),
            Self::EntityMeta => schema(
                vec![parameter(
                    0,
                    Some("entity"),
                    CallableParameterType::Unchecked,
                    CallableParameterPassing::PositionalOnly,
                    CallableParameterPresence::Required,
                )],
                agent_result(TypeKind::AgentEntityMetadata),
                &["debug.read"],
                closed(),
                validator,
            ),
            Self::ProjectNeighbors => schema(
                vec![
                    unchecked(0, "root", CallableParameterPresence::Required),
                    optional_named(1, "depth", TypeKind::U32),
                ],
                agent_result(TypeKind::AgentProjectGraphNeighborhood),
                &["debug.read"],
                closed(),
                validator,
            ),
            Self::Signal => probe_schema(EntityKind::Signal, validator),
            Self::Metric => probe_schema(EntityKind::Metric, validator),
            Self::StatePath => one_positional(
                "path",
                TypeKind::String,
                TypeKind::DebugStatePath,
                &[],
                validator,
            ),
            Self::ObservationPath => one_positional(
                "path",
                TypeKind::String,
                TypeKind::ObservationFieldPath,
                &[],
                validator,
            ),
            Self::State => schema(
                vec![required_choice(
                    0,
                    "path",
                    TypeKind::String,
                    TypeKind::DebugStatePath,
                )],
                TypeKind::Probe(Box::new(TypeKind::AgentValue)),
                &["debug.read"],
                closed(),
                validator,
            ),
            Self::Observation => schema(
                vec![required_choice(
                    0,
                    "path",
                    TypeKind::String,
                    TypeKind::ObservationFieldPath,
                )],
                TypeKind::Probe(Box::new(TypeKind::AgentValue)),
                &["agent.observe"],
                closed(),
                validator,
            ),
            Self::Diagnostics => empty(named("Diagnostics"), &["agent.observe"], validator),
            Self::Exists => one_positional(
                "probe",
                TypeKind::Probe(Box::new(named("_"))),
                TypeKind::Predicate,
                &[],
                validator,
            ),
            Self::ActionEnabled => one_positional(
                "target",
                TypeKind::ActionTarget,
                TypeKind::Predicate,
                &[],
                validator,
            ),
            Self::All | Self::Any => schema(
                vec![parameter(
                    0,
                    Some("predicates"),
                    CallableParameterType::Exact(TypeKind::Predicate),
                    CallableParameterPassing::RestPositional,
                    CallableParameterPresence::Required,
                )],
                TypeKind::Predicate,
                &[],
                closed(),
                validator,
            ),
            Self::Not => one_positional(
                "predicate",
                TypeKind::Predicate,
                TypeKind::Predicate,
                &[],
                validator,
            ),
            Self::Wait => schema(
                vec![
                    required_positional(0, "predicate", TypeKind::Predicate),
                    required(1, "timeout", TypeKind::Duration),
                    optional_named(2, "stable_frames", TypeKind::U32),
                    optional_named(3, "poll_frames", TypeKind::U32),
                ],
                TypeKind::Result {
                    ok: Box::new(TypeKind::Observation),
                    error: Box::new(named("WaitError")),
                },
                &["agent.wait", "agent.observe"],
                closed(),
                validator,
            ),
            Self::AdvanceText => empty(
                agent_result(TypeKind::ActionResult),
                &["agent.act.semantic"],
                validator,
            ),
            Self::ViewportPoint => schema(
                vec![
                    required(0, "x", TypeKind::U32),
                    required(1, "y", TypeKind::U32),
                ],
                named("ViewportPoint"),
                &[],
                closed(),
                validator,
            ),
            Self::PointerClick => schema(
                vec![
                    required(0, "point", named("ViewportPoint")),
                    defaulted_named(1, "button", TypeKind::ActionName),
                ],
                agent_result(TypeKind::ActionResult),
                &["agent.act.physical"],
                closed(),
                validator,
            ),
            Self::Invoke => schema(
                vec![
                    unchecked(0, "target", CallableParameterPresence::Required),
                    required(1, "action", TypeKind::ActionName),
                    parameter(
                        2,
                        Some("args"),
                        CallableParameterType::Exact(TypeKind::Map {
                            kind: MapKind::Sorted,
                            key: Box::new(TypeKind::String),
                            value: Box::new(TypeKind::AgentValue),
                        }),
                        CallableParameterPassing::PositionalOrNamed,
                        CallableParameterPresence::Optional,
                    ),
                ],
                agent_result(TypeKind::ActionResult),
                &["agent.act.semantic"],
                closed(),
                validator,
            ),
            Self::RagQuery => schema(
                vec![
                    required_positional(0, "query", TypeKind::String),
                    optional_named_unchecked(1, "roots"),
                    optional_named(2, "graph_depth", TypeKind::U32),
                    optional_named(3, "limit", TypeKind::USize),
                ],
                TypeKind::Result {
                    ok: Box::new(TypeKind::RagContextPack),
                    error: Box::new(named("RagError")),
                },
                &["rag.query"],
                closed(),
                validator,
            ),
        }
    }
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "all callable families share a fallible schema construction boundary"
)]
pub(in crate::callable) fn presentation_schema(
    id: PresentationCallableId,
    owner: Option<&ResolvedCharacterOwner>,
) -> Result<CallableSignatureSchema, CallableSchemaError> {
    let validator = CallableValidator::Presentation(id);
    let result = presentation_result(id);
    let (parameters, policy) = match id {
        PresentationCallableId::View
        | PresentationCallableId::Menu
        | PresentationCallableId::Overlay => (view_parameters(id), open_unchecked()),
        PresentationCallableId::Background => (
            vec![
                required_positional(0, "asset", TypeKind::entity_ref(EntityKind::Asset)),
                optional_presentation_named(id, 1, "target"),
                optional_presentation_named(id, 2, "slot"),
                optional_presentation_named(id, 3, "scope"),
                optional_presentation_named(id, 4, "fade"),
                optional_presentation_named(id, 5, "fit"),
                optional_presentation_named(id, 6, "opacity"),
            ],
            open_checked(),
        ),
        PresentationCallableId::Image => (
            vec![
                parameter(
                    0,
                    Some("source"),
                    CallableParameterType::Unchecked,
                    CallableParameterPassing::PositionalOnly,
                    CallableParameterPresence::Optional,
                ),
                optional_presentation_named(id, 1, "asset"),
                optional_presentation_named(id, 2, "lifetime"),
                optional_presentation_named(id, 3, "target"),
                optional_presentation_named(id, 4, "layer"),
                optional_presentation_named(id, 5, "depth"),
                optional_presentation_named(id, 6, "enabled"),
                optional_presentation_named(id, 7, "visible"),
                optional_presentation_named(id, 8, "id"),
                optional_presentation_named(id, 9, "action"),
                optional_presentation_named(id, 10, "actions"),
                optional_presentation_named(id, 11, "fit"),
                optional_presentation_named(id, 12, "opacity"),
                optional_presentation_named(id, 13, "x"),
                optional_presentation_named(id, 14, "y"),
                optional_presentation_named(id, 15, "width"),
                optional_presentation_named(id, 16, "height"),
                optional_presentation_named(id, 17, "focus"),
                optional_presentation_named(id, 18, "input_capture"),
                optional_presentation_named(id, 19, "owner"),
                optional_presentation_named(id, 20, "drop"),
            ],
            open_checked(),
        ),
        PresentationCallableId::PlayerViewport => (
            vec![
                optional_presentation_named(id, 0, "width"),
                optional_presentation_named(id, 1, "height"),
                optional_presentation_named(id, 2, "fit"),
            ],
            open_checked(),
        ),
        PresentationCallableId::Show => (
            character_parameters(id, owner.map(ResolvedCharacterOwner::character), true),
            open_unchecked(),
        ),
        PresentationCallableId::RefShow | PresentationCallableId::Hide => (
            character_parameters(id, owner.map(ResolvedCharacterOwner::character), false),
            open_unchecked(),
        ),
        PresentationCallableId::RefBackground | PresentationCallableId::ClearBackground => {
            (background_reference_parameters(id), open_unchecked())
        }
    };
    Ok(schema(parameters, result, &[], policy, validator))
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "all callable families share a fallible schema construction boundary"
)]
pub(in crate::callable) fn dialogue_schema(
    id: DialogueCallableId,
    callee: &DialogueCalleeIdentity,
) -> Result<CallableSignatureSchema, CallableSchemaError> {
    let character = match callee {
        DialogueCalleeIdentity::Speaker { character }
        | DialogueCalleeIdentity::SpeakerPreset { character } => Some(character),
        DialogueCalleeIdentity::Content { .. } => None,
    };
    let look = character.map_or(CallableParameterType::Unchecked, |character| {
        CallableParameterType::Exact(TypeKind::character_look(character.clone()))
    });
    let parameters = vec![
        optional_named(0, "id", TypeKind::entity_ref(EntityKind::DialogueLine)),
        optional_named(1, "text_key", TypeKind::entity_ref(EntityKind::Text)),
        optional_named_unchecked(2, "voice"),
        parameter(
            3,
            Some("look"),
            look,
            CallableParameterPassing::NamedOnly,
            CallableParameterPresence::Optional,
        ),
        optional_named_unchecked(4, "stage"),
        optional_named_unchecked(5, "portrait"),
        optional_named_unchecked(6, "focus"),
        optional_named_unchecked(7, "cleanup"),
        optional_named(8, "view", TypeKind::entity_ref(EntityKind::View)),
        optional_named(9, "source_locale", TypeKind::String),
        optional_named_unchecked(10, "hooks"),
        optional_named_unchecked(11, "style"),
        optional_named_unchecked(12, "rich_text"),
        parameter(
            13,
            Some("line_args"),
            CallableParameterType::Unchecked,
            CallableParameterPassing::RestNamed,
            CallableParameterPresence::Optional,
        ),
    ];
    Ok(schema(
        parameters,
        TypeKind::Unit,
        &[],
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::OpenChecked,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Dialogue(id),
    ))
}

fn presentation_result(id: PresentationCallableId) -> TypeKind {
    match id {
        PresentationCallableId::View => TypeKind::presentation_handle("View"),
        PresentationCallableId::Menu => TypeKind::presentation_handle("Menu"),
        PresentationCallableId::Overlay => TypeKind::presentation_handle("Overlay"),
        PresentationCallableId::Background => TypeKind::presentation_handle("BackgroundSurface"),
        PresentationCallableId::Image => TypeKind::presentation_handle("ImageSurface"),
        PresentationCallableId::PlayerViewport => TypeKind::presentation_handle("Viewport"),
        PresentationCallableId::Show => TypeKind::presentation_handle("CharacterSurface"),
        PresentationCallableId::RefBackground => named("SlotRef<BackgroundSurface>"),
        PresentationCallableId::RefShow => named("SlotRef<CharacterSurface>"),
        PresentationCallableId::ClearBackground => named("Option<BackgroundSurface>"),
        PresentationCallableId::Hide => named("Option<CharacterSurface>"),
    }
}

fn view_parameters(id: PresentationCallableId) -> Vec<CallableParameter> {
    vec![
        required_presentation(id, 0, "view"),
        optional_presentation_named(id, 1, "lifetime"),
        optional_presentation_named(id, 2, "target"),
        optional_presentation_named(id, 3, "layer"),
        optional_presentation_named(id, 4, "id"),
        optional_presentation_named(id, 5, "handle"),
        optional_presentation_named(id, 6, "key"),
        optional_presentation_named(id, 7, "mount"),
        optional_presentation_named(id, 8, "depth"),
        optional_presentation_named(id, 9, "visible"),
        optional_presentation_named(id, 10, "enabled"),
    ]
}

fn character_parameters(
    id: PresentationCallableId,
    character: Option<&CharacterId>,
    include_look: bool,
) -> Vec<CallableParameter> {
    let mut parameters = vec![required(
        0,
        "character",
        TypeKind::entity_ref(EntityKind::Character),
    )];
    if include_look {
        parameters.push(parameter(
            1,
            Some("look"),
            character.map_or(CallableParameterType::Unchecked, |character| {
                CallableParameterType::Exact(TypeKind::character_look(character.clone()))
            }),
            CallableParameterPassing::PositionalOrNamed,
            CallableParameterPresence::Optional,
        ));
    }
    let offset = usize::from(include_look);
    parameters.extend([
        optional_presentation_named(id, 1 + offset, "target"),
        optional_presentation_named(id, 2 + offset, "slot"),
        optional_presentation_named(id, 3 + offset, "scope"),
    ]);
    parameters
}

fn background_reference_parameters(id: PresentationCallableId) -> Vec<CallableParameter> {
    vec![
        optional_presentation_named(id, 0, "target"),
        optional_presentation_named(id, 1, "slot"),
        optional_presentation_named(id, 2, "scope"),
    ]
}

fn probe_schema(kind: EntityKind, validator: CallableValidator) -> CallableSignatureSchema {
    schema(
        vec![required_positional(0, "entity", TypeKind::entity_ref(kind))],
        TypeKind::Probe(Box::new(named("_"))),
        &["agent.observe"],
        closed(),
        validator,
    )
}

fn agent_result(ok: TypeKind) -> TypeKind {
    TypeKind::Result {
        ok: Box::new(ok),
        error: Box::new(named("AgentError")),
    }
}

fn named(name: &str) -> TypeKind {
    TypeKind::Named(name.to_owned())
}

fn homogeneous(
    arity: usize,
    input: &TypeKind,
    result: TypeKind,
    validator: CallableValidator,
) -> CallableSignatureSchema {
    let parameters = (0..arity)
        .map(|index| required_positional(index, &format!("arg{index}"), input.clone()))
        .collect();
    schema(parameters, result, &[], closed(), validator)
}

fn one_positional(
    name: &str,
    input: TypeKind,
    result: TypeKind,
    effects: &[&str],
    validator: CallableValidator,
) -> CallableSignatureSchema {
    schema(
        vec![required_positional(0, name, input)],
        result,
        effects,
        closed(),
        validator,
    )
}

fn variadic_unchecked(
    result: TypeKind,
    validator: CallableValidator,
    effects: &[&str],
) -> CallableSignatureSchema {
    schema(
        vec![parameter(
            0,
            Some("args"),
            CallableParameterType::Unchecked,
            CallableParameterPassing::RestPositional,
            CallableParameterPresence::Optional,
        )],
        result,
        effects,
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::OpenUnchecked,
            SpreadArgumentPolicy::Unchecked,
        ),
        validator,
    )
}

fn empty(
    result: TypeKind,
    effects: &[&str],
    validator: CallableValidator,
) -> CallableSignatureSchema {
    schema(Vec::new(), result, effects, closed(), validator)
}

fn schema(
    parameters: Vec<CallableParameter>,
    result: TypeKind,
    effects: &[&str],
    argument_policy: CallableArgumentPolicy,
    validator: CallableValidator,
) -> CallableSignatureSchema {
    let group = CallableParameterGroup::try_new(
        crate::callable::CallableGroupIndex::ZERO,
        CallableGroupKind::Initial,
        parameters,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("family schema coordinates are contiguous");
    let effects = EffectSet::from_labels(effects).expect("family effect labels are canonical");
    CallableSignatureSchema::try_new(
        vec![group],
        result,
        CallableEffectSchema::fixed(EffectRow::closed(effects)),
        argument_policy,
        validator,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("family schema satisfies callable invariants")
}

fn required(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterType::Exact(ty),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
    )
}

fn required_positional(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterType::Exact(ty),
        CallableParameterPassing::PositionalOnly,
        CallableParameterPresence::Required,
    )
}

fn required_named(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterType::Exact(ty),
        CallableParameterPassing::NamedOnly,
        CallableParameterPresence::Required,
    )
}

fn optional(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterType::Exact(ty),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Optional,
    )
}

fn optional_named(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterType::Exact(ty),
        CallableParameterPassing::NamedOnly,
        CallableParameterPresence::Optional,
    )
}

fn optional_presentation_named(
    id: PresentationCallableId,
    index: usize,
    name: &str,
) -> CallableParameter {
    parameter(
        index,
        Some(name),
        presentation_parameter_type(id, name),
        CallableParameterPassing::NamedOnly,
        CallableParameterPresence::Optional,
    )
}

fn required_presentation(
    id: PresentationCallableId,
    index: usize,
    name: &str,
) -> CallableParameter {
    parameter(
        index,
        Some(name),
        presentation_parameter_type(id, name),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
    )
}

fn presentation_parameter_type(id: PresentationCallableId, name: &str) -> CallableParameterType {
    let argument = id
        .resolve_named_argument(name)
        .expect("presentation schema parameter belongs to the callable argument catalog");
    match argument.value_policy() {
        PresentationArgumentValuePolicy::Exact(ty)
        | PresentationArgumentValuePolicy::TokenScalar(ty) => CallableParameterType::Exact(ty),
        PresentationArgumentValuePolicy::Unchecked
        | PresentationArgumentValuePolicy::MetadataScalar => CallableParameterType::Unchecked,
    }
}

fn optional_named_unchecked(index: usize, name: &str) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterType::Unchecked,
        CallableParameterPassing::NamedOnly,
        CallableParameterPresence::Optional,
    )
}

fn defaulted_named(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterType::Exact(ty),
        CallableParameterPassing::NamedOnly,
        CallableParameterPresence::Defaulted,
    )
}

fn unchecked(index: usize, name: &str, presence: CallableParameterPresence) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterType::Unchecked,
        CallableParameterPassing::PositionalOrNamed,
        presence,
    )
}

fn required_choice(index: usize, name: &str, left: TypeKind, right: TypeKind) -> CallableParameter {
    required(index, name, TypeKind::Choice(vec![left, right]))
}

fn parameter(
    index: usize,
    name: Option<&str>,
    ty: CallableParameterType,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
) -> CallableParameter {
    let index = CallableParameterIndex::try_from_usize(index)
        .expect("family parameter index fits the production limit");
    let name =
        name.map(|name| CallableName::try_new(name).expect("family parameter name is valid"));
    CallableParameter::try_new(index, name, ty, passing, presence, None, None)
        .expect("family parameter satisfies callable invariants")
}

const fn closed() -> CallableArgumentPolicy {
    CallableArgumentPolicy::new(
        UnknownNamedArgumentPolicy::Reject,
        SpreadArgumentPolicy::Reject,
    )
}

const fn open_checked() -> CallableArgumentPolicy {
    CallableArgumentPolicy::new(
        UnknownNamedArgumentPolicy::OpenChecked,
        SpreadArgumentPolicy::Reject,
    )
}

const fn open_unchecked() -> CallableArgumentPolicy {
    CallableArgumentPolicy::new(
        UnknownNamedArgumentPolicy::OpenUnchecked,
        SpreadArgumentPolicy::Reject,
    )
}
