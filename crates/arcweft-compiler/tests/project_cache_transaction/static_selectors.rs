use super::*;
use arcweft_core::{
    awbc::{
        codec::AwbcDecodeBudget,
        schema::{AwbcEntryId, AwbcProgram},
    },
    effect::LineEffectRequest,
    engine::{Engine, FlowExit, FlowFiberStatus},
    executor::{ArcweftRuntimeExecutor, ArcweftRuntimeExecutorSnapshot},
    pure::{RuntimeExternalCallBackend, RuntimeExternalCallContext, VmRuntimePureCallBackend},
    step::{
        RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepResult,
    },
    value::{RuntimeCallTarget, RuntimeEvalError, RuntimeValue},
};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::sync::Mutex;

#[test]
fn static_selector_and_value_callee_keep_exact_source_order_in_native_and_awbc() {
    assert_callable_execution(
        r#"let value = api.combine(label = mark("label", "units"), second = factory()(mark("second", 2i64)), first = mark("first", 1i64))"#,
        false,
        &[
            "label", "callee", "second", "body", "first", "combine", "12", "receiver", "x",
        ],
    );
}

#[test]
fn external_callable_value_keeps_exact_source_order_in_native_and_awbc() {
    assert_callable_execution(
        r#"
    let combine = api.combine
    let value = combine(label = mark("label", "units"), second = factory()(mark("second", 2i64)), first = mark("first", 1i64))
"#,
        false,
        &[
            "label", "callee", "second", "body", "first", "combine", "12", "receiver", "x",
        ],
    );
}

#[test]
fn external_callable_value_retains_groups_and_captured_arguments_in_native_and_awbc() {
    assert_callable_execution(
        r#"
    let combine = api.combine
    let captured = combine(mark("first", 1i64))
    let value = captured(label = mark("label", "units"), second = factory()(mark("second", 2i64)))
"#,
        true,
        &[
            "first", "label", "callee", "second", "body", "combine", "12", "receiver", "x",
        ],
    );
}

#[test]
fn effectful_external_callable_value_keeps_its_frame_and_source_order_across_awbc_restoration() {
    assert_callable_execution_with_effects(
        r#"
    let combine = api.combine
    let captured = combine(mark("first", 1i64))
    let value = captured(label = mark("label", "units"), second = factory()(mark("second", 2i64)))
"#,
        true,
        &[
            "first", "label", "callee", "second", "body", "combine", "12", "receiver", "x",
        ],
        true,
    );
}

fn assert_callable_execution(call_source: &str, staged: bool, expected_events: &[&str]) {
    assert_callable_execution_with_effects(call_source, staged, expected_events, false);
}

