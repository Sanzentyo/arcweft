use super::*;
use arcweft_core::awbc::fiber::FiberState;
use arcweft_core::awbc::schema::{
    AwbcConstant, AwbcEffectKind, AwbcEntryId, AwbcEntryTarget, AwbcFunctionId, AwbcInstruction,
    AwbcPattern, AwbcPatternRest, AwbcProgram, AwbcRuntimeType, AwbcTerminator, AwbcTrapCode,
};
use arcweft_core::awbc::vm::{self, VmError, VmExit, VmHost, VmObservation, VmStepOptions};
use arcweft_core::effect::{LineEffectRequest, RuntimeCall};
use arcweft_core::entry::{EntryBindingIdentity, RuntimeCallableId, RuntimeEntryRoles};
use arcweft_core::entry::{RuntimeNominalTypeId, TypeLayoutHash};
use arcweft_core::pattern::{
    RuntimeCheckedType, RuntimeCheckedVariantCase, RuntimeOpaqueTypeAdmission,
    RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId, RuntimePattern, RuntimePatternRest,
    RuntimeRecordPatternField, RuntimeSemanticTypeId, RuntimeVariantIdentity,
};
use arcweft_core::plan::{
    EntryRuntimeId, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimeFlow, RuntimePlan, RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin,
    RuntimePureInputType, RuntimePureOutputType, RuntimeRouteSpec,
};
use arcweft_core::value::{
    RuntimeAgentCompareOp, RuntimeAgentExpr, RuntimeAgentPredicate, RuntimeAgentPredicateExpr,
    RuntimeAgentProbe, RuntimeAgentProbeExpr, RuntimeAgentValue, RuntimeBinaryOp,
    RuntimeCallTarget, RuntimeExpr, RuntimeFieldExpr, RuntimeNominalRecordExpr,
    RuntimeNominalRecordLayout, RuntimeValue,
};
use std::sync::Arc;

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::canonical(value).expect("test flow ID is valid")
}

fn lower_plan(plan: &RuntimePlan) -> AwbcLowerReport {
    AwbcLowerer::new(
        plan,
        &arcweft_text_model::DialogueContentCatalog::new(),
        "test.arcw",
    )
    .lower()
    .expect("AWBC lowers runtime plan")
}

