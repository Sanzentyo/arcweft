use arcweft_compiler::source::compile_source;
use arcweft_core::plan::FlowOp;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[test]
fn compact_choice_goto_preserves_checked_ids_and_flow_target_through_awbc() {
    let compiled = compile_source(
        r#"
entry cli @entry.main { goto @flow.main }

flow main {
    choice @.first {
        @.next "Next" -> @flow.done
    }
}

flow done() -> String {
    return "done"
}
"#,
    )
    .expect("static Choice goto compiles");

    let (id, options) = compiled
        .plan
        .flows()
        .iter()
        .flat_map(|flow| flow.ops.iter())
        .find_map(|op| match op {
            FlowOp::Scope(ops) => ops.iter().find_map(|op| match op {
                FlowOp::Choice { id, options } => Some((id, options)),
                _ => None,
            }),
            FlowOp::Choice { id, options } => Some((id, options)),
            _ => None,
        })
        .expect("one lowered Choice operation");
    assert_eq!(id.as_deref(), Some("choice.main.first"));
    let [option] = options.as_slice() else {
        panic!("one lowered Choice option")
    };
    assert_eq!(option.id.as_deref(), Some("choice.main.first.next"));
    assert_eq!(option.label, "Next");
    let target = option.target.as_ref().expect("checked Flow target");
    assert_eq!(target.public_label().as_str(), "flow.done");

    AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "choice_static.arcw",
    )
    .lower()
    .expect("static Choice goto lowers to verified AWBC");
}
