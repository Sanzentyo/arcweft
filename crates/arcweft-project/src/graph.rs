use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// One typed import edge in the project module graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDependency {
    target: CanonicalModulePath,
}

/// One module and its resolved package-local imports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleNode {
    path: CanonicalModulePath,
    dependencies: Vec<ModuleDependency>,
}

/// Stable index into [`ModuleGraph::compile_units`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompileUnitId(usize);

/// Strongly connected body-compilation unit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileUnit {
    id: CompileUnitId,
    modules: Vec<CanonicalModulePath>,
    dependencies: Vec<CompileUnitId>,
}

/// Deterministic module graph and compile-unit schedule.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleGraph {
    nodes: BTreeMap<CanonicalModulePath, ModuleNode>,
    units: Vec<CompileUnit>,
    compile_order: Vec<CompileUnitId>,
    module_units: BTreeMap<CanonicalModulePath, CompileUnitId>,
}

/// Invalid project module graph.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ModuleGraphError {
    #[error("module `{module}` is declared more than once")]
    DuplicateModule { module: CanonicalModulePath },
    #[error("module `{module}` imports missing module `{dependency}`")]
    MissingDependency {
        module: CanonicalModulePath,
        dependency: CanonicalModulePath,
    },
}

impl ModuleDependency {
    pub const fn new(target: CanonicalModulePath) -> Self {
        Self { target }
    }

    pub const fn target(&self) -> &CanonicalModulePath {
        &self.target
    }

    /// Sorts dependencies and removes repeated targets.
    pub fn normalize(dependencies: impl IntoIterator<Item = Self>) -> Vec<Self> {
        dependencies
            .into_iter()
            .map(|dependency| dependency.target)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(Self::new)
            .collect()
    }
}

impl ModuleNode {
    pub fn new(
        path: CanonicalModulePath,
        dependencies: impl IntoIterator<Item = ModuleDependency>,
    ) -> Self {
        Self {
            path,
            dependencies: ModuleDependency::normalize(dependencies),
        }
    }

    pub const fn path(&self) -> &CanonicalModulePath {
        &self.path
    }

    pub fn dependencies(&self) -> &[ModuleDependency] {
        &self.dependencies
    }
}

impl CompileUnitId {
    pub const fn index(self) -> usize {
        self.0
    }
}

impl CompileUnit {
    pub const fn id(&self) -> CompileUnitId {
        self.id
    }

    pub fn modules(&self) -> &[CanonicalModulePath] {
        &self.modules
    }

    /// Compile units whose bodies must be available first.
    pub fn dependencies(&self) -> &[CompileUnitId] {
        &self.dependencies
    }
}

impl ModuleGraph {
    pub fn new(nodes: impl IntoIterator<Item = ModuleNode>) -> Result<Self, ModuleGraphError> {
        let mut node_map = BTreeMap::new();
        for node in nodes {
            let path = node.path.clone();
            if node_map.insert(path.clone(), node).is_some() {
                return Err(ModuleGraphError::DuplicateModule { module: path });
            }
        }
        for node in node_map.values() {
            for dependency in &node.dependencies {
                if !node_map.contains_key(&dependency.target) {
                    return Err(ModuleGraphError::MissingDependency {
                        module: node.path.clone(),
                        dependency: dependency.target.clone(),
                    });
                }
            }
        }

        let paths = node_map.keys().cloned().collect::<Vec<_>>();
        let path_indices = paths
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, path)| (path, index))
            .collect::<BTreeMap<_, _>>();
        let body_adjacency = paths
            .iter()
            .map(|path| {
                node_map[path]
                    .dependencies
                    .iter()
                    .map(|dependency| path_indices[&dependency.target])
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let components = strongly_connected_components(&body_adjacency);

        let mut unit_modules = components
            .into_iter()
            .map(|component| {
                let mut modules = component
                    .into_iter()
                    .map(|index| paths[index].clone())
                    .collect::<Vec<_>>();
                modules.sort();
                modules
            })
            .collect::<Vec<_>>();
        unit_modules.sort_by(|left, right| left[0].cmp(&right[0]));

        let module_units = unit_modules
            .iter()
            .enumerate()
            .flat_map(|(unit, modules)| {
                modules
                    .iter()
                    .cloned()
                    .map(move |module| (module, CompileUnitId(unit)))
            })
            .collect::<BTreeMap<_, _>>();

        let units = unit_modules
            .into_iter()
            .enumerate()
            .map(|(index, modules)| {
                let id = CompileUnitId(index);
                let mut dependencies = Vec::new();
                for dependency in modules
                    .iter()
                    .flat_map(|module| node_map[module].dependencies.iter())
                {
                    let dependency_unit = module_units[&dependency.target];
                    if dependency_unit == id {
                        continue;
                    }
                    dependencies.push(dependency_unit);
                }
                dependencies.sort();
                dependencies.dedup();
                CompileUnit {
                    id,
                    modules,
                    dependencies,
                }
            })
            .collect::<Vec<_>>();
        let compile_order = dependency_order(&units);

        Ok(Self {
            nodes: node_map,
            units,
            compile_order,
            module_units,
        })
    }

    pub fn nodes(&self) -> impl Iterator<Item = &ModuleNode> {
        self.nodes.values()
    }

    pub fn node(&self, path: &CanonicalModulePath) -> Option<&ModuleNode> {
        self.nodes.get(path)
    }

    pub fn compile_units(&self) -> &[CompileUnit] {
        &self.units
    }

    pub fn compile_order(&self) -> &[CompileUnitId] {
        &self.compile_order
    }

    pub fn compile_unit(&self, id: CompileUnitId) -> &CompileUnit {
        &self.units[id.index()]
    }

    pub fn unit_for_module(&self, module: &CanonicalModulePath) -> Option<CompileUnitId> {
        self.module_units.get(module).copied()
    }
}

