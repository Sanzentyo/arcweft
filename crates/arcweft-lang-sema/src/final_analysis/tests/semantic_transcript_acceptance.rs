use super::*;

#[path = "semantic_transcript_acceptance/body_roots.rs"]
mod body_roots;
#[path = "semantic_transcript_acceptance/expression_corpus.rs"]
mod expression_corpus;
#[path = "semantic_transcript_acceptance/expression_inputs.rs"]
mod expression_inputs;
#[path = "semantic_transcript_acceptance/expression_shapes.rs"]
mod expression_shapes;
#[path = "semantic_transcript_acceptance/generation_invariance.rs"]
mod generation_invariance;
#[path = "semantic_transcript_acceptance/patterns.rs"]
mod patterns;
#[path = "semantic_transcript_acceptance/statements.rs"]
mod statements;
#[path = "semantic_transcript_acceptance/view_calls.rs"]
mod view_calls;
#[path = "semantic_transcript_acceptance/view_defaults.rs"]
mod view_defaults;
#[path = "semantic_transcript_acceptance/view_roots.rs"]
mod view_roots;

#[derive(Clone, Copy)]
struct MatchObservation {
    owner: arcweft_lang_hir::identity::ExprId,
    source_start: usize,
    source_end: usize,
    digest: [u8; 32],
}

fn match_observations(source: &str) -> Vec<MatchObservation> {
    let world = super::fixture(source, None);
    match_observations_for_fixture(&world)
}

fn match_observations_for_fixture(world: &super::Fixture) -> Vec<MatchObservation> {
    let report = super::analyze(world)
        .unwrap_or_else(|error| panic!("semantic transcript fixture should check: {error:?}"));
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let source_identity = module.provenance().source_identity();

    module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .map(|owner| {
            let source = module
                .source_site(
                    source_identity,
                    HirSourceQuery::Expr {
                        owner,
                        role: HirExprSourceRole::Whole,
                    },
                )
                .expect("Match source site");
            let HirSourcePresence::Present(HirSourceSite::Span(span)) = source.presence() else {
                panic!("Match expression retains its authored span");
            };
            let product =
                super::checked_match_product(&report, project, module, &world.symbols, owner);
            MatchObservation {
                owner,
                source_start: span.range().start(),
                source_end: span.range().end(),
                digest: *product.semantic_digest().as_bytes(),
            }
        })
        .collect()
}

fn outermost(observations: &[MatchObservation]) -> MatchObservation {
    *observations
        .iter()
        .max_by_key(|observation| observation.source_end - observation.source_start)
        .expect("at least one Match expression")
}

fn source_match_digest(source: &str) -> [u8; 32] {
    outermost(&match_observations(source)).digest
}

fn match_arm_let_source(value: &str) -> String {
    format!(
        r"
fn root(flag: bool) -> i64 {{
    match flag {{
        true => {{
            let value = {value}
            value
        }}
        false => 0i64
    }}
}}
"
    )
}

fn nested_if_statement_source(body: &str) -> String {
    format!(
        "fn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {{\n            if true {{\n{body}\n            }}\n            0i64\n        }}\n        false => 0i64\n    }}\n}}\n"
    )
}

fn bool_match_i64_source(expression: &str) -> String {
    format!(
        "fn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {expression}\n        false => 0i64\n    }}\n}}\n"
    )
}

fn numeric_sequence_match_source(expression: &str) -> String {
    format!(
        "fn root(flag: bool) -> Vec<i64> {{\n    match flag {{\n        true => {expression}\n        false => [0i64]\n    }}\n}}\n"
    )
}

fn callable_value_match_source(copy: &str) -> String {
    format!(
        "fn identity(value: i64) -> i64 {{ value }}\nfn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {{ let saved = identity; let copy = {copy}; 1i64 }}\n        false => 0i64\n    }}\n}}\n"
    )
}

fn field_match_source(member: &str) -> String {
    format!(
        "struct Pair {{ left: i64, right: i64 }}\nfn root(pair: Pair, flag: bool) -> i64 {{\n    match flag {{\n        true => pair.{member}\n        false => 0i64\n    }}\n}}\n"
    )
}

fn method_match_source(prefix: &str, call: &str) -> String {
    format!(
        "{prefix}view Main(dialogue: DialogueView, speed: f32) {{\n    match true {{\n        true => {call}\n        false => Button()\n    }}\n}}\n"
    )
}

fn try_match_source(first: &str) -> String {
    format!(
        "fn root(first: Result<i64, String>, second: Result<i64, String>, flag: bool) -> Result<i64, String> {{\n    result {{\n        match flag {{\n            true => try {first}\n            false => try second\n        }}\n    }}\n}}\n"
    )
}

fn pipe_match_source(left: &str) -> String {
    format!(
        "fn identity(value: i32) -> i32 {{ value }}\nfn shifted(value: i32) -> i32 {{ value + 1i32 }}\nfn apply(callback: i32 -> i32, value: i32) -> i32 {{ callback(value) }}\nfn root(flag: bool) -> (i32, i32) {{\n    match flag {{\n        true => {left} |> (apply(^, 1i32), apply(^, 2i32))\n        false => (0i32, 0i32)\n    }}\n}}\n"
    )
}

fn choice_match_source(prefix: &str, plan: &str) -> String {
    format!(
        "{prefix}flow done() -> String {{ return \"done\" }}\nflow main() {{\n    let selected = match true {{\n        true => choice @choice.main {{ @.go \"Go\" -> @flow.done }} {plan}\n        false => ()\n    }}\n}}\n"
    )
}
