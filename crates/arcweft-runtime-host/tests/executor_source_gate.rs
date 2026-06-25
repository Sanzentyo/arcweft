use std::fs;
use std::path::{Path, PathBuf};

const TARGET_CRATES: &[&str] = &[
    "arcweft-agent-runner",
    "arcweft-cli",
    "arcweft-runtime-driver",
    "arcweft-runtime-host",
];

#[test]
fn application_runtime_crates_do_not_construct_low_level_executors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("runtime-host crate is under workspace crates directory");
    let mut offenders = Vec::new();
    for crate_name in TARGET_CRATES {
        collect_executor_mentions(
            &root.join("crates").join(crate_name).join("src"),
            &mut offenders,
        );
    }

    assert!(
        offenders.is_empty(),
        "low-level executor mentions must stay inside the core facade:\n{}",
        offenders.join("\n")
    );
}

fn collect_executor_mentions(path: &Path, offenders: &mut Vec<String>) {
    let entries = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    for entry in entries {
        let entry = entry.expect("source directory entry is readable");
        let path = entry.path();
        if path.is_dir() {
            collect_executor_mentions(&path, offenders);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        if source.contains("BytecodeVmExecutor") || source.contains("AotExecutor") {
            offenders.push(format_relative(&path));
        }
    }
}

fn format_relative(path: &Path) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("runtime-host crate is under workspace crates directory")
        .to_path_buf();
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
