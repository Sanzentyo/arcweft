use super::*;

#[test]
fn captured_map_restores_its_original_callback_and_continues_remaining_rows() {
    assert_captured_map_snapshot(false);
}

#[test]
fn retained_map_prefix_restores_its_mapping_and_original_captured_callback() {
    assert_captured_map_snapshot(true);
}

fn assert_captured_map_snapshot(retained: bool) {
    let original = r#"
entry cli @entry.main { goto @flow.main }
fn score(base: i64, bonus: i64) -> i64 { base * (bonus + 2i64) }
fn mapped(values: Array<i64, 3>, bonus: i64) -> i64 {
    values.map(|item| score(item, bonus)).sum()
}
flow main() -> i64 { return mapped([1i64, 2i64, 3i64], 2i64) }
"#;
    let source = if retained {
        original.replace("values.map(|item| score(item, bonus)).sum()", "let mapper: Array<i64, 3> -> Array<i64, 3> effects {} = map(|item| score(item, bonus)); mapper(values).sum()")
    } else {
        original.to_owned()
    };
    let compiled = compile_source(&source).unwrap();
    assert_eq!(
        compiled
            .plan
            .function_sites()
            .iter()
            .filter(|site| site.capture_inputs().next().is_some())
            .count(),
        if retained { 2 } else { 1 }
    );
    let callbacks = compiled
        .plan
        .function_sites()
        .iter()
        .filter(|site| {
            site.capture_inputs().any(|input| {
                let local = compiled
                    .plan
                    .local_declarations()
                    .get(input.input_local())
                    .unwrap();
                compiled
                    .plan
                    .type_table()
                    .get(local.ty())
                    .unwrap()
                    .semantic_identity()
                    == arcweft_core::pattern::RuntimeCheckedType::Signed(
                        arcweft_core::value::RuntimeSignedIntWidth::I64,
                    )
                    .semantic_identity_digest()
            })
        })
        .map(|site| site.definition())
        .collect::<Vec<_>>();
    let [callback] = callbacks.as_slice() else {
        panic!("one captured Map callback")
    };
    let flow = compiled.plan.flows()[0].id.clone();
    let mut native = Engine::for_flow(compiled.plan, &flow).unwrap();
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::OneOp,
        budget: RuntimeStepBudget { max_ops: 1 },
        ..RuntimeStepOptions::default()
    };
    for _ in 0..1024 {
        let step = native.step(RuntimeStepInput::default(), options);
        assert!(step.stats.executed_ops <= 1);
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        if matches!(step.fiber_status, FlowFiberStatus::Done(_)) {
            break;
        }
        assert!(matches!(step.fiber_status, FlowFiberStatus::Running));
    }
    assert_eq!(
        native.fiber().frame.status,
        FlowFiberStatus::Done(FlowExit::Return("24".to_owned()))
    );

    let bytes = awfb_bytes_from_source(&source, [0x36; 32]);
    let mut session = session_from_bytes(&bytes);
    let RuntimeProgramOwner::Awbc(program) = session.program_owner() else {
        panic!("canonical AWBC owner")
    };
    let callback_function = program
        .functions
        .iter()
        .position(|function| function.definition == *callback)
        .map(|index| arcweft_core::awbc::schema::AwbcFunctionId(u32::try_from(index).unwrap()))
        .unwrap();
    let mut restored_callback = false;
    for _ in 0..4096 {
        let snapshot = session.snapshot_session().unwrap();
        if !restored_callback
            && snapshot
                .executor
                .state
                .fiber
                .frames
                .iter()
                .any(|frame| frame.function == callback_function)
        {
            let save = session.export_session_save_bytes().unwrap();
            let mut restored = session_from_bytes(&bytes);
            restored
                .import_session_save_bytes(&save, &Default::default())
                .unwrap();
            assert_eq!(restored.snapshot_session().unwrap(), snapshot);
            session = restored;
            restored_callback = true;
        }
        if session.is_finished() {
            break;
        }
        let step = session.step_with_clock(
            RuntimeClockStep::from_millis(1, 16).unwrap(),
            BundleStepInput::default(),
        );
        assert!(step.diagnostics.is_empty(), "{:?}", step.diagnostics);
    }
    assert!(
        restored_callback,
        "save/restore must occur inside the original captured callable"
    );
    assert!(session.is_finished());
    assert_eq!(
        session
            .snapshot_session()
            .unwrap()
            .executor
            .state
            .fiber
            .return_summary
            .as_deref(),
        Some("24")
    );
}
