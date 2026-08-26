//! Closed family schemas used by the shared callable resolver.

use crate::{
    callable::CharacterDialoguePatchContext,
    character_dialogue::CharacterDialogueFieldCoordinate,
    effect_row::EffectRow,
    effects::EffectSet,
    env::{
        RegisteredTypeCheckEnv,
        nominal::{AcceptedNominalCatalog, standard_agent_error_type, standard_reduction_record},
    },
    types::{
        CharacterDialogueCharacterType, CharacterDialogueType, EntityKind, GenericParameterOwnerId,
        GenericTypeParameterId, LanguageIntrinsicGenericOwner, MapKind, TypeKind,
    },
};
use arcweft_character::id::CharacterId;
use arcweft_lang_syntax::reference::BorrowKind;

use super::{
    CallableArgumentPolicy, CallableEffectSchema, CallableEvaluatedEffect,
    CallableGenericParameterIssuer, CallableGroupKind, CallableParameter,
    CallableParameterAdmission, CallableParameterConsumer, CallableParameterGroup,
    CallableParameterPassing, CallableParameterPresence, CallableParameterValueRule,
    CallableSignatureSchema, CallableValidator, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
};
use crate::callable::PromotionCallableId;
use crate::callable::{
    AgentIntrinsicSignatureId, BuiltinCallableId, CallableName, CallableParameterIndex,
    CallableSchemaError, CapabilityCallableId, CapacityMethodId, CollectionMethodId,
    DialogueCallableId, DialogueCallableResultContext, DialogueCalleeIdentity,
    DialogueSchemaContext, DomainMethodId, DropCallableId, FloatWidth, FxCallableSignatureId,
    IntegerMethodId, LineContextMethodId, LineScheduleCallableId, MathCallableId,
    OptionConstructorKind, PRODUCTION_CALLABLE_LIMITS, PresentationArgumentValuePolicy,
    PresentationCallableId, PresentationHandleMethodId, ReductionConstructorKind,
    ResolvedCharacterOwner, ResultConstructorKind, StageMethodId, StdFloatCallableId,
    StdFloatOperation, VectorDimensions,
};

pub(in crate::callable) fn dialogue_schema(
    id: DialogueCallableId,
    context: DialogueSchemaContext<'_>,
) -> Result<CallableSignatureSchema, CallableSchemaError> {
    if !id.supports_callee(context.callee) {
        return Err(CallableSchemaError::FamilyInvariant {
            family: crate::callable::CallableFamily::Dialogue,
            code: crate::callable::CallableFamilyInvariantCode::InvalidOwner,
        });
    }
    match (id, context.result) {
        (
            DialogueCallableId::ContentApplication,
            DialogueCallableResultContext::ContentApplication { .. },
        )
        | (
            DialogueCallableId::CharacterFactory
            | DialogueCallableId::CharacterReconfigure
            | DialogueCallableId::ContentCall,
            DialogueCallableResultContext::Declared,
        ) => {}
        _ => {
            return Err(CallableSchemaError::FamilyInvariant {
                family: crate::callable::CallableFamily::Dialogue,
                code: crate::callable::CallableFamilyInvariantCode::InvalidOwner,
            });
        }
    }
    let (parameters, result, policy) = match (id, context.callee) {
        (DialogueCallableId::CharacterFactory, DialogueCalleeIdentity::Character { character })
        | (
            DialogueCallableId::CharacterReconfigure,
            DialogueCalleeIdentity::CharacterDialogue { character },
        ) => {
            let mut parameters = character_dialogue_patch_parameters(character);
            if context.patch_context == CharacterDialoguePatchContext::ImmediateContentApplication {
                let next = parameters.len();
                parameters.push(parameter_with_consumer(
                    next,
                    Some("id"),
                    CallableParameterAdmission::checked(TypeKind::entity_ref(
                        EntityKind::DialogueLine,
                    )),
                    CallableParameterPassing::NamedOnly,
                    CallableParameterPresence::Optional,
                    CallableParameterConsumer::DialogueApplicationMetadata(
                        super::DialogueApplicationMetadataCoordinate::Id,
                    ),
                ));
                parameters.push(parameter_with_consumer(
                    next + 1,
                    Some("text_key"),
                    CallableParameterAdmission::checked(TypeKind::entity_ref(EntityKind::Text)),
                    CallableParameterPassing::NamedOnly,
                    CallableParameterPresence::Optional,
                    CallableParameterConsumer::DialogueApplicationMetadata(
                        super::DialogueApplicationMetadataCoordinate::TextKey,
                    ),
                ));
            }
            for (name, descriptor) in context.custom_fields.visible_bindings(context.module) {
                let index = parameters.len();
                parameters.push(dialogue_patch(
                    index,
                    name,
                    descriptor.value_type().clone(),
                    CharacterDialogueFieldCoordinate::Custom(descriptor.id().clone()),
                    descriptor.clearable(),
                ));
            }
            (
                parameters,
                TypeKind::CharacterDialogue(CharacterDialogueType::new(character.clone())),
                closed(),
            )
        }
        (DialogueCallableId::ContentApplication, DialogueCalleeIdentity::Character { .. })
        | (
            DialogueCallableId::ContentApplication,
            DialogueCalleeIdentity::CharacterDialogue { .. },
        ) => {
            let target = match context.callee {
                DialogueCalleeIdentity::Character { .. } => {
                    TypeKind::entity_ref(EntityKind::Character)
                }
                DialogueCalleeIdentity::CharacterDialogue { character } => {
                    TypeKind::CharacterDialogue(CharacterDialogueType::new(character.clone()))
                }
                DialogueCalleeIdentity::Content { .. } => {
                    return Err(CallableSchemaError::FamilyInvariant {
                        family: crate::callable::CallableFamily::Dialogue,
                        code: crate::callable::CallableFamilyInvariantCode::InvalidOwner,
                    });
                }
            };
            let DialogueCallableResultContext::ContentApplication { line_result } = context.result
            else {
                return Err(CallableSchemaError::FamilyInvariant {
                    family: crate::callable::CallableFamily::Dialogue,
                    code: crate::callable::CallableFamilyInvariantCode::InvalidOwner,
                });
            };
            (
                vec![
                    required_positional(0, "target", target),
                    required_positional(
                        1,
                        "content",
                        TypeKind::Named("DialogueContent".to_owned()),
                    ),
                    parameter(
                        2,
                        Some("line_plan"),
                        CallableParameterAdmission::checked(TypeKind::Named("LinePlan".to_owned())),
                        CallableParameterPassing::PositionalOnly,
                        CallableParameterPresence::Optional,
                    ),
                ],
                TypeKind::DialogueLine(Box::new(line_result.clone())),
                closed(),
            )
        }
        (DialogueCallableId::ContentCall, DialogueCalleeIdentity::Content { .. }) => {
            (content_call_parameters(context), TypeKind::Unit, closed())
        }
        _ => {
            return Err(CallableSchemaError::FamilyInvariant {
                family: crate::callable::CallableFamily::Dialogue,
                code: crate::callable::CallableFamilyInvariantCode::InvalidOwner,
            });
        }
    };
    Ok(schema(
        parameters,
        result,
        &[],
        policy,
        CallableValidator::Dialogue(id),
    ))
}

