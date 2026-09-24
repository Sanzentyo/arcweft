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
    AwbcAgentTypeShape, AwbcBinaryOp, AwbcBindMode, AwbcBlockId, AwbcConstant,
    AwbcDialogueValueRole, AwbcDropPolicy, AwbcEffectSetId, AwbcFrameLayout, AwbcFrameSlotRole,
    AwbcFunctionFlag, AwbcFunctionKind, AwbcInstruction, AwbcPattern, AwbcPatternId,
    AwbcPatternRest, AwbcProgram, AwbcProjectCallAttachedPresence, AwbcProjectCallOperandMode,
    AwbcProjectCallOrdinaryMaterialization, AwbcRegisterId, AwbcResumePointId, AwbcRuntimeType,
    AwbcRuntimeTypeShape, AwbcSafePointKind, AwbcScopeId, AwbcSignatureId, AwbcTerminator,
    AwbcTraitReceiverMode, AwbcTypeId, AwbcUnaryOp, AwbcUnsignedIntKind, AwbcVariantIdentity,
};
use crate::pattern::RuntimeBuiltinVariantCaseIdentity;
use crate::plan::{
    RuntimeAgentTypeProjection, RuntimeCallableAttachedContract, RuntimeCallableParameterKind,
    RuntimeCallablePosition, RuntimeCallableRetainedRole, RuntimeCallableTransition,
    RuntimePlanSequenceKind,
};
use crate::value::{
    RuntimeAgentField, RuntimeAgentFieldResult, RuntimeAgentFieldValue, RuntimeAgentSignatureError,
    RuntimeAgentTypeContext, RuntimeAgentTypeOperand, RuntimeCharacterDialogueProducerId,
    RuntimeDialogueOpaqueRole, RuntimeReductionProducer,
};
use arcweft_interaction_model::dialogue::{
    CharacterDialogueOperation, CharacterDialoguePatchOperation,
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
        verify_function(verifier, function, None)?;
    }
    Ok(())
}

fn verify_dialogue_effect_callable_state<'a>(
    program: &'a AwbcProgram,
    state_id: crate::runtime_id::RuntimeCallableStateId,
    capture_types: &[AwbcTypeId],
    at: &str,
) -> Result<
    &'a crate::plan::RuntimeCallableStateDefinition<
        AwbcTypeId,
        crate::awbc::schema::AwbcFunctionId,
    >,
    AwbcVerifyError,
> {
    let state = program
        .callable_states
        .get(state_id.index())
        .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "dialogue effect callback state is absent".to_owned(),
        })?;
    let function_type = program
        .runtime_types
        .get(state.function_type.index())
        .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "dialogue effect callback function type is absent".to_owned(),
        })?;
    let AwbcRuntimeTypeShape::Function {
        parameters, result, ..
    } = function_type.shape()
    else {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "dialogue effect callback state does not have a function type".to_owned(),
        });
    };
    if !parameters.is_empty()
        || !matches!(
            runtime_shape(program, *result),
            Some(AwbcRuntimeTypeShape::Unit)
        )
        || state.result != *result
        || !state.parameters.is_empty()
        || !matches!(state.attached, RuntimeCallableAttachedContract::None)
        || state.position != RuntimeCallablePosition::Unapplied
        || state.retained.len() != capture_types.len()
        || state.retained.iter().zip(capture_types).enumerate().any(
            |(position, (retained, expected))| {
                retained.ty != *expected
                    || retained.role
                        != (RuntimeCallableRetainedRole::Capture {
                            position: u32::try_from(position).unwrap_or(u32::MAX),
                        })
            },
        )
    {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "dialogue effect callback state does not match its closed capture ABI"
                .to_owned(),
        });
    }
    let RuntimeCallableTransition::Invoke {
        function,
        captures,
        arguments,
    } = &state.transition
    else {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "dialogue effect callback state does not invoke a body".to_owned(),
        });
    };
    if !arguments.is_empty() || captures.len() != capture_types.len() {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "dialogue effect callback invocation is not a zero-argument closure"
                .to_owned(),
        });
    }
    let target = program.functions.get(function.index()).ok_or_else(|| {
        AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "dialogue effect callback body is absent".to_owned(),
        }
    })?;
    if target.kind != AwbcFunctionKind::Ordinary {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "dialogue effect callback body is not ordinary".to_owned(),
        });
    }
    let signature = program
        .signatures
        .get(target.signature.index())
        .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "dialogue effect callback body signature is absent".to_owned(),
        })?;
    if signature.result != Some(state.result) || signature.params.as_slice() != capture_types {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "dialogue effect callback body signature disagrees with its state".to_owned(),
        });
    }
    Ok(state)
}

