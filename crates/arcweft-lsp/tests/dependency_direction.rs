use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    process::Command,
};

use serde::Deserialize;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    resolve: Resolve,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    source: Option<String>,
}

#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    id: String,
    deps: Vec<NodeDependency>,
}

#[derive(Deserialize)]
struct NodeDependency {
    pkg: String,
    dep_kinds: Vec<DependencyKind>,
}

#[derive(Deserialize)]
struct DependencyKind {
    kind: Option<String>,
}

fn normal_dependencies(package: &Package, metadata: &Metadata) -> BTreeSet<String> {
    let packages = metadata
        .packages
        .iter()
        .map(|candidate| (candidate.id.as_str(), candidate))
        .collect::<BTreeMap<_, _>>();
    metadata
        .resolve
        .nodes
        .iter()
        .find(|node| node.id == package.id)
        .expect("workspace package has one resolve node")
        .deps
        .iter()
        .filter(|dependency| dependency.dep_kinds.iter().any(|kind| kind.kind.is_none()))
        .map(|dependency| {
            packages
                .get(dependency.pkg.as_str())
                .expect("dependency package ID resolves")
                .name
                .clone()
        })
        .collect()
}

fn external_dependencies<'a>(
    dependencies: &'a BTreeSet<String>,
    workspace: &BTreeSet<&str>,
) -> BTreeSet<&'a str> {
    dependencies
        .iter()
        .filter(|dependency| !workspace.contains(dependency.as_str()))
        .map(String::as_str)
        .collect()
}

#[test]
fn associated_capacity_dependency_direction() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(workspace)
        .args(["metadata", "--format-version", "1", "--all-features"])
        .output()
        .expect("cargo metadata executes");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Metadata =
        serde_json::from_slice(&output.stdout).expect("typed cargo metadata JSON");
    let packages = metadata
        .packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let hir = packages["arcweft-lang-hir"];
    let sema = packages["arcweft-lang-sema"];
    let loader = packages["arcweft-project-loader"];
    let lsp = packages["arcweft-lsp"];
    for package in [hir, sema, loader, lsp] {
        assert!(
            package.source.is_none(),
            "{} is a workspace package",
            package.name
        );
    }

    let hir_dependencies = normal_dependencies(hir, &metadata);
    let sema_dependencies = normal_dependencies(sema, &metadata);
    let loader_dependencies = normal_dependencies(loader, &metadata);
    let lsp_dependencies = normal_dependencies(lsp, &metadata);

    assert!(!hir_dependencies.contains("arcweft-lang-sema"));
    assert!(!hir_dependencies.contains("arcweft-project-loader"));
    assert!(!hir_dependencies.contains("arcweft-lsp"));
    assert!(sema_dependencies.contains("arcweft-lang-hir"));
    assert!(!sema_dependencies.contains("arcweft-project-loader"));
    assert!(!sema_dependencies.contains("arcweft-lsp"));
    assert!(loader_dependencies.contains("arcweft-lang-hir"));
    assert!(loader_dependencies.contains("arcweft-lang-sema"));
    assert!(!loader_dependencies.contains("arcweft-lsp"));
    assert!(lsp_dependencies.contains("arcweft-lang-hir"));
    assert!(lsp_dependencies.contains("arcweft-lang-sema"));
    assert!(lsp_dependencies.contains("arcweft-project-loader"));

    let workspace_ids = metadata
        .packages
        .iter()
        .filter(|package| package.source.is_none())
        .map(|package| package.name.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        external_dependencies(&hir_dependencies, &workspace_ids),
        BTreeSet::from(["thiserror"])
    );
    assert_eq!(
        external_dependencies(&sema_dependencies, &workspace_ids),
        BTreeSet::from(["blake3", "thiserror"])
    );
    assert_eq!(
        external_dependencies(&loader_dependencies, &workspace_ids),
        BTreeSet::from(["blake3", "serde", "serde_json", "thiserror", "ureq",])
    );
    assert_eq!(
        external_dependencies(&lsp_dependencies, &workspace_ids),
        BTreeSet::from([
            "crossbeam-channel",
            "lsp-server",
            "lsp-types",
            "serde",
            "serde_json",
            "thiserror",
            "tracing",
            "tracing-subscriber",
        ])
    );
}
