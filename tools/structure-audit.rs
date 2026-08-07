#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
name = "structure-audit"
version = "0.1.0"
edition = "2024"
rust-version = "1.96"
publish = false

[dependencies]
anyhow = "1.0.102"
clap = { version = "4.6.1", features = ["derive"] }
serde_json = "1.0.150"
---

/*
Single-file structural scanner for a repository checkout. It does not modify source files.
The default is dry-run: it scans and prints a summary without writing reports.

This remains one Cargo script intentionally: its cohesive responsibility is deterministic
structural screening, while the internal modules separate metadata authority, measurement,
rules, reporting, and traversal. Splitting those modules into a workspace crate solely to
reduce this file's LOC would widen the production build graph without changing ownership.

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo +nightly -Zscript tools/structure-audit.rs \
  --root . --write target/structure-audit --fail-on-blocking
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
    let workspace = workspace::load(&root)?;
    let paths = walk::collect_files(&root, &workspace)?;
    let generated_paths = generated_files::load(&root)?;
    let files = metrics::analyze_files(&root, &paths, &generated_paths, &workspace)?;
    let findings = rules::evaluate(&files, &workspace);
    let blocking_count = findings
        .iter()
        .filter(|finding| finding.blocks_validation())
        .count();

    let write_dir = arguments.write_dir.map(|directory| {
        if directory.is_absolute() {
            directory
        } else {
            root.join(directory)
        }
    });
    if let Some(directory) = write_dir.as_deref() {
        report::write_reports(directory, &files, &workspace, &findings)?;
    }
    report::print_summary(
        &files,
        &workspace,
        &findings,
        blocking_count,
        write_dir.as_deref(),
    );

    Ok(validation_exit_code(
        arguments.fail_on_blocking,
        blocking_count,
    ))
}

const fn validation_exit_code(fail_on_blocking: bool, blocking_count: usize) -> i32 {
    if fail_on_blocking && blocking_count > 0 {
        2
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::validation_exit_code;

    #[test]
    fn only_blocking_findings_fail_the_gate() {
        assert_eq!(validation_exit_code(false, 4), 0);
        assert_eq!(validation_exit_code(true, 0), 0);
        assert_eq!(validation_exit_code(true, 1), 2);
    }
}

mod generated_files {
    use anyhow::Result;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    pub fn load(root: &Path) -> Result<BTreeSet<String>> {
        let path = root.join(".gitattributes");
        if !path.exists() {
            return Ok(BTreeSet::new());
        }
        let source = fs::read_to_string(path)?;
        Ok(source
            .lines()
            .filter_map(generated_path)
            .map(|path| path.replace('\\', "/"))
            .collect())
    }

    fn generated_path(line: &str) -> Option<String> {
        let line = line
            .split_once('#')
            .map_or(line, |(prefix, _)| prefix)
            .trim();
        if line.is_empty() {
            return None;
        }
        let mut fields = line.split_whitespace();
        let path = fields.next()?;
        fields
            .any(|field| matches!(field, "linguist-generated" | "linguist-generated=true"))
            .then(|| path.to_owned())
    }
}

mod args {
    use anyhow::Result;
    use clap::{Parser, error::ErrorKind};
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[derive(Clone, Debug, Eq, Parser, PartialEq)]
    #[command(
        name = "structure-audit",
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
        #[arg(long = "fail-on-blocking")]
        pub fail_on_blocking: bool,
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
        let arguments = std::iter::once(OsString::from("structure-audit")).chain(arguments);

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
            "structure-audit\n\n",
            "Usage:\n",
            "  structure-audit [--root PATH] [--write DIR] ",
            "[--fail-on-blocking]\n\n",
            "Options:\n",
            "  --root PATH             Repository root (default: .)\n",
            "  --write DIR             Write CSV/Markdown reports; omitted means dry-run\n",
            "  --fail-on-blocking      Exit with status 2 when blocking violations are found; ",
            "review triggers remain non-blocking\n",
            "  -h, --help              Show this help\n",
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn blocking_flag_directly_replaces_the_obsolete_violations_flag() {
            let ParseOutcome::Run(arguments) =
                parse_from([OsString::from("--fail-on-blocking")]).expect("final flag must parse")
            else {
                panic!("expected runnable arguments");
            };
            assert!(arguments.fail_on_blocking);
            assert!(parse_from([OsString::from("--fail-on-violations")]).is_err());
        }
    }
}

mod workspace {
    use anyhow::{Context, Result, bail};
    use serde_json::Value;
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub enum DependencyKind {
        Normal,
        Development,
        Build,
    }

    impl DependencyKind {
        pub const fn as_str(&self) -> &'static str {
            match self {
                Self::Normal => "normal",
                Self::Development => "development",
                Self::Build => "build",
            }
        }
    }

    #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct Dependency {
        pub name: String,
        pub kind: DependencyKind,
        pub target: Option<String>,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct Target {
        pub source_path: PathBuf,
        pub kinds: BTreeSet<String>,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct Package {
        pub name: String,
        pub manifest_path: String,
        pub root: PathBuf,
        pub dependencies: Vec<Dependency>,
        pub targets: Vec<Target>,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct PackageMetrics {
        pub package: String,
        pub manifest_path: String,
        pub workspace_fan_in: usize,
        pub workspace_fan_out: usize,
        pub development_workspace_fan_in: usize,
        pub development_workspace_fan_out: usize,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct Workspace {
        packages: Vec<Package>,
    }

    impl Workspace {
        pub fn packages(&self) -> &[Package] {
            &self.packages
        }

        pub fn package_roots(&self) -> Vec<PathBuf> {
            self.packages
                .iter()
                .map(|package| package.root.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        }

        pub fn owner_for(&self, path: &Path) -> Option<&str> {
            self.packages
                .iter()
                .filter(|package| path.starts_with(&package.root))
                .max_by_key(|package| package.root.components().count())
                .map(|package| package.name.as_str())
        }

        pub fn target_kinds_for(&self, path: &Path) -> BTreeSet<String> {
            self.packages
                .iter()
                .flat_map(|package| &package.targets)
                .filter(|target| target.source_path.as_path() == path)
                .flat_map(|target| target.kinds.iter().cloned())
                .collect()
        }

        pub fn package_metrics(&self) -> Vec<PackageMetrics> {
            let workspace_names = self
                .packages
                .iter()
                .map(|package| package.name.as_str())
                .collect::<BTreeSet<_>>();
            let mut fan_in = BTreeMap::<&str, BTreeSet<&str>>::new();
            let mut development_fan_in = BTreeMap::<&str, BTreeSet<&str>>::new();

            let mut metrics = self
                .packages
                .iter()
                .map(|package| {
                    let direct = package
                        .dependencies
                        .iter()
                        .filter(|dependency| {
                            dependency.kind != DependencyKind::Development
                                && workspace_names.contains(dependency.name.as_str())
                        })
                        .map(|dependency| dependency.name.as_str())
                        .collect::<BTreeSet<_>>();
                    let development = package
                        .dependencies
                        .iter()
                        .filter(|dependency| {
                            dependency.kind == DependencyKind::Development
                                && workspace_names.contains(dependency.name.as_str())
                        })
                        .map(|dependency| dependency.name.as_str())
                        .collect::<BTreeSet<_>>();
                    for dependency in &direct {
                        fan_in
                            .entry(*dependency)
                            .or_default()
                            .insert(package.name.as_str());
                    }
                    for dependency in &development {
                        development_fan_in
                            .entry(*dependency)
                            .or_default()
                            .insert(package.name.as_str());
                    }
                    PackageMetrics {
                        package: package.name.clone(),
                        manifest_path: package.manifest_path.clone(),
                        workspace_fan_in: 0,
                        workspace_fan_out: direct.len(),
                        development_workspace_fan_in: 0,
                        development_workspace_fan_out: development.len(),
                    }
                })
                .collect::<Vec<_>>();

            for metric in &mut metrics {
                metric.workspace_fan_in =
                    fan_in.get(metric.package.as_str()).map_or(0, BTreeSet::len);
                metric.development_workspace_fan_in = development_fan_in
                    .get(metric.package.as_str())
                    .map_or(0, BTreeSet::len);
            }
            metrics.sort_by(|left, right| {
                left.package
                    .cmp(&right.package)
                    .then_with(|| left.manifest_path.cmp(&right.manifest_path))
            });
            metrics
        }
    }

    pub fn load(root: &Path) -> Result<Workspace> {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
        let output = Command::new(cargo)
            .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
            .arg("--manifest-path")
            .arg(root.join("Cargo.toml"))
            .current_dir(root)
            .output()
            .context("failed to execute cargo metadata")?;
        if !output.status.success() {
            bail!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let source = String::from_utf8(output.stdout).context("cargo metadata was not UTF-8")?;
        parse_metadata(root, &source)
    }

    fn parse_metadata(root: &Path, source: &str) -> Result<Workspace> {
        let metadata: Value =
            serde_json::from_str(source).context("invalid cargo metadata JSON")?;
        let members = required_array(&metadata, "workspace_members")?
            .iter()
            .map(|member| required_string(member, "workspace member"))
            .collect::<Result<BTreeSet<_>>>()?;
        let mut packages = Vec::new();
        for package in required_array(&metadata, "packages")? {
            let id = required_field_string(package, "id")?;
            if members.contains(id.as_str()) {
                packages.push(parse_package(root, package)?);
            }
        }
        packages.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.manifest_path.cmp(&right.manifest_path))
        });
        Ok(Workspace { packages })
    }

    fn parse_package(root: &Path, package: &Value) -> Result<Package> {
        let name = required_field_string(package, "name")?;
        let manifest = canonical_if_present(PathBuf::from(required_field_string(
            package,
            "manifest_path",
        )?));
        let package_root = manifest
            .parent()
            .context("workspace package manifest has no parent")?
            .to_owned();
        let mut dependencies = required_array(package, "dependencies")?
            .iter()
            .map(parse_dependency)
            .collect::<Result<Vec<_>>>()?;
        dependencies.sort();
        let mut targets = required_array(package, "targets")?
            .iter()
            .map(parse_target)
            .collect::<Result<Vec<_>>>()?;
        targets.sort_by(|left, right| {
            left.source_path
                .cmp(&right.source_path)
                .then_with(|| left.kinds.cmp(&right.kinds))
        });
        Ok(Package {
            name,
            manifest_path: normalize_relative(root, &manifest),
            root: package_root,
            dependencies,
            targets,
        })
    }

    fn parse_dependency(dependency: &Value) -> Result<Dependency> {
        let kind = match dependency.get("kind").and_then(Value::as_str) {
            None | Some("normal") => DependencyKind::Normal,
            Some("dev") => DependencyKind::Development,
            Some("build") => DependencyKind::Build,
            Some(other) => bail!("unsupported cargo dependency kind {other}"),
        };
        Ok(Dependency {
            name: required_field_string(dependency, "name")?,
            kind,
            target: dependency
                .get("target")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }

    fn parse_target(target: &Value) -> Result<Target> {
        Ok(Target {
            source_path: canonical_if_present(PathBuf::from(required_field_string(
                target, "src_path",
            )?)),
            kinds: required_array(target, "kind")?
                .iter()
                .map(|kind| required_string(kind, "target kind"))
                .collect::<Result<_>>()?,
        })
    }

    fn required_array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
        value
            .get(field)
            .and_then(Value::as_array)
            .with_context(|| format!("cargo metadata field {field} is not an array"))
    }

    fn required_field_string(value: &Value, field: &str) -> Result<String> {
        required_string(
            value
                .get(field)
                .with_context(|| format!("cargo metadata field {field} is absent"))?,
            field,
        )
    }

    fn required_string(value: &Value, field: &str) -> Result<String> {
        value
            .as_str()
            .map(str::to_owned)
            .with_context(|| format!("cargo metadata field {field} is not a string"))
    }

    fn normalize_relative(root: &Path, path: &Path) -> String {
        path.strip_prefix(root)
            .unwrap_or(path)
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    }

    fn canonical_if_present(path: PathBuf) -> PathBuf {
        path.canonicalize().unwrap_or(path)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use serde_json::json;

        #[test]
        fn metadata_members_are_the_only_package_authority() {
            let root = std::env::temp_dir().join("structure-audit-metadata-root");
            let alpha_manifest = root.join("crates/alpha/Cargo.toml");
            let alpha_source = root.join("crates/alpha/src/lib.rs");
            let beta_manifest = root.join("crates/beta/Cargo.toml");
            let beta_source = root.join("crates/beta/src/lib.rs");
            let historical_manifest = root.join("docs/history/Cargo.toml");
            let historical_source = root.join("docs/history/src/lib.rs");
            let source = json!({
                "workspace_members": ["alpha 0.1.0", "beta 0.1.0"],
                "packages": [
                    package_json("alpha 0.1.0", "alpha", &alpha_manifest, &alpha_source,
                        vec![dependency_json("beta", "normal")]),
                    package_json("beta 0.1.0", "beta", &beta_manifest, &beta_source,
                        vec![dependency_json("alpha", "dev")]),
                    package_json("historical 0.1.0", "historical", &historical_manifest,
                        &historical_source, vec![]),
                ]
            })
            .to_string();

            let workspace = parse_metadata(&root, &source).expect("metadata must parse");
            assert_eq!(
                workspace
                    .packages()
                    .iter()
                    .map(|package| package.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["alpha", "beta"]
            );
            assert_eq!(workspace.owner_for(&historical_source), None);
            assert_eq!(
                workspace.package_metrics(),
                vec![
                    PackageMetrics {
                        package: "alpha".to_owned(),
                        manifest_path: "crates/alpha/Cargo.toml".to_owned(),
                        workspace_fan_in: 0,
                        workspace_fan_out: 1,
                        development_workspace_fan_in: 1,
                        development_workspace_fan_out: 0,
                    },
                    PackageMetrics {
                        package: "beta".to_owned(),
                        manifest_path: "crates/beta/Cargo.toml".to_owned(),
                        workspace_fan_in: 1,
                        workspace_fan_out: 0,
                        development_workspace_fan_in: 0,
                        development_workspace_fan_out: 1,
                    },
                ]
            );
        }

        fn package_json(
            id: &str,
            name: &str,
            manifest: &Path,
            source: &Path,
            dependencies: Vec<Value>,
        ) -> Value {
            json!({
                "id": id,
                "name": name,
                "manifest_path": manifest,
                "dependencies": dependencies,
                "targets": [{ "src_path": source, "kind": ["lib"] }],
            })
        }

        fn dependency_json(name: &str, kind: &str) -> Value {
            json!({ "name": name, "rename": format!("alias_{name}"), "kind": kind, "target": null })
        }
    }
}

mod metrics {
    use crate::workspace::Workspace;
    use anyhow::Result;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct PublicType {
        pub kind: String,
        pub name: String,
        pub line: usize,
    }

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub enum FileClassification {
        Production,
        Test,
        Generated,
        Benchmark,
        Example,
        Tool,
        Facade,
        Text,
    }

    impl FileClassification {
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::Production => "production",
                Self::Test => "test",
                Self::Generated => "generated",
                Self::Benchmark => "benchmark",
                Self::Example => "example",
                Self::Tool => "tool",
                Self::Facade => "facade",
                Self::Text => "text",
            }
        }

        pub const fn receives_production_review(self) -> bool {
            matches!(self, Self::Production | Self::Facade | Self::Tool)
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct FileMetrics {
        pub path: String,
        pub bytes: u64,
        pub physical_lines: usize,
        pub code_lines: usize,
        pub is_rust: bool,
        pub owner: String,
        pub classification: FileClassification,
        pub embedded_test_lines: usize,
        pub public_types: Vec<PublicType>,
    }

    impl FileMetrics {
        pub const fn classification(&self) -> &'static str {
            self.classification.as_str()
        }
    }

    pub fn analyze_files(
        root: &Path,
        files: &[PathBuf],
        generated_paths: &BTreeSet<String>,
        workspace: &Workspace,
    ) -> Result<Vec<FileMetrics>> {
        files
            .iter()
            .filter_map(
                |path| match analyze_file(root, path, generated_paths, workspace) {
                    Ok(Some(metrics)) => Some(Ok(metrics)),
                    Ok(None) => None,
                    Err(error) => Some(Err(error)),
                },
            )
            .collect()
    }

    fn analyze_file(
        root: &Path,
        path: &Path,
        generated_paths: &BTreeSet<String>,
        workspace: &Workspace,
    ) -> Result<Option<FileMetrics>> {
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
        let package_owner = workspace.owner_for(path);
        let owner = package_owner
            .map(str::to_owned)
            .or_else(|| {
                normalized
                    .starts_with("tools/")
                    .then(|| "repository-tools".to_owned())
            })
            .unwrap_or_else(|| "repository".to_owned());
        let lines = content.lines().collect::<Vec<_>>();
        let physical_lines = if content.is_empty() {
            0
        } else {
            content.bytes().filter(|byte| *byte == b'\n').count()
                + usize::from(!content.ends_with('\n'))
        };
        let code_lines = count_code_lines(&lines);
        let is_generated = is_rust && generated_paths.contains(&normalized);
        let target_kinds = workspace.target_kinds_for(path);
        let classification = classify(
            &normalized,
            is_rust,
            is_generated,
            package_owner.is_some(),
            &target_kinds,
        );
        let public_types = if is_rust {
            public_types(&lines)
        } else {
            Vec::new()
        };
        Ok(Some(FileMetrics {
            path: normalized.clone(),
            bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            physical_lines,
            code_lines,
            is_rust,
            owner,
            classification,
            embedded_test_lines: is_rust.then(|| embedded_test_lines(&lines)).unwrap_or(0),
            public_types,
        }))
    }

    fn classify(
        path: &str,
        is_rust: bool,
        is_generated: bool,
        has_package_owner: bool,
        target_kinds: &BTreeSet<String>,
    ) -> FileClassification {
        if !is_rust {
            return FileClassification::Text;
        }
        if is_generated {
            return FileClassification::Generated;
        }
        if path.starts_with("tools/") {
            return FileClassification::Tool;
        }
        if target_kinds.contains("bench") || path.contains("/benches/") {
            return FileClassification::Benchmark;
        }
        if target_kinds.contains("example") || path.contains("/examples/") {
            return FileClassification::Example;
        }
        if target_kinds.contains("test") || is_test_path(path) {
            return FileClassification::Test;
        }
        if is_facade_path(path) {
            return FileClassification::Facade;
        }
        if has_package_owner {
            return FileClassification::Production;
        }
        FileClassification::Tool
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

    fn is_facade_path(path: &str) -> bool {
        path.ends_with("/lib.rs") || path.ends_with("/main.rs")
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

    fn embedded_test_lines(lines: &[&str]) -> usize {
        let mut total = 0;
        let mut index = 0;
        while index < lines.len() {
            if !lines[index].trim().starts_with("#[cfg(test)]") {
                index += 1;
                continue;
            }
            let Some(module_start) = (index..lines.len().min(index + 8)).find(|cursor| {
                let line = lines[*cursor].trim();
                line.contains("mod ") && line.contains('{')
            }) else {
                index += 1;
                continue;
            };
            let mut depth = 0_i64;
            let mut saw_open = false;
            let mut module_end = module_start;
            for (cursor, line) in lines.iter().enumerate().skip(module_start) {
                let opens =
                    i64::try_from(line.chars().filter(|character| *character == '{').count())
                        .unwrap_or(i64::MAX);
                let closes =
                    i64::try_from(line.chars().filter(|character| *character == '}').count())
                        .unwrap_or(i64::MAX);
                saw_open |= opens > 0;
                depth = depth.saturating_add(opens).saturating_sub(closes);
                module_end = cursor;
                if saw_open && depth <= 0 {
                    break;
                }
            }
            total += module_end.saturating_sub(index) + 1;
            index = module_end + 1;
        }
        total
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn examples_benchmarks_tools_and_facades_have_distinct_classifications() {
            assert_eq!(
                classify(
                    "crates/demo/examples/demo.rs",
                    true,
                    false,
                    true,
                    &BTreeSet::new(),
                ),
                FileClassification::Example
            );
            assert_eq!(
                classify(
                    "crates/demo/benches/demo.rs",
                    true,
                    false,
                    true,
                    &BTreeSet::new(),
                ),
                FileClassification::Benchmark
            );
            assert_eq!(
                classify(
                    "tools/structure-audit.rs",
                    true,
                    false,
                    false,
                    &BTreeSet::new(),
                ),
                FileClassification::Tool
            );
            assert_eq!(
                classify(
                    "crates/demo/src/lib.rs",
                    true,
                    false,
                    true,
                    &BTreeSet::new(),
                ),
                FileClassification::Facade
            );
            assert_eq!(
                classify(
                    "crates/demo/src/table.rs",
                    true,
                    true,
                    true,
                    &BTreeSet::new(),
                ),
                FileClassification::Generated
            );
        }

        #[test]
        fn embedded_test_loc_is_measured_without_becoming_a_gate() {
            let lines = [
                "#[cfg(test)]",
                "mod tests {",
                "    #[test]",
                "    fn works() {}",
                "}",
                "fn production() {}",
            ];
            assert_eq!(embedded_test_lines(&lines), 5);
        }
    }
}

mod report {
    use crate::metrics::FileMetrics;
    use crate::rules::Finding;
    use crate::workspace::Workspace;
    use anyhow::Result;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::Path;

    pub fn print_summary(
        files: &[FileMetrics],
        workspace: &Workspace,
        findings: &[Finding],
        blocking_count: usize,
        wrote_to: Option<&Path>,
    ) {
        let rust_files = files.iter().filter(|file| file.is_rust).count();
        let rust_loc = files
            .iter()
            .filter(|file| file.is_rust)
            .map(|file| file.physical_lines)
            .sum::<usize>();
        let review_triggers = findings
            .iter()
            .filter(|finding| !finding.blocks_validation())
            .count();
        println!("files scanned: {}", files.len());
        println!("Rust files: {rust_files}");
        println!("Rust physical LOC: {rust_loc}");
        println!("workspace packages: {}", workspace.packages().len());
        println!("review triggers: {review_triggers}");
        println!("blocking violations: {blocking_count}");
        for finding in findings {
            let location = finding.line().map_or_else(
                || finding.path().to_owned(),
                |line| format!("{}:{line}", finding.path()),
            );
            println!(
                "{} {}: {} — {}",
                finding.kind_label(),
                finding.code(),
                location,
                finding.message()
            );
        }
        match wrote_to {
            Some(path) => println!("reports written to {}", path.display()),
            None => println!("dry-run: no report files written (use --write DIR)"),
        }
    }

    pub fn write_reports(
        directory: &Path,
        files: &[FileMetrics],
        workspace: &Workspace,
        findings: &[Finding],
    ) -> Result<()> {
        fs::create_dir_all(directory)?;
        let obsolete_violations = directory.join("violations.md");
        if obsolete_violations.is_file() {
            fs::remove_file(obsolete_violations)?;
        }
        fs::write(directory.join("file_metrics.csv"), file_metrics_csv(files))?;
        fs::write(
            directory.join("dependency_edges.csv"),
            dependency_edges_csv(workspace),
        )?;
        fs::write(
            directory.join("package_metrics.csv"),
            package_metrics_csv(workspace),
        )?;
        fs::write(
            directory.join("public_type_duplicates.csv"),
            public_type_duplicates_csv(files),
        )?;
        fs::write(directory.join("findings.md"), findings_markdown(findings))?;
        Ok(())
    }

    fn file_metrics_csv(files: &[FileMetrics]) -> String {
        let mut output = String::from(
            "path,owner,bytes,physical_lines,code_lines,classification,is_rust,embedded_test_lines\n",
        );
        for file in files {
            output.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                csv(&file.path),
                csv(&file.owner),
                file.bytes,
                file.physical_lines,
                file.code_lines,
                file.classification(),
                file.is_rust,
                file.embedded_test_lines,
            ));
        }
        output
    }

    fn dependency_edges_csv(workspace: &Workspace) -> String {
        let mut output = String::from("package,dependency,kind,target,manifest\n");
        for package in workspace.packages() {
            for dependency in &package.dependencies {
                output.push_str(&format!(
                    "{},{},{},{},{}\n",
                    csv(&package.name),
                    csv(&dependency.name),
                    dependency.kind.as_str(),
                    csv(dependency.target.as_deref().unwrap_or("")),
                    csv(&package.manifest_path)
                ));
            }
        }
        output
    }

    fn package_metrics_csv(workspace: &Workspace) -> String {
        let mut output = String::from(
            "package,manifest,workspace_fan_in,workspace_fan_out,development_workspace_fan_in,development_workspace_fan_out\n",
        );
        for metric in workspace.package_metrics() {
            output.push_str(&format!(
                "{},{},{},{},{},{}\n",
                csv(&metric.package),
                csv(&metric.manifest_path),
                metric.workspace_fan_in,
                metric.workspace_fan_out,
                metric.development_workspace_fan_in,
                metric.development_workspace_fan_out,
            ));
        }
        output
    }

    fn public_type_duplicates_csv(files: &[FileMetrics]) -> String {
        let mut occurrences = BTreeMap::<String, Vec<(String, String, String, usize)>>::new();
        for file in files {
            for item in &file.public_types {
                occurrences.entry(item.name.clone()).or_default().push((
                    item.kind.clone(),
                    file.owner.clone(),
                    file.path.clone(),
                    item.line,
                ));
            }
        }

        let mut output = String::from("name,kind,path,line,crate_count\n");
        for (name, entries) in occurrences {
            let crates = entries
                .iter()
                .map(|(_, owner, _, _)| owner.clone())
                .collect::<BTreeSet<_>>();
            if crates.len() <= 1 {
                continue;
            }
            for (kind, _, path, line) in entries {
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

    fn findings_markdown(findings: &[Finding]) -> String {
        let mut output = String::from("# Structural findings\n\n");
        if findings.is_empty() {
            output.push_str("No configured findings were found.\n");
            return output;
        }
        for finding in findings {
            let location = finding.line().map_or_else(
                || finding.path().to_owned(),
                |line| format!("{}:{line}", finding.path()),
            );
            output.push_str(&format!(
                "## {} {} — `{}`\n\n{}\n\n**Disposition:** {}\n\n",
                finding.kind_label(),
                finding.code(),
                location,
                finding.message(),
                finding.suggestion(),
            ));
        }
        output.pop();
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

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::rules::{FindingData, ReviewTrigger};

        #[test]
        fn findings_report_names_review_triggers_without_calling_them_errors() {
            let findings = [Finding::ReviewTrigger(ReviewTrigger::new(FindingData {
                code: "SIZE001",
                path: "crates/demo/src/lib.rs".to_owned(),
                line: None,
                message: "review ownership".to_owned(),
                suggestion: "record a disposition".to_owned(),
            }))];
            assert_eq!(
                findings_markdown(&findings),
                concat!(
                    "# Structural findings\n\n",
                    "## review-trigger SIZE001 — `crates/demo/src/lib.rs`\n\n",
                    "review ownership\n\n",
                    "**Disposition:** record a disposition\n",
                )
            );
        }
    }
}

mod rules {
    use crate::metrics::{FileClassification, FileMetrics};
    use crate::workspace::{DependencyKind, Workspace};

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct FindingData {
        pub code: &'static str,
        pub path: String,
        pub line: Option<usize>,
        pub message: String,
        pub suggestion: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct ReviewTrigger(FindingData);

    impl ReviewTrigger {
        pub const fn new(data: FindingData) -> Self {
            Self(data)
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct BlockingViolation(FindingData);

    impl BlockingViolation {
        pub const fn new(data: FindingData) -> Self {
            Self(data)
        }
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum Finding {
        ReviewTrigger(ReviewTrigger),
        BlockingViolation(BlockingViolation),
    }

    impl Finding {
        pub const fn blocks_validation(&self) -> bool {
            matches!(self, Self::BlockingViolation(_))
        }

        pub const fn kind_label(&self) -> &'static str {
            match self {
                Self::ReviewTrigger(_) => "review-trigger",
                Self::BlockingViolation(_) => "blocking-violation",
            }
        }

        pub const fn code(&self) -> &'static str {
            self.data().code
        }

        pub fn path(&self) -> &str {
            &self.data().path
        }

        pub const fn line(&self) -> Option<usize> {
            self.data().line
        }

        pub fn message(&self) -> &str {
            &self.data().message
        }

        pub fn suggestion(&self) -> &str {
            &self.data().suggestion
        }

        const fn data(&self) -> &FindingData {
            match self {
                Self::ReviewTrigger(ReviewTrigger(data))
                | Self::BlockingViolation(BlockingViolation(data)) => data,
            }
        }
    }

    pub fn evaluate(files: &[FileMetrics], workspace: &Workspace) -> Vec<Finding> {
        let mut findings = Vec::new();
        evaluate_file_size(files, &mut findings);
        evaluate_dependencies(workspace, &mut findings);
        findings.sort_by(|left, right| {
            right
                .blocks_validation()
                .cmp(&left.blocks_validation())
                .then_with(|| left.path().cmp(right.path()))
                .then_with(|| left.line().cmp(&right.line()))
                .then_with(|| left.code().cmp(right.code()))
        });
        findings
    }

    fn evaluate_file_size(files: &[FileMetrics], findings: &mut Vec<Finding>) {
        for file in files.iter().filter(|file| file.is_rust) {
            let thresholds = match file.classification {
                FileClassification::Test => Some((2_500, 8_000)),
                classification if classification.receives_production_review() => {
                    Some((1_200, 2_500))
                }
                _ => None,
            };
            if let Some((review, upper)) = thresholds {
                if file.physical_lines > upper {
                    findings.push(review_trigger(
                        "SIZE001",
                        file,
                        format!(
                            "{} physical LOC exceeds the {} LOC upper ownership-review trigger; LOC alone is not a structural error",
                            file.physical_lines, upper
                        ),
                        "record the named owner and cohesive responsibility, then either decompose along state/dependency/test boundaries or add an explicit repository-visible cohesion justification",
                    ));
                } else if file.physical_lines > review {
                    findings.push(review_trigger(
                        "SIZE001",
                        file,
                        format!(
                            "{} physical LOC exceeds the {} LOC ownership-review trigger",
                            file.physical_lines, review
                        ),
                        "name the owner and responsibility, then review state, dependency, API, and test cohesion before adding more code",
                    ));
                }
            }

            if file.classification == FileClassification::Facade && file.physical_lines > 1_000 {
                findings.push(review_trigger(
                    "SIZE002",
                    file,
                    format!(
                        "facade file has {} physical LOC; target is below 250 LOC",
                        file.physical_lines
                    ),
                    "review whether the root owns implementation; if not, move responsibilities to named modules and keep intentional re-exports",
                ));
            }
            if file.classification.receives_production_review()
                && file.physical_lines > 1_200
                && file.embedded_test_lines > 0
            {
                findings.push(review_trigger(
                    "TEST001",
                    file,
                    format!(
                        "large maintained owner contains {} physical LOC of embedded test module source; source layout alone is not a blocking test-coupling finding",
                        file.embedded_test_lines
                    ),
                    "review whether tests follow the same responsibility boundary; move them only when that ownership review identifies real coupling",
                ));
            }
        }
    }

    fn review_trigger(
        code: &'static str,
        file: &FileMetrics,
        message: String,
        suggestion: &str,
    ) -> Finding {
        Finding::ReviewTrigger(ReviewTrigger::new(FindingData {
            code,
            path: file.path.clone(),
            line: None,
            message,
            suggestion: suggestion.to_owned(),
        }))
    }

    fn evaluate_dependencies(workspace: &Workspace, findings: &mut Vec<Finding>) {
        for package in workspace.packages() {
            for dependency in package
                .dependencies
                .iter()
                .filter(|dependency| dependency.kind != DependencyKind::Development)
            {
                if let Some((code, message, suggestion)) =
                    forbidden_dependency(package.name.as_str(), dependency.name.as_str())
                {
                    findings.push(Finding::BlockingViolation(BlockingViolation::new(
                        FindingData {
                            code,
                            path: package.manifest_path.clone(),
                            line: None,
                            message,
                            suggestion,
                        },
                    )));
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
                "arcweft-view",
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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn loc_and_embedded_test_source_are_review_triggers_only() {
            let files = [file(
                "crates/demo/src/lib.rs",
                2_501,
                400,
                FileClassification::Facade,
            )];
            let mut findings = Vec::new();
            evaluate_file_size(&files, &mut findings);
            assert_eq!(
                findings.iter().map(Finding::code).collect::<Vec<_>>(),
                vec!["SIZE001", "SIZE002", "TEST001"]
            );
            assert!(findings.iter().all(|finding| !finding.blocks_validation()));
        }

        #[test]
        fn examples_and_benchmarks_do_not_inherit_production_size_rules() {
            let files = [
                file(
                    "crates/demo/examples/demo.rs",
                    9_000,
                    500,
                    FileClassification::Example,
                ),
                file(
                    "crates/demo/benches/demo.rs",
                    9_000,
                    500,
                    FileClassification::Benchmark,
                ),
            ];
            let mut findings = Vec::new();
            evaluate_file_size(&files, &mut findings);
            assert!(findings.is_empty());
        }

        #[test]
        fn dependency_direction_is_a_blocking_structural_violation() {
            let (code, _, _) = forbidden_dependency("arcweft-core", "arcweft-cli")
                .expect("the typed dependency edge must be rejected");
            assert_eq!(code, "ARCH001");
            let finding = Finding::BlockingViolation(BlockingViolation::new(FindingData {
                code,
                path: "crates/arcweft-core/Cargo.toml".to_owned(),
                line: None,
                message: "invalid edge".to_owned(),
                suggestion: "invert the dependency".to_owned(),
            }));
            assert!(finding.blocks_validation());
        }

        fn file(
            path: &str,
            physical_lines: usize,
            embedded_test_lines: usize,
            classification: FileClassification,
        ) -> FileMetrics {
            FileMetrics {
                path: path.to_owned(),
                bytes: 0,
                physical_lines,
                code_lines: physical_lines,
                is_rust: true,
                owner: "demo".to_owned(),
                classification,
                embedded_test_lines,
                public_types: Vec::new(),
            }
        }
    }
}

mod walk {
    use crate::workspace::Workspace;
    use anyhow::Result;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    const IGNORED_DIRECTORIES: &[&str] = &[
        ".arcweft",
        ".arcweft-local",
        ".git",
        ".idea",
        ".jj",
        ".vscode",
        "node_modules",
        "target",
        "vendor",
    ];

    pub fn collect_files(root: &Path, workspace: &Workspace) -> Result<Vec<PathBuf>> {
        let mut files = BTreeSet::new();
        for package_root in workspace.package_roots() {
            visit(&package_root, &mut files)?;
        }
        let tools = root.join("tools");
        if tools.is_dir() {
            visit(&tools, &mut files)?;
        }
        Ok(files.into_iter().collect())
    }

    fn visit(path: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
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
                files.insert(path);
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
