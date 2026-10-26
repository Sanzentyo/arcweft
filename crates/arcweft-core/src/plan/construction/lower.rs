//! Admission from semantic seeds into the sole executable carriers.

mod pattern;

// Each variant owns its admission temporaries. Keeping them in separate call
// frames prevents an exhaustive dispatch's unrelated branch storage from
// accumulating on every recursive expression edge in unoptimized builds.
macro_rules! admit_expression_variants {
    (@body $ty:expr, $locals:expr, complete $body:block) => { $body };
    (@body $ty:expr, $locals:expr, $body:block) => {
        {
            let kind = $body;
            Ok(RuntimeExpr::from_admitted_parts_with_guard_copy_locals($ty, kind, $locals))
        }
    };
    ($seed:expr, $ty:expr, $locals:expr, { $($pattern:pat => $($mode:ident)? $body:block $(,)?)* }) => {
        match $seed {
            $($pattern => Self::lower_expression_variant(||
                admit_expression_variants!(@body $ty, $locals, $($mode)? $body)
            ),)*
        }
    };
}

#[cfg(test)]
mod admission_stack_tests;
#[cfg(test)]
mod agent_tests;
#[cfg(test)]
mod field_tests;
#[cfg(test)]
mod static_effect_tests;
#[cfg(test)]
mod value_tests;

use std::collections::BTreeSet;

use arcweft_interaction_model::dialogue::{
    CharacterDialogueOperation, CharacterDialoguePatchField, CharacterDialoguePatchOperation,
};

use crate::audio::RuntimeAudioCommand;
use crate::effect::{LineEffectRequest, RuntimeEffectExpr};
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimePattern,
    RuntimePatternBindingDeclaration,
};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use crate::stream::{StreamMatchArm, StreamOp, StreamPlan};
use crate::task::{
    AwaitManyTarget, HostTaskRequestTemplate, NamedHostArg, RuntimeHostArgumentTemplate,
};
use crate::value::{
    RuntimeAgentConstructor, RuntimeAgentExpr, RuntimeAgentFieldOwner, RuntimeAgentFieldResult,
    RuntimeAgentFieldValue, RuntimeAgentSignatureError, RuntimeAgentTypeContext,
    RuntimeAgentTypeOperand, RuntimeBinaryOp, RuntimeCallArgument, RuntimeCallArgumentMode,
    RuntimeExpr, RuntimeExprKind, RuntimeExprMatchArm, RuntimeFieldProjection,
    RuntimeFmtParameterId, RuntimeFormatContentOperand, RuntimeMutablePlace,
    RuntimeNominalRecordExpr, RuntimeRecordFieldId, RuntimeRecordFieldIdError,
    RuntimeReductionProducer, RuntimeSignedIntWidth, RuntimeStandardMapFamily, RuntimeUnaryOp,
    RuntimeUnsignedIntWidth, RuntimeValue,
};

use super::super::{
    ChoiceRuntimeOption, FlowOp, RuntimeAgentTypeProjection, RuntimeBuiltinIteratorEvidence,
    RuntimeBuiltinIteratorFamily, RuntimeDialogueResultTarget, RuntimeFunctionInputBinding,
    RuntimeFunctionInputSource, RuntimeFunctionSiteBodyKind, RuntimeHostCallTarget,
    RuntimeIteratorEvidence, RuntimeIteratorWitnessEvidence, RuntimeIteratorWitnessExecutable,
    RuntimeLineOperation, RuntimeMatchArm, RuntimeMatchGuard, RuntimePlanRecordField,
    RuntimePlanSequenceKind, RuntimePlanTypeProjection, RuntimePureInputType,
    RuntimePureOutputType, RuntimeReceiverMode, RuntimeSequenceMutation,
};
use super::{
    RuntimeAgentExprSeed, RuntimeAudioCommandSeed, RuntimeBuiltinIteratorEvidenceSeed,
    RuntimeCallArgumentSeed, RuntimeChoiceOptionSeed, RuntimeEvaluatedEffectSeed,
    RuntimeExprMatchArmSeed, RuntimeExprSeed, RuntimeExprSeedKind, RuntimeFieldProjectionSeed,
    RuntimeFlowMatchArmSeed, RuntimeFlowOpSeed, RuntimeHostArgumentSeed, RuntimeHostCallTargetSeed,
    RuntimeHostTaskRequestTemplateSeed, RuntimeIteratorEvidenceSeed,
    RuntimeIteratorWitnessEvidenceSeed, RuntimeIteratorWitnessExecutableSeed,
    RuntimeLineEffectSeed, RuntimeLineOperationSeed, RuntimeLocalSeedId, RuntimeMutablePlaceSeed,
    RuntimeNominalRecordFieldSeed, RuntimePatternRestSeed, RuntimePatternSeed,
    RuntimePatternSeedKind, RuntimePlanBuildError, RuntimePlanBuilder, RuntimeRecordFieldSeedId,
    RuntimeStreamMatchArmSeed, RuntimeStreamOpSeed, RuntimeStreamPlanSeed,
};

enum FlowOpLowerFrame {
    /// Retains the source-order parent list while a nested Match arm is lowered.
    List {
        seeds: std::vec::IntoIter<RuntimeFlowOpSeed>,
        lowered: Vec<FlowOp>,
    },
    /// Lowers each arm body before the next arm, matching recursive admission order.
    MatchArms {
        scrutinee: RuntimeExpr,
        arms: std::vec::IntoIter<RuntimeFlowMatchArmSeed>,
        lowered: Vec<RuntimeMatchArm>,
    },
    /// Receives the completed arm list and appends it to its owning Match.
    FinishMatchArm {
        pattern: RuntimePattern,
        guard: Option<RuntimeMatchGuard>,
    },
    FinishMatchGuard {
        pattern: RuntimePattern,
        candidate: RuntimeLocalDeclarationId,
        result: RuntimePlanTypeId,
        condition: Option<RuntimeExpr>,
        copy_locals: Box<[RuntimeLocalDeclarationId]>,
        arm_ops: Vec<RuntimeFlowOpSeed>,
    },
}

#[derive(Clone)]
struct FlowLocalValidationFrame<'a> {
    ops: &'a [FlowOp],
    next: usize,
    scope: BTreeSet<RuntimeLocalDeclarationId>,
    scope_frames: Vec<BTreeSet<RuntimeLocalDeclarationId>>,
    final_condition: Option<&'a RuntimeExpr>,
}

enum FlowLocalValidationWork<'a> {
    List(FlowLocalValidationFrame<'a>),
    MatchArms {
        arms: std::slice::Iter<'a, RuntimeMatchArm>,
        scope: BTreeSet<RuntimeLocalDeclarationId>,
        scope_frames: Vec<BTreeSet<RuntimeLocalDeclarationId>>,
    },
}

/// Admission context borrowed from the aggregate owner. Its lexical scope is
/// derived only from an admitted function contract, never inferred from seeds.
pub(super) struct RuntimePlanBodyConstruction<'plan> {
    plan: &'plan RuntimePlanBuilder,
    scope: super::super::RuntimeTypeScope,
}

impl std::ops::Deref for RuntimePlanBodyConstruction<'_> {
    type Target = RuntimePlanBuilder;
    fn deref(&self) -> &Self::Target {
        self.plan
    }
}

impl RuntimePlanBuilder {
    pub(super) fn root_body_context(&self) -> RuntimePlanBodyConstruction<'_> {
        RuntimePlanBodyConstruction {
            plan: self,
            scope: super::super::RuntimeTypeScope::root(),
        }
    }

    pub(super) fn function_body_context(
        &self,
        function: Option<RuntimePlanTypeId>,
    ) -> Result<RuntimePlanBodyConstruction<'_>, RuntimePlanBuildError> {
        let Some(function) = function else {
            return Ok(self.root_body_context());
        };
        let declaration =
            self.types
                .get(function)
                .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                    context: "function body contract",
                    ty: function,
                })?;
        let RuntimePlanTypeProjection::Function { contract, .. } = declaration.projection() else {
            return invalid_projection("function body contract", function);
        };
        if !declaration.scope().is_root() {
            return invalid_projection("function body contract", function);
        }
        let scope = contract.child_scope(declaration.scope()).map_err(|_| {
            RuntimePlanBuildError::InvalidTypeProjection {
                context: "function body contract",
                ty: function,
            }
        })?;
        Ok(RuntimePlanBodyConstruction { plan: self, scope })
    }

    #[cfg(test)]
    pub(crate) fn lower_pattern_seed_for_test(
        &self,
        seed: RuntimePatternSeed,
    ) -> Result<RuntimePattern, RuntimePlanBuildError> {
        self.root_body_context().lower_pattern_seed(seed)
    }
    pub(super) fn lower_pattern_seed(
        &self,
        seed: RuntimePatternSeed,
    ) -> Result<RuntimePattern, RuntimePlanBuildError> {
        self.root_body_context().lower_pattern_seed(seed)
    }

    pub(super) fn lower_expression(
        &self,
        seed: RuntimeExprSeed,
    ) -> Result<RuntimeExpr, RuntimePlanBuildError> {
        self.root_body_context().lower_expression(seed)
    }

    pub(super) fn resolve_seed_type(
        &self,
        context: &'static str,
        semantic_identity: crate::pattern::RuntimeSemanticTypeId,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
        self.root_body_context()
            .resolve_seed_type(context, semantic_identity)
    }

    pub(super) fn projection(
        &self,
        ty: RuntimePlanTypeId,
    ) -> Result<&RuntimePlanTypeProjection<RuntimePlanTypeId>, RuntimePlanBuildError> {
        self.root_body_context().projection(ty)
    }

    pub(super) fn validate_callable_input_abi(
        &self,
        context: &'static str,
        inputs: &[(RuntimeLocalDeclarationId, RuntimePlanTypeId)],
        abi: &[RuntimePureInputType],
    ) -> Result<(), RuntimePlanBuildError> {
        self.root_body_context()
            .validate_callable_input_abi(context, inputs, abi)
    }

    pub(super) fn validate_callable_output_abi(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
        abi: RuntimePureOutputType,
    ) -> Result<(), RuntimePlanBuildError> {
        self.root_body_context()
            .validate_callable_output_abi(context, ty, abi)
    }

    pub(super) fn is_string(&self, ty: RuntimePlanTypeId) -> Result<bool, RuntimePlanBuildError> {
        self.root_body_context().is_string(ty)
    }

    pub(super) fn validate_callable_body_locals(
        &self,
        body: &RuntimeExpr,
        params: &[RuntimeLocalDeclarationId],
        captures: &[RuntimeLocalDeclarationId],
    ) -> Result<(), RuntimePlanBuildError> {
        self.root_body_context()
            .validate_callable_body_locals(body, params, captures)
    }

    pub(super) fn lower_stream_plan_seed(
        &self,
        seed: RuntimeStreamPlanSeed,
    ) -> Result<StreamPlan, RuntimePlanBuildError> {
        self.root_body_context().lower_stream_plan_seed(seed)
    }

    pub(super) fn lower_flow_ops(
        &self,
        seeds: Vec<RuntimeFlowOpSeed>,
    ) -> Result<Vec<FlowOp>, RuntimePlanBuildError> {
        self.root_body_context().lower_flow_ops(seeds)
    }

    pub(super) fn validate_flow_operation_locals(
        &self,
        ops: &[FlowOp],
        scope: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        self.root_body_context()
            .validate_flow_operation_locals(ops, scope)
    }

    pub(super) fn validate_line_task_actions_locals(
        &self,
        actions: &[&[FlowOp]],
        captures: &BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<BTreeSet<RuntimeLocalDeclarationId>, RuntimePlanBuildError> {
        self.root_body_context()
            .validate_line_task_actions_locals(actions, captures)
    }
}

impl RuntimeAgentTypeContext for RuntimePlanBodyConstruction<'_> {
    type Type = RuntimePlanTypeId;

    fn is_string(&self, ty: Self::Type) -> bool {
        matches!(self.projection(ty), Ok(RuntimePlanTypeProjection::String))
    }

    fn is_entity_reference(&self, ty: Self::Type) -> bool {
        matches!(
            self.projection(ty),
            Ok(RuntimePlanTypeProjection::EntityReference)
        )
    }

    fn is_u32(&self, ty: Self::Type) -> bool {
        matches!(
            self.projection(ty),
            Ok(RuntimePlanTypeProjection::Unsigned(
                RuntimeUnsignedIntWidth::U32
            ))
        )
    }

    fn agent_type(&self, ty: Self::Type) -> Option<RuntimeAgentTypeProjection<Self::Type>> {
        match self.projection(ty).ok()? {
            RuntimePlanTypeProjection::Agent(agent) => Some(agent.clone()),
            _ => None,
        }
    }

    fn sequence_type(&self, ty: Self::Type) -> Option<(Self::Type, Option<u64>)> {
        match self.projection(ty).ok()? {
            RuntimePlanTypeProjection::Sequence { item, .. } => Some((*item, None)),
            RuntimePlanTypeProjection::Array { item, length } => {
                length.constant().map(|length| (*item, Some(length)))
            }
            _ => None,
        }
    }

    fn tuple_type(&self, ty: Self::Type) -> Option<&[Self::Type]> {
        match self.projection(ty).ok()? {
            RuntimePlanTypeProjection::Tuple(items) => Some(items),
            _ => None,
        }
    }
}

