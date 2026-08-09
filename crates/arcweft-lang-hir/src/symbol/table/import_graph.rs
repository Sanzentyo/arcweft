//! Deterministic classification of unresolved final-HIR imports.

use std::collections::BTreeMap;

use arcweft_lang_syntax::ast::{module_path::CanonicalModulePath, symbol_path::SymbolPath};

use crate::item::HirUseBindingKind;

use super::imports::linked_path;
use super::{
    ImportResolutionError, ProjectImportRef, ProjectSymbolLinkError, ProjectSymbolTable, sort_spans,
};

pub(super) fn classify_unresolved_imports(
    table: &ProjectSymbolTable,
    imports: &[ProjectImportRef<'_>],
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
    let first_source = unresolved
        .first()
        .map(|index| imports[*index].whole_source());
    ProjectSymbolTable::charge(work, units, first_source).map_err(Box::new)?;

    let cyclic_components = cyclic_components(&edges);
    Ok(unresolved
        .iter()
        .enumerate()
        .map(|(local, &import_index)| {
            unresolved_import_error(
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
    imports: &[ProjectImportRef<'_>],
    unresolved: &[usize],
) -> Vec<Vec<usize>> {
    let mut exact_producers = BTreeMap::<(CanonicalModulePath, String), Vec<usize>>::new();
    let mut glob_producers = BTreeMap::<CanonicalModulePath, Vec<usize>>::new();
    for (local, &import_index) in unresolved.iter().enumerate() {
        let import = imports[import_index];
        match import_produced_name(import) {
            Some(name) => exact_producers
                .entry((import.module_path.clone(), name))
                .or_default()
                .push(local),
            None => glob_producers
                .entry(import.module_path.clone())
                .or_default()
                .push(local),
        }
    }

    let mut edges = vec![Vec::new(); unresolved.len()];
    for (local, &import_index) in unresolved.iter().enumerate() {
        let import = imports[import_index];
        for (target_module, name) in import_requirements(table, import) {
            if let Some(name) = name {
                if let Some(producers) = exact_producers.get(&(target_module.clone(), name)) {
                    edges[local].extend(producers.iter().copied());
                }
            } else {
                for ((_module, _), producers) in exact_producers
                    .range((target_module.clone(), String::new())..)
                    .take_while(|((module, _), _)| module == &target_module)
                {
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

fn unresolved_import_error(
    imports: &[ProjectImportRef<'_>],
    unresolved: &[usize],
    local: usize,
    import_index: usize,
    cyclic_component: Option<&[usize]>,
) -> ProjectSymbolLinkError {
    let import = imports[import_index];
    let source = import.whole_source();
    let reference = import_reference(import)
        .expect("unresolved final HIR import already passed typed path validation");
    let Some(component) = cyclic_component else {
        return ProjectSymbolLinkError::UnknownImport {
            module: import.module_path.clone(),
            import: reference,
            source,
        };
    };
    let mut related = component
        .iter()
        .filter(|&&node| node != local)
        .map(|&node| imports[unresolved[node]].whole_source())
        .collect::<Vec<_>>();
    sort_spans(&mut related);
    related.dedup();
    ProjectSymbolLinkError::CyclicImport {
        module: import.module_path.clone(),
        import: reference,
        source,
        related: related.into_boxed_slice(),
    }
}

fn import_produced_name(import: ProjectImportRef<'_>) -> Option<String> {
    if import.binding.kind() == HirUseBindingKind::Glob {
        return None;
    }
    if let Some(alias) = import.binding.alias() {
        return Some(alias.as_str().to_owned());
    }
    linked_path(import.binding.path().as_resolved()?)
        .ok()
        .map(|path| path.unaliased_binding().to_string())
}

fn import_requirements(
    table: &ProjectSymbolTable,
    import: ProjectImportRef<'_>,
) -> Vec<(CanonicalModulePath, Option<String>)> {
    let Some(path) = import.binding.path().as_resolved() else {
        return Vec::new();
    };
    let Ok(path) = linked_path(path) else {
        return Vec::new();
    };
    match import.binding.kind() {
        HirUseBindingKind::Item => {
            let reference = path.reference();
            let Ok(module) = ProjectSymbolTable::qualifier_module(import.module_path, reference)
            else {
                return Vec::new();
            };
            if table.modules.contains(&module) {
                vec![(module, Some(reference.leaf().to_owned()))]
            } else {
                Vec::new()
            }
        }
        HirUseBindingKind::Glob => table
            .module_for_symbol_path(import.module_path, path.reference())
            .map(|module| vec![(module, None)])
            .unwrap_or_default(),
    }
}

fn import_reference(import: ProjectImportRef<'_>) -> Result<SymbolPath, ImportResolutionError> {
    let path = import
        .binding
        .path()
        .as_resolved()
        .ok_or(ImportResolutionError::Unknown)?;
    linked_path(path).map(|path| path.reference)
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