fn character_dialogue_patch_parameters(
    character: &CharacterDialogueCharacterType,
) -> Vec<CallableParameter> {
    let mut parameters = Vec::new();
    if let CharacterDialogueCharacterType::Exact(character) = character {
        parameters.push(dialogue_patch(
            parameters.len(),
            "look",
            TypeKind::character_look(character.clone()),
            CharacterDialogueFieldCoordinate::Look,
            true,
        ));
    }
    let fixed = [
        (
            "voice",
            TypeKind::Named("DialogueVoice".to_owned()),
            CharacterDialogueFieldCoordinate::Voice,
        ),
        (
            "stage",
            TypeKind::Named("DialogueStage".to_owned()),
            CharacterDialogueFieldCoordinate::Stage,
        ),
        (
            "portrait",
            TypeKind::Named("DialoguePortrait".to_owned()),
            CharacterDialogueFieldCoordinate::Portrait,
        ),
        (
            "focus",
            TypeKind::Named("DialogueFocus".to_owned()),
            CharacterDialogueFieldCoordinate::Focus,
        ),
        (
            "cleanup",
            TypeKind::Named("DialogueCleanup".to_owned()),
            CharacterDialogueFieldCoordinate::Cleanup,
        ),
        (
            "view",
            TypeKind::entity_ref(EntityKind::View),
            CharacterDialogueFieldCoordinate::View,
        ),
        (
            "source_locale",
            TypeKind::String,
            CharacterDialogueFieldCoordinate::SourceLocale,
        ),
        (
            "hooks",
            TypeKind::Seq(Box::new(TypeKind::Named("DialogueHook".to_owned()))),
            CharacterDialogueFieldCoordinate::Hooks,
        ),
        (
            "style",
            TypeKind::Choice(vec![
                TypeKind::entity_ref(EntityKind::Style),
                TypeKind::Named("RichTextStyle".to_owned()),
            ]),
            CharacterDialogueFieldCoordinate::Style,
        ),
        (
            "rich_text",
            TypeKind::Named("RichTextStyle".to_owned()),
            CharacterDialogueFieldCoordinate::RichText,
        ),
        (
            "inline_error",
            TypeKind::Named("InlineFailurePolicy".to_owned()),
            CharacterDialogueFieldCoordinate::InlineFailure,
        ),
        (
            "inline_error_policy",
            TypeKind::Named("InlineFailurePolicy".to_owned()),
            CharacterDialogueFieldCoordinate::InlineFailure,
        ),
        (
            "inline_fallback",
            TypeKind::Named("InlineFailurePolicy".to_owned()),
            CharacterDialogueFieldCoordinate::InlineFailure,
        ),
    ];
    for (name, ty, coordinate) in fixed {
        let index = parameters.len();
        parameters.push(dialogue_patch(index, name, ty, coordinate, true));
    }
    parameters
}

