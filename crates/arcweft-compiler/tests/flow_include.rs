//! Include invokes the original Flow function under both execution backends.

use std::sync::Arc;

use arcweft_compiler::source::compile_source;
use arcweft_core::{
    awbc::{
        codec::AwbcDecodeBudget,
        product_step::AwbcProductProgram,
        schema::AwbcProgram,
        verify::{AwbcVerifyBudget, AwbcVerifyContext},
    },
    engine::{FlowExit, FlowFiberStatus},
    executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor},
    plan::{RuntimeEntryTarget, RuntimeTaskSourceArtifactLimits},
    step::{RuntimeStepInput, RuntimeStepOptions},
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[derive(Clone, Copy, Debug)]
enum Backend {
    Native,
    Awbc,
    SelectedAwbc,
}

const BACKENDS: [Backend; 3] = [Backend::Native, Backend::Awbc, Backend::SelectedAwbc];

fn executor(source: &str, backend: Backend) -> ArcweftRuntimeExecutor {
    let compiled =
        compile_source(source).expect("Include source compiles through its checked target");
    let [entry] = compiled.plan.entries() else {
        panic!("fixture has one actual Entry")
    };
    let RuntimeEntryTarget::Flow(flow) = &entry.target else {
        panic!("fixture Entry owns a Flow")
    };
    let flow = flow.clone();
    if !matches!(backend, Backend::Native) {
        let lowerer = match backend {
            Backend::SelectedAwbc => AwbcLowerer::for_entry(
                &compiled.plan,
                &compiled.dialogue_content,
                "flow-include.arcw",
                &entry.id,
            ),
            Backend::Awbc => AwbcLowerer::new(
                &compiled.plan,
                &compiled.dialogue_content,
                "flow-include.arcw",
            ),
            Backend::Native => unreachable!(),
        };
        let lowered = lowerer.lower().expect("original Flow calls lower to AWBC");
        let decoded = AwbcProgram::decode_canonical(
            &lowered.program.encode_canonical().unwrap(),
            AwbcDecodeBudget::default(),
        )
        .unwrap();
        let program = AwbcProductProgram::admit(
            Arc::new(decoded),
            AwbcVerifyBudget::default(),
            AwbcVerifyContext::default(),
            RuntimeTaskSourceArtifactLimits::default(),
            None,
        )
        .unwrap();
        ArcweftRuntimeExecutor::from_admitted_awbc_product_flow_invocation(
            program,
            compiled.plan.seal_flow_invocation(flow, []).unwrap(),
        )
        .unwrap()
    } else {
        ArcweftRuntimeExecutor::from_runtime_flow_invocation(
            compiled.plan.seal_flow_invocation(flow, []).unwrap(),
            ArcweftExecutionTier::RuntimePlanVm,
        )
        .unwrap()
    }
}

fn returns(source: &str, expected: &str) {
    for backend in BACKENDS {
        let mut executor = executor(source, backend);
        let mut done = false;
        for step in 0..128 {
            let output = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
            assert!(output.stats.executed_ops <= 1);
            assert!(
                output.output.diagnostics.is_empty(),
                "backend={backend:?}, step={step}, output={:?}, fiber={:?}",
                output.output,
                executor.fiber()
            );
            if matches!(
                executor.fiber().frame.status,
                FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
            ) {
                assert_eq!(
                    executor.fiber().frame.status,
                    FlowFiberStatus::Done(FlowExit::Return(expected.into()))
                );
                done = true;
                break;
            }
        }
        assert!(
            done,
            "Include did not resume its caller; backend={backend:?}"
        );
    }
}

#[test]
fn forward_include_implicitly_completes_and_resumes_native_and_decoded_awbc() {
    returns(
        concat!(
            "flow main() -> String { include @flow.snippet; return \"after\" }\n",
            "flow snippet() {}\n",
            "entry cli @entry.main { goto @flow.main }\n",
        ),
        "after",
    );
}

#[test]
fn include_explicit_unit_and_value_returns_resume_native_and_decoded_awbc() {
    for snippet in [
        "flow snippet() { return () }",
        "flow snippet() -> String { return \"callee\" }",
    ] {
        returns(
            &format!(
                "flow main() -> String {{ include @flow.snippet; return \"after\" }}\n{snippet}\nentry cli @entry.main {{ goto @flow.main }}\n"
            ),
            "after",
        );
    }
}

#[test]
fn repeated_and_nested_includes_share_original_functions_and_resume_their_callers() {
    returns(
        concat!(
            "flow main() -> String { include @flow.first; include @flow.first; return \"after\" }\n",
            "flow first() { include @flow.second }\n",
            "flow second() {}\n",
            "entry cli @entry.main { goto @flow.main }\n",
        ),
        "after",
    );
}

#[test]
fn mutually_recursive_includes_execute_under_the_original_one_operation_budget() {
    let source = concat!(
        "flow main() { include @flow.first }\n",
        "flow first() { include @flow.second }\n",
        "flow second() { include @flow.first }\n",
        "entry cli @entry.main { goto @flow.main }\n",
    );
    for backend in BACKENDS {
        let mut executor = executor(source, backend);
        for _ in 0..16 {
            let output = executor.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
            assert_eq!(output.stats.executed_ops, 1);
            assert!(
                output.output.diagnostics.is_empty(),
                "{:?}",
                output.output.diagnostics
            );
            assert!(!matches!(
                executor.fiber().frame.status,
                FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
            ));
        }
    }
}

#[test]
fn goto_inside_an_include_replaces_the_actual_flow_without_returning_to_its_caller() {
    returns(
        concat!(
            "flow main() -> String { include @flow.snippet; return \"not-reached\" }\n",
            "flow snippet() -> Never { goto @flow.destination }\n",
            "flow destination() -> String { return \"destination\" }\n",
            "entry cli @entry.main { goto @flow.main }\n",
        ),
        "destination",
    );
}
