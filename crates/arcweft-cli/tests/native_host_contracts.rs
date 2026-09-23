use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct NativeHostFixture {
    root: PathBuf,
}

impl NativeHostFixture {
    fn new(source: &str, adapter: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "arcweft-native-host-contract-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join(".arcweft/save")).unwrap();
        fs::write(root.join("src/main.arcw"), source).unwrap();
        fs::write(
            root.join("arcw.toml"),
            format!(
                r#"schema = 1
[package]
id = "org.arcweft.test.native-host-contract"
version = "0.1.0"
[profiles.fixture]
kind = "cli"
source = "src/main.arcw"
entry = "@entry.main"
adapter = "{adapter}"
"#,
            ),
        )
        .unwrap();
        Self { root }
    }

    fn command(&self, verb: &str) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
        command
            .arg(verb)
            .arg("--manifest-path")
            .arg(self.root.join("arcw.toml"))
            .arg("--profile")
            .arg("fixture");
        command
    }
}

impl Drop for NativeHostFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("remove disposable host fixture");
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

const FILE_SOURCE: &str = r#"
extern capability fs {
    type FsError
    fn read_text(path: VirtualPath) -> Need<Result<String, FsError>> effects { fs.read }
    fn write_text(path: VirtualPath, body: String) -> Need<Result<Unit, FsError>> effects { fs.write }
}
extern capability path { fn save(path: String) -> VirtualPath }
entry cli @entry.main { goto @flow.main }
flow main() -> String effects { fs.read, fs.write } {
    let text = match (await fs.read_text(path.save("input.txt"))) {
        .Ok(text) => text
        .Err(_) => "read-failed"
    }
    let result = match (await fs.write_text(path.save("output.txt"), text)) {
        .Ok(_) => text
        .Err(_) => "write-failed"
    }
    return result
}
"#;

#[test]
fn typed_virtual_paths_read_and_write_through_native_and_awbc() {
    for executor in ["bytecode-vm", "awbc-product"] {
        let fixture = NativeHostFixture::new(FILE_SOURCE, "native-file");
        fs::write(
            fixture.root.join(".arcweft/save/input.txt"),
            "typed-native-path",
        )
        .unwrap();
        assert_success(&fixture.command("check").output().unwrap());
        let output = fixture
            .command("run")
            .args([
                "--executor",
                executor,
                "--mode",
                "drain",
                "--steps",
                "24",
                "--max-ops",
                "64",
                "--json",
            ])
            .output()
            .unwrap();
        assert_success(&output);
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(
            report["final_status"]
                .as_str()
                .is_some_and(|status| status.contains("typed-native-path")),
            "execution did not return the read text: {report}"
        );
        assert_eq!(
            fs::read_to_string(fixture.root.join(".arcweft/save/output.txt")).unwrap(),
            "typed-native-path"
        );
    }
}

#[test]
fn host_calls_require_the_selected_target_effects() {
    let fixture = NativeHostFixture::new(FILE_SOURCE, "sans-io");
    let output = fixture.command("check").output().unwrap();
    assert!(
        !output.status.success(),
        "Sans I/O must reject file effects"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("AWF-EFX-007"),
        "wrong target diagnostic: {stderr}"
    );
}

#[test]
fn pure_host_calls_require_membership_in_the_selected_manifest() {
    let source = r#"
extern capability path { fn save(path: String) -> VirtualPath }
entry cli @entry.main { goto @flow.main }
flow main() -> VirtualPath { return path.save("unused.txt") }
"#;
    for adapter in ["sans-io", "native-cli"] {
        let fixture = NativeHostFixture::new(source, adapter);
        let output = fixture.command("check").output().unwrap();
        assert!(
            !output.status.success(),
            "{adapter} must not provide path.save implicitly"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("AWF-EFX-007") && stderr.contains("does not provide this host call"),
            "wrong host membership diagnostic: {stderr}"
        );
    }
}

#[test]
fn selected_native_cli_preserves_cli_arguments() {
    let source = r"
extern capability cli { fn args() -> Vec<String> }
entry cli @entry.main { goto @flow.main }
flow main() -> Vec<String> { return cli.args() }
";
    let fixture = NativeHostFixture::new(source, "native-cli");
    let output = fixture
        .command("cli")
        .args(["--steps", "4", "--json", "--", "alice"])
        .output()
        .unwrap();
    assert_success(&output);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        report["final_status"]
            .as_str()
            .is_some_and(|status| status.contains("seq/values/1")),
        "selected CLI adapter lost the supplied argument: {report}"
    );
    let direct = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("cli")
        .arg(fixture.root.join("src/main.arcw"))
        .args([
            "--entry",
            "entry.main",
            "--steps",
            "4",
            "--json",
            "--",
            "alice",
        ])
        .output()
        .unwrap();
    assert_success(&direct);
}

