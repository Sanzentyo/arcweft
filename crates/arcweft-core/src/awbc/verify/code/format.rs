use super::super::AwbcVerifyError;
use super::super::structure::{Verifier, check_index, checked_range};
use super::{
    FlowState, argument_count, check_args_budget, invalid_type, is_exact_dialogue_content_type,
    read_register, register_type, runtime_shape, write_register,
};
use crate::awbc::schema::{
    AwbcDialogueValueRole, AwbcFormatAttemptOperand, AwbcFunctionFlag, AwbcFunctionKind,
    AwbcInstruction, AwbcProgram, AwbcRuntimeTypeShape, AwbcTraitReceiverMode, AwbcTypeId,
};
use crate::pattern::RuntimeBuiltinVariantCaseIdentity;
use crate::runtime_id::RuntimeFormatAttemptId;
use crate::value::RuntimeFmtParameterId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FormatAttemptFlowState {
    id: RuntimeFormatAttemptId,
    next_operand: usize,
    active: Option<RuntimeFmtParameterId>,
}

#[derive(Clone, Copy)]
pub(super) struct FormatAttemptManifest<'a> {
    owner: usize,
    operands: &'a [AwbcFormatAttemptOperand],
}

pub(super) type FormatAttemptCatalog<'a> =
    BTreeMap<RuntimeFormatAttemptId, FormatAttemptManifest<'a>>;

pub(super) fn format_attempt_catalog(
    program: &AwbcProgram,
) -> Result<FormatAttemptCatalog<'_>, AwbcVerifyError> {
    let mut attempts = BTreeMap::new();
    for (owner, function) in program.functions.iter().enumerate() {
        let blocks = checked_range(
            function.blocks,
            program.blocks.len(),
            "blocks",
            &format!("function {owner}"),
        )?;
        for block in blocks {
            let instructions = checked_range(
                program.blocks[block].instructions,
                program.instructions.len(),
                "instructions",
                &format!("block {block}"),
            )?;
            for instruction in instructions {
                if let AwbcInstruction::FormatContent {
                    attempt: Some(id),
                    attempt_operands,
                    ..
                } = &program.instructions[instruction]
                {
                    if attempts
                        .insert(
                            *id,
                            FormatAttemptManifest {
                                owner,
                                operands: attempt_operands,
                            },
                        )
                        .is_some()
                    {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: format!("instruction {instruction}"),
                            message: "format attempt has more than one final FormatContent owner"
                                .to_owned(),
                        });
                    }
                }
            }
        }
    }
    Ok(attempts)
}

