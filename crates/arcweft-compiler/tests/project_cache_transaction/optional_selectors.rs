//! A selected declaration retains omission independently of its structural arrow.

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
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn defaulted_external_value_keeps_omitted_formals_in_compiler_projection() {
    let parameter = AdapterFunctionParam::try_new(
        AdapterCallableParameterIndex::try_from_usize(0).unwrap(),
        Some(AdapterCallableName::try_new("value").unwrap()),
        AdapterTypeKind::I64,
        AdapterParameterPassing::PositionalOrNamed,
        AdapterParameterPresence::Defaulted,
    )
    .unwrap();
    let group = AdapterParameterGroup::try_new(
        AdapterCallableGroupIndex::try_from_usize(0).unwrap(),
        vec![parameter],
    )
    .unwrap();
    let signature = AdapterFunctionSignature::try_new(vec![group], AdapterTypeKind::I64).unwrap();
    let manifest = AdapterManifest::new("optional-api", "Optional API").with_function_signature(
        AdapterCallablePath::try_new(
            ["api", "choose"].map(|name| AdapterCallableName::try_new(name).unwrap()),
        )
        .unwrap(),
        AdapterCallableOverloadIndex::try_from_usize(0).unwrap(),
        signature,
        vec![],
    );
    for (case, call) in [
        ("direct-omitted", "let value = api.choose()"),
        (
            "alias-present",
            "let choose = api.choose; let value = choose(value = 7i64)",
        ),
        (
            "alias-omitted",
            "let choose = api.choose; let value = choose()",
        ),
    ] {
        let source = format!(
            "entry cli @entry.main {{ goto @flow.main }}\nflow main() {{\n{call}\nlog.info(value)\n}}\n"
        );
        let (project, facts, base) = fixture_with_manifest(&source, case, &manifest);
        let compiled = AttachedCompiler::new(&project)
            .compile(
                &project,
                &context(base, facts),
                &mut RecordingCache::default(),
            )
            .unwrap_or_else(|error| panic!("{case} must retain the declared omission: {error:?}"));
        let expected = if case == "alias-present" {
            Some(7)
        } else {
            None
        };
        let runtime = compiled.runtime_plan();
        let report = AwbcLowerer::new(
            &runtime.plan,
            &runtime.dialogue_content_catalog,
            "optional.arcw",
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
            panic!("one flow");
        };
        let mut native = Engine::for_flow(runtime.plan.clone(), &flow.id).unwrap();
        let native_calls = Arc::new(AtomicUsize::new(0));
        let mut native_backend =
            VmRuntimePureCallBackend::default().with_external_calls(OptionalApi {
                expected,
                calls: Arc::clone(&native_calls),
            });
        run_optional(
            &mut |input, options| {
                native.step_with_pure_backend(input, options, &mut native_backend)
            },
            expected,
        );
        assert_eq!(native_calls.load(Ordering::Relaxed), 1);
        let mut awbc =
            Some(ArcweftRuntimeExecutor::from_awbc_product(decoded, AwbcEntryId(0)).unwrap());
        let awbc_calls = Arc::new(AtomicUsize::new(0));
        let mut awbc_backend =
            VmRuntimePureCallBackend::default().with_external_calls(OptionalApi {
                expected,
                calls: Arc::clone(&awbc_calls),
            });
        run_optional(
            &mut |input, options| {
                let mut executor = awbc.take().unwrap();
                let result = executor.step_with_pure_backend(input, options, &mut awbc_backend);
                let ArcweftRuntimeExecutorSnapshot::AwbcProduct(saved) =
                    executor.snapshot().unwrap();
                let saved =
                    arcweft_core::awbc::product_step::AwbcProductExecutorSaveSnapshot::from_live(
                        &saved,
                        executor.admitted_product_awbc_program().unwrap(),
                    )
                    .unwrap();
                let saved = serde_json::from_slice(&serde_json::to_vec(&saved).unwrap()).unwrap();
                let snapshot = ArcweftRuntimeExecutorSnapshot::AwbcProduct(
                    executor.bind_product_save_snapshot(saved).unwrap(),
                );
                let expected = snapshot.clone();
                let restored = executor.restore_snapshot(snapshot).unwrap();
                assert_eq!(restored.snapshot().unwrap(), expected);
                awbc = Some(restored);
                result
            },
            expected,
        );
        assert_eq!(awbc_calls.load(Ordering::Relaxed), 1);
    }
}

struct OptionalApi {
    expected: Option<i64>,
    calls: Arc<AtomicUsize>,
}

impl RuntimeExternalCallBackend for OptionalApi {
    fn call_external(
        &mut self,
        context: &RuntimeExternalCallContext,
        callee: &RuntimeCallTarget,
        args: &[RuntimeValue],
    ) -> Option<Result<RuntimeValue, RuntimeEvalError>> {
        let RuntimeCallTarget::External(call) = callee else {
            return None;
        };
        assert_eq!(
            call.declaration().provider(),
            &arcweft_core::value::RuntimeExternalCallableProvider::Adapter("optional-api".into())
        );
        assert!(context.program_owner().is_some());
        match self.expected {
            Some(value) => {
                assert_eq!(args, [RuntimeValue::i64(value)]);
                assert_eq!(call.operands().len(), 1);
                assert_eq!(call.operands()[0].parameter.unwrap().parameter, 0);
            }
            None => {
                assert!(args.is_empty());
                assert!(call.operands().is_empty());
            }
        }
        self.calls.fetch_add(1, Ordering::Relaxed);
        Some(Ok(RuntimeValue::i64(self.expected.unwrap_or(41))))
    }
}

fn run_optional(
    step: &mut dyn FnMut(RuntimeStepInput, RuntimeStepOptions) -> RuntimeStepResult,
    expected: Option<i64>,
) {
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..RuntimeStepOptions::default()
    };
    let mut logs = Vec::new();
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
                logs.push(log.message);
            }
        }
        match result.fiber_status {
            FlowFiberStatus::Running => {}
            FlowFiberStatus::Done(FlowExit::Done) => {
                assert_eq!(logs, [expected.unwrap_or(41).to_string()]);
                return;
            }
            status => panic!("unexpected optional execution status: {status:?}"),
        }
    }
    panic!("optional execution exceeded its deterministic OneOp limit");
}
