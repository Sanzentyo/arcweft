use super::{AwbcProductStepExecutor, AwbcProductStepParityBlocker};
use crate::awbc::schema::{
    AwbcChoiceId, AwbcEffectPlanId, AwbcInstruction, AwbcProgram, AwbcRegisterId,
    AwbcResumePointId, AwbcTerminator,
};
use crate::engine::FlowFiberStatus;
use crate::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepStopReason};

#[test]
fn empty_product_program_finishes_without_diagnostics() {
    let mut executor = AwbcProductStepExecutor::for_entry(
        AwbcProgram::default(),
        crate::awbc::schema::AwbcEntryId(0),
        64,
    )
    .expect("empty product AWBC executor starts");

    let result = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert_eq!(result.stop_reason, RuntimeStepStopReason::Done);
    assert!(result.output.diagnostics.is_empty());
    assert!(matches!(result.fiber_status, FlowFiberStatus::Done(_)));
}

#[test]
fn implemented_instruction_and_terminator_families_have_no_static_blocker() {
    let instruction = AwbcInstruction::EmitEffect {
        effect: AwbcEffectPlanId(0),
        args: Vec::new(),
    };
    let terminator = AwbcTerminator::Choice {
        choice: AwbcChoiceId(0),
        dst: AwbcRegisterId(0),
        resume: AwbcResumePointId(0),
    };

    assert_eq!(instruction.product_step_parity_blocker(), None);
    assert_eq!(terminator.product_step_parity_blocker(), None);
}

#[test]
fn product_program_inventory_is_empty_after_adapter_coverage() {
    let program = AwbcProgram {
        instructions: vec![AwbcInstruction::EmitEffect {
            effect: AwbcEffectPlanId(0),
            args: Vec::new(),
        }],
        ..AwbcProgram::default()
    };

    assert_eq!(
        program.product_step_parity_blockers(),
        Vec::<AwbcProductStepParityBlocker>::new()
    );
}
