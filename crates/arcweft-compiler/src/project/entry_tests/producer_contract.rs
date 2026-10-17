use super::{ProjectEntrySelectionKind, compile_attached_project, entry_project};
use arcweft_core::awbc::{
    codec::AwbcDecodeBudget,
    product_step::AwbcProductStepExecutor,
    schema::{AwbcEntryId, AwbcProgram},
};
use arcweft_core::engine::Engine;
use arcweft_core::plan::{FlowOp, RuntimeEntryTarget, RuntimeFunctionSiteBody};
use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions};
use arcweft_core::task::TaskSubmission;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

fn first_submission(call: &str, effect: &str) -> TaskSubmission {
    let source = format!(
        "fn controller() -> Result<Unit, AgentError> effects {{ {effect} }} {{ {call}\nreturn Ok(()) }}\nentry agent @entry.agent.controller {{ controller = controller }}"
    );
    let (project, context) = entry_project(
        &source,
        "entry.agent.controller",
        ProjectEntrySelectionKind::Agent,
    );
    let compiled =
        compile_attached_project(&project, &context).expect("selected agent controller compiles");
    let runtime = compiled.runtime_plan();
    let plan = &runtime.plan;
    let entry = plan.entries().first().expect("the selected Agent entry");
    let RuntimeEntryTarget::Controller(controller) = &entry.target else {
        panic!("the selected entry owns its real controller")
    };
    let targets = plan
        .function_sites()
        .iter()
        .flat_map(|function| match function.body() {
            RuntimeFunctionSiteBody::Executable(body) => body.ops(),
            RuntimeFunctionSiteBody::Expression(_) => &[],
        })
        .filter_map(|op| match op {
            FlowOp::HostCall { target, .. } => Some(target),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [target] = targets.as_slice() else {
        panic!("the checked source owns exactly one host call")
    };
    let expected_key = plan
        .task_plan(&target.producer.plan)
        .expect("the host producer belongs to the completed original program")
        .key();
    let expected_contract = target.producer.contract;
    let mut native =
        Engine::for_flow(plan.clone(), controller).expect("the selected controller starts");
    let mut native_task = None;
    for _ in 0..128 {
        let step = native.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        if !step.output.requests.host_calls.is_empty() {
            assert_eq!(step.output.requests.host_calls.len(), 1);
            native_task = step
                .output
                .requests
                .host_calls
                .into_iter()
                .next()
                .map(|request| request.into_submission());
            break;
        }
    }
    let native_task = native_task.expect("Native emits the real host request");
    assert_eq!(native_task.spec().producer.plan(), expected_key);
    assert_eq!(native_task.spec().producer.contract(), expected_contract);

    let lowered = AwbcLowerer::for_entry(
        plan,
        &runtime.dialogue_content_catalog,
        "agent_producer_contract.arcw",
        &entry.id,
    )
    .lower()
    .expect("the same source lowers to verified AWBC");
    let encoded = lowered.program.encode_canonical().expect("canonical AWBC");
    let decoded = AwbcProgram::decode_canonical(&encoded, AwbcDecodeBudget::default())
        .expect("canonical AWBC decodes");
    let mut product = AwbcProductStepExecutor::for_entry(decoded, AwbcEntryId(0), 64)
        .expect("the decoded product entry starts");
    let mut product_task = None;
    for _ in 0..128 {
        let step = product.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        if !step.output.requests.host_calls.is_empty() {
            assert_eq!(step.output.requests.host_calls.len(), 1);
            product_task = step
                .output
                .requests
                .host_calls
                .into_iter()
                .next()
                .map(|request| request.into_submission());
            break;
        }
    }
    let product_task = product_task.expect("decoded Product AWBC emits the real host request");
    assert_eq!(product_task, native_task);
    native_task
}

#[test]
fn agent_host_producer_contract_is_the_selected_schema_across_argument_changes() {
    let first = first_submission("checkpoint(\"first\")", "debug.record");
    let second = first_submission("checkpoint(\"second\")", "debug.record");
    let observation = first_submission("observe()", "agent.observe");

    assert_eq!(
        first.spec().producer.contract(),
        second.spec().producer.contract(),
        "changing an evaluated argument must not change the selected callable contract"
    );
    assert_ne!(
        first.spec().producer.contract(),
        observation.spec().producer.contract(),
        "distinct selected callable schemas retain distinct producer contracts"
    );
    assert_ne!(
        first.spec().producer.plan(),
        second.spec().producer.plan(),
        "the completed executable key still commits the changed argument expression"
    );
}
