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
    program_from_source(
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
}

fn program_from_source(source: &str) -> arcweft_compiler::lower::CompiledDeterministicProgram {
    let compiled = compile_source(source).expect("Source image factory compiles");
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

fn assert_result(value: &RuntimeValue) {
    let RuntimeValue::Tuple(fields) = value else {
        panic!("owned result tuple")
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[1], RuntimeValue::i32(12));
    let RuntimeValue::Opaque(handle) = &fields[0] else {
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
    assert_result(native.program_result().unwrap().1);
    assert!(matches!(native.take_program_result(), Err(arcweft_core::value::ownership::RuntimeDetachedValueError::PresentationHandleCustodyRequired { .. })));
    assert_result(native.program_result().unwrap().1);
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
        let snapshot = awbc
            .snapshot()
            .expect("whole executor retains the resource and its owning bank");
        awbc = awbc
            .restore_snapshot_owned(snapshot)
            .unwrap_or_else(|(_, error)| panic!("owned restore: {error}"));
        if matches!(step.fiber_status, FlowFiberStatus::Done(_)) {
            done = true;
            break;
        }
    }
    assert!(done);
    assert_result(awbc.program_result().unwrap().1);
    assert!(matches!(awbc.take_program_result(), Err(arcweft_core::value::ownership::RuntimeDetachedValueError::PresentationHandleCustodyRequired { .. })));
    assert_result(awbc.program_result().unwrap().1);
}

#[derive(Clone, Copy, Debug)]
enum Product {
    Tuple,
    Call,
    NestedTuple,
    NestedCall,
}

fn sibling_factories(product: Product) -> arcweft_compiler::lower::CompiledDeterministicProgram {
    let expression = match product {
        Product::Tuple => {
            "(image(param.channel = scope { order = order * 10i32 + 1i32; 1i32 }), image(param.channel = scope { order = order * 10i32 + 2i32; 2i32 }))"
        }
        Product::Call => {
            "pair(image(param.channel = scope { order = order * 10i32 + 1i32; 1i32 }), image(param.channel = scope { order = order * 10i32 + 2i32; 2i32 }))"
        }
        Product::NestedTuple => {
            "(make(scope { order = order * 10i32 + 1i32; 1i32 }), make(scope { order = order * 10i32 + 2i32; 2i32 }))"
        }
        Product::NestedCall => {
            "pair(make(scope { order = order * 10i32 + 1i32; 1i32 }), make(scope { order = order * 10i32 + 2i32; 2i32 }))"
        }
    };
    program_from_source(&format!(
        r#"
pub fn make(channel: i32) -> Handle<ImageSurface> {{ image(param.channel = channel) }}
pub fn pair(first: Handle<ImageSurface>, second: Handle<ImageSurface>) -> (Handle<ImageSurface>, Handle<ImageSurface>) {{ (first, second) }}
pub fn root() -> (Handle<ImageSurface>, Handle<ImageSurface>, i32) {{
    let mut order = 0i32;
    let (first, second) = {expression};
    (first, second, order)
}}
flow main() -> i64 {{ return 0i64 }}
"#
    ))
}

fn sibling_code(selected: &arcweft_compiler::lower::CompiledDeterministicProgram) -> AwbcProgram {
    let lowered = AwbcLowerer::new(
        selected.plan(),
        &selected.lowering_report().dialogue_content_catalog,
        "sibling-factories.arcw",
    )
    .lower()
    .unwrap()
    .program;
    let bytes = lowered.encode_canonical().unwrap();
    let decoded = AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default()).unwrap();
    assert_eq!(decoded, lowered);
    decoded
}

fn assert_siblings(value: &RuntimeValue) {
    let RuntimeValue::Tuple(fields) = value else {
        panic!("owned result tuple")
    };
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[2], RuntimeValue::i32(12));
    let tokens = fields[..2]
        .iter()
        .map(|value| {
            let RuntimeValue::Opaque(handle) = value else {
                panic!("typed image handle")
            };
            let token =
                RuntimePresentationHandleToken::try_decode_payload(handle.payload()).unwrap();
            assert_eq!(token.resource(), PresentationResourceKind::Image);
            assert_eq!(
                token.bank(),
                arcweft_core::runtime_id::RuntimePresentationBankId::Root
            );
            token
        })
        .collect::<Vec<_>>();
    assert_ne!(tokens[0], tokens[1]);
}