/// A content call applies a content-owned patch to a dialogue value.  It is
/// intentionally separate from CharacterFactory/Reconfigure: content calls
/// have no concrete character owner, so every visible field is supply-only
/// and unknown names are rejected by the closed argument policy.
fn content_call_parameters(context: DialogueSchemaContext<'_>) -> Vec<CallableParameter> {
    let fixed = [
        (
            "voice",
            TypeKind::Named("DialogueVoice".to_owned()),
            CharacterDialogueFieldCoordinate::Voice,
        ),
        (
            "stage",
            TypeKind::Named("DialogueStage".to_owned()),
            CharacterDialogueFieldCoordinate::Stage,
        ),
        (
            "portrait",
            TypeKind::Named("DialoguePortrait".to_owned()),
            CharacterDialogueFieldCoordinate::Portrait,
        ),
        (
            "focus",
            TypeKind::Named("DialogueFocus".to_owned()),
            CharacterDialogueFieldCoordinate::Focus,
        ),
        (
            "cleanup",
            TypeKind::Named("DialogueCleanup".to_owned()),
            CharacterDialogueFieldCoordinate::Cleanup,
        ),
        (
            "view",
            TypeKind::entity_ref(EntityKind::View),
            CharacterDialogueFieldCoordinate::View,
        ),
        (
            "source_locale",
            TypeKind::String,
            CharacterDialogueFieldCoordinate::SourceLocale,
        ),
        (
            "hooks",
            TypeKind::Seq(Box::new(TypeKind::Named("DialogueHook".to_owned()))),
            CharacterDialogueFieldCoordinate::Hooks,
        ),
        (
            "style",
            TypeKind::Choice(vec![
                TypeKind::entity_ref(EntityKind::Style),
                TypeKind::Named("RichTextStyle".to_owned()),
            ]),
            CharacterDialogueFieldCoordinate::Style,
        ),
        (
            "rich_text",
            TypeKind::Named("RichTextStyle".to_owned()),
            CharacterDialogueFieldCoordinate::RichText,
        ),
        (
            "inline_error",
            TypeKind::Named("InlineFailurePolicy".to_owned()),
            CharacterDialogueFieldCoordinate::InlineFailure,
        ),
        (
            "inline_error_policy",
            TypeKind::Named("InlineFailurePolicy".to_owned()),
            CharacterDialogueFieldCoordinate::InlineFailure,
        ),
        (
            "inline_fallback",
            TypeKind::Named("InlineFailurePolicy".to_owned()),
            CharacterDialogueFieldCoordinate::InlineFailure,
        ),
    ];
    let mut parameters = fixed
        .into_iter()
        .enumerate()
        .map(|(index, (name, ty, coordinate))| dialogue_patch(index, name, ty, coordinate, false))
        .collect::<Vec<_>>();
    for (name, descriptor) in context.custom_fields.visible_bindings(context.module) {
        let index = parameters.len();
        parameters.push(dialogue_patch(
            index,
            name,
            descriptor.value_type().clone(),
            CharacterDialogueFieldCoordinate::Custom(descriptor.id().clone()),
            false,
        ));
    }
    parameters
}

fn dialogue_patch(
    index: usize,
    name: &str,
    declared: TypeKind,
    coordinate: CharacterDialogueFieldCoordinate,
    clearable: bool,
) -> CallableParameter {
    let admission = if clearable {
        CallableParameterAdmission::checked_with_rule(
            declared,
            CallableParameterValueRule::clearable_option(),
        )
    } else {
        CallableParameterAdmission::checked(declared)
    };
    parameter_with_consumer(
        index,
        Some(name),
        admission,
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Optional,
        CallableParameterConsumer::DialoguePatch(coordinate),
    )
}

