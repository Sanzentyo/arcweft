use crate::cargo_manifest::Manifest;
use crate::metrics::FileMetrics;
use crate::rules::{Severity, Violation};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
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
) -> io::Result<()> {
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
            violation.severity, violation.code, location, violation.message, violation.suggestion
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quotes_delimiters_and_quotes() {
        assert_eq!(csv("a,b\"c"), "\"a,b\"\"c\"");
    }
}