fn run_siblings(product: Product, native: bool) {
    run_selected(sibling_factories(product), native, assert_siblings);
}

fn run_selected(
    selected: arcweft_compiler::lower::CompiledDeterministicProgram,
    native: bool,
    expected: fn(&RuntimeValue),
) {
    let code = sibling_code(&selected);
    if native {
        let bytes = code.encode_canonical().unwrap();
        let mut plan = selected.plan().as_ref().clone();
        plan.bind_artifact(
            arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes(
                *blake3::hash(&bytes).as_bytes(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut engine =
            Engine::for_program_invocation(Arc::new(plan), selected.program(), vec![]).unwrap();
        let mut done = false;
        for _ in 0..256 {
            let step = engine.step(Default::default(), options());
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
        expected(engine.program_result().unwrap().1);
    } else {
        let mut engine = AwbcProductStepExecutor::for_program_invocation(
            Arc::new(code),
            selected.program(),
            vec![],
            arcweft_core::task::GenerationId::new(0),
            1,
        )
        .unwrap();
        let mut backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
        let mut done = false;
        for _ in 0..256 {
            let step = engine.step_with_pure_backend(Default::default(), options(), &mut backend);
            assert!(step.stats.executed_ops <= 1);
            assert!(
                step.output.diagnostics.is_empty(),
                "{:?}",
                step.output.diagnostics
            );
            assert!(step.output.requests.tasks.is_empty());
            assert!(!matches!(step.fiber_status, FlowFiberStatus::Failed(_)));
            let snapshot = engine
                .snapshot()
                .expect("whole sibling resource owner saves");
            engine = engine
                .restore_snapshot_owned(snapshot)
                .unwrap_or_else(|(_, error)| panic!("owned restore: {error}"));
            if matches!(step.fiber_status, FlowFiberStatus::Done(_)) {
                done = true;
                break;
            }
        }
        assert!(done);
        expected(engine.program_result().unwrap().1);
    }
}

#[test]
fn source_sibling_image_factories_keep_tuple_temporaries_in_native() {
    run_siblings(Product::Tuple, true);
}

#[test]
fn source_sibling_image_factories_keep_tuple_temporaries_in_decoded_awbc() {
    run_siblings(Product::Tuple, false);
}

#[test]
fn source_sibling_image_factories_keep_call_temporaries_in_native() {
    run_siblings(Product::Call, true);
}

#[test]
fn source_sibling_image_factories_keep_call_temporaries_in_decoded_awbc() {
    run_siblings(Product::Call, false);
}

#[test]
fn source_sibling_image_factories_keep_nested_tuple_temporaries_in_native() {
    run_siblings(Product::NestedTuple, true);
}

#[test]
fn source_sibling_image_factories_keep_nested_tuple_temporaries_in_decoded_awbc() {
    run_siblings(Product::NestedTuple, false);
}

#[test]
fn source_sibling_image_factories_keep_nested_call_temporaries_in_native() {
    run_siblings(Product::NestedCall, true);
}

#[test]
fn source_sibling_image_factories_keep_nested_call_temporaries_in_decoded_awbc() {
    run_siblings(Product::NestedCall, false);
}

fn single_wrapper() -> arcweft_compiler::lower::CompiledDeterministicProgram {
    program_from_source(
        r#"
pub fn make(channel: i32) -> Handle<ImageSurface> { image(param.channel = channel) }
pub fn root() -> (Handle<ImageSurface>, i32) {
    let handle = make(7i32);
    (handle, 12i32)
}
flow main() -> i64 { return 0i64 }
"#,
    )
}

#[test]
fn source_image_factory_inside_sync_function_in_native() {
    run_selected(single_wrapper(), true, assert_result);
}

#[test]
fn source_image_factory_inside_sync_function_in_decoded_awbc() {
    run_selected(single_wrapper(), false, assert_result);
}
