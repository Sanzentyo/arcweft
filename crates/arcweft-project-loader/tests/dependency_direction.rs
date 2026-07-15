use std::{collections::BTreeSet, process::Command};

use serde_json::Value;

#[test]
fn project_loader_has_required_direct_construction_dependencies() {
    let metadata = workspace_metadata();
    let packages = metadata["packages"].as_array().expect("packages array");
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .expect("resolved nodes array");

    let source = package_id(packages, "arcweft-source");
    let character = package_id(packages, "arcweft-character");
    let launch = package_id(packages, "arcweft-launch");
    let project = package_id(packages, "arcweft-project");
    let hir = package_id(packages, "arcweft-lang-hir");
    let sema = package_id(packages, "arcweft-lang-sema");
    let loader = package_id(packages, "arcweft-project-loader");
    let blake3 = package_id(packages, "blake3");

    let direct = |id: &str| normal_dependencies(nodes, id);
    assert!(direct(source).contains(blake3));
    assert!(direct(character).contains(source));
    assert!(direct(character).contains(blake3));
    assert!(direct(launch).contains(source));
    assert!(direct(project).contains(source));
    assert!(direct(loader).contains(source));
    assert!(direct(loader).contains(hir));
    assert!(direct(loader).contains(sema));

    assert!(!has_non_dev_path(nodes, sema, loader));
    assert!(!has_non_dev_path(nodes, hir, loader));
    assert!(has_non_dev_path(nodes, loader, sema));
}

#[test]
fn dependency_direction() {
    project_loader_has_required_direct_construction_dependencies();
}

#[test]
fn character_public_api_does_not_depend_on_sema() {
    let metadata = workspace_metadata();
    let packages = metadata["packages"].as_array().expect("packages array");
    let nodes = metadata["resolve"]["nodes"]
        .as_array()
        .expect("resolved nodes array");
    let character = package_id(packages, "arcweft-character");
    let sema = package_id(packages, "arcweft-lang-sema");

    assert!(!has_non_dev_path(nodes, character, sema));
}

fn workspace_metadata() -> Value {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("project-loader belongs to the workspace crates directory");
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1"])
        .current_dir(workspace_root)
        .output()
        .expect("cargo metadata starts");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("metadata is JSON")
}

fn package_id<'a>(packages: &'a [Value], name: &str) -> &'a str {
    let ids = packages
        .iter()
        .filter(|package| package["name"].as_str() == Some(name))
        .map(|package| package["id"].as_str().expect("package id"))
        .collect::<Vec<_>>();
    assert_eq!(
        ids.len(),
        1,
        "workspace package `{name}` must resolve to one exact package id"
    );
    ids[0]
}

fn normal_dependencies<'a>(nodes: &'a [Value], id: &str) -> BTreeSet<&'a str> {
    let node = nodes
        .iter()
        .find(|node| node["id"].as_str() == Some(id))
        .expect("resolved package node");
    node["deps"]
        .as_array()
        .expect("dependency array")
        .iter()
        .filter(|dependency| {
            dependency["dep_kinds"]
                .as_array()
                .expect("dependency kinds")
                .iter()
                .any(|kind| kind["kind"].is_null() || kind["kind"].as_str() == Some("build"))
        })
        .map(|dependency| dependency["pkg"].as_str().expect("dependency package id"))
        .collect()
}

fn has_non_dev_path(nodes: &[Value], start: &str, target: &str) -> bool {
    let mut pending = vec![start];
    let mut visited = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current) {
            continue;
        }
        for dependency in normal_dependencies(nodes, current) {
            if dependency == target {
                return true;
            }
            pending.push(dependency);
        }
    }
    false
}
