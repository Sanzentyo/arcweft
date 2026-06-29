use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn cli_and_agent_repl_do_not_introduce_parallel_runtime_task_registry() {
    let root = repo_root();
    let checked_roots = [
        root.join("crates/arcweft-cli/src"),
        root.join("crates/arcweft-agent-repl/src"),
    ];
    let forbidden = [
        "RuntimeTaskRegistry",
        "struct CliTaskRegistry",
        "struct ReplTaskRegistry",
        "pending_task_events",
        "parallel scheduler registry",
    ];

    for source in checked_roots
        .iter()
        .flat_map(|root| rust_sources(root.as_path()))
    {
        let text = fs::read_to_string(&source)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()));
        for needle in forbidden {
            assert!(
                !text.contains(needle),
                "{} contains forbidden task-registry marker `{needle}`; :tasks/:cancel must use runtime-driver RuntimeTaskOwner",
                source.display()
            );
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives under crates/<name>")
        .to_path_buf()
}

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_rust_sources(root, &mut out);
    out
}

fn collect_rust_sources(path: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}