pub(super) fn apply_instruction(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    instruction_index: usize,
    state: &mut FlowState,
    attempts: &FormatAttemptCatalog<'_>,
) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    let instruction = &program.instructions[instruction_index];
    let at = format!("instruction {instruction_index}");
    match instruction {
        AwbcInstruction::FormatOperandAttempt { attempt, parameter } => {
            let Some(manifest) = attempts.get(attempt) else {
                return invalid_type(&at, "formatter begin has no final FormatContent owner");
            };
            if manifest.owner != function || manifest.operands.is_empty() {
                return invalid_type(&at, "formatter begin has a foreign or empty manifest");
            }
            if state
                .format_attempts
                .last()
                .is_none_or(|current| current.id != *attempt)
            {
                if state
                    .format_attempts
                    .iter()
                    .any(|current| current.id == *attempt)
                {
                    return invalid_type(&at, "formatter attempt cannot nest within itself");
                }
                state.format_attempts.push(FormatAttemptFlowState {
                    id: *attempt,
                    next_operand: 0,
                    active: None,
                });
            }
            let current = state
                .format_attempts
                .last_mut()
                .expect("formatter begin pushed state");
            if current.active.is_some()
                || manifest
                    .operands
                    .get(current.next_operand)
                    .is_none_or(|operand| operand.parameter != *parameter)
            {
                return invalid_type(&at, "formatter begins operands out of manifest order");
            }
            current.active = Some(*parameter);
        }
        AwbcInstruction::CompleteFormatOperand {
            attempt,
            parameter,
            value,
        } => {
            let Some(manifest) = attempts.get(attempt) else {
                return invalid_type(&at, "formatter completion has no final owner");
            };
            if manifest.owner != function {
                return invalid_type(&at, "formatter completion belongs to another function");
            }
            let Some(current) = state.format_attempts.last() else {
                return invalid_type(&at, "formatter completion has no active attempt");
            };
            let Some(expected) = manifest.operands.get(current.next_operand) else {
                return invalid_type(&at, "formatter completion exceeds its manifest");
            };
            if current.id != *attempt
                || current.active != Some(*parameter)
                || expected.parameter != *parameter
            {
                return invalid_type(&at, "formatter completion does not match its begin");
            }
            let actual = read_register(verifier, function, block, *value, state)?;
            if actual != expected.ty {
                return invalid_type(&at, "formatter operand value has wrong type");
            }
            let current = state
                .format_attempts
                .last_mut()
                .expect("checked attempt remains");
            current.active = None;
            current.next_operand += 1;
        }
        AwbcInstruction::AbandonFormatAttempt { attempt } => {
            let Some(manifest) = attempts.get(attempt) else {
                return invalid_type(&at, "formatter abandon has no final owner");
            };
            if manifest.owner != function
                || state
                    .format_attempts
                    .last()
                    .is_none_or(|current| current.id != *attempt)
            {
                return invalid_type(&at, "formatter abandon does not match active attempt");
            }
            state.format_attempts.pop();
        }
        AwbcInstruction::FormatContent {
            destination,
            template,
            attempt,
            attempt_operands,
            project_method,
            project_option,
            project_result,
            operands,
        } => {
            match attempt {
                Some(id) => {
                    if !operands.is_empty()
                        || attempts
                            .get(id)
                            .is_none_or(|manifest| manifest.owner != function)
                    {
                        return invalid_type(
                            &at,
                            "FormatContent attempt must own an exact same-function manifest and no inline operands",
                        );
                    }
                }
                None if !attempt_operands.is_empty() => {
                    return invalid_type(
                        &at,
                        "inline FormatContent cannot carry a Flow attempt manifest",
                    );
                }
                None => {}
            }
            let capture_count = operands.iter().try_fold(0_usize, |total, operand| {
                total.checked_add(operand.captures.len()).ok_or_else(|| {
                    AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "FormatContent capture register count overflows usize".to_owned(),
                    }
                })
            })?;
            let total_argument_count = operands
                .len()
                .checked_add(attempt_operands.len())
                .and_then(|count| count.checked_add(capture_count))
                .and_then(|count| count.checked_add(if project_method.is_some() { 2 } else { 0 }))
                .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "FormatContent operand register count overflows usize".to_owned(),
                })?;
            check_args_budget(verifier, total_argument_count)?;

            let project_signature = match (project_method, project_result) {
                (None, None) if !project_option => None,
                (Some(method_id), Some(result_register)) => {
                    check_index(
                        program.trait_methods.len(),
                        method_id.0,
                        "trait_methods",
                        &at,
                    )?;
                    let method = &program.trait_methods[method_id.index()];
                    if method.receiver != AwbcTraitReceiverMode::Owned
                        || method.receiver_state_slot.is_some()
                    {
                        return invalid_type(
                            &at,
                            "FormatContent DisplayText method requires an owned receiver",
                        );
                    }
                    check_index(program.functions.len(), method.function.0, "functions", &at)?;
                    let target = &program.functions[method.function.index()];
                    if target.kind != AwbcFunctionKind::TraitMethod
                        || !target.flags.contains(AwbcFunctionFlag::Deterministic)
                        || target.flags.contains(AwbcFunctionFlag::MaySuspend)
                        || target.signature != method.signature
                    {
                        return invalid_type(
                            &at,
                            "FormatContent DisplayText method must be a deterministic, non-suspending trait method",
                        );
                    }
                    check_index(
                        program.signatures.len(),
                        method.signature.0,
                        "signatures",
                        &at,
                    )?;
                    let signature = &program.signatures[method.signature.index()];
                    if signature.params.len() != 2 {
                        return argument_count(&at, 2, signature.params.len());
                    }
                    check_index(
                        program.effect_sets.len(),
                        signature.effects.0,
                        "effect_sets",
                        &at,
                    )?;
                    if !program.effect_sets[signature.effects.index()]
                        .effects
                        .is_empty()
                    {
                        return invalid_type(
                            &at,
                            "FormatContent DisplayText method must be effect-free",
                        );
                    }
                    let Some(result) = signature.result else {
                        return invalid_type(
                            &at,
                            "FormatContent DisplayText method must return Result<Content, DisplayError>",
                        );
                    };
                    if !is_display_context_type(program, signature.params[1]) {
                        return invalid_type(&at, "FormatContent DisplayContext parameter");
                    }
                    let ok = program
                        .builtin_variant_payload_item(
                            result,
                            crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultOk,
                        )
                        .is_some_and(|ty| is_exact_dialogue_content_type(program, ty));
                    let error = program.builtin_variant_payload_item(
                        result,
                        crate::pattern::RuntimeBuiltinVariantCaseIdentity::ResultErr,
                    );
                    if !ok || !error.is_some_and(|ty| is_display_error_type(program, ty)) {
                        return invalid_type(
                            &at,
                            "FormatContent DisplayText result must be Result<Content, DisplayError>",
                        );
                    }
                    let result_register_type =
                        register_type(verifier, function, block, *result_register)?;
                    if result_register_type != result
                        || *result_register == *destination
                        || !state.initialized[result_register.index()].is_uninitialized()
                    {
                        return invalid_type(&at, "FormatContent project result temporary");
                    }
                    Some((signature.params[0], result_register_type))
                }
                _ => {
                    return invalid_type(
                        &at,
                        "FormatContent project method, option mode, and result temporary disagree",
                    );
                }
            };

            let Some(template) = program
                .content_templates
                .iter()
                .find(|candidate| candidate.id == *template)
            else {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "FormatContent references a missing template manifest".to_owned(),
                });
            };
            if !matches!(
                template.slots.as_slice(),
                [slot]
                    if slot.role == AwbcDialogueValueRole::Formatted
                        && is_exact_dialogue_content_type(program, slot.semantic_type)
            ) || !template.effects.is_empty()
            {
                return invalid_type(
                    &at,
                    "FormatContent requires one exact Formatted/Content slot and no effects",
                );
            }

            let mut seen = BTreeSet::new();
            let mut value_present = false;
            let mut value_result_type = None;
            let mut failure_policy_count = 0_u8;
            for operand in attempt_operands {
                if !seen.insert(operand.parameter) {
                    return invalid_type(&at, "FormatContent parameter identities must be unique");
                }
                if operand.parameter == RuntimeFmtParameterId::Value {
                    value_present = true;
                    value_result_type = Some(operand.ty);
                }
                if matches!(
                    operand.parameter,
                    RuntimeFmtParameterId::OnError
                        | RuntimeFmtParameterId::Fallback
                        | RuntimeFmtParameterId::DiscardError
                ) {
                    failure_policy_count += 1;
                    if failure_policy_count > 1 {
                        return invalid_type(
                            &at,
                            "FormatContent has multiple mutually exclusive failure policies",
                        );
                    }
                }
                check_index(
                    program.runtime_types.len(),
                    operand.ty.0,
                    "runtime_types",
                    &at,
                )?;
                if !(operand.parameter == RuntimeFmtParameterId::Value
                    && project_signature.is_some())
                    && !format_operand_result_is_valid(program, operand.parameter, operand.ty)
                {
                    return invalid_type(
                        &at,
                        "FormatContent Flow operand type is not admitted by its parameter",
                    );
                }
            }
            for operand in operands {
                if !seen.insert(operand.parameter) {
                    return invalid_type(&at, "FormatContent parameter identities must be unique");
                }
                if operand.parameter == RuntimeFmtParameterId::Value {
                    value_present = true;
                }
                if matches!(
                    operand.parameter,
                    RuntimeFmtParameterId::OnError
                        | RuntimeFmtParameterId::Fallback
                        | RuntimeFmtParameterId::DiscardError
                ) {
                    failure_policy_count += 1;
                    if failure_policy_count > 1 {
                        return invalid_type(
                            &at,
                            "FormatContent has multiple mutually exclusive failure policies",
                        );
                    }
                }

                check_index(
                    program.functions.len(),
                    operand.function.0,
                    "functions",
                    &at,
                )?;
                let target = &program.functions[operand.function.index()];
                if target.kind != AwbcFunctionKind::Synthetic
                    || !target.flags.contains(AwbcFunctionFlag::Deterministic)
                    || target.flags.contains(AwbcFunctionFlag::MaySuspend)
                {
                    return invalid_type(
                        &at,
                        "FormatContent operand must target a deterministic, non-suspending Synthetic function",
                    );
                }
                check_index(
                    program.signatures.len(),
                    target.signature.0,
                    "signatures",
                    &at,
                )?;
                let signature = &program.signatures[target.signature.index()];
                check_index(
                    program.effect_sets.len(),
                    signature.effects.0,
                    "effect_sets",
                    &at,
                )?;
                if !program.effect_sets[signature.effects.index()]
                    .effects
                    .is_empty()
                {
                    return invalid_type(&at, "FormatContent operand function must be effect-free");
                }
                if signature.params.len() != operand.captures.len() {
                    return argument_count(&at, signature.params.len(), operand.captures.len());
                }
                let capture_types = operand
                    .captures
                    .iter()
                    .map(|capture| read_register(verifier, function, block, *capture, state))
                    .collect::<Result<Vec<_>, _>>()?;
                if signature.params != capture_types {
                    return invalid_type(
                        &at,
                        "FormatContent operand capture types must exactly match its function signature",
                    );
                }
                let Some(result) = signature.result else {
                    return invalid_type(&at, "FormatContent operand function must return a value");
                };
                if operand.parameter == RuntimeFmtParameterId::Value {
                    value_result_type = Some(result);
                }
                if !(operand.parameter == RuntimeFmtParameterId::Value
                    && project_signature.is_some())
                    && !format_operand_result_is_valid(program, operand.parameter, result)
                {
                    return invalid_type(
                        &at,
                        "FormatContent operand result type is not admitted by its parameter",
                    );
                }
            }
            if !value_present {
                return invalid_type(&at, "FormatContent requires the Value parameter");
            }
            if let Some(id) = attempt {
                let Some(current) = state.format_attempts.last() else {
                    return invalid_type(&at, "FormatContent has no active Flow attempt");
                };
                if current.id != *id
                    || current.active.is_some()
                    || current.next_operand != attempt_operands.len()
                {
                    return invalid_type(&at, "FormatContent Flow attempt is incomplete");
                }
                state.format_attempts.pop();
            }
            if let Some((receiver_type, _)) = project_signature {
                let value_type =
                    value_result_type.ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "FormatContent selected project method has no Value result type"
                            .to_owned(),
                    })?;
                let receiver_matches = if *project_option {
                    program.builtin_variant_payload_item(
                        value_type,
                        crate::pattern::RuntimeBuiltinVariantCaseIdentity::OptionSome,
                    ) == Some(receiver_type)
                } else {
                    value_type == receiver_type
                };
                if !receiver_matches {
                    return invalid_type(
                        &at,
                        "FormatContent Value type does not match the selected DisplayText receiver",
                    );
                }
            }

            let destination_type = register_type(verifier, function, block, *destination)?;
            if !is_exact_dialogue_content_type(program, destination_type) {
                return invalid_type(&at, "FormatContent destination");
            }
            write_register(verifier, function, block, *destination, state)?;
        }
        _ => unreachable!("format verifier dispatched a non-format instruction"),
    }
    Ok(())
}

