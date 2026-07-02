#![allow(
    clippy::too_many_lines,
    reason = "AWBC structural verification checks many stable table families in one pass"
)]

use super::{AwbcVerifyBudget, AwbcVerifyContext, AwbcVerifyError};
use crate::awbc::schema::{
    AWBC_ABI_VERSION, AwbcAudioCommandId, AwbcAudioValueRef, AwbcBlockId, AwbcCodeLocation,
    AwbcConstant, AwbcEffectKind, AwbcEffectPlan, AwbcEffectSetId, AwbcEntryKind, AwbcEntryTarget,
    AwbcFrameSlotRole, AwbcFunctionId, AwbcFunctionKind, AwbcLineTaskNode, AwbcLineTaskTrigger,
    AwbcPattern, AwbcPatternId, AwbcProgram, AwbcRouteBindingSource, AwbcRuntimeType,
    AwbcSignatureId, AwbcStringId, AwbcTableRange, AwbcTypeId,
};
use std::collections::BTreeSet;

pub(super) struct Verifier<'program, 'context> {
    pub(super) program: &'program AwbcProgram,
    pub(super) budget: AwbcVerifyBudget,
    pub(super) context: AwbcVerifyContext<'context>,
    pub(super) block_owner: Vec<usize>,
}

pub(super) fn verify_program(
    program: &AwbcProgram,
    budget: AwbcVerifyBudget,
    context: AwbcVerifyContext<'_>,
) -> Result<(), AwbcVerifyError> {
    verify_header(program, context)?;
    verify_strings(program)?;
    if context.require_entrypoint && program.entries.is_empty() {
        return Err(AwbcVerifyError::MissingEntrypoint);
    }
    verify_runtime_types(program)?;
    verify_constants(program)?;
    verify_effect_sets(program, context)?;
    verify_signatures(program, budget)?;
    verify_frame_layouts(program, budget)?;
    let block_owner = verify_function_blocks(program)?;
    verify_block_instructions(program)?;
    let verifier = Verifier {
        program,
        budget,
        context,
        block_owner,
    };
    verify_resume_points(&verifier)?;
    verify_patterns(&verifier)?;
    verify_runtime_tables(&verifier)?;
    super::code::verify_code(&verifier)?;
    verify_entries(&verifier)?;
    verify_maps_and_resources(&verifier)?;
    Ok(())
}

fn verify_header(
    program: &AwbcProgram,
    context: AwbcVerifyContext<'_>,
) -> Result<(), AwbcVerifyError> {
    if program.header.abi_version != AWBC_ABI_VERSION {
        return Err(AwbcVerifyError::UnsupportedAbi {
            actual: program.header.abi_version,
            expected: AWBC_ABI_VERSION,
        });
    }
    if program.header.minimum_runtime_abi > context.runtime_abi_version {
        return Err(AwbcVerifyError::RuntimeAbiTooOld {
            required: program.header.minimum_runtime_abi,
            actual: context.runtime_abi_version,
        });
    }
    let unsupported = program.header.feature_bits & !context.supported_feature_bits;
    if unsupported != 0 {
        return Err(AwbcVerifyError::UnsupportedFeatureBits { unsupported });
    }
    if context
        .expected_host_abi_digest
        .is_some_and(|expected| expected != program.header.host_abi_digest)
    {
        return Err(AwbcVerifyError::HostAbiDigestMismatch);
    }
    Ok(())
}

fn verify_strings(program: &AwbcProgram) -> Result<(), AwbcVerifyError> {
    for (index, pair) in program.strings.windows(2).enumerate() {
        if pair[0] >= pair[1] {
            return Err(AwbcVerifyError::NonCanonicalStringTable { index: index + 1 });
        }
    }
    Ok(())
}

