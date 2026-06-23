use crate::object::ModuleObject;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_project::fingerprint::BuildDigest;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Linker request for a set of module objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkRequest {
    selected_entries: Vec<String>,
}

/// Linked program summary produced from module objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkedProgram {
    selected_entries: Vec<String>,
    modules: Vec<CanonicalModulePath>,
    object_root: BuildDigest,
}

/// Link-time structural failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum LinkError {
    #[error("link request must select at least one entry")]
    MissingEntry,
    #[error("link request has no module objects")]
    MissingObjects,
    #[error("module object `{module}` appears more than once")]
    DuplicateObject { module: CanonicalModulePath },
}

impl LinkRequest {
    /// Creates a link request with deterministic selected entries.
    pub fn new(selected_entries: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut selected_entries = selected_entries
            .into_iter()
            .map(Into::into)
            .collect::<Vec<_>>();
        selected_entries.sort();
        selected_entries.dedup();
        Self { selected_entries }
    }

    pub fn selected_entries(&self) -> &[String] {
        &self.selected_entries
    }
}

impl LinkedProgram {
    pub fn selected_entries(&self) -> &[String] {
        &self.selected_entries
    }

    pub fn modules(&self) -> &[CanonicalModulePath] {
        &self.modules
    }

    pub const fn object_root(&self) -> BuildDigest {
        self.object_root
    }
}

/// Links module objects into a deterministic program summary.
pub fn link_project(
    objects: impl IntoIterator<Item = ModuleObject>,
    request: &LinkRequest,
) -> Result<LinkedProgram, LinkError> {
    if request.selected_entries().is_empty() {
        return Err(LinkError::MissingEntry);
    }
    let mut by_module = BTreeMap::new();
    for object in objects {
        let module = object.module().clone();
        if by_module.insert(module.clone(), object).is_some() {
            return Err(LinkError::DuplicateObject { module });
        }
    }
    if by_module.is_empty() {
        return Err(LinkError::MissingObjects);
    }
    let modules = by_module.keys().cloned().collect::<Vec<_>>();
    let object_root = object_root(&by_module);
    Ok(LinkedProgram {
        selected_entries: request.selected_entries().to_vec(),
        modules,
        object_root,
    })
}

fn object_root(objects: &BTreeMap<CanonicalModulePath, ModuleObject>) -> BuildDigest {
    let mut bytes = Vec::new();
    let len = u32::try_from(objects.len()).expect("object count fits u32");
    bytes.extend_from_slice(&len.to_le_bytes());
    for (module, object) in objects {
        put_string(&mut bytes, &module.to_string());
        bytes.extend_from_slice(&object.object_digest().as_bytes());
    }
    BuildDigest::of(&bytes)
}

fn put_string(out: &mut Vec<u8>, value: &str) {
    let len = u32::try_from(value.len()).expect("link string length fits u32");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

/// Returns the selected entries that are not represented by known stable names.
pub fn missing_entry_names<'a>(
    selected_entries: &'a [String],
    known_entries: impl IntoIterator<Item = &'a str>,
) -> Vec<&'a str> {
    let known_entries = known_entries.into_iter().collect::<BTreeSet<_>>();
    selected_entries
        .iter()
        .map(String::as_str)
        .filter(|entry| !known_entries.contains(entry))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{LinkError, LinkRequest, link_project};
    use crate::object::ModuleObject;
    use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModulePath};
    use arcweft_project::fingerprint::BuildDigest;

    fn module(path: &str) -> CanonicalModulePath {
        path.parse::<ModulePath>()
            .expect("module path")
            .resolve_from(&CanonicalModulePath::crate_root())
            .expect("canonical path")
    }

    fn object(path: &str) -> ModuleObject {
        ModuleObject::new(
            module(path),
            BuildDigest::of(b"interface"),
            BuildDigest::of(b"body"),
            path.as_bytes().to_vec(),
        )
    }

    #[test]
    fn link_project_orders_entries_and_modules() {
        let request = LinkRequest::new(["game.release", "game.dev", "game.dev"]);
        let linked = link_project([object("b"), object("a")], &request).expect("links");

        assert_eq!(linked.selected_entries(), &["game.dev", "game.release"]);
        assert_eq!(linked.modules()[0], module("a"));
        assert_eq!(linked.modules()[1], module("b"));
    }

    #[test]
    fn link_project_rejects_duplicate_objects() {
        let request = LinkRequest::new(["game.dev"]);
        assert!(matches!(
            link_project([object("a"), object("a")], &request),
            Err(LinkError::DuplicateObject { .. })
        ));
    }
}
