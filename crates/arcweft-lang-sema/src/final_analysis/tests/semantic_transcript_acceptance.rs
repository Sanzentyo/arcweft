use super::*;

#[derive(Clone, Copy)]
struct MatchObservation {
    owner: arcweft_lang_hir::identity::ExprId,
    source_start: usize,
    source_end: usize,
    digest: [u8; 32],
}

fn match_observations(source: &str) -> Vec<MatchObservation> {
    let world = super::fixture(source, None);
    let report = super::analyze(&world)
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

#[test]
fn checked_match_transcript_ignores_raw_ids_spans_and_formatting_in_closure_body() {
    let compact = match_observations(
        r"
fn root(flag: bool) -> i64 {
    let callback = || {
        match flag {
            true => 1i64
            false => 2i64
        }
    }
    callback()
}
",
    );
    let reformatted = match_observations(
        r"
fn unrelated() -> i64 { 99i64 }

fn root ( flag : bool ) -> i64 {
    let callback = || {
        // Formatting moves the Match span; the preceding declaration shifts
        // arena allocation without changing this closure body's children.
        match flag {
            true => 0x1_i64
            false => 0x2_i64
        }
    }
    callback()
}
",
    );

    let first = outermost(&compact);
    let second = outermost(&reformatted);
    assert_ne!(
        first.owner, second.owner,
        "the fixture perturbs the raw HIR ID"
    );
    assert_ne!(first.source_start, second.source_start);
    assert_ne!(first.source_end, second.source_end);
    assert_eq!(first.digest, second.digest);
}

#[test]
fn nested_match_semantic_change_reaches_the_outer_match_digest() {
    let original = match_observations(
        r"
fn root(outer: bool, inner: bool) -> i64 {
    match outer {
        true => match inner {
            true => 1i64
            false => 2i64
        }
        false => 0i64
    }
}
",
    );
    let changed = match_observations(
        r"
fn root(outer: bool, inner: bool) -> i64 {
    match outer {
        true => match inner {
            true => 9i64
            false => 2i64
        }
        false => 0i64
    }
}
",
    );

    assert_eq!(original.len(), 2, "outer and nested Match roots");
    assert_eq!(changed.len(), 2, "outer and nested Match roots");
    let original_outer = outermost(&original);
    let changed_outer = outermost(&changed);
    assert_ne!(original_outer.digest, changed_outer.digest);
    let original_inner = original
        .iter()
        .find(|observation| observation.owner != original_outer.owner)
        .expect("nested Match");
    let changed_inner = changed
        .iter()
        .find(|observation| observation.owner != changed_outer.owner)
        .expect("nested Match");
    assert_ne!(original_inner.digest, changed_inner.digest);
}

#[test]
fn checked_match_transcript_commits_arm_block_body_meaning() {
    let original = match_observations(
        r"
fn root(flag: bool) -> i64 {
    match flag {
        true => {
            let value = 1i64
            value
        }
        false => 0i64
    }
}
",
    );
    let changed = match_observations(
        r"
fn root(flag: bool) -> i64 {
    match flag {
        true => {
            let value = 2i64
            value
        }
        false => 0i64
    }
}
",
    );

    assert_eq!(original.len(), 1);
    assert_eq!(changed.len(), 1);
    assert_ne!(outermost(&original).digest, outermost(&changed).digest);
}

#[test]
fn checked_match_transcript_commits_nested_statement_body_meaning() {
    let source = |body: &str| {
        format!(
            "fn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {{\n            if true {{\n{body}\n            }}\n            0i64\n        }}\n        false => 0i64\n    }}\n}}\n"
        )
    };
    let digest = |body| outermost(&match_observations(&source(body))).digest;

    let first = digest("                let first = 1i64");
    let changed_value = digest("                let first = 2i64");
    let empty = digest("");
    let ordered = digest("                let first = 1i64\n                let second = 2i64");
    let reversed = digest("                let second = 2i64\n                let first = 1i64");

    assert_ne!(first, changed_value, "nested statement body value");
    assert_ne!(first, empty, "nonempty versus empty statement body");
    assert_ne!(ordered, reversed, "nested statement body order");
}

fn source_match_digest(source: &str) -> [u8; 32] {
    outermost(&match_observations(source)).digest
}

fn first_arm_pattern_digest(source: &str) -> [u8; 32] {
    let world = super::fixture(source, None);
    let report = super::analyze(&world).expect("pattern transcript fixture should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match expression");
    let product = super::checked_match_product(&report, project, module, &world.symbols, owner);
    *product.arms()[0].pattern().as_bytes()
}

#[test]
fn checked_match_transcript_commits_binary_operator_and_range_inclusivity() {
    let result = |expression: &str| {
        source_match_digest(&format!(
            "fn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {expression}\n        false => 0i64\n    }}\n}}\n"
        ))
    };
    assert_ne!(result("1i64 + 2i64"), result("1i64 - 2i64"));

    let range = |expression: &str| {
        source_match_digest(&format!(
            "fn root(flag: bool) {{\n    let selected = match flag {{\n        true => {expression}\n        false => 0i64..1i64\n    }}\n}}\n"
        ))
    };
    assert_ne!(range("0i64..1i64"), range("0i64..=1i64"));
}

#[test]
fn checked_match_transcript_commits_compact_numeric_values_but_not_radix() {
    let result = |expression: &str| {
        source_match_digest(&format!(
            "fn root(flag: bool) -> Vec<i64> {{\n    match flag {{\n        true => {expression}\n        false => [0i64]\n    }}\n}}\n"
        ))
    };
    assert_ne!(result("[1i64, 2i64]"), result("[1i64, 3i64]"));
    assert_eq!(result("[1i64, 2i64]"), result("[0x1_i64, 0b10_i64]"));
}

#[test]
fn checked_match_pattern_transcript_distinguishes_exact_and_rest_sequences() {
    let source = |pattern: &str| {
        format!(
            "fn root(values: Vec<bool>) -> i64 {{\n    match values {{\n        {pattern} => 1i64\n        _ => 0i64\n    }}\n}}\n"
        )
    };
    assert_ne!(
        first_arm_pattern_digest(&source("[true]")),
        first_arm_pattern_digest(&source("[true, ..]")),
    );
    assert_ne!(
        first_arm_pattern_digest(&source("[]")),
        first_arm_pattern_digest(&source("[..]")),
    );
}
