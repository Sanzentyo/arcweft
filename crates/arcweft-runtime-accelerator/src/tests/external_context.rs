use super::*;

struct ProgramExternal {
    owner: RuntimeProgramOwner,
}

impl RuntimeExternalCallBackend for ProgramExternal {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        _callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        let owner = context.program_owner()?;
        if !self.owner.same_program(&owner) {
            return None;
        }
        match args {
            [value] => Some(Ok(RuntimeValue::i64(value.try_i64()?.checked_add(1)?))),
            _ => None,
        }
    }
}

#[test]
fn installed_external_context_keeps_the_original_program_and_borrowed_arguments() {
    let scalar = RuntimeSemanticTypeId::from_bytes([0x3f; 32]);
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_type_batch(
            [RuntimePlanTypeSeed::new(
                scalar,
                RuntimePlanTypeProjection::Signed(RuntimeSignedIntWidth::I64),
            )],
            std::iter::empty(),
        )
        .unwrap();
    let plan = Arc::new(builder.finish().unwrap());
    let mut accelerator =
        RuntimePureAccelerator::with_config(RuntimePureAcceleratorConfig::default(), &plan);
    let owner = RuntimeProgramOwner::Plan(Arc::clone(&plan));
    let context = RuntimeExternalCallContext::for_program(
        arcweft_core::pure::RuntimeProgramExecutionContext::native(
            Arc::clone(&plan),
            Default::default(),
        ),
        [scalar],
        scalar,
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    let target = callable_target("fixture.external");
    let args = [RuntimeValue::i64(41)];
    assert!(
        accelerator
            .call_external(&context, &target, &args)
            .is_none()
    );
    accelerator.set_external_calls(ProgramExternal { owner });
    assert_eq!(
        accelerator
            .call_external(&context, &target, &args)
            .unwrap()
            .unwrap(),
        RuntimeValue::i64(42)
    );
    assert_eq!(args, [RuntimeValue::i64(41)]);
    let foreign = RuntimeExternalCallContext::for_program(
        arcweft_core::pure::RuntimeProgramExecutionContext::native(
            Arc::new((*plan).clone()),
            Default::default(),
        ),
        [scalar],
        scalar,
        RuntimeSchemaLimits::engine_default(),
    )
    .unwrap();
    assert!(
        accelerator
            .call_external(&foreign, &target, &args)
            .is_none()
    );
    assert!(
        accelerator
            .program_owner()
            .same_program(&RuntimeProgramOwner::Plan(plan))
    );
}