fn verify_runtime_types(program: &AwbcProgram) -> Result<(), AwbcVerifyError> {
    for (index, ty) in program.runtime_types.iter().enumerate() {
        let at = format!("runtime type {index}");
        match ty {
            AwbcRuntimeType::Tuple(items) => {
                for item in items {
                    check_index(program.runtime_types.len(), item.0, "runtime_types", &at)?;
                }
            }
            AwbcRuntimeType::Sequence(item) => {
                check_index(program.runtime_types.len(), item.0, "runtime_types", &at)?;
            }
            AwbcRuntimeType::Record { public_id, fields } => {
                check_optional_string(program, *public_id, &at)?;
                let mut names = BTreeSet::new();
                for field in fields {
                    check_string(program, field.name, &at)?;
                    check_index(
                        program.runtime_types.len(),
                        field.ty.0,
                        "runtime_types",
                        &at,
                    )?;
                    if !names.insert(field.name) {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "record type contains duplicate field names".to_owned(),
                        });
                    }
                }
            }
            AwbcRuntimeType::Variant { public_id, cases } => {
                check_optional_string(program, *public_id, &at)?;
                let mut names = BTreeSet::new();
                for case in cases {
                    check_string(program, case.name, &at)?;
                    if let Some(payload) = case.payload {
                        check_index(program.runtime_types.len(), payload.0, "runtime_types", &at)?;
                    }
                    if !names.insert(case.name) {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "variant type contains duplicate case names".to_owned(),
                        });
                    }
                }
            }
            AwbcRuntimeType::Unit
            | AwbcRuntimeType::Bool
            | AwbcRuntimeType::Int(_)
            | AwbcRuntimeType::UInt(_)
            | AwbcRuntimeType::F32
            | AwbcRuntimeType::F64
            | AwbcRuntimeType::String
            | AwbcRuntimeType::Char
            | AwbcRuntimeType::Duration
            | AwbcRuntimeType::EntityRef
            | AwbcRuntimeType::MatrixF32
            | AwbcRuntimeType::MatrixF64
            | AwbcRuntimeType::TensorF32
            | AwbcRuntimeType::TensorF64
            | AwbcRuntimeType::TaskHandle
            | AwbcRuntimeType::NeedHandle
            | AwbcRuntimeType::Dynamic => {}
        }
    }
    Ok(())
}

fn verify_constants(program: &AwbcProgram) -> Result<(), AwbcVerifyError> {
    for (index, constant) in program.constants.iter().enumerate() {
        let at = format!("constant {index}");
        match constant {
            AwbcConstant::String(id) | AwbcConstant::EntityRef(id) => {
                check_string(program, *id, &at)?;
            }
            AwbcConstant::Char(value) => {
                if char::from_u32(*value).is_none() {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at,
                        message: "char constant is not a Unicode scalar value".to_owned(),
                    });
                }
            }
            AwbcConstant::Tuple(items) | AwbcConstant::Sequence(items) => {
                for item in items {
                    check_index(program.constants.len(), item.0, "constants", &at)?;
                }
            }
            AwbcConstant::Record { ty, fields } => {
                check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
                for field in fields {
                    check_index(program.constants.len(), field.0, "constants", &at)?;
                }
                let AwbcRuntimeType::Record {
                    fields: type_fields,
                    ..
                } = &program.runtime_types[ty.index()]
                else {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at,
                        message: "record constant references a non-record type".to_owned(),
                    });
                };
                if type_fields.len() != fields.len() {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: format!("constant {index}"),
                        message: "record constant field count does not match type".to_owned(),
                    });
                }
            }
            AwbcConstant::Variant { ty, case, payload } => {
                check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
                let AwbcRuntimeType::Variant { cases, .. } = &program.runtime_types[ty.index()]
                else {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at,
                        message: "variant constant references a non-variant type".to_owned(),
                    });
                };
                let Some(case_layout) = cases.get(*case as usize) else {
                    return Err(AwbcVerifyError::IndexOutOfBounds {
                        table: "variant cases",
                        index: *case,
                        at: format!("constant {index}"),
                    });
                };
                if case_layout.payload.is_some() != payload.is_some() {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: format!("constant {index}"),
                        message: "variant constant payload shape does not match type".to_owned(),
                    });
                }
                if let Some(payload) = payload {
                    check_index(program.constants.len(), payload.0, "constants", &at)?;
                }
            }
            AwbcConstant::Range { start, end, .. } => {
                if let Some(start) = start {
                    check_index(program.constants.len(), start.0, "constants", &at)?;
                }
                if let Some(end) = end {
                    check_index(program.constants.len(), end.0, "constants", &at)?;
                }
            }
            AwbcConstant::TensorF32 { shape, values } => {
                verify_tensor_shape(shape, values.len(), &at)?;
            }
            AwbcConstant::TensorF64 { shape, values } => {
                verify_tensor_shape(shape, values.len(), &at)?;
            }
            AwbcConstant::Unit
            | AwbcConstant::Bool(_)
            | AwbcConstant::Int { .. }
            | AwbcConstant::UInt { .. }
            | AwbcConstant::F32Bits(_)
            | AwbcConstant::F64Bits(_)
            | AwbcConstant::DurationNanos(_)
            | AwbcConstant::Bytes(_) => {}
        }
    }
    Ok(())
}

fn verify_tensor_shape(shape: &[u32], value_len: usize, at: &str) -> Result<(), AwbcVerifyError> {
    let elements = shape.iter().try_fold(1_usize, |total, dimension| {
        total.checked_mul(*dimension as usize)
    });
    if elements != Some(value_len) {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "tensor shape does not match element count".to_owned(),
        });
    }
    Ok(())
}