fn assert_callable_execution_with_effects(
    call_source: &str,
    staged: bool,
    expected_events: &[&str],
    effectful: bool,
) {
    let manifest = selector_manifest(staged, effectful);
    let source = [
        r#"
entry cli @entry.main { goto @flow.main }
fn mark<T>(label: String, value: T) -> T effects { log.write } {
    log.info(label)
    value
}
flow main() {
    let factory = || { log.info("callee"); |value: i64| { log.info("body"); value } }
"#,
        call_source,
        r#"
    log.info(value)
    let text = mark("receiver", " x ").trim()
    log.info(text)
}
"#,
    ]
    .concat();
    let (project, facts, base) = fixture_with_manifest(&source, "static-selector", &manifest);
    let compiled = AttachedCompiler::new(&project)
        .compile(
            &project,
            &context(base, facts),
            &mut RecordingCache::default(),
        )
        .expect(
            "static namespace selects its function without requiring a runtime namespace value",
        );
    let declared_targets = compiled
        .analysis_lease()
        .final_analysis()
        .checked_callables()
        .records()
        .filter(|facts| {
            matches!(facts.provider(), arcweft_lang_sema::callable::CallableProviderId::Adapter(package) if package.as_str() == "selector-api")
        })
        .map(|facts| facts.id().runtime_target().unwrap())
        .collect::<Vec<_>>();
    let [declared_target] = declared_targets.as_slice() else {
        panic!("one accepted declaration from the fixture provider")
    };
    let runtime = compiled.runtime_plan();
    let report = AwbcLowerer::new(
        &runtime.plan,
        &runtime.dialogue_content_catalog,
        "selectors.arcw",
    )
    .lower()
    .unwrap();
    let decoded = AwbcProgram::decode_canonical(
        &report.program.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    assert_eq!(decoded, report.program);
    let [flow] = runtime.plan.flows() else {
        panic!("one flow")
    };
    let mut native = Engine::for_flow(runtime.plan.clone(), &flow.id).unwrap();
    let mut awbc =
        Some(ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0)).unwrap());
    let native_observed = Arc::new(Mutex::new(Observed::default()));
    let awbc_observed = Arc::new(Mutex::new(Observed::default()));
    let mut native_backend = VmRuntimePureCallBackend::default().with_external_calls(Combine {
        observed: Arc::clone(&native_observed),
    });
    let mut awbc_backend = VmRuntimePureCallBackend::default().with_external_calls(Combine {
        observed: Arc::clone(&awbc_observed),
    });
    run_one_op(&native_observed, &mut |input, options| {
        native.step_with_pure_backend(input, options, &mut native_backend)
    });
    run_one_op(&awbc_observed, &mut |input, options| {
        let mut executor = awbc.take().expect("one owned AWBC executor");
        let result = executor.step_with_pure_backend(input, options, &mut awbc_backend);
        let snapshot =
            match executor.snapshot().expect("save every OneOp boundary") {
                ArcweftRuntimeExecutorSnapshot::AwbcProduct(saved) => {
                    let saved =
                    arcweft_core::awbc::product_step::AwbcProductExecutorSaveSnapshot::from_live(
                        &saved, executor.admitted_product_awbc_program().unwrap())
                    .unwrap();
                    let bytes = serde_json::to_vec(&saved).unwrap();
                    let decoded = serde_json::from_slice(&bytes).unwrap();
                    ArcweftRuntimeExecutorSnapshot::AwbcProduct(
                        executor.bind_product_save_snapshot(decoded).unwrap(),
                    )
                }
            };
        let expected = snapshot.clone();
        let restored = executor
            .restore_snapshot(snapshot)
            .expect("restore for the original program");
        assert_eq!(restored.snapshot().unwrap(), expected);
        awbc = Some(restored);
        result
    });
    let native_observed = native_observed.lock().unwrap();
    let awbc_observed = awbc_observed.lock().unwrap();
    assert_eq!(*native_observed, *awbc_observed);
    assert_eq!(native_observed.events, expected_events);
    let [(target, args)] = native_observed.calls.as_slice() else {
        panic!("one registered invocation")
    };
    assert_eq!(target, declared_target);
    let RuntimeCallTarget::External(target) = target else {
        panic!("selected external declaration")
    };
    assert!(
        target
            .identity()
            .as_str()
            .starts_with("arcweft.checked.v1.")
    );
    assert_eq!(
        target.declaration().provider(),
        &arcweft_core::value::RuntimeExternalCallableProvider::Adapter("selector-api".into())
    );
    assert_eq!(
        target.declaration().export(),
        &arcweft_core::value::RuntimeExternalCallableExport::Function(
            ["api".into(), "combine".into()].into()
        )
    );
    assert_eq!(target.declaration().overload(), 0);
    assert_eq!(
        args,
        &[
            RuntimeValue::i64(1),
            RuntimeValue::i64(2),
            RuntimeValue::String("units".into())
        ]
    );
}