fn nominal_record_fields_named<'a>(
    program: &'a AwbcProgram,
    ty: AwbcTypeId,
    expected_public_id: &str,
) -> Option<&'a [crate::awbc::schema::AwbcRecordField]> {
    let AwbcRuntimeTypeShape::NominalRecord {
        public_id,
        arguments,
        shape,
        fields,
        ..
    } = runtime_shape(program, ty)?
    else {
        return None;
    };
    (program
        .strings
        .get(public_id.index())
        .is_some_and(|id| id == expected_public_id)
        && arguments.is_empty()
        && *shape == crate::entry::RuntimeNominalRecordShape::Record)
        .then_some(fields)
}

fn record_field_named(
    program: &AwbcProgram,
    field: &crate::awbc::schema::AwbcRecordField,
    expected_name: &str,
) -> bool {
    field
        .name
        .and_then(|name| program.strings.get(name.index()))
        .is_some_and(|name| name == expected_name)
}

fn is_display_context_type(program: &AwbcProgram, ty: AwbcTypeId) -> bool {
    let Some(fields) = nominal_record_fields_named(program, ty, "standard::DisplayContext") else {
        return false;
    };
    let [locale, style, currency] = fields else {
        return false;
    };
    record_field_named(program, locale, "locale")
        && record_field_named(program, style, "style")
        && record_field_named(program, currency, "currency")
        && matches!(
            runtime_shape(program, locale.ty),
            Some(AwbcRuntimeTypeShape::String)
        )
        && program
            .builtin_variant_payload_item(style.ty, RuntimeBuiltinVariantCaseIdentity::OptionSome)
            == Some(locale.ty)
        && program.builtin_variant_payload_item(
            currency.ty,
            RuntimeBuiltinVariantCaseIdentity::OptionSome,
        ) == Some(locale.ty)
}