impl BuiltinCallableId {
    /// Returns a schema only for a closed builtin whose identity is complete
    /// without an accepted-world join. Reduction deliberately returns None.
    pub fn closed_signature_schema(&self) -> Option<CallableSignatureSchema> {
        let validator = CallableValidator::Builtin(self.clone());
        Some(match self {
            Self::InlineFailureFallback => {
                variadic_unchecked(TypeKind::Named("InlineFailure".to_owned()), validator, &[])
            }
            Self::Panic | Self::Fail | Self::Bail => {
                variadic_unchecked(TypeKind::Never, validator, &[])
            }
            Self::Ensure => schema(
                vec![
                    parameter(
                        0,
                        Some("condition"),
                        CallableParameterAdmission::checked(TypeKind::Bool),
                        CallableParameterPassing::PositionalOrNamed,
                        CallableParameterPresence::Required,
                    ),
                    parameter(
                        1,
                        Some("details"),
                        CallableParameterAdmission::unchecked_supply(),
                        CallableParameterPassing::RestPositional,
                        CallableParameterPresence::Optional,
                    ),
                ],
                TypeKind::Unit,
                &[],
                open_supply(),
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
                    .with_evaluated_effect(CallableEvaluatedEffect::EmitEvent)
            }
            Self::Reduction(_) => return None,
        })
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
    pub(crate) fn accepted_signature_schema(
        self,
        catalog: &AcceptedNominalCatalog,
    ) -> Option<CallableSignatureSchema> {
        let record = standard_reduction_record(catalog)?;
        let declaration = record.id().clone();
        match self {
            Self::Unchanged => {
                let owner = GenericParameterOwnerId::AcceptedNominal(declaration.clone());
                let state = generic(owner, 0);
                let state_ref = TypeKind::BorrowRef {
                    kind: BorrowKind::Shared,
                    lifetime: None,
                    inner: Box::new(state.clone()),
                };
                Some(schema_with_issuer(
                    vec![parameter(
                        0,
                        Some("state"),
                        CallableParameterAdmission::checked(state_ref),
                        CallableParameterPassing::PositionalOnly,
                        CallableParameterPresence::Required,
                    )],
                    TypeKind::AcceptedNominal(crate::types::AcceptedNominalType::new(
                        declaration.clone(),
                        [state],
                    )),
                    &[],
                    closed(),
                    CallableValidator::ReductionConstructor(self),
                    CallableGenericParameterIssuer::accepted_nominal(declaration.clone(), 1, 0)
                        .expect("accepted reduction issuer"),
                ))
            }
        }
    }
}

impl ResultConstructorKind {
    pub(crate) fn signature_schema(self) -> CallableSignatureSchema {
        let owner = GenericParameterOwnerId::LanguageIntrinsic(
            LanguageIntrinsicGenericOwner::ResultConstructor,
        );
        let ok = generic(owner.clone(), 0);
        let error = generic(owner, 1);
        let payload = match self {
            Self::Ok => ok.clone(),
            Self::Err => error.clone(),
        };
        schema_with_issuer(
            vec![parameter(
                0,
                Some("payload"),
                CallableParameterAdmission::checked(payload),
                CallableParameterPassing::PositionalOnly,
                CallableParameterPresence::Required,
            )],
            TypeKind::Result {
                ok: Box::new(ok),
                error: Box::new(error),
            },
            &[],
            closed(),
            CallableValidator::ResultConstructor(self),
            CallableGenericParameterIssuer::language_intrinsic(
                LanguageIntrinsicGenericOwner::ResultConstructor,
                2,
                0,
            )
            .expect("Result issuer"),
        )
    }
}

impl OptionConstructorKind {
    pub(crate) fn signature_schema(self) -> CallableSignatureSchema {
        let item = generic(
            GenericParameterOwnerId::LanguageIntrinsic(
                LanguageIntrinsicGenericOwner::OptionConstructor,
            ),
            0,
        );
        schema_with_issuer(
            vec![parameter(
                0,
                Some("payload"),
                CallableParameterAdmission::checked(item.clone()),
                CallableParameterPassing::PositionalOnly,
                CallableParameterPresence::Required,
            )],
            TypeKind::Option(Box::new(item)),
            &[],
            closed(),
            CallableValidator::OptionConstructor(self),
            CallableGenericParameterIssuer::language_intrinsic(
                LanguageIntrinsicGenericOwner::OptionConstructor,
                1,
                0,
            )
            .expect("Option issuer"),
        )
    }
}

impl CollectionMethodId {
    pub(crate) fn signature_schema(self, receiver: &TypeKind) -> Option<CallableSignatureSchema> {
        let item = sequence_item(receiver)?;
        let validator = CallableValidator::Collection(self);
        Some(match self {
            Self::Len => empty(TypeKind::USize, &[], validator),
            Self::Map => {
                let output = generic(
                    GenericParameterOwnerId::LanguageIntrinsic(
                        LanguageIntrinsicGenericOwner::CollectionMap,
                    ),
                    0,
                );
                one_positional_with_issuer(
                    "mapping",
                    TypeKind::function([item], output.clone()),
                    collection_with_item(receiver, output)?,
                    &[],
                    validator,
                    CallableGenericParameterIssuer::language_intrinsic(
                        LanguageIntrinsicGenericOwner::CollectionMap,
                        1,
                        0,
                    )
                    .expect("Collection::map issuer"),
                )
            }
            Self::Filter => one_positional(
                "predicate",
                TypeKind::function([item], TypeKind::Bool),
                receiver.clone(),
                &[],
                validator,
            ),
            Self::Sum => empty(TypeKind::I64, &[], validator),
            Self::Contains => one_positional("item", item, TypeKind::Bool, &[], validator),
        })
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
    pub(crate) fn signature_schema(&self, receiver: &TypeKind) -> Option<CallableSignatureSchema> {
        let validator = CallableValidator::Domain(self.clone());
        Some(match self {
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
                variadic_unchecked(context_result(receiver)?, validator, &[])
            }
        })
    }
}

impl CapacityMethodId {
    pub(crate) fn signature_schema(&self) -> CallableSignatureSchema {
        let result = self.result_type();
        let validator = CallableValidator::Capacity(self.clone());
        match (self.method().as_str(), self.arity()) {
            ("with_capacity", _) => variadic_unchecked(result, validator, &[]),
            ("trim" | "to_string" | "pop" | "pop_front" | "collect" | "shrink", 0) => {
                empty(result, &[], validator)
            }
            ("push" | "reserve" | "shrink_to", 1) => schema(
                vec![parameter(
                    0,
                    Some("value"),
                    CallableParameterAdmission::unchecked_supply(),
                    CallableParameterPassing::PositionalOrNamed,
                    CallableParameterPresence::Required,
                )],
                result,
                &[],
                closed(),
                validator,
            ),
            _ => unreachable!("CapacityMethodId constructors retain a supported arity"),
        }
    }
}

impl StageMethodId {
    pub(crate) fn signature_schema(self, receiver: &TypeKind) -> CallableSignatureSchema {
        let validator = CallableValidator::Stage(self);
        match self {
            Self::Acquire => {
                let TypeKind::StageApi(character) = receiver else {
                    unreachable!("Stage acquire retains an exact StageApi receiver")
                };
                schema(
                    vec![parameter(
                        0,
                        Some("scope"),
                        CallableParameterAdmission::checked(named("PresentationLifetime")),
                        CallableParameterPassing::PositionalOrNamed,
                        CallableParameterPresence::Required,
                    )],
                    TypeKind::StageActorHandle(crate::types::StageActorHandleType::Exact(
                        character.clone(),
                    )),
                    &[],
                    closed(),
                    validator,
                )
            }
            Self::Look => {
                let TypeKind::StageActorHandle(crate::types::StageActorHandleType::Exact(
                    character,
                )) = receiver
                else {
                    unreachable!("Stage look retains an exact StageActorHandle receiver")
                };
                schema(
                    vec![
                        parameter(
                            0,
                            Some("look"),
                            CallableParameterAdmission::checked(TypeKind::character_look(
                                character.clone(),
                            )),
                            CallableParameterPassing::PositionalOrNamed,
                            CallableParameterPresence::Required,
                        ),
                        parameter(
                            1,
                            Some("crossfade"),
                            CallableParameterAdmission::checked(TypeKind::Duration),
                            CallableParameterPassing::PositionalOrNamed,
                            CallableParameterPresence::Optional,
                        ),
                    ],
                    TypeKind::CueHandle,
                    &[],
                    closed(),
                    validator,
                )
            }
        }
    }
}

impl LineContextMethodId {
    pub(crate) fn signature_schema(self) -> CallableSignatureSchema {
        empty(
            TypeKind::VoiceHandle,
            &["dialogue.voice"],
            CallableValidator::LineContext(self),
        )
    }
}

impl LineScheduleCallableId {
    pub fn signature_schema(self) -> CallableSignatureSchema {
        let callback = TypeKind::function_with_effects(
            std::iter::empty(),
            TypeKind::CueHandle,
            EffectRow::closed(EffectSet::new()),
        );
        one_positional(
            "anchor",
            TypeKind::Duration,
            TypeKind::function_with_effects(
                [callback],
                TypeKind::CueHandle,
                EffectRow::closed(EffectSet::new()),
            ),
            &["dialogue.schedule"],
            CallableValidator::Ordinary,
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

fn collection_with_item(receiver: &TypeKind, item: TypeKind) -> Option<TypeKind> {
    match receiver {
        TypeKind::Vec(_) => Some(TypeKind::Vec(Box::new(item))),
        TypeKind::Seq(_) => Some(TypeKind::Seq(Box::new(item))),
        TypeKind::Slice(_) => Some(TypeKind::Slice(Box::new(item))),
        TypeKind::Array { len, .. } => Some(TypeKind::Array {
            item: Box::new(item),
            len: len.clone(),
        }),
        TypeKind::String => None,
        _ => None,
    }
}

fn context_result(receiver: &TypeKind) -> Option<TypeKind> {
    match receiver {
        TypeKind::Need(_) => Some(receiver.clone()),
        TypeKind::Option(inner) => Some(TypeKind::Result {
            ok: inner.clone(),
            error: Box::new(named("ArcError")),
        }),
        TypeKind::Result { ok, .. } => Some(TypeKind::Result {
            ok: ok.clone(),
            error: Box::new(named("ArcError")),
        }),
        _ => None,
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
                    CallableParameterAdmission::checked(TypeKind::Vec(Box::new(fx.clone()))),
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
                    CallableParameterAdmission::unchecked_supply(),
                    CallableParameterPassing::PositionalOnly,
                    CallableParameterPresence::Optional,
                )],
                fx,
                &[],
                open_supply(),
                validator,
            ),
            Self::Transform => schema(
                vec![parameter(
                    0,
                    Some("sample"),
                    CallableParameterAdmission::checked(TypeKind::function(
                        [named("FxSampleContext")],
                        named("Transform2D"),
                    )),
                    CallableParameterPassing::NamedOnly,
                    CallableParameterPresence::Optional,
                )],
                fx,
                &[],
                open_supply(),
                validator,
            ),
            Self::Style
            | Self::Text
            | Self::Color
            | Self::Mask
            | Self::Filter
            | Self::Transition => schema(Vec::new(), fx, &[], open_supply(), validator),
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
            Self::Observe => empty(
                agent_result(TypeKind::Observation),
                &["agent.observe"],
                validator,
            ),
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
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::ObservedObjectId),
                TypeKind::CaptureTarget,
                &[],
                validator,
            ),
            Self::Capture => schema(
                vec![
                    required(0, "target", TypeKind::CaptureTarget),
                    optional_named(1, "name", TypeKind::String),
                    optional_named(
                        2,
                        "format",
                        TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::CaptureFormat),
                    ),
                    optional_named(
                        3,
                        "kind",
                        TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::CaptureKind),
                    ),
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
                    CallableParameterAdmission::unchecked_supply(),
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
            Self::Diagnostics => empty(
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::Diagnostics),
                &["agent.observe"],
                validator,
            ),
            Self::Exists => one_positional_with_issuer(
                "probe",
                TypeKind::Probe(Box::new(generic(
                    GenericParameterOwnerId::LanguageIntrinsic(
                        LanguageIntrinsicGenericOwner::FxExists,
                    ),
                    0,
                ))),
                TypeKind::Predicate,
                &[],
                validator,
                CallableGenericParameterIssuer::language_intrinsic(
                    LanguageIntrinsicGenericOwner::FxExists,
                    1,
                    0,
                )
                .expect("exists issuer"),
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
                    CallableParameterAdmission::checked(TypeKind::Predicate),
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
                    error: Box::new(TypeKind::AgentBuiltin(
                        crate::types::AgentBuiltinType::WaitError,
                    )),
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
                TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::ViewportPoint),
                &[],
                closed(),
                validator,
            ),
            Self::PointerClick => schema(
                vec![
                    required(
                        0,
                        "point",
                        TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::ViewportPoint),
                    ),
                    defaulted_named(
                        1,
                        "button",
                        TypeKind::AgentBuiltin(crate::types::AgentBuiltinType::PointerButton),
                    ),
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
                        CallableParameterAdmission::checked(TypeKind::Map {
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
                    error: Box::new(TypeKind::AgentBuiltin(
                        crate::types::AgentBuiltinType::RagError,
                    )),
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
    environment: Option<&RegisteredTypeCheckEnv>,
) -> Result<CallableSignatureSchema, CallableSchemaError> {
    let validator = CallableValidator::Presentation(id);
    let result = presentation_result(id);
    let character = owner.and_then(|owner| {
        let character = owner.character();
        environment
            .and_then(|environment| environment.character_manifest(character))
            .filter(|manifest| !manifest.looks().is_empty())
            .map(|_| character)
    });
    let (parameters, policy) = match id {
        PresentationCallableId::View
        | PresentationCallableId::Menu
        | PresentationCallableId::Overlay => (view_parameters(id), open_supply()),
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
            open_supply(),
        ),
        PresentationCallableId::Image => (
            vec![
                parameter(
                    0,
                    Some("source"),
                    CallableParameterAdmission::unchecked_supply(),
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
            open_supply(),
        ),
        PresentationCallableId::PlayerViewport => (
            vec![
                optional_presentation_named(id, 0, "width"),
                optional_presentation_named(id, 1, "height"),
                optional_presentation_named(id, 2, "fit"),
            ],
            open_supply(),
        ),
        PresentationCallableId::Show => (character_parameters(id, character, true), open_supply()),
        PresentationCallableId::RefShow | PresentationCallableId::Hide => {
            (character_parameters(id, character, false), open_supply())
        }
        PresentationCallableId::RefBackground | PresentationCallableId::ClearBackground => {
            (background_reference_parameters(id), open_supply())
        }
    };
    let reserved_open_names = match (id, character.is_some()) {
        (PresentationCallableId::Show, true) => Vec::new(),
        (PresentationCallableId::Show, false)
        | (PresentationCallableId::RefShow, _)
        | (PresentationCallableId::Hide, _) => {
            vec![CallableName::try_new("look").expect("presentation reserved name is valid")]
        }
        _ => Vec::new(),
    };
    schema(parameters, result, &[], policy, validator)
        .try_with_reserved_open_names(reserved_open_names, &PRODUCTION_CALLABLE_LIMITS)
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
    let character = include_look.then_some(character).flatten();
    let mut parameters = vec![required(
        0,
        "character",
        TypeKind::entity_ref(EntityKind::Character),
    )];
    if let Some(character) = character {
        parameters.push(parameter(
            1,
            Some("look"),
            CallableParameterAdmission::checked(TypeKind::character_look(character.clone())),
            CallableParameterPassing::PositionalOrNamed,
            CallableParameterPresence::Optional,
        ));
    }
    let offset = usize::from(character.is_some());
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
    let owner = match kind {
        EntityKind::Signal => LanguageIntrinsicGenericOwner::AgentSignal,
        EntityKind::Metric => LanguageIntrinsicGenericOwner::AgentMetric,
        _ => unreachable!("probe schemas are only published for signal and metric references"),
    };
    let value = TypeKind::GenericParam(GenericTypeParameterId::new(
        GenericParameterOwnerId::LanguageIntrinsic(owner),
        0,
    ));
    schema_with_issuer(
        vec![required_positional(
            0,
            "entity",
            TypeKind::entity_ref_with_value(kind, value.clone()),
        )],
        TypeKind::Probe(Box::new(value)),
        &["agent.observe"],
        closed(),
        validator,
        CallableGenericParameterIssuer::language_intrinsic(owner, 1, 0).expect("probe issuer"),
    )
}

fn agent_result(ok: TypeKind) -> TypeKind {
    TypeKind::Result {
        ok: Box::new(ok),
        error: Box::new(standard_agent_error_type()),
    }
}

fn named(name: &str) -> TypeKind {
    TypeKind::Named(name.to_owned())
}

fn generic(owner: GenericParameterOwnerId, ordinal: u16) -> TypeKind {
    TypeKind::GenericParam(GenericTypeParameterId::new(owner, ordinal))
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

fn one_positional_with_issuer(
    name: &str,
    input: TypeKind,
    result: TypeKind,
    effects: &[&str],
    validator: CallableValidator,
    issuer: CallableGenericParameterIssuer,
) -> CallableSignatureSchema {
    schema_with_issuer(
        vec![required_positional(0, name, input)],
        result,
        effects,
        closed(),
        validator,
        issuer,
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
            CallableParameterAdmission::unchecked_supply(),
            CallableParameterPassing::RestPositional,
            CallableParameterPresence::Optional,
        )],
        result,
        effects,
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::OpenSupply,
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
    schema_with_issuer(
        parameters,
        result,
        effects,
        argument_policy,
        validator,
        CallableGenericParameterIssuer::empty(),
    )
}

fn schema_with_issuer(
    parameters: Vec<CallableParameter>,
    result: TypeKind,
    effects: &[&str],
    argument_policy: CallableArgumentPolicy,
    validator: CallableValidator,
    generic_issuer: CallableGenericParameterIssuer,
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
        generic_issuer,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("family schema satisfies callable invariants")
}

fn required(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterAdmission::checked(ty),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Required,
    )
}

fn required_positional(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterAdmission::checked(ty),
        CallableParameterPassing::PositionalOnly,
        CallableParameterPresence::Required,
    )
}

fn required_named(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterAdmission::checked(ty),
        CallableParameterPassing::NamedOnly,
        CallableParameterPresence::Required,
    )
}

