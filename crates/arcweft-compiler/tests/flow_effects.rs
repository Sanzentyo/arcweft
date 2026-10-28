//! Closed Flow effects survive semantic admission, runtime lowering and execution.

use std::sync::Arc;

use arcweft_compiler::{source::compile_source, types::CompiledSource};
use arcweft_core::{
    awbc::{
        fiber::FiberState,
        schema::{AwbcEntryId, AwbcProgram},
        vm::{self, VmExit, VmStepOptions},
    },
    engine::{Engine, FlowExit, FlowFiberStatus},
    step::{RuntimeStepInput, RuntimeStepOptions},
    value::RuntimeValue,
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

fn verify_flow_effects(compiled: &CompiledSource, expected: &[(&str, &[&str])]) -> AwbcProgram {
    let program = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "flow_effects.arcw",
    )
    .lower()
    .expect("closed Flow contracts lower to verified AWBC")
    .program;
    assert_eq!(compiled.plan.flows().len(), expected.len());
    for (name, effects) in expected {
        let flow = compiled
            .plan
            .flows()
            .iter()
            .find(|flow| flow.id.public_label().as_str() == format!("flow.{name}"))
            .expect("expected runtime Flow");
        assert_eq!(
            flow.function()
                .invocation_effects()
                .iter()
                .map(arcweft_id::EffectId::as_str)
                .collect::<Vec<_>>(),
            *effects,
            "runtime effects for {name}"
        );
        let function = program
            .flow_function(&flow.id)
            .expect("exact AWBC Flow binding");
        let signature = &program.signatures[program.functions[function.index()].signature.index()];
        assert_eq!(
            program.effect_sets[signature.effects.index()]
                .effects
                .iter()
                .map(|effect| program.strings[effect.index()].as_str())
                .collect::<Vec<_>>(),
            *effects,
            "AWBC effects for {name}"
        );
    }
    program
}

fn assert_returns_42(compiled: CompiledSource, program: AwbcProgram) {
    let main = compiled
        .plan
        .flows()
        .iter()
        .find(|flow| flow.id.public_label().as_str() == "flow.main")
        .expect("accepted main Flow identity")
        .id
        .clone();
    let mut engine = Engine::for_flow(compiled.plan, &main).expect("native main starts");
    let mut finished = false;
    for _ in 0..256 {
        let output = engine
            .step(RuntimeStepInput::default(), RuntimeStepOptions::default())
            .output;
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        match &engine.fiber().frame.status {
            FlowFiberStatus::Running => {}
            FlowFiberStatus::Done(exit) => {
                assert_eq!(exit, &FlowExit::Return("42".to_owned()));
                finished = true;
                break;
            }
            status => panic!("unexpected native state: {status:?}"),
        }
    }
    assert!(finished, "native execution exceeded its step limit");
    let encoded = program.encode_canonical().expect("canonical Flow AWBC");
    let artifact = arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
        *blake3::hash(&encoded).as_bytes(),
    )
    .expect("Flow program fingerprint");
    let program = Arc::new(program);
    let context = vm::VmExecutionContext::for_program(
        artifact,
        arcweft_core::awbc::product_step::AwbcProductProgram::admit(
            Arc::clone(&program),
            arcweft_core::awbc::verify::AwbcVerifyBudget::default(),
            arcweft_core::awbc::verify::AwbcVerifyContext::default(),
            arcweft_core::plan::RuntimeTaskSourceArtifactLimits::default(),
            None,
        )
        .expect("the standalone Flow program is admitted"),
    );
    let mut host = vm::RejectingVmHost;
    let mut fiber =
        FiberState::for_entry(&program, AwbcEntryId(0), 1, 65_536).expect("AWBC main starts");
    for _ in 0..256 {
        match vm::step_with_host_context(
            &program,
            &mut fiber,
            VmStepOptions::default(),
            &context,
            &mut host,
        )
        .expect("AWBC step")
        .exit
        {
            VmExit::Running => {}
            VmExit::Returned(value) => {
                assert_eq!(value, Some(RuntimeValue::i64(42)));
                return;
            }
            VmExit::DialogueResultSelected(value) => {
                panic!("ordinary Flow selected a DialogueLine result: {value:?}")
            }
            exit => panic!("unexpected AWBC exit: {exit:?}"),
        }
    }
    panic!("AWBC execution exceeded its step limit");
}

#[test]
fn authored_and_inferred_flow_effects_authorize_ordinary_calls() {
    for clause in ["", "effects { fs.read }"] {
        let compiled = compile_source(&format!(
            "fn read() -> i64 effects {{ fs.read }} {{ 42i64 }}\n\
             flow main() -> i64 {clause} {{ return read() }}\n\
             entry cli @entry.main {{ goto @flow.main }}\n"
        ))
        .expect("Flow effect bound admits the ordinary call");
        let program = verify_flow_effects(&compiled, &[("main", &["fs.read"])]);
        assert_returns_42(compiled, program);
    }
}