fn verify_effect_sets(
    program: &AwbcProgram,
    context: AwbcVerifyContext<'_>,
) -> Result<(), AwbcVerifyError> {
    for (index, effect_set) in program.effect_sets.iter().enumerate() {
        for effect in &effect_set.effects {
            check_string(program, *effect, &format!("effect set {index}"))?;
            if let Some(allowed) = context.allowed_effects {
                let name = &program.strings[effect.index()];
                if !allowed.contains(name) {
                    return Err(AwbcVerifyError::EffectDenied {
                        effect: name.clone(),
                    });
                }
            }
        }
        if effect_set.effects.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(AwbcVerifyError::NonCanonicalEffectSet { effect_set: index });
        }
    }
    Ok(())
}

fn verify_signatures(
    program: &AwbcProgram,
    budget: AwbcVerifyBudget,
) -> Result<(), AwbcVerifyError> {
    for (index, signature) in program.signatures.iter().enumerate() {
        if signature.params.len() > budget.params_per_signature {
            return Err(AwbcVerifyError::SignatureBudgetExceeded {
                signature: index,
                budget: "params_per_signature",
            });
        }
        for ty in &signature.params {
            check_index(
                program.runtime_types.len(),
                ty.0,
                "runtime_types",
                &format!("signature {index}"),
            )?;
        }
        if let Some(result) = signature.result {
            check_index(
                program.runtime_types.len(),
                result.0,
                "runtime_types",
                &format!("signature {index}"),
            )?;
        }
        check_index(
            program.effect_sets.len(),
            signature.effects.0,
            "effect_sets",
            &format!("signature {index}"),
        )?;
    }
    Ok(())
}

fn verify_frame_layouts(
    program: &AwbcProgram,
    budget: AwbcVerifyBudget,
) -> Result<(), AwbcVerifyError> {
    for (index, layout) in program.frame_layouts.iter().enumerate() {
        if layout.slots.len() > budget.frame_slots_per_function {
            return Err(AwbcVerifyError::FrameBudgetExceeded {
                layout: index,
                budget: "frame_slots_per_function",
            });
        }
        for slot in &layout.slots {
            check_optional_string(program, slot.name, &format!("frame layout {index}"))?;
            check_index(
                program.runtime_types.len(),
                slot.ty.0,
                "runtime_types",
                &format!("frame layout {index}"),
            )?;
            if slot.scope_depth > layout.max_scope_depth {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: format!("frame layout {index}"),
                    message: "slot scope depth exceeds frame maximum".to_owned(),
                });
            }
        }
    }
    Ok(())
}

fn verify_function_blocks(program: &AwbcProgram) -> Result<Vec<usize>, AwbcVerifyError> {
    let mut owners = vec![None; program.blocks.len()];
    for (function_index, function) in program.functions.iter().enumerate() {
        let at = format!("function {function_index}");
        check_optional_string(program, function.public_id, &at)?;
        check_index(
            program.signatures.len(),
            function.signature.0,
            "signatures",
            &at,
        )?;
        check_index(
            program.frame_layouts.len(),
            function.frame_layout.0,
            "frame_layouts",
            &at,
        )?;
        let range = checked_range(function.blocks, program.blocks.len(), "blocks", &at)?;
        if !range.contains(&function.entry_block.index()) {
            return Err(AwbcVerifyError::EntryBlockOutsideFunction {
                function: function_index,
                block: function.entry_block.0,
            });
        }
        for block_index in range {
            if owners[block_index].replace(function_index).is_some() {
                return Err(AwbcVerifyError::InvalidTableOwnership {
                    table: "blocks",
                    index: block_index,
                });
            }
            let actual = program.blocks[block_index].owner.0;
            if actual as usize != function_index {
                return Err(AwbcVerifyError::BlockOwnerMismatch {
                    block: block_index,
                    actual,
                    expected: function_index,
                });
            }
        }
        verify_parameter_layout(program, function_index)?;
    }
    owners
        .into_iter()
        .enumerate()
        .map(|(index, owner)| {
            owner.ok_or(AwbcVerifyError::InvalidTableOwnership {
                table: "blocks",
                index,
            })
        })
        .collect()
}

fn verify_parameter_layout(
    program: &AwbcProgram,
    function_index: usize,
) -> Result<(), AwbcVerifyError> {
    let function = &program.functions[function_index];
    let signature = &program.signatures[function.signature.index()];
    let layout = &program.frame_layouts[function.frame_layout.index()];
    let parameter_slots = layout
        .slots
        .iter()
        .take_while(|slot| slot.role == AwbcFrameSlotRole::Parameter)
        .collect::<Vec<_>>();
    if parameter_slots.len() != signature.params.len()
        || parameter_slots
            .iter()
            .zip(&signature.params)
            .any(|(slot, ty)| slot.ty != *ty)
        || layout
            .slots
            .iter()
            .skip(parameter_slots.len())
            .any(|slot| slot.role == AwbcFrameSlotRole::Parameter)
    {
        return Err(AwbcVerifyError::ParameterLayoutMismatch {
            function: function_index,
        });
    }
    Ok(())
}

