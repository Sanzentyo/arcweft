use super::*;
use arcweft_core::awbc::fiber::FiberState;
use arcweft_core::awbc::schema::{
    AwbcEntryId, AwbcEntryTarget, AwbcFunctionId, AwbcInstruction, AwbcProgram, AwbcTerminator,
};
use arcweft_core::awbc::vm::{self, VmError, VmExit, VmHost, VmStepOptions};
use arcweft_core::plan::{
    EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeFlow, RuntimePlan, RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin,
    RuntimePureInputType, RuntimePureOutputType, RuntimeRouteSpec,
};
use arcweft_core::value::{RuntimeBinaryOp, RuntimeExpr, RuntimeValue};

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::canonical(value).expect("test flow ID is valid")
}

fn lower_plan(plan: &RuntimePlan) -> AwbcLowerReport {
    AwbcLowerer::new(
        plan,
        &arcweft_render_text::LineDisplayCatalog::default(),
        "test.arcw",
    )
    .lower()
    .expect("AWBC lowers runtime plan")
}

fn entry_id(value: &str) -> EntryRuntimeId {
    EntryRuntimeId::canonical(value).expect("test entry ID is valid")
}

fn run_entry(program: &AwbcProgram, host: &mut impl VmHost) -> VmExit {
    let mut fiber =
        FiberState::for_entry(program, AwbcEntryId(0), 0, 256).expect("AWBC fiber initializes");
    vm::step_with_host(
        program,
        &mut fiber,
        VmStepOptions {
            max_instructions: 128,
        },
        host,
    )
    .expect("AWBC VM executes entry")
    .exit
}

#[derive(Default)]
struct TestPureHelperHost;

impl VmHost for TestPureHelperHost {
    fn call_intrinsic(
        &mut self,
        _program: &AwbcProgram,
        intrinsic: arcweft_core::awbc::schema::AwbcIntrinsicId,
        _args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, VmError> {
        Err(VmError::MissingIntrinsic(intrinsic))
    }

    fn call_pure_helper(
        &mut self,
        program: &AwbcProgram,
        helper: arcweft_core::awbc::schema::AwbcPureHelperId,
        args: &[RuntimeValue],
    ) -> Result<RuntimeValue, VmError> {
        let function = program
            .pure_helpers
            .get(helper.index())
            .map(|record| record.function)
            .ok_or_else(|| VmError::Runtime(format!("missing pure helper {}", helper.0)))?;
        run_function(program, function, args, self)
    }
}

fn run_function(
    program: &AwbcProgram,
    function: AwbcFunctionId,
    args: &[RuntimeValue],
    host: &mut impl VmHost,
) -> Result<RuntimeValue, VmError> {
    let mut fiber = FiberState::for_function(program, AwbcEntryId(0), function, 0, 256)?;
    fiber
        .active_frame_mut()?
        .bind_positional_arguments(program, args)?;
    loop {
        let output = vm::step_with_host(
            program,
            &mut fiber,
            VmStepOptions {
                max_instructions: 128,
            },
            host,
        )?;
        match output.exit {
            VmExit::Running => {}
            VmExit::Returned(value) => return Ok(value.unwrap_or(RuntimeValue::Unit)),
            VmExit::Trapped(trap) => {
                return Err(VmError::Runtime(format!(
                    "test pure helper trapped: {trap:?}"
                )));
            }
            VmExit::Suspended(reason) => {
                return Err(VmError::Runtime(format!(
                    "test pure helper suspended: {reason:?}"
                )));
            }
            VmExit::BudgetYield(_) => {
                return Err(VmError::Runtime(
                    "test pure helper budget-yielded".to_owned(),
                ));
            }
        }
    }
}

#[test]
fn lowers_constant_return_plan_to_awbc_tables() {
    let plan = RuntimePlan::new(
        Some(flow_id("main")),
        vec![RuntimeFlow {
            id: flow_id("main"),
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
        Some(flow_id("main")),
        vec![RuntimeFlow {
            id: flow_id("main"),
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

#[test]
fn generated_awbc_partial_apply_returns_function_value() {
    let plan = RuntimePlan::new(
        Some(flow_id("main")),
        vec![RuntimeFlow {
            id: flow_id("main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Apply {
                callee: Box::new(RuntimeExpr::Function {
                    params: vec!["x".to_owned(), "y".to_owned()],
                    body: Box::new(RuntimeExpr::Local("y".to_owned())),
                }),
                args: vec![RuntimeExpr::Value(RuntimeValue::i64(2))],
            })],
        }],
        Vec::new(),
    )
    .expect("plan builds");
    let report = lower_plan(&plan);
    let mut host = TestPureHelperHost;

    let VmExit::Returned(Some(RuntimeValue::Function(function))) =
        run_entry(&report.program, &mut host)
    else {
        panic!("expected partial apply to return a function value");
    };
    assert_eq!(function.arity(), 1);
    assert_eq!(function.captures.len(), 1);
}

#[test]
fn generated_awbc_curried_closure_apply_executes_returned_function() {
    let make_adder = RuntimeExpr::Function {
        params: vec!["x".to_owned()],
        body: Box::new(RuntimeExpr::Function {
            params: vec!["y".to_owned()],
            body: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("x".to_owned())),
                op: RuntimeBinaryOp::Add,
                rhs: Box::new(RuntimeExpr::Local("y".to_owned())),
            }),
        }),
    };
    let plan = RuntimePlan::new(
        Some(flow_id("main")),
        vec![RuntimeFlow {
            id: flow_id("main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Apply {
                callee: Box::new(RuntimeExpr::Apply {
                    callee: Box::new(make_adder),
                    args: vec![RuntimeExpr::Value(RuntimeValue::i64(2))],
                }),
                args: vec![RuntimeExpr::Value(RuntimeValue::i64(5))],
            })],
        }],
        Vec::new(),
    )
    .expect("plan builds");
    let report = lower_plan(&plan);
    let mut host = TestPureHelperHost;

