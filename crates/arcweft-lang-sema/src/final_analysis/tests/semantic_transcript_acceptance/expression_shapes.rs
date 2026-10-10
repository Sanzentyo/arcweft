use super::*;

struct MatchArmExpressionObservation {
    semantic_digest: [u8; 32],
    hir_kind: HirExprKind,
    checked_resolution: CheckedExpressionResolution,
    value_type: TypeKind,
}

fn checked_match_arm_expression_observation(
    source: &str,
    arm_ordinal: usize,
) -> MatchArmExpressionObservation {
    let world = super::fixture(source, None);
    let report = super::analyze(&world)
        .unwrap_or_else(|error| panic!("Match expression fixture should check: {error:?}"));
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
    let HirExprKind::Match(authored) = module.resolve_expr(owner).expect("Match owner").kind()
    else {
        panic!("selected expression is a Match");
    };
    let arm = authored
        .arms()
        .get(arm_ordinal)
        .expect("selected Match arm");
    let arm_expression = module
        .resolve_expr(arm.value())
        .expect("Match arm expression");
    let checked = report
        .expression(arm.value())
        .expect("checked Match arm expression");
    let product = super::checked_match_product(&report, project, module, &world.symbols, owner);

    MatchArmExpressionObservation {
        semantic_digest: *product.semantic_digest().as_bytes(),
        hir_kind: arm_expression.kind().clone(),
        checked_resolution: checked.resolution().clone(),
        value_type: checked
            .value_type()
            .expect("checked Match arm value type")
            .clone(),
    }
}

#[test]
fn checked_match_transcript_commits_pipe_owner_and_checked_binding() {
    let identity = checked_match_arm_expression_observation(&pipe_match_source("identity"), 0);
    let shifted = checked_match_arm_expression_observation(&pipe_match_source("shifted"), 0);

    assert_ne!(
        identity.semantic_digest, shifted.semantic_digest,
        "changing the checked Pipe source changes the Match digest",
    );
    assert!(matches!(identity.hir_kind, HirExprKind::Pipe(_)));
    assert_eq!(
        identity.value_type,
        TypeKind::Tuple(vec![TypeKind::I32, TypeKind::I32])
    );
    let CheckedExpressionResolution::Pipe(pipe) = &identity.checked_resolution else {
        panic!("Match arm Pipe has a checked Pipe binding");
    };
    assert_eq!(pipe.occurrences().len(), 2);
}

#[test]
fn checked_match_transcript_commits_try_carrier_and_operand() {
    let first = checked_match_arm_expression_observation(&try_match_source("first"), 0);
    let changed = checked_match_arm_expression_observation(&try_match_source("second"), 0);

    assert_ne!(
        first.semantic_digest, changed.semantic_digest,
        "changing the checked Try operand changes the Match digest",
    );
    assert!(matches!(first.hir_kind, HirExprKind::Try(_)));
    assert_eq!(first.value_type, TypeKind::I64);
    let CheckedExpressionResolution::Try(tried) = &first.checked_resolution else {
        panic!("Match arm Try has a checked residual carrier");
    };
    assert_eq!(tried.carrier().success(), &TypeKind::I64);
    assert!(matches!(
        tried.boundary().owner(),
        CheckedTryBoundaryOwner::CarrierBlock(_)
    ));
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
    let project = callable_value_match_source("identity");
    let local = callable_value_match_source("saved");
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

    let left = field_match_source("left");
    let world = super::fixture(&left, None);
    let report = super::analyze(&world).expect("checked field selection");
    assert!(report.expressions().any(|(_, expression)| matches!(
        expression.resolution(),
        CheckedExpressionResolution::Select(select @ CheckedSelectResolution::Field(_))
            if select.semantic_transcript_tag() == 0x0404
    )));
    assert_ne!(
        source_match_digest(&left),
        source_match_digest(&field_match_source("right")),
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
    let observe = |prefix: &str, call: &str| {
        let world = super::fixture(&method_match_source(prefix, call), None);
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
    let digest = |plan: &str| source_match_digest(&choice_match_source("", plan));
    let absent = digest("");
    let empty = digest("with {}");
    let window = digest("with { window = true }");
    let layout = digest("with { layout = true }");
    let input = digest("with { cancel on input(_) {} }");
    let task = digest("with { cancel on task(_) {} }");
    let expression = digest("with { cancel on true {} }");
    let timeout = digest("with { cancel on timeout(1s) {} }");
    let signal = source_match_digest(&choice_match_source(
        "signal ready: Watch<bool>\n",
        "with { cancel on signal(@signal.ready, value) { let observed = value } }",
    ));
    let formatted = source_match_digest(&choice_match_source(
        "fn unrelated() -> i64 { 99i64 }\n",
        "with {  window = true  }",
    ));

    assert_ne!(absent, empty, "absent versus present-empty plan");
    assert_ne!(window, layout, "closed assignment key");
    assert_ne!(input, task, "closed cancel trigger family");
    assert_ne!(expression, timeout, "checked trigger expression kind");
    assert_ne!(input, signal, "checked Signal target and payload pattern");
    assert_eq!(window, formatted, "plan formatting and source revision");

    let invalid = super::fixture(&choice_match_source("", "with { unknown = true }"), None);
    assert!(
        super::analyze(&invalid).is_err(),
        "unknown plan key rejects"
    );
    for (label, plan) in [
        ("timeout duration", "with { timeout true {} }"),
        ("cancel Boolean", "with { cancel on 1i64 {} }"),
        ("Signal target", "with { cancel on signal(true) {} }"),
    ] {
        let invalid = super::fixture(&choice_match_source("", plan), None);
        assert!(super::analyze(&invalid).is_err(), "{label} rejects");
    }
    let invalid = super::fixture(
        &choice_match_source(
            "signal ready: Watch<bool>\n",
            "with { cancel on signal(@signal.ready, \"wrong\") {} }",
        ),
        None,
    );
    assert!(
        super::analyze(&invalid).is_err(),
        "Signal payload pattern must match the checked Signal type",
    );

    let effectful = super::fixture(
        &choice_match_source("", "with { window = thread {} }"),
        None,
    );
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

#[test]
fn checked_match_transcript_commits_binary_operator_and_range_inclusivity() {
    let result = |expression: &str| source_match_digest(&bool_match_i64_source(expression));
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
    let result = |expression: &str| source_match_digest(&numeric_sequence_match_source(expression));
    assert_ne!(result("[1i64, 2i64]"), result("[1i64, 3i64]"));
    assert_eq!(result("[1i64, 2i64]"), result("[0x1_i64, 0b10_i64]"));
}