fn verify_block_instructions(program: &AwbcProgram) -> Result<Vec<usize>, AwbcVerifyError> {
    let mut owners = vec![None; program.instructions.len()];
    for (block_index, block) in program.blocks.iter().enumerate() {
        check_optional_index(
            program.source_map.len(),
            block.source_map.map(|id| id.0),
            "source_map",
            &format!("block {block_index}"),
        )?;
        let range = checked_range(
            block.instructions,
            program.instructions.len(),
            "instructions",
            &format!("block {block_index}"),
        )?;
        for instruction_index in range {
            if owners[instruction_index].replace(block_index).is_some() {
                return Err(AwbcVerifyError::InvalidTableOwnership {
                    table: "instructions",
                    index: instruction_index,
                });
            }
        }
    }
    owners
        .into_iter()
        .enumerate()
        .map(|(index, owner)| {
            owner.ok_or(AwbcVerifyError::InvalidTableOwnership {
                table: "instructions",
                index,
            })
        })
        .collect()
}

fn verify_resume_points(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    for (index, resume) in program.resume_points.iter().enumerate() {
        let at = format!("resume point {index}");
        check_index(program.functions.len(), resume.function.0, "functions", &at)?;
        check_index(program.blocks.len(), resume.block.0, "blocks", &at)?;
        check_index(
            program.frame_layouts.len(),
            resume.frame_layout.0,
            "frame_layouts",
            &at,
        )?;
        let function = &program.functions[resume.function.index()];
        if verifier.block_owner[resume.block.index()] != resume.function.index()
            || function.frame_layout != resume.frame_layout
        {
            return Err(AwbcVerifyError::ResumePointMismatch {
                resume: u32::try_from(index).expect("AWBC resume indices originate from u32 ids"),
                at,
            });
        }
    }
    Ok(())
}

fn verify_patterns(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    for (index, pattern) in program.patterns.iter().enumerate() {
        let at = format!("pattern {index}");
        match pattern {
            AwbcPattern::Bind { expected, .. } => {
                if let Some(ty) = expected {
                    check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
                }
            }
            AwbcPattern::Literal(constant) => {
                check_index(program.constants.len(), constant.0, "constants", &at)?;
            }
            AwbcPattern::Entity(string) => check_string(program, *string, &at)?,
            AwbcPattern::Tuple(items) | AwbcPattern::Sequence { items, .. } => {
                for child in items {
                    check_index(program.patterns.len(), child.0, "patterns", &at)?;
                }
            }
            AwbcPattern::Record { ty, fields, .. } => {
                if let Some(ty) = ty {
                    check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
                }
                for field in fields {
                    check_index(program.patterns.len(), field.pattern.0, "patterns", &at)?;
                }
            }
            AwbcPattern::Variant { ty, payload, .. } => {
                if let Some(ty) = ty {
                    check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
                }
                if let Some(payload) = payload {
                    check_index(program.patterns.len(), payload.0, "patterns", &at)?;
                }
            }
            AwbcPattern::Whole { inner, .. } => {
                check_index(program.patterns.len(), inner.0, "patterns", &at)?;
            }
            AwbcPattern::Discard => {}
        }
    }
    let mut state = vec![0_u8; program.patterns.len()];
    for index in 0..program.patterns.len() {
        visit_pattern(verifier, index, 0, &mut state)?;
    }
    Ok(())
}

fn visit_pattern(
    verifier: &Verifier<'_, '_>,
    index: usize,
    depth: usize,
    state: &mut [u8],
) -> Result<(), AwbcVerifyError> {
    if depth > verifier.budget.pattern_depth {
        return Err(AwbcVerifyError::PatternDepthExceeded {
            pattern: index,
            limit: verifier.budget.pattern_depth,
        });
    }
    match state[index] {
        1 => return Err(AwbcVerifyError::PatternCycle { pattern: index }),
        2 => return Ok(()),
        _ => {}
    }
    state[index] = 1;
    for child in pattern_children(&verifier.program.patterns[index]) {
        visit_pattern(verifier, child.index(), depth + 1, state)?;
    }
    state[index] = 2;
    Ok(())
}

