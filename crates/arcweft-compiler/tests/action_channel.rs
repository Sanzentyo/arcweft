//! Source Action effects retain typed message custody in their real sessions.

use arcweft_compiler::source::compile_source;
use arcweft_core::{
    awbc::{
        product_step::{AwbcProductExecutorSaveSnapshot, AwbcProductStepExecutor},
        schema::{AwbcEntryId, AwbcProgram},
    },
    engine::{Engine, FlowExit, FlowFiberStatus},
    step::{RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

fn options() -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        ..Default::default()
    }
}

fn compile(body: &str) -> (arcweft_compiler::types::CompiledSource, AwbcProgram) {
    let source = format!(
        "pub action @action:.feedback.submit submit(value: i64)\n{body}\nentry cli @entry.main {{ goto @flow.main }}\n"
    );
    let compiled = compile_source(&source).unwrap_or_else(|error| panic!("{error:?}"));
    let program = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "action_channel.arcw",
    )
    .lower()
    .unwrap()
    .program;
    (compiled, program)
}

fn assert_source_emission(body: &str) {
    let (compiled, program) = compile(body);
    let encoded = program.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&encoded, Default::default()).unwrap();
    let [flow] = compiled.plan.flows() else {
        panic!("one source Flow");
    };
    let flow = flow.id.clone();
    let mut native = Engine::for_flow(compiled.plan, &flow).unwrap();
    for _ in 0..64 {
        let result = native.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        assert!(result.output.effects.line.is_empty());
        assert!(result.output.requests.tasks.is_empty());
        if matches!(native.fiber().frame.status, FlowFiberStatus::Done(_)) {
            break;
        }
    }
    assert_eq!(
        native.fiber().frame.status,
        FlowFiberStatus::Done(FlowExit::Return("42".into()))
    );

    let mut product = AwbcProductStepExecutor::for_entry(decoded, AwbcEntryId(0), 64).unwrap();
    let mut finished = false;
    let mut messages = 0;
    for _ in 0..64 {
        let result = product.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        assert!(result.output.effects.line.is_empty());
        assert!(result.output.requests.tasks.is_empty());
        let image = product.snapshot().unwrap();
        let saved =
            AwbcProductExecutorSaveSnapshot::from_live(&image, product.admitted_program()).unwrap();
        assert!(saved.actions.len() >= messages);
        messages = saved.actions.len();
        assert!(messages <= 1, "one authored emission must not replay");
        for (key, declaration, payload) in &saved.actions {
            let arcweft_core::value::AwbcRuntimeValueSnapshot::NominalRecord(record) = payload
            else {
                panic!("original nominal payload");
            };
            assert_eq!(*key, record.semantic_identity);
            let program = product.program();
            let row = &program.runtime_types[program.semantic_type_id(*key).unwrap().index()];
            assert_eq!(Some(declaration), row.nominal_declaration());
            assert_eq!(
                row.nominal_declaration()
                    .unwrap()
                    .action()
                    .unwrap()
                    .as_str(),
                "action.feedback.submit"
            );
            assert_eq!(record.fields.len(), 1);
            assert_eq!(
                record.fields[0].value,
                arcweft_core::value::AwbcRuntimeValueSnapshot::Int(
                    arcweft_core::value::RuntimeInt::I64(42)
                )
            );
        }
        let saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
        finished = matches!(result.fiber_status, FlowFiberStatus::Done(_));
        product = product
            .restore_inert_snapshot_owned(saved)
            .unwrap_or_else(|(_, error)| panic!("Action save restore: {error}"));
        if finished {
            assert_eq!(
                result.fiber_status,
                FlowFiberStatus::Done(FlowExit::Return("42".into()))
            );
            break;
        }
    }
    assert!(finished, "bounded Product execution finishes");
    assert_eq!(
        messages, 1,
        "the emitted payload remains in the actual session save"
    );
}

#[test]
fn source_action_emission_reaches_native_and_decoded_product_without_host_effects() {
    assert_source_emission("flow main() -> i64 { emit(submit(value = 42i64)); return 42i64 }");
}

