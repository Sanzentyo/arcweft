use super::*;
use arcweft_core::awbc::fiber::FiberState;
use arcweft_core::awbc::schema::{AwbcEntryId, AwbcInstruction};
use arcweft_core::awbc::vm::{self, VmExit, VmStepOptions};
use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};

#[test]
fn lowers_constant_return_plan_to_awbc_tables() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("main".to_owned()),
            ops: vec![FlowOp::Let {
                pattern: arcweft_core::pattern::RuntimePattern::Ident("x".to_owned()),
                expr: RuntimeExpr::Value(RuntimeValue::i64(7)),
            }],
        }],
        Vec::new(),
    )
    .expect("plan builds");
    let display = arcweft_render_text::LineDisplayCatalog::default();
    let report = AwbcLowerer::new(&plan, &display, "test.arcw")
        .with_options(AwbcLowerOptions {
            verify: false,
            ..AwbcLowerOptions::default()
        })
        .lower()
        .expect("AWBC lowers");
    assert_eq!(report.program.functions.len(), 1);
    assert!(!report.program.instructions.is_empty());
    assert!(
        report
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.is_error())
    );
}

#[test]
fn lowers_runtime_function_apply_to_awbc_closure_instructions() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("main".to_owned()),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Apply {
                callee: Box::new(RuntimeExpr::Function {
                    params: vec!["x".to_owned()],
                    body: Box::new(RuntimeExpr::Local("x".to_owned())),
                }),
                args: vec![RuntimeExpr::Value(RuntimeValue::String("ok".to_owned()))],
            })],
        }],
        Vec::new(),
    )
    .expect("plan builds");
    let display = arcweft_render_text::LineDisplayCatalog::default();
    let report = AwbcLowerer::new(&plan, &display, "test.arcw")
        .lower()
        .expect("AWBC lowers runtime function apply");

    assert!(report.program.instructions.iter().any(|instruction| {
        matches!(instruction, AwbcInstruction::MakeFunction { params, .. } if params.len() == 1)
    }));
    assert!(
        report
            .program
            .instructions
            .iter()
            .any(|instruction| matches!(instruction, AwbcInstruction::ApplyFunction { .. }))
    );
    assert!(!report.program.intrinsics.iter().any(|intrinsic| {
        report.program.strings[intrinsic.public_id.index()].as_str() == "function.apply"
    }));

    let mut fiber = FiberState::for_entry(&report.program, AwbcEntryId(0), 0, 64)
        .expect("AWBC fiber initializes");
    let output = vm::step(
        &report.program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 32,
        },
    )
    .expect("AWBC VM executes closure apply");
    assert_eq!(
        output.exit,
        VmExit::Returned(Some(RuntimeValue::String("ok".to_owned())))
    );
}