#[test]
fn pattern_rest_projection_preserves_modes_and_allocates_children_before_rest() {
    let mut inventory = AwbcInventory::new("test.arcw", AwbcLowerOptions::default());
    inventory.intern_runtime_primitives();
    let mut frame = FrameBuilder::new();
    let record = RuntimePattern::Record {
        nominal_layout: None,
        fields: vec![RuntimeRecordPatternField {
            name: "first".to_owned(),
            pattern: RuntimePattern::Ident("field".to_owned()),
        }],
        rest: RuntimePatternRest::Bind("record".to_owned()),
    };
    let record_id = super::pattern::lower_pattern(&mut inventory, &mut frame, &record);
    let exact_id = super::pattern::lower_pattern(
        &mut inventory,
        &mut frame,
        &RuntimePattern::BracketSeq {
            items: Vec::new(),
            rest: RuntimePatternRest::Exact,
        },
    );
    let ignore_id = super::pattern::lower_pattern(
        &mut inventory,
        &mut frame,
        &RuntimePattern::BracketSeq {
            items: Vec::new(),
            rest: RuntimePatternRest::Ignore,
        },
    );
    let whole = RuntimePattern::Whole {
        name: "whole".to_owned(),
        pattern: Box::new(RuntimePattern::Ident("inner".to_owned())),
    };
    let whole_id = super::pattern::lower_pattern(&mut inventory, &mut frame, &whole);
    let program = inventory.finish();

    let AwbcPattern::Record { fields, rest, .. } = &program.patterns[record_id.index()] else {
        panic!("record pattern remains a record");
    };
    let AwbcPattern::Bind { target: child, .. } = program.patterns[fields[0].pattern.index()]
    else {
        panic!("record field binding remains a binding");
    };
    let AwbcPatternRest::Bind(rest) = rest else {
        panic!("record rest remains a binding rest");
    };
    assert!(
        child.0 < rest.0,
        "child registers are allocated before rest"
    );
    assert!(matches!(
        program.patterns[exact_id.index()],
        AwbcPattern::Sequence {
            rest: AwbcPatternRest::Exact,
            ..
        }
    ));
    assert!(matches!(
        program.patterns[ignore_id.index()],
        AwbcPattern::Sequence {
            rest: AwbcPatternRest::Ignore,
            ..
        }
    ));
    let AwbcPattern::Whole { target, inner } = &program.patterns[whole_id.index()] else {
        panic!("whole pattern remains a whole binding");
    };
    let AwbcPattern::Bind {
        target: inner_target,
        ..
    } = &program.patterns[inner.index()]
    else {
        panic!("whole child remains a binding");
    };
    assert!(
        inner_target.0 < target.0,
        "whole child registers are allocated before the whole binding"
    );
    assert_eq!(
        super::pattern::pattern_binding_names(&whole),
        ["inner".to_owned(), "whole".to_owned()]
    );
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

#[test]
fn opaque_checked_types_and_values_lower_to_exact_awbc_rows() {
    let producer =
        RuntimeOpaqueTypeProducerId::try_new("fixture.runtime-plan").expect("valid producer");
    let owner =
        RuntimeOpaqueTypeOwner::exact(producer, RuntimeSemanticTypeId::from_bytes([61; 32]));
    let checked = RuntimeCheckedType::Opaque {
        owner: owner.clone(),
    };
    let value = owner
        .try_wrap(RuntimeValue::String("payload".to_owned()))
        .expect("exact owner wraps a payload");
    let mut inventory = AwbcInventory::new("test.arcw", AwbcLowerOptions::default());
    inventory.intern_runtime_primitives();

    let ty = super::pattern::intern_runtime_type(&mut inventory, &checked);
    let result = RuntimeCheckedType::Result {
        ok: Box::new(RuntimeCheckedType::Unit),
        error: Box::new(checked.clone()),
    };
    let ok_owner = super::pattern::intern_runtime_type(&mut inventory, &result);
    let error_owner = super::pattern::intern_runtime_type(&mut inventory, &result);
    let constant = inventory.constant_runtime_value(&value);
    let mut program = inventory.finish();
    program.canonicalize_string_table();

    assert!(matches!(
        program.runtime_types.get(ty.index()),
        Some(AwbcRuntimeType::Opaque {
            semantic_identity,
            admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
            ..
        }) if semantic_identity == &[61; 32]
    ));
    assert_eq!(ok_owner, error_owner);
    assert!(matches!(
        program.constants.get(constant.index()),
        Some(AwbcConstant::Opaque { ty: actual, payload })
            if *actual == ty && payload.index() < constant.index()
    ));
    program
        .verify(
            arcweft_core::awbc::verify::AwbcVerifyBudget::default(),
            arcweft_core::awbc::verify::AwbcVerifyContext {
                require_entrypoint: false,
                ..arcweft_core::awbc::verify::AwbcVerifyContext::default()
            },
        )
        .expect("lowered opaque rows verify");
}

#[test]
fn result_err_opaque_payload_uses_exact_type_and_rejects_foreign_producer() {
    let expected_producer =
        RuntimeOpaqueTypeProducerId::try_new("fixture.expected").expect("valid producer");
    let foreign_producer =
        RuntimeOpaqueTypeProducerId::try_new("fixture.foreign").expect("valid producer");
    let expected_owner = RuntimeOpaqueTypeOwner::exact(
        expected_producer,
        RuntimeSemanticTypeId::from_bytes([71; 32]),
    );
    let foreign_owner = RuntimeOpaqueTypeOwner::exact(
        foreign_producer,
        RuntimeSemanticTypeId::from_bytes([71; 32]),
    );
    let result_owner = RuntimeCheckedType::Result {
        ok: Box::new(RuntimeCheckedType::Unit),
        error: Box::new(RuntimeCheckedType::Opaque {
            owner: expected_owner.clone(),
        }),
    };
    let lower = |payload_owner: &RuntimeOpaqueTypeOwner| {
        let payload = payload_owner
            .try_wrap(RuntimeValue::String("error".to_owned()))
            .expect("exact owner wraps payload");
        let plan = RuntimePlan::new(
            vec![RuntimeFlow {
                id: flow_id("main"),
                ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Variant {
                    owner: result_owner.clone(),
                    ordinal: 1,
                    name: "Err".to_owned(),
                    payload: Some(Box::new(RuntimeExpr::Value(payload))),
                })],
            }],
            Vec::new(),
        )
        .expect("result plan builds");
        AwbcLowerer::new(
            &with_test_entry(plan, flow_id("main")),
            &arcweft_text_model::DialogueContentCatalog::new(),
            "test.arcw",
        )
        .lower()
    };

    let report = lower(&expected_owner).expect("matching opaque Err lowers");
    let returned = run_entry(&report.program, &mut TestPureHelperHost);
    assert!(matches!(
        returned,
        VmExit::Returned(Some(RuntimeValue::Variant {
            owner: RuntimeVariantIdentity::Result,
            ordinal: 1,
            ref name,
            payload: Some(_),
        })) if name == "Err"
    ));

    let error = lower(&foreign_owner).expect_err("foreign opaque Err must reject");
    assert!(matches!(
        error,
        AwbcLowerError::Verify(arcweft_core::awbc::verify::AwbcVerifyError::TypeMismatch { .. })
    ));
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

