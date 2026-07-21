//! Deterministic classification of unresolved imports.

use std::collections::BTreeMap;

use arcweft_lang_syntax::ast::{
    common::{UseItem, UseTreeKind},
    module_path::CanonicalModulePath,
    symbol_path::SymbolPath,
};

use crate::project::HirProject;

use super::{
    ImportResolutionError, LinkedProjectSymbolPath, ProjectSymbolLinkError, ProjectSymbolTable,
    sort_spans, source_span,
};

pub(super) fn classify_unresolved_imports(
    project: &HirProject,
    table: &ProjectSymbolTable,
    imports: &[(CanonicalModulePath, &UseItem)],
    unresolved: &[usize],
    work: &mut u64,
) -> Result<Vec<ProjectSymbolLinkError>, Box<ProjectSymbolLinkError>> {
    if unresolved.is_empty() {
        return Ok(Vec::new());
    }

    let edges = unresolved_import_edges(table, imports, unresolved);
    let units = u64::try_from(unresolved.len())
        .unwrap_or(u64::MAX)
        .saturating_add(
            edges
                .iter()
                .map(|targets| u64::try_from(targets.len()).unwrap_or(u64::MAX))
                .fold(0_u64, u64::saturating_add),
        );
    let first_source = unresolved.first().map(|index| {
        let (module, import) = &imports[*index];
        source_span(project, module, *import.range())
    });
    ProjectSymbolTable::charge(work, units, first_source).map_err(Box::new)?;

    let cyclic_components = cyclic_components(&edges);
    Ok(unresolved
        .iter()
        .enumerate()
        .map(|(local, &import_index)| {
            unresolved_import_error(
                project,
                imports,
                unresolved,
                local,
                import_index,
                cyclic_components[local].as_deref(),
            )
        })
        .collect())
}

fn unresolved_import_edges(
    table: &ProjectSymbolTable,
    imports: &[(CanonicalModulePath, &UseItem)],
    unresolved: &[usize],
) -> Vec<Vec<usize>> {
    let mut exact_producers = BTreeMap::<(CanonicalModulePath, String), Vec<usize>>::new();
    let mut glob_producers = BTreeMap::<CanonicalModulePath, Vec<usize>>::new();
    for (local, &import_index) in unresolved.iter().enumerate() {
        let (module, import) = &imports[import_index];
        match import_produced_names(import) {
            Some(names) => {
                for name in names {
                    exact_producers
                        .entry((module.clone(), name))
                        .or_default()
                        .push(local);
                }
            }
            None => glob_producers
                .entry(module.clone())
                .or_default()
                .push(local),
        }
    }

    let mut edges = vec![Vec::new(); unresolved.len()];
    for (local, &import_index) in unresolved.iter().enumerate() {
        let (module, import) = &imports[import_index];
        for (target_module, name) in import_requirements(table, module, import) {
            if let Some(name) = name {
                if let Some(producers) = exact_producers.get(&(target_module.clone(), name)) {
                    edges[local].extend(producers.iter().copied());
                }
            } else {
                for ((module, _), producers) in exact_producers
                    .range((target_module.clone(), String::new())..)
                    .take_while(|((module, _), _)| module == &target_module)
                {
                    let _ = module;
                    edges[local].extend(producers.iter().copied());
                }
            }
            if let Some(producers) = glob_producers.get(&target_module) {
                edges[local].extend(producers.iter().copied());
            }
        }
        edges[local].sort_unstable();
        edges[local].dedup();
    }
    edges
}

fn cyclic_components(edges: &[Vec<usize>]) -> Vec<Option<Vec<usize>>> {
    let components = strongly_connected_components(edges);
    let mut cyclic_component = vec![None; edges.len()];
    for component in components {
        let cyclic = component.len() > 1
            || component
                .first()
                .is_some_and(|node| edges[*node].binary_search(node).is_ok());
        if cyclic {
            for &node in &component {
                cyclic_component[node] = Some(component.clone());
            }
        }
    }
    cyclic_component
}