impl RuntimePlanBodyConstruction<'_> {
    #[allow(
        clippy::too_many_lines,
        reason = "the match is the exhaustive expression-admission authority"
    )]
    pub(super) fn lower_expression(
        &self,
        seed: RuntimeExprSeed,
    ) -> Result<RuntimeExpr, RuntimePlanBuildError> {
        self.lower_expression_with_guard_role(seed, false)
    }

    fn lower_guard_expression(
        &self,
        seed: RuntimeExprSeed,
        pattern: &RuntimePattern,
    ) -> Result<RuntimeExpr, RuntimePlanBuildError> {
        let guard = self.lower_expression_with_guard_role(seed, true)?;
        for &local in guard.guard_copy_locals() {
            if !crate::pattern::runtime_pattern_contains_binding(pattern, local) {
                return Err(RuntimePlanBuildError::InvalidGuardCopyBinding { local });
            }
        }
        Ok(guard)
    }

    fn lower_expression_with_guard_role(
        &self,
        seed: RuntimeExprSeed,
        is_pattern_guard: bool,
    ) -> Result<RuntimeExpr, RuntimePlanBuildError> {
        let (semantic_ty, kind, guard_copy_locals) = seed.into_parts();
        if !is_pattern_guard && !guard_copy_locals.is_empty() {
            return Err(RuntimePlanBuildError::GuardCopyRequirementOutsideGuard);
        }
        let guard_copy_locals = guard_copy_locals
            .into_iter()
            .map(|seed| self.resolve_local(&seed).map(|(local, _)| local))
            .collect::<Result<Vec<_>, _>>()?;
        let ty = self.resolve_seed_type("expression", semantic_ty)?;
        admit_expression_variants!(kind, ty, guard_copy_locals, {
            RuntimeExprSeedKind::Value(value) => {
                self.validate_plan_value("expression constant", ty, &value)?;
                RuntimeExprKind::Value(value)
            }
            RuntimeExprSeedKind::Agent(agent) => {
                RuntimeExprKind::Agent(self.lower_agent_expression(ty, agent)?)
            }
            RuntimeExprSeedKind::Local(local) => {
                let (local_seed, mode, fields) = local.into_parts();
                let (local, local_ty) = self.resolve_local(&local_seed)?;
                let mut selected_ty = local_ty;
                let fields = fields
                    .into_vec()
                    .into_iter()
                    .map(|field| {
                        let (field, field_ty) = self.resolve_record_field(selected_ty, field)?;
                        selected_ty = field_ty;
                        Ok(field)
                    })
                    .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
                require_same("local place expression", ty, selected_ty)?;
                RuntimeExprKind::Local(crate::value::RuntimeLocalRead::from_admitted_place(
                    local,
                    mode,
                    fields.into_boxed_slice(),
                ))
            }
            RuntimeExprSeedKind::SequencePopFront { place } => {
                let (place, item) = self.lower_sequence_place(
                    place,
                    "Vec.pop_front place",
                    RuntimeSequenceMutation::PopFront,
                )?;
                self.require_projection("Vec.pop_front result", ty, |projection| {
                    matches!(
                        projection,
                        RuntimePlanTypeProjection::Option { item: result, .. } if *result == item
                    )
                })?;
                RuntimeExprKind::SequencePopFront { place }
            }
            RuntimeExprSeedKind::SequencePush { place, value } => {
                let (place, item) = self.lower_sequence_place(
                    place,
                    "sequence append place",
                    RuntimeSequenceMutation::Append,
                )?;
                let value = self.lower_expression(*value)?;
                require_same("sequence append item", item, value.ty())?;
                self.require_projection("sequence append result", ty, |projection| {
                    matches!(projection, RuntimePlanTypeProjection::Unit)
                })?;
                RuntimeExprKind::SequencePush {
                    place,
                    value: Box::new(value),
                }
            }
            RuntimeExprSeedKind::SequencePopBack { place } => {
                let (place, item) = self.lower_sequence_place(
                    place,
                    "Vec.pop_back place",
                    RuntimeSequenceMutation::PopBack,
                )?;
                self.require_projection("Vec.pop_back result", ty, |projection| {
                    matches!(
                        projection,
                        RuntimePlanTypeProjection::Option { item: result, .. } if *result == item
                    )
                })?;
                RuntimeExprKind::SequencePopBack { place }
            }
            RuntimeExprSeedKind::EntityRef(entity) => {
                self.require_projection("entity-reference expression", ty, |projection| {
                    matches!(projection, RuntimePlanTypeProjection::EntityReference)
                })?;
                RuntimeExprKind::EntityRef(entity)
            }
            RuntimeExprSeedKind::Let {
                binding,
                expr,
                body,
            } => {
                let (binding, binding_ty) = self.resolve_local(&binding)?;
                let expr = self.lower_expression(*expr)?;
                let body = self.lower_expression(*body)?;
                self.require_expression_assignable("let binding", binding_ty, expr.ty())?;
                require_same("let body", ty, body.ty())?;
                RuntimeExprKind::Let {
                    binding,
                    expr: Box::new(expr),
                    body: Box::new(body),
                }
            }
            RuntimeExprSeedKind::Scope { identity, body } => {
                let body = self.lower_expression(*body)?;
                require_same("scope body", ty, body.ty())?;
                RuntimeExprKind::Scope {
                    identity,
                    body: Box::new(body),
                }
            }
            RuntimeExprSeedKind::Tuple(items) => {
                let traversal;
                let expected: &[RuntimePlanTypeId] = match self.projection(ty)? {
                    RuntimePlanTypeProjection::Tuple(items) => items.as_ref(),
                    RuntimePlanTypeProjection::Traversal { source, mapping } => {
                        traversal = [*source, *mapping];
                        &traversal
                    }
                    _ => return invalid_projection("tuple expression", ty),
                };
                if expected.len() != items.len() {
                    return invalid_projection("tuple expression arity", ty);
                }
                let mut lowered = Vec::with_capacity(items.len());
                for (item, expected) in items.into_vec().into_iter().zip(expected) {
                    let item = self.lower_expression(item)?;
                    self.require_expression_assignable("tuple element", *expected, item.ty())?;
                    lowered.push(item);
                }
                RuntimeExprKind::Tuple(lowered)
            }
            RuntimeExprSeedKind::CharacterDialogue {
                operation,
                target,
                fields,
            } => {
                let producer = crate::value::RuntimeCharacterDialogueProducerId::get();
                self.require_projection("CharacterDialogue result", ty, |projection| {
                    matches!(
                        projection,
                        RuntimePlanTypeProjection::Opaque {
                            producer: actual,
                            value_class: crate::value::RuntimeOpaqueValueClass::Plain,
                            persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                            ..
                        } if actual == &producer
                    )
                })?;
                let target = self.lower_expression(*target)?;
                match operation {
                    CharacterDialogueOperation::Factory => {
                        self.require_projection(
                            "CharacterDialogue factory target",
                            target.ty(),
                            |projection| {
                                matches!(projection, RuntimePlanTypeProjection::EntityReference)
                            },
                        )?;
                    }
                    CharacterDialogueOperation::Reconfigure => {
                        self.require_projection("CharacterDialogue reconfigure target", target.ty(), |projection| {
                            matches!(projection, RuntimePlanTypeProjection::Opaque { producer: actual, .. } if actual == &producer)
                        })?;
                    }
                }
                let mut lowered_fields = Vec::with_capacity(fields.len());
                for field in fields.into_vec() {
                    let operation = match field.operation {
                        CharacterDialoguePatchOperation::Set(value) => {
                            CharacterDialoguePatchOperation::Set(self.lower_expression(value)?)
                        }
                        CharacterDialoguePatchOperation::Clear => {
                            CharacterDialoguePatchOperation::Clear
                        }
                    };
                    lowered_fields.push(CharacterDialoguePatchField {
                        coordinate: field.coordinate,
                        operation,
                    });
                }
                RuntimeExprKind::CharacterDialogue {
                    operation,
                    target: Box::new(target),
                    fields: lowered_fields,
                }
            }
            RuntimeExprSeedKind::DialogueContent {
                template,
                values,
                effects,
            } => complete {
                if !self.is_exact_dialogue_content_type(ty) {
                    return Err(RuntimePlanBuildError::InvalidDialogueContentType {
                        slot: crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0)
                            .expect("dialogue content type diagnostics use slot zero"),
                        ty,
                    });
                }
                let manifest = self
                    .dialogue_content
                    .template(template)
                    .ok_or(RuntimePlanBuildError::MissingDialogueTemplateManifest { template })?;
                if manifest.slots().len() != values.len() {
                    return Err(RuntimePlanBuildError::DialogueValueCountMismatch {
                        expected: manifest.slots().len(),
                        actual: values.len(),
                    });
                }
                let expected_slots = manifest.slots();
                let expected_effects = manifest.effects();
                let mut lowered = Vec::with_capacity(values.len());
                for (value, expected) in values.into_vec().into_iter().zip(expected_slots) {
                    let value = self.lower_expression(value)?;
                    let expected_ty = self
                        .resolve_seed_type("dialogue content binding", expected.semantic_type())?;
                    require_same("dialogue content binding", expected_ty, value.ty())?;
                    lowered.push(value);
                }
                if effects.len() != expected_effects.len() {
                    return Err(RuntimePlanBuildError::DialogueEffectCountMismatch {
                        expected: expected_effects.len(),
                        actual: effects.len(),
                    });
                }
                let mut lowered_effects = Vec::with_capacity(effects.len());
                for (index, effect) in effects.into_vec().into_iter().enumerate() {
                    let expected = expected_effects.get(index).ok_or(
                        RuntimePlanBuildError::DialogueEffectCountMismatch {
                            expected: expected_effects.len(),
                            actual: index,
                        },
                    )?;
                    if effect.site != expected.site() {
                        return Err(RuntimePlanBuildError::NonCanonicalDialogueEffectSite {
                            expected: expected.site(),
                            actual: effect.site,
                        });
                    }
                    lowered_effects.push(self.lower_dialogue_effect_site(effect, expected)?.into());
                }
                return Ok(RuntimeExpr::dialogue_content_with_effects(
                    ty,
                    template,
                    lowered,
                    lowered_effects,
                ));
            }
            RuntimeExprSeedKind::FormatContent {
                template,
                attempt,
                operands,
                project_method: project_method_seed,
                project_option,
            } => {
                if !self.is_exact_dialogue_content_type(ty) {
                    return Err(RuntimePlanBuildError::InvalidDialogueContentType {
                        slot: crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(0)
                            .expect("fmt Content diagnostics use slot zero"),
                        ty,
                    });
                }
                let manifest = self
                    .dialogue_content
                    .template(template)
                    .ok_or(RuntimePlanBuildError::MissingDialogueTemplateManifest { template })?;
                let exact_formatted_slot = matches!(
                    manifest.slots(),
                    [slot]
                        if slot.role() == super::super::RuntimeDialogueValueRole::Formatted
                            && slot.semantic_type()
                                == crate::value::RuntimeDialogueOpaqueRole::Content.semantic_identity()
                );
                if !exact_formatted_slot || !manifest.effects().is_empty() {
                    return Err(RuntimePlanBuildError::InvalidFormatContentTemplate { template });
                }

                if let Some(attempt_seed) = attempt {
                    let attempt = attempt_seed
                        .resolve(&self.issuer)
                        .ok_or(RuntimePlanBuildError::ForeignFormatAttemptSeed)?;
                    if !operands.is_empty() {
                        return Err(RuntimePlanBuildError::FormatAttemptHasInlineOperands {
                            attempt,
                        });
                    }
                    let format_attempt = self
                        .format_attempts
                        .get(attempt.index())
                        .filter(|candidate| candidate.id() == attempt)
                        .ok_or(RuntimePlanBuildError::UnknownFormatAttempt { attempt })?;
                    if format_attempt.template() != template {
                        return Err(RuntimePlanBuildError::FormatAttemptTemplateMismatch {
                            attempt,
                            template,
                        });
                    }
                    let value_ty = format_attempt
                        .operands()
                        .iter()
                        .find(|operand| operand.parameter() == RuntimeFmtParameterId::Value)
                        .map(super::super::RuntimeFormatAttemptOperand::ty)
                        .ok_or(RuntimePlanBuildError::MissingFormatPrimaryValue)?;
                    let receiver_type = if project_option {
                        match self.projection(value_ty)? {
                            RuntimePlanTypeProjection::Option { item, .. } => *item,
                            _ => {
                                return Err(RuntimePlanBuildError::InvalidFormatDisplayMethod {
                                    context: "OptionProject receiver type",
                                });
                            }
                        }
                    } else {
                        value_ty
                    };
                    let project_method = project_method_seed
                        .as_ref()
                        .map(|method| {
                            let (method_id, receiver, inputs, result) = method
                                .resolve(&self.issuer)
                                .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
                            if receiver != RuntimeReceiverMode::Owned
                                || inputs.len() != 2
                                || inputs[0] != receiver_type
                            {
                                return Err(RuntimePlanBuildError::InvalidFormatDisplayMethod {
                                    context: "receiver and DisplayContext inputs",
                                });
                            }
                            let returns_content = match self.projection(result)? {
                                RuntimePlanTypeProjection::Result { value, .. } => {
                                    self.is_exact_dialogue_content_type(*value)
                                }
                                _ => false,
                            };
                            if !returns_content {
                                return Err(RuntimePlanBuildError::InvalidFormatDisplayMethod {
                                    context: "Result<Content, DisplayError> output",
                                });
                            }
                            Ok(method_id)
                        })
                        .transpose()?;
                    if project_option && project_method.is_none() {
                        return Err(RuntimePlanBuildError::InvalidFormatDisplayMethod {
                            context: "OptionProject requires a selected project method",
                        });
                    }
                    if project_method.is_none()
                        && (project_option || !self.is_format_display_type(value_ty)?)
                    {
                        return Err(RuntimePlanBuildError::InvalidFormatParameterType {
                            parameter: RuntimeFmtParameterId::Value,
                            ty: value_ty,
                        });
                    }
                    return Ok(RuntimeExpr::from_admitted_parts(
                        ty,
                        RuntimeExprKind::FormatContent {
                            template,
                            attempt: Some(attempt),
                            operands: Vec::new(),
                            project_method,
                            project_option,
                        },
                    ));
                }

                let mut seen = BTreeSet::new();
                let mut primary_present = false;
                let mut failure_policy_count = 0;
                let mut lowered = Vec::with_capacity(operands.len());
                for operand in operands.into_vec() {
                    if !seen.insert(operand.parameter) {
                        return Err(RuntimePlanBuildError::DuplicateFormatParameter {
                            parameter: operand.parameter,
                        });
                    }
                    if operand.parameter == RuntimeFmtParameterId::Value {
                        primary_present = true;
                    }
                    if matches!(
                        operand.parameter,
                        RuntimeFmtParameterId::OnError
                            | RuntimeFmtParameterId::Fallback
                            | RuntimeFmtParameterId::DiscardError
                    ) {
                        failure_policy_count += 1;
                        if failure_policy_count > 1 {
                            return Err(RuntimePlanBuildError::ConflictingFormatFailurePolicy);
                        }
                    }
                    let expression = self.lower_expression(operand.expression)?;
                    let valid_type = match operand.parameter {
                        RuntimeFmtParameterId::Style
                        | RuntimeFmtParameterId::Locale
                        | RuntimeFmtParameterId::Currency
                        | RuntimeFmtParameterId::NoneValue
                        | RuntimeFmtParameterId::Fallback => self.is_string(expression.ty())?,
                        RuntimeFmtParameterId::Color => {
                            matches!(
                                self.projection(expression.ty())?,
                                RuntimePlanTypeProjection::Color
                            )
                        }
                        RuntimeFmtParameterId::DiscardError => {
                            matches!(
                                self.projection(expression.ty())?,
                                RuntimePlanTypeProjection::Bool
                            )
                        }
                        RuntimeFmtParameterId::Value => match project_method_seed.as_ref() {
                            Some(method) => {
                                let receiver_type = if project_option {
                                    match self.projection(expression.ty())? {
                                        RuntimePlanTypeProjection::Option { item, .. } => *item,
                                        _ => {
                                            return Err(
                                                RuntimePlanBuildError::InvalidFormatDisplayMethod {
                                                    context: "OptionProject receiver type",
                                                },
                                            );
                                        }
                                    }
                                } else {
                                    expression.ty()
                                };
                                let (_, receiver, inputs, result) = method
                                    .resolve(&self.issuer)
                                    .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
                                if receiver != RuntimeReceiverMode::Owned
                                    || inputs.len() != 2
                                    || inputs[0] != receiver_type
                                {
                                    return Err(
                                        RuntimePlanBuildError::InvalidFormatDisplayMethod {
                                            context: "receiver and DisplayContext inputs",
                                        },
                                    );
                                }
                                let returns_content = match self.projection(result)? {
                                    RuntimePlanTypeProjection::Result { value, .. } => {
                                        self.is_exact_dialogue_content_type(*value)
                                    }
                                    _ => false,
                                };
                                if !returns_content {
                                    return Err(
                                        RuntimePlanBuildError::InvalidFormatDisplayMethod {
                                            context: "Result<Content, DisplayError> output",
                                        },
                                    );
                                }
                                true
                            }
                            None => {
                                !project_option && self.is_format_display_type(expression.ty())?
                            }
                        },
                        // InlineFailure's nominal identity belongs to the
                        // selected project schema, which Core deliberately
                        // does not duplicate. The selected producer's checked
                        // type remains attached to this value expression.
                        RuntimeFmtParameterId::OnError => true,
                    };
                    if !valid_type {
                        return Err(RuntimePlanBuildError::InvalidFormatParameterType {
                            parameter: operand.parameter,
                            ty: expression.ty(),
                        });
                    }
                    lowered.push(RuntimeFormatContentOperand::from_admitted_parts(
                        operand.definition,
                        operand.parameter,
                        expression,
                    ));
                }
                if !primary_present {
                    return Err(RuntimePlanBuildError::MissingFormatPrimaryValue);
                }
                if project_option && project_method_seed.is_none() {
                    return Err(RuntimePlanBuildError::InvalidFormatDisplayMethod {
                        context: "OptionProject requires a selected project method",
                    });
                }
                let project_method = project_method_seed
                    .as_ref()
                    .map(|method| {
                        method
                            .resolve(&self.issuer)
                            .map(|(method_id, _, _, _)| method_id)
                            .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)
                    })
                    .transpose()?;
                RuntimeExprKind::FormatContent {
                    template,
                    attempt: None,
                    operands: lowered,
                    project_method,
                    project_option,
                }
            }
            RuntimeExprSeedKind::BracketSeq(items) => {
                let (item_ty, fixed_len) = self.sequence_projection(ty, "sequence expression")?;
                if let Some(expected) = fixed_len {
                    validate_sequence_length(expected, items.len())?;
                }
                let mut lowered = Vec::with_capacity(items.len());
                for item in items {
                    let item = self.lower_expression(item)?;
                    require_same("sequence element", item_ty, item.ty())?;
                    lowered.push(item);
                }
                RuntimeExprKind::BracketSeq(lowered)
            }
            RuntimeExprSeedKind::RepeatSeq { value, len } => {
                let (item_ty, fixed_len) = self.sequence_projection(ty, "repeat expression")?;
                if let Some(expected) = fixed_len {
                    validate_sequence_length(expected, len)?;
                }
                let value = self.lower_expression(*value)?;
                require_same("repeat element", item_ty, value.ty())?;
                RuntimeExprKind::RepeatSeq {
                    value: Box::new(value),
                    len,
                }
            }
            RuntimeExprSeedKind::ArrayFromVec { value } => {
                let item = match self.projection(ty)? {
                    RuntimePlanTypeProjection::Array { item, .. } => *item,
                    _ => return invalid_projection("Array construction result", ty),
                };
                let value = self.lower_expression(*value)?;
                self.require_projection("Array construction source", value.ty(), |projection| {
                    matches!(projection, RuntimePlanTypeProjection::Sequence {
                        kind: super::super::RuntimePlanSequenceKind::Vec,
                        item: source_item,
                    } if *source_item == item)
                })?;
                RuntimeExprKind::ArrayFromVec {
                    value: Box::new(value),
                }
            }
            RuntimeExprSeedKind::Range {
                start,
                end,
                inclusive,
            } => {
                if start.is_none() && end.is_none() {
                    return Err(RuntimePlanBuildError::EmptyRangeExpression);
                }
                let item_ty = match self.projection(ty)? {
                    RuntimePlanTypeProjection::Range(item) => *item,
                    _ => return invalid_projection("range expression", ty),
                };
                let start = self.lower_optional_expression(start)?;
                let end = self.lower_optional_expression(end)?;
                for bound in start.iter().chain(end.iter()) {
                    require_same("range bound", item_ty, bound.ty())?;
                }
                RuntimeExprKind::Range {
                    start: start.map(Box::new),
                    end: end.map(Box::new),
                    inclusive,
                }
            }
            RuntimeExprSeedKind::NominalRecord(fields) => {
                RuntimeExprKind::NominalRecord(self.lower_nominal_record(ty, fields)?)
            }
            RuntimeExprSeedKind::Variant { ordinal, payload } => {
                let payload = self.lower_optional_expression(payload)?;
                self.validate_variant_payload(ty, ordinal, payload.as_ref().map(RuntimeExpr::ty))?;
                RuntimeExprKind::Variant {
                    ordinal,
                    payload: payload.map(Box::new),
                }
            }
            RuntimeExprSeedKind::Field { target, field } => {
                let target = match target {
                    super::RuntimeFieldTargetSeed::Value(value) => {
                        crate::value::RuntimeFieldTarget::Value(Box::new(
                            self.lower_expression(*value)?,
                        ))
                    }
                    super::RuntimeFieldTargetSeed::Inspect(place) => {
                        let (place, ty) = self.lower_mutable_place(place, "field inspection")?;
                        crate::value::RuntimeFieldTarget::Inspect { place, ty }
                    }
                };
                let field = self.lower_field_projection(ty, target.ty(), &field)?;
                if matches!(target, crate::value::RuntimeFieldTarget::Inspect { .. })
                    && !self.field_inspection_result_is_copy(
                        ty,
                        0,
                        &mut std::collections::BTreeMap::new(),
                    )?
                {
                    return invalid_projection("Copy field inspection result", ty);
                }
                RuntimeExprKind::Field { target, field }
            }
            RuntimeExprSeedKind::ProjectTuple { target, ordinal } => {
                let target = self.lower_expression(*target)?;
                let ordinal = usize::try_from(ordinal).map_err(|_| {
                    RuntimePlanBuildError::InvalidTypeProjection {
                        context: "tuple projection ordinal",
                        ty: target.ty(),
                    }
                })?;
                let expected = match self.projection(target.ty())? {
                    RuntimePlanTypeProjection::Tuple(items) => items.get(ordinal).copied(),
                    _ => None,
                }
                .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                    context: "tuple projection",
                    ty: target.ty(),
                })?;
                require_same("tuple projection result", expected, ty)?;
                RuntimeExprKind::ProjectTuple {
                    target: Box::new(target),
                    ordinal,
                }
            }
            RuntimeExprSeedKind::ProjectRecord { target, field } => {
                let target = self.lower_expression(*target)?;
                let (field, field_ty) = self.resolve_record_field(target.ty(), field)?;
                require_same("record projection result", field_ty, ty)?;
                RuntimeExprKind::ProjectRecord {
                    target: Box::new(target),
                    ordinal: usize::try_from(field.zero_based()).map_err(|_| {
                        RuntimePlanBuildError::RecordFieldIdentity(
                            RuntimeRecordFieldIdError::OrdinalOverflow,
                        )
                    })?,
                }
            }
            RuntimeExprSeedKind::Assign { place, expr, body } => {
                let (place, place_ty) = self.lower_assignment_place(place, "assignment place")?;
                let expr = self.lower_expression(*expr)?;
                let body = self.lower_expression(*body)?;
                require_same("assigned place", place_ty, expr.ty())?;
                require_same("assignment body", ty, body.ty())?;
                RuntimeExprKind::Assign {
                    place,
                    expr: Box::new(expr),
                    body: Box::new(body),
                }
            }
            RuntimeExprSeedKind::Call { callee, args } => {
                let args = self.lower_call_arguments(args)?;
                if let Some(intrinsic) = callee.as_intrinsic() {
                    self.validate_collection_intrinsic_signature(intrinsic, &args, ty)?;
                }
                RuntimeExprKind::Call { callee, args }
            }
            RuntimeExprSeedKind::Function { site, captures } => {
                let (_, input_sources, input_types, ..) =
                    site.resolve(&self.issuer)
                        .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
                let capture_types = input_sources
                    .iter()
                    .zip(input_types)
                    .filter_map(|(source, ty)| {
                        matches!(
                            source,
                            RuntimeFunctionInputSource::Capture { .. }
                                | RuntimeFunctionInputSource::CapturedParameter { .. }
                        )
                        .then_some(*ty)
                    })
                    .collect::<Vec<_>>();
                if captures.len() != capture_types.len() {
                    return Err(RuntimePlanBuildError::CallableAbiArity {
                        context: "function capture expressions",
                        expected: capture_types.len(),
                        actual: captures.len(),
                    });
                }
                let captures = captures
                    .into_vec()
                    .into_iter()
                    .zip(capture_types)
                    .map(|(capture, expected)| {
                        let capture = self.lower_expression(capture)?;
                        require_same("function capture expression", expected, capture.ty())?;
                        Ok(capture)
                    })
                    .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
                let state = self.function_callable_state(ty, &site)?;
                RuntimeExprKind::MakeCallable { state, captures }
            }
            RuntimeExprSeedKind::MakeCallable { state, captures } => {
                let state = self.resolve_callable_state_seed(&state)?;
                let definition = self.callable_states.borrow().states[state.index()]
                    .clone()
                    .ok_or(RuntimePlanBuildError::UnknownCallableState { state })?;
                require_same("callable expression", definition.function_type, ty)?;
                if captures.len() != definition.retained.len() {
                    return Err(RuntimePlanBuildError::CallableAbiArity {
                        context: "callable retained values",
                        expected: definition.retained.len(),
                        actual: captures.len(),
                    });
                }
                let captures = captures
                    .into_vec()
                    .into_iter()
                    .zip(&definition.retained)
                    .map(|(capture, input)| {
                        let capture = self.lower_expression(capture)?;
                        require_same("callable retained value", input.ty, capture.ty())?;
                        Ok(capture)
                    })
                    .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
                RuntimeExprKind::MakeCallable { state, captures }
            }
            RuntimeExprSeedKind::SpecializeCallable {
                value,
                specialization,
            } => {
                let value = self.lower_expression(*value)?;
                let (specialization, definition) =
                    self.resolve_callable_specialization_seed(&specialization)?;
                require_same("specialization source", definition.source_type, value.ty())?;
                require_same("specialization target", definition.target_type, ty)?;
                RuntimeExprKind::SpecializeCallable {
                    value: Box::new(value),
                    specialization,
                }
            }
            RuntimeExprSeedKind::Apply { callee, args } => {
                let (callee, args) = self.lower_function_application(*callee, args, ty)?;
                RuntimeExprKind::ApplyGroup {
                    callee: Box::new(callee),
                    args,
                }
            }
            RuntimeExprSeedKind::TraitCall {
                callable,
                receiver,
                args,
            } => {
                let (callable, receiver_mode, parameters, result) = callable
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
                if self.trait_methods.get(callable.0).is_none() {
                    return Err(RuntimePlanBuildError::ForeignTraitMethodSeed);
                }
                let Some((&receiver_ty, argument_types)) = parameters.split_first() else {
                    return Err(RuntimePlanBuildError::MissingTraitMethodReceiver);
                };
                let receiver = self.lower_expression(*receiver)?;
                require_same("trait-call receiver", receiver_ty, receiver.ty())?;
                let args = self.lower_call_arguments(args)?;
                let actual = self.expanded_argument_types(&args)?;
                if argument_types != actual.as_slice() {
                    return invalid_projection("trait-call arguments", ty);
                }
                require_same("trait-call result", result, ty)?;
                RuntimeExprKind::TraitCall {
                    callable,
                    receiver: Box::new(receiver),
                    receiver_mode,
                    args,
                }
            }
            RuntimeExprSeedKind::PureCall { helper, args } => {
                let (helper, parameters, result) = helper
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignPureHelperSeed)?;
                if self.pure_helpers.get(helper.0).is_none() {
                    return Err(RuntimePlanBuildError::ForeignPureHelperSeed);
                }
                let args = self.lower_call_arguments(args)?;
                let actual = self.expanded_argument_types(&args)?;
                if parameters != actual.as_slice() {
                    return invalid_projection("pure-call arguments", ty);
                }
                require_same("pure-call result", result, ty)?;
                RuntimeExprKind::PureCall { helper, args }
            }
            RuntimeExprSeedKind::Sum { source } => {
                let source = self.lower_expression(*source)?;
                let (item, _) = self.sequence_projection(source.ty(), "sum source expression")?;
                if !matches!(
                    self.projection(item)?,
                    RuntimePlanTypeProjection::Signed(_) | RuntimePlanTypeProjection::Unsigned(_)
                ) {
                    return invalid_projection("sum element", item);
                }
                if !matches!(
                    self.projection(ty)?,
                    RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64)
                ) {
                    return invalid_projection("sum result", ty);
                }
                RuntimeExprKind::Sum {
                    source: Box::new(source),
                }
            }
            RuntimeExprSeedKind::Unary { op, expr } => {
                let expr = self.lower_expression(*expr)?;
                self.validate_unary(op, expr.ty(), ty)?;
                RuntimeExprKind::Unary {
                    op,
                    expr: Box::new(expr),
                }
            }
            RuntimeExprSeedKind::Binary { lhs, op, rhs } => {
                let lhs = self.lower_expression(*lhs)?;
                let rhs = self.lower_expression(*rhs)?;
                self.validate_binary(op, lhs.ty(), rhs.ty(), ty)?;
                RuntimeExprKind::Binary {
                    lhs: Box::new(lhs),
                    op,
                    rhs: Box::new(rhs),
                }
            }
            RuntimeExprSeedKind::If {
                condition,
                then_expr,
                else_expr,
            } => {
                let condition = self.lower_expression(*condition)?;
                let then_expr = self.lower_expression(*then_expr)?;
                let else_expr = self.lower_expression(*else_expr)?;
                self.require_bool("if condition", condition.ty())?;
                self.require_expression_assignable("if then branch", ty, then_expr.ty())?;
                self.require_expression_assignable("if else branch", ty, else_expr.ty())?;
                RuntimeExprKind::If {
                    condition: Box::new(condition),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                }
            }
            RuntimeExprSeedKind::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(*expr)?;
                require_same("if-let scrutinee", pattern.ty(), expr.ty())?;
                let guard = guard
                    .map(|guard| self.lower_guard_expression(*guard, &pattern))
                    .transpose()?;
                if let Some(guard) = &guard {
                    self.require_bool("if-let guard", guard.ty())?;
                }
                let then_expr = self.lower_expression(*then_expr)?;
                let else_expr = self.lower_expression(*else_expr)?;
                self.require_expression_assignable("if-let then branch", ty, then_expr.ty())?;
                self.require_expression_assignable("if-let else branch", ty, else_expr.ty())?;
                RuntimeExprKind::IfLet {
                    pattern,
                    expr: Box::new(expr),
                    guard: guard.map(Box::new),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                }
            }
            RuntimeExprSeedKind::Match { scrutinee, arms } => {
                let scrutinee = self.lower_expression(*scrutinee)?;
                let mut lowered = Vec::with_capacity(arms.len());
                for arm in arms {
                    lowered.push(self.lower_match_arm(scrutinee.ty(), ty, arm)?);
                }
                RuntimeExprKind::Match {
                    scrutinee: Box::new(scrutinee),
                    arms: lowered,
                }
            }
            RuntimeExprSeedKind::ReductionUnchanged { state } => {
                let state = self.lower_expression(*state)?;
                let state_ty = match self.projection(state.ty())? {
                    RuntimePlanTypeProjection::Reference(inner) => *inner,
                    _ => state.ty(),
                };
                match self.projection(ty)? {
                    RuntimePlanTypeProjection::Opaque {
                        producer,
                        admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class: crate::value::RuntimeOpaqueValueClass::Plain,
                        persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                        arguments,
                    } if RuntimeReductionProducer::accepts(producer)
                        && arguments.as_ref() == [state_ty] => {}
                    _ => return Err(RuntimePlanBuildError::InvalidReductionUnchanged { ty }),
                }
                RuntimeExprKind::ReductionUnchanged {
                    state: Box::new(state),
                }
            }
        })
    }

    /// The variant's frame contains only its own construction work. This
    /// boundary remains separate from recursive exhaustive source dispatch.
    #[inline(never)]
    fn lower_expression_variant(
        admit: impl FnOnce() -> Result<RuntimeExpr, RuntimePlanBuildError>,
    ) -> Result<RuntimeExpr, RuntimePlanBuildError> {
        admit()
    }

    fn lower_optional_expression(
        &self,
        seed: Option<Box<RuntimeExprSeed>>,
    ) -> Result<Option<RuntimeExpr>, RuntimePlanBuildError> {
        seed.map(|seed| self.lower_expression(*seed)).transpose()
    }

    /// An exact producer-owned opaque value may flow into that producer's
    /// admitted top type. Its runtime value retains the exact identity.
    pub(super) fn require_expression_assignable(
        &self,
        context: &'static str,
        expected: RuntimePlanTypeId,
        actual: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        if expected == actual {
            return Ok(());
        }
        if matches!(self.projection(expected)?, RuntimePlanTypeProjection::Choice(alternatives) if alternatives.contains(&actual))
        {
            return Ok(());
        }
        let accepts = matches!(
            (self.projection(expected)?, self.projection(actual)?),
            (
                RuntimePlanTypeProjection::Opaque {
                    producer: expected_producer,
                    admission: RuntimeOpaqueTypeAdmission::ProducerWide,
                    value_class: expected_class,
                    persistence: expected_persistence,
                    arguments: expected_arguments,
                },
                RuntimePlanTypeProjection::Opaque {
                    producer: actual_producer,
                    admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                    value_class: actual_class,
                    persistence: actual_persistence,
                    arguments: actual_arguments,
                },
            ) if expected_producer == actual_producer
                && expected_class == actual_class
                && expected_persistence == actual_persistence
                && expected_arguments == actual_arguments
        );
        if accepts {
            Ok(())
        } else {
            require_same(context, expected, actual)
        }
    }

    fn lower_match_arm(
        &self,
        scrutinee_ty: RuntimePlanTypeId,
        result_ty: RuntimePlanTypeId,
        arm: RuntimeExprMatchArmSeed,
    ) -> Result<RuntimeExprMatchArm, RuntimePlanBuildError> {
        let (pattern, guard, value) = arm.into_parts();
        let pattern = self.lower_pattern_seed(pattern)?;
        require_same("match arm pattern", scrutinee_ty, pattern.ty())?;
        let guard = guard
            .map(|guard| self.lower_guard_expression(guard, &pattern))
            .transpose()?;
        if let Some(guard) = &guard {
            self.require_bool("match arm guard", guard.ty())?;
        }
        let value = self.lower_expression(value)?;
        self.require_expression_assignable("match arm value", result_ty, value.ty())?;
        Ok(RuntimeExprMatchArm::from_admitted_parts(
            pattern, guard, value,
        ))
    }
}

pub(super) fn require_same(
    context: &'static str,
    expected: RuntimePlanTypeId,
    actual: RuntimePlanTypeId,
) -> Result<(), RuntimePlanBuildError> {
    if expected == actual {
        Ok(())
    } else {
        Err(RuntimePlanBuildError::TypeMismatch {
            context,
            expected,
            actual,
        })
    }
}

fn invalid_projection<T>(
    context: &'static str,
    ty: RuntimePlanTypeId,
) -> Result<T, RuntimePlanBuildError> {
    Err(RuntimePlanBuildError::InvalidTypeProjection { context, ty })
}

fn validate_sequence_length(expected: u64, actual: usize) -> Result<(), RuntimePlanBuildError> {
    if usize::try_from(expected).ok() == Some(actual) {
        Ok(())
    } else {
        Err(RuntimePlanBuildError::SequenceLengthMismatch { expected, actual })
    }
}

