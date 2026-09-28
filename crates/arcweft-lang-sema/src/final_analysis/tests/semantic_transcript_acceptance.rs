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

fn assert_match_sensitivity_and_source_revision_invariance(
    root: &str,
    original: &str,
    changed: &str,
    revised: &str,
) {
    let original = outermost(&match_observations(original));
    let changed = outermost(&match_observations(changed));
    let revised = outermost(&match_observations(revised));

    assert_ne!(
        original.digest, changed.digest,
        "semantic change in {root} must reach the Match transcript",
    );
    assert_ne!(
        original.owner, revised.owner,
        "source revision in {root} must perturb the raw Match ID",
    );
    assert_ne!(
        original.source_start, revised.source_start,
        "source revision in {root} must move the Match span",
    );
    assert_ne!(
        original.source_end, revised.source_end,
        "source revision in {root} must move the Match span",
    );
    assert_eq!(
        original.digest, revised.digest,
        "source revision in {root} must preserve checked meaning",
    );
}

#[test]
fn checked_match_transcript_commits_predicate_body_and_ignores_source_revision() {
    let original = r"
predicate guarded(value: bool) = match value {
    true => true
    false => false
}
";
    let changed = r"
predicate guarded(value: bool) = match value {
    true => false
    false => false
}
";
    let revised = r"
fn unrelated() -> i64 { 99i64 }
predicate guarded ( value : bool ) = match value {
true=>true
false=>false
}
";

    assert_match_sensitivity_and_source_revision_invariance(
        "predicate body",
        original,
        changed,
        revised,
    );
}

#[test]
fn checked_match_transcript_commits_proof_body_and_ignores_source_revision() {
    let original = r"
proof row() = match true {
        true => { let value = 1i64; value; () }
        false => ()
    }
";
    let changed = original.replace("value = 1i64", "value = 2i64");
    let revised = r"
fn unrelated() -> i64 { 99i64 }
proof row( ) = match true { true=>{let value=0x1_i64; value; ()} false=>() }
";

    assert_match_sensitivity_and_source_revision_invariance(
        "proof body",
        original,
        &changed,
        revised,
    );
}

#[test]
fn checked_match_transcript_commits_flow_body_and_ignores_source_revision() {
    let original = r"
flow row(flag: bool) {
    let selected = match flag {
        true => 1i64
        false => 0i64
    }
    return ()
}
";
    let changed = original.replace("true => 1i64", "true => 2i64");
    let revised = r"
fn unrelated() -> i64 { 99i64 }
flow row(flag: bool) {
    let selected = match flag {
        true => 0x1_i64
        false => 0i64
    }
    return ()
}
";

    assert_match_sensitivity_and_source_revision_invariance(
        "Flow body",
        original,
        &changed,
        revised,
    );
}

#[test]
fn checked_match_transcript_commits_selected_call_argument_passing() {
    let source = |call: &str| {
        format!(
            "fn reorder(first: i64, second: i64) -> i64 {{ first + second }}\nfn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {call}\n        false => 0i64\n    }}\n}}\n"
        )
    };
    let positional = source_match_digest(&source("reorder(1i64, 2i64)"));
    let named = source_match_digest(&source("reorder(first=1i64, second=2i64)"));
    let reformatted = source_match_digest(&source("reorder( first = 1i64, second = 2i64 )"));
    let radix = source_match_digest(&source("reorder(first=0x1_i64, second=0b10_i64)"));
    let selected_join = |call: &str| {
        let world = super::fixture(&source(call), None);
        let report = super::analyze(&world).expect("checked call");
        let (owner, _) = report.calls().next().expect("one call");
        let join = report
            .edge_facts
            .get(&owner)
            .expect("call edges")
            .as_ref()
            .expect("checked call edges")
            .callable()
            .expect("selected callable join");
        let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
            report.accepted_root_catalog(),
            &report,
        );
        *join
            .stable_transcript_digest(&coordinates)
            .expect("accepted callable transcript identity")
            .as_bytes()
    };

    assert_eq!(
        selected_join("reorder(1i64, 2i64)"),
        selected_join("reorder(first=1i64, second=2i64)"),
        "selected callable and argument slots are unchanged",
    );
    assert_ne!(positional, named, "selected source argument passing");
    assert_eq!(
        named, reformatted,
        "spelling and layout do not alter passing"
    );
    assert_eq!(named, radix, "integer radix does not alter checked values");
}