fn pattern_children(pattern: &AwbcPattern) -> Vec<AwbcPatternId> {
    match pattern {
        AwbcPattern::Tuple(items) | AwbcPattern::Sequence { items, .. } => items.clone(),
        AwbcPattern::Record { fields, .. } => fields.iter().map(|field| field.pattern).collect(),
        AwbcPattern::Variant {
            payload: Some(payload),
            ..
        } => vec![*payload],
        AwbcPattern::Whole { inner, .. } => vec![*inner],
        AwbcPattern::Bind { .. }
        | AwbcPattern::Discard
        | AwbcPattern::Literal(_)
        | AwbcPattern::Entity(_)
        | AwbcPattern::Variant { payload: None, .. } => Vec::new(),
    }
}

fn verify_runtime_tables(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    for (index, intrinsic) in program.intrinsics.iter().enumerate() {
        let at = format!("intrinsic {index}");
        check_string(program, intrinsic.public_id, &at)?;
        check_index(
            program.signatures.len(),
            intrinsic.signature.0,
            "signatures",
            &at,
        )?;
    }
    for (index, call) in program.host_calls.iter().enumerate() {
        let at = format!("host call {index}");
        check_string(program, call.public_id, &at)?;
        check_capability(verifier, call.capability, &at)?;
        check_string(program, call.operation, &at)?;
        check_index(
            program.signatures.len(),
            call.signature.0,
            "signatures",
            &at,
        )?;
    }
    for (index, task) in program.task_plans.iter().enumerate() {
        let at = format!("task plan {index}");
        check_string(program, task.public_id, &at)?;
        check_string(program, task.need_id, &at)?;
        check_capability(verifier, task.capability, &at)?;
        check_string(program, task.operation, &at)?;
        check_string(program, task.cancel_scope, &at)?;
        check_index(
            program.signatures.len(),
            task.signature.0,
            "signatures",
            &at,
        )?;
        for argument in &task.arguments {
            check_optional_string(program, argument.name, &at)?;
        }
        if task.many.as_ref().is_some_and(|many| many.limit == 0) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "await-many task limit must be non-zero".to_owned(),
            });
        }
    }
    for (index, effect) in program.effect_plans.iter().enumerate() {
        let at = format!("effect plan {index}");
        check_index(
            program.signatures.len(),
            effect.signature.0,
            "signatures",
            &at,
        )?;
        if let Some(capability) = effect.capability {
            check_capability(verifier, capability, &at)?;
        }
        verify_effect_audio_payload(program, index, effect)?;
        for constant in &effect.static_args {
            check_index(program.constants.len(), constant.0, "constants", &at)?;
        }
        for access in &effect.resources {
            check_index(program.resources.len(), access.resource.0, "resources", &at)?;
        }
    }
    for (index, choice) in program.choices.iter().enumerate() {
        check_optional_string(program, choice.public_id, &format!("choice {index}"))?;
        checked_range(
            choice.options,
            program.choice_options.len(),
            "choice_options",
            &format!("choice {index}"),
        )?;
    }
    for (index, option) in program.choice_options.iter().enumerate() {
        let at = format!("choice option {index}");
        check_optional_string(program, option.public_id, &at)?;
        check_string(program, option.label, &at)?;
        check_optional_index(
            program.functions.len(),
            option.condition.map(|id| id.0),
            "functions",
            &at,
        )?;
        check_optional_index(
            program.functions.len(),
            option.target.map(|id| id.0),
            "functions",
            &at,
        )?;
        check_optional_index(
            program.effect_plans.len(),
            option.out_effect.map(|id| id.0),
            "effect_plans",
            &at,
        )?;
        for effect in &option.effects {
            check_index(program.effect_plans.len(), effect.0, "effect_plans", &at)?;
        }
    }
    verify_content_and_line_tables(verifier)?;
    verify_stream_and_source_tables(verifier)?;
    for (index, helper) in program.pure_helpers.iter().enumerate() {
        let at = format!("pure helper {index}");
        check_string(program, helper.public_id, &at)?;
        check_index(
            program.signatures.len(),
            helper.signature.0,
            "signatures",
            &at,
        )?;
        check_index(program.functions.len(), helper.function.0, "functions", &at)?;
        let function = &program.functions[helper.function.index()];
        if function.signature != helper.signature || function.kind != AwbcFunctionKind::PureHelper {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "pure-helper table does not match function signature/kind".to_owned(),
            });
        }
    }
    Ok(())
}

