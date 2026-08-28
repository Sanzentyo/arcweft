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
    AwbcBinaryOp, AwbcBindMode, AwbcBlockId, AwbcConstant, AwbcDialogueValueRole, AwbcDropPolicy,
    AwbcEffectSetId, AwbcFrameLayout, AwbcFrameSlotRole, AwbcFunctionFlag, AwbcFunctionKind,
    AwbcInstruction, AwbcPattern, AwbcPatternId, AwbcPatternRest, AwbcProgram, AwbcRegisterId,
    AwbcResumePointId, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcSafePointKind, AwbcScopeId,
    AwbcSignatureId, AwbcTerminator, AwbcTraitReceiverMode, AwbcTypeId, AwbcUnaryOp,
    AwbcUnsignedIntKind, AwbcVariantIdentity,
};
use crate::value::{
    RuntimeAgentField, RuntimeAgentFieldResult, RuntimeAgentFieldValue, RuntimeReductionProducer,
};
use std::collections::{BTreeSet, VecDeque};

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
        AwbcFunctionKind::Ordinary
        | AwbcFunctionKind::PureHelper
        | AwbcFunctionKind::TraitMethod
        | AwbcFunctionKind::Synthetic
        | AwbcFunctionKind::GeneratorProducer
        | AwbcFunctionKind::StreamTransform
        | AwbcFunctionKind::LineActivation
        | AwbcFunctionKind::LineTask => AwbcSafePointKind::CallableBoundary,
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
            if dst != src {
                clear_register(verifier, function, block, *src, state)?;
            }
        }
        AwbcInstruction::CopyValue { dst, src } => {
            let src_ty = read_register(verifier, function, block, *src, state)?;
            if !runtime_type_permits_copy(program, src_ty, 0) {
                return invalid_type(&at, "recursively unrestricted CopyValue source");
            }
            let dst_ty = register_type(verifier, function, block, *dst)?;
            require_compatible(program, dst_ty, src_ty, &at)?;
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::Clear { register } => {
            let ty = read_register(verifier, function, block, *register, state)?;
            if !runtime_type_permits_copy(program, ty, 0) {
                return invalid_type(&at, "recursively unrestricted Clear source");
            }
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
            let Some(AwbcRuntimeTypeShape::Tuple(types)) = runtime_shape(program, dst_ty) else {
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
            let (item_ty, expected_len) = match runtime_shape(program, dst_ty) {
                Some(AwbcRuntimeTypeShape::Sequence(item_ty)) => (*item_ty, None),
                Some(AwbcRuntimeTypeShape::Array { item, length }) => {
                    let expected = usize::try_from(*length)
                        .map_err(|_| AwbcVerifyError::ResultShapeMismatch { at: at.clone() })?;
                    (*item, Some(expected))
                }
                _ => return invalid_type(&at, "sequence destination"),
            };
            if expected_len.is_some_and(|expected| expected != items.len()) {
                return argument_count(&at, expected_len.unwrap_or_default(), items.len());
            }
            for item in items {
                let actual = read_register(verifier, function, block, *item, state)?;
                require_compatible(program, item_ty, actual, &at)?;
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::RepeatSequence { dst, value, len } => {
            let dst_ty = register_type(verifier, function, block, *dst)?;
            let Some(AwbcRuntimeTypeShape::Sequence(item_ty)) = runtime_shape(program, dst_ty)
            else {
                return invalid_type(&at, "sequence destination");
            };
            let value_ty = read_register(verifier, function, block, *value, state)?;
            require_compatible(program, *item_ty, value_ty, &at)?;
            let len_ty = read_register(verifier, function, block, *len, state)?;
            if !is_integer(runtime_shape(program, len_ty)) {
                return invalid_type(&at, "integer repeat length");
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::SequenceLen { dst, sequence } => {
            let sequence_ty = read_register(verifier, function, block, *sequence, state)?;
            if !is_sequence_or_dynamic(runtime_shape(program, sequence_ty)) {
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
            if !is_integer(runtime_shape(program, index_ty)) {
                return invalid_type(&at, "integer sequence index");
            }
            let dst_ty = register_type(verifier, function, block, *dst)?;
            match runtime_shape(program, sequence_ty) {
                Some(AwbcRuntimeTypeShape::Sequence(item_ty)) => {
                    require_compatible(program, dst_ty, *item_ty, &at)?;
                }
                Some(AwbcRuntimeTypeShape::Array { item, .. }) => {
                    require_compatible(program, dst_ty, *item, &at)?;
                }
                Some(AwbcRuntimeTypeShape::Dynamic) => {}
                _ => return invalid_type(&at, "sequence input"),
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
            if !is_integer(runtime_shape(program, start_ty)) {
                return invalid_type(&at, "integer sequence slice start");
            }
            let dst_ty = register_type(verifier, function, block, *dst)?;
            require_compatible(program, dst_ty, sequence_ty, &at)?;
            if !is_sequence_or_dynamic(runtime_shape(program, sequence_ty)) {
                return invalid_type(&at, "sequence input");
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::SequencePush { sequence, value } => {
            let sequence_ty = read_register(verifier, function, block, *sequence, state)?;
            let value_ty = read_register(verifier, function, block, *value, state)?;
            if let Some(AwbcRuntimeTypeShape::Sequence(item_ty)) =
                runtime_shape(program, sequence_ty)
            {
                require_compatible(program, *item_ty, value_ty, &at)?;
            } else if !is_dynamic(runtime_shape(program, sequence_ty)) {
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
            match runtime_shape(program, *ty) {
                Some(
                    AwbcRuntimeTypeShape::Record {
                        fields: type_fields,
                        ..
                    }
                    | AwbcRuntimeTypeShape::NominalRecord {
                        fields: type_fields,
                        ..
                    },
                ) => {
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
                Some(AwbcRuntimeTypeShape::Dynamic) => {
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
            match runtime_shape(program, *ty) {
                Some(AwbcRuntimeTypeShape::Variant { cases, .. }) => {
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
        AwbcInstruction::MakeAgent {
            dst,
            constructor,
            operands,
        } => {
            if !constructor.accepts_operand_count(operands.len()) {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: format!(
                        "Agent constructor {constructor:?} rejects {} operand(s)",
                        operands.len()
                    ),
                });
            }
            for (ordinal, operand) in operands.iter().enumerate() {
                let ty = read_register(verifier, function, block, *operand, state)?;
                if !agent_operand_type_is_valid(program, *constructor, ordinal, ty) {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: format!(
                            "Agent constructor {constructor:?} rejects operand {ordinal} runtime type"
                        ),
                    });
                }
            }
            let dst_ty = register_type(verifier, function, block, *dst)?;
            match runtime_shape(program, dst_ty) {
                Some(AwbcRuntimeTypeShape::Agent(actual))
                    if actual.operational_type() == constructor.result_type() => {}
                Some(AwbcRuntimeTypeShape::Dynamic) => {}
                _ => return invalid_type(&at, "Agent constructor destination"),
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::MakeReductionUnchanged {
            dst,
            ty,
            state: value,
        } => {
            let Some(AwbcRuntimeTypeShape::Opaque {
                admission,
                arguments,
                ..
            }) = runtime_shape(program, *ty)
            else {
                return invalid_type(&at, "Reduction opaque type");
            };
            let Some(owner) = program
                .runtime_types
                .get(ty.index())
                .and_then(|row| row.try_opaque_owner(&program.strings).ok().flatten())
            else {
                return invalid_type(&at, "Reduction opaque owner");
            };
            if *admission != crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity
                || !RuntimeReductionProducer::accepts(owner.producer())
                || arguments.len() != 1
            {
                return invalid_type(&at, "exact std.reduction opaque type with one argument");
            }
            let state_ty = read_register(verifier, function, block, *value, state)?;
            require_compatible(program, arguments[0], state_ty, &at)?;
            let dst_ty = register_type(verifier, function, block, *dst)?;
            require_compatible(program, dst_ty, *ty, &at)?;
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
        AwbcInstruction::ProjectField { dst, target, field } => match field {
            crate::awbc::schema::AwbcFieldProjection::Named(field) => {
                check_string(program, *field, &at)?;
                let target_ty = read_register(verifier, function, block, *target, state)?;
                let dst_ty = register_type(verifier, function, block, *dst)?;
                match runtime_shape(program, target_ty) {
                    Some(
                        AwbcRuntimeTypeShape::Record { fields, .. }
                        | AwbcRuntimeTypeShape::NominalRecord { fields, .. },
                    ) => {
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
                    Some(AwbcRuntimeTypeShape::Dynamic) => {}
                    Some(AwbcRuntimeTypeShape::Progress) => {
                        let label = program
                            .strings
                            .get(field.index())
                            .map(String::as_str)
                            .unwrap_or_default();
                        let destination = runtime_shape(program, dst_ty);
                        let destination_matches = match label {
                            "ratio" => matches!(destination, Some(AwbcRuntimeTypeShape::F32)),
                            "label" => matches!(
                                destination,
                                Some(AwbcRuntimeTypeShape::Variant {
                                    owner: crate::awbc::schema::AwbcVariantIdentity::Builtin(
                                        crate::pattern::RuntimeBuiltinVariantIdentity::Option
                                    ),
                                    cases,
                                    ..
                                }) if cases.first().and_then(|case| case.payload).is_some_and(|item| {
                                    matches!(
                                        runtime_shape(program, item),
                                        Some(AwbcRuntimeTypeShape::String)
                                    )
                                })
                            ),
                            _ => false,
                        };
                        if !destination_matches {
                            return invalid_type(&at, "Progress field projection destination");
                        }
                    }
                    Some(AwbcRuntimeTypeShape::Agent(agent)) => {
                        let label = program
                            .strings
                            .get(field.index())
                            .map(String::as_str)
                            .unwrap_or_default();
                        let Some(field) =
                            RuntimeAgentField::from_owner_label(agent.operational_type(), label)
                        else {
                            return Err(AwbcVerifyError::InvalidInvariant {
                                at,
                                message: "projected Agent field does not exist".to_owned(),
                            });
                        };
                        let destination = runtime_shape(program, dst_ty);
                        let destination_matches = match field.result() {
                            RuntimeAgentFieldResult::Required(value) => {
                                agent_field_value_destination_matches(program, destination, value)
                            }
                            RuntimeAgentFieldResult::Optional(value) => match destination {
                                Some(AwbcRuntimeTypeShape::Variant {
                                    owner:
                                        crate::awbc::schema::AwbcVariantIdentity::Builtin(
                                            crate::pattern::RuntimeBuiltinVariantIdentity::Option,
                                        ),
                                    cases,
                                    ..
                                }) => cases.first().and_then(|case| case.payload).is_some_and(
                                    |item| {
                                        agent_field_value_destination_matches(
                                            program,
                                            runtime_shape(program, item),
                                            value,
                                        )
                                    },
                                ),
                                Some(AwbcRuntimeTypeShape::Dynamic) => true,
                                _ => false,
                            },
                        };
                        if !destination_matches {
                            return invalid_type(&at, "Agent field projection destination");
                        }
                    }
                    _ => return invalid_type(&at, "record projection target"),
                }
                write_register(verifier, function, block, *dst, state)?;
            }
            crate::awbc::schema::AwbcFieldProjection::OpaqueRecord {
                owner,
                field: _,
                field_type,
            } => {
                check_index(program.runtime_types.len(), owner.0, "runtime_types", &at)?;
                check_index(
                    program.runtime_types.len(),
                    field_type.0,
                    "runtime_types",
                    &at,
                )?;
                let target_ty = read_register(verifier, function, block, *target, state)?;
                if target_ty != *owner {
                    return type_mismatch(&at, *owner, target_ty);
                }
                let exact_owner = program
                    .runtime_types
                    .get(owner.index())
                    .and_then(|row| row.try_opaque_owner(&program.strings).ok().flatten())
                    .is_some_and(|owner| {
                        owner.admission()
                            == crate::pattern::RuntimeOpaqueTypeAdmission::ExactIdentity
                    });
                if !exact_owner {
                    return invalid_type(&at, "exact opaque-record projection owner");
                }
                let dst_ty = register_type(verifier, function, block, *dst)?;
                if dst_ty != *field_type {
                    return type_mismatch(&at, *field_type, dst_ty);
                }
                write_register(verifier, function, block, *dst, state)?;
            }
        },
        AwbcInstruction::Unary { dst, op, src } => {
            let src_ty = read_register(verifier, function, block, *src, state)?;
            let dst_ty = register_type(verifier, function, block, *dst)?;
            match op {
                AwbcUnaryOp::Not => {
                    if !is_bool(runtime_shape(program, src_ty))
                        || !is_bool(runtime_shape(program, dst_ty))
                    {
                        return invalid_type(&at, "bool unary operands");
                    }
                }
                AwbcUnaryOp::Neg => {
                    if !is_numeric(runtime_shape(program, src_ty)) {
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
                    if !is_bool(runtime_shape(program, dst_ty)) {
                        return invalid_type(&at, "bool comparison destination");
                    }
                }
                AwbcBinaryOp::Add | AwbcBinaryOp::Sub | AwbcBinaryOp::Mul | AwbcBinaryOp::Div => {
                    if !is_numeric(runtime_shape(program, lhs_ty)) {
                        return invalid_type(&at, "numeric binary operands");
                    }
                    require_compatible(program, dst_ty, lhs_ty, &at)?;
                }
                AwbcBinaryOp::And | AwbcBinaryOp::Or => {
                    if !is_bool(runtime_shape(program, lhs_ty))
                        || !is_bool(runtime_shape(program, dst_ty))
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
        AwbcInstruction::AssignRecordField {
            target,
            field,
            value,
        } => {
            let target_ty = read_register(verifier, function, block, *target, state)?;
            let value_ty = read_register(verifier, function, block, *value, state)?;
            match runtime_shape(program, target_ty) {
                Some(AwbcRuntimeTypeShape::Record { fields, .. }) => {
                    let Some(field_layout) = fields.get(*field as usize) else {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at,
                            message: "assigned field does not exist".to_owned(),
                        });
                    };
                    require_compatible(program, field_layout.ty, value_ty, "field assignment")?;
                }
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
                &format!("intrinsic {}", intrinsic.identity),
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
                &format!("effect plan {}", effect.kind.encoded()),
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
                &format!("cleanup effect plan {}", effect.kind.encoded()),
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
        AwbcInstruction::ExecuteLineOperation {
            dst,
            operation,
            args,
        } => {
            check_index(
                program.line_operations.len(),
                operation.0,
                "line_operations",
                &at,
            )?;
            let group = line_group_for_function(program, function, &at)?;
            let operation = &program.line_operations[operation.index()];
            if program
                .line_task_groups
                .get(operation.group().index())
                .is_none_or(|owner| !std::ptr::eq(owner, group))
            {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "line operation is referenced outside its owning group".to_owned(),
                });
            }
            let site = group
                .handle_sites
                .get(operation.site().index())
                .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "line operation references a site outside its owning group".to_owned(),
                })?;
            if site.result_type != operation.result_type() {
                return type_mismatch(&at, site.result_type, operation.result_type());
            }
            let dst_ty = register_type(verifier, function, block, *dst)?;
            require_compatible(program, operation.result_type(), dst_ty, &at)?;
            match operation {
                crate::awbc::schema::AwbcLineOperation::AcquireActor {
                    character,
                    scope: crate::line_task::RuntimeLineHandleScope::Line,
                    ..
                } => {
                    if !args.is_empty()
                        || site.kind != crate::value::RuntimeHandleKind::StageActor
                        || site.character.as_ref() != Some(character)
                        || site.scheduled_child.is_some()
                    {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "AcquireActor ABI does not match its handle site".to_owned(),
                        });
                    }
                }
                crate::awbc::schema::AwbcLineOperation::Schedule {
                    child, captures, ..
                } => {
                    if args.len() != captures.len().saturating_add(1)
                        || site.kind != crate::value::RuntimeHandleKind::Cue
                        || site.character.is_some()
                        || site.scheduled_child != Some(*child)
                    {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "Schedule ABI does not match its handle site".to_owned(),
                        });
                    }
                    let delay = read_register(verifier, function, block, args[0], state)?;
                    if !matches!(
                        runtime_shape(program, delay),
                        Some(AwbcRuntimeTypeShape::Duration)
                    ) {
                        return invalid_type(&at, "Schedule Duration argument");
                    }
                    let mut capture_locals = BTreeSet::new();
                    for (argument, capture) in args[1..].iter().zip(captures) {
                        if !capture_locals.insert(capture.local) {
                            return Err(AwbcVerifyError::InvalidInvariant {
                                at: at.clone(),
                                message: "Schedule capture destination local is duplicated"
                                    .to_owned(),
                            });
                        }
                        let actual = read_register(verifier, function, block, *argument, state)?;
                        require_compatible(program, capture.ty, actual, &at)?;
                    }
                }
                crate::awbc::schema::AwbcLineOperation::ActorLook {
                    character,
                    actor_type,
                    look_type,
                    ..
                } => {
                    if args.len() != 3
                        || site.kind != crate::value::RuntimeHandleKind::Cue
                        || site.character.as_ref() != Some(character)
                        || site.scheduled_child.is_some()
                    {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "ActorLook ABI does not match its handle site".to_owned(),
                        });
                    }
                    let actor = read_register(verifier, function, block, args[0], state)?;
                    let look = read_register(verifier, function, block, args[1], state)?;
                    let crossfade = read_register(verifier, function, block, args[2], state)?;
                    require_compatible(program, *actor_type, actor, &at)?;
                    require_compatible(program, *look_type, look, &at)?;
                    if !matches!(
                        runtime_shape(program, crossfade),
                        Some(AwbcRuntimeTypeShape::Duration)
                    ) {
                        return invalid_type(&at, "ActorLook crossfade Duration");
                    }
                }
                crate::awbc::schema::AwbcLineOperation::VoiceHandle { .. } => {
                    if !args.is_empty()
                        || site.kind != crate::value::RuntimeHandleKind::Voice
                        || site.character.is_some()
                        || site.scheduled_child.is_some()
                    {
                        return Err(AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "VoiceHandle ABI does not match its handle site".to_owned(),
                        });
                    }
                }
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::CommitDialogueResult { source } => {
            let group = line_group_for_function(program, function, &at)?;
            if program.functions[function].kind != AwbcFunctionKind::LineActivation
                || group.activation.index() != function
            {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "CommitDialogueResult is outside its owning activation function"
                        .to_owned(),
                });
            }
            let source_ty = read_register(verifier, function, block, *source, state)?;
            require_compatible(program, group.result_type, source_ty, &at)?;
        }
        AwbcInstruction::Drop { register, policy } => {
            read_register(verifier, function, block, *register, state)?;
            if let AwbcDropPolicy::Stop { fade } = policy {
                let fade_ty = read_register(verifier, function, block, *fade, state)?;
                if !matches!(
                    runtime_shape(program, fade_ty),
                    Some(AwbcRuntimeTypeShape::Duration)
                ) {
                    return invalid_type(&at, "Duration Drop Stop fade register");
                }
            }
            clear_register(verifier, function, block, *register, state)?;
        }
    }
    Ok(())
}

fn agent_operand_type_is_valid(
    program: &AwbcProgram,
    constructor: crate::value::RuntimeAgentConstructor,
    ordinal: usize,
    ty: AwbcTypeId,
) -> bool {
    use crate::plan::RuntimeAgentOperationalType as AgentType;
    use crate::value::RuntimeAgentConstructor as Constructor;

    let Some(ty) = runtime_shape(program, ty) else {
        return false;
    };
    if matches!(ty, AwbcRuntimeTypeShape::Dynamic) {
        return true;
    }
    match constructor {
        Constructor::CaptureViewport | Constructor::Diagnostics => false,
        Constructor::ChoiceAction | Constructor::CaptureLayer | Constructor::CaptureObject => {
            matches!(
                ty,
                AwbcRuntimeTypeShape::String | AwbcRuntimeTypeShape::EntityRef
            )
        }
        Constructor::StatePath | Constructor::ObservationPath => {
            matches!(ty, AwbcRuntimeTypeShape::String)
        }
        Constructor::ProbeSignal | Constructor::ProbeMetric => {
            matches!(
                ty,
                AwbcRuntimeTypeShape::String | AwbcRuntimeTypeShape::EntityRef
            )
        }
        Constructor::ProbeState => agent_operational_type_is(ty, AgentType::DebugStatePath),
        Constructor::ProbeObservation => {
            agent_operational_type_is(ty, AgentType::ObservationFieldPath)
        }
        Constructor::PredicateExists => agent_operational_type_is(ty, AgentType::Probe),
        Constructor::PredicateActionEnabled => {
            agent_operational_type_is(ty, AgentType::ActionTarget)
        }
        Constructor::PredicateDiagnosticsHasError => {
            agent_operational_type_is(ty, AgentType::Diagnostics)
        }
        Constructor::PredicateAll | Constructor::PredicateAny => {
            agent_predicate_collection_operand_type_is_valid(program, ty)
        }
        Constructor::PredicateNot => agent_operational_type_is(ty, AgentType::Predicate),
        Constructor::PredicateEq
        | Constructor::PredicateNotEq
        | Constructor::PredicateGreater
        | Constructor::PredicateGreaterOrEqual
        | Constructor::PredicateLess
        | Constructor::PredicateLessOrEqual => {
            ordinal != 0 || agent_operational_type_is(ty, AgentType::Probe)
        }
        Constructor::ViewportPoint => {
            matches!(
                ty,
                AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::U32)
                    | AwbcRuntimeTypeShape::Dynamic
            )
        }
    }
}

fn agent_predicate_collection_operand_type_is_valid(
    program: &AwbcProgram,
    ty: &AwbcRuntimeTypeShape,
) -> bool {
    use crate::plan::RuntimeAgentOperationalType as AgentType;

    let predicate_item = |ty: AwbcTypeId| {
        runtime_shape(program, ty).is_some_and(|ty| {
            matches!(ty, AwbcRuntimeTypeShape::Dynamic)
                || agent_operational_type_is(ty, AgentType::Predicate)
        })
    };
    match ty {
        AwbcRuntimeTypeShape::Agent(agent) if agent.operational_type() == AgentType::Predicate => {
            true
        }
        AwbcRuntimeTypeShape::Dynamic => true,
        AwbcRuntimeTypeShape::Sequence(item) => predicate_item(*item),
        AwbcRuntimeTypeShape::Tuple(items) => {
            !items.is_empty() && items.iter().copied().all(predicate_item)
        }
        _ => false,
    }
}

fn agent_operational_type_is(
    ty: &AwbcRuntimeTypeShape,
    expected: crate::plan::RuntimeAgentOperationalType,
) -> bool {
    matches!(
        ty,
        AwbcRuntimeTypeShape::Agent(actual) if actual.operational_type() == expected
    )
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
            if !is_bool(runtime_shape(program, condition_ty)) {
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
                            .is_none_or(|ty| !is_bool(runtime_shape(program, ty)))
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
            if !is_dynamic_target(runtime_shape(program, target_ty)) {
                return invalid_type(&at, "dynamic target string/entity/dynamic");
            }
            check_args_budget(verifier, args.len())?;
            for arg in args {
                read_register(verifier, function, block, *arg, state)?;
            }
            if !program.functions[function]
                .flags
                .contains(AwbcFunctionFlag::HasDynamicTarget)
            {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "dynamic goto requires HAS_DYNAMIC_TARGET function flag".to_owned(),
                });
            }
        }
        AwbcTerminator::Dialogue {
            content,
            values,
            line_task_captures,
            result,
            resume,
        } => {
            check_index(program.content_units.len(), content.0, "content_units", &at)?;
            let group = program.content_units[content.index()]
                .line_task_group
                .and_then(|group| program.line_task_groups.get(group.index()));
            if group.is_some_and(|group| group.captures.len() != line_task_captures.len()) {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "dialogue line-task capture arity disagrees with its content group"
                        .to_owned(),
                });
            }
            if group.is_none() && !line_task_captures.is_empty() {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "dialogue without a content-owned line-task group carries captures"
                        .to_owned(),
                });
            }
            let Some(group) = group else {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "typed dialogue result requires a content-owned line-task group"
                        .to_owned(),
                });
            };
            if result.ty != group.result_type {
                return type_mismatch(&at, group.result_type, result.ty);
            }
            let destination_ty = register_type(verifier, function, block, result.destination)?;
            require_compatible(program, result.ty, destination_ty, &at)?;
            let mut next = state.clone();
            validate_pattern(
                verifier,
                function,
                block,
                result.pattern,
                result.ty,
                Some(AwbcBindMode::Declare),
                &mut next,
                0,
            )?;
            let capture_types = program
                .functions
                .get(group.activation.index())
                .and_then(|function| program.signatures.get(function.signature.index()))
                .map(|signature| signature.params.as_slice())
                .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "line activation capture signature is absent".to_owned(),
                })?;
            for (capture, expected) in line_task_captures.iter().zip(capture_types) {
                let actual = read_register(verifier, function, block, *capture, state)?;
                require_compatible(program, *expected, actual, &at)?;
                if !runtime_type_permits_copy(program, actual, 0) {
                    return invalid_type(&at, "recursively unrestricted line-task group capture");
                }
            }
            for (index, binding) in values.iter().enumerate() {
                let expected =
                    crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(index)
                        .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "dialogue value slot count exceeds u32".to_owned(),
                        })?;
                if binding.slot != expected {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "dialogue value slots are not canonical and contiguous".to_owned(),
                    });
                }
                let ty = read_register(verifier, function, block, binding.value, state)?;
                if binding.role == AwbcDialogueValueRole::Condition
                    && !is_bool(runtime_shape(program, ty))
                {
                    return invalid_type(&at, "dialogue condition Bool");
                }
            }
            successors.push((
                verify_resume(
                    verifier,
                    function,
                    *resume,
                    AwbcSafePointKind::Dialogue,
                    &at,
                )?,
                next,
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
            observer,
            resume,
        } => {
            let handle_ty = read_register(verifier, function, block, *handle, state)?;
            if !is_await_handle(runtime_shape(program, handle_ty)) {
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
            if let Some(observer) = observer {
                require_type_kind(
                    verifier,
                    function,
                    block,
                    observer.destination,
                    is_progress,
                    "await Progress observer",
                    &at,
                )?;
                let mut pending = state.clone();
                write_register(
                    verifier,
                    function,
                    block,
                    observer.destination,
                    &mut pending,
                )?;
                successors.push((
                    verify_resume(
                        verifier,
                        function,
                        observer.resume,
                        AwbcSafePointKind::Await,
                        &at,
                    )?,
                    pending,
                ));
            }
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
            if !is_sequence_or_dynamic(runtime_shape(program, source_ty)) {
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
    if depth == 0 {
        validate_unique_pattern_binding_targets(
            verifier.program,
            pattern,
            pattern,
            &mut BTreeSet::new(),
            0,
            verifier.budget.pattern_depth,
        )?;
    }
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
                runtime_shape(program, value_ty),
                Some(AwbcRuntimeTypeShape::EntityRef | AwbcRuntimeTypeShape::Dynamic)
            ) {
                return invalid_type("entity pattern", "entity reference");
            }
        }
        AwbcPattern::Tuple(items) => match runtime_shape(program, value_ty) {
            Some(AwbcRuntimeTypeShape::Tuple(types)) => {
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
            }
            Some(AwbcRuntimeTypeShape::Dynamic) => {
                for child in items {
                    validate_pattern(
                        verifier,
                        function,
                        block,
                        *child,
                        value_ty,
                        mode,
                        state,
                        depth + 1,
                    )?;
                }
            }
            _ => return invalid_type("tuple pattern", "tuple scrutinee"),
        },
        AwbcPattern::Record { ty, fields, rest } => {
            if let Some(expected) = ty {
                require_compatible(program, *expected, value_ty, "record pattern")?;
            }
            let record_ty = ty.unwrap_or(value_ty);
            match runtime_shape(program, record_ty) {
                Some(
                    AwbcRuntimeTypeShape::Record {
                        fields: type_fields,
                        ..
                    }
                    | AwbcRuntimeTypeShape::NominalRecord {
                        fields: type_fields,
                        ..
                    },
                ) => {
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
                Some(AwbcRuntimeTypeShape::Dynamic) if ty.is_none() => {
                    for field in fields {
                        validate_pattern(
                            verifier,
                            function,
                            block,
                            field.pattern,
                            value_ty,
                            mode,
                            state,
                            depth + 1,
                        )?;
                    }
                }
                _ => return invalid_type("record pattern", "typed record scrutinee"),
            }
            if let AwbcPatternRest::Bind(rest) = rest {
                let rest_ty = register_type(verifier, function, block, *rest)?;
                require_compatible(program, rest_ty, value_ty, "record rest binding")?;
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
        AwbcPattern::Sequence { items, rest } => {
            let item_ty = match runtime_shape(program, value_ty) {
                Some(AwbcRuntimeTypeShape::Sequence(item_ty)) => Some(*item_ty),
                Some(AwbcRuntimeTypeShape::Dynamic) => dynamic_type(program),
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
            if let AwbcPatternRest::Bind(rest) = rest {
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
            match runtime_shape(program, *ty) {
                Some(AwbcRuntimeTypeShape::Variant { cases, .. }) => {
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
    }
    Ok(())
}

fn validate_unique_pattern_binding_targets(
    program: &AwbcProgram,
    root: AwbcPatternId,
    pattern: AwbcPatternId,
    targets: &mut BTreeSet<AwbcRegisterId>,
    depth: usize,
    limit: usize,
) -> Result<(), AwbcVerifyError> {
    if depth > limit {
        return Err(AwbcVerifyError::PatternDepthExceeded {
            pattern: pattern.index(),
            limit,
        });
    }
    let record = &program.patterns[pattern.index()];
    match record {
        AwbcPattern::Bind { target, .. } => insert_pattern_binding(root, *target, targets),
        AwbcPattern::Tuple(children)
        | AwbcPattern::Sequence {
            items: children,
            rest: AwbcPatternRest::Exact | AwbcPatternRest::Ignore,
        } => {
            for child in children {
                validate_unique_pattern_binding_targets(
                    program,
                    root,
                    *child,
                    targets,
                    depth + 1,
                    limit,
                )?;
            }
            Ok(())
        }
        AwbcPattern::Sequence {
            items,
            rest: AwbcPatternRest::Bind(target),
        } => {
            for child in items {
                validate_unique_pattern_binding_targets(
                    program,
                    root,
                    *child,
                    targets,
                    depth + 1,
                    limit,
                )?;
            }
            insert_pattern_binding(root, *target, targets)
        }
        AwbcPattern::Record { fields, rest, .. } => {
            for field in fields {
                validate_unique_pattern_binding_targets(
                    program,
                    root,
                    field.pattern,
                    targets,
                    depth + 1,
                    limit,
                )?;
            }
            if let AwbcPatternRest::Bind(target) = rest {
                insert_pattern_binding(root, *target, targets)?;
            }
            Ok(())
        }
        AwbcPattern::Variant {
            payload: Some(payload),
            ..
        } => validate_unique_pattern_binding_targets(
            program,
            root,
            *payload,
            targets,
            depth + 1,
            limit,
        ),
        AwbcPattern::Whole { target, inner } => {
            validate_unique_pattern_binding_targets(
                program,
                root,
                *inner,
                targets,
                depth + 1,
                limit,
            )?;
            insert_pattern_binding(root, *target, targets)
        }
        AwbcPattern::Discard
        | AwbcPattern::Literal(_)
        | AwbcPattern::Entity(_)
        | AwbcPattern::Variant { payload: None, .. } => Ok(()),
    }
}

fn insert_pattern_binding(
    root: AwbcPatternId,
    target: AwbcRegisterId,
    targets: &mut BTreeSet<AwbcRegisterId>,
) -> Result<(), AwbcVerifyError> {
    if targets.insert(target) {
        Ok(())
    } else {
        Err(AwbcVerifyError::DuplicatePatternBindingTarget {
            pattern: root.index(),
            register: target.0,
        })
    }
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
    let projected = match runtime_shape(program, target_ty) {
        Some(AwbcRuntimeTypeShape::Tuple(items)) if tuple => items.get(ordinal as usize).copied(),
        Some(AwbcRuntimeTypeShape::Record { fields, .. }) if !tuple => {
            fields.get(ordinal as usize).map(|field| field.ty)
        }
        Some(AwbcRuntimeTypeShape::Dynamic) => Some(target_ty),
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
    predicate: fn(Option<&AwbcRuntimeTypeShape>) -> bool,
    label: &str,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    let ty = register_type(verifier, function, block, register)?;
    if predicate(runtime_shape(verifier.program, ty)) {
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
    let Some(ty_layout) = runtime_shape(program, ty) else {
        return false;
    };
    if matches!(ty_layout, AwbcRuntimeTypeShape::Dynamic) {
        return true;
    }
    match (constant, ty_layout) {
        (AwbcConstant::Unit, AwbcRuntimeTypeShape::Unit)
        | (AwbcConstant::Bool(_), AwbcRuntimeTypeShape::Bool)
        | (AwbcConstant::F32Bits(_), AwbcRuntimeTypeShape::F32)
        | (AwbcConstant::F64Bits(_), AwbcRuntimeTypeShape::F64)
        | (AwbcConstant::String(_), AwbcRuntimeTypeShape::String)
        | (AwbcConstant::Char(_), AwbcRuntimeTypeShape::Char)
        | (AwbcConstant::DurationNanos(_), AwbcRuntimeTypeShape::Duration)
        | (AwbcConstant::EntityRef(_), AwbcRuntimeTypeShape::EntityRef)
        | (AwbcConstant::Bytes(_), AwbcRuntimeTypeShape::Bytes)
        | (AwbcConstant::TensorF32 { .. }, AwbcRuntimeTypeShape::MatrixF32)
        | (AwbcConstant::TensorF32 { .. }, AwbcRuntimeTypeShape::TensorF32)
        | (AwbcConstant::TensorF64 { .. }, AwbcRuntimeTypeShape::MatrixF64)
        | (AwbcConstant::TensorF64 { .. }, AwbcRuntimeTypeShape::TensorF64) => true,
        (AwbcConstant::Int { kind, .. }, AwbcRuntimeTypeShape::Int(expected)) => *kind == *expected,
        (AwbcConstant::UInt { kind, .. }, AwbcRuntimeTypeShape::UInt(expected)) => {
            *kind == *expected
        }
        (AwbcConstant::Tuple(values), AwbcRuntimeTypeShape::Tuple(types)) => {
            values.len() == types.len()
                && values.iter().zip(types).all(|(value, ty)| {
                    program
                        .constants
                        .get(value.index())
                        .is_some_and(|value| constant_matches_type(program, value, *ty, depth + 1))
                })
        }
        (AwbcConstant::Sequence(values), AwbcRuntimeTypeShape::Sequence(item_ty)) => {
            values.iter().all(|value| {
                program
                    .constants
                    .get(value.index())
                    .is_some_and(|value| constant_matches_type(program, value, *item_ty, depth + 1))
            })
        }
        (AwbcConstant::Sequence(values), AwbcRuntimeTypeShape::Array { item, length }) => {
            usize::try_from(*length).ok() == Some(values.len())
                && values.iter().all(|value| {
                    program.constants.get(value.index()).is_some_and(|value| {
                        constant_matches_type(program, value, *item, depth + 1)
                    })
                })
        }
        (
            AwbcConstant::Record { ty: actual, .. },
            AwbcRuntimeTypeShape::Record { .. } | AwbcRuntimeTypeShape::NominalRecord { .. },
        ) => actual == &ty,
        (AwbcConstant::Variant { ty: actual, .. }, AwbcRuntimeTypeShape::Variant { .. }) => {
            actual == &ty
        }
        (AwbcConstant::Opaque { ty: actual, .. }, AwbcRuntimeTypeShape::Opaque { .. }) => {
            types_compatible(program, ty, *actual)
        }
        _ => false,
    }
}

fn dynamic_type(program: &AwbcProgram) -> Option<AwbcTypeId> {
    program
        .runtime_types
        .iter()
        .position(|ty| matches!(ty.shape(), AwbcRuntimeTypeShape::Dynamic))
        .and_then(|index| u32::try_from(index).ok())
        .map(AwbcTypeId)
}

fn runtime_shape(program: &AwbcProgram, ty: AwbcTypeId) -> Option<&AwbcRuntimeTypeShape> {
    program
        .runtime_types
        .get(ty.index())
        .map(AwbcRuntimeType::shape)
}

fn line_group_for_function<'a>(
    program: &'a AwbcProgram,
    function: usize,
    at: &str,
) -> Result<&'a crate::awbc::schema::AwbcLineTaskGroup, AwbcVerifyError> {
    let function = u32::try_from(function).map_err(|_| AwbcVerifyError::InvalidInvariant {
        at: at.to_owned(),
        message: "line function index exceeds the AWBC identity domain".to_owned(),
    })?;
    let function = crate::awbc::schema::AwbcFunctionId(function);
    let mut matches = program.line_task_groups.iter().filter(|group| {
        group.activation == function
            || group.cleanup_completed == Some(function)
            || group.cleanup_cancelled == Some(function)
            || group.cleanup_failed == Some(function)
            || group
                .cancel_handlers
                .iter()
                .any(|handler| handler.function == function)
            || group.nodes.checked_end().is_some_and(|end| {
                (group.nodes.start..end).any(|node| {
                    matches!(
                        program.line_task_nodes.get(node as usize),
                        Some(crate::awbc::schema::AwbcLineTaskNode::Action(owner))
                            if *owner == function
                    )
                })
            })
    });
    let group = matches
        .next()
        .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "line instruction function has no owning line-task group".to_owned(),
        })?;
    if matches.next().is_some() {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "line instruction function belongs to multiple line-task groups".to_owned(),
        });
    }
    Ok(group)
}

pub(super) fn runtime_type_permits_copy(
    program: &AwbcProgram,
    ty: AwbcTypeId,
    depth: usize,
) -> bool {
    fn visit(
        program: &AwbcProgram,
        ty: AwbcTypeId,
        depth: usize,
        active: &mut BTreeSet<AwbcTypeId>,
    ) -> bool {
        if depth > 64 || !active.insert(ty) {
            return false;
        }
        let permits = match runtime_shape(program, ty) {
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
                | AwbcRuntimeTypeShape::Progress
                | AwbcRuntimeTypeShape::EntityRef
                | AwbcRuntimeTypeShape::Bytes
                | AwbcRuntimeTypeShape::Never
                | AwbcRuntimeTypeShape::MatrixF32
                | AwbcRuntimeTypeShape::MatrixF64
                | AwbcRuntimeTypeShape::TensorF32
                | AwbcRuntimeTypeShape::TensorF64,
            ) => true,
            Some(AwbcRuntimeTypeShape::Tuple(items) | AwbcRuntimeTypeShape::Choice(items)) => items
                .iter()
                .all(|item| visit(program, *item, depth + 1, active)),
            Some(
                AwbcRuntimeTypeShape::Sequence(item)
                | AwbcRuntimeTypeShape::Range(item)
                | AwbcRuntimeTypeShape::Iterator(item)
                | AwbcRuntimeTypeShape::Array { item, .. },
            ) => visit(program, *item, depth + 1, active),
            Some(
                AwbcRuntimeTypeShape::Record { fields, .. }
                | AwbcRuntimeTypeShape::NominalRecord { fields, .. },
            ) => fields
                .iter()
                .all(|field| visit(program, field.ty, depth + 1, active)),
            Some(AwbcRuntimeTypeShape::Variant { cases, .. }) => cases.iter().all(|case| {
                case.payload
                    .is_none_or(|payload| visit(program, payload, depth + 1, active))
            }),
            Some(AwbcRuntimeTypeShape::Opaque {
                value_class: crate::value::RuntimeOpaqueValueClass::Plain,
                arguments,
                ..
            }) => arguments
                .iter()
                .all(|argument| visit(program, *argument, depth + 1, active)),
            Some(AwbcRuntimeTypeShape::Map { key, value }) => {
                visit(program, *key, depth + 1, active) && visit(program, *value, depth + 1, active)
            }
            Some(
                AwbcRuntimeTypeShape::Nominal { .. }
                | AwbcRuntimeTypeShape::Opaque {
                    value_class: crate::value::RuntimeOpaqueValueClass::AffineHandle(_),
                    ..
                }
                | AwbcRuntimeTypeShape::AgentValue
                | AwbcRuntimeTypeShape::Agent(_)
                | AwbcRuntimeTypeShape::Need(_)
                | AwbcRuntimeTypeShape::Task(_)
                | AwbcRuntimeTypeShape::Stream { .. }
                | AwbcRuntimeTypeShape::Shared(_)
                | AwbcRuntimeTypeShape::Reference(_)
                | AwbcRuntimeTypeShape::Function { .. }
                | AwbcRuntimeTypeShape::Dynamic,
            )
            | None => false,
        };
        active.remove(&ty);
        permits
    }

    visit(program, ty, depth, &mut BTreeSet::new())
}

fn agent_field_value_destination_matches(
    program: &AwbcProgram,
    destination: Option<&AwbcRuntimeTypeShape>,
    expected: RuntimeAgentFieldValue,
) -> bool {
    match expected {
        RuntimeAgentFieldValue::String => matches!(
            destination,
            Some(AwbcRuntimeTypeShape::String | AwbcRuntimeTypeShape::Dynamic)
        ),
        RuntimeAgentFieldValue::Bool => is_bool(destination),
        RuntimeAgentFieldValue::U32 => matches!(
            destination,
            Some(
                AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::U32)
                    | AwbcRuntimeTypeShape::Dynamic
            )
        ),
        RuntimeAgentFieldValue::U64 => matches!(
            destination,
            Some(
                AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::U64)
                    | AwbcRuntimeTypeShape::Dynamic
            )
        ),
        RuntimeAgentFieldValue::Agent(expected) => {
            matches!(
                destination,
                Some(AwbcRuntimeTypeShape::Agent(actual))
                    if actual.operational_type() == expected
            ) || is_dynamic(destination)
        }
        RuntimeAgentFieldValue::BuiltinVariant(expected) => {
            matches!(
                destination,
                Some(AwbcRuntimeTypeShape::Variant {
                    owner: AwbcVariantIdentity::Builtin(actual),
                    ..
                }) if *actual == expected
            ) || is_dynamic(destination)
        }
        RuntimeAgentFieldValue::VecAgent(expected) => match destination {
            Some(AwbcRuntimeTypeShape::Sequence(item)) => matches!(
                runtime_shape(program, *item),
                Some(AwbcRuntimeTypeShape::Agent(actual))
                    if actual.operational_type() == expected
            ),
            Some(AwbcRuntimeTypeShape::Dynamic) => true,
            _ => false,
        },
        RuntimeAgentFieldValue::AgentValueMap => match destination {
            Some(AwbcRuntimeTypeShape::Map { key, value }) => {
                matches!(
                    runtime_shape(program, *key),
                    Some(AwbcRuntimeTypeShape::AgentValue)
                ) && matches!(
                    runtime_shape(program, *value),
                    Some(AwbcRuntimeTypeShape::AgentValue)
                )
            }
            Some(AwbcRuntimeTypeShape::Dynamic) => true,
            _ => false,
        },
    }
}

fn is_bool(ty: Option<&AwbcRuntimeTypeShape>) -> bool {
    matches!(
        ty,
        Some(AwbcRuntimeTypeShape::Bool | AwbcRuntimeTypeShape::Dynamic)
    )
}

fn is_integer(ty: Option<&AwbcRuntimeTypeShape>) -> bool {
    matches!(
        ty,
        Some(
            AwbcRuntimeTypeShape::Int(_)
                | AwbcRuntimeTypeShape::UInt(_)
                | AwbcRuntimeTypeShape::Dynamic
        )
    )
}

fn is_numeric(ty: Option<&AwbcRuntimeTypeShape>) -> bool {
    matches!(
        ty,
        Some(
            AwbcRuntimeTypeShape::Int(_)
                | AwbcRuntimeTypeShape::UInt(_)
                | AwbcRuntimeTypeShape::F32
                | AwbcRuntimeTypeShape::F64
                | AwbcRuntimeTypeShape::Dynamic
        )
    )
}

fn is_sequence_or_dynamic(ty: Option<&AwbcRuntimeTypeShape>) -> bool {
    matches!(
        ty,
        Some(
            AwbcRuntimeTypeShape::Sequence(_)
                | AwbcRuntimeTypeShape::Array { .. }
                | AwbcRuntimeTypeShape::Dynamic
        )
    )
}

fn is_dynamic(ty: Option<&AwbcRuntimeTypeShape>) -> bool {
    matches!(ty, Some(AwbcRuntimeTypeShape::Dynamic))
}

fn is_await_handle(ty: Option<&AwbcRuntimeTypeShape>) -> bool {
    matches!(
        ty,
        Some(
            AwbcRuntimeTypeShape::Task(_)
                | AwbcRuntimeTypeShape::Need(_)
                | AwbcRuntimeTypeShape::Dynamic
        )
    )
}

fn is_progress(ty: Option<&AwbcRuntimeTypeShape>) -> bool {
    matches!(ty, Some(AwbcRuntimeTypeShape::Progress))
}

fn is_task_handle(ty: Option<&AwbcRuntimeTypeShape>) -> bool {
    matches!(
        ty,
        Some(AwbcRuntimeTypeShape::Task(_) | AwbcRuntimeTypeShape::Dynamic)
    )
}

fn is_dynamic_target(ty: Option<&AwbcRuntimeTypeShape>) -> bool {
    matches!(
        ty,
        Some(
            AwbcRuntimeTypeShape::String
                | AwbcRuntimeTypeShape::EntityRef
                | AwbcRuntimeTypeShape::Dynamic
        )
    )
}

fn is_choice_value(ty: Option<&AwbcRuntimeTypeShape>) -> bool {
    matches!(
        ty,
        Some(
            AwbcRuntimeTypeShape::String
                | AwbcRuntimeTypeShape::UInt(_)
                | AwbcRuntimeTypeShape::Dynamic
        )
    )
}
