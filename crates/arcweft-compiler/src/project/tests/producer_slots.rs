//! Expanded Need arguments execute through the real native and Product owners.

use super::*;
use arcweft_adapter_context::manifest::{
    AdapterCallableGroupIndex, AdapterCallableName, AdapterCallableParameterIndex,
    AdapterEffectCapability, AdapterFunctionParam, AdapterFunctionSignature, AdapterHostCall,
    AdapterManifest, AdapterParameterGroup, AdapterParameterPassing, AdapterParameterPresence,
    AdapterTypeKind,
};
use arcweft_adapter_sema::registration::AdapterSemanticRegistration;
use arcweft_core::awbc::codec::AwbcDecodeBudget;
use arcweft_core::awbc::product_step::AwbcProductStepExecutor;
use arcweft_core::awbc::schema::{AwbcEntryId, AwbcProgram};
use arcweft_core::engine::{Engine, FlowExit, FlowFiberStatus};
use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions, RuntimeStepResult};
use arcweft_core::task::{
    HostTaskRequest, LogicalEpoch, TaskEvent, TaskEventKind, TaskPublicationCursor, TaskSequence,
    TaskSubmission,
};
use arcweft_core::value::{RuntimeInt, RuntimePayload, RuntimeValue};
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

fn compiled(arguments: &str) -> CompiledProject {
    let source = format!(
        "entry cli @entry.main {{ goto @flow.main }}\nextern capability fixture {{ fn pair(left: i64, right: i64) -> Need<i64> effects {{ control.suspend }} }}\nflow main() -> i64 {{ return await fixture.pair({arguments}) }}"
    );
    let (project, context) = removed_role_project(&source);
    let effect = AdapterEffectCapability::new("control.suspend");
    let parameters = ["left", "right"]
        .into_iter()
        .enumerate()
        .map(|(ordinal, name)| {
            AdapterFunctionParam::try_new(
                AdapterCallableParameterIndex::try_from_usize(ordinal).unwrap(),
                Some(AdapterCallableName::try_new(name).unwrap()),
                AdapterTypeKind::I64,
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
                parameters,
            )
            .unwrap(),
        ],
        AdapterTypeKind::Need {
            item: Box::new(AdapterTypeKind::I64),
        },
    )
    .unwrap();
    let manifest = AdapterManifest::new("fixture.expanded-need", "Expanded Need")
        .with_effect(effect.clone())
        .with_host_call(AdapterHostCall::with_signature(
            "fixture.pair",
            signature,
            [effect],
        ));
    let registration = AdapterSemanticRegistration::new(&manifest);
    let parts = registration.source_backed_facts(0).unwrap().into_parts();
    let mut documents = project
        .modules()
        .map(|module| Arc::clone(module.document()))
        .collect::<Vec<_>>();
    documents.push(parts.document);
    let facts = ProjectRegistrationFacts::try_new(
        context.facts().world().clone(),
        documents,
        parts.externals.into_vec(),
        Vec::new(),
        vec![parts.environment],
    )
    .unwrap();
    let context = ProjectCompilationContext::new(
        Arc::new(registration.declare_target(TypeCheckEnv::standard())),
        Arc::new(facts),
        Arc::clone(context.resource_types()),
        None,
        None,
    );
    let (mut compiler, parsed) = compilation_state(&project);
    compile_project(&mut compiler, &project, &parsed, &context)
        .expect("expanded physical Need arguments compile")
}

fn assert_request(task: &TaskSubmission) {
    let HostTaskRequest::Custom {
        args, named_args, ..
    } = &task.spec().request
    else {
        panic!("manifest request")
    };
    assert!(named_args.is_empty());
    assert_eq!(
        args.iter().map(RuntimePayload::value).collect::<Vec<_>>(),
        vec![
            &RuntimeValue::Int(RuntimeInt::i64(11)),
            &RuntimeValue::Int(RuntimeInt::i64(31))
        ]
    );
}