fn verify_effect_audio_payload(
    program: &AwbcProgram,
    effect_index: usize,
    effect: &AwbcEffectPlan,
) -> Result<(), AwbcVerifyError> {
    match (effect.kind, effect.audio) {
        (AwbcEffectKind::Audio, Some(command)) => {
            if !effect.static_args.is_empty() {
                return Err(AwbcVerifyError::MalformedAudioPayload {
                    effect: effect_index,
                    message: "audio effect must use typed payload rows, not legacy static args"
                        .to_owned(),
                });
            }
            verify_audio_command_refs(program, effect_index, effect.signature, command)
        }
        (AwbcEffectKind::Audio, None) => Err(AwbcVerifyError::MalformedAudioPayload {
            effect: effect_index,
            message: "audio effect is missing typed payload row".to_owned(),
        }),
        (_, Some(_)) => Err(AwbcVerifyError::MalformedAudioPayload {
            effect: effect_index,
            message: "non-audio effect carries an audio payload row".to_owned(),
        }),
        (_, None) => Ok(()),
    }
}

fn verify_audio_command_refs(
    program: &AwbcProgram,
    effect_index: usize,
    signature: AwbcSignatureId,
    command: AwbcAudioCommandId,
) -> Result<(), AwbcVerifyError> {
    check_index(
        program.audio_commands.len(),
        command.0,
        "audio_commands",
        &format!("effect plan {effect_index}"),
    )?;
    let arg_count = program.signatures[signature.index()].params.len();
    for value in program.audio_commands[command.index()].value_refs() {
        match value {
            AwbcAudioValueRef::Arg(arg) if arg.index() >= arg_count => {
                return Err(AwbcVerifyError::MalformedAudioPayload {
                    effect: effect_index,
                    message: format!(
                        "audio arg {} exceeds effect signature arity {arg_count}",
                        arg.0
                    ),
                });
            }
            AwbcAudioValueRef::Arg(_) => {}
            AwbcAudioValueRef::Const(constant) => check_index(
                program.constants.len(),
                constant.0,
                "constants",
                &format!("effect plan {effect_index} audio payload"),
            )?,
        }
    }
    Ok(())
}

fn verify_content_and_line_tables(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    for (index, content) in program.content_units.iter().enumerate() {
        let at = format!("content unit {index}");
        check_string(program, content.public_id, &at)?;
        check_optional_index(
            program.line_task_groups.len(),
            content.line_task_group.map(|id| id.0),
            "line_task_groups",
            &at,
        )?;
        check_optional_index(
            program.display_map.len(),
            content.display.map(|id| id.0),
            "display_map",
            &at,
        )?;
        check_optional_index(
            program.source_map.len(),
            content.source.map(|id| id.0),
            "source_map",
            &at,
        )?;
        for resource in &content.resources {
            check_index(program.resources.len(), resource.0, "resources", &at)?;
        }
    }
    for (index, group) in program.line_task_groups.iter().enumerate() {
        let at = format!("line task group {index}");
        check_index(
            program.line_task_nodes.len(),
            group.root.0,
            "line_task_nodes",
            &at,
        )?;
        for option in &group.options {
            check_string(program, option.name, &at)?;
            check_index(program.constants.len(), option.value.0, "constants", &at)?;
        }
        for function in [group.bindings, group.out].into_iter().flatten() {
            check_index(program.functions.len(), function.0, "functions", &at)?;
        }
        for handler in &group.cancel_handlers {
            check_string(program, handler.trigger, &at)?;
            check_index(
                program.functions.len(),
                handler.function.0,
                "functions",
                &at,
            )?;
        }
    }
    for (index, node) in program.line_task_nodes.iter().enumerate() {
        let at = format!("line task node {index}");
        match node {
            AwbcLineTaskNode::Sequence(nodes)
            | AwbcLineTaskNode::Start(nodes)
            | AwbcLineTaskNode::Parallel {
                children: nodes, ..
            } => {
                for node in nodes {
                    check_index(
                        program.line_task_nodes.len(),
                        node.0,
                        "line_task_nodes",
                        &at,
                    )?;
                }
            }
            AwbcLineTaskNode::Child {
                task,
                trigger,
                scope,
                ..
            } => {
                check_index(program.task_plans.len(), task.0, "task_plans", &at)?;
                check_index(
                    program.line_task_nodes.len(),
                    scope.0,
                    "line_task_nodes",
                    &at,
                )?;
                if let AwbcLineTaskTrigger::Mark(mark) = trigger {
                    check_string(program, *mark, &at)?;
                }
            }
            AwbcLineTaskNode::Effect(effect) => {
                check_index(program.effect_plans.len(), effect.0, "effect_plans", &at)?;
            }
        }
    }
    Ok(())
}

