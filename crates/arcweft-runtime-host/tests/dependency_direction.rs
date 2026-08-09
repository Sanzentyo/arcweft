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
    all_edges: BTreeMap<String, Vec<String>>,
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
        let mut edges = BTreeMap::new();
        let mut all_edges = BTreeMap::new();
        for node in metadata.resolve.nodes {
            let normal = node
                .deps
                .iter()
                .filter(|dependency| dependency.dep_kinds.iter().any(|kind| kind.kind.is_none()))
                .map(|dependency| dependency.pkg.clone())
                .collect();
            let all = node
                .deps
                .into_iter()
                .map(|dependency| dependency.pkg)
                .collect();
            edges.insert(node.id.clone(), normal);
            all_edges.insert(node.id, all);
        }
        Self {
            names,
            package_ids,
            edges,
            all_edges,
        }
    }

    fn path(&self, from: &str, to: &str) -> Option<Vec<&str>> {
        self.path_in(&self.edges, from, to)
    }

    fn all_path(&self, from: &str, to: &str) -> Option<Vec<&str>> {
        self.path_in(&self.all_edges, from, to)
    }

    fn path_in<'a>(
        &'a self,
        edges: &'a BTreeMap<String, Vec<String>>,
        from: &str,
        to: &str,
    ) -> Option<Vec<&'a str>> {
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
            for dependency in edges.get(current).into_iter().flatten() {
                if visited.insert(dependency) {
                    predecessor.insert(dependency, current);
                    frontier.push_back(dependency);
                }
            }
        }
        None
    }
}

fn workspace_dependency_graph() -> NormalDependencyGraph {
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
    NormalDependencyGraph::from_metadata(
        serde_json::from_slice(&output.stdout).expect("typed cargo metadata JSON"),
    )
}

#[test]
fn runtime_host_normal_graph_excludes_hir_and_runtime_plan() {
    let graph = workspace_dependency_graph();
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
}

#[test]
fn adapter_context_normal_graph_excludes_compiler_layers() {
    let graph = workspace_dependency_graph();
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

#[test]
fn core_dependency_graph_excludes_compiler_layers() {
    let graph = workspace_dependency_graph();
    for forbidden in [
        "arcweft-lang-syntax",
        "arcweft-lang-hir",
        "arcweft-lang-sema",
        "arcweft-runtime-plan",
        "arcweft-compiler",
        "arcweft-cli",
        "arcweft-lsp",
    ] {
        assert!(
            graph.all_path("arcweft-core", forbidden).is_none(),
            "core has a forbidden compiler-layer dependency path across normal/dev/target edges: {}",
            graph
                .all_path("arcweft-core", forbidden)
                .unwrap()
                .join(" -> ")
        );
    }
}