fn selector_manifest(staged: bool, effectful: bool) -> AdapterManifest {
    let manifest = AdapterManifest::new("selector-api", "Selector API");
    let effects = if effectful {
        vec![arcweft_adapter_context::manifest::AdapterEffectCapability::new("log.write")]
    } else {
        vec![]
    };
    let manifest = effects
        .iter()
        .cloned()
        .fold(manifest, AdapterManifest::with_effect);
    let group_specs = if staged {
        vec![
            vec![("first", AdapterTypeKind::I64)],
            vec![
                ("second", AdapterTypeKind::I64),
                ("label", AdapterTypeKind::String),
            ],
        ]
    } else {
        vec![vec![
            ("first", AdapterTypeKind::I64),
            ("second", AdapterTypeKind::I64),
            ("label", AdapterTypeKind::String),
        ]]
    };
    let groups = group_specs
        .into_iter()
        .enumerate()
        .map(|(group, params)| {
            let params = params
                .into_iter()
                .enumerate()
                .map(|(index, (name, ty))| {
                    AdapterFunctionParam::try_new(
                        AdapterCallableParameterIndex::try_from_usize(index).unwrap(),
                        Some(AdapterCallableName::try_new(name).unwrap()),
                        ty,
                        AdapterParameterPassing::PositionalOrNamed,
                        AdapterParameterPresence::Required,
                    )
                    .unwrap()
                })
                .collect();
            AdapterParameterGroup::try_new(
                AdapterCallableGroupIndex::try_from_usize(group).unwrap(),
                params,
            )
            .unwrap()
        })
        .collect();
    let signature = AdapterFunctionSignature::try_new(groups, AdapterTypeKind::I64).unwrap();
    manifest.with_function_signature(
        AdapterCallablePath::try_new(
            ["api", "combine"].map(|name| AdapterCallableName::try_new(name).unwrap()),
        )
        .unwrap(),
        AdapterCallableOverloadIndex::try_from_usize(0).unwrap(),
        signature,
        effects,
    )
}

#[derive(Debug, Default, PartialEq)]
struct Observed {
    events: Vec<String>,
    calls: Vec<(RuntimeCallTarget, Vec<RuntimeValue>)>,
}

struct Combine {
    observed: Arc<Mutex<Observed>>,
}

impl RuntimeExternalCallBackend for Combine {
    fn call_external(
        &mut self,
        _context: &RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        let RuntimeCallTarget::External(target) = callee else {
            return None;
        };
        let declaration = target.declaration();
        if declaration.provider()
            != &arcweft_core::value::RuntimeExternalCallableProvider::Adapter("selector-api".into())
            || declaration.export()
                != &arcweft_core::value::RuntimeExternalCallableExport::Function(
                    ["api".into(), "combine".into()].into(),
                )
            || declaration.overload() != 0
        {
            return None;
        }
        let [
            RuntimeValue::Int(first),
            RuntimeValue::Int(second),
            RuntimeValue::String(label),
        ] = args
        else {
            panic!("the admitted scalar ABI")
        };
        assert_eq!(label.as_str(), "units");
        let first = i64::try_from(first.as_i128()).unwrap();
        let second = i64::try_from(second.as_i128()).unwrap();
        let mut observed = self.observed.lock().unwrap();
        observed.events.push("combine".to_owned());
        observed.calls.push((callee.clone(), args.to_vec()));
        Some(Ok(RuntimeValue::i64(first * 10 + second)))
    }
}

fn run_one_op(
    observed: &Arc<Mutex<Observed>>,
    step: &mut dyn FnMut(RuntimeStepInput, RuntimeStepOptions) -> RuntimeStepResult,
) {
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..RuntimeStepOptions::default()
    };
    for _ in 0..512 {
        let result = step(RuntimeStepInput::default(), options);
        assert!(result.stats.executed_ops <= 1);
        assert!(
            result.output.diagnostics.is_empty(),
            "{:?}",
            result.output.diagnostics
        );
        for effect in result.output.effects.line {
            if let LineEffectRequest::Log(log) = effect {
                observed.lock().unwrap().events.push(log.message);
            }
        }
        match result.fiber_status {
            FlowFiberStatus::Running => {}
            FlowFiberStatus::Done(FlowExit::Done) => return,
            status => panic!("unexpected selector execution status: {status:?}"),
        }
    }
    panic!("selector execution exceeded its deterministic OneOp limit");
}