impl<'plan> RuntimePlanBodyConstruction<'plan> {
    /// Resolves a value in the authenticated frame scope. Root construction
    /// accepts only closed roots; scoped descendants cannot escape their owner.
    pub(super) fn resolve_seed_type(
        &self,
        context: &'static str,
        semantic_identity: crate::pattern::RuntimeSemanticTypeId,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
        let ty = self.types.id_for_semantic(semantic_identity).ok_or(
            RuntimePlanBuildError::UnknownSeedType {
                context,
                semantic_identity,
            },
        )?;
        self.validate_value_type_scope(context, ty)?;
        Ok(ty)
    }

    pub(super) fn validate_value_type_scope(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        if !self
            .types
            .get(ty)
            .is_some_and(|row| row.scope().is_root() || row.scope() == &self.scope)
        {
            return invalid_projection(context, ty);
        }
        Ok(())
    }

    pub(super) fn projection(
        &self,
        ty: RuntimePlanTypeId,
    ) -> Result<&'plan RuntimePlanTypeProjection<RuntimePlanTypeId>, RuntimePlanBuildError> {
        self.plan
            .types
            .get(ty)
            .map(super::super::RuntimePlanTypeDeclaration::projection)
            .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                context: "plan-local type identity",
                ty,
            })
    }

    fn require_projection(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
        accepts: impl FnOnce(&RuntimePlanTypeProjection<RuntimePlanTypeId>) -> bool,
    ) -> Result<(), RuntimePlanBuildError> {
        if accepts(self.projection(ty)?) {
            Ok(())
        } else {
            invalid_projection(context, ty)
        }
    }

    fn resolve_local(
        &self,
        local: &RuntimeLocalSeedId,
    ) -> Result<(RuntimeLocalDeclarationId, RuntimePlanTypeId), RuntimePlanBuildError> {
        let (local, ty) = local
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignLocalSeed)?;
        if !self
            .locals
            .lock()
            .expect("local request table only runs checked, callback-free transitions")
            .contains(local)
        {
            return Err(RuntimePlanBuildError::UnknownFunctionLocal { local });
        }
        Ok((local, ty))
    }

    fn sequence_projection(
        &self,
        ty: RuntimePlanTypeId,
        context: &'static str,
    ) -> Result<(RuntimePlanTypeId, Option<u64>), RuntimePlanBuildError> {
        match self.projection(ty)? {
            RuntimePlanTypeProjection::Sequence { item, .. } => Ok((*item, None)),
            RuntimePlanTypeProjection::Array { item, length } => length
                .constant()
                .map(|length| (*item, Some(length)))
                .ok_or(RuntimePlanBuildError::InvalidTypeProjection {
                    context: "closed array length",
                    ty,
                }),
            _ => invalid_projection(context, ty),
        }
    }

    fn standard_map_item_projection(
        &self,
        family: RuntimeStandardMapFamily,
        source: RuntimePlanTypeId,
        result: RuntimePlanTypeId,
    ) -> Result<(RuntimePlanTypeId, RuntimePlanTypeId), RuntimePlanBuildError> {
        match (family, self.projection(source)?, self.projection(result)?) {
            (
                RuntimeStandardMapFamily::Vec,
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item: input,
                },
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item: output,
                },
            )
            | (
                RuntimeStandardMapFamily::Seq,
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Seq,
                    item: input,
                },
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Seq,
                    item: output,
                },
            )
            | (
                RuntimeStandardMapFamily::Slice,
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Slice,
                    item: input,
                },
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item: output,
                },
            )
            | (
                RuntimeStandardMapFamily::Option,
                RuntimePlanTypeProjection::Option { item: input, .. },
                RuntimePlanTypeProjection::Option { item: output, .. },
            ) => Ok((*input, *output)),
            (
                RuntimeStandardMapFamily::Array,
                RuntimePlanTypeProjection::Array {
                    item: input,
                    length: input_length,
                },
                RuntimePlanTypeProjection::Array {
                    item: output,
                    length: output_length,
                },
            ) if input_length == output_length => Ok((*input, *output)),
            (
                RuntimeStandardMapFamily::Result,
                RuntimePlanTypeProjection::Result {
                    value: input,
                    error: input_error,
                    ..
                },
                RuntimePlanTypeProjection::Result {
                    value: output,
                    error: output_error,
                    ..
                },
            ) if input_error == output_error => Ok((*input, *output)),
            _ => invalid_projection("standard map family", source),
        }
    }

    fn lower_nominal_record(
        &self,
        owner: RuntimePlanTypeId,
        fields: Box<[RuntimeNominalRecordFieldSeed]>,
    ) -> Result<RuntimeNominalRecordExpr, RuntimePlanBuildError> {
        let domain = self
            .nominal_record_domains
            .get(owner)
            .ok_or(RuntimePlanBuildError::UnknownNominalRecordDomain { owner })?;
        let mut seen = BTreeSet::new();
        let mut lowered = Vec::with_capacity(fields.len());
        for field in fields {
            let (field, value) = field.into_parts();
            let (field, expected) = self.resolve_record_field(owner, field)?;
            if !seen.insert(field) {
                return Err(RuntimePlanBuildError::DuplicateRecordField { owner, field });
            }
            let value = self.lower_expression(value)?;
            require_same("nominal record field", expected, value.ty())?;
            lowered.push((field, value));
        }
        for field in domain.fields() {
            let field = field.field();
            if !seen.contains(&field) {
                return Err(RuntimePlanBuildError::MissingRecordField { owner, field });
            }
        }
        Ok(RuntimeNominalRecordExpr::from_admitted_parts(lowered))
    }

    fn resolve_record_field(
        &self,
        owner: RuntimePlanTypeId,
        field: RuntimeRecordFieldSeedId,
    ) -> Result<(RuntimeRecordFieldId, RuntimePlanTypeId), RuntimePlanBuildError> {
        let domain = self
            .nominal_record_domains
            .get(owner)
            .ok_or(RuntimePlanBuildError::UnknownNominalRecordDomain { owner })?;
        let ordinal = usize::try_from(field.zero_based()).map_err(|_| {
            RuntimePlanBuildError::RecordFieldIdentity(RuntimeRecordFieldIdError::OrdinalOverflow)
        })?;
        let field =
            domain
                .fields()
                .get(ordinal)
                .ok_or(RuntimePlanBuildError::UnknownRecordField {
                    owner,
                    ordinal: field.zero_based(),
                })?;
        Ok((field.field(), field.ty()))
    }

    fn resolve_pattern_record_field(
        &self,
        owner: RuntimePlanTypeId,
        field: RuntimeRecordFieldSeedId,
    ) -> Result<(RuntimeRecordFieldId, RuntimePlanTypeId), RuntimePlanBuildError> {
        if self.nominal_record_domains.get(owner).is_some() {
            return self.resolve_record_field(owner, field);
        }
        let ordinal = usize::try_from(field.zero_based()).map_err(|_| {
            RuntimePlanBuildError::RecordFieldIdentity(RuntimeRecordFieldIdError::OrdinalOverflow)
        })?;
        let admitted = RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)?;
        let RuntimePlanTypeProjection::Record(fields) = self.projection(owner)? else {
            return invalid_projection("structural record pattern owner", owner);
        };
        let field_ty = fields
            .get(ordinal)
            .map(RuntimePlanRecordField::ty)
            .copied()
            .ok_or(RuntimePlanBuildError::UnknownRecordField {
                owner,
                ordinal: field.zero_based(),
            })?;
        Ok((admitted, field_ty))
    }

    /// Proves unrestricted results from this builder's admitted type and nominal
    /// domains. Value-dependent carriers require evidence this operation does
    /// not publish and therefore cannot be inspected by copying.
    fn field_inspection_result_is_copy(
        &self,
        ty: RuntimePlanTypeId,
        depth: usize,
        memo: &mut std::collections::BTreeMap<RuntimePlanTypeId, Option<bool>>,
    ) -> Result<bool, RuntimePlanBuildError> {
        if depth > 64 {
            return Ok(false);
        }
        if let Some(proof) = memo.get(&ty) {
            return Ok(*proof == Some(true));
        }
        if memo.len() >= 65_536 {
            return Ok(false);
        }
        memo.insert(ty, None);
        let projection = self.projection(ty)?;
        let children: Option<Vec<RuntimePlanTypeId>> = match projection {
            RuntimePlanTypeProjection::Never
            | RuntimePlanTypeProjection::Unit
            | RuntimePlanTypeProjection::Bool
            | RuntimePlanTypeProjection::Signed(_)
            | RuntimePlanTypeProjection::Unsigned(_)
            | RuntimePlanTypeProjection::F32
            | RuntimePlanTypeProjection::F64
            | RuntimePlanTypeProjection::String
            | RuntimePlanTypeProjection::Color
            | RuntimePlanTypeProjection::Char
            | RuntimePlanTypeProjection::Bytes
            | RuntimePlanTypeProjection::Duration
            | RuntimePlanTypeProjection::Progress
            | RuntimePlanTypeProjection::DenseNumeric(_)
            | RuntimePlanTypeProjection::EntityReference => Some(Vec::new()),
            RuntimePlanTypeProjection::Range(_)
            | RuntimePlanTypeProjection::Sequence { .. }
            | RuntimePlanTypeProjection::Array { .. }
            | RuntimePlanTypeProjection::Map { .. }
            | RuntimePlanTypeProjection::Result { .. }
            | RuntimePlanTypeProjection::Option { .. }
            | RuntimePlanTypeProjection::BuiltinVariant { .. }
            | RuntimePlanTypeProjection::Tuple(_)
            | RuntimePlanTypeProjection::Record(_)
            | RuntimePlanTypeProjection::Choice(_) => {
                Some(projection.children().iter().map(|child| **child).collect())
            }
            RuntimePlanTypeProjection::Nominal { .. } => {
                if let Some(domain) = self.nominal_record_domains.get(ty) {
                    Some(domain.fields().iter().map(|field| field.ty()).collect())
                } else {
                    self.variant_domains.get(ty).map(|domain| {
                        domain
                            .cases()
                            .iter()
                            .filter_map(|case| case.payload())
                            .collect()
                    })
                }
            }
            RuntimePlanTypeProjection::BoundType(_)
            | RuntimePlanTypeProjection::AgentValue
            | RuntimePlanTypeProjection::Iterator(_)
            | RuntimePlanTypeProjection::Need(_)
            | RuntimePlanTypeProjection::Traversal { .. }
            | RuntimePlanTypeProjection::Stream { .. }
            | RuntimePlanTypeProjection::ThreadHandle(_)
            | RuntimePlanTypeProjection::Shared(_)
            | RuntimePlanTypeProjection::Reference(_)
            | RuntimePlanTypeProjection::Function { .. }
            | RuntimePlanTypeProjection::Opaque { .. }
            | RuntimePlanTypeProjection::Agent(_) => None,
        };
        let mut copy = children.is_some();
        if let Some(children) = children {
            for child in children {
                if !self.field_inspection_result_is_copy(child, depth + 1, memo)? {
                    copy = false;
                    break;
                }
            }
        }
        memo.insert(ty, Some(copy));
        Ok(copy)
    }

    fn lower_field_projection(
        &self,
        result_ty: RuntimePlanTypeId,
        target_ty: RuntimePlanTypeId,
        field: &RuntimeFieldProjectionSeed,
    ) -> Result<RuntimeFieldProjection, RuntimePlanBuildError> {
        match field {
            RuntimeFieldProjectionSeed::Nominal { owner, field } => {
                let owner = self.resolve_seed_type("nominal field owner", *owner)?;
                require_same("nominal field target", owner, target_ty)?;
                let (field, field_ty) = self.resolve_record_field(owner, *field)?;
                require_same("nominal field result", field_ty, result_ty)?;
                Ok(RuntimeFieldProjection::Nominal(field))
            }
            RuntimeFieldProjectionSeed::OpaqueRecord {
                owner,
                producer,
                field,
                field_type,
            } => {
                let semantic_owner = *owner;
                let owner = self.resolve_seed_type("opaque record field owner", semantic_owner)?;
                require_same("opaque record field target", owner, target_ty)?;
                let expected_result =
                    self.resolve_seed_type("opaque record field result", *field_type)?;
                require_same("opaque record field result", expected_result, result_ty)?;
                let opaque_owner = match self.projection(target_ty)? {
                    RuntimePlanTypeProjection::Opaque {
                        producer: accepted,
                        admission,
                        value_class,
                        persistence,
                        ..
                    } if accepted == producer
                        && *admission == RuntimeOpaqueTypeAdmission::ExactIdentity =>
                    {
                        RuntimeOpaqueTypeOwner::with_admission(
                            accepted.clone(),
                            semantic_owner,
                            *admission,
                            *value_class,
                            *persistence,
                        )
                    }
                    _ => return invalid_projection("opaque record field target", target_ty),
                };
                let ordinal = usize::try_from(field.zero_based()).map_err(|_| {
                    RuntimePlanBuildError::RecordFieldIdentity(
                        RuntimeRecordFieldIdError::OrdinalOverflow,
                    )
                })?;
                Ok(RuntimeFieldProjection::OpaqueRecord {
                    owner: opaque_owner,
                    field: RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)?,
                })
            }
            RuntimeFieldProjectionSeed::Agent(field) => {
                if !self.agent_field_owner_matches(target_ty, field.owner())? {
                    return invalid_projection("Agent field owner", target_ty);
                }
                if !self.agent_field_result_matches(result_ty, field.result())? {
                    return invalid_projection("Agent field result", result_ty);
                }
                if field.owner() == RuntimeAgentFieldOwner::Reference {
                    return match crate::value::RuntimeEntityReferenceField::try_from(*field) {
                        Ok(reference) => Ok(RuntimeFieldProjection::EntityReference(reference)),
                        Err(_) => {
                            invalid_projection("entity-reference field coordinate", target_ty)
                        }
                    };
                }
                Ok(RuntimeFieldProjection::Agent(*field))
            }
            RuntimeFieldProjectionSeed::EntityReference(field) => {
                self.require_projection(
                    "entity-reference field target",
                    target_ty,
                    |projection| matches!(projection, RuntimePlanTypeProjection::EntityReference),
                )?;
                if !self.is_string(result_ty)? {
                    return invalid_projection("entity-reference field result", result_ty);
                }
                Ok(RuntimeFieldProjection::EntityReference(*field))
            }
            RuntimeFieldProjectionSeed::Progress(field) => {
                self.require_projection("Progress field target", target_ty, |projection| {
                    matches!(projection, RuntimePlanTypeProjection::Progress)
                })?;
                let result_matches = match (field, self.projection(result_ty)?) {
                    (crate::value::RuntimeProgressField::Ratio, RuntimePlanTypeProjection::F32) => {
                        true
                    }
                    (
                        crate::value::RuntimeProgressField::Label,
                        RuntimePlanTypeProjection::Option { item, .. },
                    ) => self.is_string(*item)?,
                    _ => false,
                };
                if !result_matches {
                    return invalid_projection("Progress field result", result_ty);
                }
                Ok(RuntimeFieldProjection::Progress(*field))
            }
        }
    }

    fn validate_variant_payload(
        &self,
        owner: RuntimePlanTypeId,
        ordinal: u32,
        actual: Option<RuntimePlanTypeId>,
    ) -> Result<(), RuntimePlanBuildError> {
        let expected = self.variant_case(owner, ordinal)?.payload();
        if expected == actual {
            Ok(())
        } else {
            Err(RuntimePlanBuildError::VariantPayloadMismatch {
                owner,
                ordinal,
                expected,
                actual,
            })
        }
    }

    fn lower_assignment_place(
        &self,
        assignment: super::seed::RuntimeAssignmentSeed,
        context: &'static str,
    ) -> Result<(crate::value::RuntimeAssignment, RuntimePlanTypeId), RuntimePlanBuildError> {
        let (place, place_type) = self.lower_mutable_place(assignment.place, context)?;
        let displacement = match assignment.displacement {
            crate::value::RuntimePlaceDisplacement::Unreachable => {
                crate::value::RuntimePlaceDisplacement::Unreachable
            }
            crate::value::RuntimePlaceDisplacement::Reachable {
                initialization,
                fields,
            } => {
                let mut fields = fields
                    .into_vec()
                    .into_iter()
                    .map(|path| {
                        if path.fields.is_empty() {
                            return invalid_projection(context, place_type);
                        }
                        let mut current = place_type;
                        let fields = path
                            .fields
                            .into_vec()
                            .into_iter()
                            .map(|field| {
                                let (field, ty) = self.resolve_record_field(current, field)?;
                                current = ty;
                                Ok(field)
                            })
                            .collect::<Result<Box<[_]>, RuntimePlanBuildError>>()?;
                        Ok(crate::value::RuntimeDisplacedField {
                            fields,
                            initialization: path.initialization,
                        })
                    })
                    .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
                fields.sort_by(|left, right| left.fields.cmp(&right.fields));
                if fields
                    .windows(2)
                    .any(|pair| pair[0].fields == pair[1].fields)
                {
                    return invalid_projection(context, place_type);
                }
                crate::value::RuntimePlaceDisplacement::Reachable {
                    initialization,
                    fields: fields.into_boxed_slice(),
                }
            }
        };
        Ok((
            crate::value::RuntimeAssignment::new(place, displacement),
            place_type,
        ))
    }

    fn lower_mutable_place(
        &self,
        place: RuntimeMutablePlaceSeed,
        context: &'static str,
    ) -> Result<(RuntimeMutablePlace, RuntimePlanTypeId), RuntimePlanBuildError> {
        let (place, place_type) = match place {
            RuntimeMutablePlaceSeed::Local(local) => {
                let (local, sequence_type) = self.resolve_local(&local)?;
                (RuntimeMutablePlace::Local(local), sequence_type)
            }
            RuntimeMutablePlaceSeed::Fields { base, fields } => {
                let (base, mut ty) = self.resolve_local(&base)?;
                if fields.is_empty() {
                    return invalid_projection(context, ty);
                }
                let mut path = Vec::with_capacity(fields.len());
                for field in fields {
                    if self.nominal_record_domains.get(ty).is_none() {
                        return invalid_projection(context, ty);
                    }
                    let (field, child) = self.resolve_record_field(ty, field)?;
                    path.push(field);
                    ty = child;
                }
                (
                    RuntimeMutablePlace::Fields {
                        base,
                        fields: path.into_boxed_slice(),
                    },
                    ty,
                )
            }
        };
        Ok((place, place_type))
    }

    fn lower_sequence_place(
        &self,
        place: RuntimeMutablePlaceSeed,
        context: &'static str,
        operation: RuntimeSequenceMutation,
    ) -> Result<(RuntimeMutablePlace, RuntimePlanTypeId), RuntimePlanBuildError> {
        let (place, sequence_type) = self.lower_mutable_place(place, context)?;
        let item = match self.projection(sequence_type)? {
            RuntimePlanTypeProjection::Sequence { kind, item }
                if kind.admits_mutation(operation) =>
            {
                *item
            }
            _ => return invalid_projection(context, sequence_type),
        };
        Ok((place, item))
    }

    fn lower_function_application(
        &self,
        callee: RuntimeExprSeed,
        args: Box<[RuntimeCallArgumentSeed]>,
        result_type: RuntimePlanTypeId,
    ) -> Result<(RuntimeExpr, Vec<RuntimeCallArgument>), RuntimePlanBuildError> {
        let callee = self.lower_expression(callee)?;
        let (parameters, result) = match self.projection(callee.ty())? {
            RuntimePlanTypeProjection::Function {
                parameters, result, ..
            } => (parameters.clone(), *result),
            _ => return invalid_projection("function application callee", callee.ty()),
        };
        let args = self.lower_call_arguments(args)?;
        let actual = self.expanded_argument_types(&args)?;
        if !parameters.starts_with(&actual) {
            return invalid_projection("function application arguments", callee.ty());
        }
        if parameters.len() == actual.len() {
            require_same("function application result", result, result_type)?;
        } else {
            match self.projection(result_type)? {
                RuntimePlanTypeProjection::Function {
                    parameters: remaining,
                    result: remaining_result,
                    ..
                } if remaining.as_ref() == &parameters[actual.len()..]
                    && *remaining_result == result => {}
                _ => return invalid_projection("partial function application result", result_type),
            }
            self.callable_states.borrow_mut().request_partial(
                callee.ty(),
                result_type,
                actual.len(),
            )?;
        }
        Ok((callee, args))
    }

    /// The closed collection intrinsic ABI uses this exact admitted type graph,
    /// including already checked spread/position expansion. It cannot publish
    /// a result inferred from an unrelated same-width or receiver type.
    fn validate_collection_intrinsic_signature(
        &self,
        intrinsic: crate::value::RuntimeIntrinsic,
        args: &[RuntimeCallArgument],
        result: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        use crate::value::RuntimeIntrinsic;
        if !matches!(
            intrinsic,
            RuntimeIntrinsic::CoreSeqLen | RuntimeIntrinsic::CoreSeqSum
        ) {
            return Ok(());
        }
        let inputs = self.expanded_argument_types(args)?;
        let [receiver] = inputs.as_slice() else {
            return Err(RuntimePlanBuildError::CallableAbiArity {
                context: "collection intrinsic",
                expected: 1,
                actual: inputs.len(),
            });
        };
        let (item, _) = self.sequence_projection(*receiver, "collection intrinsic receiver")?;
        match intrinsic {
            RuntimeIntrinsic::CoreSeqLen => {
                if !matches!(
                    self.projection(result)?,
                    RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::USize)
                ) {
                    return invalid_projection("collection length result", result);
                }
            }
            RuntimeIntrinsic::CoreSeqSum => {
                if !matches!(
                    self.projection(item)?,
                    RuntimePlanTypeProjection::Signed(_) | RuntimePlanTypeProjection::Unsigned(_)
                ) {
                    return invalid_projection("collection sum element", item);
                }
                if !matches!(
                    self.projection(result)?,
                    RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64)
                ) {
                    return invalid_projection("collection sum result", result);
                }
            }
            _ => unreachable!("the closed collection intrinsic family was selected above"),
        }
        Ok(())
    }

    fn lower_call_arguments(
        &self,
        args: Box<[RuntimeCallArgumentSeed]>,
    ) -> Result<Vec<RuntimeCallArgument>, RuntimePlanBuildError> {
        let lowered = args
            .into_vec()
            .into_iter()
            .enumerate()
            .map(|(_index, argument)| {
                let (value, mode, abi_position) = argument.into_parts();
                let value = self.lower_expression(value)?;
                if mode == RuntimeCallArgumentMode::Spread {
                    self.require_spreadable(value.ty())?;
                }
                Ok(RuntimeCallArgument::from_admitted_parts(
                    value,
                    mode,
                    abi_position,
                ))
            })
            .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
        self.validate_call_argument_positions(&lowered)?;
        Ok(lowered)
    }

    fn validate_call_argument_positions(
        &self,
        args: &[RuntimeCallArgument],
    ) -> Result<(), RuntimePlanBuildError> {
        let mut saw_spread = false;
        let mut ranges = args
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                if saw_spread {
                    return Err(RuntimePlanBuildError::InvalidCallArgumentPosition {
                        index,
                        position: argument.abi_position(),
                    });
                }
                if argument.mode() == RuntimeCallArgumentMode::Spread {
                    saw_spread = true;
                }
                let width = match argument.mode() {
                    RuntimeCallArgumentMode::Value => 1_u32,
                    RuntimeCallArgumentMode::Spread => match self
                        .projection(argument.value().ty())?
                    {
                        RuntimePlanTypeProjection::Tuple(items) => u32::try_from(items.len())
                            .map_err(|_| RuntimePlanBuildError::InvalidCallArgumentPosition {
                                index,
                                position: argument.abi_position(),
                            })?,
                        RuntimePlanTypeProjection::Array { length, .. } => u32::try_from(
                            length
                                .constant()
                                .expect("admitted executable array length is closed"),
                        )
                        .map_err(|_| RuntimePlanBuildError::InvalidCallArgumentPosition {
                            index,
                            position: argument.abi_position(),
                        })?,
                        _ => {
                            return Err(RuntimePlanBuildError::IndeterminateSpreadArgument {
                                ty: argument.value().ty(),
                            });
                        }
                    },
                };
                let end = argument.abi_position().checked_add(width).ok_or(
                    RuntimePlanBuildError::InvalidCallArgumentPosition {
                        index,
                        position: argument.abi_position(),
                    },
                )?;
                Ok((index, argument.abi_position(), end))
            })
            .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?;
        ranges.sort_by_key(|(_, start, _)| *start);
        let mut expected = 0_u32;
        for (index, start, end) in ranges {
            if start != expected {
                return Err(if start < expected {
                    RuntimePlanBuildError::InvalidCallArgumentPosition {
                        index,
                        position: start,
                    }
                } else {
                    RuntimePlanBuildError::NonContiguousCallArgumentPosition { position: expected }
                });
            }
            expected = end;
        }
        Ok(())
    }

    fn require_spreadable(&self, ty: RuntimePlanTypeId) -> Result<(), RuntimePlanBuildError> {
        if matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Tuple(_) | RuntimePlanTypeProjection::Array { .. }
        ) {
            Ok(())
        } else {
            Err(RuntimePlanBuildError::IndeterminateSpreadArgument { ty })
        }
    }

    fn expanded_argument_types(
        &self,
        args: &[RuntimeCallArgument],
    ) -> Result<Vec<RuntimePlanTypeId>, RuntimePlanBuildError> {
        let mut groups = Vec::new();
        for argument in args {
            let mut types = Vec::new();
            match argument.mode() {
                RuntimeCallArgumentMode::Value => types.push(argument.value().ty()),
                RuntimeCallArgumentMode::Spread => match self.projection(argument.value().ty())? {
                    RuntimePlanTypeProjection::Tuple(items) => {
                        types.extend(items.iter().copied());
                    }
                    RuntimePlanTypeProjection::Array { item, length } => {
                        let count = usize::try_from(
                            length
                                .constant()
                                .expect("admitted executable array length is closed"),
                        )
                        .map_err(|_| {
                            RuntimePlanBuildError::IndeterminateSpreadArgument {
                                ty: argument.value().ty(),
                            }
                        })?;
                        types.try_reserve(count).map_err(|_| {
                            RuntimePlanBuildError::IndeterminateSpreadArgument {
                                ty: argument.value().ty(),
                            }
                        })?;
                        types.extend(std::iter::repeat_n(*item, count));
                    }
                    _ => {
                        return Err(RuntimePlanBuildError::IndeterminateSpreadArgument {
                            ty: argument.value().ty(),
                        });
                    }
                },
            }
            groups.push((argument.abi_position(), types));
        }
        groups.sort_by_key(|(position, _)| *position);
        let mut result = Vec::new();
        for (_, types) in groups {
            result.extend(types);
        }
        Ok(result)
    }

    fn validate_unary(
        &self,
        op: RuntimeUnaryOp,
        operand: RuntimePlanTypeId,
        result: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        match op {
            RuntimeUnaryOp::Not => {
                self.require_bool("logical-not operand", operand)?;
                self.require_bool("logical-not result", result)
            }
            RuntimeUnaryOp::Neg => {
                self.require_negatable("numeric-negation operand", operand)?;
                require_same("numeric-negation result", operand, result)
            }
        }
    }

    fn validate_binary(
        &self,
        op: RuntimeBinaryOp,
        lhs: RuntimePlanTypeId,
        rhs: RuntimePlanTypeId,
        result: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        match op {
            RuntimeBinaryOp::Eq | RuntimeBinaryOp::Ne => {
                require_same("equality operands", lhs, rhs)?;
                self.require_bool("equality result", result)
            }
            RuntimeBinaryOp::Lt
            | RuntimeBinaryOp::Le
            | RuntimeBinaryOp::Gt
            | RuntimeBinaryOp::Ge => {
                require_same("comparison operands", lhs, rhs)?;
                self.require_comparable("comparison operand", lhs)?;
                self.require_bool("comparison result", result)
            }
            RuntimeBinaryOp::Add
            | RuntimeBinaryOp::Sub
            | RuntimeBinaryOp::Mul
            | RuntimeBinaryOp::Div => {
                require_same("arithmetic operands", lhs, rhs)?;
                self.require_numeric("arithmetic operand", lhs)?;
                require_same("arithmetic result", lhs, result)
            }
            RuntimeBinaryOp::And | RuntimeBinaryOp::Or => {
                self.require_bool("logical lhs", lhs)?;
                self.require_bool("logical rhs", rhs)?;
                self.require_bool("logical result", result)
            }
        }
    }

    pub(super) fn require_bool(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        if self.is_bool(ty)? {
            Ok(())
        } else {
            invalid_projection(context, ty)
        }
    }

    pub(super) fn validate_callable_input_abi(
        &self,
        context: &'static str,
        inputs: &[(RuntimeLocalDeclarationId, RuntimePlanTypeId)],
        abi: &[RuntimePureInputType],
    ) -> Result<(), RuntimePlanBuildError> {
        if inputs.len() != abi.len() {
            return Err(RuntimePlanBuildError::CallableAbiArity {
                context,
                expected: inputs.len(),
                actual: abi.len(),
            });
        }
        for (index, ((_, ty), abi)) in inputs.iter().zip(abi).enumerate() {
            if !self.scalar_input_abi_matches(*ty, *abi)? {
                return Err(RuntimePlanBuildError::CallableInputAbi {
                    context,
                    index,
                    ty: *ty,
                });
            }
        }
        Ok(())
    }

    pub(super) fn validate_callable_output_abi(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
        abi: RuntimePureOutputType,
    ) -> Result<(), RuntimePlanBuildError> {
        if self.scalar_output_abi_matches(ty, abi)? {
            Ok(())
        } else {
            Err(RuntimePlanBuildError::CallableOutputAbi { context, ty })
        }
    }

    fn scalar_input_abi_matches(
        &self,
        ty: RuntimePlanTypeId,
        abi: RuntimePureInputType,
    ) -> Result<bool, RuntimePlanBuildError> {
        let projection = self.projection(ty)?;
        Ok(match abi {
            RuntimePureInputType::I8 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I8)
            ),
            RuntimePureInputType::I16 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I16)
            ),
            RuntimePureInputType::I32 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I32)
            ),
            RuntimePureInputType::I64 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64)
            ),
            RuntimePureInputType::I128 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I128)
            ),
            RuntimePureInputType::ISize => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::ISize)
            ),
            RuntimePureInputType::U8 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U8)
            ),
            RuntimePureInputType::U16 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U16)
            ),
            RuntimePureInputType::U32 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32)
            ),
            RuntimePureInputType::U64 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U64)
            ),
            RuntimePureInputType::U128 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U128)
            ),
            RuntimePureInputType::USize => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::USize)
            ),
            RuntimePureInputType::F32 => matches!(projection, RuntimePlanTypeProjection::F32),
            RuntimePureInputType::F64 => matches!(projection, RuntimePlanTypeProjection::F64),
            RuntimePureInputType::Value => true,
        })
    }

    fn scalar_output_abi_matches(
        &self,
        ty: RuntimePlanTypeId,
        abi: RuntimePureOutputType,
    ) -> Result<bool, RuntimePlanBuildError> {
        if abi == RuntimePureOutputType::Value {
            return Ok(true);
        }
        let projection = self.projection(ty)?;
        Ok(match abi {
            RuntimePureOutputType::Bool => matches!(projection, RuntimePlanTypeProjection::Bool),
            RuntimePureOutputType::I8 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I8)
            ),
            RuntimePureOutputType::I16 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I16)
            ),
            RuntimePureOutputType::I32 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I32)
            ),
            RuntimePureOutputType::I64 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64)
            ),
            RuntimePureOutputType::I128 => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I128)
            ),
            RuntimePureOutputType::ISize => matches!(
                projection,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::ISize)
            ),
            RuntimePureOutputType::U8 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U8)
            ),
            RuntimePureOutputType::U16 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U16)
            ),
            RuntimePureOutputType::U32 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32)
            ),
            RuntimePureOutputType::U64 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U64)
            ),
            RuntimePureOutputType::U128 => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U128)
            ),
            RuntimePureOutputType::USize => matches!(
                projection,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::USize)
            ),
            RuntimePureOutputType::F32 => matches!(projection, RuntimePlanTypeProjection::F32),
            RuntimePureOutputType::F64 => matches!(projection, RuntimePlanTypeProjection::F64),
            RuntimePureOutputType::Value => true,
        })
    }

    fn require_numeric(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        if matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Signed(_)
                | RuntimePlanTypeProjection::Unsigned(_)
                | RuntimePlanTypeProjection::F32
                | RuntimePlanTypeProjection::F64
        ) {
            Ok(())
        } else {
            invalid_projection(context, ty)
        }
    }

    fn require_comparable(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        if matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Signed(_)
                | RuntimePlanTypeProjection::Unsigned(_)
                | RuntimePlanTypeProjection::F32
                | RuntimePlanTypeProjection::F64
        ) {
            Ok(())
        } else {
            invalid_projection(context, ty)
        }
    }

    fn require_negatable(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        if matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Signed(_)
                | RuntimePlanTypeProjection::F32
                | RuntimePlanTypeProjection::F64
        ) {
            Ok(())
        } else {
            invalid_projection(context, ty)
        }
    }

    fn is_bool(&self, ty: RuntimePlanTypeId) -> Result<bool, RuntimePlanBuildError> {
        Ok(matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Bool
        ))
    }

    pub(super) fn is_string(&self, ty: RuntimePlanTypeId) -> Result<bool, RuntimePlanBuildError> {
        Ok(matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::String
        ))
    }

    fn is_format_display_type(&self, ty: RuntimePlanTypeId) -> Result<bool, RuntimePlanBuildError> {
        if self.is_exact_dialogue_content_type(ty) {
            return Ok(true);
        }
        if let RuntimePlanTypeProjection::Option { item, .. } = self.projection(ty)? {
            return self.is_format_display_scalar_type(*item);
        }
        self.is_format_display_scalar_type(ty)
    }

    fn is_format_display_scalar_type(
        &self,
        ty: RuntimePlanTypeId,
    ) -> Result<bool, RuntimePlanBuildError> {
        Ok(matches!(
            self.projection(ty)?,
            RuntimePlanTypeProjection::Unit
                | RuntimePlanTypeProjection::Bool
                | RuntimePlanTypeProjection::Signed(_)
                | RuntimePlanTypeProjection::Unsigned(_)
                | RuntimePlanTypeProjection::F32
                | RuntimePlanTypeProjection::F64
                | RuntimePlanTypeProjection::String
                | RuntimePlanTypeProjection::Char
                | RuntimePlanTypeProjection::Duration
                | RuntimePlanTypeProjection::EntityReference
                | RuntimePlanTypeProjection::Progress
        ))
    }

    fn agent_field_owner_matches(
        &self,
        ty: RuntimePlanTypeId,
        owner: RuntimeAgentFieldOwner,
    ) -> Result<bool, RuntimePlanBuildError> {
        Ok(match (owner, self.projection(ty)?) {
            (RuntimeAgentFieldOwner::Agent(owner), RuntimePlanTypeProjection::Agent(target)) => {
                target.operational_type() == owner
            }
            (RuntimeAgentFieldOwner::Reference, RuntimePlanTypeProjection::EntityReference) => true,
            _ => false,
        })
    }

    fn agent_field_result_matches(
        &self,
        ty: RuntimePlanTypeId,
        result: RuntimeAgentFieldResult,
    ) -> Result<bool, RuntimePlanBuildError> {
        match result {
            RuntimeAgentFieldResult::Required(value) => self.agent_field_value_matches(ty, value),
            RuntimeAgentFieldResult::Optional(value) => {
                let RuntimePlanTypeProjection::Option { item, .. } = self.projection(ty)? else {
                    return Ok(false);
                };
                self.agent_field_value_matches(*item, value)
            }
        }
    }

    fn agent_field_value_matches(
        &self,
        ty: RuntimePlanTypeId,
        value: RuntimeAgentFieldValue,
    ) -> Result<bool, RuntimePlanBuildError> {
        Ok(match (value, self.projection(ty)?) {
            (RuntimeAgentFieldValue::Bool, RuntimePlanTypeProjection::Bool)
            | (RuntimeAgentFieldValue::String, RuntimePlanTypeProjection::String)
            | (
                RuntimeAgentFieldValue::U32,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U32),
            )
            | (
                RuntimeAgentFieldValue::U64,
                RuntimePlanTypeProjection::Unsigned(RuntimeUnsignedIntWidth::U64),
            ) => true,
            (RuntimeAgentFieldValue::Agent(expected), RuntimePlanTypeProjection::Agent(actual)) => {
                actual.operational_type() == expected
            }
            (
                RuntimeAgentFieldValue::BuiltinVariant(expected),
                RuntimePlanTypeProjection::BuiltinVariant { owner, .. },
            ) => *owner == expected,
            (
                RuntimeAgentFieldValue::VecAgent(expected),
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item,
                },
            ) => matches!(
                self.projection(*item)?,
                RuntimePlanTypeProjection::Agent(actual) if actual.operational_type() == expected
            ),
            (
                RuntimeAgentFieldValue::AgentValueMap,
                RuntimePlanTypeProjection::Map { key, value, .. },
            ) => matches!(
                (self.projection(*key)?, self.projection(*value)?),
                (
                    RuntimePlanTypeProjection::AgentValue,
                    RuntimePlanTypeProjection::AgentValue,
                )
            ),
            _ => false,
        })
    }
}