fn optional(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterAdmission::checked(ty),
        CallableParameterPassing::PositionalOrNamed,
        CallableParameterPresence::Optional,
    )
}

fn optional_named(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterAdmission::checked(ty),
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

fn presentation_parameter_type(
    id: PresentationCallableId,
    name: &str,
) -> CallableParameterAdmission {
    let argument = id
        .resolve_named_argument(name)
        .expect("presentation schema parameter belongs to the callable argument catalog");
    match argument.value_policy() {
        PresentationArgumentValuePolicy::Exact(ty)
        | PresentationArgumentValuePolicy::TokenScalar(ty) => {
            CallableParameterAdmission::checked(ty)
        }
        PresentationArgumentValuePolicy::Unchecked
        | PresentationArgumentValuePolicy::MetadataScalar => {
            CallableParameterAdmission::unchecked_supply()
        }
    }
}

fn optional_named_unchecked(index: usize, name: &str) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterAdmission::unchecked_supply(),
        CallableParameterPassing::NamedOnly,
        CallableParameterPresence::Optional,
    )
}

fn defaulted_named(index: usize, name: &str, ty: TypeKind) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterAdmission::checked(ty),
        CallableParameterPassing::NamedOnly,
        CallableParameterPresence::Defaulted,
    )
}

fn unchecked(index: usize, name: &str, presence: CallableParameterPresence) -> CallableParameter {
    parameter(
        index,
        Some(name),
        CallableParameterAdmission::unchecked_supply(),
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
    admission: CallableParameterAdmission,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
) -> CallableParameter {
    let index = CallableParameterIndex::try_from_usize(index)
        .expect("family parameter index fits the production limit");
    let name =
        name.map(|name| CallableName::try_new(name).expect("family parameter name is valid"));
    CallableParameter::try_new(index, name, admission, passing, presence, None, None)
        .expect("family parameter satisfies callable invariants")
}

fn parameter_with_consumer(
    index: usize,
    name: Option<&str>,
    admission: CallableParameterAdmission,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
    consumer: CallableParameterConsumer,
) -> CallableParameter {
    parameter(index, name, admission, passing, presence).with_consumer(consumer)
}

const fn closed() -> CallableArgumentPolicy {
    CallableArgumentPolicy::new(
        UnknownNamedArgumentPolicy::Reject,
        SpreadArgumentPolicy::Reject,
    )
}

const fn open_supply() -> CallableArgumentPolicy {
    CallableArgumentPolicy::new(
        UnknownNamedArgumentPolicy::OpenSupply,
        SpreadArgumentPolicy::Reject,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callable::{CallableGroupIndex, CallableSchemaGenericRole};

    #[test]
    fn character_any_reserves_look_without_closing_other_open_names() {
        let show = presentation_schema(PresentationCallableId::Show, None, None)
            .expect("Character-Any Show schema");
        let [group] = show.groups() else {
            panic!("Show schema must have one parameter group")
        };
        assert_eq!(
            group
                .parameters()
                .iter()
                .map(|parameter| parameter.name().map(CallableName::as_str))
                .collect::<Vec<_>>(),
            vec![
                Some("character"),
                Some("target"),
                Some("slot"),
                Some("scope")
            ]
        );
        let look = CallableName::try_new("look").expect("look name");
        let custom = CallableName::try_new("custom").expect("custom name");
        assert_eq!(show.reserved_open_names(), std::slice::from_ref(&look));
        assert!(!show.allows_open_name(&look));
        assert!(show.allows_open_name(&custom));

        let hide = presentation_schema(PresentationCallableId::Hide, None, None)
            .expect("Character-Any Hide schema");
        assert!(!hide.allows_open_name(&look));
    }

    #[test]
    fn reserved_open_names_validate_policy_collisions_and_duplicates() {
        let schema = presentation_schema(PresentationCallableId::Show, None, None)
            .expect("Character-Any Show schema");
        let look = CallableName::try_new("look").expect("look name");
        let duplicate = schema.clone().try_with_reserved_open_names(
            vec![look.clone(), look.clone()],
            &PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            duplicate,
            Err(CallableSchemaError::DuplicateReservedOpenName { .. })
        ));

        let collision = schema.clone().try_with_reserved_open_names(
            vec![CallableName::try_new("character").expect("parameter name")],
            &PRODUCTION_CALLABLE_LIMITS,
        );
        assert!(matches!(
            collision,
            Err(CallableSchemaError::ReservedOpenNameParameterCollision { .. })
        ));

        let closed = OptionConstructorKind::Some
            .signature_schema()
            .try_with_reserved_open_names(vec![look], &PRODUCTION_CALLABLE_LIMITS);
        assert!(matches!(
            closed,
            Err(CallableSchemaError::ReservedOpenNamesRequireOpenPolicy)
        ));
    }

    #[test]
    fn event_emit_builtin_owns_its_evaluated_effect_disposition() {
        let schema = BuiltinCallableId::Capability(CapabilityCallableId::EventEmit)
            .closed_signature_schema()
            .expect("closed event builtin schema");
        assert_eq!(
            schema.evaluated_effect(),
            Some(CallableEvaluatedEffect::EmitEvent)
        );
    }

    #[test]
    fn reduction_has_no_schema_without_an_accepted_catalog_join() {
        assert!(
            BuiltinCallableId::Reduction(ReductionConstructorKind::Unchanged)
                .closed_signature_schema()
                .is_none()
        );
    }

    #[test]
    fn reduction_schema_uses_the_accepted_nominal_issuer() {
        let environment = crate::env::TypeCheckEnv::standard();
        let schema = ReductionConstructorKind::Unchanged
            .accepted_signature_schema(environment.nominal_catalog())
            .expect("standard Reduction catalog row");
        assert_eq!(
            schema
                .generic_inventory()
                .types()
                .iter()
                .filter(|entry| entry.role() == CallableSchemaGenericRole::Candidate)
                .count(),
            1,
        );
        let [parameter] = schema.groups()[0].parameters() else {
            panic!("Reduction.unchanged must accept one state borrow")
        };
        let TypeKind::BorrowRef {
            kind: BorrowKind::Shared,
            lifetime: None,
            inner,
        } = parameter
            .declared_type()
            .expect("Reduction state parameter is checked")
        else {
            panic!("Reduction.unchanged must accept an elided shared state borrow")
        };
        assert!(matches!(inner.as_ref(), TypeKind::GenericParam(_)));
        let TypeKind::AcceptedNominal(reduction) = schema.result() else {
            panic!("Reduction.unchanged must return the accepted Reduction owner")
        };
        assert_eq!(reduction.arguments(), [inner.as_ref().clone()]);
    }

    #[test]
    fn signal_and_metric_schemas_share_their_exact_payload_parameter() {
        for (intrinsic, kind, owner) in [
            (
                AgentIntrinsicSignatureId::Signal,
                EntityKind::Signal,
                LanguageIntrinsicGenericOwner::AgentSignal,
            ),
            (
                AgentIntrinsicSignatureId::Metric,
                EntityKind::Metric,
                LanguageIntrinsicGenericOwner::AgentMetric,
            ),
        ] {
            let schema = intrinsic.signature_schema();
            let [group] = schema.groups() else {
                panic!("probe schema must have one parameter group")
            };
            let [parameter] = group.parameters() else {
                panic!("probe schema must have one parameter")
            };
            let Some(TypeKind::Ref(entity)) = parameter.declared_type() else {
                panic!("probe schema must accept one typed entity reference")
            };
            assert_eq!(entity.kind(), &kind);
            let Some(TypeKind::GenericParam(parameter)) = entity.value() else {
                panic!("probe entity reference must retain its payload parameter")
            };
            assert_eq!(
                parameter.owner(),
                &GenericParameterOwnerId::LanguageIntrinsic(owner)
            );
            assert_eq!(parameter.ordinal(), 0);
            assert_eq!(
                schema.result(),
                &TypeKind::Probe(Box::new(TypeKind::GenericParam(parameter.clone())))
            );
        }
    }

    #[test]
    fn constructors_are_context_free_generic_schemas() {
        let option = OptionConstructorKind::Some.signature_schema();
        let TypeKind::Option(item) = option.result() else {
            panic!("Some must publish an Option result")
        };
        let TypeKind::GenericParam(option_item) = item.as_ref() else {
            panic!("Some must publish a generic item")
        };
        assert_eq!(
            option_item.owner(),
            &GenericParameterOwnerId::LanguageIntrinsic(
                LanguageIntrinsicGenericOwner::OptionConstructor
            )
        );
        assert_eq!(option_item.ordinal(), 0);
        assert_eq!(
            option.groups()[0].parameters()[0].declared_type(),
            Some(item.as_ref())
        );

        for (kind, ordinal) in [
            (ResultConstructorKind::Ok, 0_u16),
            (ResultConstructorKind::Err, 1_u16),
        ] {
            let schema = kind.signature_schema();
            let TypeKind::Result { ok, error } = schema.result() else {
                panic!("Result constructor must publish a Result result")
            };
            let payload = schema.groups()[0].parameters()[0]
                .declared_type()
                .expect("Result payload is checked");
            let TypeKind::GenericParam(payload) = payload else {
                panic!("Result payload must be generic")
            };
            assert_eq!(payload.ordinal(), ordinal);
            assert_eq!(
                payload.owner(),
                &GenericParameterOwnerId::LanguageIntrinsic(
                    LanguageIntrinsicGenericOwner::ResultConstructor
                )
            );
            assert!(matches!(ok.as_ref(), TypeKind::GenericParam(_)));
            assert!(matches!(error.as_ref(), TypeKind::GenericParam(_)));
            assert!(!schema.result().source_label().contains("_"));
        }
    }

    #[test]
    fn collection_map_and_fx_exists_reject_untyped_receiver_fallbacks() {
        let map = CollectionMethodId::Map
            .signature_schema(&TypeKind::Vec(Box::new(TypeKind::I32)))
            .expect("Vec map has a typed schema");
        let TypeKind::Vec(item) = map.result() else {
            panic!("map preserves the concrete collection constructor")
        };
        let TypeKind::GenericParam(item) = item.as_ref() else {
            panic!("map result item is generic")
        };
        assert_eq!(
            item.owner(),
            &GenericParameterOwnerId::LanguageIntrinsic(
                LanguageIntrinsicGenericOwner::CollectionMap
            )
        );
        assert_eq!(item.ordinal(), 0);
        let callback = map.groups()[0].parameters()[0]
            .declared_type()
            .expect("map callback is checked");
        let TypeKind::Function { params, .. } = callback else {
            panic!("map callback must be a function")
        };
        assert_eq!(params.as_slice(), &[TypeKind::I32]);
        let wrong_callback = TypeKind::function([TypeKind::String], TypeKind::Bool);
        assert!(!callback.accepts(&wrong_callback));
        assert!(
            CollectionMethodId::Map
                .signature_schema(&TypeKind::Named("Unsupported".to_owned()))
                .is_none()
        );

        let exists = AgentIntrinsicSignatureId::Exists.signature_schema();
        let TypeKind::Probe(item) = exists.groups()[0].parameters()[0]
            .declared_type()
            .expect("exists probe is checked")
        else {
            panic!("exists must retain its generic probe payload")
        };
        let TypeKind::GenericParam(item) = item.as_ref() else {
            panic!("exists payload is generic")
        };
        assert_eq!(
            item.owner(),
            &GenericParameterOwnerId::LanguageIntrinsic(LanguageIntrinsicGenericOwner::FxExists)
        );
    }

    #[test]
    fn typed_rest_is_single_supply_identity_and_both_rest_kinds_are_rejected() {
        let positional = parameter(
            0,
            Some("items"),
            CallableParameterAdmission::checked(TypeKind::String),
            CallableParameterPassing::RestPositional,
            CallableParameterPresence::Required,
        );
        let named = parameter(
            1,
            Some("fields"),
            CallableParameterAdmission::checked(TypeKind::Map {
                kind: MapKind::Sorted,
                key: Box::new(TypeKind::String),
                value: Box::new(TypeKind::String),
            }),
            CallableParameterPassing::RestNamed,
            CallableParameterPresence::Required,
        );
        assert!(matches!(
            CallableParameterGroup::try_new(
                CallableGroupIndex::ZERO,
                CallableGroupKind::Initial,
                vec![positional.clone(), named],
                &PRODUCTION_CALLABLE_LIMITS,
            ),
            Err(CallableSchemaError::InvalidRestParameter { .. })
        ));

        let clearable = parameter(
            0,
            Some("items"),
            CallableParameterAdmission::checked_with_rule(
                TypeKind::String,
                CallableParameterValueRule::clearable_option(),
            ),
            CallableParameterPassing::RestPositional,
            CallableParameterPresence::Required,
        );
        let group = CallableParameterGroup::try_new(
            CallableGroupIndex::ZERO,
            CallableGroupKind::Initial,
            vec![clearable],
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("one rest parameter has a valid shape");
        assert!(matches!(
            CallableSignatureSchema::try_new(
                vec![group],
                TypeKind::Unit,
                CallableEffectSchema::fixed(EffectRow::closed(EffectSet::new())),
                CallableArgumentPolicy::new(
                    UnknownNamedArgumentPolicy::Reject,
                    SpreadArgumentPolicy::TypedRest,
                ),
                CallableValidator::Ordinary,
                CallableGenericParameterIssuer::empty(),
                &PRODUCTION_CALLABLE_LIMITS,
            ),
            Err(CallableSchemaError::InvalidParameterAdmission { .. }
                | CallableSchemaError::InvalidParameterConsumer { .. },)
        ));
    }
}