fn verify_stream_and_source_tables(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    for (index, stream) in program.stream_plans.iter().enumerate() {
        let at = format!("stream plan {index}");
        check_string(program, stream.public_id, &at)?;
        check_index(
            program.runtime_types.len(),
            stream.item_type.0,
            "runtime_types",
            &at,
        )?;
        check_index(
            program.runtime_types.len(),
            stream.error_type.0,
            "runtime_types",
            &at,
        )?;
        check_index(
            program.functions.len(),
            stream.transform.0,
            "functions",
            &at,
        )?;
        if program.functions[stream.transform.index()].kind != AwbcFunctionKind::StreamTransform {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "stream transform references wrong function kind".to_owned(),
            });
        }
    }
    for (index, source) in program.source_plans.iter().enumerate() {
        let at = format!("source plan {index}");
        check_string(program, source.public_id, &at)?;
        check_index(
            program.runtime_types.len(),
            source.item_type.0,
            "runtime_types",
            &at,
        )?;
        check_index(
            program.runtime_types.len(),
            source.error_type.0,
            "runtime_types",
            &at,
        )?;
        check_index(program.functions.len(), source.open.0, "functions", &at)?;
        if program.functions[source.open.index()].kind != AwbcFunctionKind::SourceOpen {
            return Err(AwbcVerifyError::InvalidInvariant {
                at: at.clone(),
                message: "source open references wrong function kind".to_owned(),
            });
        }
        let mut kinds = BTreeSet::new();
        for handler in &source.handlers {
            check_index(
                program.functions.len(),
                handler.function.0,
                "functions",
                &at,
            )?;
            if let Some(pattern) = handler.pattern {
                check_index(program.patterns.len(), pattern.0, "patterns", &at)?;
            }
            if program.functions[handler.function.index()].kind != AwbcFunctionKind::SourceHandler {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "source handler references wrong function kind".to_owned(),
                });
            }
            if !kinds.insert(handler.kind as u8) {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "source plan contains duplicate handler kind".to_owned(),
                });
            }
        }
        if source.policy.max_queue == 0 {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "source max_queue must be non-zero".to_owned(),
            });
        }
    }
    Ok(())
}