fn strongly_connected_components(adjacency: &[Vec<usize>]) -> Vec<Vec<usize>> {
    fn visit(node: usize, adjacency: &[Vec<usize>], seen: &mut [bool], order: &mut Vec<usize>) {
        if core::mem::replace(&mut seen[node], true) {
            return;
        }
        for &next in &adjacency[node] {
            visit(next, adjacency, seen, order);
        }
        order.push(node);
    }

    fn collect(node: usize, reverse: &[Vec<usize>], seen: &mut [bool], component: &mut Vec<usize>) {
        if core::mem::replace(&mut seen[node], true) {
            return;
        }
        component.push(node);
        for &next in &reverse[node] {
            collect(next, reverse, seen, component);
        }
    }

    let mut order = Vec::with_capacity(adjacency.len());
    let mut seen = vec![false; adjacency.len()];
    for node in 0..adjacency.len() {
        visit(node, adjacency, &mut seen, &mut order);
    }
    let mut reverse = vec![Vec::new(); adjacency.len()];
    for (source, targets) in adjacency.iter().enumerate() {
        for &target in targets {
            reverse[target].push(source);
        }
    }
    let mut seen = vec![false; adjacency.len()];
    let mut components = Vec::new();
    for node in order.into_iter().rev() {
        if seen[node] {
            continue;
        }
        let mut component = Vec::new();
        collect(node, &reverse, &mut seen, &mut component);
        components.push(component);
    }
    components
}

fn dependency_order(units: &[CompileUnit]) -> Vec<CompileUnitId> {
    let mut remaining = units
        .iter()
        .map(|unit| (unit.id, unit.dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<CompileUnitId, BTreeSet<CompileUnitId>>::new();
    for unit in units {
        for dependency in &unit.dependencies {
            dependents.entry(*dependency).or_default().insert(unit.id);
        }
    }
    let mut ready = remaining
        .iter()
        .filter_map(|(unit, count)| (*count == 0).then_some(*unit))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(units.len());
    while let Some(unit) = ready.pop_first() {
        order.push(unit);
        if let Some(items) = dependents.get(&unit) {
            for dependent in items {
                let count = remaining
                    .get_mut(dependent)
                    .expect("compile unit dependency is registered");
                *count -= 1;
                if *count == 0 {
                    ready.insert(*dependent);
                }
            }
        }
    }
    debug_assert_eq!(order.len(), units.len());
    order
}

#[cfg(test)]
mod tests {
    use super::{ModuleDependency, ModuleGraph, ModuleNode};
    use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModulePath, ModuleSegment};

    fn canonical(path: &str) -> CanonicalModulePath {
        path.parse::<ModulePath>()
            .unwrap()
            .resolve_from(&CanonicalModulePath::crate_root())
            .unwrap()
    }

    #[test]
    fn groups_body_cycles_into_one_compile_unit() {
        let graph = ModuleGraph::new([
            ModuleNode::new(canonical("a"), [ModuleDependency::new(canonical("b"))]),
            ModuleNode::new(canonical("b"), [ModuleDependency::new(canonical("a"))]),
            ModuleNode::new(canonical("c"), []),
        ])
        .unwrap();
        assert_eq!(graph.compile_units().len(), 2);
        assert!(
            graph
                .compile_units()
                .iter()
                .any(|unit| unit.modules().len() == 2)
        );
    }

    #[test]
    fn duplicate_imports_are_deduplicated_by_target() {
        let target = canonical("shared");
        let node = ModuleNode::new(
            canonical("app"),
            [
                ModuleDependency::new(target.clone()),
                ModuleDependency::new(target.clone()),
                ModuleDependency::new(target),
            ],
        );
        assert_eq!(node.dependencies().len(), 1);
        assert_eq!(node.dependencies()[0].target(), &canonical("shared"));
    }

    #[test]
    fn ordinary_cycles_merge_body_compile_units() {
        let graph = ModuleGraph::new([
            ModuleNode::new(canonical("a"), [ModuleDependency::new(canonical("b"))]),
            ModuleNode::new(canonical("b"), [ModuleDependency::new(canonical("a"))]),
        ])
        .unwrap();
        assert_eq!(graph.compile_units().len(), 1);
    }

    #[test]
    fn dependency_order_places_body_dependencies_first() {
        let graph = ModuleGraph::new([
            ModuleNode::new(
                canonical("app"),
                [ModuleDependency::new(canonical("shared"))],
            ),
            ModuleNode::new(canonical("shared"), []),
        ])
        .unwrap();
        let order = graph.compile_order();
        let shared = graph.unit_for_module(&canonical("shared")).unwrap();
        let app = graph.unit_for_module(&canonical("app")).unwrap();
        assert_eq!(order, &[shared, app]);
    }

    #[test]
    fn segment_type_stays_constructible_for_graph_clients() {
        assert_eq!(
            ModuleSegment::new("valid_name").unwrap().as_str(),
            "valid_name"
        );
    }
}