#[derive(Default)]
struct OrderedNominalProbeHost {
    calls: Vec<String>,
}

impl VmHost for OrderedNominalProbeHost {
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
        let label = program.strings[record.public_id.index()].clone();
        self.calls.push(label.clone());
        match label.as_str() {
            "probe.z" => Ok(Some(RuntimeValue::String("second".to_owned()))),
            "probe.a" => Ok(Some(RuntimeValue::Bool(true))),
            _ => Err(VmError::Runtime(format!(
                "unexpected nominal test intrinsic `{label}`"
            ))),
        }
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
            VmExit::Cancelled => {
                return Err(VmError::Runtime(
                    "test pure helper was cancelled".to_owned(),
                ));
            }
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
fn let_bound_call_is_evaluated_once_and_shared_by_awbc_reads() {
    let binding = "pipe_left".to_owned();
    let lowered = RuntimeExpr::Let {
        name: binding.clone(),
        expr: Box::new(RuntimeExpr::Call {
            callee: RuntimeCallTarget::callable(
                RuntimeCallableId::try_new("probe").expect("test callable identity"),
            ),
            args: Vec::new(),
        }),
        body: Box::new(RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local(binding.clone())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local(binding)),
        }),
    };
    assert!(matches!(
        &lowered,
        RuntimeExpr::Let { name, body, .. }
            if matches!(
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
        "let-bound callable must execute exactly once"
    );
    assert_eq!(exit, VmExit::Returned(Some(RuntimeValue::i64(10))));
}

#[test]
fn typed_agent_expression_lowers_roundtrips_and_executes_through_make_agent() {
    let expression = RuntimeExpr::Agent(RuntimeAgentExpr::Predicate(
        RuntimeAgentPredicateExpr::Compare {
            probe: Box::new(RuntimeExpr::Agent(RuntimeAgentExpr::Probe(
                RuntimeAgentProbeExpr::Signal {
                    target: Box::new(RuntimeExpr::EntityRef("signal.ready".to_owned())),
                },
            ))),
            op: RuntimeAgentCompareOp::Eq,
            value: Box::new(RuntimeExpr::Value(RuntimeValue::Bool(false))),
        },
    ));
    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: flow_id("main"),
            ops: vec![FlowOp::ReturnExpr(expression)],
        }],
        Vec::new(),
    )
    .expect("plan builds");
    let plan = with_test_entry(plan, flow_id("main"));
    let report = lower_plan(&plan);
    assert_eq!(
        report
            .program
            .instructions
            .iter()
            .filter(|instruction| matches!(instruction, AwbcInstruction::MakeAgent { .. }))
            .count(),
        2
    );

    let encoded = report
        .program
        .encode_canonical()
        .expect("encode typed Agent AWBC");
    let decoded = AwbcProgram::decode_canonical(
        &encoded,
        arcweft_core::awbc::codec::AwbcDecodeBudget::default(),
    )
    .expect("decode typed Agent AWBC");
    let mut host = TestPureHelperHost;
    let exit = run_entry(&decoded, &mut host);

    assert!(matches!(
        exit,
        VmExit::Returned(Some(RuntimeValue::Agent(RuntimeAgentValue::Predicate(
            RuntimeAgentPredicate::Compare {
                probe: RuntimeAgentProbe::Signal { ref target },
                op: RuntimeAgentCompareOp::Eq,
                ref value,
            }
        )))) if target.as_str() == "signal.ready" && value.as_ref() == &RuntimeValue::Bool(false)
    ));
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
    let dialogue_content = arcweft_text_model::DialogueContentCatalog::new();
    let report = AwbcLowerer::new(&plan, &dialogue_content, "test.arcw")
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
fn nominal_record_expression_lowers_and_executes_with_layout_identity() {
    let layout = Arc::new(
        RuntimeNominalRecordLayout::try_from_checked_projection(
            RuntimeNominalTypeId::try_new("game.Pair").unwrap(),
            RuntimeSemanticTypeId::from_bytes([21; 32]),
            TypeLayoutHash::from_bytes([22; 32]),
            vec![
                ("alpha".to_owned(), RuntimeCheckedType::Bool),
                ("zeta".to_owned(), RuntimeCheckedType::String),
            ],
        )
        .unwrap(),
    );
    let expression = RuntimeNominalRecordExpr::try_from_checked_initializers(
        layout.clone(),
        vec![
            (
                "zeta".to_owned(),
                RuntimeExpr::Call {
                    callee: RuntimeCallTarget::callable(
                        RuntimeCallableId::try_new("probe.z").unwrap(),
                    ),
                    args: Vec::new(),
                },
            ),
            (
                "alpha".to_owned(),
                RuntimeExpr::Call {
                    callee: RuntimeCallTarget::callable(
                        RuntimeCallableId::try_new("probe.a").unwrap(),
                    ),
                    args: Vec::new(),
                },
            ),
        ],
    )
    .unwrap();
    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: flow_id("main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::NominalRecord(expression))],
        }],
        Vec::new(),
    )
    .unwrap();
    let report = lower_plan(&with_test_entry(plan, flow_id("main")));

    let descriptor = report
        .program
        .runtime_types
        .iter()
        .find_map(|ty| match ty {
            AwbcRuntimeType::NominalRecord { fields, .. } => Some(fields),
            _ => None,
        })
        .expect("nominal expression publishes one executable descriptor");
    assert_eq!(report.program.strings[descriptor[0].name.index()], "alpha");
    assert_eq!(report.program.strings[descriptor[1].name.index()], "zeta");

    let mut host = OrderedNominalProbeHost::default();
    let VmExit::Returned(Some(RuntimeValue::NominalRecord(value))) =
        run_entry(&report.program, &mut host)
    else {
        panic!("nominal record must survive AWBC execution");
    };
    assert_eq!(value.type_id(), layout.nominal());
    assert_eq!(value.layout(), layout.layout());
    assert_eq!(host.calls, ["probe.z", "probe.a"]);
    assert_eq!(
        value.fields(),
        &[
            RuntimeValue::Bool(true),
            RuntimeValue::String("second".to_owned())
        ]
    );
}

