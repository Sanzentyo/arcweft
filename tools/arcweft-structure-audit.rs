#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "arcweft-structure-audit"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false

[dependencies]
anyhow = "1.0.102"
clap = { version = "4.6.1", features = ["derive"] }
---

/*
Single-file structural scanner for an Arcweft checkout. It does not modify source files.
The default is dry-run: it scans and prints a summary without writing reports.

```bash
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root .
cargo +nightly -Zscript tools/arcweft-structure-audit.rs \
  --root . --write target/structure-audit --fail-on-violations
```
*/

use anyhow::Result;
use args::ParseOutcome;

fn main() {
    match run() {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32> {
    let arguments = match args::parse()? {
        ParseOutcome::Help => {
            print!("{}", args::help());
            return Ok(0);
        }
        ParseOutcome::Run(arguments) => arguments,
    };

    let root = arguments.root.canonicalize()?;
    let paths = walk::collect_files(&root)?;
    let files = metrics::analyze_files(&root, &paths)?;
    let manifests = cargo_manifest::parse_manifests(&root, &paths)?;
    let violations = rules::evaluate(&files, &manifests);

    if let Some(directory) = arguments.write_dir.as_deref() {
        report::write_reports(directory, &files, &manifests, &violations)?;
    }
    report::print_summary(
        &files,
        &manifests,
        &violations,
        arguments.write_dir.as_deref(),
    );

    Ok(if arguments.fail_on_violations && !violations.is_empty() {
        2
    } else {
        0
    })
}

mod args {
    use anyhow::Result;
    use clap::{error::ErrorKind, Parser};
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[derive(Clone, Debug, Eq, Parser, PartialEq)]
    #[command(
        name = "arcweft-structure-audit",
        disable_version_flag = true,
        args_override_self = true
    )]
    pub struct Args {
        #[arg(
            long,
            value_name = "PATH",
            default_value = ".",
            allow_hyphen_values = true
        )]
        pub root: PathBuf,
        #[arg(long = "write", value_name = "DIR", allow_hyphen_values = true)]
        pub write_dir: Option<PathBuf>,
        #[arg(long = "fail-on-violations")]
        pub fail_on_violations: bool,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum ParseOutcome {
        Run(Args),
        Help,
    }

    pub fn parse() -> Result<ParseOutcome> {
        parse_from(std::env::args_os().skip(1))
    }

    fn parse_from<I>(arguments: I) -> Result<ParseOutcome>
    where
        I: IntoIterator<Item = OsString>,
    {
        let arguments = std::iter::once(OsString::from("arcweft-structure-audit")).chain(arguments);

        match Args::try_parse_from(arguments) {
            Ok(arguments) => Ok(ParseOutcome::Run(arguments)),
            Err(error) if error.kind() == ErrorKind::DisplayHelp => Ok(ParseOutcome::Help),
            Err(error) => Err(anyhow::Error::msg(clap_error_message(error))),
        }
    }

    fn clap_error_message(error: clap::Error) -> String {
        let message = error.to_string();
        message
            .strip_prefix("error: ")
            .unwrap_or(&message)
            .trim_end()
            .to_owned()
    }

    pub fn help() -> &'static str {
        concat!(
            "arcweft-structure-audit\n\n",
            "Usage:\n",
            "  arcweft-structure-audit [--root PATH] [--write DIR] ",
            "[--fail-on-violations]\n\n",
            "Options:\n",
            "  --root PATH             Repository root (default: .)\n",
            "  --write DIR             Write CSV/Markdown reports; omitted means dry-run\n",
            "  --fail-on-violations    Exit with status 2 when warnings or errors are found\n",
            "  -h, --help              Show this help\n",
        )
    }
}

mod cargo_manifest {
    use anyhow::Result;
    use std::collections::BTreeSet;
    use std::fs;
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