fn verify_entries(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    let mut ids = BTreeSet::new();
    for (entry_index, entry) in program.entries.iter().enumerate() {
        let at = format!("entry {entry_index}");
        check_string(program, entry.public_id, &at)?;
        if !ids.insert(entry.public_id) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "duplicate public entry id".to_owned(),
            });
        }
        if let AwbcEntryKind::Custom(kind) = entry.kind {
            check_string(program, kind, &format!("entry {entry_index}"))?;
        }
        check_index(
            program.signatures.len(),
            entry.signature.0,
            "signatures",
            &at,
        )?;
        match &entry.target {
            AwbcEntryTarget::Function(function) => {
                verify_entry_function(program, entry_index, entry.signature, *function)?;
            }
            AwbcEntryTarget::Routes(routes) => {
                if routes.is_empty() {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at,
                        message: "route entry must contain at least one route".to_owned(),
                    });
                }
                let mut route_ids = BTreeSet::new();
                for route in routes {
                    check_string(program, route.method, &format!("entry {entry_index} route"))?;
                    check_string(program, route.path, &format!("entry {entry_index} route"))?;
                    if !route_ids.insert((route.method, route.path)) {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: format!("entry {entry_index}"),
                            message: "duplicate route method/path".to_owned(),
                        });
                    }
                    verify_entry_function(program, entry_index, entry.signature, route.target)?;
                    let layout = &program.frame_layouts
                        [program.functions[route.target.index()].frame_layout.index()];
                    for binding in &route.bindings {
                        if binding.register.index() >= layout.slots.len() {
                            return Err(AwbcVerifyError::RegisterOutOfBounds {
                                function: route.target.index(),
                                block: program.functions[route.target.index()].entry_block.index(),
                                register: binding.register.0,
                            });
                        }
                        match binding.source {
                            AwbcRouteBindingSource::PathParameter(name) => {
                                check_string(program, name, &format!("entry {entry_index} route"))?;
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn verify_entry_function(
    program: &AwbcProgram,
    entry_index: usize,
    signature: AwbcSignatureId,
    function: AwbcFunctionId,
) -> Result<(), AwbcVerifyError> {
    check_index(
        program.functions.len(),
        function.0,
        "functions",
        &format!("entry {entry_index}"),
    )?;
    if program.functions[function.index()].signature != signature {
        return Err(AwbcVerifyError::EntrypointSignatureMismatch {
            entry: entry_index,
            function: function.0,
        });
    }
    Ok(())
}

fn verify_maps_and_resources(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    let mut displays = BTreeSet::new();
    for (index, display) in program.display_map.iter().enumerate() {
        check_index(
            program.content_units.len(),
            display.content.0,
            "content_units",
            &format!("display map {index}"),
        )?;
        check_string(
            program,
            display.display_key,
            &format!("display map {index}"),
        )?;
        if !displays.insert(display.content) {
            return Err(AwbcVerifyError::DuplicateMapIdentity { entry: index });
        }
    }
    let mut sources = BTreeSet::new();
    for (index, source) in program.source_map.iter().enumerate() {
        check_string(program, source.source_file, &format!("source map {index}"))?;
        check_optional_string(program, source.anchor, &format!("source map {index}"))?;
        if source.start > source.end
            || source.end - source.start > verifier.budget.source_span_bytes
        {
            return Err(AwbcVerifyError::InvalidSourceSpan {
                entry: index,
                start: source.start,
                end: source.end,
            });
        }
        let identity = match source.location {
            AwbcCodeLocation::Instruction(id) => {
                check_index(
                    program.instructions.len(),
                    id.0,
                    "instructions",
                    &format!("source map {index}"),
                )?;
                (0_u8, id.0)
            }
            AwbcCodeLocation::Block(id) => {
                check_index(
                    program.blocks.len(),
                    id.0,
                    "blocks",
                    &format!("source map {index}"),
                )?;
                (1_u8, id.0)
            }
            AwbcCodeLocation::ResumePoint(id) => {
                check_index(
                    program.resume_points.len(),
                    id.0,
                    "resume_points",
                    &format!("source map {index}"),
                )?;
                (2_u8, id.0)
            }
        };
        if !sources.insert(identity) {
            return Err(AwbcVerifyError::DuplicateMapIdentity { entry: index });
        }
    }
    let mut resources = BTreeSet::new();
    for (index, resource) in program.resources.iter().enumerate() {
        let at = format!("resource {index}");
        check_string(program, resource.public_id, &at)?;
        check_string(program, resource.kind, &at)?;
        if !resources.insert(resource.public_id) {
            return Err(AwbcVerifyError::InvalidInvariant {
                at,
                message: "duplicate resource public id".to_owned(),
            });
        }
    }
    Ok(())
}

fn check_capability(
    verifier: &Verifier<'_, '_>,
    capability: AwbcStringId,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    check_string(verifier.program, capability, at)?;
    if let Some(allowed) = verifier.context.allowed_capabilities {
        let name = &verifier.program.strings[capability.index()];
        if !allowed.contains(name) {
            return Err(AwbcVerifyError::CapabilityDenied {
                capability: name.clone(),
            });
        }
    }
    Ok(())
}

pub(super) fn check_index(
    len: usize,
    index: u32,
    table: &'static str,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    if index as usize >= len {
        Err(AwbcVerifyError::IndexOutOfBounds {
            table,
            index,
            at: at.to_owned(),
        })
    } else {
        Ok(())
    }
}

pub(super) fn check_optional_index(
    len: usize,
    index: Option<u32>,
    table: &'static str,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    if let Some(index) = index {
        check_index(len, index, table, at)?;
    }
    Ok(())
}

pub(super) fn checked_range(
    range: AwbcTableRange,
    len: usize,
    table: &'static str,
    at: &str,
) -> Result<std::ops::Range<usize>, AwbcVerifyError> {
    let Some(end) = range.checked_end() else {
        return Err(AwbcVerifyError::RangeOutOfBounds {
            table,
            start: range.start,
            len: range.len,
            at: at.to_owned(),
        });
    };
    if end as usize > len {
        return Err(AwbcVerifyError::RangeOutOfBounds {
            table,
            start: range.start,
            len: range.len,
            at: at.to_owned(),
        });
    }
    Ok(range.start as usize..end as usize)
}

pub(super) fn check_string(
    program: &AwbcProgram,
    id: AwbcStringId,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    check_index(program.strings.len(), id.0, "strings", at)
}

pub(super) fn check_optional_string(
    program: &AwbcProgram,
    id: Option<AwbcStringId>,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    check_optional_index(program.strings.len(), id.map(|id| id.0), "strings", at)
}

pub(super) fn block_is_in_function(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: AwbcBlockId,
) -> bool {
    block.index() < verifier.block_owner.len() && verifier.block_owner[block.index()] == function
}

pub(super) fn types_compatible(
    program: &AwbcProgram,
    expected: AwbcTypeId,
    actual: AwbcTypeId,
) -> bool {
    expected == actual
        || matches!(
            program.runtime_types.get(expected.index()),
            Some(AwbcRuntimeType::Dynamic)
        )
        || matches!(
            program.runtime_types.get(actual.index()),
            Some(AwbcRuntimeType::Dynamic)
        )
}

pub(super) fn effect_set_is_subset(
    program: &AwbcProgram,
    subset: AwbcEffectSetId,
    superset: AwbcEffectSetId,
) -> bool {
    let Some(subset) = program.effect_sets.get(subset.index()) else {
        return false;
    };
    let Some(superset) = program.effect_sets.get(superset.index()) else {
        return false;
    };
    subset
        .effects
        .iter()
        .all(|effect| superset.effects.binary_search(effect).is_ok())
}