fn unresolved_import_error(
    project: &HirProject,
    imports: &[(CanonicalModulePath, &UseItem)],
    unresolved: &[usize],
    local: usize,
    import_index: usize,
    cyclic_component: Option<&[usize]>,
) -> ProjectSymbolLinkError {
    let (module, import) = &imports[import_index];
    let source = source_span(project, module, *import.range());
    let reference = import_reference(import)
        .expect("an unresolved import already passed typed path validation");
    let Some(component) = cyclic_component else {
        return ProjectSymbolLinkError::UnknownImport {
            module: module.clone(),
            import: reference,
            source,
        };
    };
    let mut related = component
        .iter()
        .filter(|&&node| node != local)
        .map(|&node| {
            let (module, import) = &imports[unresolved[node]];
            source_span(project, module, *import.range())
        })
        .collect::<Vec<_>>();
    sort_spans(&mut related);
    related.dedup();
    ProjectSymbolLinkError::CyclicImport {
        module: module.clone(),
        import: reference,
        source,
        related: related.into_boxed_slice(),
    }
}

fn import_produced_names(import: &UseItem) -> Option<Vec<String>> {
    match import.tree().kind() {
        UseTreeKind::Path { path, alias } => Some(vec![alias.as_ref().map_or_else(
            || {
                LinkedProjectSymbolPath::try_new(path.path())
                    .expect("parsed import path is valid")
                    .unaliased_binding()
                    .to_string()
            },
            |alias| alias.name().as_str().to_owned(),
        )]),
        UseTreeKind::Glob { .. } => None,
        UseTreeKind::Group { names, .. } => Some(
            names
                .iter()
                .map(|name| {
                    name.alias().map_or_else(
                        || name.name().as_str().to_owned(),
                        |alias| alias.name().as_str().to_owned(),
                    )
                })
                .collect(),
        ),
    }
}

fn import_requirements(
    table: &ProjectSymbolTable,
    importer: &CanonicalModulePath,
    import: &UseItem,
) -> Vec<(CanonicalModulePath, Option<String>)> {
    match import.tree().kind() {
        UseTreeKind::Path { path, .. } => {
            let Ok(path) = LinkedProjectSymbolPath::try_new(path.path()) else {
                return Vec::new();
            };
            let reference = path.reference();
            let Ok(module) = ProjectSymbolTable::qualifier_module(importer, reference) else {
                return Vec::new();
            };
            if table.modules.contains(&module) {
                vec![(module, Some(reference.leaf().to_owned()))]
            } else {
                Vec::new()
            }
        }
        UseTreeKind::Glob { module } => {
            let Ok(path) = LinkedProjectSymbolPath::try_new(module.path()) else {
                return Vec::new();
            };
            table
                .module_for_symbol_path(importer, path.reference())
                .map(|module| vec![(module, None)])
                .unwrap_or_default()
        }
        UseTreeKind::Group { module, names } => {
            let Ok(path) = LinkedProjectSymbolPath::try_new(module.path()) else {
                return Vec::new();
            };
            table
                .module_for_symbol_path(importer, path.reference())
                .map(|module| {
                    names
                        .iter()
                        .map(|name| (module.clone(), Some(name.name().as_str().to_owned())))
                        .collect()
                })
                .unwrap_or_default()
        }
    }
}

fn import_reference(import: &UseItem) -> Result<SymbolPath, ImportResolutionError> {
    match import.tree().kind() {
        UseTreeKind::Path { path, .. } => {
            LinkedProjectSymbolPath::try_new(path.path()).map(|path| path.reference)
        }
        UseTreeKind::Glob { module } | UseTreeKind::Group { module, .. } => {
            LinkedProjectSymbolPath::try_new(module.path()).map(|path| path.reference)
        }
    }
}

fn strongly_connected_components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut visited = vec![false; edges.len()];
    let mut order = Vec::with_capacity(edges.len());
    for start in 0..edges.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![(start, 0_usize)];
        while let Some((node, next)) = stack.pop() {
            if let Some(&target) = edges[node].get(next) {
                stack.push((node, next + 1));
                if !visited[target] {
                    visited[target] = true;
                    stack.push((target, 0));
                }
            } else {
                order.push(node);
            }
        }
    }

    let mut reverse = vec![Vec::new(); edges.len()];
    for (source, targets) in edges.iter().enumerate() {
        for &target in targets {
            reverse[target].push(source);
        }
    }
    for sources in &mut reverse {
        sources.sort_unstable();
        sources.dedup();
    }
    visited.fill(false);
    let mut components = Vec::new();
    while let Some(start) = order.pop() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut component = Vec::new();
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            component.push(node);
            for &source in reverse[node].iter().rev() {
                if !visited[source] {
                    visited[source] = true;
                    stack.push(source);
                }
            }
        }
        component.sort_unstable();
        components.push(component);
    }
    components.sort_by_key(|component| component[0]);
    components
}
