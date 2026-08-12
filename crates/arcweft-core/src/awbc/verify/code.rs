#![allow(
    clippy::too_many_lines,
    reason = "AWBC verifier walks complete instruction and terminator families with shared dataflow state"
)]

use super::AwbcVerifyError;
use super::structure::{
    Verifier, block_is_in_function, check_index, check_string, checked_range, effect_set_is_subset,
    types_compatible,
};
use crate::awbc::schema::{
    AwbcBinaryOp, AwbcBindMode, AwbcBlockId, AwbcConstant, AwbcEffectSetId, AwbcFrameLayout,
    AwbcFrameSlotRole, AwbcFunctionFlags, AwbcFunctionKind, AwbcInstruction, AwbcPattern,
    AwbcPatternId, AwbcProgram, AwbcRegisterId, AwbcResumePointId, AwbcRuntimeType,
    AwbcSafePointKind, AwbcScopeId, AwbcSignatureId, AwbcTerminator, AwbcTraitReceiverMode,
    AwbcTypeId, AwbcUnaryOp,
};
use std::collections::VecDeque;

#[derive(Clone, Debug, Eq, PartialEq)]
struct FlowState {
    initialized: Vec<bool>,
    scopes: Vec<AwbcScopeId>,
}

fn block_index_to_u32(index: usize) -> u32 {
    u32::try_from(index).expect("AWBC block indices originate from u32 ids")
}

pub(super) fn verify_code(verifier: &Verifier<'_, '_>) -> Result<(), AwbcVerifyError> {
    for function in 0..verifier.program.functions.len() {
        verify_function(verifier, function)?;
    }
    Ok(())
}

fn verify_function(
    verifier: &Verifier<'_, '_>,
    function_index: usize,
) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    let function = &program.functions[function_index];
    let layout = &program.frame_layouts[function.frame_layout.index()];
    let block_range = checked_range(
        function.blocks,
        program.blocks.len(),
        "blocks",
        &format!("function {function_index}"),
    )?;
    let mut states = vec![None::<FlowState>; program.blocks.len()];
    let mut initial = FlowState {
        initialized: vec![false; layout.slots.len()],
        scopes: Vec::new(),
    };
    for (slot, initialized) in layout.slots.iter().zip(&mut initial.initialized) {
        *initialized = matches!(
            slot.role,
            AwbcFrameSlotRole::Parameter | AwbcFrameSlotRole::RuntimeState
        );
    }
    states[function.entry_block.index()] = Some(initial);
    let mut queue = VecDeque::from([function.entry_block.index()]);
    let mut steps = 0_usize;
    let mut edges = 0_usize;

    while let Some(block_index) = queue.pop_front() {
        steps = steps.saturating_add(1);
        if steps > verifier.budget.dataflow_steps {
            return Err(AwbcVerifyError::BudgetExceeded {
                budget: "dataflow_steps",
            });
        }
        let mut state = states[block_index]
            .clone()
            .expect("queued AWBC block has an incoming state");
        verify_entry_safe_point(verifier, function_index, block_index)?;
        let block = &program.blocks[block_index];
        let instruction_range = checked_range(
            block.instructions,
            program.instructions.len(),
            "instructions",
            &format!("block {block_index}"),
        )?;
        for instruction_index in instruction_range {
            apply_instruction(
                verifier,
                function_index,
                block_index,
                instruction_index,
                &mut state,
            )?;
        }
        let successors = apply_terminator(
            verifier,
            function_index,
            block_index,
            &block.terminator,
            &state,
        )?;
        edges = edges.saturating_add(successors.len());
        if edges > verifier.budget.cfg_edges {
            return Err(AwbcVerifyError::BudgetExceeded {
                budget: "cfg_edges",
            });
        }
        for (target, incoming) in successors {
            if target <= block_index {
                let safe_point = program.blocks[target].safe_point;
                if safe_point != AwbcSafePointKind::LoopBackedge {
                    return Err(AwbcVerifyError::BackedgeWithoutSafePoint {
                        block: block_index,
                        target: block_index_to_u32(target),
                    });
                }
            }
            merge_state(
                verifier,
                function_index,
                target,
                incoming,
                &mut states,
                &mut queue,
            )?;
        }
    }

    for block_index in block_range {
        if states[block_index].is_none() {
            return Err(AwbcVerifyError::UnreachableBlock {
                function: function_index,
                block: block_index,
            });
        }
    }
    Ok(())
}

fn verify_entry_safe_point(
    verifier: &Verifier<'_, '_>,
    function_index: usize,
    block_index: usize,
) -> Result<(), AwbcVerifyError> {
    let function = &verifier.program.functions[function_index];
    if block_index != function.entry_block.index() {
        return Ok(());
    }
    let expected = match function.kind {
        AwbcFunctionKind::Flow => AwbcSafePointKind::FlowEntry,
        AwbcFunctionKind::PureHelper
        | AwbcFunctionKind::TraitMethod
        | AwbcFunctionKind::StreamTransform
        | AwbcFunctionKind::SourceOpen
        | AwbcFunctionKind::SourceHandler
        | AwbcFunctionKind::LineTask
        | AwbcFunctionKind::Synthetic => AwbcSafePointKind::CallableBoundary,
    };
    let actual = verifier.program.blocks[block_index].safe_point;
    if actual != expected {
        return Err(AwbcVerifyError::SafePointMismatch {
            block: block_index,
            actual,
            expected,
        });
    }
    Ok(())
}

fn merge_state(
    verifier: &Verifier<'_, '_>,
    function_index: usize,
    target: usize,
    incoming: FlowState,
    states: &mut [Option<FlowState>],
    queue: &mut VecDeque<usize>,
) -> Result<(), AwbcVerifyError> {
    let target_id = block_index_to_u32(target);
    if !block_is_in_function(verifier, function_index, AwbcBlockId(target_id)) {
        return Err(AwbcVerifyError::ControlFlowEscapesFunction {
            function: function_index,
            block: target,
            target: target_id,
        });
    }
    match &mut states[target] {
        None => {
            states[target] = Some(incoming);
            queue.push_back(target);
        }
        Some(current) => {
            if current.scopes != incoming.scopes {
                return Err(AwbcVerifyError::ScopeDiscipline {
                    function: function_index,
                    block: target,
                    message: "control-flow predecessors have different scope stacks".to_owned(),
                });
            }
            let mut changed = false;
            for (current, incoming) in current.initialized.iter_mut().zip(incoming.initialized) {
                let merged = *current && incoming;
                changed |= merged != *current;
                *current = merged;
            }
            if changed {
                queue.push_back(target);
            }
        }
    }
    Ok(())
}

