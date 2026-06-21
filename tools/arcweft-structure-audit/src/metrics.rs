use std::fs;
use std::io;
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

pub fn analyze_files(root: &Path, files: &[PathBuf]) -> io::Result<Vec<FileMetrics>> {
    files
        .iter()
        .filter_map(|path| match analyze_file(root, path) {
            Ok(Some(metrics)) => Some(Ok(metrics)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

fn analyze_file(root: &Path, path: &Path) -> io::Result<Option<FileMetrics>> {
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
    } else if let Some(rest) = trimmed.strip_prefix("pub(") {
        let (_, rest) = rest.split_once(')')?;
        rest.trim_start()
    } else {
        return None;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_code_while_ignoring_comment_only_lines() {
        let lines = [
            "// comment",
            "",
            "pub struct Item;",
            "/* block",
            "still block */",
            "fn run() {} // trailing",
        ];
        assert_eq!(count_code_lines(&lines), 2);
    }

    #[test]
    fn extracts_restricted_public_types() {
        let item = parse_public_type("pub(crate) struct RuntimePlan {", 9).expect("type");
        assert_eq!(item.kind, "struct");
        assert_eq!(item.name, "RuntimePlan");
        assert_eq!(item.line, 9);
    }

    #[test]
    fn embedded_test_module_ignores_external_test_module_declaration() {
        assert!(!has_embedded_test_module(&["#[cfg(test)]", "mod tests;"]));
        assert!(has_embedded_test_module(&[
            "#[cfg(test)]",
            "mod tests {",
            "}"
        ]));
    }

    #[test]
    fn detects_generated_source_headers() {
        assert!(is_generated_source(&[
            "// This file is generated from Unicode data.",
            "// Do not edit by hand.",
        ]));
        assert!(!is_generated_source(&["// normal hand-written module"]));
    }
}
