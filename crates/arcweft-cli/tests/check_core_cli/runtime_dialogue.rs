use super::{CommandOutput, temp_arcw};
use std::{ffi::OsStr, fs, path::Path};

const SOURCE: &str = r"
entry cli @entry.run { goto @flow.run }
pub character alice {}
fn configured() -> CharacterDialogue { @<character.alice>() }
flow run {
    let speaker = configured()
    speaker()[Ready[p]]
}
";

fn assert_source_dialogue_report(output: &CommandOutput) {
    output.assert_success();
    let report: serde_json::Value = serde_json::from_str(&output.stdout()).expect("runtime report");
    assert_eq!(report["final_status"], "dialogue", "{report}");
    let steps = report["steps"].as_array().expect("runtime steps");
    assert!(
        steps.iter().any(|step| step["flow_events"]
            .as_array()
            .expect("flow events")
            .iter()
            .any(|event| event
                .as_str()
                .is_some_and(|label| label.starts_with("dialogue ")))),
        "the accepted producer must publish the actual dialogue activation: {report}"
    );
    for step in steps {
        assert_eq!(step["diagnostics"], serde_json::json!([]), "{report}");
    }
}

fn build_bundle(source: &Path, bundle: &Path) {
    let output = CommandOutput::run([
        OsStr::new("bundle"),
        source.as_os_str(),
        OsStr::new("--output"),
        bundle.as_os_str(),
        OsStr::new("--json"),
    ])
    .expect("build the real source-owned AWFB");
    output.assert_success();
}

#[test]
fn headless_dialogue_binds_original_generation_for_entry_and_flow_in_every_tier() {
    let source = temp_arcw("dialogue-generation", SOURCE);
    for executor in ["bytecode-vm", "aot", "awbc-product"] {
        for explicit_flow in [false, true] {
            let mut args = vec![
                OsStr::new("run"),
                source.as_os_str(),
                OsStr::new("--executor"),
                OsStr::new(executor),
                OsStr::new("--mode"),
                OsStr::new("drain"),
                OsStr::new("--steps"),
                OsStr::new("2"),
                OsStr::new("--max-ops"),
                OsStr::new("64"),
                OsStr::new("--json"),
            ];
            if explicit_flow {
                args.extend([OsStr::new("--flow"), OsStr::new("run")]);
            } else {
                args.extend([OsStr::new("--entry"), OsStr::new("entry.run")]);
            }
            assert_source_dialogue_report(
                &CommandOutput::run(args).expect("run real headless CLI"),
            );
        }
    }
    fs::remove_file(source).expect("remove source fixture");
}

#[test]
fn bundled_dialogue_binds_original_generation_for_report_and_resumable_hosts() {
    let source = temp_arcw("bundled-dialogue-generation", SOURCE);
    let bundle_path = source.with_extension("awfb");
    build_bundle(&source, &bundle_path);
    let bytes = fs::read(&bundle_path).expect("read source-owned AWFB");
    let bundle = arcweft_bundle::ArcweftBundle::from_awfb_slice_with_resource_types(
        &bytes,
        &arcweft_resource_model::registry::ResourceTypeRegistry::empty(),
    )
    .expect("admitted AWFB");
    let options = arcweft_runtime_host::BundleRunnerOptions {
        entry: Some(
            arcweft_core::plan::EntryRuntimeId::from_source_entity_body("entry.run").unwrap(),
        ),
        steps: 2,
        max_ops: 64,
        ..arcweft_runtime_host::BundleRunnerOptions::default()
    };
    let report = arcweft_runtime_host::run_bundle_with_native_adapters(&bundle, &options, &[])
        .expect("bundle report host binds its producer");
    assert_eq!(report.final_status, "dialogue");
    assert!(report.steps.iter().all(|step| step.diagnostics.is_empty()));
    assert!(report.steps.iter().any(|step| {
        step.flow_events
            .iter()
            .any(|event| matches!(event, arcweft_core::plan::FlowEvent::DialogueLine { .. }))
    }));
    let mut session = arcweft_runtime_host::BundleRunnerSession::with_adapter_installer(
        &bundle,
        &options,
        |_, builder| Ok(builder),
    )
    .expect("resumable host binds its producer");
    while session.step().expect("step admitted bundle").is_some() {}
    assert_eq!(session.final_status(), "dialogue");
    assert!(
        session
            .steps()
            .iter()
            .all(|step| step.diagnostics.is_empty())
    );
    assert!(session.steps().iter().any(|step| {
        step.flow_events
            .iter()
            .any(|event| matches!(event, arcweft_core::plan::FlowEvent::DialogueLine { .. }))
    }));

    let owner = arcweft_core::task::RuntimeProgramOwner::Awbc(
        bundle.admit_runtime_program().unwrap().program_arc(),
    );
    let mut missing_declaration = bundle.clone();
    missing_declaration.character_dialogue_generation = None;
    assert!(matches!(
        missing_declaration.admit_character_dialogue_runtime(owner),
        Err(arcweft_bundle::BundleCodecError::InvalidCharacterDialogueGeneration { .. })
    ));
    fs::remove_file(bundle_path).expect("remove AWFB fixture");
    fs::remove_file(source).expect("remove source fixture");
}
