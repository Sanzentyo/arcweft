//! Canonical Product AWBC projection for the standard dialogue View handler.

use super::dialogue_primary_action_program_id;
use arcweft_core::{
    awbc::{
        schema::{
            AwbcBlock, AwbcBlockId, AwbcEffectSet, AwbcEffectSetId, AwbcFieldProjection,
            AwbcFrameLayout, AwbcFrameLayoutId, AwbcFrameSlot, AwbcFrameSlotRole, AwbcFunction,
            AwbcFunctionFlag, AwbcFunctionFlags, AwbcFunctionId, AwbcFunctionKind, AwbcInstruction,
            AwbcProgram, AwbcPureHelper, AwbcPureHelperId, AwbcPureHelperOrigin,
            AwbcPureProgramBinding, AwbcRegisterId, AwbcRuntimeType, AwbcRuntimeTypeShape,
            AwbcSafePointKind, AwbcSignature, AwbcSignatureId, AwbcStringId, AwbcTableRange,
            AwbcTerminator, AwbcTypeId,
        },
        verify::{AwbcVerifyBudget, AwbcVerifyContext, AwbcVerifyError},
    },
    pattern::RuntimeOpaqueTypeAdmission,
    value::{RuntimeDialogueOpaqueRole, RuntimeDialogueViewField},
};
use thiserror::Error;

const STANDARD_HELPER_NAME: &str = "std.view.dialogue.primary_action";

