use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DependencyKind {
    Normal,
    Development,
    Build,
    Target,
}

impl DependencyKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Development => "development",
            Self::Build => "build",
            Self::Target => "target",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Dependency {
    pub name: String,
    pub kind: DependencyKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Manifest {
    pub package: String,
    pub path: String,
    pub dependencies: Vec<Dependency>,
}

pub fn parse_manifests(root: &Path, files: &[PathBuf]) -> io::Result<Vec<Manifest>> {
    let mut manifests = files
        .iter()
        .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml"))
        .filter_map(|path| match parse_manifest(root, path) {
            Ok(Some(manifest)) => Some(Ok(manifest)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        })
        .collect::<io::Result<Vec<_>>>()?;
    manifests.sort_by(|left, right| left.package.cmp(&right.package));
    Ok(manifests)
}

fn parse_manifest(root: &Path, path: &Path) -> io::Result<Option<Manifest>> {
    let content = fs::read_to_string(path)?;
    parse_manifest_text(root, path, &content)
}

fn parse_manifest_text(root: &Path, path: &Path, content: &str) -> io::Result<Option<Manifest>> {
    let mut section = String::new();
    let mut package = None;
    let mut dependencies = BTreeSet::new();

    for raw_line in content.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_owned();
            if let Some((name, kind)) = dependency_from_table_section(&section) {
                dependencies.insert(Dependency { name, kind });
            }
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key.trim().trim_matches('"');
        let value = raw_value.trim();
        if section == "package" && key == "name" {
            package = parse_quoted(value).map(str::to_owned);
            continue;
        }
        if let Some(kind) = dependency_kind(&section) {
            let name = key.strip_suffix(".workspace").unwrap_or(key).to_owned();
            if !name.is_empty() {
                dependencies.insert(Dependency { name, kind });
            }
        }
    }

    let Some(package) = package else {
        return Ok(None);
    };
    let relative = path.strip_prefix(root).unwrap_or(path);
    Ok(Some(Manifest {
        package,
        path: relative.to_string_lossy().replace('\\', "/"),
        dependencies: dependencies.into_iter().collect(),
    }))
}

fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        match character {
            '\\' if quoted => escaped = !escaped,
            '"' if !escaped => quoted = !quoted,
            '#' if !quoted => return &line[..index],
            _ => escaped = false,
        }
    }
    line
}

fn parse_quoted(value: &str) -> Option<&str> {
    value
        .strip_prefix('"')?
        .split_once('"')
        .map(|(value, _)| value)
}

fn dependency_kind(section: &str) -> Option<DependencyKind> {
    match section {
        "dependencies" => Some(DependencyKind::Normal),
        "dev-dependencies" => Some(DependencyKind::Development),
        "build-dependencies" => Some(DependencyKind::Build),
        section if section.contains(".dependencies") => Some(DependencyKind::Target),
        _ => None,
    }
}

fn dependency_from_table_section(section: &str) -> Option<(String, DependencyKind)> {
    for (marker, kind) in [
        ("dependencies.", DependencyKind::Normal),
        ("dev-dependencies.", DependencyKind::Development),
        ("build-dependencies.", DependencyKind::Build),
    ] {
        if let Some(name) = section.strip_prefix(marker) {
            return Some((name.trim_matches('"').to_owned(), kind));
        }
    }
    let marker = ".dependencies.";
    section
        .rsplit_once(marker)
        .map(|(_, name)| (name.trim_matches('"').to_owned(), DependencyKind::Target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_workspace_and_inline_dependencies() {
        let text = r#"
            [package]
            name = "arcweft-runtime-plan"

            [dependencies]
            arcweft-core.workspace = true
            arcweft-render-text = { workspace = true }

            [dev-dependencies]
            serde_json.workspace = true
        "#;
        let manifest = parse_manifest_text(Path::new("."), Path::new("Cargo.toml"), text)
            .expect("parse manifest")
            .expect("package manifest");
        assert_eq!(manifest.package, "arcweft-runtime-plan");
        assert!(manifest.dependencies.iter().any(|dependency| {
            dependency.name == "arcweft-core" && dependency.kind == DependencyKind::Normal
        }));
        assert!(manifest.dependencies.iter().any(|dependency| {
            dependency.name == "serde_json" && dependency.kind == DependencyKind::Development
        }));
    }
}