fn verify_function(
    verifier: &Verifier<'_, '_>,
    function_index: usize,
    scope_query: Option<(usize, u32)>,
) -> Result<Option<Vec<AwbcScopeId>>, AwbcVerifyError> {
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
    let mut queried_scopes = None;

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
        for (offset, instruction_index) in instruction_range.enumerate() {
            if scope_query == Some((block_index, block_index_to_u32(offset))) {
                queried_scopes = Some(state.scopes.clone());
            }
            apply_instruction(
                verifier,
                function_index,
                block_index,
                instruction_index,
                &mut state,
            )?;
        }
        if scope_query == Some((block_index, block.instructions.len)) {
            queried_scopes = Some(state.scopes.clone());
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
    Ok(queried_scopes)
}

pub(super) fn scope_stack_at(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    offset: u32,
) -> Result<Vec<AwbcScopeId>, AwbcVerifyError> {
    if verifier.program.functions.get(function).is_none()
        || verifier.block_owner.get(block) != Some(&function)
    {
        return Err(AwbcVerifyError::InvalidInvariant {
            at: "scope resume coordinate".to_owned(),
            message: "scope resume coordinate has a foreign function or block".to_owned(),
        });
    }
    verify_function(verifier, function, Some((block, offset)))?.ok_or_else(|| {
        AwbcVerifyError::InvalidInvariant {
            at: "scope resume coordinate".to_owned(),
            message: "scope resume offset is outside the selected block".to_owned(),
        }
    })
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
            let layout = function_layout(verifier, function);
            let definition = layout.scopes.get(scope.index()).ok_or_else(|| {
                AwbcVerifyError::ScopeDiscipline {
                    function,
                    block,
                    message: format!("scope {} has no frame definition", scope.0),
                }
            })?;
            if definition.parent != state.scopes.last().copied() {
                return Err(AwbcVerifyError::ScopeDiscipline {
                    function,
                    block,
                    message: format!("scope {} has a different lexical parent", scope.0),
                });
            }
            if state.scopes.contains(scope) {
                return Err(AwbcVerifyError::ScopeDiscipline {
                    function,
                    block,
                    message: format!("scope {} is entered twice", scope.0),
                });
            }
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
                Some(AwbcRuntimeTypeShape::Sequence { item: item_ty, .. }) => (*item_ty, None),
                Some(AwbcRuntimeTypeShape::Array { item, length }) => {
                    let expected = length
                        .constant()
                        .and_then(|length| usize::try_from(length).ok())
                        .ok_or_else(|| AwbcVerifyError::ResultShapeMismatch { at: at.clone() })?;
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
            let Some(AwbcRuntimeTypeShape::Sequence { item: item_ty, .. }) =
                runtime_shape(program, dst_ty)
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
                Some(AwbcRuntimeTypeShape::Sequence { item: item_ty, .. }) => {
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
            if let Some(AwbcRuntimeTypeShape::Sequence { item: item_ty, .. }) =
                runtime_shape(program, sequence_ty)
            {
                require_compatible(program, *item_ty, value_ty, &at)?;
            } else if !is_dynamic(runtime_shape(program, sequence_ty)) {
                return invalid_type(&at, "sequence input");
            }
        }
        AwbcInstruction::MakeRecord { dst, ty, fields } => {
            check_index(program.runtime_types.len(), ty.0, "runtime_types", &at)?;
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
                    for (field, expected) in fields.iter().zip(type_fields) {
                        let actual = read_register(verifier, function, block, *field, state)?;
                        require_compatible(program, expected.ty, actual, &at)?;
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
            let operand_types = operands
                .iter()
                .map(|operand| {
                    read_register(verifier, function, block, *operand, state)
                        .map(RuntimeAgentTypeOperand::Typed)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let dst_ty = register_type(verifier, function, block, *dst)?;
            constructor
                .validate_types(program, dst_ty, &operand_types)
                .map_err(|error| AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: match error {
                        RuntimeAgentSignatureError::OperandCount { actual } => format!(
                            "Agent constructor {constructor:?} rejects {actual} operand(s)"
                        ),
                        RuntimeAgentSignatureError::OperandType { operand } => format!(
                            "Agent constructor {constructor:?} rejects operand {operand} runtime type"
                        ),
                        RuntimeAgentSignatureError::ResultType => {
                            "Agent constructor destination".to_owned()
                        }
                    },
                })?;
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
                        let Some(field_layout) = fields
                            .iter()
                            .find(|candidate| candidate.name == Some(*field))
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
                            "label" => program
                                .builtin_variant_payload_item(
                                    dst_ty,
                                    RuntimeBuiltinVariantCaseIdentity::OptionSome,
                                )
                                .is_some_and(|item| {
                                    matches!(
                                        runtime_shape(program, item),
                                        Some(AwbcRuntimeTypeShape::String)
                                    )
                                }),
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
                            RuntimeAgentFieldResult::Optional(value) => {
                                is_dynamic(destination)
                                    || program
                                        .builtin_variant_payload_item(
                                            dst_ty,
                                            RuntimeBuiltinVariantCaseIdentity::OptionSome,
                                        )
                                        .is_some_and(|item| {
                                            agent_field_value_destination_matches(
                                                program,
                                                runtime_shape(program, item),
                                                value,
                                            )
                                        })
                            }
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
                Some(
                    AwbcRuntimeTypeShape::Record { fields, .. }
                    | AwbcRuntimeTypeShape::NominalRecord { fields, .. },
                ) => {
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
        AwbcInstruction::MakeDialogueContent {
            destination,
            template,
            values,
            effects,
        } => {
            let capture_registers = effects.iter().try_fold(0_usize, |total, effect| {
                total
                    .checked_add(effect.captures.len())
                    .ok_or(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "MakeDialogueContent capture register count overflows usize"
                            .to_owned(),
                    })
            })?;
            check_args_budget(verifier, values.len().saturating_add(capture_registers))?;
            let Some(template) = program
                .content_templates
                .iter()
                .find(|candidate| candidate.id == *template)
            else {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at,
                    message: "MakeDialogueContent references a missing template manifest"
                        .to_owned(),
                });
            };
            if template.slots.len() != values.len() {
                return argument_count(&at, template.slots.len(), values.len());
            }
            for (index, (binding, slot)) in values.iter().zip(&template.slots).enumerate() {
                let Some(expected_slot) =
                    crate::runtime_id::RuntimeDialogueValueSlotId::from_zero_based(index)
                else {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message:
                            "MakeDialogueContent value slot count exceeds the slot identity domain"
                                .to_owned(),
                    });
                };
                if binding.slot != expected_slot || binding.role != slot.role {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "MakeDialogueContent value binding does not match its canonical template slot"
                            .to_owned(),
                    });
                }
                let actual = read_register(verifier, function, block, binding.value, state)?;
                require_compatible(
                    program,
                    slot.semantic_type,
                    actual,
                    "dialogue content binding",
                )?;
            }
            if template.effects.len() != effects.len() {
                return argument_count(&at, template.effects.len(), effects.len());
            }
            for (index, (binding, slot)) in effects.iter().zip(&template.effects).enumerate() {
                let expected_site =
                    crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                        .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message:
                                "MakeDialogueContent effect count exceeds the effect-site identity domain"
                                    .to_owned(),
                        })?;
                if binding.site != expected_site || binding.site != slot.site {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "MakeDialogueContent effect binding does not match its canonical template effect slot"
                            .to_owned(),
                    });
                }
                verify_dialogue_effect_callable_state(
                    program,
                    binding.state,
                    &slot.capture_types,
                    &at,
                )?;
                if binding.captures.len() != slot.capture_types.len() {
                    return argument_count(&at, slot.capture_types.len(), binding.captures.len());
                }
                for (capture, expected) in binding.captures.iter().zip(&slot.capture_types) {
                    let actual = read_register(verifier, function, block, *capture, state)?;
                    require_compatible(
                        program,
                        *expected,
                        actual,
                        "dialogue content effect capture",
                    )?;
                }
            }
            let destination_type = register_type(verifier, function, block, *destination)?;
            if !is_exact_dialogue_content_type(program, destination_type) {
                return invalid_type(&at, "MakeDialogueContent destination");
            }
            write_register(verifier, function, block, *destination, state)?;
        }
        AwbcInstruction::CharacterDialogue {
            destination,
            operation,
            target,
            fields,
        } => {
            check_args_budget(verifier, fields.len().saturating_add(1))?;
            let target_type = read_register(verifier, function, block, *target, state)?;
            match operation {
                CharacterDialogueOperation::Factory => {
                    if !matches!(
                        runtime_shape(program, target_type),
                        Some(AwbcRuntimeTypeShape::EntityRef)
                    ) {
                        return invalid_type(&at, "CharacterDialogue factory target");
                    }
                }
                CharacterDialogueOperation::Reconfigure => {
                    if !is_character_dialogue_type(program, target_type) {
                        return invalid_type(&at, "CharacterDialogue reconfigure target");
                    }
                }
            }
            let destination_type = register_type(verifier, function, block, *destination)?;
            if !is_character_dialogue_type(program, destination_type) {
                return invalid_type(&at, "CharacterDialogue destination");
            }
            for field in fields {
                if let CharacterDialoguePatchOperation::Set(value) = &field.operation {
                    read_register(verifier, function, block, *value, state)?;
                }
            }
            write_register(verifier, function, block, *destination, state)?;
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
        AwbcInstruction::MakeCallable {
            dst,
            state: state_id,
            captures,
        } => {
            check_args_budget(verifier, captures.len())?;
            let definition = program
                .callable_states
                .get(state_id.index())
                .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "callable construction references an absent state".to_owned(),
                })?;
            if definition.origin != *state_id
                || definition.position != RuntimeCallablePosition::Unapplied
                || definition.retained.len() != captures.len()
            {
                return invalid_type(
                    &at,
                    "callable construction requires its unapplied state layout",
                );
            }
            for (position, (capture, retained)) in
                captures.iter().zip(&definition.retained).enumerate()
            {
                let Some(expected_position) = u32::try_from(position).ok() else {
                    return invalid_type(&at, "callable capture count exceeds u32");
                };
                if !matches!(
                    retained.role,
                    RuntimeCallableRetainedRole::Capture { position } if position == expected_position
                ) {
                    return invalid_type(
                        &at,
                        "callable construction retained layout is not captures",
                    );
                }
                let actual = read_register(verifier, function, block, *capture, state)?;
                require_compatible(program, retained.ty, actual, &at).map_err(|_| {
                    AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: format!("callable capture {position} has an incompatible type"),
                    }
                })?;
            }
            let actual = register_type(verifier, function, block, *dst)?;
            require_compatible(program, definition.function_type, actual, &at)?;
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::SpecializeCallable {
            dst,
            src,
            specialization,
        } => {
            let source_type = read_register(verifier, function, block, *src, state)?;
            let definition = program
                .callable_specializations
                .get(specialization.index())
                .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "callable specialization references an absent table row".to_owned(),
                })?;
            let target_type = register_type(verifier, function, block, *dst)?;
            if source_type != definition.source_type || target_type != definition.target_type {
                return invalid_type(
                    &at,
                    "callable specialization instruction types do not match its admitted relation",
                );
            }
            write_register(verifier, function, block, *dst, state)?;
        }
        AwbcInstruction::ApplyGroup { dst, callee, args } => {
            check_args_budget(verifier, args.len())?;
            read_register(verifier, function, block, *callee, state)?;
            for arg in args {
                read_register(verifier, function, block, *arg, state)?;
            }
            verify_apply_group(verifier, function, block, *dst, *callee, args, &at)?;
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

impl RuntimeAgentTypeContext for AwbcProgram {
    type Type = AwbcTypeId;

    fn is_string(&self, ty: Self::Type) -> bool {
        matches!(runtime_shape(self, ty), Some(AwbcRuntimeTypeShape::String))
    }

    fn is_entity_reference(&self, ty: Self::Type) -> bool {
        matches!(
            runtime_shape(self, ty),
            Some(AwbcRuntimeTypeShape::EntityRef)
        )
    }

    fn is_u32(&self, ty: Self::Type) -> bool {
        matches!(
            runtime_shape(self, ty),
            Some(AwbcRuntimeTypeShape::UInt(AwbcUnsignedIntKind::U32))
        )
    }

    fn agent_type(&self, ty: Self::Type) -> Option<RuntimeAgentTypeProjection<Self::Type>> {
        match runtime_shape(self, ty)? {
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Probe(item)) => {
                Some(RuntimeAgentTypeProjection::Probe(*item))
            }
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::DataShape(item)) => {
                Some(RuntimeAgentTypeProjection::DataShape(*item))
            }
            AwbcRuntimeTypeShape::Agent(AwbcAgentTypeShape::Leaf(kind)) => {
                RuntimeAgentTypeProjection::try_leaf(*kind)
            }
            _ => None,
        }
    }

    fn sequence_type(&self, ty: Self::Type) -> Option<(Self::Type, Option<u64>)> {
        match runtime_shape(self, ty)? {
            AwbcRuntimeTypeShape::Sequence { item, .. } => Some((*item, None)),
            AwbcRuntimeTypeShape::Array { item, length } => Some((*item, length.constant())),
            _ => None,
        }
    }

    fn tuple_type(&self, ty: Self::Type) -> Option<&[Self::Type]> {
        match runtime_shape(self, ty)? {
            AwbcRuntimeTypeShape::Tuple(items) => Some(items),
            _ => None,
        }
    }
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
        AwbcTerminator::ProjectCall { call } => {
            let mut next = state.clone();
            verify_project_call(verifier, function, block, call, &mut next, &at)?;
            successors.push((
                verify_resume(
                    verifier,
                    function,
                    call.resume,
                    AwbcSafePointKind::CallableBoundary,
                    &at,
                )?,
                next,
            ));
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
            // A terminal Flow transfer changes the active effect scope. It
            // shares the argument ABI with calls, but has no returning caller.
            verify_args(
                verifier,
                function,
                block,
                program.functions[target.index()].signature,
                args,
                state,
                &at,
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
            target,
            content,
            values,
            effects,
            line_task_captures,
            result,
            resume,
        } => {
            let target_type = read_register(verifier, function, block, *target, state)?;
            if !is_character_dialogue_type(program, target_type) {
                return invalid_type(&at, "CharacterDialogue opaque target");
            }
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
            let content_unit = &program.content_units[content.index()];
            let template = program
                .content_templates
                .iter()
                .find(|template| template.id == content_unit.template)
                .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "dialogue content unit references a missing template manifest"
                        .to_owned(),
                })?;
            if values.len() != template.slots.len() {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message: "dialogue terminator bindings do not match the content slot manifest"
                        .to_owned(),
                });
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
                let manifest = &template.slots[index];
                if binding.role != manifest.role {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message: "dialogue terminator binding role disagrees with the content slot manifest"
                            .to_owned(),
                    });
                }
                let ty = read_register(verifier, function, block, binding.value, state)?;
                if ty != manifest.semantic_type {
                    return type_mismatch(&at, manifest.semantic_type, ty);
                }
                match binding.role {
                    AwbcDialogueValueRole::Interpolation => {}
                    AwbcDialogueValueRole::Content => {
                        if !is_exact_dialogue_content_type(program, ty) {
                            return invalid_type(&at, "exact DialogueContent opaque value");
                        }
                    }
                }
            }
            if effects.len() != template.effects.len() {
                return Err(AwbcVerifyError::InvalidInvariant {
                    at: at.clone(),
                    message:
                        "dialogue terminator effect bindings do not match the content effect manifest"
                            .to_owned(),
                });
            }
            for (index, binding) in effects.iter().enumerate() {
                let expected =
                    crate::runtime_id::RuntimeDialogueEffectSiteId::from_zero_based(index)
                        .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                            at: at.clone(),
                            message: "dialogue effect site count exceeds u32".to_owned(),
                        })?;
                let manifest = &template.effects[index];
                if binding.site != expected || binding.site != manifest.site {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message:
                            "dialogue terminator effect sites are not canonical and contiguous"
                                .to_owned(),
                    });
                }
                verify_dialogue_effect_callable_state(
                    program,
                    binding.state,
                    &manifest.capture_types,
                    &at,
                )?;
                if binding.captures.len() != manifest.capture_types.len() {
                    return Err(AwbcVerifyError::InvalidInvariant {
                        at: at.clone(),
                        message:
                            "dialogue effect callback capture count disagrees with its manifest"
                                .to_owned(),
                    });
                }
                for (capture, expected) in binding.captures.iter().zip(&manifest.capture_types) {
                    let actual = read_register(verifier, function, block, *capture, state)?;
                    require_compatible(program, *expected, actual, &at)?;
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

fn verify_project_call(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    call: &crate::awbc::schema::AwbcProjectCall,
    state: &mut FlowState,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    check_args_budget(verifier, call.operands.len())?;
    check_args_budget(verifier, call.ordinary.len())?;
    let definition = program
        .callable_states
        .get(call.state.index())
        .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "project-call input state is absent".to_owned(),
        })?;
    let expected_group =
        match &definition.position {
            RuntimeCallablePosition::Unapplied => 0,
            RuntimeCallablePosition::WithinGroup { group, .. } => *group,
            RuntimeCallablePosition::AfterGroup { completed } => completed
                .checked_add(1)
                .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                    at: at.to_owned(),
                    message: "project-call state group overflows".to_owned(),
                })?,
        };
    if call.completed_group != expected_group {
        return invalid_type(
            at,
            "project-call group disagrees with its checked state position",
        );
    }
    if call.ordinary.len() != definition.parameters.len() {
        return argument_count(at, definition.parameters.len(), call.ordinary.len());
    }
    let actual_callee = read_register(verifier, function, block, call.callee, state)?;
    require_project_call_compatible(program, definition.function_type, actual_callee, at)?;

    let mut sources = BTreeSet::new();
    for (ordinary_index, row) in call.ordinary.iter().enumerate() {
        let input = &definition.parameters[ordinary_index];
        let parameter = match row {
            AwbcProjectCallOrdinaryMaterialization::Fixed { parameter, .. }
            | AwbcProjectCallOrdinaryMaterialization::Rest { parameter, .. } => *parameter,
        };
        let expected_parameter = u32::try_from(ordinary_index).unwrap_or(u32::MAX);
        if parameter != expected_parameter {
            return invalid_type(at, "project-call logical parameter rows are not dense");
        }
        match row {
            AwbcProjectCallOrdinaryMaterialization::Fixed { source_index, .. } => {
                if input.kind != RuntimeCallableParameterKind::Fixed {
                    return invalid_type(at, "fixed call materialization targets a rest parameter");
                }
                let operand = project_call_operand(call, *source_index, at)?;
                if operand.mode != AwbcProjectCallOperandMode::Value {
                    return invalid_type(at, "fixed project-call source must be a value operand");
                }
                let actual = read_register(verifier, function, block, operand.value, state)?;
                require_project_call_compatible(program, input.abi_ty, actual, at)?;
                insert_project_call_source(&mut sources, *source_index, call.operands.len(), at)?;
            }
            AwbcProjectCallOrdinaryMaterialization::Rest { source_indices, .. } => {
                if input.kind != RuntimeCallableParameterKind::Rest
                    || source_indices.windows(2).any(|pair| pair[0] >= pair[1])
                {
                    return invalid_type(
                        at,
                        "rest call materialization disagrees with its parameter",
                    );
                }
                let Some(AwbcRuntimeTypeShape::Sequence {
                    kind: RuntimePlanSequenceKind::Vec,
                    item,
                }) = runtime_shape(program, input.binding_ty)
                else {
                    return invalid_type(at, "rest binding type must be a sequence");
                };
                require_project_call_compatible(program, input.abi_ty, *item, at)?;
                for source_index in source_indices {
                    let operand = project_call_operand(call, *source_index, at)?;
                    let actual = read_register(verifier, function, block, operand.value, state)?;
                    match operand.mode {
                        AwbcProjectCallOperandMode::Value => {
                            require_project_call_compatible(program, input.abi_ty, actual, at)?;
                        }
                        AwbcProjectCallOperandMode::Spread => {
                            if !is_sequence_or_tuple_of(program, actual, input.abi_ty) {
                                return invalid_type(
                                    at,
                                    "rest spread source has the wrong item type",
                                );
                            }
                        }
                    }
                    insert_project_call_source(
                        &mut sources,
                        *source_index,
                        call.operands.len(),
                        at,
                    )?;
                }
            }
        }
    }
    if let Some(attached) = &call.attached {
        verify_project_call_attached(
            verifier,
            function,
            block,
            call,
            attached,
            &definition.attached,
            &mut sources,
            state,
            at,
        )?;
    } else if !matches!(definition.attached, RuntimeCallableAttachedContract::None) {
        return invalid_type(
            at,
            "project-call omits its attached materialization evidence",
        );
    }
    for index in 0..call.operands.len() {
        let index = u32::try_from(index).map_err(|_| AwbcVerifyError::InvalidInvariant {
            at: at.to_owned(),
            message: "project-call source count exceeds u32".to_owned(),
        })?;
        if !sources.contains(&index) {
            return invalid_type(at, "project-call source operand is not consumed");
        }
    }

    if let RuntimeCallableAttachedContract::Defaulted { default, .. } = &definition.attached {
        let crate::plan::RuntimeCallableDefault::Body {
            function: default_function,
            ..
        } = default
        else {
            return invalid_type(at, "project-call cannot execute an unbound generic default");
        };
        let target = program
            .functions
            .get(default_function.index())
            .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                at: at.to_owned(),
                message: "callable default target is absent".to_owned(),
            })?;
        require_effects(
            verifier,
            function,
            program.signatures[target.signature.index()].effects,
            "callable default",
        )?;
    }
    if let RuntimeCallableTransition::Invoke {
        function: target, ..
    } = &definition.transition
    {
        let function_row = program.functions.get(target.index()).ok_or_else(|| {
            AwbcVerifyError::InvalidInvariant {
                at: at.to_owned(),
                message: "callable invocation target is absent".to_owned(),
            }
        })?;
        require_effects(
            verifier,
            function,
            program.signatures[function_row.signature.index()].effects,
            "callable invocation",
        )?;
    }
    validate_pattern(
        verifier,
        function,
        block,
        call.result_pattern,
        definition.result,
        Some(AwbcBindMode::Declare),
        state,
        0,
    )
}