fn apply_instruction(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    instruction_index: usize,
    state: &mut FlowState,
) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    let instruction = &program.instructions[instruction_index];
    let at = format!("instruction {instruction_index}");
    match instruction {
        AwbcInstruction::Nop => {}
        AwbcInstruction::LoadConst { dst, constant } => {
            check_index(program.constants.len(), constant.0, "constants", &at)?;
            let dst_ty = register_type(verifier, function, block, *dst)?;
            if !constant_matches_type(program, &program.constants[constant.index()], dst_ty, 0) {
                return invalid_type(&at, "constant compatible with destination register");
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::Move { dst, src } => {
            let src_ty = read_register(verifier, function, block, *src, state)?;
            let dst_ty = register_type(verifier, function, block, *dst)?;
            require_compatible(program, dst_ty, src_ty, &at)?;
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::Clear { register } => {
            clear_register(verifier, function, block, *register, state)?;
        }
        AwbcInstruction::EnterScope { scope } => {
            if state.scopes.contains(scope) {
                return Err(AwbcVerifyError::ScopeDiscipline {
                    function,
                    block,
                    message: format!("scope {} is entered twice", scope.0),
                });
            }
            let layout = function_layout(verifier, function);
            if state.scopes.len() + 1 > layout.max_scope_depth as usize {
                return Err(AwbcVerifyError::ScopeDiscipline {
                    function,
                    block,
                    message: "scope depth exceeds frame layout".to_owned(),
                });
            }
            state.scopes.push(*scope);
        }
        AwbcInstruction::ExitScope { scope } => {
            if state.scopes.last() != Some(scope) {
                return Err(AwbcVerifyError::ScopeDiscipline {
                    function,
                    block,
                    message: format!("scope {} is not the active scope", scope.0),
                });
            }
            state.scopes.pop();
            let depth =
                u32::try_from(state.scopes.len()).map_err(|_| AwbcVerifyError::BudgetExceeded {
                    budget: "scope_depth",
                })?;
            for (slot, initialized) in function_layout(verifier, function)
                .slots
                .iter()
                .zip(&mut state.initialized)
            {
                if slot.scope_depth > depth
                    && !matches!(
                        slot.role,
                        AwbcFrameSlotRole::Parameter | AwbcFrameSlotRole::RuntimeState
                    )
                {
                    *initialized = false;
                }
            }
        }
        AwbcInstruction::BindPattern {
            pattern,
            value,
            mode,
        } => {
            let value_ty = read_register(verifier, function, block, *value, state)?;
            validate_pattern(
                verifier,
                function,
                block,
                *pattern,
                value_ty,
                Some(*mode),
                state,
                0,
            )?;
        }
        AwbcInstruction::TestPattern {
            dst,
            pattern,
            value,
        } => {
            let value_ty = read_register(verifier, function, block, *value, state)?;
            validate_pattern(
                verifier, function, block, *pattern, value_ty, None, state, 0,
            )?;
            require_type_kind(verifier, function, block, *dst, is_bool, "bool", &at)?;
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::MakeTuple { dst, items } => {
            check_args_budget(verifier, items.len())?;
            let dst_ty = register_type(verifier, function, block, *dst)?;
            let Some(AwbcRuntimeType::Tuple(types)) = program.runtime_types.get(dst_ty.index())
            else {
                return invalid_type(&at, "tuple destination");
            };
            if types.len() != items.len() {
                return argument_count(&at, types.len(), items.len());
            }
            for (item, expected) in items.iter().zip(types) {
                let actual = read_register(verifier, function, block, *item, state)?;
                require_compatible(program, *expected, actual, &at)?;
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::MakeSequence { dst, items } => {
            check_args_budget(verifier, items.len())?;
            let dst_ty = register_type(verifier, function, block, *dst)?;
            let Some(AwbcRuntimeType::Sequence(item_ty)) =
                program.runtime_types.get(dst_ty.index())
            else {
                return invalid_type(&at, "sequence destination");
            };
            for item in items {
                let actual = read_register(verifier, function, block, *item, state)?;
                require_compatible(program, *item_ty, actual, &at)?;
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::RepeatSequence { dst, value, len } => {
            let dst_ty = register_type(verifier, function, block, *dst)?;
            let Some(AwbcRuntimeType::Sequence(item_ty)) =
                program.runtime_types.get(dst_ty.index())
            else {
                return invalid_type(&at, "sequence destination");
            };
            let value_ty = read_register(verifier, function, block, *value, state)?;
            require_compatible(program, *item_ty, value_ty, &at)?;
            let len_ty = read_register(verifier, function, block, *len, state)?;
            if !is_integer(program.runtime_types.get(len_ty.index())) {
                return invalid_type(&at, "integer repeat length");
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::SequenceLen { dst, sequence } => {
            let sequence_ty = read_register(verifier, function, block, *sequence, state)?;
            if !is_sequence_or_dynamic(program.runtime_types.get(sequence_ty.index())) {
                return invalid_type(&at, "sequence input");
            }
            require_type_kind(verifier, function, block, *dst, is_integer, "integer", &at)?;
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::SequenceGet {
            dst,
            sequence,
            index,
        } => {
            let sequence_ty = read_register(verifier, function, block, *sequence, state)?;
            let index_ty = read_register(verifier, function, block, *index, state)?;
            if !is_integer(program.runtime_types.get(index_ty.index())) {
                return invalid_type(&at, "integer sequence index");
            }
            let dst_ty = register_type(verifier, function, block, *dst)?;
            if let Some(AwbcRuntimeType::Sequence(item_ty)) =
                program.runtime_types.get(sequence_ty.index())
            {
                require_compatible(program, dst_ty, *item_ty, &at)?;
            } else if !is_dynamic(program.runtime_types.get(sequence_ty.index())) {
                return invalid_type(&at, "sequence input");
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::SequenceSlice {
            dst,
            sequence,
            start,
        } => {
            let sequence_ty = read_register(verifier, function, block, *sequence, state)?;
            let start_ty = read_register(verifier, function, block, *start, state)?;
            if !is_integer(program.runtime_types.get(start_ty.index())) {
                return invalid_type(&at, "integer sequence slice start");
            }
            let dst_ty = register_type(verifier, function, block, *dst)?;
            require_compatible(program, dst_ty, sequence_ty, &at)?;
            if !is_sequence_or_dynamic(program.runtime_types.get(sequence_ty.index())) {
                return invalid_type(&at, "sequence input");
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::SequencePush { sequence, value } => {
            let sequence_ty = read_register(verifier, function, block, *sequence, state)?;
            let value_ty = read_register(verifier, function, block, *value, state)?;
            if let Some(AwbcRuntimeType::Sequence(item_ty)) =
                program.runtime_types.get(sequence_ty.index())
            {
                require_compatible(program, *item_ty, value_ty, &at)?;
            } else if !is_dynamic(program.runtime_types.get(sequence_ty.index())) {
                return invalid_type(&at, "sequence input");
            }
        }
        AwbcInstruction::MakeRecord {
            dst,
            ty,
            field_names,
            fields,
        } => {
            check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
            if field_names.len() != fields.len() {
                return argument_count(&at, field_names.len(), fields.len());
            }
            for field_name in field_names {
                check_string(program, *field_name, &at)?;
            }
            let dst_ty = register_type(verifier, function, block, *dst)?;
            require_compatible(program, dst_ty, *ty, &at)?;
            match program.runtime_types.get(ty.index()) {
                Some(AwbcRuntimeType::Record {
                    fields: type_fields,
                    ..
                }) => {
                    if type_fields.len() != fields.len() {
                        return argument_count(&at, type_fields.len(), fields.len());
                    }
                    for ((field_name, field), expected) in
                        field_names.iter().zip(fields).zip(type_fields)
                    {
                        if *field_name != expected.name {
                            return invalid_type(&at, "record field name");
                        }
                        let actual = read_register(verifier, function, block, *field, state)?;
                        require_compatible(program, expected.ty, actual, &at)?;
                    }
                }
                Some(AwbcRuntimeType::Dynamic) => {
                    for field in fields {
                        read_register(verifier, function, block, *field, state)?;
                    }
                }
                _ => return invalid_type(&at, "record type"),
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::MakeVariant {
            dst,
            ty,
            case,
            case_name,
            payload,
        } => {
            check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
            check_string(program, *case_name, &at)?;
            match program.runtime_types.get(ty.index()) {
                Some(AwbcRuntimeType::Variant { cases, .. }) => {
                    let Some(case_layout) = cases.get(*case as usize) else {
                        return Err(AwbcVerifyError::IndexOutOfBounds {
                            table: "variant cases",
                            index: *case,
                            at,
                        });
                    };
                    if case_layout.name != *case_name {
                        return invalid_type(&at, "variant case name");
                    }
                    match (case_layout.payload, payload) {
                        (Some(expected), Some(register)) => {
                            let actual =
                                read_register(verifier, function, block, *register, state)?;
                            require_compatible(program, expected, actual, "variant payload")?;
                        }
                        (None, None) => {}
                        _ => {
                            return Err(AwbcVerifyError::ResultShapeMismatch {
                                at: "variant payload".to_owned(),
                            });
                        }
                    }
                }
                _ => return invalid_type(&at, "variant type"),
            }
            let dst_ty = register_type(verifier, function, block, *dst)?;
            require_compatible(program, dst_ty, *ty, "variant destination")?;
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::ProjectTuple {
            dst,
            target,
            ordinal,
        } => {
            project_ordinal(
                verifier, function, block, *dst, *target, *ordinal, true, state, &at,
            )?;
        }
        AwbcInstruction::ProjectRecord {
            dst,
            target,
            ordinal,
        } => {
            project_ordinal(
                verifier, function, block, *dst, *target, *ordinal, false, state, &at,
            )?;
        }
        AwbcInstruction::ProjectField { dst, target, field } => {
            check_string(program, *field, &at)?;
            let target_ty = read_register(verifier, function, block, *target, state)?;
            let dst_ty = register_type(verifier, function, block, *dst)?;
            match program.runtime_types.get(target_ty.index()) {
                Some(AwbcRuntimeType::Record { fields, .. }) => {
                    let Some(field_layout) =
                        fields.iter().find(|candidate| candidate.name == *field)
                    else {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at,
                            message: "projected field does not exist".to_owned(),
                        });
                    };
                    require_compatible(program, dst_ty, field_layout.ty, "field projection")?;
                }
                Some(AwbcRuntimeType::Dynamic) => {}
                _ => return invalid_type(&at, "record projection target"),
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::Unary { dst, op, src } => {
            let src_ty = read_register(verifier, function, block, *src, state)?;
            let dst_ty = register_type(verifier, function, block, *dst)?;
            match op {
                AwbcUnaryOp::Not => {
                    if !is_bool(program.runtime_types.get(src_ty.index()))
                        || !is_bool(program.runtime_types.get(dst_ty.index()))
                    {
                        return invalid_type(&at, "bool unary operands");
                    }
                }
                AwbcUnaryOp::Neg => {
                    if !is_numeric(program.runtime_types.get(src_ty.index())) {
                        return invalid_type(&at, "numeric unary operand");
                    }
                    require_compatible(program, dst_ty, src_ty, &at)?;
                }
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::Binary { dst, op, lhs, rhs } => {
            let lhs_ty = read_register(verifier, function, block, *lhs, state)?;
            let rhs_ty = read_register(verifier, function, block, *rhs, state)?;
            require_compatible(program, lhs_ty, rhs_ty, &at)?;
            let dst_ty = register_type(verifier, function, block, *dst)?;
            match op {
                AwbcBinaryOp::Eq
                | AwbcBinaryOp::Ne
                | AwbcBinaryOp::Lt
                | AwbcBinaryOp::Le
                | AwbcBinaryOp::Gt
                | AwbcBinaryOp::Ge => {
                    if !is_bool(program.runtime_types.get(dst_ty.index())) {
                        return invalid_type(&at, "bool comparison destination");
                    }
                }
                AwbcBinaryOp::Add | AwbcBinaryOp::Sub | AwbcBinaryOp::Mul | AwbcBinaryOp::Div => {
                    if !is_numeric(program.runtime_types.get(lhs_ty.index())) {
                        return invalid_type(&at, "numeric binary operands");
                    }
                    require_compatible(program, dst_ty, lhs_ty, &at)?;
                }
                AwbcBinaryOp::And | AwbcBinaryOp::Or => {
                    if !is_bool(program.runtime_types.get(lhs_ty.index()))
                        || !is_bool(program.runtime_types.get(dst_ty.index()))
                    {
                        return invalid_type(&at, "bool logical operands");
                    }
                }
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::CallPureHelper { dst, helper, args } => {
            check_index(program.pure_helpers.len(), helper.0, "pure_helpers", &at)?;
            let helper = &program.pure_helpers[helper.index()];
            verify_callable(
                verifier,
                function,
                block,
                helper.signature,
                args,
                Some(*dst),
                state,
                &at,
                &format!("pure helper {}", helper.public_id.0),
            )?;
        }
        AwbcInstruction::AssignField {
            target,
            field,
            value,
        } => {
            check_string(program, *field, &at)?;
            let target_ty = read_register(verifier, function, block, *target, state)?;
            let value_ty = read_register(verifier, function, block, *value, state)?;
            match program.runtime_types.get(target_ty.index()) {
                Some(AwbcRuntimeType::Record { fields, .. }) => {
                    let Some(field_layout) =
                        fields.iter().find(|candidate| candidate.name == *field)
                    else {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at,
                            message: "assigned field does not exist".to_owned(),
                        });
                    };
                    require_compatible(program, field_layout.ty, value_ty, "field assignment")?;
                }
                Some(AwbcRuntimeType::Dynamic) => {}
                _ => return invalid_type(&at, "record assignment target"),
            }
        }
        AwbcInstruction::CallTraitMethod {
            dst,
            method,
            receiver,
            args,
            receiver_out,
        } => {
            check_index(program.trait_methods.len(), method.0, "trait_methods", &at)?;
            let method = &program.trait_methods[method.index()];
            verify_trait_method_call(
                verifier,
                function,
                block,
                method.signature,
                method.receiver,
                *receiver,
                args,
                *dst,
                *receiver_out,
                state,
                &at,
            )?;
        }
        AwbcInstruction::CallIntrinsic {
            dst,
            intrinsic,
            args,
        } => {
            check_index(program.intrinsics.len(), intrinsic.0, "intrinsics", &at)?;
            let intrinsic = &program.intrinsics[intrinsic.index()];
            verify_callable(
                verifier,
                function,
                block,
                intrinsic.signature,
                args,
                *dst,
                state,
                &at,
                &format!("intrinsic {}", intrinsic.public_id.0),
            )?;
        }
        AwbcInstruction::EnsureContent { content } => {
            check_index(program.content_units.len(), content.0, "content_units", &at)?;
        }
        AwbcInstruction::EmitEffect { effect, args } => {
            check_index(program.effect_plans.len(), effect.0, "effect_plans", &at)?;
            let effect = &program.effect_plans[effect.index()];
            verify_callable(
                verifier,
                function,
                block,
                effect.signature,
                args,
                None,
                state,
                &at,
                &format!("effect plan {}", effect.kind as u8),
            )?;
        }
        AwbcInstruction::RegisterCleanup { key, effect, args } => {
            check_string(program, *key, &at)?;
            check_index(program.effect_plans.len(), effect.0, "effect_plans", &at)?;
            let effect = &program.effect_plans[effect.index()];
            verify_callable(
                verifier,
                function,
                block,
                effect.signature,
                args,
                None,
                state,
                &at,
                &format!("cleanup effect plan {}", effect.kind as u8),
            )?;
        }
        AwbcInstruction::CancelCleanup { key } => {
            check_string(program, *key, &at)?;
        }
        AwbcInstruction::MakeFunction {
            dst,
            function: target,
            params,
            capture_names,
            captures,
        } => {
            check_args_budget(verifier, params.len().saturating_add(captures.len()))?;
            check_index(program.functions.len(), target.0, "functions", &at)?;
            if capture_names.len() != captures.len() {
                return argument_count(&at, capture_names.len(), captures.len());
            }
            for param in params {
                check_string(program, *param, &at)?;
            }
            for capture_name in capture_names {
                check_string(program, *capture_name, &at)?;
            }
            for capture in captures {
                read_register(verifier, function, block, *capture, state)?;
            }
            let signature =
                &program.signatures[program.functions[target.index()].signature.index()];
            let expected = params.len().saturating_add(captures.len());
            if signature.params.len() != expected {
                return argument_count(&at, signature.params.len(), expected);
            }
            let target_layout =
                &program.frame_layouts[program.functions[target.index()].frame_layout.index()];
            let target_parameters = target_layout
                .slots
                .iter()
                .filter(|slot| slot.role == AwbcFrameSlotRole::Parameter)
                .collect::<Vec<_>>();
            if target_parameters.len() != expected {
                return argument_count(&at, target_parameters.len(), expected);
            }
            for (position, (name, slot)) in capture_names
                .iter()
                .chain(params)
                .zip(&target_parameters)
                .enumerate()
            {
                if slot.name != Some(*name) {
                    return invalid_type(
                        &at,
                        &format!("function parameter {position} name matching its closure binding"),
                    );
                }
                if slot.ty != signature.params[position] {
                    return type_mismatch(&at, signature.params[position], slot.ty);
                }
            }
            for (position, capture) in captures.iter().enumerate() {
                let actual = register_type(verifier, function, block, *capture)?;
                require_compatible(program, signature.params[position], actual, &at)?;
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::ApplyFunction { dst, callee, args } => {
            check_args_budget(verifier, args.len())?;
            read_register(verifier, function, block, *callee, state)?;
            for arg in args {
                read_register(verifier, function, block, *arg, state)?;
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::StartTask { dst, plan, args } => {
            check_index(program.task_plans.len(), plan.0, "task_plans", &at)?;
            let task = &program.task_plans[plan.index()];
            verify_call_args(
                verifier,
                function,
                block,
                task.signature,
                args,
                state,
                &at,
                &format!("task plan {}", task.public_id.0),
            )?;
            require_type_kind(
                verifier,
                function,
                block,
                *dst,
                is_task_handle,
                "task handle",
                &at,
            )?;
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::SpawnFiber {
            dst,
            function: target,
            args,
        } => {
            check_index(program.functions.len(), target.0, "functions", &at)?;
            let signature = program.functions[target.index()].signature;
            verify_call_args(
                verifier,
                function,
                block,
                signature,
                args,
                state,
                &at,
                &format!("function {}", target.0),
            )?;
            if let Some(dst) = dst {
                require_type_kind(
                    verifier,
                    function,
                    block,
                    *dst,
                    is_task_handle,
                    "task handle",
                    &at,
                )?;
                write_register(verifier, function, block, *dst, state)?;
            }
        }
        AwbcInstruction::StreamYield { stream, value } => {
            check_index(program.stream_plans.len(), stream.0, "stream_plans", &at)?;
            let actual = read_register(verifier, function, block, *value, state)?;
            require_compatible(
                program,
                program.stream_plans[stream.index()].item_type,
                actual,
                &at,
            )?;
        }
        AwbcInstruction::StreamClose { stream } => {
            check_index(program.stream_plans.len(), stream.0, "stream_plans", &at)?;
        }
        AwbcInstruction::SourceClose { source } => {
            check_index(program.source_plans.len(), source.0, "source_plans", &at)?;
        }
        AwbcInstruction::SourceYield { source, value } => {
            check_index(program.source_plans.len(), source.0, "source_plans", &at)?;
            let actual = read_register(verifier, function, block, *value, state)?;
            require_compatible(
                program,
                program.source_plans[source.index()].item_type,
                actual,
                &at,
            )?;
        }
        AwbcInstruction::Drop { register } => {
            read_register(verifier, function, block, *register, state)?;
            clear_register(verifier, function, block, *register, state)?;
        }
    }
    Ok(())
}

fn apply_terminator(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    terminator: &AwbcTerminator,
    state: &FlowState,
) -> Result<Vec<(usize, FlowState)>, AwbcVerifyError> {
    let program = verifier.program;
    let at = format!("terminator of block {block}");
    let mut successors = Vec::new();
    match terminator {
        AwbcTerminator::Jump { target } => {
            push_target(verifier, function, block, *target, state, &mut successors)?;
        }
        AwbcTerminator::Branch {
            condition,
            then_block,
            else_block,
        } => {
            let condition_ty = read_register(verifier, function, block, *condition, state)?;
            if !is_bool(program.runtime_types.get(condition_ty.index())) {
                return invalid_type(&at, "bool branch condition");
            }
            push_target(
                verifier,
                function,
                block,
                *then_block,
                state,
                &mut successors,
            )?;
            push_target(
                verifier,
                function,
                block,
                *else_block,
                state,
                &mut successors,
            )?;
        }
        AwbcTerminator::Match {
            scrutinee,
            arms,
            default,
        } => {
            let scrutinee_ty = read_register(verifier, function, block, *scrutinee, state)?;
            let range = checked_range(*arms, program.match_arms.len(), "match_arms", &at)?;
            for arm_index in range {
                let arm = &program.match_arms[arm_index];
                validate_pattern(
                    verifier,
                    function,
                    block,
                    arm.pattern,
                    scrutinee_ty,
                    None,
                    &mut state.clone(),
                    0,
                )?;
                if let Some(guard) = arm.guard {
                    check_index(program.functions.len(), guard.0, "functions", &at)?;
                    let signature =
                        &program.signatures[program.functions[guard.index()].signature.index()];
                    if signature.params.len() != 1
                        || !types_compatible(program, signature.params[0], scrutinee_ty)
                        || signature
                            .result
                            .is_none_or(|ty| !is_bool(program.runtime_types.get(ty.index())))
                    {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: format!("match arm {arm_index}"),
                            message: "guard must have signature (scrutinee) -> bool".to_owned(),
                        });
                    }
                    require_effects(
                        verifier,
                        function,
                        signature.effects,
                        &format!("match guard {}", guard.0),
                    )?;
                }
                push_target(
                    verifier,
                    function,
                    block,
                    arm.target,
                    state,
                    &mut successors,
                )?;
            }
            push_target(verifier, function, block, *default, state, &mut successors)?;
        }
        AwbcTerminator::CallFunction {
            function: callee,
            args,
            dst,
            resume,
        } => {
            check_index(program.functions.len(), callee.0, "functions", &at)?;
            verify_callable(
                verifier,
                function,
                block,
                program.functions[callee.index()].signature,
                args,
                *dst,
                &mut state.clone(),
                &at,
                &format!("function {}", callee.0),
            )?;
            let target = verify_resume(
                verifier,
                function,
                *resume,
                AwbcSafePointKind::CallableBoundary,
                &at,
            )?;
            successors.push((target, state.clone()));
        }
        AwbcTerminator::GotoStatic {
            function: target,
            args,
        } => {
            check_index(program.functions.len(), target.0, "functions", &at)?;
            if program.flow_identity(*target).is_none() {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "static goto target has no exact semantic Flow binding".to_owned(),
                });
            }
            verify_call_args(
                verifier,
                function,
                block,
                program.functions[target.index()].signature,
                args,
                state,
                &at,
                &format!("goto function {}", target.0),
            )?;
        }
        AwbcTerminator::GotoDynamic { target, args } => {
            let target_ty = read_register(verifier, function, block, *target, state)?;
            if !is_dynamic_target(program.runtime_types.get(target_ty.index())) {
                return invalid_type(&at, "dynamic target string/entity/dynamic");
            }
            check_args_budget(verifier, args.len())?;
            for arg in args {
                read_register(verifier, function, block, *arg, state)?;
            }
            if !program.functions[function]
                .flags
                .contains(AwbcFunctionFlags::HAS_DYNAMIC_TARGET)
            {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "dynamic goto requires HAS_DYNAMIC_TARGET function flag".to_owned(),
                });
            }
        }
        AwbcTerminator::Dialogue {
            content,
            line_task_group,
            resume,
        } => {
            check_index(program.content_units.len(), content.0, "content_units", &at)?;
            check_index(
                program.line_task_groups.len(),
                line_task_group.0,
                "line_task_groups",
                &at,
            )?;
            if program.content_units[content.index()].line_task_group != Some(*line_task_group) {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "dialogue content and line-task group disagree".to_owned(),
                });
            }
            successors.push((
                verify_resume(
                    verifier,
                    function,
                    *resume,
                    AwbcSafePointKind::Dialogue,
                    &at,
                )?,
                state.clone(),
            ));
        }
        AwbcTerminator::Choice {
            choice,
            dst,
            resume,
        } => {
            check_index(program.choices.len(), choice.0, "choices", &at)?;
            require_type_kind(
                verifier,
                function,
                block,
                *dst,
                is_choice_value,
                "choice result",
                &at,
            )?;
            let mut next = state.clone();
            write_register(verifier, function, block, *dst, &mut next)?;
            successors.push((
                verify_resume(verifier, function, *resume, AwbcSafePointKind::Choice, &at)?,
                next,
            ));
        }
        AwbcTerminator::Await {
            handle,
            binding,
            resume,
        } => {
            let handle_ty = read_register(verifier, function, block, *handle, state)?;
            if !is_await_handle(program.runtime_types.get(handle_ty.index())) {
                return invalid_type(&at, "task or need handle");
            }
            let mut next = state.clone();
            if let Some(pattern) = binding {
                let dynamic =
                    dynamic_type(program).ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "await binding requires Dynamic runtime type".to_owned(),
                    })?;
                validate_pattern(
                    verifier,
                    function,
                    block,
                    *pattern,
                    dynamic,
                    Some(AwbcBindMode::Declare),
                    &mut next,
                    0,
                )?;
            }
            successors.push((
                verify_resume(verifier, function, *resume, AwbcSafePointKind::Await, &at)?,
                next,
            ));
        }
        AwbcTerminator::AwaitMany {
            plan,
            source,
            binding,
            resume,
        } => {
            check_index(program.task_plans.len(), plan.0, "task_plans", &at)?;
            if program.task_plans[plan.index()].many.is_none() {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "await-many references a single-task plan".to_owned(),
                });
            }
            let source_ty = read_register(verifier, function, block, *source, state)?;
            if !is_sequence_or_dynamic(program.runtime_types.get(source_ty.index())) {
                return invalid_type(&at, "await-many sequence source");
            }
            let mut next = state.clone();
            if let Some(pattern) = binding {
                let dynamic =
                    dynamic_type(program).ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "await-many binding requires Dynamic runtime type".to_owned(),
                    })?;
                validate_pattern(
                    verifier,
                    function,
                    block,
                    *pattern,
                    dynamic,
                    Some(AwbcBindMode::Declare),
                    &mut next,
                    0,
                )?;
            }
            successors.push((
                verify_resume(
                    verifier,
                    function,
                    *resume,
                    AwbcSafePointKind::AwaitMany,
                    &at,
                )?,
                next,
            ));
        }
        AwbcTerminator::HostCall {
            call,
            args,
            dst,
            resume,
        } => {
            check_index(program.host_calls.len(), call.0, "host_calls", &at)?;
            let call = &program.host_calls[call.index()];
            let mut next = state.clone();
            verify_callable(
                verifier,
                function,
                block,
                call.signature,
                args,
                *dst,
                &mut next,
                &at,
                &format!("host call {}", call.public_id.0),
            )?;
            successors.push((
                verify_resume(
                    verifier,
                    function,
                    *resume,
                    AwbcSafePointKind::HostCall,
                    &at,
                )?,
                next,
            ));
        }
        AwbcTerminator::Return { value } => {
            let signature = &program.signatures[program.functions[function].signature.index()];
            match (signature.result, value) {
                (None, None) => {}
                (Some(expected), Some(register)) => {
                    let actual = read_register(verifier, function, block, *register, state)?;
                    require_compatible(program, expected, actual, &at)?;
                }
                _ => {
                    return Err(AwbcVerifyError::ResultShapeMismatch { at });
                }
            }
            if !state.scopes.is_empty() {
                return Err(AwbcVerifyError::ScopeDiscipline {
                    function,
                    block,
                    message: "return leaves lexical scopes open".to_owned(),
                });
            }
        }
        AwbcTerminator::Trap { message, .. } => {
            if let Some(message) = message {
                check_string(program, *message, &at)?;
            }
        }
        AwbcTerminator::BudgetYield { resume } => {
            successors.push((
                verify_resume(
                    verifier,
                    function,
                    *resume,
                    AwbcSafePointKind::BudgetYield,
                    &at,
                )?,
                state.clone(),
            ));
        }
        AwbcTerminator::Unreachable => {}
    }
    Ok(successors)
}

fn push_target(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    target: AwbcBlockId,
    state: &FlowState,
    successors: &mut Vec<(usize, FlowState)>,
) -> Result<(), AwbcVerifyError> {
    if !block_is_in_function(verifier, function, target) {
        return Err(AwbcVerifyError::ControlFlowEscapesFunction {
            function,
            block,
            target: target.0,
        });
    }
    successors.push((target.index(), state.clone()));
    Ok(())
}

fn verify_resume(
    verifier: &Verifier<'_, '_>,
    function: usize,
    resume: AwbcResumePointId,
    expected: AwbcSafePointKind,
    at: &str,
) -> Result<usize, AwbcVerifyError> {
    check_index(
        verifier.program.resume_points.len(),
        resume.0,
        "resume_points",
        at,
    )?;
    let point = &verifier.program.resume_points[resume.index()];
    let function_layout = verifier.program.functions[function].frame_layout;
    if point.function.index() != function
        || point.frame_layout != function_layout
        || point.kind != expected
        || !block_is_in_function(verifier, function, point.block)
    {
        return Err(AwbcVerifyError::ResumePointMismatch {
            resume: resume.0,
            at: at.to_owned(),
        });
    }
    Ok(point.block.index())
}

#[allow(
    clippy::too_many_arguments,
    reason = "trait-call verification keeps receiver, arguments, and write-back state explicit"
)]
fn verify_trait_method_call(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    signature_id: AwbcSignatureId,
    receiver_mode: AwbcTraitReceiverMode,
    receiver: AwbcRegisterId,
    args: &[AwbcRegisterId],
    dst: AwbcRegisterId,
    receiver_out: Option<AwbcRegisterId>,
    state: &mut FlowState,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    check_index(
        verifier.program.signatures.len(),
        signature_id.0,
        "signatures",
        at,
    )?;
    check_args_budget(verifier, args.len())?;
    let program = verifier.program;
    let signature = &program.signatures[signature_id.index()];
    let Some(receiver_ty) = signature.params.first().copied() else {
        return invalid_type(at, "trait method receiver parameter");
    };
    let actual_receiver = read_register(verifier, function, block, receiver, state)?;
    require_compatible(program, receiver_ty, actual_receiver, at)?;

    let expected_args = signature.params.len().saturating_sub(1);
    if expected_args != args.len() {
        return argument_count(at, expected_args, args.len());
    }
    for (expected, arg) in signature.params.iter().skip(1).zip(args) {
        let actual = read_register(verifier, function, block, *arg, state)?;
        require_compatible(program, *expected, actual, at)?;
    }
    require_effects(verifier, function, signature.effects, "trait method")?;

    let Some(result) = signature.result else {
        return Err(AwbcVerifyError::ResultShapeMismatch { at: at.to_owned() });
    };
    let dst_ty = register_type(verifier, function, block, dst)?;
    require_compatible(program, result, dst_ty, at)?;
    write_register(verifier, function, block, dst, state)?;

    match receiver_mode {
        AwbcTraitReceiverMode::MutRef => {
            let Some(receiver_out) = receiver_out else {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.to_owned(),
                    message: "mut trait call must write receiver_out".to_owned(),
                });
            };
            let out_ty = register_type(verifier, function, block, receiver_out)?;
            require_compatible(program, receiver_ty, out_ty, at)?;
            write_register(verifier, function, block, receiver_out, state)?;
        }
        AwbcTraitReceiverMode::Owned | AwbcTraitReceiverMode::SharedRef => {
            if receiver_out.is_some() {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.to_owned(),
                    message: "non-mut trait call cannot write receiver_out".to_owned(),
                });
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "call verification keeps the function/block/state operands visible at the ABI boundary"
)]
fn verify_callable(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    signature: AwbcSignatureId,
    args: &[AwbcRegisterId],
    dst: Option<AwbcRegisterId>,
    state: &mut FlowState,
    at: &str,
    callee: &str,
) -> Result<(), AwbcVerifyError> {
    verify_call_args(
        verifier, function, block, signature, args, state, at, callee,
    )?;
    let signature = &verifier.program.signatures[signature.index()];
    match (signature.result, dst) {
        (None, None) => {}
        (Some(expected), Some(dst)) => {
            let actual = register_type(verifier, function, block, dst)?;
            require_compatible(verifier.program, expected, actual, at)?;
            write_register(verifier, function, block, dst, state)?;
        }
        _ => {
            return Err(AwbcVerifyError::ResultShapeMismatch { at: at.to_owned() });
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "call verification keeps the function/block/state operands visible at the ABI boundary"
)]
fn verify_call_args(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    signature: AwbcSignatureId,
    args: &[AwbcRegisterId],
    state: &FlowState,
    at: &str,
    callee: &str,
) -> Result<(), AwbcVerifyError> {
    check_index(
        verifier.program.signatures.len(),
        signature.0,
        "signatures",
        at,
    )?;
    check_args_budget(verifier, args.len())?;
    let signature = &verifier.program.signatures[signature.index()];
    if signature.params.len() != args.len() {
        return argument_count(at, signature.params.len(), args.len());
    }
    for (arg, expected) in args.iter().zip(&signature.params) {
        let actual = read_register(verifier, function, block, *arg, state)?;
        require_compatible(verifier.program, *expected, actual, at)?;
    }
    require_effects(verifier, function, signature.effects, callee)
}

fn require_effects(
    verifier: &Verifier<'_, '_>,
    calling_function: usize,
    required: AwbcEffectSetId,
    callee: &str,
) -> Result<(), AwbcVerifyError> {
    let caller_effects = verifier.program.signatures[verifier.program.functions[calling_function]
        .signature
        .index()]
    .effects;
    if !effect_set_is_subset(verifier.program, required, caller_effects) {
        return Err(AwbcVerifyError::EffectSetMismatch {
            caller: calling_function,
            callee: callee.to_owned(),
        });
    }
    Ok(())
}

fn check_args_budget(verifier: &Verifier<'_, '_>, actual: usize) -> Result<(), AwbcVerifyError> {
    if actual > verifier.budget.args_per_call {
        Err(AwbcVerifyError::BudgetExceeded {
            budget: "args_per_call",
        })
    } else {
        Ok(())
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "recursive pattern verification carries explicit function, block, type, mode, state, and depth invariants"
)]
fn validate_pattern(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    pattern: AwbcPatternId,
    value_ty: AwbcTypeId,
    mode: Option<AwbcBindMode>,
    state: &mut FlowState,
    depth: usize,
) -> Result<(), AwbcVerifyError> {
    if depth > verifier.budget.pattern_depth {
        return Err(AwbcVerifyError::PatternDepthExceeded {
            pattern: pattern.index(),
            limit: verifier.budget.pattern_depth,
        });
    }
    check_index(
        verifier.program.patterns.len(),
        pattern.0,
        "patterns",
        &format!("pattern use in block {block}"),
    )?;
    let program = verifier.program;
    match &program.patterns[pattern.index()] {
        AwbcPattern::Bind {
            target, expected, ..
        } => {
            let target_ty = register_type(verifier, function, block, *target)?;
            require_compatible(program, target_ty, value_ty, "pattern binding")?;
            if let Some(expected) = expected {
                require_compatible(program, *expected, value_ty, "typed pattern")?;
            }
            match mode {
                Some(AwbcBindMode::Declare) => {
                    write_register(verifier, function, block, *target, state)?;
                }
                Some(AwbcBindMode::Assign) => {
                    read_register(verifier, function, block, *target, state)?;
                }
                None => {}
            }
        }
        AwbcPattern::Discard => {}
        AwbcPattern::Literal(constant) => {
            check_index(
                program.constants.len(),
                constant.0,
                "constants",
                "literal pattern",
            )?;
            if !constant_matches_type(program, &program.constants[constant.index()], value_ty, 0) {
                return invalid_type("literal pattern", "constant compatible with scrutinee");
            }
        }
        AwbcPattern::Entity(_) => {
            if !matches!(
                program.runtime_types.get(value_ty.index()),
                Some(AwbcRuntimeType::EntityRef | AwbcRuntimeType::Dynamic)
            ) {
                return invalid_type("entity pattern", "entity reference");
            }
        }
        AwbcPattern::Tuple(items) => {
            if let Some(AwbcRuntimeType::Tuple(types)) = program.runtime_types.get(value_ty.index())
            {
                if items.len() != types.len() {
                    return argument_count("tuple pattern", types.len(), items.len());
                }
                for (child, ty) in items.iter().zip(types) {
                    validate_pattern(
                        verifier,
                        function,
                        block,
                        *child,
                        *ty,
                        mode,
                        state,
                        depth + 1,
                    )?;
                }
            } else if !is_dynamic(program.runtime_types.get(value_ty.index())) {
                return invalid_type("tuple pattern", "tuple scrutinee");
            }
        }
        AwbcPattern::Record { ty, fields, .. } => {
            if let Some(expected) = ty {
                require_compatible(program, *expected, value_ty, "record pattern")?;
            }
            let record_ty = ty.unwrap_or(value_ty);
            match program.runtime_types.get(record_ty.index()) {
                Some(AwbcRuntimeType::Record {
                    fields: type_fields,
                    ..
                }) => {
                    for field in fields {
                        let Some(field_ty) = type_fields.get(field.field as usize) else {
                            return Err(AwbcVerifyError::IndexOutOfBounds {
                                table: "record fields",
                                index: field.field,
                                at: "record pattern".to_owned(),
                            });
                        };
                        validate_pattern(
                            verifier,
                            function,
                            block,
                            field.pattern,
                            field_ty.ty,
                            mode,
                            state,
                            depth + 1,
                        )?;
                    }
                }
                Some(AwbcRuntimeType::Nominal { .. }) => {
                    let field_ty =
                        dynamic_type(program).ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                            at: "record pattern".to_owned(),
                            message: "nominal record fields require the dynamic leaf type"
                                .to_owned(),
                        })?;
                    for field in fields {
                        validate_pattern(
                            verifier,
                            function,
                            block,
                            field.pattern,
                            field_ty,
                            mode,
                            state,
                            depth + 1,
                        )?;
                    }
                }
                Some(AwbcRuntimeType::Dynamic) if ty.is_none() => {}
                _ => return invalid_type("record pattern", "typed record scrutinee"),
            }
        }
        AwbcPattern::Sequence { items, rest } => {
            let item_ty = match program.runtime_types.get(value_ty.index()) {
                Some(AwbcRuntimeType::Sequence(item_ty)) => Some(*item_ty),
                Some(AwbcRuntimeType::Dynamic) => dynamic_type(program),
                _ => None,
            }
            .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                at: "sequence pattern".to_owned(),
                message: "sequence pattern requires sequence/dynamic type".to_owned(),
            })?;
            for child in items {
                validate_pattern(
                    verifier,
                    function,
                    block,
                    *child,
                    item_ty,
                    mode,
                    state,
                    depth + 1,
                )?;
            }
            if let Some(rest) = rest {
                let rest_ty = register_type(verifier, function, block, *rest)?;
                require_compatible(program, rest_ty, value_ty, "sequence rest binding")?;
                match mode {
                    Some(AwbcBindMode::Declare) => {
                        write_register(verifier, function, block, *rest, state)?;
                    }
                    Some(AwbcBindMode::Assign) => {
                        read_register(verifier, function, block, *rest, state)?;
                    }
                    None => {}
                }
            }
        }
        AwbcPattern::Variant {
            ty,
            case,
            case_name,
            payload,
        } => {
            check_string(program, *case_name, "variant pattern")?;
            require_compatible(program, *ty, value_ty, "variant pattern")?;
            match program.runtime_types.get(ty.index()) {
                Some(AwbcRuntimeType::Variant { cases, .. }) => {
                    let Some(case_layout) = cases.get(*case as usize) else {
                        return Err(AwbcVerifyError::IndexOutOfBounds {
                            table: "variant cases",
                            index: *case,
                            at: "variant pattern".to_owned(),
                        });
                    };
                    if case_layout.name != *case_name {
                        return invalid_type("variant pattern", "variant case name");
                    }
                    match (case_layout.payload, payload) {
                        (Some(payload_ty), Some(pattern)) => validate_pattern(
                            verifier,
                            function,
                            block,
                            *pattern,
                            payload_ty,
                            mode,
                            state,
                            depth + 1,
                        )?,
                        (None, None) => {}
                        _ => {
                            return Err(AwbcVerifyError::ResultShapeMismatch {
                                at: "variant pattern payload".to_owned(),
                            });
                        }
                    }
                }
                _ => return invalid_type("variant pattern", "typed variant scrutinee"),
            }
        }
        AwbcPattern::Whole { target, inner } => {
            let target_ty = register_type(verifier, function, block, *target)?;
            require_compatible(program, target_ty, value_ty, "whole pattern")?;
            match mode {
                Some(AwbcBindMode::Declare) => {
                    write_register(verifier, function, block, *target, state)?;
                }
                Some(AwbcBindMode::Assign) => {
                    read_register(verifier, function, block, *target, state)?;
                }
                None => {}
            }
            validate_pattern(
                verifier,
                function,
                block,
                *inner,
                value_ty,
                mode,
                state,
                depth + 1,
            )?;
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "projection verification mirrors the typed opcode operands plus dataflow state"
)]
fn project_ordinal(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    dst: AwbcRegisterId,
    target: AwbcRegisterId,
    ordinal: u32,
    tuple: bool,
    state: &mut FlowState,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    let target_ty = read_register(verifier, function, block, target, state)?;
    let dst_ty = register_type(verifier, function, block, dst)?;
    let projected = match program.runtime_types.get(target_ty.index()) {
        Some(AwbcRuntimeType::Tuple(items)) if tuple => items.get(ordinal as usize).copied(),
        Some(AwbcRuntimeType::Record { fields, .. }) if !tuple => {
            fields.get(ordinal as usize).map(|field| field.ty)
        }
        Some(AwbcRuntimeType::Dynamic) => Some(target_ty),
        _ => None,
    }
    .ok_or_else(|| AwbcVerifyError::IndexOutOfBounds {
        table: if tuple {
            "tuple fields"
        } else {
            "record fields"
        },
        index: ordinal,
        at: at.to_owned(),
    })?;
    require_compatible(program, dst_ty, projected, at)?;
    write_register(verifier, function, block, dst, state)
}

fn read_register(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    register: AwbcRegisterId,
    state: &FlowState,
) -> Result<AwbcTypeId, AwbcVerifyError> {
    let ty = register_type(verifier, function, block, register)?;
    if !state.initialized[register.index()] {
        return Err(AwbcVerifyError::UninitializedRegister {
            function,
            block,
            register: register.0,
        });
    }
    Ok(ty)
}

fn write_register(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    register: AwbcRegisterId,
    state: &mut FlowState,
) -> Result<(), AwbcVerifyError> {
    register_type(verifier, function, block, register)?;
    state.initialized[register.index()] = true;
    Ok(())
}

fn clear_register(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    register: AwbcRegisterId,
    state: &mut FlowState,
) -> Result<(), AwbcVerifyError> {
    register_type(verifier, function, block, register)?;
    state.initialized[register.index()] = false;
    Ok(())
}

fn register_type(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    register: AwbcRegisterId,
) -> Result<AwbcTypeId, AwbcVerifyError> {
    let layout = function_layout(verifier, function);
    layout
        .slots
        .get(register.index())
        .map(|slot| slot.ty)
        .ok_or(AwbcVerifyError::RegisterOutOfBounds {
            function,
            block,
            register: register.0,
        })
}

fn function_layout<'a>(verifier: &'a Verifier<'_, '_>, function: usize) -> &'a AwbcFrameLayout {
    &verifier.program.frame_layouts[verifier.program.functions[function].frame_layout.index()]
}

fn require_type_kind(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    register: AwbcRegisterId,
    predicate: fn(Option<&AwbcRuntimeType>) -> bool,
    label: &str,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    let ty = register_type(verifier, function, block, register)?;
    if predicate(verifier.program.runtime_types.get(ty.index())) {
        Ok(())
    } else {
        invalid_type(at, label)
    }
}

fn require_compatible(
    program: &AwbcProgram,
    expected: AwbcTypeId,
    actual: AwbcTypeId,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    if types_compatible(program, expected, actual) {
        Ok(())
    } else {
        type_mismatch(at, expected, actual)
    }
}

fn type_mismatch<T>(
    at: &str,
    expected: AwbcTypeId,
    actual: AwbcTypeId,
) -> Result<T, AwbcVerifyError> {
    Err(AwbcVerifyError::TypeMismatch {
        at: at.to_owned(),
        expected: expected.0,
        actual: actual.0,
    })
}

fn argument_count<T>(at: &str, expected: usize, actual: usize) -> Result<T, AwbcVerifyError> {
    Err(AwbcVerifyError::ArgumentCountMismatch {
        at: at.to_owned(),
        expected,
        actual,
    })
}

fn invalid_type<T>(at: &str, message: &str) -> Result<T, AwbcVerifyError> {
    Err(AwbcVerifyError::InvalidInvariant {
        at: at.to_owned(),
        message: message.to_owned(),
    })
}

fn constant_matches_type(
    program: &AwbcProgram,
    constant: &AwbcConstant,
    ty: AwbcTypeId,
    depth: usize,
) -> bool {
    if depth > 64 {
        return false;
    }
    let Some(ty_layout) = program.runtime_types.get(ty.index()) else {
        return false;
    };
    if matches!(ty_layout, AwbcRuntimeType::Dynamic) {
        return true;
    }
    match (constant, ty_layout) {
        (AwbcConstant::Unit, AwbcRuntimeType::Unit)
        | (AwbcConstant::Bool(_), AwbcRuntimeType::Bool)
        | (AwbcConstant::F32Bits(_), AwbcRuntimeType::F32)
        | (AwbcConstant::F64Bits(_), AwbcRuntimeType::F64)
        | (AwbcConstant::String(_), AwbcRuntimeType::String)
        | (AwbcConstant::Char(_), AwbcRuntimeType::Char)
        | (AwbcConstant::DurationNanos(_), AwbcRuntimeType::Duration)
        | (AwbcConstant::EntityRef(_), AwbcRuntimeType::EntityRef)
        | (AwbcConstant::Bytes(_), AwbcRuntimeType::Sequence(_))
        | (AwbcConstant::TensorF32 { .. }, AwbcRuntimeType::MatrixF32)
        | (AwbcConstant::TensorF32 { .. }, AwbcRuntimeType::TensorF32)
        | (AwbcConstant::TensorF64 { .. }, AwbcRuntimeType::MatrixF64)
        | (AwbcConstant::TensorF64 { .. }, AwbcRuntimeType::TensorF64) => true,
        (AwbcConstant::Int { kind, .. }, AwbcRuntimeType::Int(expected)) => *kind == *expected,
        (AwbcConstant::UInt { kind, .. }, AwbcRuntimeType::UInt(expected)) => *kind == *expected,
        (AwbcConstant::Tuple(values), AwbcRuntimeType::Tuple(types)) => {
            values.len() == types.len()
                && values.iter().zip(types).all(|(value, ty)| {
                    program
                        .constants
                        .get(value.index())
                        .is_some_and(|value| constant_matches_type(program, value, *ty, depth + 1))
                })
        }
        (AwbcConstant::Sequence(values), AwbcRuntimeType::Sequence(item_ty)) => {
            values.iter().all(|value| {
                program
                    .constants
                    .get(value.index())
                    .is_some_and(|value| constant_matches_type(program, value, *item_ty, depth + 1))
            })
        }
        (AwbcConstant::Record { ty: actual, .. }, AwbcRuntimeType::Record { .. }) => actual == &ty,
        (AwbcConstant::Variant { ty: actual, .. }, AwbcRuntimeType::Variant { .. }) => {
            actual == &ty
        }
        (AwbcConstant::Opaque { ty: actual, .. }, AwbcRuntimeType::Opaque { .. }) => {
            types_compatible(program, ty, *actual)
        }
        _ => false,
    }
}

fn dynamic_type(program: &AwbcProgram) -> Option<AwbcTypeId> {
    program
        .runtime_types
        .iter()
        .position(|ty| matches!(ty, AwbcRuntimeType::Dynamic))
        .and_then(|index| u32::try_from(index).ok())
        .map(AwbcTypeId)
}

fn is_bool(ty: Option<&AwbcRuntimeType>) -> bool {
    matches!(ty, Some(AwbcRuntimeType::Bool | AwbcRuntimeType::Dynamic))
}

fn is_integer(ty: Option<&AwbcRuntimeType>) -> bool {
    matches!(
        ty,
        Some(AwbcRuntimeType::Int(_) | AwbcRuntimeType::UInt(_) | AwbcRuntimeType::Dynamic)
    )
}

fn is_numeric(ty: Option<&AwbcRuntimeType>) -> bool {
    matches!(
        ty,
        Some(
            AwbcRuntimeType::Int(_)
                | AwbcRuntimeType::UInt(_)
                | AwbcRuntimeType::F32
                | AwbcRuntimeType::F64
                | AwbcRuntimeType::Dynamic
        )
    )
}

fn is_sequence_or_dynamic(ty: Option<&AwbcRuntimeType>) -> bool {
    matches!(
        ty,
        Some(AwbcRuntimeType::Sequence(_) | AwbcRuntimeType::Dynamic)
    )
}

fn is_dynamic(ty: Option<&AwbcRuntimeType>) -> bool {
    matches!(ty, Some(AwbcRuntimeType::Dynamic))
}

fn is_await_handle(ty: Option<&AwbcRuntimeType>) -> bool {
    matches!(
        ty,
        Some(AwbcRuntimeType::TaskHandle | AwbcRuntimeType::NeedHandle | AwbcRuntimeType::Dynamic)
    )
}

fn is_task_handle(ty: Option<&AwbcRuntimeType>) -> bool {
    matches!(
        ty,
        Some(AwbcRuntimeType::TaskHandle | AwbcRuntimeType::Dynamic)
    )
}

fn is_dynamic_target(ty: Option<&AwbcRuntimeType>) -> bool {
    matches!(
        ty,
        Some(AwbcRuntimeType::String | AwbcRuntimeType::EntityRef | AwbcRuntimeType::Dynamic)
    )
}

fn is_choice_value(ty: Option<&AwbcRuntimeType>) -> bool {
    matches!(
        ty,
        Some(AwbcRuntimeType::String | AwbcRuntimeType::UInt(_) | AwbcRuntimeType::Dynamic)
    )
}