#[test]
fn checked_match_transcript_commits_explicit_call_type_application() {
    let source = |call: &str| {
        format!(
            "fn identity<T>(value: T) -> T {{ value }}\nfn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {call}\n        false => 0i64\n    }}\n}}\n"
        )
    };
    let digest = |call: &str| source_match_digest(&source(call));
    let selected_join = |call: &str| {
        let world = super::fixture(&source(call), None);
        let report = super::analyze(&world).expect("checked generic call");
        let (owner, _) = report.calls().next().expect("one call");
        let join = report
            .edge_facts
            .get(&owner)
            .expect("call edges")
            .as_ref()
            .expect("checked call edges")
            .callable()
            .expect("selected callable join");
        let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
            report.accepted_root_catalog(),
            &report,
        );
        *join
            .stable_transcript_digest(&coordinates)
            .expect("selected generic instantiation")
            .as_bytes()
    };

    let inferred = "identity(1i64)";
    let explicit = "identity::<i64>(1i64)";
    let formatted = "identity::<i64>( 1i64 )";
    assert_eq!(
        selected_join(inferred),
        selected_join(explicit),
        "explicit arguments retain the selected target and instantiation",
    );
    assert_ne!(
        digest(inferred),
        digest(explicit),
        "explicitness is an atom"
    );
    assert_eq!(digest(explicit), digest(formatted), "layout is omitted");

    let method_source = |call: &str| {
        format!(
            "fn root(items: Seq<i64>, flag: bool) -> Vec<i64> {{\n    match flag {{\n        true => {call}\n        false => [0i64]\n    }}\n}}\n"
        )
    };
    let _direct_method = source_match_digest(&method_source("items.collect<Vec<i64>>()"));

    for (label, call) in [
        ("missing type argument", "identity::<>()"),
        ("invalid type argument", "identity::<9bad>(1i64)"),
        ("missing type close", "identity::<i64(1i64)"),
    ] {
        let invalid = super::fixture(&source(call), None);
        assert!(
            invalid.project.analysis_view().is_err(),
            "{label} rejects before final semantic analysis",
        );
    }
    let mismatched = super::fixture(&source("identity::<bool>(1i64)"), None);
    assert!(
        super::analyze(&mismatched).is_err(),
        "mismatched checked type rejects",
    );
}

#[test]
fn checked_match_transcript_uses_accepted_project_callable_value_identity() {
    let original = source_match_digest(
        "fn identity(value: i64) -> i64 { value }\nfn root(flag: bool) -> i64 {\n    match flag {\n        true => { let saved = identity; 1i64 }\n        false => 0i64\n    }\n}\n",
    );
    let reformatted = source_match_digest(
        "fn unrelated() -> i64 { 99i64 }\nfn identity ( value : i64 ) -> i64 { value }\nfn root(flag: bool) -> i64 {\n    match flag {\n        true => { let saved = identity; 1i64 }\n        false => 0i64\n    }\n}\n",
    );
    assert_eq!(original, reformatted);
}

#[test]
fn checked_match_transcript_uses_closed_value_and_select_families() {
    let value_source = |copy: &str| {
        format!(
            "fn identity(value: i64) -> i64 {{ value }}\nfn root(flag: bool) -> i64 {{\n    match flag {{\n        true => {{ let saved = identity; let copy = {copy}; 1i64 }}\n        false => 0i64\n    }}\n}}\n"
        )
    };
    let project = value_source("identity");
    let local = value_source("saved");
    let world = super::fixture(&local, None);
    let report = super::analyze(&world).expect("checked local and project callable values");
    let value_tags = report.expressions().filter_map(|(_, expression)| {
        let CheckedExpressionResolution::Value(value) = expression.resolution() else {
            return None;
        };
        Some(value.semantic_transcript_tag())
    });
    let observed = value_tags.collect::<BTreeSet<_>>();
    assert!(observed.contains(&0x0300), "checked local value");
    assert!(observed.contains(&0x0303), "checked project callable value");
    assert_ne!(
        source_match_digest(&project),
        source_match_digest(&local),
        "a direct callable and an accepted local binding are different Value meanings",
    );

    let field_source = |member: &str| {
        format!(
            "struct Pair {{ left: i64, right: i64 }}\nfn root(pair: Pair, flag: bool) -> i64 {{\n    match flag {{\n        true => pair.{member}\n        false => 0i64\n    }}\n}}\n"
        )
    };
    let left = field_source("left");
    let world = super::fixture(&left, None);
    let report = super::analyze(&world).expect("checked field selection");
    assert!(report.expressions().any(|(_, expression)| matches!(
        expression.resolution(),
        CheckedExpressionResolution::Select(select @ CheckedSelectResolution::Field(_))
            if select.semantic_transcript_tag() == 0x0404
    )));
    assert_ne!(
        source_match_digest(&left),
        source_match_digest(&field_source("right")),
        "different accepted fields reach the Match digest",
    );
}

