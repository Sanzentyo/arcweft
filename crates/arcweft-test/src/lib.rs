//! Sans I/O script test and bench planning.
//!
//! The crate extracts `test` and `bench` declarations from HIR into a stable
//! manifest that CLI, LSP, and future runtime adapters can consume. It does not
//! open files, drive a renderer, sleep, or run benchmark timers.

use arcweft_lang_hir::model::{HirModule, HirTopLevelDecl};
use arcweft_lang_syntax::ast::{
    common::TextRange,
    ids::IdRef,
    proof::{BenchItem, TestItem},
};
use serde::{Deserialize, Serialize};

pub mod agent;

/// Tool-facing manifest of script-level tests and benches.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScriptTestManifest {
    pub tests: Vec<ScriptTest>,
    pub benches: Vec<ScriptBench>,
}

/// One top-level `test @test.id kind { ... }` declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScriptTest {
    pub id: String,
    pub kind: String,
    pub steps: Vec<ScriptStep>,
    pub source: ManifestSpan,
}

/// One top-level `bench @bench.id { ... }` declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScriptBench {
    pub id: String,
    pub sections: Vec<BenchSection>,
    pub source: ManifestSpan,
}

/// A call-based row inside a script test body.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScriptStep {
    pub command: String,
    pub text: String,
}

/// A top-level section inside a bench body, such as `setup`, `measure`, or `report`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchSection {
    pub name: String,
    pub text: String,
}

/// Stable byte span copied from syntax ranges.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManifestSpan {
    pub start: usize,
    pub end: usize,
}

/// Extracts script tests and benches without executing them.
#[must_use]
pub fn collect_script_tests(module: &HirModule) -> ScriptTestManifest {
    let mut manifest = ScriptTestManifest::default();
    for declaration in module.declarations() {
        match declaration {
            HirTopLevelDecl::Test(item) => manifest.tests.push(script_test(item)),
            HirTopLevelDecl::Bench(item) => manifest.benches.push(script_bench(item)),
            _ => {}
        }
    }
    manifest
}

fn script_test(item: &TestItem) -> ScriptTest {
    ScriptTest {
        id: id_ref_label(item.id(), "test"),
        kind: item.kind().as_str().to_owned(),
        steps: command_rows(item.body()),
        source: span(item.range()),
    }
}

fn script_bench(item: &BenchItem) -> ScriptBench {
    ScriptBench {
        id: id_ref_label(item.id(), "bench"),
        sections: command_rows(item.body())
            .into_iter()
            .map(|step| BenchSection {
                name: step.command,
                text: step.text,
            })
            .collect(),
        source: span(item.range()),
    }
}

fn command_rows(body: &str) -> Vec<ScriptStep> {
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .map(|line| {
            let command = line
                .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '{' | '('))
                .next()
                .unwrap_or(line)
                .to_owned();
            ScriptStep {
                command,
                text: line.to_owned(),
            }
        })
        .collect()
}

fn id_ref_label(id: &IdRef, default_family: &str) -> String {
    match id {
        IdRef::Absolute(entity) => entity.body().to_owned(),
        IdRef::Relative(relative) => format!("{default_family}.{}", relative.suffix()),
        IdRef::FamilyRelative(relative) => {
            format!("{}.{}", relative.family(), relative.relative().suffix())
        }
    }
}

fn span(range: &TextRange) -> ManifestSpan {
    ManifestSpan {
        start: range.start(),
        end: range.end(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_lang_hir::lower::lower_document_to_hir;
    use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use std::sync::Arc;

    #[test]
    fn collects_script_test_and_bench_manifest() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://arcweft-test/script-manifest.arcw")
                    .expect("script manifest fixture source ID"),
                SourceName::path("arcweft-test/script-manifest.arcw"),
                r#"
test @test.opening scenario {
    goto @flow.opening
    expect.no_assertion_failures()
}

bench @bench.opening {
    setup { let state = fixture<GameState>("opening.json") }
    measure iterations = 10 { opening_choices() }
    report { cpu_time }
}
"#,
            )
            .expect("script manifest fixture source document"),
        );
        let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
        assert!(parsed.errors().is_empty());
        let hir =
            lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("HIR lowers");
        let manifest = collect_script_tests(&hir);

        assert_eq!(manifest.tests[0].id, "test.opening");
        assert_eq!(manifest.tests[0].kind, "scenario");
        assert_eq!(manifest.tests[0].steps.len(), 2);
        assert_eq!(manifest.benches[0].id, "bench.opening");
        assert_eq!(manifest.benches[0].sections[1].name, "measure");
    }
}