fn verify_apply_group(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    destination: AwbcRegisterId,
    callee: AwbcRegisterId,
    arguments: &[AwbcRegisterId],
    at: &str,
) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    let callee_type = register_type(verifier, function, block, callee)?;
    let Some(AwbcRuntimeTypeShape::Function { parameters, .. }) =
        runtime_shape(program, callee_type)
    else {
        return invalid_type(at, "group application callee must have a function type");
    };
    if arguments.len() > parameters.len() {
        return invalid_type(
            at,
            "group application supplies more values than its function arrow",
        );
    }
    for (argument, expected) in arguments.iter().zip(parameters) {
        let actual = register_type(verifier, function, block, *argument)?;
        require_project_call_compatible(program, *expected, actual, at)?;
    }
    let destination_type = register_type(verifier, function, block, destination)?;
    let mut accepted = false;
    for definition in program
        .callable_states
        .iter()
        .filter(|definition| definition.function_type == callee_type)
    {
        let arrow_arity = parameters.len();
        let result_type = if arguments.len() < arrow_arity {
            if arguments.is_empty() {
                Some(definition.function_type)
            } else {
                let coordinates = definition
                    .parameters
                    .iter()
                    .take(arguments.len())
                    .map(|input| input.coordinate)
                    .collect::<Vec<_>>();
                definition
                    .partials
                    .iter()
                    .find(|partial| partial.parameters.as_ref() == coordinates.as_slice())
                    .and_then(|partial| {
                        program
                            .callable_states
                            .get(partial.state.index())
                            .map(|target| target.function_type)
                    })
            }
        } else if arguments.len() == arrow_arity {
            Some(definition.result)
        } else {
            None
        };
        if result_type.is_some_and(|expected| {
            project_call_types_compatible(program, expected, destination_type)
        }) {
            accepted = true;
            break;
        }
    }
    if !accepted {
        return invalid_type(
            at,
            "group application has no sealed callable state with this result type",
        );
    }
    Ok(())
}

