use std::sync::Arc;

use arcweft_compiler::source::compile_source;
use arcweft_core::awbc::fiber::{AwbcFiberStateSnapshot, FiberState};
use arcweft_core::awbc::schema::{AwbcEntryId, AwbcProgram};
use arcweft_core::awbc::vm::{self, VmExit, VmStepOptions};
use arcweft_core::task::RuntimeProgramOwner;
use arcweft_core::value::RuntimeValue;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[path = "support/execution.rs"]
mod execution;

#[test]
fn named_scope_result_is_bound_in_the_parent_before_its_continuation() {
    let source = r#"
entry cli @entry.main { goto @flow.main }
flow main() -> String {
    let result = scope rain {
        let inner = scope window { "done" }
        inner
    }
    return result
}
"#;
    execution::assert_native_return(source, "done");
    execution::assert_awbc_return(source, RuntimeValue::String("done".to_owned()));
}

#[test]
fn pure_function_scope_keeps_its_lexical_identity_and_value_in_both_backends() {
    let source = r#"
entry cli @entry.main { goto @flow.main }
fn value() -> String { scope answer { let result = "done"; result } }
flow main() -> String { return value() }
"#;
    execution::assert_native_return(source, "done");
    execution::assert_awbc_return(source, RuntimeValue::String("done".to_owned()));
    let compiled = compile_source(source).unwrap();
    let report = AwbcLowerer::new(&compiled.plan, &compiled.dialogue_content, "scope.arcw")
        .lower()
        .unwrap();
    assert!(
        report
            .program
            .frame_layouts
            .iter()
            .flat_map(|layout| &layout.scopes)
            .any(|scope| scope
                .identity
                .name()
                .is_some_and(|name| name.as_str() == "answer"))
    );
}

#[test]
fn mixed_scope_choice_ids_and_active_frames_survive_awbc_save_restore() {
    let compiled = compile_source(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> Never {
    scope rain {
        return scope window {
            choice @.first { @.next "Next" -> @flow.done }
        }
    }
}
flow done() -> String { return "done" }
"#,
    )
    .expect("mixed lexical scopes compile");
    let report = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "scope-choice.arcw",
    )
    .lower()
    .expect("mixed lexical scopes lower to verified AWBC");
    let bytes = report.program.encode_canonical().unwrap();
    let program = AwbcProgram::decode_canonical(&bytes, Default::default()).unwrap();
    let choice = &program.choices[0];
    assert_eq!(
        &program.strings[choice.public_id.unwrap().index()],
        "choice.main.rain.window.first"
    );
    let option = &program.choice_options[choice.options.start as usize];
    assert_eq!(
        &program.strings[option.public_id.unwrap().index()],
        "choice.main.rain.window.first.next"
    );

    let mut fiber = FiberState::for_entry(&program, AwbcEntryId(0), 1, 65_536).unwrap();
    let output = vm::step(&program, &mut fiber, VmStepOptions::default()).unwrap();
    assert!(
        matches!(output.exit, VmExit::Suspended(_)),
        "{:?}",
        output.exit
    );
    fiber.validate_for_program(&program).unwrap();
    let frame = fiber.active_frame().unwrap();
    let function = &program.functions[frame.function.index()];
    let layout = &program.frame_layouts[function.frame_layout.index()];
    let names = frame
        .scopes
        .iter()
        .filter_map(|scope| layout.scopes[scope.id.index()].identity.name())
        .map(arcweft_id::DeclarationName::as_str)
        .collect::<Vec<_>>();
    assert_eq!(names, ["rain", "window"]);
    let snapshot = AwbcFiberStateSnapshot::from_live(&fiber).unwrap();
    let restored = snapshot
        .into_live_for_program(&RuntimeProgramOwner::Awbc(Arc::new(program)))
        .unwrap();
    assert_eq!(restored, fiber);
}