fn completed(task: &TaskSubmission) -> RuntimeStepInput {
    RuntimeStepInput {
        task_events: vec![TaskEvent {
            correlation: task.handle().correlation,
            cursor: TaskPublicationCursor {
                logical_epoch: LogicalEpoch(0),
                sequence: TaskSequence(1),
            },
            kind: TaskEventKind::Ready(RuntimePayload::new(RuntimeValue::Int(RuntimeInt::i64(42)))),
        }],
        ..RuntimeStepInput::default()
    }
}

fn assert_delivered(step: &RuntimeStepResult, task: &TaskSubmission) {
    assert_eq!(step.output.diagnostics.len(), 1);
    assert_eq!(
        step.output.diagnostics[0].message,
        format!("task {} sequence 1 delivered", task.task_id())
    );
}

fn assert_native_returns(mut native: Engine) {
    let options = RuntimeStepOptions::default();
    let mut task = None;
    for _ in 0..128 {
        let step = native.step(RuntimeStepInput::default(), options);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        if let Some(request) = step.output.requests.tasks.into_iter().next() {
            task = Some(request);
            break;
        }
    }
    let task = task.expect("Native emits the actual request");
    assert_request(&task);
    let step = native.step(completed(&task), options);
    assert_delivered(&step, &task);
    for _ in 0..128 {
        if matches!(native.fiber().frame.status, FlowFiberStatus::Done(_)) {
            break;
        }
        let step = native.step(RuntimeStepInput::default(), options);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
    }
    assert_eq!(
        native.fiber().frame.status,
        FlowFiberStatus::Done(FlowExit::Return("42".into()))
    );
}

fn assert_awbc_returns(program: &AwbcProgram) {
    let options = RuntimeStepOptions::default();
    let program = AwbcProgram::decode_canonical(
        &program.encode_canonical().unwrap(),
        AwbcDecodeBudget::default(),
    )
    .unwrap();
    let mut product = AwbcProductStepExecutor::for_entry(program, AwbcEntryId(0), 64).unwrap();
    let mut task = None;
    for _ in 0..128 {
        let step = product.step(RuntimeStepInput::default(), options);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        if let Some(request) = step.output.requests.tasks.into_iter().next() {
            task = Some(request);
            break;
        }
    }
    let task = task.expect("AWBC emits the actual request");
    assert_request(&task);
    let image = product.snapshot().unwrap();
    assert!(matches!(
        product.snapshot_for_save(),
        Err(arcweft_core::awbc::product_step::AwbcProductSaveError::NeedsQuiescence {
            needs
        }) if needs.len() == 1
    ));
    let candidate = product.snapshot().unwrap();
    let (mut product, error) = product.restore_snapshot_owned(candidate).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("invalid Need producer registry snapshot"),
        "{error}"
    );
    assert_eq!(product.snapshot().unwrap(), image);
    let step = product.step(completed(&task), options);
    assert_delivered(&step, &task);
    for _ in 0..128 {
        let image = product.snapshot().unwrap();
        if image.fiber.return_summary.is_some() {
            break;
        }
        let step = product.step(RuntimeStepInput::default(), options);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
    }
    assert_eq!(
        product.snapshot().unwrap().fiber.return_summary.as_deref(),
        Some("42")
    );
    let image = product.snapshot().unwrap();
    let candidate = product.snapshot().unwrap();
    product = product.restore_snapshot_owned(candidate).unwrap();
    assert_eq!(product.snapshot().unwrap(), image);
}

#[test]
fn compact_and_expression_need_spreads_emit_in_order_then_resume_native_and_awbc() {
    for arguments in [
        "[11i64, 31i64]...",
        "[11i64 + 0i64, 31i64 + 0i64]...",
        "[11i64]..., 31i64",
        "11i64, [31i64]...",
    ] {
        let compiled = compiled(arguments);
        let runtime = compiled.runtime_plan();
        let [flow] = runtime.plan.flows() else {
            panic!("one flow")
        };
        assert_native_returns(Engine::for_flow(runtime.plan.clone(), &flow.id).unwrap());
        let program = AwbcLowerer::new(
            &runtime.plan,
            &runtime.dialogue_content_catalog,
            "expanded-need.arcw",
        )
        .lower()
        .unwrap()
        .program;
        assert_awbc_returns(&program);
    }
}