fn project_call_operand<'a>(
    call: &'a crate::awbc::schema::AwbcProjectCall,
    index: u32,
    at: &str,
) -> Result<&'a crate::awbc::schema::AwbcProjectCallOperand, AwbcVerifyError> {
    let index = usize::try_from(index).map_err(|_| AwbcVerifyError::IndexOutOfBounds {
        table: "project-call operands",
        index,
        at: at.to_owned(),
    })?;
    call.operands
        .get(index)
        .ok_or_else(|| AwbcVerifyError::IndexOutOfBounds {
            table: "project-call operands",
            index: u32::try_from(index).unwrap_or(u32::MAX),
            at: at.to_owned(),
        })
}

fn insert_project_call_source(
    sources: &mut BTreeSet<u32>,
    index: u32,
    operand_count: usize,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    let index_usize = usize::try_from(index).map_err(|_| AwbcVerifyError::IndexOutOfBounds {
        table: "project-call operands",
        index,
        at: at.to_owned(),
    })?;
    if index_usize >= operand_count || !sources.insert(index) {
        return invalid_type(at, "project-call source operand is repeated");
    }
    Ok(())
}

fn is_sequence_or_tuple_of(program: &AwbcProgram, ty: AwbcTypeId, item: AwbcTypeId) -> bool {
    match runtime_shape(program, ty) {
        Some(AwbcRuntimeTypeShape::Sequence { item: actual, .. }) => {
            project_call_types_compatible(program, *actual, item)
        }
        Some(AwbcRuntimeTypeShape::Array { item: actual, .. }) => {
            project_call_types_compatible(program, *actual, item)
        }
        Some(AwbcRuntimeTypeShape::Tuple(items)) => items
            .iter()
            .all(|actual| project_call_types_compatible(program, *actual, item)),
        _ => false,
    }
}