/// Atomic standard-handler installation failure.
#[derive(Debug, Error)]
pub enum StandardViewAwbcError {
    #[error("product AWBC already contains the standard dialogue handler program")]
    ProgramCollision,
    #[error("product AWBC repeats semantic runtime type {role:?}")]
    DuplicateRuntimeType { role: RuntimeDialogueOpaqueRole },
    #[error("product AWBC runtime type for {role:?} conflicts with its exact owner")]
    RuntimeTypeConflict { role: RuntimeDialogueOpaqueRole },
    #[error("product AWBC table `{table}` exceeds the v1 index domain")]
    Capacity { table: &'static str },
    #[error("standard dialogue handler produced invalid Product AWBC: {0}")]
    Verify(#[from] AwbcVerifyError),
}

/// Installs the standard dialogue primary-action projection into one candidate
/// Product AWBC program and publishes the merged tables atomically.
///
/// The installer verifies the engine-owned fragment in isolation. Complete
/// Product AWBC policy, including the required public entrypoint, remains the
/// bundle verifier's authority; this extension seam does not reinterpret an
/// otherwise incomplete caller-owned program.
pub fn install_dialogue_handler_awbc(
    program: AwbcProgram,
) -> Result<AwbcProgram, StandardViewAwbcError> {
    verify_owned_dialogue_handler_fragment()?;

    let mut candidate = program;
    install_dialogue_handler_rows(&mut candidate)?;
    candidate.canonicalize_string_table();
    Ok(candidate)
}

fn verify_owned_dialogue_handler_fragment() -> Result<(), StandardViewAwbcError> {
    let mut fragment = AwbcProgram::default();
    install_dialogue_handler_rows(&mut fragment)?;
    fragment.canonicalize_string_table();
    fragment.verify(
        AwbcVerifyBudget::default(),
        AwbcVerifyContext {
            require_entrypoint: false,
            ..AwbcVerifyContext::default()
        },
    )?;
    Ok(())
}

fn install_dialogue_handler_rows(candidate: &mut AwbcProgram) -> Result<(), StandardViewAwbcError> {
    let handler = dialogue_primary_action_program_id();
    if candidate
        .pure_programs
        .iter()
        .any(|binding| binding.program == handler)
    {
        return Err(StandardViewAwbcError::ProgramCollision);
    }

    let view_type = exact_dialogue_type(candidate, RuntimeDialogueOpaqueRole::View)?;
    let action_type = exact_dialogue_type(candidate, RuntimeDialogueOpaqueRole::Action)?;
    let effect_set = empty_effect_set(candidate)?;
    let helper_name = intern_string(candidate, STANDARD_HELPER_NAME)?;

    let signature = AwbcSignatureId(table_index(candidate.signatures.len(), "signatures")?);
    candidate.signatures.push(AwbcSignature {
        params: vec![view_type],
        result: Some(action_type),
        effects: effect_set,
    });

    let frame_layout =
        AwbcFrameLayoutId(table_index(candidate.frame_layouts.len(), "frame_layouts")?);
    candidate.frame_layouts.push(AwbcFrameLayout {
        slots: vec![
            AwbcFrameSlot {
                name: None,
                ty: view_type,
                role: AwbcFrameSlotRole::Parameter,
                scope_depth: 0,
            },
            AwbcFrameSlot {
                name: None,
                ty: action_type,
                role: AwbcFrameSlotRole::Temporary,
                scope_depth: 0,
            },
        ],
        max_scope_depth: 0,
    });

    let function = AwbcFunctionId(table_index(candidate.functions.len(), "functions")?);
    let block = AwbcBlockId(table_index(candidate.blocks.len(), "blocks")?);
    let instruction_start = table_index(candidate.instructions.len(), "instructions")?;
    candidate.instructions.push(AwbcInstruction::ProjectField {
        dst: AwbcRegisterId(1),
        target: AwbcRegisterId(0),
        field: AwbcFieldProjection::OpaqueRecord {
            owner: view_type,
            field: RuntimeDialogueViewField::PrimaryAction.ordinal(),
            field_type: action_type,
        },
    });
    candidate.blocks.push(AwbcBlock {
        owner: function,
        instructions: AwbcTableRange::new(instruction_start, 1),
        terminator: AwbcTerminator::Return {
            value: Some(AwbcRegisterId(1)),
        },
        safe_point: AwbcSafePointKind::CallableBoundary,
        source_map: None,
    });
    candidate.functions.push(AwbcFunction {
        public_id: Some(helper_name),
        kind: AwbcFunctionKind::PureHelper,
        signature,
        frame_layout,
        blocks: AwbcTableRange::new(block.0, 1),
        entry_block: block,
        flags: AwbcFunctionFlags::empty().with(AwbcFunctionFlag::Deterministic),
    });

    let helper = AwbcPureHelperId(table_index(candidate.pure_helpers.len(), "pure_helpers")?);
    candidate.pure_helpers.push(AwbcPureHelper {
        public_id: helper_name,
        signature,
        function,
        scalar_eval_supported: false,
        origin: AwbcPureHelperOrigin::EngineOwned,
    });
    candidate.pure_programs.push(AwbcPureProgramBinding {
        program: handler,
        helper,
        input_types: vec![RuntimeDialogueOpaqueRole::View.semantic_identity()],
        result_type: RuntimeDialogueOpaqueRole::Action.semantic_identity(),
    });
    candidate
        .pure_programs
        .sort_by_key(|binding| binding.program);
    Ok(())
}

fn exact_dialogue_type(
    program: &mut AwbcProgram,
    role: RuntimeDialogueOpaqueRole,
) -> Result<AwbcTypeId, StandardViewAwbcError> {
    let semantic_identity = role.semantic_identity();
    let mut matches = program
        .runtime_types
        .iter()
        .enumerate()
        .filter(|(_, row)| row.semantic_identity() == semantic_identity);
    if let Some((index, row)) = matches.next() {
        if matches.next().is_some() {
            return Err(StandardViewAwbcError::DuplicateRuntimeType { role });
        }
        let exact = row
            .try_opaque_owner(&program.strings)
            .ok()
            .flatten()
            .is_some_and(|owner| role.accepts_exact_owner(&owner));
        let no_arguments = matches!(
            row.shape(),
            AwbcRuntimeTypeShape::Opaque { arguments, .. } if arguments.is_empty()
        );
        if !exact || !no_arguments {
            return Err(StandardViewAwbcError::RuntimeTypeConflict { role });
        }
        return table_index(index, "runtime_types").map(AwbcTypeId);
    }

    let producer = intern_string(program, role.producer().as_str())?;
    let id = AwbcTypeId(table_index(program.runtime_types.len(), "runtime_types")?);
    program.runtime_types.push(AwbcRuntimeType::new(
        semantic_identity,
        AwbcRuntimeTypeShape::Opaque {
            producer,
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            value_class: role.value_class(),
            persistence: role.persistence(),
            arguments: Vec::new(),
        },
    ));
    Ok(id)
}

fn empty_effect_set(program: &mut AwbcProgram) -> Result<AwbcEffectSetId, StandardViewAwbcError> {
    if let Some(index) = program
        .effect_sets
        .iter()
        .position(|effects| effects.effects.is_empty())
    {
        return table_index(index, "effect_sets").map(AwbcEffectSetId);
    }
    let id = AwbcEffectSetId(table_index(program.effect_sets.len(), "effect_sets")?);
    program.effect_sets.push(AwbcEffectSet::default());
    Ok(id)
}

fn intern_string(
    program: &mut AwbcProgram,
    value: &str,
) -> Result<AwbcStringId, StandardViewAwbcError> {
    if let Some(index) = program
        .strings
        .iter()
        .position(|candidate| candidate == value)
    {
        return table_index(index, "strings").map(AwbcStringId);
    }
    let id = AwbcStringId(table_index(program.strings.len(), "strings")?);
    program.strings.push(value.to_owned());
    Ok(id)
}

fn table_index(index: usize, table: &'static str) -> Result<u32, StandardViewAwbcError> {
    u32::try_from(index).map_err(|_| StandardViewAwbcError::Capacity { table })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::{
        awbc::product_step::evaluate_pure_program_with_backend,
        pure::VmRuntimePureCallBackend,
        value::{RuntimeDialogueActionValue, RuntimeDialogueViewValue, RuntimeValue},
    };

    fn dialogue_view_value() -> RuntimeValue {
        let wrap = |role: RuntimeDialogueOpaqueRole| {
            role.exact_owner()
                .try_wrap(RuntimeValue::Unit)
                .expect("standard dialogue role is exact")
        };
        RuntimeDialogueViewValue::try_new(
            wrap(RuntimeDialogueOpaqueRole::Character),
            wrap(RuntimeDialogueOpaqueRole::Content),
            wrap(RuntimeDialogueOpaqueRole::Occurrence),
            wrap(RuntimeDialogueOpaqueRole::Stage),
            wrap(RuntimeDialogueOpaqueRole::Reveal),
            RuntimeDialogueActionValue::None.into_runtime_value(),
        )
        .expect("canonical DialogueView payload")
        .into_runtime_value()
    }

    #[test]
    fn installer_publishes_one_verified_typed_projection() {
        let program = install_dialogue_handler_awbc(AwbcProgram::default())
            .expect("standard handler installs");
        let binding = program
            .pure_program_binding(dialogue_primary_action_program_id())
            .expect("standard program binding");
        assert_eq!(
            binding.input_types,
            vec![RuntimeDialogueOpaqueRole::View.semantic_identity()]
        );
        assert_eq!(
            binding.result_type,
            RuntimeDialogueOpaqueRole::Action.semantic_identity()
        );

        let result = evaluate_pure_program_with_backend(
            &program,
            dialogue_primary_action_program_id(),
            &[dialogue_view_value()],
            &mut VmRuntimePureCallBackend::default(),
        )
        .expect("standard pure projection executes");
        assert_eq!(
            RuntimeDialogueActionValue::try_from_runtime_value(&result),
            Ok(RuntimeDialogueActionValue::None)
        );
    }

    #[test]
    fn installer_rejects_program_collision_and_conflicting_type_before_publication() {
        let installed = install_dialogue_handler_awbc(AwbcProgram::default())
            .expect("first installation succeeds");
        assert!(matches!(
            install_dialogue_handler_awbc(installed.clone()),
            Err(StandardViewAwbcError::ProgramCollision)
        ));

        let mut conflicting = AwbcProgram::default();
        conflicting.runtime_types.push(AwbcRuntimeType::new(
            RuntimeDialogueOpaqueRole::View.semantic_identity(),
            AwbcRuntimeTypeShape::Bool,
        ));
        let snapshot = conflicting.clone();
        assert!(matches!(
            install_dialogue_handler_awbc(conflicting),
            Err(StandardViewAwbcError::RuntimeTypeConflict {
                role: RuntimeDialogueOpaqueRole::View,
            })
        ));
        assert_eq!(snapshot.runtime_types.len(), 3);
        assert!(snapshot.pure_programs.is_empty());
    }
}