    pub fn parse_manifests(root: &Path, files: &[PathBuf]) -> Result<Vec<Manifest>> {
        let mut manifests = files
            .iter()
            .filter(|path| path.file_name().and_then(|name| name.to_str()) == Some("Cargo.toml"))
            .filter_map(|path| match parse_manifest(root, path) {
                Ok(Some(manifest)) => Some(Ok(manifest)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>>>()?;
        manifests.sort_by(|left, right| left.package.cmp(&right.package));
        Ok(manifests)
    }

    fn parse_manifest(root: &Path, path: &Path) -> Result<Option<Manifest>> {
        let content = fs::read_to_string(path)?;
        parse_manifest_text(root, path, &content)
    }

    fn parse_manifest_text(root: &Path, path: &Path, content: &str) -> Result<Option<Manifest>> {
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
}

mod metrics {
    use anyhow::Result;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct PublicType {
        pub kind: String,
        pub name: String,
        pub line: usize,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct SourceHit {
        pub line: usize,
        pub text: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct FileMetrics {
        pub path: String,
        pub bytes: u64,
        pub physical_lines: usize,
        pub code_lines: usize,
        pub is_rust: bool,
        pub is_test: bool,
        pub is_generated: bool,
        pub has_embedded_tests: bool,
        pub public_types: Vec<PublicType>,
        pub stringly_boundary_hits: Vec<SourceHit>,
    }

    impl FileMetrics {
        pub const fn classification(&self) -> &'static str {
            if self.is_generated {
                "generated"
            } else if self.is_test {
                "test"
            } else if self.is_rust {
                "production"
            } else {
                "text"
            }
        }
    }

    pub fn analyze_files(root: &Path, files: &[PathBuf]) -> Result<Vec<FileMetrics>> {
        files
            .iter()
            .filter_map(|path| match analyze_file(root, path) {
                Ok(Some(metrics)) => Some(Ok(metrics)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    fn analyze_file(root: &Path, path: &Path) -> Result<Option<FileMetrics>> {
        if !is_text_candidate(path) {
            return Ok(None);
        }
        let bytes = fs::read(path)?;
        let content = match String::from_utf8(bytes.clone()) {
            Ok(content) => content,
            Err(_) => return Ok(None),
        };
        let relative = path.strip_prefix(root).unwrap_or(path);
        let normalized = normalize_path(relative);
        let is_rust = path.extension().and_then(|value| value.to_str()) == Some("rs");
        let lines = content.lines().collect::<Vec<_>>();
        let physical_lines = if content.is_empty() {
            0
        } else {
            content.bytes().filter(|byte| *byte == b'\n').count()
                + usize::from(!content.ends_with('\n'))
        };
        let code_lines = count_code_lines(&lines);
        let is_generated = is_rust && is_generated_source(&lines);
        let public_types = if is_rust {
            public_types(&lines)
        } else {
            Vec::new()
        };
        let stringly_boundary_hits = if is_rust {
            stringly_boundary_hits(&lines)
        } else {
            Vec::new()
        };

        Ok(Some(FileMetrics {
            path: normalized.clone(),
            bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            physical_lines,
            code_lines,
            is_rust,
            is_test: is_test_path(&normalized),
            is_generated,
            has_embedded_tests: is_rust && has_embedded_test_module(&lines),
            public_types,
            stringly_boundary_hits,
        }))
    }

    fn is_text_candidate(path: &Path) -> bool {
        matches!(
            path.extension().and_then(|value| value.to_str()),
            Some(
                "arcw"
                    | "awfagent"
                    | "css"
                    | "html"
                    | "js"
                    | "json"
                    | "md"
                    | "rs"
                    | "sh"
                    | "toml"
                    | "ts"
                    | "txt"
                    | "wgsl"
                    | "yml"
                    | "yaml"
            )
        )
    }

    fn normalize_path(path: &Path) -> String {
        path.components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    }

    fn is_test_path(path: &str) -> bool {
        path.contains("/tests/")
            || path.starts_with("tests/")
            || path.ends_with("/tests.rs")
            || path.ends_with("_test.rs")
            || path.ends_with("_tests.rs")
    }

    fn is_generated_source(lines: &[&str]) -> bool {
        lines.iter().take(20).any(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("generated")
                || lower.contains("do not edit")
                || lower.contains("@generated")
        })
    }

    fn count_code_lines(lines: &[&str]) -> usize {
        let mut block_comment = false;
        lines
            .iter()
            .filter(|line| line_has_code(line, &mut block_comment))
            .count()
    }

    fn line_has_code(line: &str, block_comment: &mut bool) -> bool {
        let mut rest = line.trim();
        if rest.is_empty() {
            return false;
        }

        loop {
            if *block_comment {
                let Some(end) = rest.find("*/") else {
                    return false;
                };
                rest = rest[end + 2..].trim_start();
                *block_comment = false;
                if rest.is_empty() {
                    return false;
                }
                continue;
            }
            if rest.starts_with("//") {
                return false;
            }
            if rest.starts_with("/*") {
                *block_comment = true;
                rest = &rest[2..];
                continue;
            }
            return true;
        }
    }

    fn public_types(lines: &[&str]) -> Vec<PublicType> {
        lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| parse_public_type(line, index + 1))
            .collect()
    }

    fn parse_public_type(line: &str, line_number: usize) -> Option<PublicType> {
        let trimmed = line.trim_start();
        let rest = if let Some(rest) = trimmed.strip_prefix("pub ") {
            rest
        } else {
            let rest = trimmed.strip_prefix("pub(")?;
            let (_, rest) = rest.split_once(')')?;
            rest.trim_start()
        };

        ["struct", "enum", "trait", "type"]
            .into_iter()
            .find_map(|kind| {
                let rest = rest.strip_prefix(kind)?.trim_start();
                let name = rest
                    .chars()
                    .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                    .collect::<String>();
                (!name.is_empty()).then(|| PublicType {
                    kind: kind.to_owned(),
                    name,
                    line: line_number,
                })
            })
    }

    fn stringly_boundary_hits(lines: &[&str]) -> Vec<SourceHit> {
        const PATTERNS: &[&str] = &[
            "pub kind: String",
            "pub payload: Option<String>",
            "pub event_type: String",
            "pub action: String",
        ];
        lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| {
                let trimmed = line.trim();
                PATTERNS
                    .iter()
                    .any(|pattern| trimmed.contains(pattern))
                    .then(|| SourceHit {
                        line: index + 1,
                        text: trimmed.to_owned(),
                    })
            })
            .collect()
    }

    fn has_embedded_test_module(lines: &[&str]) -> bool {
        lines.iter().enumerate().any(|(index, line)| {
            let trimmed = line.trim();
            if !trimmed.starts_with("#[cfg(test)]") {
                return false;
            }
            if trimmed.contains("mod tests {") {
                return true;
            }
            lines
                .get(index + 1)
                .is_some_and(|next| next.trim_start().starts_with("mod tests {"))
        })
    }
}

mod report {
    use crate::cargo_manifest::Manifest;
    use crate::metrics::FileMetrics;
    use crate::rules::{Severity, Violation};
    use anyhow::Result;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::Path;

    pub fn print_summary(
        files: &[FileMetrics],
        manifests: &[Manifest],
        violations: &[Violation],
        wrote_to: Option<&Path>,
    ) {
        let rust_files = files.iter().filter(|file| file.is_rust).count();
        let rust_loc = files
            .iter()
            .filter(|file| file.is_rust)
            .map(|file| file.physical_lines)
            .sum::<usize>();
        let warnings = violations
            .iter()
            .filter(|violation| violation.severity == Severity::Warning)
            .count();
        let errors = violations
            .iter()
            .filter(|violation| violation.severity == Severity::Error)
            .count();

        println!("files scanned: {}", files.len());
        println!("Rust files: {rust_files}");
        println!("Rust physical LOC: {rust_loc}");
        println!("package manifests: {}", manifests.len());
        println!("violations: {errors} error(s), {warnings} warning(s)");
        match wrote_to {
            Some(path) => println!("reports written to {}", path.display()),
            None => println!("dry-run: no report files written (use --write DIR)"),
        }
    }

    pub fn write_reports(
        directory: &Path,
        files: &[FileMetrics],
        manifests: &[Manifest],
        violations: &[Violation],
    ) -> Result<()> {
        fs::create_dir_all(directory)?;
        fs::write(directory.join("file_metrics.csv"), file_metrics_csv(files))?;
        fs::write(
            directory.join("dependency_edges.csv"),
            dependency_edges_csv(manifests),
        )?;
        fs::write(
            directory.join("public_type_duplicates.csv"),
            public_type_duplicates_csv(files),
        )?;
        fs::write(
            directory.join("violations.md"),
            violations_markdown(violations),
        )?;
        Ok(())
    }

    fn file_metrics_csv(files: &[FileMetrics]) -> String {
        let mut output = String::from(
            "path,bytes,physical_lines,code_lines,classification,is_rust,is_test,is_generated,has_embedded_tests\n",
        );
        for file in files {
            output.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                csv(&file.path),
                file.bytes,
                file.physical_lines,
                file.code_lines,
                file.classification(),
                file.is_rust,
                file.is_test,
                file.is_generated,
                file.has_embedded_tests
            ));
        }
        output
    }

    fn dependency_edges_csv(manifests: &[Manifest]) -> String {
        let mut output = String::from("package,dependency,kind,manifest\n");
        for manifest in manifests {
            for dependency in &manifest.dependencies {
                output.push_str(&format!(
                    "{},{},{},{}\n",
                    csv(&manifest.package),
                    csv(&dependency.name),
                    dependency.kind.as_str(),
                    csv(&manifest.path)
                ));
            }
        }
        output
    }

    fn public_type_duplicates_csv(files: &[FileMetrics]) -> String {
        let mut occurrences = BTreeMap::<String, Vec<(String, String, usize)>>::new();
        for file in files {
            for item in &file.public_types {
                occurrences.entry(item.name.clone()).or_default().push((
                    item.kind.clone(),
                    file.path.clone(),
                    item.line,
                ));
            }
        }

        let mut output = String::from("name,kind,path,line,crate_count\n");
        for (name, entries) in occurrences {
            let crates = entries
                .iter()
                .filter_map(|(_, path, _)| crate_directory(path))
                .map(ToOwned::to_owned)
                .collect::<BTreeSet<_>>();
            if crates.len() <= 1 {
                continue;
            }
            for (kind, path, line) in entries {
                output.push_str(&format!(
                    "{},{},{},{},{}\n",
                    csv(&name),
                    csv(&kind),
                    csv(&path),
                    line,
                    crates.len()
                ));
            }
        }
        output
    }

    fn crate_directory(path: &str) -> Option<&str> {
        let mut parts = path.split('/');
        (parts.next()? == "crates").then(|| parts.next()).flatten()
    }

    fn violations_markdown(violations: &[Violation]) -> String {
        let mut output = String::from("# Structural violations\n\n");
        if violations.is_empty() {
            output.push_str("No configured violations were found.\n");
            return output;
        }
        for violation in violations {
            let location = violation.line.map_or_else(
                || violation.path.clone(),
                |line| format!("{}:{line}", violation.path),
            );
            output.push_str(&format!(
                "## {} {} — `{}`\n\n{}\n\n**Fix:** {}\n\n",
                violation.severity,
                violation.code,
                location,
                violation.message,
                violation.suggestion
            ));
        }
        output
    }

    fn csv(value: &str) -> String {
        if value
            .chars()
            .any(|character| matches!(character, ',' | '"' | '\n' | '\r'))
        {
            format!("\"{}\"", value.replace('"', "\"\""))
        } else {
            value.to_owned()
        }
    }
}

mod rules {
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
                    suggestion: "review responsibility boundaries before adding more code"
                        .to_owned(),
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
                    suggestion:
                        "move implementations to named modules and keep intentional re-exports"
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
                    suggestion:
                        "move tests to domain-specific child test modules or integration tests"
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

    fn forbidden_dependency(
        package: &str,
        dependency: &str,
    ) -> Option<(&'static str, String, String)> {
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
                "depend on arcweft-text-model; keep resolver/parsers in arcweft-render-text"
                    .to_owned(),
            ));
        }
        if package == "arcweft-agent-protocol"
            && matches!(dependency, "arcweft-render-text" | "base64")
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
}

mod walk {
    use anyhow::Result;
    use std::fs;
    use std::path::{Path, PathBuf};

    const IGNORED_DIRECTORIES: &[&str] = &[
        ".git",
        ".idea",
        ".vscode",
        "node_modules",
        "target",
        "vendor",
    ];

    pub fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        visit(root, &mut files)?;
        files.sort();
        Ok(files)
    }

    fn visit(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        let mut entries = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);

        for entry in entries {
            let file_type = entry.file_type()?;
            let path = entry.path();
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                if !is_ignored_directory(&path) {
                    visit(&path, files)?;
                }
            } else if file_type.is_file() {
                files.push(path);
            }
        }
        Ok(())
    }

    fn is_ignored_directory(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| IGNORED_DIRECTORIES.contains(&name))
    }
}
