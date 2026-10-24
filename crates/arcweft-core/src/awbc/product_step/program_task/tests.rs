use super::*;
use crate::awbc::schema::*;
use crate::plan::FlowOp;
use std::sync::Arc;

#[test]
fn program_handle_private_fiber_checkpoint_restores_its_original_source() {
    use crate::awbc::fiber::{
        AwbcFiberRoot, AwbcFiberStateSnapshot, FiberState, validate_program_task_argument_refs,
    };
    use crate::task::{
        GenerationId, NeedProducerInstance, NeedProducerRegistry, RuntimeNeedHandle,
        RuntimeTaskTemplateArguments, TaskRequest, TaskSpec,
    };
    use crate::value::{RuntimeRetainedValueAdmission, RuntimeValue};

    let mut code = crate::plan::task_source::program_task_executable_fixture();
    code.runtime_types.push(AwbcRuntimeType::new(
        crate::pattern::RuntimeSemanticTypeId::from_bytes([0x47; 32]),
        AwbcRuntimeTypeShape::Task(AwbcTypeId(0)),
    ));
    code.frame_layouts[0].slots.push(AwbcFrameSlot {
        name: None,
        ty: AwbcTypeId(2),
        role: AwbcFrameSlotRole::Temporary,
        scope_depth: 0,
    });
    let admitted = super::super::AwbcProductProgram::admit(
        Arc::new(code),
        crate::awbc::verify::AwbcVerifyBudget::default(),
        crate::awbc::verify::AwbcVerifyContext {
            require_entrypoint: false,
            ..Default::default()
        },
        crate::plan::RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap();
    let program = admitted.task_source.as_ref().unwrap();
    let owner = crate::task::RuntimeProgramOwner::Awbc(admitted.program_arc());
    let crate::task::RuntimeProgramOwner::Awbc(code) = &owner else {
        unreachable!()
    };
    let source = program
        .awbc_program_source(
            AwbcTaskPlanId(0),
            RuntimeTaskPlanSealLimits::default(),
            &mut crate::task::semantic::TaskSemanticMeter::new(1_000_000, 1_000_000),
        )
        .unwrap();
    let capture = RuntimeValue::Bool(true);
    let mut bank = RuntimeRetainedValueAdmission::for_program(
        owner.clone(),
        crate::entry::RuntimeSchemaLimits::engine_default(),
    );
    let checked = source
        .template_producer_spec(
            &program,
            RuntimeTaskTemplateArguments::Thread {
                captures: &[&capture],
            },
            None,
            &mut bank,
        )
        .unwrap();
    let Some(FlowOp::Thread { producer, .. }) = source.request_endpoint().operation() else {
        unreachable!()
    };
    let spec = TaskSpec {
        generation: GenerationId::new(0),
        producer: NeedProducerInstance::try_from(&checked).unwrap(),
        class: producer.class.clone(),
        priority: producer.priority,
        cancel_scope: producer.cancel_scope.clone(),
        policy: producer.policy,
        outcome: producer.outcome(),
        request: TaskRequest::Program(checked.source().clone()),
        debug_label: producer.debug_label.clone(),
    };
    let mut registry = NeedProducerRegistry::default();
    let (_, _, handle) = registry
        .ensure_program_task_with_binding(spec, |receipt| {
            Ok::<_, Box<dyn std::error::Error>>(RuntimeValue::NeedHandle(
                RuntimeNeedHandle::try_from(receipt.clone())?,
            ))
        })
        .unwrap();
    let inputs = validate_program_task_argument_refs(
        code,
        AwbcTaskPlanId(0),
        &[&capture],
        None,
        &bank.work(),
    )
    .unwrap();
    let mut fiber = FiberState::for_function_with_arguments_prepared(
        code,
        AwbcFiberRoot::Task(AwbcTaskPlanId(0)),
        vec![capture],
        inputs,
        crate::runtime_id::RuntimeFiberInstanceId::from_allocated(std::num::NonZeroU64::MIN),
        0,
        64,
    );
    fiber
        .active_frame_mut()
        .unwrap()
        .set_register(AwbcRegisterId(2), handle)
        .unwrap();
    let before = AwbcFiberStateSnapshot::from_live(&fiber, &owner).unwrap();
    let checkpoint = fiber.checkpoint(&owner).unwrap();
    let foreign = super::super::AwbcProductProgram::admit(
        Arc::new((**code).clone()),
        crate::awbc::verify::AwbcVerifyBudget::default(),
        crate::awbc::verify::AwbcVerifyContext {
            require_entrypoint: false,
            ..Default::default()
        },
        crate::plan::RuntimeTaskSourceArtifactLimits::default(),
        None,
    )
    .unwrap();
    assert!(matches!(
        fiber.restore(checkpoint.clone(), &foreign),
        Err(crate::awbc::fiber::FiberStateError::CheckpointProgramOwner)
    ));
    assert_eq!(
        AwbcFiberStateSnapshot::from_live(&fiber, &owner).unwrap(),
        before
    );
    assert!(matches!(
        fiber
            .active_frame_mut()
            .unwrap()
            .take_register(AwbcRegisterId(2))
            .unwrap(),
        RuntimeValue::NeedHandle(_)
    ));
    fiber
        .restore(checkpoint, &admitted)
        .expect("private rollback retains the actual original Program Source");
    assert_eq!(
        AwbcFiberStateSnapshot::from_live(&fiber, &owner).unwrap(),
        before
    );
}
