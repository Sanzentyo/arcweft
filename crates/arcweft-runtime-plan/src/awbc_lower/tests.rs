use super::*;
use arcweft_core::awbc::fiber::FiberState;
use arcweft_core::awbc::schema::{
    AwbcEffectKind, AwbcEntryId, AwbcEntryTarget, AwbcFunctionId, AwbcInstruction, AwbcProgram,
    AwbcTerminator,
};
use arcweft_core::awbc::vm::{self, VmError, VmExit, VmHost, VmObservation, VmStepOptions};
use arcweft_core::effect::{LineEffectRequest, RuntimeCall};
use arcweft_core::entry::{EntryBindingIdentity, RuntimeEntryRoles};
use arcweft_core::plan::{
    EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeFlow, RuntimePlan, RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin,
    RuntimePureInputType, RuntimePureOutputType, RuntimeRouteSpec,
};
use arcweft_core::value::{RuntimeBinaryOp, RuntimeExpr, RuntimeFieldExpr, RuntimeValue};

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

fn with_test_entry(plan: RuntimePlan, flow: FlowRuntimeId) -> RuntimePlan {
    plan.with_entries(vec![RuntimeEntrySpec {
        id: entry_id("test"),
        kind: RuntimeEntryKind::Cli,
        binding: EntryBindingIdentity::from_bytes([1; 32]),
        target: RuntimeEntryTarget::Flow(flow),
        roles: RuntimeEntryRoles::None,
    }])
}

fn run_entry(program: &AwbcProgram, host: &mut impl VmHost) -> VmExit {
    step_entry(program, host).exit
}

fn step_entry(program: &AwbcProgram, host: &mut impl VmHost) -> vm::VmStepOutput {
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

#[derive(Default)]
struct CountingProbeHost {
    calls: usize,
}

impl VmHost for CountingProbeHost {
    fn call_intrinsic(
        &mut self,
        program: &AwbcProgram,
        intrinsic: arcweft_core::awbc::schema::AwbcIntrinsicId,
        _args: &[RuntimeValue],
    ) -> Result<Option<RuntimeValue>, VmError> {
        let record = program
            .intrinsics
            .get(intrinsic.index())
            .ok_or(VmError::MissingIntrinsic(intrinsic))?;
        let label = &program.strings[record.public_id.index()];
        if label != "probe" {
            return Err(VmError::Runtime(format!(
                "unexpected test intrinsic `{label}`"
            )));
        }
        self.calls += 1;
        Ok(Some(RuntimeValue::i64(5)))
    }

    fn call_pure_helper(
        &mut self,
        _program: &AwbcProgram,
        helper: arcweft_core::awbc::schema::AwbcPureHelperId,
        _args: &[RuntimeValue],
    ) -> Result<RuntimeValue, VmError> {
        Err(VmError::Runtime(format!(
            "unexpected pure helper {}",
            helper.0
        )))
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
fn pipe_left_value_is_evaluated_once_and_shared_by_awbc_reads() {
    let source = arcweft_lang_syntax::expr::parse_expr("probe() |> (^ + ^)")
        .expect("pipe expression parses");
    let lowered = crate::expr::lower_runtime_expr_strict(&source)
        .expect("pipe expression lowers to an exact-once binding");
    assert!(matches!(
        &lowered,
        RuntimeExpr::Let { name, body, .. }
            if name.starts_with('\0')
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Binary { lhs, rhs, .. }
                        if matches!(
                            (lhs.as_ref(), rhs.as_ref()),
                            (RuntimeExpr::Local(first), RuntimeExpr::Local(second))
                                if first == name && second == name
                        )
                )
    ));

    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: flow_id("main"),
            ops: vec![FlowOp::ReturnExpr(lowered)],
        }],
        Vec::new(),
    )
    .expect("plan builds");
    let plan = with_test_entry(plan, flow_id("main"));
    let report = lower_plan(&plan);
    let mut host = CountingProbeHost::default();
    let exit = run_entry(&report.program, &mut host);

    assert_eq!(
        host.calls, 1,
        "pipe lhs intrinsic must execute exactly once"
    );
    assert_eq!(exit, VmExit::Returned(Some(RuntimeValue::i64(10))));
}

