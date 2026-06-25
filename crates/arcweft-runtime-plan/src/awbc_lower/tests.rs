use super::*;
use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};
use arcweft_core::value::{RuntimeExpr, RuntimeValue};

#[test]
fn lowers_constant_return_plan_to_awbc_tables() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![FlowOp::Let {
                pattern: arcweft_core::pattern::RuntimePattern::Ident("x".to_owned()),
                expr: RuntimeExpr::Value(RuntimeValue::i64(7)),
            }],
        }],
        Vec::new(),
    )
    .expect("plan builds");
    let report = lower_runtime_plan_to_awbc_with_input(AwbcLowerInput {
        plan: &plan,
        display: &arcweft_render_text::LineDisplayCatalog::default(),
        source_label: "test.arcw",
        options: AwbcLowerOptions {
            verify: false,
            ..AwbcLowerOptions::default()
        },
    })
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