fn verify_project_call_attached(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    call: &crate::awbc::schema::AwbcProjectCall,
    attached: &crate::awbc::schema::AwbcProjectCallAttachedMaterialization,
    contract: &RuntimeCallableAttachedContract<AwbcTypeId, crate::awbc::schema::AwbcFunctionId>,
    sources: &mut BTreeSet<u32>,
    state: &FlowState,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    let program = verifier.program;
    let present = matches!(
        &attached.presence,
        AwbcProjectCallAttachedPresence::RequiredPresent
            | AwbcProjectCallAttachedPresence::OptionalPresent
            | AwbcProjectCallAttachedPresence::DefaultedPresent
    );
    if present != attached.source_index.is_some() {
        return invalid_type(at, "project-call attached source parity is invalid");
    }
    let expected_value = match (contract, &attached.presence) {
        (
            RuntimeCallableAttachedContract::Required { ty },
            AwbcProjectCallAttachedPresence::RequiredPresent,
        )
        | (
            RuntimeCallableAttachedContract::Defaulted { ty, .. },
            AwbcProjectCallAttachedPresence::DefaultedPresent,
        ) => Some(*ty),
        (
            RuntimeCallableAttachedContract::Optional { value, .. },
            AwbcProjectCallAttachedPresence::OptionalPresent,
        ) => Some(*value),
        (
            RuntimeCallableAttachedContract::Optional { .. },
            AwbcProjectCallAttachedPresence::OptionalOmitted,
        )
        | (
            RuntimeCallableAttachedContract::Defaulted { .. },
            AwbcProjectCallAttachedPresence::DefaultedOmitted,
        ) => None,
        _ => {
            return invalid_type(
                at,
                "project-call attached evidence disagrees with its state",
            );
        }
    };
    if let Some(expected) = expected_value {
        let index = attached
            .source_index
            .ok_or_else(|| AwbcVerifyError::InvalidInvariant {
                at: at.to_owned(),
                message: "present attached source is absent".to_owned(),
            })?;
        let operand = project_call_operand(call, index, at)?;
        if operand.mode != AwbcProjectCallOperandMode::Value {
            return invalid_type(at, "attached source must be a value operand");
        }
        let actual = read_register(verifier, function, block, operand.value, state)?;
        require_project_call_compatible(program, expected, actual, at)?;
        insert_project_call_source(sources, index, call.operands.len(), at)?;
    }
    Ok(())
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
    verify_args(verifier, function, block, signature, args, state, at)?;
    let effects = verifier.program.signatures[signature.index()].effects;
    require_effects(verifier, function, effects, callee)
}