#[test]
fn lowers_constant_return_plan_to_awbc_tables() {
    let plan = RuntimePlan::new(
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
    let plan = with_test_entry(plan, flow_id("main"));
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
    let plan = with_test_entry(plan, flow_id("main"));
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
    let plan = with_test_entry(plan, flow_id("main"));
    let report = lower_plan(&plan);
    let mut host = TestPureHelperHost;

    assert_eq!(
        run_entry(&report.program, &mut host),
        VmExit::Returned(Some(RuntimeValue::i64(7)))
    );
}

#[test]
fn entry_parameter_inference_keeps_let_scope_locals_inside_block_value() {
    let main = flow_id("main");
    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: main,
            ops: vec![
                FlowOp::LetScope {
                    pattern: arcweft_core::pattern::RuntimePattern::Ident("result".to_owned()),
                    ops: vec![FlowOp::Let {
                        pattern: arcweft_core::pattern::RuntimePattern::Ident("event".to_owned()),
                        expr: RuntimeExpr::Record(vec![RuntimeFieldExpr {
                            name: "value".to_owned(),
                            value: RuntimeExpr::Value(RuntimeValue::String("ok".to_owned())),
                        }]),
                    }],
                    value: RuntimeExpr::Field {
                        target: Box::new(RuntimeExpr::Local("event".to_owned())),
                        field: "value".to_owned(),
                    },
                },
                FlowOp::ReturnExpr(RuntimeExpr::Local("result".to_owned())),
            ],
        }],
        Vec::new(),
    )
    .expect("plan builds");
    let plan = with_test_entry(plan, flow_id("main"));
    let report = lower_plan(&plan);
    let entry = &report.program.entries[0];
    let entry_signature = &report.program.signatures[entry.signature.index()];
    assert!(
        entry_signature.params.is_empty(),
        "block-local value references must not become entry parameters"
    );
    let AwbcEntryTarget::Function(function) = entry.target else {
        panic!("test entry targets a single flow function");
    };
    let function_signature =
        &report.program.signatures[report.program.functions[function.index()].signature.index()];
    assert!(
        function_signature.params.is_empty(),
        "flow function must stay zero-arity for a normal game entry"
    );

    let mut host = TestPureHelperHost;
    assert_eq!(
        run_entry(&report.program, &mut host),
        VmExit::Returned(Some(RuntimeValue::String("ok".to_owned())))
    );
}

#[test]
fn let_scope_exit_emits_registered_cleanup_before_parent_binding() {
    let main = flow_id("main");
    let cleanup = LineEffectRequest::Call(RuntimeCall {
        callee: "presentation.handle.dispose".to_owned(),
        args: vec!["handle = @handle.flow.main.panel".to_owned()],
    });
    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: main,
            ops: vec![
                FlowOp::LetScope {
                    pattern: arcweft_core::pattern::RuntimePattern::Ident("result".to_owned()),
                    ops: vec![
                        FlowOp::RegisterCleanup {
                            key: "handle.flow.main.panel".to_owned(),
                            effect: cleanup,
                        },
                        FlowOp::Let {
                            pattern: arcweft_core::pattern::RuntimePattern::Ident(
                                "event".to_owned(),
                            ),
                            expr: RuntimeExpr::Record(vec![RuntimeFieldExpr {
                                name: "value".to_owned(),
                                value: RuntimeExpr::Value(RuntimeValue::String("ok".to_owned())),
                            }]),
                        },
                    ],
                    value: RuntimeExpr::Field {
                        target: Box::new(RuntimeExpr::Local("event".to_owned())),
                        field: "value".to_owned(),
                    },
                },
                FlowOp::ReturnExpr(RuntimeExpr::Local("result".to_owned())),
            ],
        }],
        Vec::new(),
    )
    .expect("plan builds");
    let plan = with_test_entry(plan, flow_id("main"));
    let report = lower_plan(&plan);
    let mut host = TestPureHelperHost;
    let output = step_entry(&report.program, &mut host);

    assert_eq!(
        output.exit,
        VmExit::Returned(Some(RuntimeValue::String("ok".to_owned())))
    );
    let cleanup_effects = output
        .observations
        .iter()
        .filter_map(|observation| match observation {
            VmObservation::Effect { effect, .. } => Some(*effect),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cleanup_effects.len(),
        1,
        "leaving the let-scope should emit exactly the registered cleanup"
    );
    assert_eq!(
        report.program.effect_plans[cleanup_effects[0].index()].kind,
        AwbcEffectKind::Call
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
    let plan = with_test_entry(plan, flow_id("main"));
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
        binding: EntryBindingIdentity::from_bytes([1; 32]),
        target: RuntimeEntryTarget::Routes(vec![RuntimeRouteSpec {
            method: "GET".to_owned(),
            path: "/next".to_owned(),
            target: next,
            bindings: Vec::new(),
        }]),
        roles: RuntimeEntryRoles::None,
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
