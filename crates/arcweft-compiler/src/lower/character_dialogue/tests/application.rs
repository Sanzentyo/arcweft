use arcweft_core::{
    awbc::schema::{AwbcInstruction, AwbcTerminator},
    effect::RuntimeEffectExpr,
    plan::FlowOp,
    value::{RuntimeExprKind, RuntimeValue},
};
use arcweft_interaction_model::dialogue::CharacterDialogueOperation;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[test]
fn reference_application_retains_its_once_evaluated_target_and_empty_factory() {
    let compiled = crate::source::compile_source(
        r#"
pub character alice { display = "Alice" }
flow main() -> Unit { alice[Hello] }
entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("reference dialogue application projects its target");
    let [flow] = compiled.plan.flows() else {
        panic!("one main Flow")
    };
    let [
        FlowOp::Let { pattern, expr },
        FlowOp::Dialogue { target, .. },
    ] = flow.body().ops()
    else {
        panic!("source reference is materialized before the dialogue: {flow:?}")
    };
    assert!(matches!(expr.kind(), RuntimeExprKind::EntityRef(_)));
    let RuntimeExprKind::CharacterDialogue {
        operation,
        target: factory_target,
        fields,
    } = target.kind()
    else {
        panic!("reference applications use the typed factory")
    };
    assert_eq!(*operation, CharacterDialogueOperation::Factory);
    assert!(fields.is_empty());
    assert!(matches!(factory_target.kind(), RuntimeExprKind::Local(_)));
    assert_eq!(pattern.ty(), factory_target.ty());
    assert_ne!(factory_target.ty(), target.ty());
}

#[test]
fn configured_application_keeps_the_whole_factory_value_without_rebuilding_it() {
    let compiled = crate::source::compile_source(
        r#"
pub character alice { display = "Alice" }
flow main() -> Unit { alice(source_locale = "ja-JP")[Hello] }
entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("configured target projects as the whole checked value");
    let [flow] = compiled.plan.flows() else {
        panic!("one main Flow")
    };
    let [FlowOp::Let { expr, .. }, FlowOp::Dialogue { target, .. }] = flow.body().ops() else {
        panic!("configured target is materialized once: {flow:?}")
    };
    let mut configured = expr;
    while let RuntimeExprKind::Let { body, .. } = configured.kind() {
        configured = body;
    }
    let RuntimeExprKind::CharacterDialogue { fields, .. } = configured.kind() else {
        panic!("the complete authored factory is evaluated")
    };
    assert_eq!(fields.len(), 1);
    assert!(matches!(target.kind(), RuntimeExprKind::Local(_)));
    assert_eq!(expr.ty(), target.ty());
}

#[test]
fn awbc_application_produces_target_before_evaluating_content_values() {
    let compiled = crate::source::compile_source(
        r#"
pub character alice { display = "Alice" }
flow main() -> Unit { alice[Hello #["content"]] }
entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("dialogue content with an evaluated string slot");
    let report = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "dialogue-target",
    )
    .lower()
    .expect("the target register is admitted by the AWBC verifier");
    let (target, values) = report
        .program
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            AwbcTerminator::Dialogue { target, values, .. } => Some((*target, values)),
            _ => None,
        })
        .expect("one dialogue terminator");
    let [content] = values.as_slice() else {
        panic!("one content value register")
    };
    let target_position = report
        .program
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction,
            AwbcInstruction::CharacterDialogue { destination, .. } if *destination == target)
        })
        .expect("target has a producer instruction");
    let content_position = report
        .program
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction,
            AwbcInstruction::LoadConst { dst, .. } if *dst == content.value)
        })
        .expect("content string is evaluated into its slot register");
    assert!(target_position < content_position);
}

#[test]
fn application_target_runs_flow_effects_before_the_line() {
    let compiled = crate::source::compile_source(
        r#"
pub character alice { display = "Alice" }
flow main() -> Unit { ({ log.info("target"); alice() })[Hello] }
entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("dialogue target is lowered through Flow continuations");
    let [flow] = compiled.plan.flows() else {
        panic!("one main Flow")
    };
    let dialogue_position = flow
        .body()
        .ops()
        .iter()
        .position(|op| matches!(op, FlowOp::Dialogue { .. }))
        .expect("the target continuation reaches the dialogue");
    let messages = flow.body().ops()[..dialogue_position]
        .iter()
        .filter_map(|op| match op {
            FlowOp::EvaluatedEffect(RuntimeEffectExpr::Log { message, .. }) => Some(message),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [message] = messages.as_slice() else {
        panic!("the target's logging effect executes once before the dialogue")
    };
    assert!(
        matches!(message.kind(), RuntimeExprKind::Value(RuntimeValue::String(value)) if value == "target")
    );
}

#[test]
fn configured_application_target_is_owned_by_each_closed_function_instance() {
    use arcweft_core::plan::RuntimeFunctionSiteBody;

    let compiled = crate::source::compile_source(
        r#"
pub character alice { display = "Alice" }
fn speak<T>(value: T) { alice(source_locale = "ja-JP")[#[value]]; }
flow main() -> Unit { speak(1i64); speak("content"); }
entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("each generic instance owns its target and content types");
    let targets = compiled
        .plan
        .function_sites()
        .iter()
        .filter_map(|site| match site.body() {
            RuntimeFunctionSiteBody::Executable(body) => Some(body.ops()),
            RuntimeFunctionSiteBody::Expression(_) => None,
        })
        .flatten()
        .filter_map(|operation| match operation {
            FlowOp::Dialogue { target, .. } => Some(target),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(targets.len(), 2);
    assert!(
        targets
            .iter()
            .all(|target| matches!(target.kind(), RuntimeExprKind::Local(_)))
    );
    let report = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "instance-targets",
    )
    .lower()
    .expect("target locals are admitted by their owning AWBC frames");
    let [first, second] = report.program.content_units.as_slice() else {
        panic!("one content unit for each closed application key")
    };
    assert_eq!(first.public_id, second.public_id);
    assert_ne!(first.template, second.template);
}

#[test]
fn unit_function_return_evaluates_its_body_before_the_declared_unit_return() {
    let compiled = crate::source::compile_source(
        r#"
fn finish() { return { log.info("returned"); () }; }
flow main() -> Unit { finish(); }
entry cli @entry.main { goto @flow.main }
"#,
    )
    .expect("Unit return operands retain their evaluated effects");
    let report = AwbcLowerer::new(&compiled.plan, &compiled.dialogue_content, "unit-return")
        .lower()
        .expect("ordinary Unit results retain their declared AWBC ABI");
    let blocks = report
        .program
        .blocks
        .iter()
        .filter(|block| {
            let start = usize::try_from(block.instructions.start).unwrap();
            let end = usize::try_from(block.instructions.checked_end().unwrap()).unwrap();
            report.program.instructions[start..end]
                .iter()
                .any(|instruction| matches!(instruction, AwbcInstruction::EmitEffect { .. }))
        })
        .collect::<Vec<_>>();
    let [block] = blocks.as_slice() else {
        panic!("the return operand's logging effect is retained once")
    };
    assert!(matches!(
        block.terminator,
        AwbcTerminator::Return { value: Some(_) }
    ));
    let function = &report.program.functions[block.owner.index()];
    let result = report.program.signatures[function.signature.index()]
        .result
        .expect("ordinary Unit functions have a result");
    assert!(matches!(
        report.program.runtime_types[result.index()].shape(),
        arcweft_core::awbc::schema::AwbcRuntimeTypeShape::Unit
    ));
}