fn is_display_error_type(program: &AwbcProgram, ty: AwbcTypeId) -> bool {
    let Some(fields) = nominal_record_fields_named(program, ty, "standard::DisplayError") else {
        return false;
    };
    let [message] = fields else {
        return false;
    };
    record_field_named(program, message, "message")
        && matches!(
            runtime_shape(program, message.ty),
            Some(AwbcRuntimeTypeShape::String)
        )
}

fn format_operand_result_is_valid(
    program: &AwbcProgram,
    parameter: RuntimeFmtParameterId,
    ty: AwbcTypeId,
) -> bool {
    let shape = runtime_shape(program, ty);
    let inline_scalar = matches!(
        shape,
        Some(
            AwbcRuntimeTypeShape::Unit
                | AwbcRuntimeTypeShape::Bool
                | AwbcRuntimeTypeShape::Int(_)
                | AwbcRuntimeTypeShape::UInt(_)
                | AwbcRuntimeTypeShape::F32
                | AwbcRuntimeTypeShape::F64
                | AwbcRuntimeTypeShape::String
                | AwbcRuntimeTypeShape::Char
                | AwbcRuntimeTypeShape::Duration
                | AwbcRuntimeTypeShape::EntityRef
                | AwbcRuntimeTypeShape::Progress
        )
    );

    match parameter {
        RuntimeFmtParameterId::Value => {
            inline_scalar
                || is_exact_dialogue_content_type(program, ty)
                || program
                    .builtin_variant_payload_item(ty, RuntimeBuiltinVariantCaseIdentity::OptionSome)
                    .is_some_and(|item| {
                        matches!(
                            runtime_shape(program, item),
                            Some(
                                AwbcRuntimeTypeShape::Unit
                                    | AwbcRuntimeTypeShape::Bool
                                    | AwbcRuntimeTypeShape::Int(_)
                                    | AwbcRuntimeTypeShape::UInt(_)
                                    | AwbcRuntimeTypeShape::F32
                                    | AwbcRuntimeTypeShape::F64
                                    | AwbcRuntimeTypeShape::String
                                    | AwbcRuntimeTypeShape::Char
                                    | AwbcRuntimeTypeShape::Duration
                                    | AwbcRuntimeTypeShape::EntityRef
                                    | AwbcRuntimeTypeShape::Progress
                            )
                        )
                    })
        }
        RuntimeFmtParameterId::Style
        | RuntimeFmtParameterId::Locale
        | RuntimeFmtParameterId::Currency
        | RuntimeFmtParameterId::NoneValue
        | RuntimeFmtParameterId::Fallback => {
            matches!(shape, Some(AwbcRuntimeTypeShape::String))
        }
        RuntimeFmtParameterId::Color => matches!(shape, Some(AwbcRuntimeTypeShape::Color)),
        RuntimeFmtParameterId::OnError => matches!(
            shape,
            Some(AwbcRuntimeTypeShape::Variant {
                owner: crate::awbc::schema::AwbcVariantIdentity::Nominal { .. },
                arguments,
                cases,
            }) if arguments.is_empty() && !cases.is_empty()
        ),
        RuntimeFmtParameterId::DiscardError => {
            matches!(shape, Some(AwbcRuntimeTypeShape::Bool))
        }
    }
}