impl RuntimePlanBodyConstruction<'_> {
    fn lower_agent_expression(
        &self,
        result_ty: RuntimePlanTypeId,
        seed: RuntimeAgentExprSeed,
    ) -> Result<RuntimeAgentExpr, RuntimePlanBuildError> {
        let (constructor, choice, operand_seeds) = match seed {
            RuntimeAgentExprSeed::ChoiceAction { choice } => (
                RuntimeAgentConstructor::ChoiceAction,
                Some(choice),
                Vec::new(),
            ),
            RuntimeAgentExprSeed::CaptureViewport => {
                (RuntimeAgentConstructor::CaptureViewport, None, Vec::new())
            }
            RuntimeAgentExprSeed::CaptureLayer { target } => {
                (RuntimeAgentConstructor::CaptureLayer, None, vec![*target])
            }
            RuntimeAgentExprSeed::CaptureObject { target } => {
                (RuntimeAgentConstructor::CaptureObject, None, vec![*target])
            }
            RuntimeAgentExprSeed::StatePath { path } => {
                (RuntimeAgentConstructor::StatePath, None, vec![*path])
            }
            RuntimeAgentExprSeed::ObservationPath { path } => {
                (RuntimeAgentConstructor::ObservationPath, None, vec![*path])
            }
            RuntimeAgentExprSeed::ProbeSignal { target } => {
                (RuntimeAgentConstructor::ProbeSignal, None, vec![*target])
            }
            RuntimeAgentExprSeed::ProbeMetric { target } => {
                (RuntimeAgentConstructor::ProbeMetric, None, vec![*target])
            }
            RuntimeAgentExprSeed::ProbeState { path } => {
                (RuntimeAgentConstructor::ProbeState, None, vec![*path])
            }
            RuntimeAgentExprSeed::ProbeObservation { path } => {
                (RuntimeAgentConstructor::ProbeObservation, None, vec![*path])
            }
            RuntimeAgentExprSeed::Diagnostics => {
                (RuntimeAgentConstructor::Diagnostics, None, Vec::new())
            }
            RuntimeAgentExprSeed::PredicateExists { probe } => {
                (RuntimeAgentConstructor::PredicateExists, None, vec![*probe])
            }
            RuntimeAgentExprSeed::PredicateActionEnabled { target } => (
                RuntimeAgentConstructor::PredicateActionEnabled,
                None,
                vec![*target],
            ),
            RuntimeAgentExprSeed::PredicateDiagnosticsHasError { diagnostics } => (
                RuntimeAgentConstructor::PredicateDiagnosticsHasError,
                None,
                vec![*diagnostics],
            ),
            RuntimeAgentExprSeed::PredicateAll { predicates } => (
                RuntimeAgentConstructor::PredicateAll,
                None,
                predicates.into_vec(),
            ),
            RuntimeAgentExprSeed::PredicateAny { predicates } => (
                RuntimeAgentConstructor::PredicateAny,
                None,
                predicates.into_vec(),
            ),
            RuntimeAgentExprSeed::PredicateNot { predicate } => (
                RuntimeAgentConstructor::PredicateNot,
                None,
                vec![*predicate],
            ),
            RuntimeAgentExprSeed::PredicateCompare { probe, op, value } => {
                let constructor = match op {
                    crate::value::RuntimeAgentCompareOp::Eq => RuntimeAgentConstructor::PredicateEq,
                    crate::value::RuntimeAgentCompareOp::NotEq => {
                        RuntimeAgentConstructor::PredicateNotEq
                    }
                    crate::value::RuntimeAgentCompareOp::Greater => {
                        RuntimeAgentConstructor::PredicateGreater
                    }
                    crate::value::RuntimeAgentCompareOp::GreaterOrEqual => {
                        RuntimeAgentConstructor::PredicateGreaterOrEqual
                    }
                    crate::value::RuntimeAgentCompareOp::Less => {
                        RuntimeAgentConstructor::PredicateLess
                    }
                    crate::value::RuntimeAgentCompareOp::LessOrEqual => {
                        RuntimeAgentConstructor::PredicateLessOrEqual
                    }
                };
                (constructor, None, vec![*probe, *value])
            }
            RuntimeAgentExprSeed::ViewportPoint { x, y } => {
                (RuntimeAgentConstructor::ViewportPoint, None, vec![*x, *y])
            }
        };
        let operands = operand_seeds
            .into_iter()
            .map(|operand| self.lower_expression(operand))
            .collect::<Result<Vec<_>, _>>()?;
        self.admit_agent_expression(result_ty, constructor, choice, operands)
    }

    fn admit_agent_expression(
        &self,
        result_ty: RuntimePlanTypeId,
        constructor: RuntimeAgentConstructor,
        choice: Option<crate::entry::RuntimeCommandTargetId>,
        operands: Vec<RuntimeExpr>,
    ) -> Result<RuntimeAgentExpr, RuntimePlanBuildError> {
        let operand_types = choice
            .as_ref()
            .map(RuntimeAgentTypeOperand::Choice)
            .into_iter()
            .chain(
                operands
                    .iter()
                    .map(|operand| RuntimeAgentTypeOperand::Typed(operand.ty())),
            )
            .collect::<Vec<_>>();
        constructor
            .validate_types(self, result_ty, &operand_types)
            .map_err(|error| match error {
                RuntimeAgentSignatureError::OperandType { operand } => match operand_types[operand]
                {
                    RuntimeAgentTypeOperand::Typed(actual) => {
                        RuntimePlanBuildError::InvalidAgentOperandType {
                            constructor,
                            operand,
                            actual,
                        }
                    }
                    RuntimeAgentTypeOperand::Choice(_) => {
                        RuntimePlanBuildError::InvalidAgentExpression { constructor }
                    }
                },
                RuntimeAgentSignatureError::OperandCount { .. } => {
                    RuntimePlanBuildError::InvalidAgentExpression { constructor }
                }
                RuntimeAgentSignatureError::ResultType => {
                    RuntimePlanBuildError::InvalidAgentResultType {
                        constructor,
                        actual: result_ty,
                    }
                }
            })?;
        RuntimeAgentExpr::try_from_admitted_constructor(constructor, choice, operands)
            .map_err(|_| RuntimePlanBuildError::InvalidAgentExpression { constructor })
    }
}