fn verify_args(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    signature: AwbcSignatureId,
    args: &[AwbcRegisterId],
    state: &FlowState,
    at: &str,
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
    Ok(())
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
                Some(AwbcRuntimeTypeShape::Sequence { item: item_ty, .. }) => Some(*item_ty),
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
        Some(
            AwbcRuntimeTypeShape::Record { fields, .. }
            | AwbcRuntimeTypeShape::NominalRecord { fields, .. },
        ) if !tuple => fields.get(ordinal as usize).map(|field| field.ty),
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

fn require_project_call_compatible(
    program: &AwbcProgram,
    expected: AwbcTypeId,
    actual: AwbcTypeId,
    at: &str,
) -> Result<(), AwbcVerifyError> {
    if matches!(
        runtime_shape(program, expected),
        Some(AwbcRuntimeTypeShape::Dynamic)
    ) || matches!(
        runtime_shape(program, actual),
        Some(AwbcRuntimeTypeShape::Dynamic)
    ) {
        return invalid_type(at, "project-call ABI cannot use Dynamic types");
    }
    if project_call_types_compatible(program, expected, actual) {
        Ok(())
    } else {
        type_mismatch(at, expected, actual)
    }
}

fn project_call_types_compatible(
    program: &AwbcProgram,
    expected: AwbcTypeId,
    actual: AwbcTypeId,
) -> bool {
    !matches!(
        runtime_shape(program, expected),
        Some(AwbcRuntimeTypeShape::Dynamic)
    ) && !matches!(
        runtime_shape(program, actual),
        Some(AwbcRuntimeTypeShape::Dynamic)
    ) && types_compatible(program, expected, actual)
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
        (AwbcConstant::Sequence(values), AwbcRuntimeTypeShape::Sequence { item: item_ty, .. }) => {
            values.iter().all(|value| {
                program
                    .constants
                    .get(value.index())
                    .is_some_and(|value| constant_matches_type(program, value, *item_ty, depth + 1))
            })
        }
        (AwbcConstant::Sequence(values), AwbcRuntimeTypeShape::Array { item, length }) => {
            length
                .constant()
                .and_then(|length| usize::try_from(length).ok())
                == Some(values.len())
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

fn is_exact_dialogue_content_type(program: &AwbcProgram, ty: AwbcTypeId) -> bool {
    let Some(AwbcRuntimeTypeShape::Opaque { arguments, .. }) = runtime_shape(program, ty) else {
        return false;
    };
    arguments.is_empty()
        && program
            .opaque_owner(ty)
            .ok()
            .flatten()
            .is_some_and(|owner| RuntimeDialogueOpaqueRole::Content.accepts_exact_owner(&owner))
}

fn is_character_dialogue_type(program: &AwbcProgram, ty: AwbcTypeId) -> bool {
    let Some(AwbcRuntimeTypeShape::Opaque {
        arguments,
        value_class: crate::value::RuntimeOpaqueValueClass::Plain,
        persistence: crate::value::RuntimeOpaquePersistence::ConstantAndSnapshot,
        ..
    }) = runtime_shape(program, ty)
    else {
        return false;
    };
    arguments.is_empty()
        && program
            .opaque_owner(ty)
            .ok()
            .flatten()
            .is_some_and(|owner| owner.producer() == &RuntimeCharacterDialogueProducerId::get())
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
                AwbcRuntimeTypeShape::Sequence { item, .. }
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
            Some(AwbcRuntimeTypeShape::Map { key, value, .. }) => {
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
                | AwbcRuntimeTypeShape::BoundType(_)
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
            Some(AwbcRuntimeTypeShape::Sequence { item, .. }) => matches!(
                runtime_shape(program, *item),
                Some(AwbcRuntimeTypeShape::Agent(actual))
                    if actual.operational_type() == expected
            ),
            Some(AwbcRuntimeTypeShape::Dynamic) => true,
            _ => false,
        },
        RuntimeAgentFieldValue::AgentValueMap => match destination {
            Some(AwbcRuntimeTypeShape::Map { key, value, .. }) => {
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
            AwbcRuntimeTypeShape::Sequence { .. }
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
