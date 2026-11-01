use super::*;
use arcweft_core::{
    awbc::{codec::AwbcDecodeBudget, product_step::AwbcProductStepExecutor, schema::AwbcProgram},
    engine::{Engine, FlowFiberStatus},
    runtime_id::RuntimePresentationHandleToken,
    step::{RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions},
    value::RuntimeCallTarget,
};
use arcweft_interaction_model::presentation::{PresentationArgumentRole, PresentationResourceKind};
use arcweft_lang_sema::final_analysis::{CheckedExecutionBodyOwner, CheckedExecutionSource};

fn program() -> arcweft_compiler::lower::CompiledDeterministicProgram {
    let compiled = compile_source(
        r#"
pub fn root() -> (Handle<ImageSurface>, i32) {
    let mut order = 0i32;
    let handle = image(
        param.channel = scope { order = order * 10i32 + 1i32; 7i32 },
        opacity = scope { order = order * 10i32 + 2i32; 0.5 },
        proxy.param.user.role = "button"
    );
    (handle, order)
}
flow main() -> i64 { return 0i64 }
"#,
    )
    .expect("Source image factory compiles");
    let declaration = compiled
        .analysis
        .final_analysis()
        .hir_topology()
        .modules()
        .iter()
        .flat_map(|module| module.entries())
        .filter_map(|entry| entry.body())
        .find(|body| body.declaration().name() == "root")
        .unwrap()
        .declaration()
        .clone();
    compiled
        .analysis
        .compile_deterministic_program(
            CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                declaration,
                role: arcweft_lang_hir::project::HirDeclarationBodyRootRole::FunctionBody,
            }),
            None,
            &Default::default(),
        )
        .expect("owned factory result retains its checked program root")
}

fn decoded(selected: &arcweft_compiler::lower::CompiledDeterministicProgram) -> AwbcProgram {
    let lowered = AwbcLowerer::new(
        selected.plan(),
        &selected.lowering_report().dialogue_content_catalog,
        "factory.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let bytes = lowered.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded, lowered);
    let calls = decoded
        .intrinsics
        .iter()
        .filter_map(|row| match &row.identity {
            RuntimeCallTarget::PresentationFactory(call) => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let call = calls[0];
    assert_eq!(call.resource(), PresentationResourceKind::Image);
    assert_eq!(
        call.inputs()
            .iter()
            .map(|key| (key.role(), key.parameter()))
            .collect::<Vec<_>>(),
        vec![
            (PresentationArgumentRole::Opacity, None),
            (PresentationArgumentRole::Parameter, Some("channel")),
            (PresentationArgumentRole::ProxyParameter, Some("user.role")),
        ]
    );
    decoded
}

fn options() -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..Default::default()
    }
}

fn assert_result(value: RuntimeValue) {
    let RuntimeValue::Tuple(mut fields) = value else {
        panic!("owned result tuple")
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields.pop(), Some(RuntimeValue::i32(12)));
    let RuntimeValue::Opaque(handle) = fields.pop().unwrap() else {
        panic!("typed handle")
    };
    let token = RuntimePresentationHandleToken::try_decode_payload(handle.payload()).unwrap();
    assert_eq!(token.resource(), PresentationResourceKind::Image);
    assert_eq!(
        token.bank(),
        arcweft_core::runtime_id::RuntimePresentationBankId::Root
    );
}

#[test]
fn source_image_factory_executes_owned_captures_in_native() {
    let selected = program();
    let bytes = decoded(&selected).encode_canonical().unwrap();
    let mut plan = selected.plan().as_ref().clone();
    plan.bind_artifact(
        arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
            *blake3::hash(&bytes).as_bytes(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut native =
        Engine::for_program_invocation(Arc::new(plan), selected.program(), vec![]).unwrap();
    let mut done = false;
    for _ in 0..256 {
        let step = native.step(Default::default(), options());
        assert!(step.stats.executed_ops <= 1);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        assert!(step.output.requests.tasks.is_empty());
        assert!(!matches!(step.fiber_status, FlowFiberStatus::Failed(_)));
        if matches!(step.fiber_status, FlowFiberStatus::Done(_)) {
            done = true;
            break;
        }
    }
    assert!(done);
    assert_result(native.take_program_result().unwrap().unwrap().1);
}

#[test]
fn source_image_factory_executes_owned_captures_in_decoded_awbc() {
    let selected = program();
    let code = Arc::new(decoded(&selected));
    let mut awbc = AwbcProductStepExecutor::for_program_invocation(
        code,
        selected.program(),
        vec![],
        arcweft_core::task::GenerationId::new(0),
        1,
    )
    .unwrap();
    let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
    let mut done = false;
    for _ in 0..256 {
        let step = awbc.step_with_pure_backend(Default::default(), options(), &mut backend);
        assert!(step.stats.executed_ops <= 1);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        assert!(step.output.requests.tasks.is_empty());
        assert!(!matches!(step.fiber_status, FlowFiberStatus::Failed(_)));
        if matches!(step.fiber_status, FlowFiberStatus::Done(_)) {
            done = true;
            break;
        }
    }
    assert!(done);
    assert_result(awbc.take_program_result().unwrap().unwrap().1);
}