#[test]
fn generated_awbc_typed_bindings_match_choice_and_nominal_and_reject_mismatch() {
    let nominal = arcweft_core::entry::RuntimeNominalTypeId::try_new("game.State")
        .expect("test nominal identity");
    let nominal_variant_type = RuntimeCheckedType::Variant {
        nominal: nominal.clone(),
        semantic_identity: RuntimeSemanticTypeId::from_bytes([9; 32]),
        cases: ["Idle", "Running", "Paused", "Ready"]
            .into_iter()
            .map(|name| RuntimeCheckedVariantCase {
                name: name.to_owned(),
                payload: None,
            })
            .collect(),
    };
    let expected = RuntimeCheckedType::Choice(vec![
        RuntimeCheckedType::Result {
            ok: Box::new(RuntimeCheckedType::Unit),
            error: Box::new(RuntimeCheckedType::String),
        },
        nominal_variant_type.clone(),
    ]);
    let lower = |expr: RuntimeExpr| {
        let plan = RuntimePlan::new(
            vec![RuntimeFlow {
                id: flow_id("main"),
                ops: vec![
                    FlowOp::Let {
                        pattern: RuntimePattern::Typed {
                            name: "value".to_owned(),
                            ty: expected.clone(),
                        },
                        expr,
                    },
                    FlowOp::ReturnExpr(RuntimeExpr::Local("value".to_owned())),
                ],
            }],
            Vec::new(),
        )
        .expect("typed binding plan builds");
        lower_plan(&with_test_entry(plan, flow_id("main")))
    };

    let result_value = RuntimeValue::result_ok(RuntimeValue::Unit);
    let result_report = lower(RuntimeExpr::Variant {
        owner: RuntimeCheckedType::Result {
            ok: Box::new(RuntimeCheckedType::Unit),
            error: Box::new(RuntimeCheckedType::String),
        },
        ordinal: 0,
        name: "Ok".to_owned(),
        payload: Some(Box::new(RuntimeExpr::Value(RuntimeValue::Unit))),
    });
    assert_eq!(
        run_entry(&result_report.program, &mut TestPureHelperHost),
        VmExit::Returned(Some(result_value))
    );

    let nominal_value = RuntimeValue::Variant {
        owner: RuntimeVariantIdentity::Nominal {
            nominal,
            semantic_identity: RuntimeSemanticTypeId::from_bytes([9; 32]),
        },
        ordinal: 3,
        name: "Ready".to_owned(),
        payload: None,
    };
    let nominal_report = lower(RuntimeExpr::Variant {
        owner: nominal_variant_type,
        ordinal: 3,
        name: "Ready".to_owned(),
        payload: None,
    });
    assert_eq!(
        run_entry(&nominal_report.program, &mut TestPureHelperHost),
        VmExit::Returned(Some(nominal_value))
    );

    let mismatch_report = lower(RuntimeExpr::Value(RuntimeValue::Bool(true)));
    let mut mismatch_fiber =
        FiberState::for_entry(&mismatch_report.program, AwbcEntryId(0), 0, 256)
            .expect("AWBC mismatch fiber initializes");
    let mismatch = vm::step(
        &mismatch_report.program,
        &mut mismatch_fiber,
        VmStepOptions {
            max_instructions: 128,
        },
    )
    .expect("typed mismatch traps instead of escaping the VM");
    assert!(matches!(
        mismatch.exit,
        VmExit::Trapped(ref trap) if trap.code == AwbcTrapCode::PatternMismatch
    ));
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
    let dialogue_content = arcweft_text_model::DialogueContentCatalog::new();
    let report = AwbcLowerer::new(&plan, &dialogue_content, "test.arcw")
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