#[test]
fn wait_operands_reach_one_typed_awbc_input_through_global_and_specialized_bodies() {
    use arcweft_core::awbc::schema::{AwbcEffectPayload, AwbcRuntimeTypeShape, AwbcWaitDuration};
    for source in [
        "flow main { wait(0.1s) }",
        "fn delay() -> Duration { 0.35s }\nflow main { wait(delay()) }",
        "fn delay(value: Duration) -> Duration { value }\nflow main { wait(delay(0.35s)) }",
        "fn delay<T>(value: T) -> Duration { 0.35s }\nflow main { wait(delay(42i64)) }",
        "fn launch<T>(value: T, delay: Duration) -> ThreadHandle<T> effects { control.spawn } {\n thread {\n wait(delay)\n return value\n }\n}\nflow main { let child = launch(42i64, 0.35s) }",
        "fn launch<T>(value: T, delay: Duration) -> ThreadHandle<T> effects { control.spawn } {\n thread {\n if true { wait(delay) }\n return value\n }\n}\nflow main { let child = launch(42i64, 0.35s) }",
        "flow main { wait(if true { 0.1s } else { 0.35s }) }",
    ] {
        let source = format!("{source}\nentry cli @entry.main {{ goto @flow.main }}\n");
        let compiled =
            compile_source(&source).unwrap_or_else(|error| panic!("{source}: {error:?}"));
        let program = AwbcLowerer::new(&compiled.plan, &compiled.dialogue_content, "wait.arcw")
            .lower()
            .expect("checked Wait lowers into a verified Duration input")
            .program;
        let waits = program
            .effect_plans
            .iter()
            .filter(|effect| matches!(effect.payload, AwbcEffectPayload::Wait(_)))
            .collect::<Vec<_>>();
        assert!(!waits.is_empty(), "{source}");
        for effect in waits {
            assert_eq!(
                effect.payload,
                AwbcEffectPayload::Wait(AwbcWaitDuration::Input)
            );
            let signature = &program.signatures[effect.signature.index()];
            let [duration] = signature.params.as_slice() else {
                panic!("one Wait operand")
            };
            assert!(matches!(
                program.runtime_types[duration.index()].shape(),
                AwbcRuntimeTypeShape::Duration
            ));
            assert_eq!(signature.result, None);
            assert!(effect.static_args().is_empty());
        }
        let bytes = program.encode_canonical().unwrap();
        let decoded = AwbcProgram::decode_canonical(&bytes, Default::default()).unwrap();
        decoded
            .verify(Default::default(), Default::default())
            .unwrap();
        assert_eq!(decoded.effect_plans, program.effect_plans);
    }
    // An ordinary function resolves wait as the Agent Predicate API; it does
    // not acquire the dedicated Flow/Thread/line-plan statement grammar.
    assert!(compile_source("fn pause(delay: Duration) { wait(delay) }\nflow main { pause(0.35s) }\nentry cli @entry.main { goto @flow.main }\n").is_err());
}

#[test]
fn authored_flow_permissions_keep_unused_members_and_scopes() {
    let compiled = compile_source(
        "fn read() -> i64 effects { fs.read } { 42i64 }\n\
         flow main() -> i64 effects { fs.read(save), fs.write } { return read() }\n\
         entry cli @entry.main { goto @flow.main }\n",
    )
    .expect("scoped Flow effect bound covers the unscoped operation");
    assert_eq!(
        compiled.plan.flows()[0]
            .body()
            .effects()
            .iter()
            .map(arcweft_id::EffectId::as_str)
            .collect::<Vec<_>>(),
        vec!["fs.read"]
    );
    let program = verify_flow_effects(&compiled, &[("main", &["fs.read(save)", "fs.write"])]);
    assert_returns_42(compiled, program);
}

#[test]
fn curried_prefix_has_argument_effects_but_keeps_body_effects_latent() {
    for (argument, effects) in [
        ("20i64", Vec::<&str>::new()),
        ("write(20i64)", vec!["fs.write"]),
    ] {
        let compiled = compile_source(&format!(
            "fn read(first: i64)(second: i64) -> i64 effects {{ fs.read }} {{ first + second }}\n\
             fn write(value: i64) -> i64 effects {{ fs.write }} {{ value }}\n\
             flow main() -> i64 {{ let prefix = read({argument}); return 42i64 }}\n\
             entry cli @entry.main {{ goto @flow.main }}\n"
        ))
        .expect("constructing a prefix does not invoke its body");
        let program = verify_flow_effects(&compiled, &[("main", &effects)]);
        assert_returns_42(compiled, program);
    }
}

#[test]
fn curried_terminal_application_executes_the_body_effects() {
    let compiled = compile_source(
        "fn read(first: i64)(second: i64) -> i64 effects { fs.read } { first + second }\n\
         flow main() -> i64 { let prefix = read(20i64); return prefix(22i64) }\n\
         entry cli @entry.main { goto @flow.main }\n",
    )
    .expect("terminal application infers the body effect");
    let program = verify_flow_effects(&compiled, &[("main", &["fs.read"])]);
    assert_returns_42(compiled, program);
}

#[test]
fn static_and_dynamic_flow_transfers_change_the_active_effect_scope() {
    for transfer in [
        "goto @flow.destination",
        "let target = @flow.destination; goto target",
    ] {
        let compiled = compile_source(&format!(
            "fn read() -> i64 effects {{ fs.read }} {{ 42i64 }}\n\
             flow main() -> i64 effects {{}} {{ {transfer} }}\n\
             flow destination() -> i64 {{ return read() }}\n\
             entry cli @entry.main {{ goto @flow.main }}\n"
        ))
        .expect("a terminal Flow transfer does not invoke the target in the old scope");
        let program =
            verify_flow_effects(&compiled, &[("main", &[]), ("destination", &["fs.read"])]);
        assert_returns_42(compiled, program);
    }
}
