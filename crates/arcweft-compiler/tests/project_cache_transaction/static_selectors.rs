use super::*;
use arcweft_core::{
    awbc::{
        codec::AwbcDecodeBudget,
        schema::{AwbcEntryId, AwbcProgram},
    },
    effect::LineEffectRequest,
    engine::{Engine, FlowExit, FlowFiberStatus},
    executor::ArcweftRuntimeExecutor,
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
    let manifest = selector_manifest();
    let (project, facts, base) = fixture_with_manifest(
        r#"
entry cli @entry.main { goto @flow.main }
fn mark<T>(label: String, value: T) -> T effects { log.write } {
    log.info(label)
    value
}
flow main() {
    let factory = || { log.info("callee"); |value: i64| { log.info("body"); value } }
    let value = api.combine(label = mark("label", "units"), second = factory()(mark("second", 2i64)), first = mark("first", 1i64))
    log.info(value)
    let text = mark("receiver", " x ").trim()
    log.info(text)
}
"#,
        "static-selector",
        &manifest,
    );
    let compiled = AttachedCompiler::new(&project)
        .compile(
            &project,
            &context(base, facts),
            &mut RecordingCache::default(),
        )
        .expect("static namespace selects its method without requiring a runtime namespace value");
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
    let mut awbc = ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0)).unwrap();
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
        awbc.step_with_pure_backend(input, options, &mut awbc_backend)
    });
    let native_observed = native_observed.lock().unwrap();
    let awbc_observed = awbc_observed.lock().unwrap();
    assert_eq!(*native_observed, *awbc_observed);
    assert_eq!(
        native_observed.events,
        [
            "label", "callee", "second", "body", "first", "combine", "12", "receiver", "x"
        ]
    );
    let [(target, args)] = native_observed.calls.as_slice() else {
        panic!("one registered invocation")
    };
    assert!(matches!(target, RuntimeCallTarget::Callable(_)));
    assert_eq!(
        args,
        &[
            RuntimeValue::i64(1),
            RuntimeValue::i64(2),
            RuntimeValue::String("units".into())
        ]
    );
}

fn selector_manifest() -> AdapterManifest {
    let manifest = AdapterManifest::new("selector-api", "Selector API");
    let params = [
        ("first", AdapterTypeKind::I64),
        ("second", AdapterTypeKind::I64),
        ("label", AdapterTypeKind::String),
    ]
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
    let signature = AdapterFunctionSignature::try_new(
        vec![
            AdapterParameterGroup::try_new(
                AdapterCallableGroupIndex::try_from_usize(0).unwrap(),
                params,
            )
            .unwrap(),
        ],
        AdapterTypeKind::I64,
    )
    .unwrap();
    manifest.with_function_signature(
        AdapterCallablePath::try_new(
            ["api", "combine"].map(|name| AdapterCallableName::try_new(name).unwrap()),
        )
        .unwrap(),
        AdapterCallableOverloadIndex::try_from_usize(0).unwrap(),
        signature,
        [],
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
        let RuntimeCallTarget::Callable(_) = callee else {
            return None;
        };
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