impl RuntimePlanBodyConstruction<'_> {
    pub(super) fn validate_callable_body_locals(
        &self,
        body: &RuntimeExpr,
        params: &[RuntimeLocalDeclarationId],
        captures: &[RuntimeLocalDeclarationId],
    ) -> Result<(), RuntimePlanBuildError> {
        let scope = params
            .iter()
            .chain(captures)
            .copied()
            .collect::<BTreeSet<_>>();
        let mut used = BTreeSet::new();
        self.validate_expression_locals(body, &scope, &mut used)?;
        for capture in captures {
            if !used.contains(capture) {
                return Err(RuntimePlanBuildError::UnusedFunctionCapture { local: *capture });
            }
        }
        Ok(())
    }

    pub(super) fn validate_function_body_locals(
        &self,
        body: &RuntimeExpr,
        inputs: &[RuntimeFunctionInputBinding],
    ) -> Result<(), RuntimePlanBuildError> {
        let scope = function_input_scope(inputs);
        let mut used = BTreeSet::new();
        self.validate_expression_locals(body, &scope, &mut used)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "lexical validation exhaustively traverses the closed expression algebra"
    )]
    fn validate_expression_locals(
        &self,
        expr: &RuntimeExpr,
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        match expr.kind() {
            RuntimeExprKind::Value(_) | RuntimeExprKind::EntityRef(_) => Ok(()),
            RuntimeExprKind::Agent(agent) => {
                for operand in agent.operands() {
                    self.validate_expression_locals(operand, scope, used)?;
                }
                Ok(())
            }
            RuntimeExprKind::Local(read) => {
                require_local_in_scope(read.local(), scope)?;
                used.insert(read.local());
                Ok(())
            }
            RuntimeExprKind::SequencePopFront { place } => {
                let base = match place {
                    RuntimeMutablePlace::Local(local) => *local,
                    RuntimeMutablePlace::Fields { base, .. } => *base,
                };
                require_local_in_scope(base, scope)?;
                used.insert(base);
                Ok(())
            }
            RuntimeExprKind::SequencePush { place, value } => {
                let base = match place {
                    RuntimeMutablePlace::Local(local) => *local,
                    RuntimeMutablePlace::Fields { base, .. } => *base,
                };
                require_local_in_scope(base, scope)?;
                used.insert(base);
                self.validate_expression_locals(value, scope, used)
            }
            RuntimeExprKind::SequencePopBack { place } => {
                let base = match place {
                    RuntimeMutablePlace::Local(local) => *local,
                    RuntimeMutablePlace::Fields { base, .. } => *base,
                };
                require_local_in_scope(base, scope)?;
                used.insert(base);
                Ok(())
            }
            RuntimeExprKind::Let {
                binding,
                expr,
                body,
            } => {
                self.validate_expression_locals(expr, scope, used)?;
                let nested = extend_scope(scope, [*binding])?;
                self.validate_expression_locals(body, &nested, used)
            }
            RuntimeExprKind::Scope { body, .. } => {
                self.validate_expression_locals(body, scope, used)
            }
            RuntimeExprKind::DialogueContent {
                values, effects, ..
            } => {
                self.validate_expression_slice_locals(values, scope, used)?;
                for effect in effects {
                    self.validate_expression_slice_locals(&effect.captures, scope, used)?;
                }
                Ok(())
            }
            RuntimeExprKind::FormatContent { operands, .. } => {
                for operand in operands {
                    self.validate_expression_locals(operand.expression(), scope, used)?;
                }
                Ok(())
            }
            RuntimeExprKind::CharacterDialogue { target, fields, .. } => {
                self.validate_expression_locals(target, scope, used)?;
                for field in fields {
                    if let CharacterDialoguePatchOperation::Set(value) = &field.operation {
                        self.validate_expression_locals(value, scope, used)?;
                    }
                }
                Ok(())
            }
            RuntimeExprKind::Tuple(items) | RuntimeExprKind::BracketSeq(items) => {
                self.validate_expression_slice_locals(items, scope, used)
            }
            RuntimeExprKind::RepeatSeq { value, .. }
            | RuntimeExprKind::ArrayFromVec { value }
            | RuntimeExprKind::SpecializeCallable { value, .. }
            | RuntimeExprKind::ProjectTuple { target: value, .. }
            | RuntimeExprKind::ProjectRecord { target: value, .. }
            | RuntimeExprKind::Sum { source: value }
            | RuntimeExprKind::Unary { expr: value, .. }
            | RuntimeExprKind::ReductionUnchanged { state: value } => {
                self.validate_expression_locals(value, scope, used)
            }
            RuntimeExprKind::Field { target, .. } => match target {
                crate::value::RuntimeFieldTarget::Value(value) => {
                    self.validate_expression_locals(value, scope, used)
                }
                crate::value::RuntimeFieldTarget::Inspect { place, .. } => {
                    require_local_in_scope(place.local(), scope)?;
                    used.insert(place.local());
                    Ok(())
                }
            },
            RuntimeExprKind::Range { start, end, .. } => {
                for bound in start.iter().chain(end.iter()) {
                    self.validate_expression_locals(bound, scope, used)?;
                }
                Ok(())
            }
            RuntimeExprKind::NominalRecord(record) => {
                for initializer in record.initializers() {
                    self.validate_expression_locals(initializer.value(), scope, used)?;
                }
                Ok(())
            }
            RuntimeExprKind::Variant { payload, .. } => {
                if let Some(payload) = payload {
                    self.validate_expression_locals(payload, scope, used)?;
                }
                Ok(())
            }
            RuntimeExprKind::Assign { place, expr, body } => {
                require_local_in_scope(place.local(), scope)?;
                used.insert(place.local());
                self.validate_expression_locals(expr, scope, used)?;
                self.validate_expression_locals(body, scope, used)
            }
            RuntimeExprKind::Call { args, .. } | RuntimeExprKind::PureCall { args, .. } => {
                self.validate_argument_locals(args, scope, used)
            }
            RuntimeExprKind::MakeCallable { state, captures } => {
                let states = self.callable_states.borrow();
                let definition = states
                    .states
                    .get(state.index())
                    .and_then(Option::as_ref)
                    .ok_or(RuntimePlanBuildError::UnknownCallableState { state: *state })?;
                if captures.len() != definition.retained.len() {
                    return Err(RuntimePlanBuildError::CallableAbiArity {
                        context: "callable retained expressions",
                        expected: definition.retained.len(),
                        actual: captures.len(),
                    });
                }
                for capture in captures {
                    self.validate_expression_locals(capture, scope, used)?;
                }
                Ok(())
            }
            RuntimeExprKind::ApplyGroup { callee, args } => {
                self.validate_expression_locals(callee, scope, used)?;
                self.validate_argument_locals(args, scope, used)
            }
            RuntimeExprKind::TraitCall { receiver, args, .. } => {
                self.validate_expression_locals(receiver, scope, used)?;
                self.validate_argument_locals(args, scope, used)
            }
            RuntimeExprKind::Binary { lhs, rhs, .. } => {
                self.validate_expression_locals(lhs, scope, used)?;
                self.validate_expression_locals(rhs, scope, used)
            }
            RuntimeExprKind::If {
                condition,
                then_expr,
                else_expr,
            } => {
                self.validate_expression_locals(condition, scope, used)?;
                self.validate_expression_locals(then_expr, scope, used)?;
                self.validate_expression_locals(else_expr, scope, used)
            }
            RuntimeExprKind::IfLet {
                pattern,
                expr,
                guard,
                then_expr,
                else_expr,
            } => {
                self.validate_expression_locals(expr, scope, used)?;
                let nested = extend_scope(
                    scope,
                    pattern
                        .binding_declarations()
                        .map(RuntimePatternBindingDeclaration::local),
                )?;
                if let Some(guard) = guard {
                    self.validate_expression_locals(guard, &nested, used)?;
                }
                self.validate_expression_locals(then_expr, &nested, used)?;
                self.validate_expression_locals(else_expr, scope, used)
            }
            RuntimeExprKind::Match { scrutinee, arms } => {
                self.validate_expression_locals(scrutinee, scope, used)?;
                for arm in arms {
                    let nested = extend_scope(
                        scope,
                        arm.pattern()
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                    if let Some(guard) = arm.guard() {
                        self.validate_expression_locals(guard, &nested, used)?;
                    }
                    self.validate_expression_locals(arm.value(), &nested, used)?;
                }
                Ok(())
            }
        }
    }

    fn validate_expression_slice_locals(
        &self,
        expressions: &[RuntimeExpr],
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        for expression in expressions {
            self.validate_expression_locals(expression, scope, used)?;
        }
        Ok(())
    }

    fn validate_argument_locals(
        &self,
        arguments: &[RuntimeCallArgument],
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        for argument in arguments {
            self.validate_expression_locals(argument.value(), scope, used)?;
        }
        Ok(())
    }

    pub(super) fn lower_stream_plan_seed(
        &self,
        seed: RuntimeStreamPlanSeed,
    ) -> Result<StreamPlan, RuntimePlanBuildError> {
        let item_ty = self.resolve_seed_type("stream item type", seed.item_ty)?;
        let error_ty = self.resolve_seed_type("stream error type", seed.error_ty)?;
        let ops = self.lower_stream_ops(seed.ops, item_ty)?;
        let mut scope = BTreeSet::new();
        self.validate_stream_ops_locals(&ops, &mut scope)?;
        Ok(StreamPlan::from_admitted_parts(
            seed.id, item_ty, error_ty, ops,
        ))
    }

    fn lower_stream_ops(
        &self,
        seeds: Vec<RuntimeStreamOpSeed>,
        item_ty: RuntimePlanTypeId,
    ) -> Result<Vec<StreamOp>, RuntimePlanBuildError> {
        seeds
            .into_iter()
            .map(|seed| self.lower_stream_op(seed, item_ty))
            .collect()
    }

    fn lower_stream_op(
        &self,
        seed: RuntimeStreamOpSeed,
        item_ty: RuntimePlanTypeId,
    ) -> Result<StreamOp, RuntimePlanBuildError> {
        Ok(match seed {
            RuntimeStreamOpSeed::Let { pattern, expr } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(expr)?;
                self.require_expression_assignable("stream let pattern", pattern.ty(), expr.ty())?;
                StreamOp::Let { pattern, expr }
            }
            RuntimeStreamOpSeed::ForNext {
                pattern,
                source,
                body,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let source = self.lower_expression(source)?;
                let (source_item, _) =
                    self.stream_projection(source.ty(), "stream for-next source expression")?;
                require_same("stream for-next pattern", source_item, pattern.ty())?;
                StreamOp::ForNext {
                    pattern,
                    source,
                    body: self.lower_stream_ops(body, item_ty)?,
                }
            }
            RuntimeStreamOpSeed::Yield { expr } => {
                let expr = self.lower_expression(expr)?;
                require_same("stream yield expression", item_ty, expr.ty())?;
                StreamOp::Yield { expr }
            }
            RuntimeStreamOpSeed::If {
                condition,
                then_ops,
                else_ops,
            } => {
                let condition = self.lower_expression(condition)?;
                self.require_bool("stream if condition", condition.ty())?;
                StreamOp::If {
                    condition,
                    then_ops: self.lower_stream_ops(then_ops, item_ty)?,
                    else_ops: self.lower_stream_ops(else_ops, item_ty)?,
                }
            }
            RuntimeStreamOpSeed::Match { scrutinee, arms } => {
                let scrutinee = self.lower_expression(scrutinee)?;
                StreamOp::Match {
                    scrutinee: scrutinee.clone(),
                    arms: arms
                        .into_iter()
                        .map(|arm| self.lower_stream_match_arm(arm, scrutinee.ty(), item_ty))
                        .collect::<Result<_, _>>()?,
                }
            }
            RuntimeStreamOpSeed::Close { source } => {
                let source = self.lower_expression(source)?;
                self.stream_projection(source.ty(), "stream close source expression")?;
                StreamOp::Close { source }
            }
            RuntimeStreamOpSeed::Return => StreamOp::Return,
        })
    }

    fn lower_stream_match_arm(
        &self,
        arm: RuntimeStreamMatchArmSeed,
        scrutinee_ty: RuntimePlanTypeId,
        item_ty: RuntimePlanTypeId,
    ) -> Result<StreamMatchArm, RuntimePlanBuildError> {
        let pattern = self.lower_pattern_seed(arm.pattern)?;
        require_same("stream match pattern", scrutinee_ty, pattern.ty())?;
        let guard = arm
            .guard
            .map(|guard| self.lower_guard_expression(guard, &pattern))
            .transpose()?;
        if let Some(guard) = &guard {
            self.require_bool("stream match guard", guard.ty())?;
        }
        Ok(StreamMatchArm {
            pattern,
            guard,
            ops: self.lower_stream_ops(arm.ops, item_ty)?,
        })
    }

    fn validate_stream_ops_locals(
        &self,
        ops: &[StreamOp],
        scope: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        let mut used = BTreeSet::new();
        self.validate_stream_ops_locals_inner(ops, scope, &mut used)
    }

    fn validate_stream_ops_locals_inner(
        &self,
        ops: &[StreamOp],
        scope: &mut BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        for op in ops {
            match op {
                StreamOp::Let { pattern, expr } => {
                    self.validate_expression_locals(expr, scope, used)?;
                    *scope = extend_scope(
                        scope,
                        pattern
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                }
                StreamOp::ForNext {
                    pattern,
                    source,
                    body,
                } => {
                    self.validate_expression_locals(source, scope, used)?;
                    let mut nested = extend_scope(
                        scope,
                        pattern
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                    self.validate_stream_ops_locals_inner(body, &mut nested, used)?;
                }
                StreamOp::Yield { expr } | StreamOp::Close { source: expr } => {
                    self.validate_expression_locals(expr, scope, used)?;
                }
                StreamOp::If {
                    condition,
                    then_ops,
                    else_ops,
                } => {
                    self.validate_expression_locals(condition, scope, used)?;
                    let mut then_scope = scope.clone();
                    self.validate_stream_ops_locals_inner(then_ops, &mut then_scope, used)?;
                    let mut else_scope = scope.clone();
                    self.validate_stream_ops_locals_inner(else_ops, &mut else_scope, used)?;
                }
                StreamOp::Match { scrutinee, arms } => {
                    self.validate_expression_locals(scrutinee, scope, used)?;
                    for arm in arms {
                        let mut arm_scope = extend_scope(
                            scope,
                            arm.pattern
                                .binding_declarations()
                                .map(RuntimePatternBindingDeclaration::local),
                        )?;
                        if let Some(guard) = &arm.guard {
                            self.validate_expression_locals(guard, &arm_scope, used)?;
                        }
                        self.validate_stream_ops_locals_inner(&arm.ops, &mut arm_scope, used)?;
                    }
                }
                StreamOp::Return => {}
            }
        }
        Ok(())
    }

    fn stream_projection(
        &self,
        ty: RuntimePlanTypeId,
        context: &'static str,
    ) -> Result<(RuntimePlanTypeId, RuntimePlanTypeId), RuntimePlanBuildError> {
        match self.projection(ty)? {
            RuntimePlanTypeProjection::Stream { item, error } => Ok((*item, *error)),
            _ => invalid_projection(context, ty),
        }
    }
}

impl RuntimePlanBodyConstruction<'_> {
    fn fold_map_consumption(
        &self,
        operations: Vec<FlowOp>,
    ) -> Result<Vec<FlowOp>, RuntimePlanBuildError> {
        let mut pending = operations.into_iter();
        let mut output = Vec::new();
        while let Some(mut operation) = pending.next() {
            if let FlowOp::StandardMap(control) = &mut operation
                && matches!(
                    control.completion(),
                    super::super::RuntimeStandardMapCompletion::Collection
                )
            {
                let ty = self
                    .locals
                    .lock()
                    .expect("local request table only runs checked, callback-free transitions")
                    .ty(control.result())
                    .ok_or(RuntimePlanBuildError::UnknownFunctionLocal {
                        local: control.result(),
                    })?;
                let consumption = super::super::sequence_sum::RuntimeSequenceSum::project(
                    control.result(),
                    ty,
                    pending.as_slice().iter(),
                )
                .map(|sum| sum.skipped_ops);
                if let Some(count) = consumption {
                    control.append_sum(pending.by_ref().take(count).collect());
                }
            }
            output.push(operation);
        }
        Ok(output)
    }

    pub(super) fn lower_flow_ops(
        &self,
        seeds: Vec<RuntimeFlowOpSeed>,
    ) -> Result<Vec<FlowOp>, RuntimePlanBuildError> {
        // Try/Await continuations can produce long right-nested Match trees.
        // Keep Match arm traversal on an explicit stack so source-order seed
        // admission does not consume one native frame per continuation.
        let mut frames = vec![FlowOpLowerFrame::List {
            seeds: seeds.into_iter(),
            lowered: Vec::new(),
        }];
        loop {
            let Some(frame) = frames.pop() else {
                return Err(RuntimePlanBuildError::FlowLoweringInvariant {
                    context: "frame stack became empty before the root list completed",
                });
            };
            match frame {
                FlowOpLowerFrame::List { mut seeds, lowered } => match seeds.next() {
                    Some(RuntimeFlowOpSeed::Match { scrutinee, arms }) => {
                        let scrutinee = self.lower_expression(scrutinee)?;
                        frames.push(FlowOpLowerFrame::List { seeds, lowered });
                        frames.push(FlowOpLowerFrame::MatchArms {
                            scrutinee,
                            arms: arms.into_iter(),
                            lowered: Vec::new(),
                        });
                    }
                    Some(seed) => {
                        frames.push(FlowOpLowerFrame::List { seeds, lowered });
                        let operation = self.lower_flow_op(seed)?;
                        let Some(FlowOpLowerFrame::List { lowered, .. }) = frames.last_mut() else {
                            return Err(RuntimePlanBuildError::FlowLoweringInvariant {
                                context: "a leaf operation lost its containing list",
                            });
                        };
                        lowered.push(operation);
                    }
                    None => {
                        let lowered = self.fold_map_consumption(lowered)?;
                        match frames.pop() {
                            Some(FlowOpLowerFrame::FinishMatchGuard {
                                pattern,
                                candidate,
                                result,
                                condition,
                                copy_locals,
                                arm_ops,
                            }) => {
                                frames.push(FlowOpLowerFrame::FinishMatchArm {
                                    pattern,
                                    guard: Some(RuntimeMatchGuard {
                                        candidate,
                                        result,
                                        condition,
                                        copy_locals,
                                        ops: lowered,
                                    }),
                                });
                                frames.push(FlowOpLowerFrame::List {
                                    seeds: arm_ops.into_iter(),
                                    lowered: Vec::new(),
                                });
                            }
                            Some(FlowOpLowerFrame::FinishMatchArm { pattern, guard }) => {
                                let Some(FlowOpLowerFrame::MatchArms {
                                    lowered: arms_lowered,
                                    ..
                                }) = frames.last_mut()
                                else {
                                    return Err(RuntimePlanBuildError::FlowLoweringInvariant {
                                        context: "a match arm body has no owning Match",
                                    });
                                };
                                arms_lowered.push(RuntimeMatchArm {
                                    pattern,
                                    guard,
                                    ops: lowered,
                                });
                            }
                            Some(_) => {
                                return Err(RuntimePlanBuildError::FlowLoweringInvariant {
                                    context: "a completed child list has an unexpected parent frame",
                                });
                            }
                            None => return Ok(lowered),
                        }
                    }
                },
                FlowOpLowerFrame::MatchArms {
                    scrutinee,
                    mut arms,
                    lowered,
                } => match arms.next() {
                    Some(arm) => {
                        let RuntimeFlowMatchArmSeed {
                            pattern,
                            guard,
                            ops,
                        } = arm;
                        let pattern = self.lower_pattern_seed(pattern)?;
                        require_same("flow match pattern", scrutinee.ty(), pattern.ty())?;
                        let scrutinee_type = scrutinee.ty();
                        frames.push(FlowOpLowerFrame::MatchArms {
                            scrutinee,
                            arms,
                            lowered,
                        });
                        if let Some(guard) = guard {
                            let (candidate, candidate_type) =
                                self.resolve_local(&guard.candidate)?;
                            require_same("flow match candidate", scrutinee_type, candidate_type)?;
                            if crate::pattern::runtime_pattern_contains_binding(&pattern, candidate)
                            {
                                return Err(RuntimePlanBuildError::InvalidGuardCopyBinding {
                                    local: candidate,
                                });
                            }
                            let result =
                                self.resolve_seed_type("flow match guard result", guard.result)?;
                            let condition = guard
                                .condition
                                .map(|condition| self.lower_expression(condition))
                                .transpose()?;
                            match &condition {
                                Some(condition) => {
                                    require_same(
                                        "flow match guard result",
                                        result,
                                        condition.ty(),
                                    )?;
                                    self.require_bool("flow match guard", result)?;
                                }
                                None => self.require_projection(
                                    "non-returning guard",
                                    result,
                                    |projection| {
                                        matches!(projection, RuntimePlanTypeProjection::Never)
                                    },
                                )?,
                            }
                            let copy_locals = guard
                                .copy_locals
                                .into_vec()
                                .into_iter()
                                .map(|local| {
                                    let (local, _) = self.resolve_local(&local)?;
                                    if !crate::pattern::runtime_pattern_contains_binding(
                                        &pattern, local,
                                    ) {
                                        return Err(
                                            RuntimePlanBuildError::InvalidGuardCopyBinding {
                                                local,
                                            },
                                        );
                                    }
                                    Ok(local)
                                })
                                .collect::<Result<Vec<_>, RuntimePlanBuildError>>()?
                                .into_boxed_slice();
                            frames.push(FlowOpLowerFrame::FinishMatchGuard {
                                pattern,
                                candidate,
                                result,
                                condition,
                                copy_locals,
                                arm_ops: ops,
                            });
                            frames.push(FlowOpLowerFrame::List {
                                seeds: guard.ops.into_iter(),
                                lowered: Vec::new(),
                            });
                        } else {
                            frames.push(FlowOpLowerFrame::FinishMatchArm {
                                pattern,
                                guard: None,
                            });
                            frames.push(FlowOpLowerFrame::List {
                                seeds: ops.into_iter(),
                                lowered: Vec::new(),
                            });
                        }
                    }
                    None => {
                        let Some(FlowOpLowerFrame::List {
                            lowered: parent_ops,
                            ..
                        }) = frames.last_mut()
                        else {
                            return Err(RuntimePlanBuildError::FlowLoweringInvariant {
                                context: "a Match has no containing operation list",
                            });
                        };
                        parent_ops.push(FlowOp::Match {
                            scrutinee,
                            arms: lowered,
                        });
                    }
                },
                FlowOpLowerFrame::FinishMatchArm { .. }
                | FlowOpLowerFrame::FinishMatchGuard { .. } => {
                    return Err(RuntimePlanBuildError::FlowLoweringInvariant {
                        context: "match arm completion ran before its child list",
                    });
                }
            }
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the match is the exhaustive canonical flow admission authority"
    )]
    fn lower_flow_op(&self, seed: RuntimeFlowOpSeed) -> Result<FlowOp, RuntimePlanBuildError> {
        Ok(match seed {
            RuntimeFlowOpSeed::Let { pattern, expr } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(expr)?;
                self.require_expression_assignable("flow let pattern", pattern.ty(), expr.ty())?;
                FlowOp::Let { pattern, expr }
            }
            RuntimeFlowOpSeed::FormatOperandAttempt {
                attempt,
                parameter,
                body,
                value,
            } => {
                let attempt = attempt
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignFormatAttemptSeed)?;
                let format_attempt = self
                    .format_attempts
                    .get(attempt.index())
                    .filter(|candidate| candidate.id() == attempt)
                    .ok_or(RuntimePlanBuildError::UnknownFormatAttempt { attempt })?;
                let expected = format_attempt
                    .operands()
                    .iter()
                    .find(|operand| operand.parameter() == parameter)
                    .map(super::super::RuntimeFormatAttemptOperand::ty)
                    .ok_or(RuntimePlanBuildError::MissingFormatAttemptOperand {
                        attempt,
                        parameter,
                    })?;
                let body = self.lower_flow_ops(body)?;
                let value = self.lower_expression(value)?;
                require_same("format-attempt operand", expected, value.ty())?;
                FlowOp::FormatOperandAttempt {
                    attempt,
                    parameter,
                    body,
                    value,
                }
            }
            RuntimeFlowOpSeed::LetElse {
                pattern,
                expr,
                else_ops,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(expr)?;
                self.require_expression_assignable(
                    "flow let-else pattern",
                    pattern.ty(),
                    expr.ty(),
                )?;
                FlowOp::LetElse {
                    pattern,
                    expr,
                    else_ops: self.lower_flow_ops(else_ops)?,
                }
            }
            RuntimeFlowOpSeed::Assign { place, value } => {
                let (place, place_ty) =
                    self.lower_assignment_place(place, "flow assignment place")?;
                let value = self.lower_expression(value)?;
                require_same("flow assignment value", place_ty, value.ty())?;
                FlowOp::Assign { place, value }
            }
            RuntimeFlowOpSeed::Dialogue {
                target,
                content,
                result,
            } => {
                let target = self.lower_expression(target)?;
                let producer = crate::value::RuntimeCharacterDialogueProducerId::get();
                self.require_projection("dialogue target", target.ty(), |projection| {
                    matches!(
                        projection,
                        RuntimePlanTypeProjection::Opaque {
                            producer: actual,
                            value_class: crate::value::RuntimeOpaqueValueClass::Plain,
                            persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
                            ..
                        } if actual == &producer
                    )
                })?;
                let content = content
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignDialogueContentSeed)?;
                if self.dialogue_content.get(content).is_none() {
                    return Err(RuntimePlanBuildError::ForeignDialogueContentSeed);
                }
                let (result_type, result_pattern) = result.into_parts();
                let ty = self.resolve_seed_type("dialogue result target", result_type)?;
                let pattern = self.lower_pattern_seed(result_pattern)?;
                require_same("dialogue result target pattern", ty, pattern.ty())?;
                FlowOp::Dialogue {
                    target,
                    content,
                    result: RuntimeDialogueResultTarget::new(ty, pattern),
                }
            }
            RuntimeFlowOpSeed::LineOperation { binding, operation } => FlowOp::LineOperation {
                binding: binding
                    .map(|binding| self.lower_pattern_seed(binding))
                    .transpose()?,
                operation: self.lower_line_operation(operation)?,
            },
            RuntimeFlowOpSeed::CommitDialogueResult { value } => FlowOp::CommitDialogueResult {
                value: self.lower_expression(value)?,
            },
            RuntimeFlowOpSeed::SelectDialogueResult { value } => FlowOp::SelectDialogueResult {
                value: self.lower_expression(value)?,
            },
            RuntimeFlowOpSeed::Choice { id, options } => FlowOp::Choice {
                id,
                options: options
                    .into_iter()
                    .map(|option| self.lower_choice_option(option))
                    .collect::<Result<_, _>>()?,
            },
            RuntimeFlowOpSeed::Await {
                binding,
                target,
                observers,
            } => {
                let source = self.lower_expression(target.source)?;
                let item = self.awaited_item_type(source.ty())?;
                let binding = binding
                    .map(|binding| self.lower_pattern_seed(binding))
                    .transpose()?;
                if let Some(binding) = &binding {
                    require_same("Await result binding", item, binding.ty())?;
                }
                FlowOp::Await {
                    binding,
                    target: crate::plan::RuntimeNeedAwaitTarget::new(source),
                    observers: observers
                        .into_iter()
                        .map(|observer| {
                            Ok(crate::plan::RuntimeAwaitPendingObserver {
                                pattern: self.lower_pattern_seed(observer.pattern)?,
                                ops: self.lower_flow_ops(observer.ops)?,
                            })
                        })
                        .collect::<Result<_, RuntimePlanBuildError>>()?,
                }
            }
            RuntimeFlowOpSeed::StartNeedProducer { binding, target } => {
                let binding = self.lower_pattern_seed(binding)?;
                let item = self.need_item_type(binding.ty())?;
                let plan_item =
                    self.resolve_seed_type("Need producer payload", target.plan.payload_type())?;
                require_same("Need producer result item", item, plan_item)?;
                if target.arguments.len() != target.plan.argument_count() {
                    return Err(
                        crate::plan::RuntimeNeedProducerStartTargetError::ArgumentCountMismatch
                            .into(),
                    );
                }
                let mut arguments = Vec::with_capacity(target.arguments.len());
                for (argument, expected_type) in target
                    .arguments
                    .into_iter()
                    .zip(target.plan.argument_types().iter().copied())
                {
                    let argument = self.lower_host_argument(argument)?;
                    let expected_type =
                        self.resolve_seed_type("Need producer argument", expected_type)?;
                    require_same(
                        "Need producer argument",
                        expected_type,
                        argument.value().ty(),
                    )?;
                    arguments.push(argument);
                }
                let target = crate::plan::RuntimeNeedProducerStartTarget::try_new(
                    target.plan,
                    target.task_plan,
                    arguments,
                )?;
                FlowOp::StartNeedProducer { binding, target }
            }
            RuntimeFlowOpSeed::AwaitMany {
                binding,
                target,
                pending,
            } => {
                if target.limit == 0 {
                    return Err(RuntimePlanBuildError::ZeroAwaitManyLimit);
                }
                for (template, binding, context) in
                    [
                        (
                            &target.base,
                            crate::plan::RuntimeTaskSemanticBinding::<
                                super::RuntimeLineTaskGroupSeedId,
                            >::AwaitManyBase,
                            "await-many base",
                        ),
                        (
                            &target.child,
                            crate::plan::RuntimeTaskSemanticBinding::<
                                super::RuntimeLineTaskGroupSeedId,
                            >::AwaitManyChild,
                            "await-many child",
                        ),
                    ]
                {
                    template
                        .definition
                        .family
                        .validate_runtime_task_binding(&binding)
                        .map_err(
                            |source| RuntimePlanBuildError::InvalidTaskProducerDefinition {
                                context,
                                source,
                            },
                        )?;
                }
                let source = self.lower_expression(target.source)?;
                let item_ty = self.await_many_item_type(source.ty())?;
                let (item_binding, binding_ty) = target
                    .item_binding
                    .resolve(&self.issuer)
                    .ok_or(RuntimePlanBuildError::ForeignLocalSeed)?;
                require_same("AwaitMany item binding", item_ty, binding_ty)?;
                FlowOp::AwaitMany {
                    binding: binding
                        .map(|binding| self.lower_pattern_seed(binding))
                        .transpose()?,
                    target: AwaitManyTarget {
                        source,
                        item_binding,
                        limit: target.limit,
                        base: self.lower_need_producer_template(target.base, &[])?,
                        child: self
                            .lower_need_producer_template(target.child, &[target.item_binding])?,
                    },
                    pending: self.lower_line_effects(pending)?,
                }
            }
            RuntimeFlowOpSeed::HostCall { binding, target } => {
                let binding = binding
                    .map(|binding| self.lower_pattern_seed(binding))
                    .transpose()?;
                let target = self.lower_host_call_target(target)?;
                if let Some(binding) = &binding {
                    require_same("host call binding", target.result, binding.ty())?;
                }
                FlowOp::HostCall { binding, target }
            }
            RuntimeFlowOpSeed::ApplyFunction {
                callee,
                args,
                result,
            } => {
                let result = self.lower_pattern_seed(result)?;
                let (callee, args) = self.lower_function_application(callee, args, result.ty())?;
                FlowOp::ApplyGroup {
                    callee,
                    args,
                    result,
                }
            }
            RuntimeFlowOpSeed::ProjectCall { mut plan, result } => {
                let placement = std::mem::replace(
                    &mut plan.placement,
                    super::super::RuntimeProjectCallSitePlacementSeed::Allocate,
                );
                let site = match placement {
                    super::super::RuntimeProjectCallSitePlacementSeed::Allocate => {
                        self.project_call_sites.borrow_mut().reserve(&self.issuer)?
                    }
                    super::super::RuntimeProjectCallSitePlacementSeed::Reserved(site) => {
                        self.project_call_sites
                            .borrow()
                            .vacant(&self.issuer, &site)?;
                        site
                    }
                };
                let plan = self.lower_project_call_plan(plan)?;
                let result = self.lower_pattern_seed(result)?;
                let expected = self.callable_states.borrow().states[plan.state().index()]
                    .as_ref()
                    .ok_or(RuntimePlanBuildError::UnknownCallableState {
                        state: plan.state(),
                    })?
                    .result;
                require_same("project-call result pattern", expected, result.ty())?;
                let site = self.project_call_sites.borrow_mut().define(
                    &self.issuer,
                    &site,
                    super::super::RuntimeProjectCallSite::from_admitted_parts(plan, result),
                )?;
                FlowOp::ProjectCall { site }
            }
            RuntimeFlowOpSeed::If {
                condition,
                then_ops,
                else_ops,
            } => {
                let condition = self.lower_expression(condition)?;
                self.require_bool("flow if condition", condition.ty())?;
                FlowOp::If {
                    condition,
                    then_ops: self.lower_flow_ops(then_ops)?,
                    else_ops: self.lower_flow_ops(else_ops)?,
                }
            }
            RuntimeFlowOpSeed::IfLet {
                pattern,
                expr,
                guard,
                then_ops,
                else_ops,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(expr)?;
                require_same("flow if-let pattern", pattern.ty(), expr.ty())?;
                let guard = guard
                    .map(|guard| self.lower_guard_expression(guard, &pattern))
                    .transpose()?;
                if let Some(guard) = &guard {
                    self.require_bool("flow if-let guard", guard.ty())?;
                }
                FlowOp::IfLet {
                    pattern,
                    expr,
                    guard,
                    then_ops: self.lower_flow_ops(then_ops)?,
                    else_ops: self.lower_flow_ops(else_ops)?,
                }
            }
            RuntimeFlowOpSeed::Match { .. } => {
                return Err(RuntimePlanBuildError::FlowLoweringInvariant {
                    context: "Match reached the recursive leaf-operation lowerer",
                });
            }
            RuntimeFlowOpSeed::Loop { result, body } => FlowOp::Loop {
                result: result
                    .map(|result| self.lower_pattern_seed(result))
                    .transpose()?,
                body: self.lower_flow_ops(body)?,
            },
            RuntimeFlowOpSeed::While { condition, body } => {
                let condition = self.lower_expression(condition)?;
                self.require_bool("flow while condition", condition.ty())?;
                FlowOp::While {
                    condition,
                    body: self.lower_flow_ops(body)?,
                }
            }
            RuntimeFlowOpSeed::WhileLet {
                pattern,
                expr,
                guard,
                body,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(expr)?;
                require_same("flow while-let pattern", pattern.ty(), expr.ty())?;
                let guard = guard
                    .map(|guard| self.lower_guard_expression(guard, &pattern))
                    .transpose()?;
                if let Some(guard) = &guard {
                    self.require_bool("flow while-let guard", guard.ty())?;
                }
                FlowOp::WhileLet {
                    pattern,
                    expr,
                    guard,
                    body: self.lower_flow_ops(body)?,
                }
            }
            RuntimeFlowOpSeed::For {
                pattern,
                source,
                evidence,
                body,
            } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let source = self.lower_expression(source)?;
                let (item_ty, evidence) = self.lower_iterator_evidence(source.ty(), evidence)?;
                require_same("flow for pattern", item_ty, pattern.ty())?;
                FlowOp::For {
                    pattern,
                    source,
                    evidence,
                    body: self.lower_flow_ops(body)?,
                }
            }
            RuntimeFlowOpSeed::ProgramTask {
                name,
                binding,
                producer,
                delivery,
                captures,
                body,
            } => {
                let binding = self.lower_pattern_seed(binding)?;
                let producer = self.lower_producer_definition(producer)?;
                let payload = self.resolve_seed_type(
                    "Program task result payload",
                    crate::pattern::RuntimeSemanticTypeId::from_bytes(
                        *producer.handle_type.payload_type().as_bytes(),
                    ),
                )?;
                match (producer.handle_type, self.projection(binding.ty())?) {
                    (
                        crate::task::TaskHandleType::Need { .. },
                        RuntimePlanTypeProjection::Need(item),
                    )
                    | (
                        crate::task::TaskHandleType::ThreadHandle { .. },
                        RuntimePlanTypeProjection::ThreadHandle(item),
                    ) => require_same("Program task result payload", payload, *item)?,
                    _ => return invalid_projection("Program task result binding", binding.ty()),
                }
                FlowOp::ProgramTask {
                    name,
                    binding,
                    producer,
                    delivery,
                    captures: {
                        let mut selected = std::collections::BTreeSet::new();
                        captures
                            .into_iter()
                            .map(|capture| {
                                let (local, _) = capture
                                    .local
                                    .resolve(&self.issuer)
                                    .ok_or(RuntimePlanBuildError::ForeignLocalSeed)?;
                                if !selected.insert(local) {
                                    return Err(
                                        RuntimePlanBuildError::DuplicateProgramTaskCapture {
                                            local,
                                        },
                                    );
                                }
                                Ok(crate::plan::RuntimeProgramTaskCapture {
                                    local,
                                    mode: capture.mode,
                                })
                            })
                            .collect::<Result<_, _>>()?
                    },
                    body: self.lower_flow_ops(body)?,
                }
            }
            RuntimeFlowOpSeed::StandardMap(control) => {
                let family = control.family();
                let (source, source_type) = self.resolve_local(control.source())?;
                let (mapping, mapping_type) = self.resolve_local(control.mapping())?;
                let (result, result_type) = self.resolve_local(control.result())?;
                let (input, output) =
                    self.standard_map_item_projection(family, source_type, result_type)?;
                match self.projection(mapping_type)? {
                    RuntimePlanTypeProjection::Function {
                        parameters, result, ..
                    } if parameters.as_ref() == [input] && *result == output => {}
                    _ => return invalid_projection("standard map callback", mapping_type),
                }
                let (body, consumption) = control.into_body_parts();
                let mut control = crate::plan::RuntimeStandardMapControl::new(
                    family,
                    source,
                    mapping,
                    result,
                    self.lower_flow_ops(body)?,
                );
                if let Some(consumption) = consumption {
                    let consumption = self.lower_flow_ops(consumption)?;
                    let valid = super::super::sequence_sum::RuntimeSequenceSum::project(
                        result,
                        result_type,
                        consumption.iter(),
                    )
                    .is_some_and(|sum| sum.skipped_ops == consumption.len());
                    if !valid {
                        return Err(RuntimePlanBuildError::InvalidStandardMapConsumption {
                            result,
                        });
                    }
                    control.append_sum(consumption);
                }
                FlowOp::StandardMap(control)
            }
            RuntimeFlowOpSeed::Scope { identity, body } => FlowOp::Scope {
                identity,
                body: self.lower_flow_ops(body)?,
            },
            RuntimeFlowOpSeed::ExitScopeBind { pattern, expr } => {
                let pattern = self.lower_pattern_seed(pattern)?;
                let expr = self.lower_expression(expr)?;
                self.require_expression_assignable(
                    "scope result binding",
                    pattern.ty(),
                    expr.ty(),
                )?;
                FlowOp::ExitScopeBind { pattern, expr }
            }
            RuntimeFlowOpSeed::Break(value) => FlowOp::Break(
                value
                    .map(|value| self.lower_expression(value))
                    .transpose()?,
            ),
            RuntimeFlowOpSeed::Continue => FlowOp::Continue,
            RuntimeFlowOpSeed::Goto(target) => FlowOp::Goto(target),
            RuntimeFlowOpSeed::GotoExpr(target) => FlowOp::GotoExpr(self.lower_expression(target)?),
            RuntimeFlowOpSeed::Return(value) => FlowOp::Return(value),
            RuntimeFlowOpSeed::ReturnExpr(value) => {
                FlowOp::ReturnExpr(self.lower_expression(value)?)
            }
            RuntimeFlowOpSeed::Effect(effect) => FlowOp::Effect(self.lower_line_effect(effect)?),
            RuntimeFlowOpSeed::EvaluatedEffect(effect) => {
                FlowOp::EvaluatedEffect(self.lower_evaluated_effect(effect)?)
            }
            RuntimeFlowOpSeed::RegisterDefer {
                site,
                outcome,
                captures,
                owner,
            } => FlowOp::RegisterDefer {
                site,
                outcome,
                captures: captures
                    .into_iter()
                    .map(|capture| self.lower_expression(capture))
                    .collect::<Result<_, _>>()?,
                owner,
            },
            RuntimeFlowOpSeed::RegisterCleanup { key, effect } => FlowOp::RegisterCleanup {
                key,
                effect: self.lower_line_effect(effect)?,
            },
            RuntimeFlowOpSeed::CancelCleanup { key } => FlowOp::CancelCleanup { key },
            RuntimeFlowOpSeed::EnterScope { identity } => FlowOp::EnterScope { identity },
            RuntimeFlowOpSeed::ExitScope => FlowOp::ExitScope,
            RuntimeFlowOpSeed::Noop => FlowOp::Noop,
        })
    }

    fn lower_project_call_plan(
        &self,
        seed: super::super::RuntimeProjectCallPlanSeed,
    ) -> Result<super::super::RuntimeProjectCallPlan, RuntimePlanBuildError> {
        use super::super::{
            RuntimeProjectCallAttachedMaterialization, RuntimeProjectCallAttachedPresence,
            RuntimeProjectCallFixedMaterialization, RuntimeProjectCallOperand,
            RuntimeProjectCallOrdinaryMaterialization, RuntimeProjectCallRestMaterialization,
        };

        let callee = self.lower_expression(seed.callee)?;
        let state = self.resolve_callable_state_seed(&seed.state)?;
        let operands = seed
            .operands
            .into_vec()
            .into_iter()
            .map(|operand| {
                Ok(RuntimeProjectCallOperand::from_admitted_parts(
                    self.lower_expression(operand.value)?,
                    operand.mode,
                ))
            })
            .collect::<Result<Box<[_]>, RuntimePlanBuildError>>()?;
        let ordinary = seed
            .ordinary
            .into_vec()
            .into_iter()
            .map(|row| -> Result<_, RuntimePlanBuildError> {
                Ok(match row {
                    super::super::RuntimeProjectCallOrdinaryMaterializationSeed::Fixed(row) => {
                        RuntimeProjectCallOrdinaryMaterialization::Fixed(
                            RuntimeProjectCallFixedMaterialization::from_admitted_parts(
                                row.parameter,
                                self.resolve_seed_type("project-call fixed ABI type", row.abi_ty)?,
                                self.resolve_seed_type(
                                    "project-call fixed binding type",
                                    row.binding_ty,
                                )?,
                                row.source_index,
                            ),
                        )
                    }
                    super::super::RuntimeProjectCallOrdinaryMaterializationSeed::Rest(row) => {
                        RuntimeProjectCallOrdinaryMaterialization::Rest(
                            RuntimeProjectCallRestMaterialization::from_admitted_parts(
                                row.parameter,
                                self.resolve_seed_type("project-call rest ABI type", row.abi_ty)?,
                                self.resolve_seed_type(
                                    "project-call rest binding type",
                                    row.binding_ty,
                                )?,
                                row.source_indices,
                            ),
                        )
                    }
                })
            })
            .collect::<Result<Box<[_]>, RuntimePlanBuildError>>()?;
        let attached = seed
            .attached
            .map(|row| -> Result<_, RuntimePlanBuildError> {
                let presence = match row.presence {
                    super::super::RuntimeProjectCallAttachedPresenceSeed::RequiredPresent => {
                        RuntimeProjectCallAttachedPresence::RequiredPresent
                    }
                    super::super::RuntimeProjectCallAttachedPresenceSeed::OptionalPresent => {
                        RuntimeProjectCallAttachedPresence::OptionalPresent
                    }
                    super::super::RuntimeProjectCallAttachedPresenceSeed::OptionalOmitted => {
                        RuntimeProjectCallAttachedPresence::OptionalOmitted
                    }
                    super::super::RuntimeProjectCallAttachedPresenceSeed::DefaultedPresent => {
                        RuntimeProjectCallAttachedPresence::DefaultedPresent
                    }
                    super::super::RuntimeProjectCallAttachedPresenceSeed::DefaultedOmitted => {
                        RuntimeProjectCallAttachedPresence::DefaultedOmitted
                    }
                };
                Ok(
                    RuntimeProjectCallAttachedMaterialization::from_admitted_parts(
                        self.resolve_seed_type("project-call attached ABI type", row.abi_ty)?,
                        self.resolve_seed_type(
                            "project-call attached binding type",
                            row.binding_ty,
                        )?,
                        row.source_index,
                        presence,
                    ),
                )
            })
            .transpose()?;
        self.validate_project_call_parts(
            &callee,
            state,
            seed.completed_group,
            &operands,
            &ordinary,
            attached.as_ref(),
        )?;
        super::super::RuntimeProjectCallPlan::try_from_admitted_parts(
            callee,
            state,
            seed.completed_group,
            operands,
            ordinary,
            attached,
        )
        .map_err(RuntimePlanBuildError::from)
    }
    fn validate_project_call_parts(
        &self,
        callee: &RuntimeExpr,
        state: crate::runtime_id::RuntimeCallableStateId,
        completed_group: u32,
        operands: &[super::super::RuntimeProjectCallOperand],
        ordinary: &[super::super::RuntimeProjectCallOrdinaryMaterialization],
        attached: Option<&super::super::RuntimeProjectCallAttachedMaterialization>,
    ) -> Result<(), RuntimePlanBuildError> {
        use super::super::{
            RuntimeCallableAttachedContract as Attached, RuntimeCallablePosition,
            RuntimeProjectCallAttachedPresence as Presence,
        };
        let states = self.callable_states.borrow();
        let definition = states
            .states
            .get(state.index())
            .and_then(Option::as_ref)
            .ok_or(RuntimePlanBuildError::UnknownCallableState { state })?;
        require_same(
            "project-call callee state",
            definition.function_type,
            callee.ty(),
        )?;
        let group = match definition.position {
            RuntimeCallablePosition::Unapplied => 0,
            RuntimeCallablePosition::WithinGroup { group, .. } => group,
            RuntimeCallablePosition::AfterGroup { completed } => completed + 1,
        };
        if group != completed_group
            || definition.parameters.len() != ordinary.len()
            || definition
                .parameters
                .iter()
                .zip(ordinary)
                .any(|(parameter, row)| {
                    parameter.coordinate.parameter != row.parameter()
                        || parameter.abi_ty != row.abi_ty()
                        || parameter.binding_ty != row.binding_ty()
                })
        {
            return Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                context: "callable state parameter row",
            });
        }
        let attached_matches = match (&definition.attached, attached) {
            (Attached::None, None) => true,
            (Attached::Required { ty }, Some(row)) => {
                row.binding_ty() == *ty && matches!(row.presence(), Presence::RequiredPresent)
            }
            (Attached::Optional { binding, .. }, Some(row)) => {
                row.binding_ty() == *binding
                    && matches!(
                        row.presence(),
                        Presence::OptionalPresent | Presence::OptionalOmitted
                    )
            }
            (Attached::Defaulted { ty, .. }, Some(row)) => {
                row.binding_ty() == *ty
                    && matches!(
                        row.presence(),
                        Presence::DefaultedPresent | Presence::DefaultedOmitted
                    )
            }
            _ => false,
        };
        if !attached_matches {
            return Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                context: "callable state attached row",
            });
        }
        for row in ordinary {
            match row {
                super::super::RuntimeProjectCallOrdinaryMaterialization::Fixed(row) => {
                    require_same(
                        "project-call fixed ABI/binding type",
                        row.abi_ty(),
                        row.binding_ty(),
                    )?;
                    let operand = self.project_call_operand(operands, row.source_index())?;
                    if operand.mode() != RuntimeCallArgumentMode::Value {
                        return Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                            context: "fixed operand mode",
                        });
                    }
                    require_same(
                        "project-call fixed source type",
                        row.abi_ty(),
                        operand.value().ty(),
                    )?;
                }
                super::super::RuntimeProjectCallOrdinaryMaterialization::Rest(row) => {
                    let item = match self.projection(row.binding_ty())? {
                        RuntimePlanTypeProjection::Sequence {
                            kind: RuntimePlanSequenceKind::Vec,
                            item,
                        } if *item == row.abi_ty() => *item,
                        _ => {
                            return Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                                context: "rest binding sequence",
                            });
                        }
                    };
                    for &source_index in row.source_indices() {
                        let operand = self.project_call_operand(operands, source_index)?;
                        match operand.mode() {
                            RuntimeCallArgumentMode::Value => {
                                require_same(
                                    "project-call rest scalar source type",
                                    item,
                                    operand.value().ty(),
                                )?;
                            }
                            RuntimeCallArgumentMode::Spread => {
                                self.validate_project_call_spread_source(
                                    operand.value().ty(),
                                    item,
                                )?;
                            }
                        }
                    }
                }
            }
        }
        if let Some(attached) = attached {
            self.validate_project_call_attached(operands, attached)?;
        }
        Ok(())
    }

    fn project_call_operand<'a>(
        &self,
        operands: &'a [super::super::RuntimeProjectCallOperand],
        index: u32,
    ) -> Result<&'a super::super::RuntimeProjectCallOperand, RuntimePlanBuildError> {
        usize::try_from(index)
            .ok()
            .and_then(|index| operands.get(index))
            .ok_or(RuntimePlanBuildError::ProjectCall(
                super::super::RuntimeProjectCallPlanError::SourceOutOfRange { index },
            ))
    }

    fn validate_project_call_spread_source(
        &self,
        source_ty: RuntimePlanTypeId,
        item: RuntimePlanTypeId,
    ) -> Result<(), RuntimePlanBuildError> {
        let homogeneous = match self.projection(source_ty)? {
            RuntimePlanTypeProjection::Sequence {
                item: source_item, ..
            }
            | RuntimePlanTypeProjection::Array {
                item: source_item, ..
            } => *source_item == item,
            RuntimePlanTypeProjection::Tuple(items) => {
                items.iter().all(|source_item| *source_item == item)
            }
            _ => false,
        };
        if homogeneous {
            Ok(())
        } else {
            Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                context: "rest spread source",
            })
        }
    }

    fn validate_project_call_attached(
        &self,
        operands: &[super::super::RuntimeProjectCallOperand],
        attached: &super::super::RuntimeProjectCallAttachedMaterialization,
    ) -> Result<(), RuntimePlanBuildError> {
        let option_item = |ty| match self.projection(ty)? {
            RuntimePlanTypeProjection::Option { item, some_payload } => {
                let payload = self.projection(*some_payload)?;
                match payload {
                    RuntimePlanTypeProjection::Tuple(fields)
                        if fields.len() == 1 && fields[0] == *item =>
                    {
                        Ok(*item)
                    }
                    _ => Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                        context: "attached option payload",
                    }),
                }
            }
            _ => Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                context: "attached option type",
            }),
        };
        match attached.presence() {
            super::super::RuntimeProjectCallAttachedPresence::RequiredPresent => {
                require_same(
                    "required attached ABI/binding type",
                    attached.abi_ty(),
                    attached.binding_ty(),
                )?;
                let operand = self.project_call_operand(
                    operands,
                    attached.source_index().ok_or(
                        RuntimePlanBuildError::InvalidProjectCallAbi {
                            context: "required attached source",
                        },
                    )?,
                )?;
                if operand.mode() != RuntimeCallArgumentMode::Value {
                    return Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                        context: "required attached operand mode",
                    });
                }
                require_same(
                    "required attached source type",
                    attached.binding_ty(),
                    operand.value().ty(),
                )?;
            }
            super::super::RuntimeProjectCallAttachedPresence::OptionalPresent => {
                require_same(
                    "optional attached ABI/binding type",
                    attached.abi_ty(),
                    attached.binding_ty(),
                )?;
                let item = option_item(attached.abi_ty())?;
                let operand = self.project_call_operand(
                    operands,
                    attached.source_index().ok_or(
                        RuntimePlanBuildError::InvalidProjectCallAbi {
                            context: "optional attached source",
                        },
                    )?,
                )?;
                if operand.mode() != RuntimeCallArgumentMode::Value {
                    return Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                        context: "optional attached operand mode",
                    });
                }
                require_same("optional attached source type", item, operand.value().ty())?;
            }
            super::super::RuntimeProjectCallAttachedPresence::OptionalOmitted => {
                require_same(
                    "omitted optional attached ABI/binding type",
                    attached.abi_ty(),
                    attached.binding_ty(),
                )?;
                let _ = option_item(attached.abi_ty())?;
            }
            super::super::RuntimeProjectCallAttachedPresence::DefaultedPresent => {
                let item = option_item(attached.abi_ty())?;
                require_same(
                    "defaulted attached binding type",
                    item,
                    attached.binding_ty(),
                )?;
                let operand = self.project_call_operand(
                    operands,
                    attached.source_index().ok_or(
                        RuntimePlanBuildError::InvalidProjectCallAbi {
                            context: "defaulted attached source",
                        },
                    )?,
                )?;
                if operand.mode() != RuntimeCallArgumentMode::Value {
                    return Err(RuntimePlanBuildError::InvalidProjectCallAbi {
                        context: "defaulted attached operand mode",
                    });
                }
                require_same(
                    "defaulted attached source type",
                    attached.binding_ty(),
                    operand.value().ty(),
                )?;
            }
            super::super::RuntimeProjectCallAttachedPresence::DefaultedOmitted => {
                let item = option_item(attached.abi_ty())?;
                require_same(
                    "omitted default attached binding type",
                    item,
                    attached.binding_ty(),
                )?;
            }
        }
        Ok(())
    }

    fn lower_line_operation(
        &self,
        operation: RuntimeLineOperationSeed,
    ) -> Result<RuntimeLineOperation, RuntimePlanBuildError> {
        Ok(match operation {
            RuntimeLineOperationSeed::AcquireActor {
                site,
                character,
                scope,
            } => RuntimeLineOperation::AcquireActor {
                site,
                character,
                scope,
            },
            RuntimeLineOperationSeed::Schedule {
                site,
                delay,
                child,
                captures,
            } => RuntimeLineOperation::Schedule {
                site,
                delay: self.lower_expression(delay)?,
                child: child.runtime_id().ok_or(
                    RuntimePlanBuildError::InvalidLineTaskNodeSeedId {
                        actual: child.get(),
                    },
                )?,
                captures: captures
                    .into_vec()
                    .into_iter()
                    .map(|capture| -> Result<_, RuntimePlanBuildError> {
                        let (local, expected) = self.resolve_local(&capture.local)?;
                        let value = self.lower_expression(capture.value)?;
                        require_same("scheduled capture local", expected, value.ty())?;
                        Ok(super::super::RuntimeScheduledCapture::new(local, value))
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_boxed_slice(),
            },
            RuntimeLineOperationSeed::ActorLook {
                site,
                character,
                actor,
                look,
                crossfade,
            } => RuntimeLineOperation::ActorLook {
                site,
                character,
                actor: super::super::RuntimeBorrowedLocal::new(
                    self.resolve_local(actor.local())?.0,
                ),
                look: self.lower_expression(look)?,
                crossfade: self.lower_expression(crossfade)?,
            },
            RuntimeLineOperationSeed::VoiceHandle { site } => {
                RuntimeLineOperation::VoiceHandle { site }
            }
        })
    }

    fn lower_iterator_evidence(
        &self,
        source: RuntimePlanTypeId,
        evidence: RuntimeIteratorEvidenceSeed,
    ) -> Result<(RuntimePlanTypeId, RuntimeIteratorEvidence), RuntimePlanBuildError> {
        match evidence {
            RuntimeIteratorEvidenceSeed::Builtin(evidence) => {
                self.lower_builtin_iterator(source, &evidence)
            }
            RuntimeIteratorEvidenceSeed::Witness(evidence) => {
                self.lower_iterator_witness(source, evidence)
            }
        }
    }

    fn builtin_iterator_item_type(
        &self,
        source: RuntimePlanTypeId,
        evidence: RuntimeBuiltinIteratorFamily,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
        let projection = self.projection(source)?;
        match (evidence, projection) {
            (RuntimeBuiltinIteratorFamily::Range, RuntimePlanTypeProjection::Range(item))
            | (
                RuntimeBuiltinIteratorFamily::Stream,
                RuntimePlanTypeProjection::Stream { item, .. },
            )
            | (
                RuntimeBuiltinIteratorFamily::Array,
                RuntimePlanTypeProjection::Array { item, .. },
            )
            | (
                RuntimeBuiltinIteratorFamily::Seq,
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Seq,
                    item,
                },
            )
            | (
                RuntimeBuiltinIteratorFamily::Vec,
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item,
                },
            )
            | (
                RuntimeBuiltinIteratorFamily::Slice,
                RuntimePlanTypeProjection::Sequence {
                    kind: RuntimePlanSequenceKind::Slice,
                    item,
                },
            ) => Ok(*item),
            (
                RuntimeBuiltinIteratorFamily::TupleHomogeneous,
                RuntimePlanTypeProjection::Tuple(items),
            ) => {
                let Some(first) = items.first().copied() else {
                    return invalid_projection("empty homogeneous tuple iterator", source);
                };
                if items.iter().all(|item| *item == first) {
                    Ok(first)
                } else {
                    invalid_projection("heterogeneous tuple iterator", source)
                }
            }
            _ => invalid_projection("flow for iterator evidence", source),
        }
    }

    fn lower_builtin_iterator(
        &self,
        source: RuntimePlanTypeId,
        evidence: &RuntimeBuiltinIteratorEvidenceSeed,
    ) -> Result<(RuntimePlanTypeId, RuntimeIteratorEvidence), RuntimePlanBuildError> {
        let item = self.resolve_seed_type("builtin iterator item", evidence.item)?;
        let iterator = self.resolve_seed_type("builtin iterator state", evidence.iterator)?;
        let next_value =
            self.resolve_seed_type("builtin iterator next value", evidence.next_value)?;
        let step = self.resolve_seed_type("builtin iterator step", evidence.step)?;
        require_same(
            "builtin iterator source item",
            item,
            self.builtin_iterator_item_type(source, evidence.family)?,
        )?;
        match self.projection(iterator)? {
            RuntimePlanTypeProjection::Iterator(actual) if *actual == item => {}
            _ => return invalid_projection("builtin iterator state", iterator),
        }
        match self.projection(next_value)? {
            RuntimePlanTypeProjection::Option { item: actual, .. } if *actual == item => {}
            _ => return invalid_projection("builtin iterator next value", next_value),
        }
        match self.projection(step)? {
            RuntimePlanTypeProjection::Tuple(items) if items.as_ref() == [iterator, next_value] => {
            }
            _ => return invalid_projection("builtin iterator step", step),
        }
        Ok((
            item,
            RuntimeIteratorEvidence::builtin(RuntimeBuiltinIteratorEvidence {
                family: evidence.family,
                item,
                iterator,
                next_value,
                step,
            }),
        ))
    }

    fn await_many_item_type(
        &self,
        source: RuntimePlanTypeId,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
        match self.projection(source)? {
            RuntimePlanTypeProjection::Sequence { item, .. }
            | RuntimePlanTypeProjection::Array { item, .. } => Ok(*item),
            RuntimePlanTypeProjection::Tuple(items) => {
                let Some(first) = items.first().copied() else {
                    return invalid_projection("empty AwaitMany tuple source", source);
                };
                if items.iter().all(|item| *item == first) {
                    Ok(first)
                } else {
                    invalid_projection("heterogeneous AwaitMany tuple source", source)
                }
            }
            _ => invalid_projection("AwaitMany source", source),
        }
    }

    fn awaited_item_type(
        &self,
        source: RuntimePlanTypeId,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
        match self.projection(source)? {
            RuntimePlanTypeProjection::Need(item)
            | RuntimePlanTypeProjection::ThreadHandle(item) => Ok(*item),
            _ => invalid_projection("Await source", source),
        }
    }

    fn need_item_type(
        &self,
        source: RuntimePlanTypeId,
    ) -> Result<RuntimePlanTypeId, RuntimePlanBuildError> {
        match self.projection(source)? {
            RuntimePlanTypeProjection::Need(item) => Ok(*item),
            _ => invalid_projection("Await source", source),
        }
    }

    fn lower_iterator_witness(
        &self,
        source: RuntimePlanTypeId,
        evidence: RuntimeIteratorWitnessEvidenceSeed,
    ) -> Result<(RuntimePlanTypeId, RuntimeIteratorEvidence), RuntimePlanBuildError> {
        let item = self.resolve_seed_type("iterator witness item", evidence.item)?;
        let iterator = self.resolve_seed_type("iterator witness state", evidence.iterator)?;
        let executable = match evidence.executable {
            RuntimeIteratorWitnessExecutableSeed::TraitCalls { into_iter, next } => {
                let into_iter = self.resolve_iterator_method(
                    &into_iter,
                    RuntimeReceiverMode::Owned,
                    &[source],
                    iterator,
                    "IntoIterator::into_iter",
                )?;
                let next = self.resolve_iterator_next(&next, iterator, item)?;
                RuntimeIteratorWitnessExecutable::TraitCalls { into_iter, next }
            }
            RuntimeIteratorWitnessExecutableSeed::IdentityIntoIterator { next } => {
                require_same("identity iterator source", source, iterator)?;
                let next = self.resolve_iterator_next(&next, iterator, item)?;
                RuntimeIteratorWitnessExecutable::IdentityIntoIterator { next }
            }
        };
        Ok((
            item,
            RuntimeIteratorEvidence::Witness(RuntimeIteratorWitnessEvidence {
                item,
                iterator,
                executable,
            }),
        ))
    }

    fn resolve_iterator_next(
        &self,
        method: &super::RuntimeTraitMethodSeedId,
        iterator: RuntimePlanTypeId,
        item: RuntimePlanTypeId,
    ) -> Result<super::super::RuntimeTraitMethodId, RuntimePlanBuildError> {
        let (method, receiver, parameters, result) = method
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
        if receiver != RuntimeReceiverMode::MutRef || parameters != [iterator] {
            return Err(RuntimePlanBuildError::InvalidIteratorWitness {
                context: "Iterator::next receiver/parameters",
            });
        }
        match self.projection(result)? {
            RuntimePlanTypeProjection::Option { item: actual, .. } if *actual == item => Ok(method),
            _ => Err(RuntimePlanBuildError::InvalidIteratorWitness {
                context: "Iterator::next result",
            }),
        }
    }

    fn resolve_iterator_method(
        &self,
        method: &super::RuntimeTraitMethodSeedId,
        expected_receiver: RuntimeReceiverMode,
        expected_parameters: &[RuntimePlanTypeId],
        expected_result: RuntimePlanTypeId,
        context: &'static str,
    ) -> Result<super::super::RuntimeTraitMethodId, RuntimePlanBuildError> {
        let (method, receiver, parameters, result) = method
            .resolve(&self.issuer)
            .ok_or(RuntimePlanBuildError::ForeignTraitMethodSeed)?;
        if receiver != expected_receiver
            || parameters != expected_parameters
            || result != expected_result
        {
            return Err(RuntimePlanBuildError::InvalidIteratorWitness { context });
        }
        Ok(method)
    }

    fn lower_choice_option(
        &self,
        option: RuntimeChoiceOptionSeed,
    ) -> Result<ChoiceRuntimeOption, RuntimePlanBuildError> {
        Ok(ChoiceRuntimeOption {
            id: option.id,
            label: option.label,
            target: option.target,
            out: option.out,
            effects: self.lower_line_effects(option.effects)?,
        })
    }

    fn lower_host_call_target(
        &self,
        target: RuntimeHostCallTargetSeed,
    ) -> Result<RuntimeHostCallTarget, RuntimePlanBuildError> {
        Ok(RuntimeHostCallTarget {
            producer: target.producer,
            public_id: target.public_id,
            capability: target.capability,
            operation: target.operation,
            contract: target.contract,
            args: target
                .args
                .into_iter()
                .map(|arg| self.lower_host_argument(arg))
                .collect::<Result<_, _>>()?,
            result: self.resolve_seed_type("host call result", target.result)?,
            mode: target.mode,
            deterministic: target.deterministic,
        })
    }

    fn lower_need_producer_template(
        &self,
        template: super::RuntimeNeedProducerTemplateSeed,
        bound: &[super::RuntimeLocalSeedId],
    ) -> Result<crate::task::NeedProducerTemplate, RuntimePlanBuildError> {
        let definition = self.lower_producer_definition(template.definition)?;
        let expected = crate::task::TaskHandleType::Need {
            payload: definition.handle_type.payload_type(),
        };
        if definition.handle_type != expected {
            return Err(RuntimePlanBuildError::ProducerHandleType {
                expected,
                actual: definition.handle_type,
            });
        }
        Ok(crate::task::NeedProducerTemplate {
            definition,
            request: self.lower_host_task_request(template.request, bound)?,
        })
    }

    fn lower_producer_definition(
        &self,
        definition: crate::task::NeedProducerDefinition,
    ) -> Result<crate::task::NeedProducerDefinition, RuntimePlanBuildError> {
        self.resolve_seed_type(
            "producer payload",
            crate::pattern::RuntimeSemanticTypeId::from_bytes(
                *definition.handle_type.payload_type().as_bytes(),
            ),
        )?;
        Ok(definition)
    }

    fn lower_host_task_request(
        &self,
        request: RuntimeHostTaskRequestTemplateSeed,
        bound: &[super::RuntimeLocalSeedId],
    ) -> Result<HostTaskRequestTemplate, RuntimePlanBuildError> {
        Ok(HostTaskRequestTemplate {
            captures: request
                .free_locals(bound)
                .iter()
                .map(|local| {
                    local
                        .resolve(&self.issuer)
                        .map(|(local, _)| local)
                        .ok_or(RuntimePlanBuildError::ForeignLocalSeed)
                })
                .collect::<Result<_, _>>()?,
            capability: request.capability,
            operation: request.operation,
            args: request
                .args
                .into_iter()
                .map(|arg| {
                    Ok(match arg {
                        RuntimeHostArgumentSeed::Positional(identity, value) => {
                            RuntimeHostArgumentTemplate::Positional(
                                identity,
                                self.lower_expression(value)?,
                            )
                        }
                        RuntimeHostArgumentSeed::Named(identity, argument) => {
                            RuntimeHostArgumentTemplate::Named(
                                identity,
                                NamedHostArg {
                                    name: argument.name,
                                    value: self.lower_expression(argument.value)?,
                                },
                            )
                        }
                        RuntimeHostArgumentSeed::Spread(identity, value) => {
                            RuntimeHostArgumentTemplate::Spread(
                                identity,
                                self.lower_expression(value)?,
                            )
                        }
                    })
                })
                .collect::<Result<_, RuntimePlanBuildError>>()?,
        })
    }

    fn lower_host_argument(
        &self,
        argument: RuntimeHostArgumentSeed,
    ) -> Result<RuntimeHostArgumentTemplate, RuntimePlanBuildError> {
        Ok(match argument {
            RuntimeHostArgumentSeed::Positional(identity, value) => {
                RuntimeHostArgumentTemplate::Positional(identity, self.lower_expression(value)?)
            }
            RuntimeHostArgumentSeed::Named(identity, argument) => {
                RuntimeHostArgumentTemplate::Named(
                    identity,
                    NamedHostArg {
                        name: argument.name,
                        value: self.lower_expression(argument.value)?,
                    },
                )
            }
            RuntimeHostArgumentSeed::Spread(identity, value) => {
                RuntimeHostArgumentTemplate::Spread(identity, self.lower_expression(value)?)
            }
        })
    }

    fn lower_line_effects(
        &self,
        effects: Vec<RuntimeLineEffectSeed>,
    ) -> Result<Vec<LineEffectRequest>, RuntimePlanBuildError> {
        effects
            .into_iter()
            .map(|effect| self.lower_line_effect(effect))
            .collect()
    }

    fn lower_line_effect(
        &self,
        effect: RuntimeLineEffectSeed,
    ) -> Result<LineEffectRequest, RuntimePlanBuildError> {
        match effect {
            RuntimeLineEffectSeed::Static(LineEffectRequest::Audio(_)) => {
                Err(RuntimePlanBuildError::RawExpressionCarrier {
                    context: "static line effect Audio",
                })
            }
            RuntimeLineEffectSeed::Static(effect) => {
                if let LineEffectRequest::SignalWrite(assignment)
                | LineEffectRequest::MetricWrite(assignment) = &effect
                {
                    super::super::value_admission::PlanValueAuthority::Building {
                        types: &self.types,
                        records: &self.nominal_record_domains,
                        variants: &self.variant_domains,
                    }
                    .validate_static_literal(
                        assignment.value(),
                        &mut crate::entry::schema::value_encoding::ValueResourceAdmission::live(
                            crate::entry::RuntimeSchemaLimits::engine_default(),
                        ),
                    )?;
                }
                Ok(effect)
            }
            RuntimeLineEffectSeed::Audio(command) => Ok(LineEffectRequest::Audio(Box::new(
                self.lower_audio_command(*command)?,
            ))),
        }
    }

    fn lower_audio_command(
        &self,
        command: RuntimeAudioCommandSeed,
    ) -> Result<RuntimeAudioCommand, RuntimePlanBuildError> {
        command.validate_metadata()?;
        command.try_map_expressions(|expression| self.lower_expression(expression))
    }
    fn lower_evaluated_effect(
        &self,
        effect: RuntimeEvaluatedEffectSeed,
    ) -> Result<RuntimeEffectExpr, RuntimePlanBuildError> {
        // Boolean policies remain at their original conversion point: failure
        // cannot lower a later message or change the first diagnostic.
        match effect {
            RuntimeEvaluatedEffectSeed::Ensure { condition, message } => {
                let condition = self.lower_expression(condition)?;
                self.require_bool("ensure condition", condition.ty())?;
                Ok(RuntimeEffectExpr::Ensure {
                    condition,
                    message: self.lower_expression(message)?,
                })
            }
            RuntimeEvaluatedEffectSeed::Assert {
                guard,
                condition,
                message,
                profile,
            } => {
                let condition = self.lower_expression(condition)?;
                self.require_bool("assert condition", condition.ty())?;
                Ok(RuntimeEffectExpr::Assert {
                    guard,
                    condition,
                    message,
                    profile,
                })
            }
            effect => effect.try_map_expressions(|expression| self.lower_expression(expression)),
        }
    }
    pub(super) fn validate_flow_operation_locals(
        &self,
        ops: &[FlowOp],
        scope: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        let mut used = BTreeSet::new();
        let mut scope_frames = Vec::new();
        self.validate_flow_operation_locals_inner(ops, scope, &mut used, &mut scope_frames)
    }

    pub(super) fn validate_flow_operation_locals_with_usage(
        &self,
        ops: &[FlowOp],
        scope: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<BTreeSet<RuntimeLocalDeclarationId>, RuntimePlanBuildError> {
        let mut used = BTreeSet::new();
        let mut scope_frames = Vec::new();
        self.validate_flow_operation_locals_inner(ops, scope, &mut used, &mut scope_frames)?;
        Ok(used)
    }

    pub(super) fn validate_line_task_actions_locals(
        &self,
        actions: &[&[FlowOp]],
        captures: &BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<BTreeSet<RuntimeLocalDeclarationId>, RuntimePlanBuildError> {
        let mut used = BTreeSet::new();
        for action in actions {
            let mut scope = captures.clone();
            let mut scope_frames = Vec::new();
            self.validate_flow_operation_locals_inner(
                action,
                &mut scope,
                &mut used,
                &mut scope_frames,
            )?;
        }
        Ok(used)
    }

    /// Conservative logical storage/work for the Line factory's source capture
    /// derivation and the actual lexical validator. Admission precedes values.
    /// The scalar summaries come from the complete borrowed operation grammar.
    pub(in crate::plan) fn line_group_source_scope_storage(
        atoms: usize,
        value_nodes: usize,
        owned_bodies: usize,
        lexical_scopes: usize,
        local_namespace: usize,
    ) -> Option<(usize, usize)> {
        // Source free-local collection retains a bound vector at each body,
        // expression scope and explicit scope frame. Invalid duplicate bindings
        // can grow those vectors: do not assume namespace-sized seed vectors.
        let seed_frames = owned_bodies
            .checked_add(value_nodes)?
            .checked_mul(2)?
            .checked_add(lexical_scopes)?
            .checked_add(3)?;
        let seed_bound = if local_namespace == 0 { 0 } else { atoms };
        let seed_slots = seed_frames.checked_mul(seed_bound)?;
        let seed_bytes = seed_slots
            .checked_mul(size_of::<super::RuntimeLocalSeedId>())?
            .checked_add(seed_frames.checked_mul(size_of::<Vec<super::RuntimeLocalSeedId>>())?)?;
        // The work frontier retains independent scope-frame snapshots. Count
        // both dimensions; nested Match within entered scopes is not linear in
        // the number of operations. Actual BTree scopes contain unique IDs.
        let frontier = owned_bodies
            .checked_add(value_nodes)?
            .checked_add(1)?
            .checked_mul(3)?;
        let runtime_sets = frontier
            .checked_mul(lexical_scopes.checked_add(1)?)?
            .checked_add(8)?;
        let unique = atoms.min(local_namespace);
        let runtime_slots = runtime_sets.checked_mul(unique)?;
        let runtime_bytes = runtime_slots
            .checked_mul(size_of::<RuntimeLocalDeclarationId>())?
            .checked_add(
                runtime_sets.checked_mul(size_of::<BTreeSet<RuntimeLocalDeclarationId>>())?,
            )?
            .checked_add(frontier.checked_mul(size_of::<FlowLocalValidationWork<'_>>())?)?;
        // Vec growth and simultaneous conversion scratch remain bounded even
        // before the ordinary duplicate/scope rejection reaches its final error.
        let storage = seed_bytes.checked_add(runtime_bytes)?.checked_mul(2)?;
        let work = seed_slots
            .checked_add(runtime_slots)?
            .checked_mul(2)?
            .checked_add(atoms.checked_mul(unique.checked_add(1)?)?)?;
        Some((storage, work))
    }

    fn validate_flow_match_arms_iterative(
        &self,
        arms: &[RuntimeMatchArm],
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
        scope_frames: &[BTreeSet<RuntimeLocalDeclarationId>],
    ) -> Result<(), RuntimePlanBuildError> {
        // Match branches clone lexical scope state, while the explicit list
        // cursor keeps a continuation chain of nested Match nodes off-stack.
        let mut work = vec![FlowLocalValidationWork::MatchArms {
            arms: arms.iter(),
            scope: scope.clone(),
            scope_frames: scope_frames.to_vec(),
        }];
        while let Some(frame) = work.pop() {
            match frame {
                FlowLocalValidationWork::MatchArms {
                    mut arms,
                    scope,
                    scope_frames,
                } => {
                    let Some(arm) = arms.next() else {
                        continue;
                    };
                    let arm_scope = extend_scope(
                        &scope,
                        arm.pattern
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                    work.push(FlowLocalValidationWork::MatchArms {
                        arms,
                        scope: scope.clone(),
                        scope_frames: scope_frames.clone(),
                    });
                    work.push(FlowLocalValidationWork::List(FlowLocalValidationFrame {
                        ops: &arm.ops,
                        next: 0,
                        scope: arm_scope.clone(),
                        scope_frames: scope_frames.clone(),
                        final_condition: None,
                    }));
                    if let Some(guard) = &arm.guard {
                        // A guard receives only its issued Copy leaves. Its
                        // retained candidate is inaccessible to executable ops.
                        let guard_scope = extend_scope(&scope, guard.copy_locals.iter().copied())?;
                        work.push(FlowLocalValidationWork::List(FlowLocalValidationFrame {
                            ops: &guard.ops,
                            next: 0,
                            scope: guard_scope,
                            scope_frames,
                            final_condition: guard.condition.as_ref(),
                        }));
                    }
                }
                FlowLocalValidationWork::List(mut frame) => {
                    let Some(op) = frame.ops.get(frame.next) else {
                        if let Some(condition) = frame.final_condition {
                            self.validate_expression_locals(condition, &frame.scope, used)?;
                        }
                        continue;
                    };
                    frame.next += 1;
                    if let FlowOp::Match { scrutinee, arms } = op {
                        self.validate_expression_locals(scrutinee, &frame.scope, used)?;
                        work.push(FlowLocalValidationWork::List(frame.clone()));
                        work.push(FlowLocalValidationWork::MatchArms {
                            arms: arms.iter(),
                            scope: frame.scope,
                            scope_frames: frame.scope_frames,
                        });
                    } else {
                        self.validate_flow_operation_locals_inner(
                            std::slice::from_ref(op),
                            &mut frame.scope,
                            used,
                            &mut frame.scope_frames,
                        )?;
                        work.push(FlowLocalValidationWork::List(frame));
                    }
                }
            }
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "lexical validation exhaustively rejects runtime-only continuation operations"
    )]
    fn validate_flow_operation_locals_inner(
        &self,
        ops: &[FlowOp],
        scope: &mut BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
        scope_frames: &mut Vec<BTreeSet<RuntimeLocalDeclarationId>>,
    ) -> Result<(), RuntimePlanBuildError> {
        for op in ops {
            match op {
                FlowOp::StandardMap(control) => {
                    require_local_in_scope(control.source(), scope)?;
                    require_local_in_scope(control.mapping(), scope)?;
                    used.insert(control.source());
                    used.insert(control.mapping());
                    let mut body_scope = scope.clone();
                    let mut body_frames = scope_frames.clone();
                    self.validate_flow_operation_locals_inner(
                        control.mapping_body(),
                        &mut body_scope,
                        used,
                        &mut body_frames,
                    )?;
                    require_local_in_scope(control.result(), &body_scope)?;
                    if body_frames != *scope_frames {
                        return Err(RuntimePlanBuildError::FlowLoweringInvariant {
                            context: "Map body changes its caller's lexical scope stack",
                        });
                    }
                    let mut continuation_scope = extend_scope(scope, [control.result()])?;
                    if let Some(continuation) = control.consuming_body() {
                        self.validate_flow_operation_locals_inner(
                            continuation,
                            &mut continuation_scope,
                            used,
                            scope_frames,
                        )?;
                    }
                    *scope = continuation_scope;
                }
                FlowOp::Let { pattern, expr } => {
                    self.validate_expression_locals(expr, scope, used)?;
                    *scope = extend_scope(
                        scope,
                        pattern
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                }
                FlowOp::FormatOperandAttempt {
                    attempt,
                    parameter,
                    body,
                    value,
                } => {
                    let manifest = self
                        .format_attempts
                        .get(attempt.index())
                        .filter(|candidate| candidate.id() == *attempt)
                        .ok_or(RuntimePlanBuildError::UnknownFormatAttempt { attempt: *attempt })?;
                    let expected = manifest
                        .operands()
                        .iter()
                        .find(|operand| operand.parameter() == *parameter)
                        .map(super::super::RuntimeFormatAttemptOperand::ty)
                        .ok_or(RuntimePlanBuildError::MissingFormatAttemptOperand {
                            attempt: *attempt,
                            parameter: *parameter,
                        })?;
                    require_same("format-attempt operand", expected, value.ty())?;
                    let mut body_scope = scope.clone();
                    let mut body_frames = scope_frames.clone();
                    self.validate_flow_operation_locals_inner(
                        body,
                        &mut body_scope,
                        used,
                        &mut body_frames,
                    )?;
                    self.validate_expression_locals(value, &body_scope, used)?;
                }
                FlowOp::LetElse {
                    pattern,
                    expr,
                    else_ops,
                } => {
                    self.validate_expression_locals(expr, scope, used)?;
                    let mut else_scope = scope.clone();
                    let mut else_frames = scope_frames.clone();
                    self.validate_flow_operation_locals_inner(
                        else_ops,
                        &mut else_scope,
                        used,
                        &mut else_frames,
                    )?;
                    *scope = extend_scope(
                        scope,
                        pattern
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                }
                FlowOp::Assign { place, value } => {
                    require_local_in_scope(place.local(), scope)?;
                    used.insert(place.local());
                    self.validate_expression_locals(value, scope, used)?;
                }
                FlowOp::LineOperation { binding, operation } => {
                    if let RuntimeLineOperation::ActorLook { actor, .. } = operation {
                        require_local_in_scope(actor.local(), scope)?;
                        used.insert(actor.local());
                    }
                    for expression in operation.argument_exprs() {
                        self.validate_expression_locals(expression, scope, used)?;
                    }
                    if let Some(binding) = binding {
                        *scope = extend_scope(
                            scope,
                            binding
                                .binding_declarations()
                                .map(RuntimePatternBindingDeclaration::local),
                        )?;
                    }
                }
                FlowOp::CommitDialogueResult { value } | FlowOp::SelectDialogueResult { value } => {
                    self.validate_expression_locals(value, scope, used)?;
                }
                FlowOp::Choice { options, .. } => {
                    for option in options {
                        self.validate_line_effect_locals(&option.effects, scope, used)?;
                    }
                }
                FlowOp::Await {
                    binding,
                    target,
                    observers,
                } => {
                    self.validate_expression_locals(&target.source, scope, used)?;
                    for observer in observers {
                        let mut observer_scope = extend_scope(
                            scope,
                            observer
                                .pattern
                                .binding_declarations()
                                .map(RuntimePatternBindingDeclaration::local),
                        )?;
                        self.validate_flow_operation_locals_inner(
                            &observer.ops,
                            &mut observer_scope,
                            used,
                            scope_frames,
                        )?;
                    }
                    if let Some(binding) = binding {
                        *scope = extend_scope(
                            scope,
                            binding
                                .binding_declarations()
                                .map(RuntimePatternBindingDeclaration::local),
                        )?;
                    }
                }
                FlowOp::StartNeedProducer { binding, target } => {
                    for argument in target.arguments() {
                        self.validate_expression_locals(argument.value(), scope, used)?;
                    }
                    *scope = extend_scope(
                        scope,
                        binding
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                }
                FlowOp::AwaitMany {
                    binding,
                    target,
                    pending,
                } => {
                    self.validate_expression_locals(&target.source, scope, used)?;
                    let item_scope = extend_scope(scope, [target.item_binding])?;
                    self.validate_task_request_locals(&target.child.request, &item_scope, used)?;
                    self.validate_task_request_locals(&target.base.request, scope, used)?;
                    self.validate_line_effect_locals(pending, scope, used)?;
                    if let Some(binding) = binding {
                        *scope = extend_scope(
                            scope,
                            binding
                                .binding_declarations()
                                .map(RuntimePatternBindingDeclaration::local),
                        )?;
                    }
                }
                FlowOp::HostCall { binding, target } => {
                    self.validate_host_argument_locals(&target.args, scope, used)?;
                    if let Some(binding) = binding {
                        *scope = extend_scope(
                            scope,
                            binding
                                .binding_declarations()
                                .map(RuntimePatternBindingDeclaration::local),
                        )?;
                    }
                }
                FlowOp::ApplyGroup {
                    callee,
                    args,
                    result,
                } => {
                    self.validate_expression_locals(callee, scope, used)?;
                    for argument in args {
                        self.validate_expression_locals(argument.value(), scope, used)?;
                    }
                    *scope = extend_scope(
                        scope,
                        result
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                }
                FlowOp::ProjectCall { site } => {
                    let row = self.project_call_sites.borrow().get(*site).cloned().ok_or(
                        RuntimePlanBuildError::InvalidProjectCallAbi {
                            context: "project-call catalog site",
                        },
                    )?;
                    let plan = row.plan();
                    self.validate_expression_locals(plan.callee(), scope, used)?;
                    for operand in plan.operands() {
                        self.validate_expression_locals(operand.value(), scope, used)?;
                    }
                    *scope = extend_scope(
                        scope,
                        row.result()
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                }
                FlowOp::If {
                    condition,
                    then_ops,
                    else_ops,
                } => {
                    self.validate_expression_locals(condition, scope, used)?;
                    let mut then_scope = scope.clone();
                    let mut then_frames = scope_frames.clone();
                    self.validate_flow_operation_locals_inner(
                        then_ops,
                        &mut then_scope,
                        used,
                        &mut then_frames,
                    )?;
                    let mut else_scope = scope.clone();
                    let mut else_frames = scope_frames.clone();
                    self.validate_flow_operation_locals_inner(
                        else_ops,
                        &mut else_scope,
                        used,
                        &mut else_frames,
                    )?;
                }
                FlowOp::IfLet {
                    pattern,
                    expr,
                    guard,
                    then_ops,
                    else_ops,
                } => {
                    self.validate_expression_locals(expr, scope, used)?;
                    let mut then_scope = extend_scope(
                        scope,
                        pattern
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                    if let Some(guard) = guard {
                        self.validate_expression_locals(guard, &then_scope, used)?;
                    }
                    let mut then_frames = scope_frames.clone();
                    self.validate_flow_operation_locals_inner(
                        then_ops,
                        &mut then_scope,
                        used,
                        &mut then_frames,
                    )?;
                    let mut else_scope = scope.clone();
                    let mut else_frames = scope_frames.clone();
                    self.validate_flow_operation_locals_inner(
                        else_ops,
                        &mut else_scope,
                        used,
                        &mut else_frames,
                    )?;
                }
                FlowOp::Match { scrutinee, arms } => {
                    self.validate_expression_locals(scrutinee, scope, used)?;
                    self.validate_flow_match_arms_iterative(arms, scope, used, scope_frames)?;
                }
                FlowOp::Loop { result, body } => {
                    let mut nested = scope.clone();
                    let mut nested_frames = scope_frames.clone();
                    self.validate_flow_operation_locals_inner(
                        body,
                        &mut nested,
                        used,
                        &mut nested_frames,
                    )?;
                    if let Some(result) = result {
                        *scope = extend_scope(
                            scope,
                            result
                                .binding_declarations()
                                .map(RuntimePatternBindingDeclaration::local),
                        )?;
                    }
                }
                FlowOp::ProgramTask {
                    binding,
                    captures,
                    body,
                    ..
                } => {
                    let mut nested = BTreeSet::new();
                    for capture in captures {
                        require_local_in_scope(capture.local, scope)?;
                        used.insert(capture.local);
                        nested.insert(capture.local);
                    }
                    let mut nested_frames = Vec::new();
                    self.validate_flow_operation_locals_inner(
                        body,
                        &mut nested,
                        used,
                        &mut nested_frames,
                    )?;
                    *scope = extend_scope(
                        scope,
                        binding
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                }
                FlowOp::Scope { body, .. } => {
                    let mut nested = scope.clone();
                    let mut nested_frames = scope_frames.clone();
                    nested_frames.push(scope.clone());
                    let expected_depth = nested_frames.len();
                    self.validate_flow_operation_locals_inner(
                        body,
                        &mut nested,
                        used,
                        &mut nested_frames,
                    )?;
                    if nested_frames.len() != expected_depth {
                        return Err(RuntimePlanBuildError::FlowScopeUnderflow {
                            operation: "Scope body",
                        });
                    }
                }
                FlowOp::While { condition, body } => {
                    self.validate_expression_locals(condition, scope, used)?;
                    let mut nested = scope.clone();
                    let mut nested_frames = scope_frames.clone();
                    self.validate_flow_operation_locals_inner(
                        body,
                        &mut nested,
                        used,
                        &mut nested_frames,
                    )?;
                }
                FlowOp::WhileLet {
                    pattern,
                    expr,
                    guard,
                    body,
                } => {
                    self.validate_expression_locals(expr, scope, used)?;
                    let mut nested = extend_scope(
                        scope,
                        pattern
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                    if let Some(guard) = guard {
                        self.validate_expression_locals(guard, &nested, used)?;
                    }
                    let mut nested_frames = scope_frames.clone();
                    self.validate_flow_operation_locals_inner(
                        body,
                        &mut nested,
                        used,
                        &mut nested_frames,
                    )?;
                }
                FlowOp::For {
                    pattern,
                    source,
                    body,
                    ..
                } => {
                    self.validate_expression_locals(source, scope, used)?;
                    let mut nested = extend_scope(
                        scope,
                        pattern
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                    let mut nested_frames = scope_frames.clone();
                    self.validate_flow_operation_locals_inner(
                        body,
                        &mut nested,
                        used,
                        &mut nested_frames,
                    )?;
                }
                FlowOp::Break(value) => {
                    if let Some(value) = value {
                        self.validate_expression_locals(value, scope, used)?;
                    }
                }
                FlowOp::GotoExpr(target) | FlowOp::ReturnExpr(target) => {
                    self.validate_expression_locals(target, scope, used)?;
                }
                FlowOp::Effect(effect) | FlowOp::RegisterCleanup { effect, .. } => {
                    self.validate_line_effect_locals(std::slice::from_ref(effect), scope, used)?;
                }
                FlowOp::EvaluatedEffect(effect) => {
                    for expression in effect.argument_exprs() {
                        self.validate_expression_locals(expression, scope, used)?;
                    }
                }
                FlowOp::RegisterDefer { captures, .. } => {
                    for capture in captures {
                        self.validate_expression_locals(capture, scope, used)?;
                    }
                }
                FlowOp::Continue
                | FlowOp::Goto(_)
                | FlowOp::Return(_)
                | FlowOp::CancelCleanup { .. }
                | FlowOp::Noop => {}
                FlowOp::EnterScope { .. } => scope_frames.push(scope.clone()),
                FlowOp::ExitScope => {
                    *scope =
                        scope_frames
                            .pop()
                            .ok_or(RuntimePlanBuildError::FlowScopeUnderflow {
                                operation: "ExitScope",
                            })?;
                }
                FlowOp::ExitScopeBind { pattern, expr } => {
                    self.validate_expression_locals(expr, scope, used)?;
                    let parent =
                        scope_frames
                            .pop()
                            .ok_or(RuntimePlanBuildError::FlowScopeUnderflow {
                                operation: "ExitScopeBind",
                            })?;
                    *scope = extend_scope(
                        &parent,
                        pattern
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                }
                FlowOp::Dialogue {
                    target,
                    content,
                    result,
                } => {
                    self.validate_expression_locals(target, scope, used)?;
                    if self.dialogue_content.get(*content).is_none() {
                        return Err(RuntimePlanBuildError::ForeignDialogueContentSeed);
                    }
                    *scope = extend_scope(
                        scope,
                        result
                            .pattern()
                            .binding_declarations()
                            .map(RuntimePatternBindingDeclaration::local),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn validate_task_request_locals(
        &self,
        request: &HostTaskRequestTemplate,
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        self.validate_host_argument_locals(&request.args, scope, used)
    }

    fn validate_host_argument_locals(
        &self,
        arguments: &[RuntimeHostArgumentTemplate],
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        for argument in arguments {
            let value = match argument {
                RuntimeHostArgumentTemplate::Positional(_, value)
                | RuntimeHostArgumentTemplate::Spread(_, value) => value,
                RuntimeHostArgumentTemplate::Named(_, argument) => &argument.value,
            };
            self.validate_expression_locals(value, scope, used)?;
        }
        Ok(())
    }

    fn validate_line_effect_locals(
        &self,
        effects: &[LineEffectRequest],
        scope: &BTreeSet<RuntimeLocalDeclarationId>,
        used: &mut BTreeSet<RuntimeLocalDeclarationId>,
    ) -> Result<(), RuntimePlanBuildError> {
        for effect in effects {
            if let LineEffectRequest::Audio(command) = effect {
                for expression in command.argument_exprs() {
                    self.validate_expression_locals(expression, scope, used)?;
                }
            }
        }
        Ok(())
    }
}

fn require_local_in_scope(
    local: RuntimeLocalDeclarationId,
    scope: &BTreeSet<RuntimeLocalDeclarationId>,
) -> Result<(), RuntimePlanBuildError> {
    if scope.contains(&local) {
        Ok(())
    } else {
        Err(RuntimePlanBuildError::UnreachableFunctionLocal { local })
    }
}

fn extend_scope(
    scope: &BTreeSet<RuntimeLocalDeclarationId>,
    locals: impl IntoIterator<Item = RuntimeLocalDeclarationId>,
) -> Result<BTreeSet<RuntimeLocalDeclarationId>, RuntimePlanBuildError> {
    let mut nested = scope.clone();
    for local in locals {
        if !nested.insert(local) {
            return Err(RuntimePlanBuildError::DuplicateFunctionLocal { local });
        }
    }
    Ok(nested)
}

pub(super) fn function_input_scope(
    inputs: &[RuntimeFunctionInputBinding],
) -> BTreeSet<RuntimeLocalDeclarationId> {
    let mut scope = BTreeSet::new();
    for input in inputs {
        // The input local names the ABI carrier. Only locals actually bound by
        // its pattern are live in the function body; binding the carrier too
        // would duplicate an affine destructured value.
        scope.extend(
            input
                .pattern()
                .binding_declarations()
                .map(RuntimePatternBindingDeclaration::local),
        );
    }
    scope
}

impl RuntimePlanBodyConstruction<'_> {
    fn validate_plan_value(
        &self,
        context: &'static str,
        ty: RuntimePlanTypeId,
        value: &RuntimeValue,
    ) -> Result<(), RuntimePlanBuildError> {
        if crate::value::visit_runtime_value_graph(value, |node| {
            if matches!(node, RuntimeValue::NeedHandle(_)) {
                Err(())
            } else {
                Ok(())
            }
        })
        .is_err()
        {
            return Err(RuntimePlanBuildError::InvalidValueType { context, ty });
        }
        if value.contains_function() {
            return Err(RuntimePlanBuildError::FunctionValueInPlan { context });
        }
        if value.contains_nonconstant_opaque() {
            return Err(RuntimePlanBuildError::NonConstantOpaqueValueInPlan { context });
        }
        if !value.ownership().permits_copy() {
            return Err(RuntimePlanBuildError::AffineLiteralInPlan { context });
        }
        let authority = super::super::value_admission::PlanValueAuthority::Building {
            types: &self.types,
            records: &self.nominal_record_domains,
            variants: &self.variant_domains,
        };
        let limits = crate::entry::RuntimeSchemaLimits {
            max_depth: u32::try_from(crate::value::MAX_RUNTIME_VALUE_NESTING_DEPTH)
                .expect("runtime value nesting limit fits the schema allowance"),
            ..crate::entry::RuntimeSchemaLimits::engine_default()
        };
        authority
            .validate_literal(ty, value, limits)
            .map_err(|source| match source {
                super::super::RuntimePlanValueAdmissionError::UnknownType { ty } => {
                    RuntimePlanBuildError::InvalidTypeProjection { context, ty }
                }
                super::super::RuntimePlanValueAdmissionError::Value { ref source, .. }
                    if source.is_choice_mismatch() =>
                {
                    RuntimePlanBuildError::InvalidValueType { context, ty }
                }
                source => RuntimePlanBuildError::ValueAdmission {
                    context,
                    source: Box::new(source),
                },
            })
    }
    fn variant_case(
        &self,
        owner: RuntimePlanTypeId,
        ordinal: u32,
    ) -> Result<super::super::RuntimePlanVariantCase<'_>, RuntimePlanBuildError> {
        let declaration = self
            .types
            .get(owner)
            .ok_or(super::super::RuntimePlanVariantCaseError::UnknownType { ty: owner })?;
        Ok(declaration.select_variant_case(owner, self.variant_domains.get(owner), ordinal)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::{
        RuntimeNominalSchemaBody, RuntimeNominalSchemaCase, RuntimeNominalSchemaDefinition,
        RuntimeNominalSchemaField, RuntimeNominalSchemaGraph, RuntimeNominalSchemaIdentity,
        RuntimeNominalTypeId, RuntimeSchemaLimits, RuntimeTypeSchema as Schema,
    };
    use crate::pattern::{
        RuntimeOpaqueTypeProducerId, RuntimePatternBindingPathError, RuntimePatternBindingStep,
        RuntimePatternKind, RuntimeSemanticTypeId,
    };
    use crate::plan::{
        RuntimeLocalDeclarationSeed, RuntimeNominalRecordDomainFieldSeed,
        RuntimeNominalRecordDomainSeed, RuntimePlanTypeSeed, RuntimeRecordPatternFieldSeed,
        RuntimeVariantCaseSeed, RuntimeVariantDomainSeed,
    };
    use crate::value::{RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValueClass};

    fn identity(marker: u8) -> RuntimeSemanticTypeId {
        RuntimeSemanticTypeId::from_bytes([marker; 32])
    }

    fn nominal(label: &str) -> RuntimeNominalTypeId {
        RuntimeNominalTypeId::try_new(label).expect("test nominal")
    }

    #[test]
    fn opaque_record_projection_retains_the_complete_exact_owner() {
        let producer =
            RuntimeOpaqueTypeProducerId::try_new("fixture.dialogue-view").expect("test producer");
        let semantic_owner = identity(91);
        let field_type = identity(92);
        let mut builder = RuntimePlanBuilder::new();
        let admission = builder
            .admit_type_batch(
                [
                    RuntimePlanTypeSeed::new(
                        semantic_owner,
                        RuntimePlanTypeProjection::Opaque {
                            producer: producer.clone(),
                            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                            value_class: RuntimeOpaqueValueClass::AffineHandle(
                                RuntimeHandleKind::StageActor,
                            ),
                            persistence: RuntimeOpaquePersistence::SnapshotOnly,
                            arguments: Box::new([]),
                        },
                    ),
                    RuntimePlanTypeSeed::new(field_type, RuntimePlanTypeProjection::String),
                ],
                [RuntimeLocalDeclarationSeed::new(manual_local_source("arcweft-core.fixture.plan.construction.lower.opaque_record_projection_retains_the_complete_exact_owner.binding_a"), semantic_owner)],
            )
            .expect("opaque record graph");
        let expression = builder
            .lower_expression(RuntimeExprSeed::new(
                field_type,
                RuntimeExprSeedKind::Field {
                    target: Box::new(RuntimeExprSeed::new(
                        semantic_owner,
                        RuntimeExprSeedKind::Local(crate::plan::RuntimeLocalReadSeed::new(
                            admission.local_ids()[0].clone(),
                            crate::value::RuntimeLocalReadMode::Move,
                        )),
                    ))
                    .into(),
                    field: RuntimeFieldProjectionSeed::OpaqueRecord {
                        owner: semantic_owner,
                        producer: producer.clone(),
                        field: RuntimeRecordFieldSeedId::from_zero_based(0),
                        field_type,
                    },
                },
            ))
            .expect("opaque record projection");

        let RuntimeExprKind::Field {
            field: RuntimeFieldProjection::OpaqueRecord { owner, field },
            ..
        } = expression.kind()
        else {
            panic!("opaque record field expression");
        };
        assert_eq!(field.zero_based(), 0);
        assert_eq!(owner.producer(), &producer);
        assert_eq!(owner.semantic_identity(), semantic_owner);
        assert_eq!(owner.admission(), RuntimeOpaqueTypeAdmission::ExactIdentity);
        assert_eq!(
            owner.value_class(),
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::StageActor)
        );
        assert_eq!(owner.persistence(), RuntimeOpaquePersistence::SnapshotOnly);
    }

    #[test]
    fn record_binding_path_uses_retained_pattern_order_not_domain_ordinal() {
        let schema = RuntimeNominalSchemaGraph::try_new(
            vec![RuntimeNominalSchemaDefinition::new(
                crate::entry::RuntimeNominalDeclarationId::from_bytes(
                    *(RuntimeNominalSchemaIdentity::new(nominal("game.Pair"), identity(1)))
                        .semantic_identity()
                        .as_bytes(),
                ),
                RuntimeNominalSchemaIdentity::new(nominal("game.Pair"), identity(1)),
                vec![],
                RuntimeNominalSchemaBody::Record {
                    shape: crate::entry::RuntimeNominalRecordShape::Record,
                    fields: ["first", "second"]
                        .into_iter()
                        .enumerate()
                        .map(|(ordinal, name)| {
                            RuntimeNominalSchemaField::new(
                                RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal).unwrap(),
                                Some(name.to_owned()),
                                Schema::Bool,
                            )
                        })
                        .collect(),
                },
            )],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
        let mut builder = RuntimePlanBuilder::new();
        let admission = builder
            .admit_semantic_batch(
                [
                    RuntimePlanTypeSeed::new(
                        identity(1),
                        RuntimePlanTypeProjection::Nominal {
                            nominal: nominal("game.Pair"),
                            layout: schema.try_layout_hash(identity(1)).unwrap(),
                            arguments: Box::new([]),
                        },
                    ),
                    RuntimePlanTypeSeed::new(identity(2), RuntimePlanTypeProjection::Bool),
                ],
                [RuntimeLocalDeclarationSeed::new(manual_local_source("arcweft-core.fixture.plan.construction.lower.record_binding_path_uses_retained_pattern_order_not_domain_ordinal.binding_a"), identity(2))],
                [RuntimeNominalRecordDomainSeed::new(
                    identity(1),
                    crate::entry::RuntimeNominalRecordShape::Record,
                    [
                        RuntimeNominalRecordDomainFieldSeed::new(
                            crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(0)
                                .unwrap(),
                            Some("first".to_owned()),
                            identity(2),
                        ),
                        RuntimeNominalRecordDomainFieldSeed::new(
                            crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(1)
                                .unwrap(),
                            Some("second".to_owned()),
                            identity(2),
                        ),
                    ],
                )],
                [],
                &schema,
            )
            .expect("record graph");
        let pattern = RuntimePatternSeed::new(
            identity(1),
            RuntimePatternSeedKind::Record {
                fields: Box::new([RuntimeRecordPatternFieldSeed::new(
                    RuntimeRecordFieldSeedId::from_zero_based(1),
                    RuntimePatternSeed::new(
                        identity(2),
                        RuntimePatternSeedKind::Bind {
                            mutable: false,
                            local: admission.local_ids()[0].clone(),
                        },
                    ),
                )]),
                rest: RuntimePatternRestSeed::Ignore,
            },
        );

        let admitted = builder
            .lower_pattern_seed(pattern)
            .expect("admitted record pattern");
        let RuntimePatternKind::Record { fields, .. } = admitted.kind() else {
            panic!("record pattern kind");
        };
        assert_eq!(fields[0].field().zero_based(), 1);
        let RuntimePatternKind::Bind { binding, .. } = fields[0].pattern().kind() else {
            panic!("field bind pattern");
        };
        assert_eq!(
            binding.path().steps(),
            [RuntimePatternBindingStep::RecordField(0)]
        );
    }

    #[test]
    fn binding_path_accepts_depth_64_and_rejects_depth_65() {
        let mut builder = RuntimePlanBuilder::new();
        let owner = nominal("game.Recursive");
        let schema = RuntimeNominalSchemaGraph::try_new(
            vec![RuntimeNominalSchemaDefinition::new(
                crate::entry::RuntimeNominalDeclarationId::from_bytes(
                    *(RuntimeNominalSchemaIdentity::new(owner.clone(), identity(1)))
                        .semantic_identity()
                        .as_bytes(),
                ),
                RuntimeNominalSchemaIdentity::new(owner.clone(), identity(1)),
                vec![],
                RuntimeNominalSchemaBody::Variant {
                    cases: vec![RuntimeNominalSchemaCase::new(
                        0,
                        "Next".to_owned(),
                        Some(Schema::NominalRef(RuntimeNominalSchemaIdentity::new(
                            owner.clone(),
                            identity(1),
                        ))),
                    )]
                    .into_boxed_slice(),
                },
            )],
            RuntimeSchemaLimits::engine_default(),
        )
        .unwrap();
        let layout = schema.try_layout_hash(identity(1)).unwrap();
        let admission = builder
            .admit_semantic_batch(
                [RuntimePlanTypeSeed::new(
                    identity(1),
                    RuntimePlanTypeProjection::Nominal {
                        nominal: owner.clone(),
                        layout,
                        arguments: Box::new([]),
                    },
                )],
                [RuntimeLocalDeclarationSeed::new(manual_local_source("arcweft-core.fixture.plan.construction.lower.binding_path_accepts_depth_64_and_rejects_depth_65.binding_a"), identity(1))],
                [],
                [RuntimeVariantDomainSeed::new(
                    identity(1),
                    owner,
                    layout,
                    [RuntimeVariantCaseSeed::new("Next", Some(identity(1)))],
                )],
                &schema,
            )
            .expect("recursive variant domain");
        let local = admission.local_ids()[0].clone();

        assert!(
            builder
                .lower_pattern_seed(nested_variant(64, local.clone()))
                .is_ok()
        );
        assert_eq!(
            builder.lower_pattern_seed(nested_variant(65, local)),
            Err(RuntimePlanBuildError::PatternBindingPath(
                RuntimePatternBindingPathError::TooDeep {
                    actual: 65,
                    maximum: 64,
                }
            ))
        );
    }

    fn nested_variant(depth: usize, local: RuntimeLocalSeedId) -> RuntimePatternSeed {
        let mut pattern = RuntimePatternSeed::new(
            identity(1),
            RuntimePatternSeedKind::Bind {
                mutable: false,
                local,
            },
        );
        for _ in 0..depth {
            pattern = RuntimePatternSeed::new(
                identity(1),
                RuntimePatternSeedKind::Variant {
                    ordinal: 0,
                    payload: Some(Box::new(pattern)),
                },
            );
        }
        pattern
    }
}

#[cfg(test)]
fn manual_local_source(declaration: &str) -> crate::plan::RuntimeLocalDeclarationSource {
    // This fixture declares a semantic binding name independent of its value,
    // type, source offset, and builder-issued local ordinal.
    let mut identity = blake3::Hasher::new();
    identity.update(b"arcweft.manual-fixture-binding.v1\0");
    identity.update(declaration.as_bytes());
    crate::plan::RuntimeLocalDeclarationSource::Binding {
        identity: *identity.finalize().as_bytes(),
        declaration: crate::plan::RuntimeLocalBindingDeclaration::new(
            crate::plan::RuntimeLocalBindingKind::PatternBinding,
            false,
            crate::plan::RuntimeLocalBindingStorage::Derived,
        ),
    }
}