#[test]
fn authored_externs_cannot_change_manifest_types_or_omit_manifest_effects() {
    for source in [
        FILE_SOURCE.replace(
            "fn save(path: String) -> VirtualPath",
            "fn save(path: String) -> String",
        ),
        FILE_SOURCE.replace(" effects { fs.read }", ""),
    ] {
        let fixture = NativeHostFixture::new(&source, "native-file");
        let output = fixture.command("check").output().unwrap();
        assert!(
            !output.status.success(),
            "mismatched host contract was admitted"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("aw.callable.catalog.registration"),
            "wrong contract diagnostic: {stderr}"
        );
    }
}

const CLI_EXIT_SOURCE: &str = r#"
extern capability cli {
    fn stdout(text: String) effects { stdio.write }
    fn stderr(text: String) effects { stdio.write }
    fn exit(code: i32) -> Never effects { process.exit }
}
entry cli @entry.main { goto @flow.main }
flow main() -> Never effects { stdio.write, process.exit } {
    cli.stdout("hello")
    cli.stderr(text = "problem")
    return cli.exit(code = EXIT_CODE)
}
"#;

#[test]
fn selected_native_cli_writes_streams_and_exits_without_a_never_payload() {
    for executor in ["bytecode-vm", "awbc-product"] {
        for code in [0, 7] {
            let source = CLI_EXIT_SOURCE.replace("EXIT_CODE", &format!("{code}i32"));
            let fixture = NativeHostFixture::new(&source, "native-cli");
            assert_success(&fixture.command("check").output().unwrap());
            let output = fixture
                .command("run")
                .args([
                    "--executor",
                    executor,
                    "--mode",
                    "drain",
                    "--steps",
                    "8",
                    "--max-ops",
                    "64",
                ])
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(code),
                "{executor}: stdout={:?}, stderr={:?}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, b"hello", "{executor}");
            assert_eq!(output.stderr, b"problem", "{executor}");
        }
    }
}

#[test]
fn implicit_native_policy_cannot_exit_the_embedding_process() {
    let source = r"
extern capability cli { fn exit(code: i32) -> Never effects { process.exit } }
entry cli @entry.main { goto @flow.main }
flow main() -> Never effects { process.exit } { return cli.exit(7i32) }
";
    let fixture = NativeHostFixture::new(source, "native-cli");
    for executor in ["bytecode-vm", "awbc-product"] {
        let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("run")
            .arg(fixture.root.join("src/main.arcw"))
            .args([
                "--executor",
                executor,
                "--entry",
                "entry.main",
                "--mode",
                "drain",
                "--steps",
                "8",
                "--json",
            ])
            .output()
            .unwrap();
        assert_ne!(
            output.status.code(),
            Some(7),
            "unselected adapter exited {executor}"
        );
        let diagnostic = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            diagnostic.contains("host call is not admitted by the active adapter manifests"),
            "wrong unselected host diagnostic for {executor}: {diagnostic}"
        );
    }
}

#[test]
fn cli_process_calls_require_matching_arguments_and_selected_manifest() {
    for source in [
        CLI_EXIT_SOURCE.replace("EXIT_CODE", "\"seven\""),
        CLI_EXIT_SOURCE
            .replace("EXIT_CODE", "7i32")
            .replace("cli.stdout(\"hello\")", "cli.stdout(7i32)"),
    ] {
        let fixture = NativeHostFixture::new(&source, "native-cli");
        let output = fixture.command("check").output().unwrap();
        assert!(
            !output.status.success(),
            "invalid CLI argument was accepted"
        );
    }
    let source = CLI_EXIT_SOURCE.replace("EXIT_CODE", "7i32");
    for adapter in ["sans-io", "native-file"] {
        let fixture = NativeHostFixture::new(&source, adapter);
        let output = fixture.command("check").output().unwrap();
        assert!(
            !output.status.success(),
            "{adapter} admitted CLI process calls"
        );
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(diagnostic.contains("AWF-EFX-007"), "{diagnostic}");
    }
}