#[test]
fn checked_match_transcript_uses_structural_method_identity() {
    struct MethodObservation {
        match_digest: [u8; 32],
        runtime_callable: [u8; 32],
        transcript_callable: [u8; 32],
    }
    let source = |prefix: &str, call: &str| {
        format!(
            "{prefix}view Main(dialogue: DialogueView, speed: f32) {{\n    match true {{\n        true => {call}\n        false => Button()\n    }}\n}}\n"
        )
    };
    let observe = |prefix: &str, call: &str| {
        let world = super::fixture(&source(prefix, call), None);
        let report = super::analyze(&world).expect("checked method selection");
        let project = world.project.analysis_view().expect("executable HIR");
        let module = project
            .module(&CanonicalModulePath::crate_root())
            .expect("root HIR module");
        let method = module
            .expressions()
            .find_map(|(owner, _)| match report.expression(owner)?.resolution() {
                CheckedExpressionResolution::Select(
                    select @ CheckedSelectResolution::Method(method),
                ) => {
                    assert_eq!(select.semantic_transcript_tag(), 0x0400);
                    Some(method)
                }
                _ => None,
            })
            .expect("Match arm has a checked method selection");
        let match_owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
            })
            .expect("Match expression");
        let product =
            super::checked_match_product(&report, project, module, &world.symbols, match_owner);
        MethodObservation {
            match_digest: *product.semantic_digest().as_bytes(),
            runtime_callable: *method.callable().as_bytes(),
            transcript_callable: *method.transcript_callable().as_bytes(),
        }
    };
    let first = observe("", "Button().on_click { dialogue.primary_action }");
    let whitespace = observe("", "Button().on_click {  dialogue.primary_action  }");
    let unrelated = observe(
        "fn unrelated() -> i64 { 99i64 }\n",
        "Button().on_click { dialogue.primary_action }",
    );
    let second = observe("", "Button().fx(wave(speed = speed))");

    assert_ne!(first.runtime_callable, whitespace.runtime_callable);
    assert_ne!(first.runtime_callable, unrelated.runtime_callable);
    assert_eq!(first.transcript_callable, whitespace.transcript_callable);
    assert_eq!(first.transcript_callable, unrelated.transcript_callable);
    assert_eq!(first.match_digest, whitespace.match_digest);
    assert_eq!(first.match_digest, unrelated.match_digest);
    assert_ne!(first.transcript_callable, second.transcript_callable);
    assert_ne!(first.match_digest, second.match_digest);
}

#[test]
fn checked_match_transcript_commits_compact_choice_plan_rows() {
    let source = |prefix: &str, plan: &str| {
        format!(
            "{prefix}flow done() -> String {{ return \"done\" }}\nflow main() {{\n    let selected = match true {{\n        true => choice @choice.main {{ @.go \"Go\" -> @flow.done }} {plan}\n        false => ()\n    }}\n}}\n"
        )
    };
    let digest = |plan: &str| source_match_digest(&source("", plan));
    let absent = digest("");
    let empty = digest("with {}");
    let window = digest("with { window = true }");
    let layout = digest("with { layout = true }");
    let input = digest("with { cancel on input(_) {} }");
    let task = digest("with { cancel on task(_) {} }");
    let expression = digest("with { cancel on true {} }");
    let timeout = digest("with { cancel on timeout(1s) {} }");
    let signal = source_match_digest(&source(
        "signal ready: bool\n",
        "with { cancel on signal(@signal.ready, value) { let observed = value } }",
    ));
    let formatted = source_match_digest(&source(
        "fn unrelated() -> i64 { 99i64 }\n",
        "with {  window = true  }",
    ));

    assert_ne!(absent, empty, "absent versus present-empty plan");
    assert_ne!(window, layout, "closed assignment key");
    assert_ne!(input, task, "closed cancel trigger family");
    assert_ne!(expression, timeout, "checked trigger expression kind");
    assert_ne!(input, signal, "checked Signal target and payload pattern");
    assert_eq!(window, formatted, "plan formatting and source revision");

    let invalid = super::fixture(&source("", "with { unknown = true }"), None);
    assert!(
        super::analyze(&invalid).is_err(),
        "unknown plan key rejects"
    );
    for (label, plan) in [
        ("timeout duration", "with { timeout true {} }"),
        ("cancel Boolean", "with { cancel on 1i64 {} }"),
        ("Signal target", "with { cancel on signal(true) {} }"),
    ] {
        let invalid = super::fixture(&source("", plan), None);
        assert!(super::analyze(&invalid).is_err(), "{label} rejects");
    }
    let invalid = super::fixture(
        &source(
            "signal ready: bool\n",
            "with { cancel on signal(@signal.ready, \"wrong\") {} }",
        ),
        None,
    );
    assert!(
        super::analyze(&invalid).is_err(),
        "Signal payload pattern must match the checked Signal type",
    );

    let effectful = super::fixture(&source("", "with { window = thread {} }"), None);
    let report = super::analyze(&effectful).expect("effectful plan value checks");
    let choice = report
        .expressions()
        .find_map(|(_, expression)| {
            matches!(
                expression.resolution(),
                CheckedExpressionResolution::Choice(_)
            )
            .then_some(expression)
        })
        .expect("checked Choice expression");
    assert!(
        choice
            .effects()
            .contains(&crate::effects::EffectId::parse("control.spawn").expect("effect ID")),
        "plan value effect reaches its checked Choice owner",
    );
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
