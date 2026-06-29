#!/usr/bin/env -S cargo +nightly -Zscript
---
[package]
edition = "2024"
---

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const ARCWEFT_FENCES: &[&str] = &["arcw", "awfagent"];
const ARCWEFT_EXTENSIONS: &[&str] = &["arcw", "awfagent"];
#[derive(Clone, Debug)]
struct Finding {
    path: PathBuf,
    line: usize,
    message: &'static str,
    text: String,
}

fn main() {
    let root = env::args().nth(1).unwrap_or_else(|| ".".to_owned());
    let root = PathBuf::from(root);
    let mut findings = Vec::new();
    for relative in [
        Path::new("docs/00-overview"),
        Path::new("docs/01-language"),
        Path::new("docs/examples"),
        Path::new("samples"),
    ] {
        scan_dir(&root.join(relative), &mut findings);
    }

    if findings.is_empty() {
        println!("arcw comment style audit: ok");
        return;
    }

    for finding in &findings {
        println!(
            "{}:{}: {}: {}",
            finding.path.display(),
            finding.line,
            finding.message,
            finding.text.trim_end()
        );
    }
    eprintln!(
        "arcw comment style audit: found {} comment-style `#` use(s)",
        findings.len()
    );
    std::process::exit(1);
}

fn scan_dir(path: &Path, findings: &mut Vec<Finding>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
        if file_name == ".git" || file_name == ".jj" || file_name == "target" {
            continue;
        }
        if path.is_dir() {
            scan_dir(&path, findings);
        } else {
            scan_file(&path, findings);
        }
    }
}

fn scan_file(path: &Path, findings: &mut Vec<Finding>) {
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return;
    };
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };

    if ARCWEFT_EXTENSIONS.contains(&extension) {
        scan_arcweft_source(path, &source, findings);
    } else if extension == "md" {
        scan_markdown_fences(path, &source, findings);
    } else if extension == "txt" && is_agent_repl_transcript(path) {
        scan_repl_transcript(path, &source, findings);
    }
}

fn scan_markdown_fences(path: &Path, source: &str, findings: &mut Vec<Finding>) {
    let mut active_fence = None;
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("```") {
            let language = rest
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(',');
            active_fence = match active_fence {
                Some(_) => None,
                None if ARCWEFT_FENCES.contains(&language) => Some(language),
                None => None,
            };
            continue;
        }
        if active_fence.is_some() {
            push_comment_style_hash(path, index + 1, line, findings);
        }
    }
}

fn scan_arcweft_source(path: &Path, source: &str, findings: &mut Vec<Finding>) {
    for (index, line) in source.lines().enumerate() {
        push_comment_style_hash(path, index + 1, line, findings);
    }
}

fn scan_repl_transcript(path: &Path, source: &str, findings: &mut Vec<Finding>) {
    for (index, line) in source.lines().enumerate() {
        if line.trim_start().starts_with("# ") {
            findings.push(Finding {
                path: path.to_owned(),
                line: index + 1,
                message: "Agent REPL transcripts use `//` comments for Arcweft sample consistency",
                text: line.to_owned(),
            });
        }
    }
}

fn push_comment_style_hash(
    path: &Path,
    line_number: usize,
    line: &str,
    findings: &mut Vec<Finding>,
) {
    let trimmed = line.trim_start();
    if trimmed.starts_with("# ") || inline_hash_comment(line) {
        findings.push(Finding {
            path: path.to_owned(),
            line: line_number,
            message: "use `//` for Arcweft comments; `#` is reserved for attributes/interpolation",
            text: line.to_owned(),
        });
    }
}

fn inline_hash_comment(line: &str) -> bool {
    let mut previous = None;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '#'
            && previous.is_some_and(char::is_whitespace)
            && characters.peek().is_some_and(|next| next.is_whitespace())
        {
            return true;
        }
        previous = Some(character);
    }
    false
}

fn is_agent_repl_transcript(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    file_name.starts_with("repl-")
        && file_name.ends_with(".txt")
        && path
            .components()
            .any(|component| component.as_os_str() == "agent-script")
}
