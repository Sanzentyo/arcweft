use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
    process::Command,
};

use serde::Deserialize;

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    resolve: Resolve,
    workspace_members: Vec<String>,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
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

struct NormalDependencyGraph {
    names: BTreeMap<String, String>,
    package_ids: BTreeMap<String, String>,
    edges: BTreeMap<String, Vec<String>>,
}

impl NormalDependencyGraph {
    fn from_metadata(metadata: Metadata) -> Self {
        let workspace_members = metadata
            .workspace_members
            .into_iter()
            .collect::<BTreeSet<_>>();
        let names = metadata
            .packages
            .iter()
            .map(|package| (package.id.clone(), package.name.clone()))
            .collect();
        let package_ids = metadata
            .packages
            .iter()
            .filter(|package| workspace_members.contains(&package.id))
            .map(|package| (package.name.clone(), package.id.clone()))
            .collect();
        let edges = metadata
            .resolve
            .nodes
            .into_iter()
            .map(|node| {
                let dependencies = node
                    .deps
                    .into_iter()
                    .filter(|dependency| {
                        dependency.dep_kinds.iter().any(|kind| kind.kind.is_none())
                    })
                    .map(|dependency| dependency.pkg)
                    .collect();
                (node.id, dependencies)
            })
            .collect();
        Self {
            names,
            package_ids,
            edges,
        }
    }

    fn path(&self, from: &str, to: &str) -> Option<Vec<&str>> {
        let start = self
            .package_ids
            .get(from)
            .unwrap_or_else(|| panic!("workspace package `{from}` exists"));
        let target = self
            .package_ids
            .get(to)
            .unwrap_or_else(|| panic!("workspace package `{to}` exists"));
        let mut frontier = VecDeque::from([start.as_str()]);
        let mut visited = BTreeSet::from([start.as_str()]);
        let mut predecessor = BTreeMap::<&str, &str>::new();

        while let Some(current) = frontier.pop_front() {
            if current == target {
                let mut path = vec![current];
                while let Some(previous) = predecessor.get(path.last().copied().unwrap()) {
                    path.push(previous);
                }
                path.reverse();
                return Some(path.into_iter().map(|id| self.names[id].as_str()).collect());
            }
            for dependency in self.edges.get(current).into_iter().flatten() {
                if visited.insert(dependency) {
                    predecessor.insert(dependency, current);
                    frontier.push_back(dependency);
                }
            }
        }
        None
    }
}

#[test]
fn runtime_host_and_adapter_context_keep_language_pipeline_dependencies_out() {
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
    let graph = NormalDependencyGraph::from_metadata(
        serde_json::from_slice(&output.stdout).expect("typed cargo metadata JSON"),
    );

    for forbidden in [
        "arcweft-lang-syntax",
        "arcweft-lang-hir",
        "arcweft-runtime-plan",
        "arcweft-compiler",
    ] {
        assert!(
            graph.path("arcweft-runtime-host", forbidden).is_none(),
            "runtime host has a forbidden normal dependency path: {}",
            graph
                .path("arcweft-runtime-host", forbidden)
                .unwrap()
                .join(" -> ")
        );
    }
    for forbidden in [
        "arcweft-lang-syntax",
        "arcweft-lang-hir",
        "arcweft-lang-sema",
    ] {
        assert!(
            graph.path("arcweft-adapter-context", forbidden).is_none(),
            "adapter context has a forbidden normal dependency path: {}",
            graph
                .path("arcweft-adapter-context", forbidden)
                .unwrap()
                .join(" -> ")
        );
    }
}
