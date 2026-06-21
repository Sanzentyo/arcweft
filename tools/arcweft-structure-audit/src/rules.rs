use crate::cargo_manifest::{DependencyKind, Manifest};
use crate::metrics::FileMetrics;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Severity {
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Warning => "warning",
            Self::Error => "error",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Violation {
    pub severity: Severity,
    pub code: &'static str,
    pub path: String,
    pub line: Option<usize>,
    pub message: String,
    pub suggestion: String,
}

pub fn evaluate(files: &[FileMetrics], manifests: &[Manifest]) -> Vec<Violation> {
    let mut violations = Vec::new();
    evaluate_file_size(files, &mut violations);
    evaluate_stringly_boundaries(files, &mut violations);
    evaluate_duplicate_boundary_types(files, &mut violations);
    evaluate_dependencies(manifests, &mut violations);
    violations.sort_by(|left, right| {
        left.severity
            .cmp(&right.severity)
            .reverse()
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.code.cmp(right.code))
    });
    violations
}

fn evaluate_file_size(files: &[FileMetrics], violations: &mut Vec<Violation>) {
    for file in files
        .iter()
        .filter(|file| file.is_rust && !file.is_generated)
    {
        let (warning, error) = if file.is_test {
            (2_500, 8_000)
        } else {
            (1_200, 2_500)
        };
        if file.physical_lines > error {
            violations.push(Violation {
                severity: Severity::Error,
                code: "SIZE001",
                path: file.path.clone(),
                line: None,
                message: format!(
                    "{} physical LOC exceeds the {} LOC error threshold",
                    file.physical_lines, error
                ),
                suggestion: "split by cohesive domain and keep a small facade".to_owned(),
            });
        } else if file.physical_lines > warning {
            violations.push(Violation {
                severity: Severity::Warning,
                code: "SIZE001",
                path: file.path.clone(),
                line: None,
                message: format!(
                    "{} physical LOC exceeds the {} LOC review threshold",
                    file.physical_lines, warning
                ),
                suggestion: "review responsibility boundaries before adding more code".to_owned(),
            });
        }

        if is_facade_name(&file.path) && file.physical_lines > 1_000 {
            violations.push(Violation {
                severity: Severity::Warning,
                code: "SIZE002",
                path: file.path.clone(),
                line: None,
                message: format!(
                    "facade file has {} physical LOC; target is below 250 LOC",
                    file.physical_lines
                ),
                suggestion: "move implementations to named modules and keep intentional re-exports"
                    .to_owned(),
            });
        }
        if !file.is_test && file.has_embedded_tests && file.physical_lines > 1_200 {
            violations.push(Violation {
                severity: Severity::Warning,
                code: "TEST001",
                path: file.path.clone(),
                line: None,
                message: "large production file contains an embedded #[cfg(test)] module"
                    .to_owned(),
                suggestion: "move tests to domain-specific child test modules or integration tests"
                    .to_owned(),
            });
        }
    }
}

fn is_facade_name(path: &str) -> bool {
    path.ends_with("/lib.rs") || path.ends_with("/main.rs")
}

fn evaluate_stringly_boundaries(files: &[FileMetrics], violations: &mut Vec<Violation>) {
    for file in files {
        let boundary_path = file.path.contains("arcweft-core/src/step.rs")
            || file.path.contains("agent-protocol")
            || file.path.contains("presentation/src/input.rs");
        if !boundary_path {
            continue;
        }
        violations.extend(file.stringly_boundary_hits.iter().map(|hit| Violation {
            severity: Severity::Warning,
            code: "TYPE001",
            path: file.path.clone(),
            line: Some(hit.line),
            message: format!("stringly boundary field: {}", hit.text),
            suggestion:
                "replace kind/payload strings with a tagged enum and typed payload".to_owned(),
        }));
    }
}

fn evaluate_duplicate_boundary_types(files: &[FileMetrics], violations: &mut Vec<Violation>) {
    let mut locations = BTreeMap::<&str, Vec<(&str, usize)>>::new();
    for file in files {
        for item in &file.public_types {
            if matches!(item.name.as_str(), "InputEvent" | "AudioEvent") {
                locations
                    .entry(item.name.as_str())
                    .or_default()
                    .push((&file.path, item.line));
            }
        }
    }
    for (name, entries) in locations {
        let crates = entries
            .iter()
            .filter_map(|(path, _)| crate_directory(path))
            .collect::<BTreeSet<_>>();
        if crates.len() <= 1 {
            continue;
        }
        for (path, line) in entries {
            violations.push(Violation {
                severity: Severity::Warning,
                code: "TYPE002",
                path: path.to_owned(),
                line: Some(line),
                message: format!("boundary type {name} is defined in multiple crates"),
                suggestion: concat!(
                    "move the shared contract to one low-level model crate and use a ",
                    "specific routed/raw name"
                )
                .to_owned(),
            });
        }
    }
}

fn crate_directory(path: &str) -> Option<&str> {
    let mut parts = path.split('/');
    (parts.next()? == "crates").then(|| parts.next()).flatten()
}

fn evaluate_dependencies(manifests: &[Manifest], violations: &mut Vec<Violation>) {
    for manifest in manifests {
        for dependency in manifest
            .dependencies
            .iter()
            .filter(|dependency| dependency.kind != DependencyKind::Development)
        {
            if let Some((code, message, suggestion)) =
                forbidden_dependency(manifest.package.as_str(), dependency.name.as_str())
            {
                violations.push(Violation {
                    severity: Severity::Warning,
                    code,
                    path: manifest.path.clone(),
                    line: None,
                    message,
                    suggestion,
                });
            }
        }
    }
}

fn forbidden_dependency(package: &str, dependency: &str) -> Option<(&'static str, String, String)> {
    if package == "arcweft-core"
        && [
            "arcweft-cli",
            "arcweft-desktop",
            "arcweft-agent-mcp",
            "arcweft-runtime-host",
            "arcweft-ui",
        ]
        .iter()
        .any(|prefix| dependency.starts_with(prefix))
    {
        return Some((
            "ARCH001",
            format!("pure runtime core depends on higher-level crate {dependency}"),
            "invert through a core-owned data/trait contract".to_owned(),
        ));
    }
    if package == "arcweft-runtime-plan" && dependency == "arcweft-render-text" {
        return Some((
            "ARCH002",
            "runtime plan owns display lowering but depends on renderer-named contract owner"
                .to_owned(),
            "depend on arcweft-text-model; keep resolver/parsers in arcweft-render-text".to_owned(),
        ));
    }
    if package == "arcweft-agent-protocol" && matches!(dependency, "arcweft-render-text" | "base64")
    {
        return Some((
            "ARCH003",
            format!("transport-neutral Agent protocol depends on {dependency}"),
            "move MCP/base64 resource mapping to arcweft-agent-mcp and model types to arcweft-text-model".to_owned(),
        ));
    }
    if package != "arcweft-cli" && dependency == "arcweft-cli" {
        return Some((
            "ARCH004",
            format!("library crate {package} depends on CLI implementation"),
            "extract the shared API to a library crate below both consumers".to_owned(),
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_plan_render_dependency_is_flagged() {
        let result = forbidden_dependency("arcweft-runtime-plan", "arcweft-render-text");
        assert!(result.is_some());
    }

    #[test]
    fn syntax_to_source_dependency_is_not_flagged() {
        assert_eq!(
            forbidden_dependency("arcweft-lang-syntax", "arcweft-source"),
            None
        );
    }
}