#[test]
fn source_action_emission_in_an_effectful_function_keeps_its_original_payload() {
    assert_source_emission(
        "fn send(value: submit) -> Unit effects { action.emit } { emit(value) }\nflow main() -> i64 { send(submit(value = 42i64)); return 42i64 }",
    );
}

#[test]
fn action_emission_without_session_authority_preserves_the_original_operand_register() {
    use arcweft_core::{
        awbc::{
            fiber::{AwbcFiberStateSnapshot, FiberState},
            product_step::AwbcProductProgram,
            vm::{self, VmExecutionContext, VmStepOptions},
        },
        effect::RuntimeArtifactFingerprint,
        entry::RuntimeSchemaLimits,
        task::RuntimeProgramOwner,
        value::RuntimeRetainedValueAdmission,
    };
    use std::sync::Arc;
    let (_, program) = compile("flow main() -> i64 { emit(submit(value = 42i64)); return 42i64 }");
    let artifact = RuntimeArtifactFingerprint::try_from_bytes(
        *blake3::hash(&program.encode_canonical().unwrap()).as_bytes(),
    )
    .unwrap();
    let program = Arc::new(program);
    let admitted = AwbcProductProgram::admit(
        Arc::clone(&program),
        Default::default(),
        Default::default(),
        Default::default(),
        None,
    )
    .unwrap();
    let context = VmExecutionContext::for_program(artifact, admitted);
    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 65536).unwrap();
    let mut refused = false;
    for _ in 0..64 {
        let mut admission = RuntimeRetainedValueAdmission::for_program(
            RuntimeProgramOwner::Awbc(Arc::clone(&program)),
            RuntimeSchemaLimits::engine_default(),
        );
        let before = AwbcFiberStateSnapshot::from_live(&fiber, &mut admission).unwrap();
        match vm::step_with_host_context(
            &program,
            &mut fiber,
            VmStepOptions {
                max_instructions: 1,
            },
            &context,
            &mut vm::RejectingVmHost,
        ) {
            Err(vm::VmError::ActionContextRequired) => {
                let mut admission = RuntimeRetainedValueAdmission::for_program(
                    RuntimeProgramOwner::Awbc(Arc::clone(&program)),
                    RuntimeSchemaLimits::engine_default(),
                );
                let after = AwbcFiberStateSnapshot::from_live(&fiber, &mut admission).unwrap();
                assert_eq!(before.cursor, after.cursor);
                assert_eq!(before.frames, after.frames);
                refused = true;
                break;
            }
            Err(error) => panic!("unexpected refusal: {error}"),
            Ok(_) => {}
        }
    }
    assert!(
        refused,
        "a program context alone cannot lend the session's mailbox"
    );
}

#[test]
fn changed_action_declaration_cannot_replace_a_session_holding_its_original_message() {
    let (_, program) = compile("flow main() -> i64 { emit(submit(value = 42i64)); return 42i64 }");
    let mut product = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap();
    let mut queued = false;
    for _ in 0..64 {
        let result = product.step(RuntimeStepInput::default(), options());
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        let image = product.snapshot().unwrap();
        let saved =
            AwbcProductExecutorSaveSnapshot::from_live(&image, product.admitted_program()).unwrap();
        if saved.actions.len() == 1 {
            queued = true;
            break;
        }
    }
    assert!(queued);
    let before = product.snapshot().unwrap();
    let mut changed = product.program().clone();
    let index = changed
        .runtime_types
        .iter()
        .position(|row| {
            row.nominal_declaration()
                .is_some_and(|declaration| declaration.action().is_some())
        })
        .unwrap();
    let declaration = changed.runtime_types[index].nominal_declaration().unwrap();
    let changed_declaration = arcweft_core::entry::RuntimeNominalDeclarationId::try_action(
        *declaration.as_bytes(),
        arcweft_id::PublicId::try_new_engine_owned("action.feedback.other").unwrap(),
    )
    .unwrap();
    // Keep the same semantic type, producer digest, field shape and layout;
    // the original program proof must still reject the substituted Action.
    changed.runtime_types[index] = changed.runtime_types[index]
        .clone()
        .with_nominal_declaration(changed_declaration);
    assert!(product.replace_program_preserving_state(changed).is_err());
    assert_eq!(product.snapshot().unwrap(), before);
}