    assert_eq!(
        run_entry(&report.program, &mut host),
        VmExit::Returned(Some(RuntimeValue::i64(7)))
    );
}

#[test]
fn generated_awbc_function_value_apply_can_call_pure_helper_body() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "add_pair".to_owned(),
        input_names: vec!["lhs".to_owned(), "rhs".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("lhs".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("rhs".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    };
    let helper_value = RuntimeExpr::Function {
        params: vec!["lhs".to_owned(), "rhs".to_owned()],
        body: Box::new(RuntimeExpr::PureCall {
            helper: RuntimePureHelperId(0),
            args: vec![
                RuntimeExpr::Local("lhs".to_owned()),
                RuntimeExpr::Local("rhs".to_owned()),
            ],
        }),
    };
    let plan = RuntimePlan::new(
        Some(flow_id("main")),
        vec![RuntimeFlow {
            id: flow_id("main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Apply {
                callee: Box::new(helper_value),
                args: vec![
                    RuntimeExpr::Value(RuntimeValue::i64(11)),
                    RuntimeExpr::Value(RuntimeValue::i64(31)),
                ],
            })],
        }],
        Vec::new(),
    )
    .expect("plan builds")
    .with_pure_helpers(vec![helper]);
    let report = lower_plan(&plan);
    let mut host = TestPureHelperHost;

    assert_eq!(
        run_entry(&report.program, &mut host),
        VmExit::Returned(Some(RuntimeValue::i64(42)))
    );
}

#[test]
fn awbc_flow_target_resolution_uses_typed_runtime_ids() {
    let main = flow_id("chapter.main");
    let next = flow_id("chapter.next");
    let plan = RuntimePlan::new(
        Some(main.clone()),
        vec![
            RuntimeFlow {
                id: main,
                ops: vec![FlowOp::Goto(next.clone())],
            },
            RuntimeFlow {
                id: next.clone(),
                ops: vec![FlowOp::Return("ok".to_owned())],
            },
        ],
        Vec::new(),
    )
    .expect("plan builds")
    .with_entries(vec![RuntimeEntrySpec {
        id: entry_id("server"),
        kind: RuntimeEntryKind::Server,
        target: RuntimeEntryTarget::Routes(vec![RuntimeRouteSpec {
            method: "GET".to_owned(),
            path: "/next".to_owned(),
            target: next,
            bindings: Vec::new(),
        }]),
    }]);
    let report = lower_plan(&plan);
    let goto_target = report
        .program
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            AwbcTerminator::GotoStatic { function, .. } => Some(function),
            _ => None,
        })
        .expect("static goto lowers to a function target");
    let route_target = match &report.program.entries[0].target {
        AwbcEntryTarget::Routes(routes) => routes[0].target,
        AwbcEntryTarget::Function(_) => panic!("test entry must lower as routes"),
    };

    assert_eq!(goto_target, route_target);
    assert!(!report.program.intrinsics.iter().any(|intrinsic| {
        report.program.strings[intrinsic.public_id.index()]
            .as_str()
            .starts_with("goto.static:")
    }));
}
