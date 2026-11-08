use super::*;
use arcweft_view::ViewTextInputTextArgument;

#[test]
fn placeholder_and_label_round_trip_in_authored_order_without_becoming_retained_value() {
    for kind in ["TextField", "TextArea", "SecureField"] {
        for arguments in [
            "placeholder = match caption == \"first\" { true => \"first hint\", false => \"changed hint\" }, label = match caption == \"first\" { true => \"first name\", false => \"changed name\" }",
            "label = match caption == \"first\" { true => \"first name\", false => \"changed name\" }, placeholder = match caption == \"first\" { true => \"first hint\", false => \"changed hint\" }",
        ] {
            let source = SOURCE
                .replace("label = \"Feedback\"", arguments)
                .replace("TextField(caption", &format!("{kind}(caption"));
            let decoded = bundle(&source);
            let text = decoded
                .view_program
                .as_ref()
                .unwrap()
                .instructions
                .iter()
                .find_map(|instruction| {
                    if let ViewProgramInstruction::BindTextInput { text, .. } = instruction {
                        Some(text)
                    } else {
                        None
                    }
                })
                .unwrap();
            assert_eq!(text.len(), 2);
            assert_eq!(
                matches!(text[0], ViewTextInputTextArgument::Placeholder { .. }),
                arguments.starts_with("placeholder")
            );
            let (mut runtime, handle) = runtime(&source);
            let initial = frame(&mut runtime, &handle);
            let input = &initial.mounts[0].text_inputs[0];
            assert_eq!(input.value, "first");
            assert_eq!(input.label.as_deref(), Some("first name"));
            assert_eq!(input.placeholder.as_deref(), Some("first hint"));
            dispatch(&mut runtime, &write(input, "", 1, true)).unwrap();
            let empty = frame(&mut runtime, &handle);
            assert_eq!(empty.mounts[0].text_inputs[0].value, "");
            assert_eq!(
                empty.mounts[0].text_inputs[0].label.as_deref(),
                Some("changed name")
            );
            assert_eq!(
                empty.mounts[0].text_inputs[0].placeholder.as_deref(),
                Some("changed hint")
            );
            assert_eq!(labels(&empty), ["", ""]);
            let saved = runtime.snapshot().unwrap();
            runtime
                .restore(&saved, std::slice::from_ref(&handle))
                .unwrap();
            let restored = frame(&mut runtime, &handle);
            assert_eq!(
                restored.mounts[0].text_inputs[0].placeholder.as_deref(),
                Some("changed hint")
            );
            assert_eq!(restored.mounts[0].text_inputs[0].value, "");
            assert!(
                dispatch(
                    &mut runtime,
                    &write(&empty.mounts[0].text_inputs[0], "stale", 2, false)
                )
                .is_err()
            );
            dispatch(
                &mut runtime,
                &write(&restored.mounts[0].text_inputs[0], "京都", 1, true),
            )
            .unwrap();
            let changed = frame(&mut runtime, &handle);
            assert_eq!(labels(&changed), ["京都", "京都"]);
            assert_eq!(
                changed.mounts[0].text_inputs[0].placeholder.as_deref(),
                Some("changed hint")
            );
        }
    }
}

#[test]
fn text_argument_errors_observe_authored_evaluation_order() {
    let functions = r#"
fn fail_zero(value: String) -> String {
    let divisor = match value == "first" { true => 0i64, false => 1i64 }
    let quotient = 1i64 / divisor
    return value
}
fn fail_index(value: String) -> String {
    let index = match value == "first" { true => 2i32, false => 0i32 }
    let values = [0i32]
    let item = values[index]
    return value
}
"#;
    for (arguments, expected) in [
        (
            "placeholder = fail_zero(caption), label = fail_index(caption)",
            "division by zero",
        ),
        (
            "label = fail_index(caption), placeholder = fail_zero(caption)",
            "outside length",
        ),
    ] {
        let source = format!(
            "{functions}\n{}",
            SOURCE.replace("label = \"Feedback\"", arguments)
        );
        let (mut runtime, handle) = runtime(&source);
        let failed = runtime.evaluate(std::slice::from_ref(&handle), &[], false);
        assert_eq!(failed.diagnostics.len(), 1, "{failed:#?}");
        assert!(
            failed.diagnostics[0].message.contains(expected),
            "{failed:#?}"
        );
    }
}

#[test]
fn reordering_text_arguments_is_a_semantic_replacement() {
    use arcweft_runtime_driver::view_runtime::ViewProgramReplacementOutcome;
    let source = SOURCE.replace(
        "label = \"Feedback\"",
        "label = \"Feedback\", placeholder = \"Hint\"",
    );
    let decoded = bundle(&source);
    let (mut runtime, handle) = runtime(&source);
    let initial = frame(&mut runtime, &handle);
    let before = runtime.snapshot().unwrap();
    let mut program = decoded.view_program.clone().unwrap();
    for instruction in &mut program.instructions {
        if let ViewProgramInstruction::BindTextInput { text, .. } = instruction {
            text.reverse();
        }
    }
    let candidate = ValidatedViewProduct::try_new(
        Some(decoded.source_map.clone()),
        Some(program),
        decoded.view_style.clone(),
        ViewProductValidationLimits::default(),
    )
    .unwrap();
    let prepared = runtime.prepare_view_program_replacement(candidate).unwrap();
    assert_eq!(runtime.snapshot().unwrap(), before);
    assert!(matches!(
        runtime.commit_view_program_replacement(prepared).unwrap(),
        ViewProgramReplacementOutcome::Semantic { .. }
    ));
    let replaced = frame(&mut runtime, &handle);
    assert_eq!(
        replaced.mounts[0].text_inputs[0].label,
        initial.mounts[0].text_inputs[0].label
    );
    assert_eq!(
        replaced.mounts[0].text_inputs[0].placeholder,
        initial.mounts[0].text_inputs[0].placeholder
    );
    assert_eq!(replaced.mounts[0].text_inputs[0].value, "first");
}

#[test]
fn text_input_rejects_wrong_placeholder_types_duplicates_and_forged_text_roles() {
    for constructor in [
        "TextField(caption, placeholder = true)",
        "TextField(caption, placeholder = 42)",
        "TextField(caption, placeholder = \"a\", placeholder = \"b\")",
        "Button(\"Send\", placeholder = \"hint\")",
    ] {
        let source = SOURCE.replace("TextField(caption, label = \"Feedback\")", constructor);
        assert!(
            project_view_fixture_with_entry(&source, "arcweft-test://placeholder-rejection")
                .compile()
                .is_err(),
            "{constructor}"
        );
    }
    let decoded = bundle(SOURCE);
    let mut product = decoded.view_program.clone().unwrap();
    let text = product
        .instructions
        .iter_mut()
        .find_map(|instruction| {
            if let ViewProgramInstruction::BindTextInput { text, .. } = instruction {
                Some(text)
            } else {
                None
            }
        })
        .unwrap();
    *text = vec![text[0].clone(), text[0].clone()].into_boxed_slice();
    assert!(
        ValidatedViewProduct::try_new(
            Some(decoded.source_map.clone()),
            Some(product),
            decoded.view_style.clone(),
            ViewProductValidationLimits::default()
        )
        .is_err()
    );
}
