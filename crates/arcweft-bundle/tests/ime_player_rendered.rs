use arcweft_bundle::resource_codec::view::ViewInputKind;
use arcweft_bundle::{ArcweftBundle, BundleFormat};
use arcweft_core::awbc::{
    fiber::{AwbcFiberRoot, FiberState},
    schema::AwbcEntryId,
    vm::{self, VmExit, VmStepOptions},
};
use arcweft_core::value::RuntimeValue;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct FixtureOutput(PathBuf);
impl Drop for FixtureOutput {
    fn drop(&mut self) {
        for name in ["first.awfb", "second.awfb"] {
            let _ = fs::remove_file(self.0.join(name));
        }
        let _ = fs::remove_dir(&self.0);
    }
}
fn generate(root: &Path, output: &Path) {
    let result = Command::new("cargo")
        .args(["+nightly", "-Zscript"])
        .arg(root.join("tools/build-web-ime-player-rendered-fixture.rs"))
        .arg("--out")
        .arg(output)
        .current_dir(root)
        .output()
        .expect("fixture generator launches");
    assert!(
        result.status.success(),
        "fixture generator failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
#[test]
fn ime_generator_publishes_deterministic_controls_and_executable_callbacks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let output = FixtureOutput(std::env::temp_dir().join(format!(
        "arcweft-ime-fixture-{}-{unique}",
        std::process::id()
    )));
    fs::create_dir(&output.0).unwrap();
    generate(root, &output.0.join("first.awfb"));
    generate(root, &output.0.join("second.awfb"));
    let bytes = fs::read(output.0.join("first.awfb")).unwrap();
    assert_eq!(bytes, fs::read(output.0.join("second.awfb")).unwrap());
    let bundle = ArcweftBundle::from_format_slice(BundleFormat::Awfb, &bytes)
        .expect("complete product decodes and verifies");
    assert_eq!(bundle.to_format_bytes(BundleFormat::Awfb).unwrap(), bytes);
    let controls = bundle
        .view_input
        .as_ref()
        .unwrap()
        .runtime_text_controls(bundle.view_text.as_ref(), bundle.view_program.as_ref());
    assert_eq!(controls.len(), 3);
    for (id, kind, y, height, change, submit) in [
        (
            "input.jp_text_field",
            ViewInputKind::TextField,
            48_000,
            48_000,
            true,
            true,
        ),
        (
            "input.long_latin_area",
            ViewInputKind::TextArea,
            112_000,
            136_000,
            true,
            false,
        ),
        (
            "input.secret_secure_field",
            ViewInputKind::SecureField,
            264_000,
            48_000,
            true,
            true,
        ),
    ] {
        let control = controls
            .iter()
            .find(|control| control.public_id == id)
            .expect("required input");
        assert_eq!(control.kind, kind);
        assert_eq!(
            (
                control.bounds.x_milli,
                control.bounds.y_milli,
                control.bounds.width_milli,
                control.bounds.height_milli
            ),
            (48_000, y, 420_000, height)
        );
        assert_eq!(control.handlers.change.is_some(), change);
        assert_eq!(control.handlers.submit.is_some(), submit);
        assert!(control.label.is_some());
    }
    let secure = controls
        .iter()
        .find(|control| control.kind == ViewInputKind::SecureField)
        .unwrap();
    assert!(secure.is_secure());
    assert_eq!(secure.value, "arcweft-secret-1234");
    assert!(secure.redacted_for_observation().value.is_empty());
    assert!(!format!("{secure:?}").contains(&secure.value));
    let programs = controls
        .iter()
        .flat_map(|control| {
            control
                .handlers
                .change
                .iter()
                .chain(&control.handlers.submit)
        })
        .map(|handler| handler.program)
        .collect::<BTreeSet<_>>();
    assert_eq!(programs.len(), 5);
    let awbc = bundle.product_awbc_program();
    for program in &programs {
        let binding = awbc
            .pure_program_binding(*program)
            .expect("callback has an executable body");
        assert!(binding.input_types.is_empty());
        let mut fiber = FiberState::for_function(
            awbc,
            AwbcFiberRoot::Program(*program),
            binding.function,
            0,
            256,
        )
        .unwrap();
        assert_eq!(
            vm::step(
                awbc,
                &mut fiber,
                VmStepOptions {
                    max_instructions: 256
                }
            )
            .unwrap()
            .exit,
            VmExit::Returned(Some(RuntimeValue::Tuple(vec![RuntimeValue::Unit])))
        );
    }
    let mut entry = FiberState::for_entry(awbc, AwbcEntryId(0), 0, 256).unwrap();
    assert_eq!(
        vm::step(
            awbc,
            &mut entry,
            VmStepOptions {
                max_instructions: 256
            }
        )
        .unwrap()
        .exit,
        VmExit::Returned(Some(RuntimeValue::Unit))
    );
    let mut missing_owner = bundle.clone();
    missing_owner.view_program = None;
    missing_owner.view_style = None;
    missing_owner.view_text = None;
    assert!(
        matches!(
            missing_owner.to_format_bytes(BundleFormat::Json),
            Err(arcweft_bundle::BundleCodecError::InvalidViewHandlerRuntime { .. })
        ),
        "text input cannot outlive its complete program owner"
    );
    let mut missing = bundle.clone();
    let removed = *programs.first().unwrap();
    missing
        .product_awbc
        .program
        .pure_programs
        .retain(|binding| binding.program != removed);
    assert!(
        missing.to_format_bytes(BundleFormat::Awfb).is_err(),
        "callback reference cannot lose its executable owner"
    );
}
