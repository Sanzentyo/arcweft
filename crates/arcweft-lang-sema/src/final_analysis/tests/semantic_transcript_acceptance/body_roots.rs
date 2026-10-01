use super::*;
use arcweft_lang_hir::{
    body_edges::{HirBodyChild, HirBodyKind},
    expr::{HirExpressionOwnedBodyRole, HirExpressionOwnedChild, HirNestedExpressionPathSegment},
};

#[derive(Debug, Eq, PartialEq)]
struct CompactChoicePlanFact {
    public_id: Option<arcweft_id::PublicId>,
    option_ids: Vec<arcweft_id::PublicId>,
    goto_targets: Vec<(u32, [u8; 32])>,
    plan_items: Vec<crate::final_analysis::CheckedChoicePlanItem>,
}

#[test]
fn checked_match_transcript_commits_compact_choice_plan_body_roots() {
    let timeout = "timeout 1s { let timeout_value = 1i64 }";
    let cancel = "cancel on input(_) { let cancel_value = 2i64 }";
    let on_select = "on select selected { let observed = selected; let selected_value = 3i64 }";
    let source = |rows: &[&str], prefix: &str| {
        choice_match_source(prefix, &format!("with {{\n{}\n}}", rows.join("\n")))
    };
    let original = source(&[timeout, cancel, on_select], "");
    let original_fact = assert_compact_choice_plan_body_roots(&original);

    let changed_timeout = original.replace("timeout_value = 1i64", "timeout_value = 9i64");
    let changed_cancel = original.replace("cancel_value = 2i64", "cancel_value = 9i64");
    let changed_on_select = original.replace("selected_value = 3i64", "selected_value = 9i64");
    let reversed = source(&[cancel, timeout, on_select], "");
    let revised = choice_match_source(
        "fn unrelated() -> i64 { 99i64 }\n",
        &format!("with {{\n  {timeout}\n\n  {cancel}\n  {on_select}\n}}"),
    );

    let original = outermost(&match_observations(&original));
    for (label, candidate) in [
        ("Timeout body", changed_timeout),
        ("Cancel body", changed_cancel),
        ("OnSelect body", changed_on_select),
    ] {
        let changed_fact = assert_compact_choice_plan_body_roots(&candidate);
        assert_eq!(
            original_fact, changed_fact,
            "{label} retains the checked Choice target and plan rows",
        );
        let changed = outermost(&match_observations(&candidate));
        assert_ne!(
            original.digest, changed.digest,
            "{label} changes Match meaning"
        );
    }
    let reversed = outermost(&match_observations(&reversed));
    assert_ne!(
        original.digest, reversed.digest,
        "plan source order changes Match meaning"
    );
    let revised = outermost(&match_observations(&revised));
    assert_ne!(
        original.owner, revised.owner,
        "unrelated declaration shifts HIR IDs"
    );
    assert_ne!(
        original.source_start, revised.source_start,
        "source revision moves the Match span"
    );
    assert_ne!(
        original.source_end, revised.source_end,
        "source revision moves the Match span"
    );
    assert_eq!(
        original.digest, revised.digest,
        "format and unrelated source revision preserve checked meaning"
    );
}

fn assert_compact_choice_plan_body_roots(source: &str) -> CompactChoicePlanFact {
    let world = super::fixture(source, None);
    let report = super::analyze(&world).expect("compact Choice plan checks");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let match_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match expression");
    let choice_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Choice(_)).then_some(owner)
        })
        .expect("Choice expression below Match");
    let Some(CheckedExpressionResolution::Choice(checked_choice)) = report
        .expression(choice_owner)
        .map(CheckedExpression::resolution)
    else {
        panic!("checked Choice expression");
    };
    let fact = CompactChoicePlanFact {
        public_id: checked_choice.public_id().cloned(),
        option_ids: checked_choice.option_ids().to_vec(),
        goto_targets: checked_choice
            .gotos()
            .iter()
            .map(|goto| (goto.arm(), *goto.target().semantic_id().as_bytes()))
            .collect(),
        plan_items: checked_choice
            .plan()
            .expect("checked lifecycle plan")
            .items()
            .to_vec(),
    };
    let product =
        super::checked_match_product(&report, project, module, &world.symbols, match_owner);
    assert!(!product.arms().is_empty(), "accepted Match product");

    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let match_path = coordinates
        .expression(match_owner)
        .expect("accepted Match path");
    let choice_path = coordinates
        .expression(choice_owner)
        .expect("accepted Choice path");
    assert!(choice_path.is_at_or_below(&match_path));
    let choice = module.resolve_expr(choice_owner).expect("Choice HIR owner");
    assert_compact_choice_body_projections(choice.kind(), &report, &coordinates, &match_path);
    assert_compact_choice_plan_patterns(
        choice.kind(),
        &report,
        &coordinates,
        &match_path,
        world.registered.environment().statement_ingress().input(),
    );
    fact
}

fn assert_compact_choice_body_projections(
    choice: &HirExprKind,
    report: &FinalSemanticAnalysis,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    match_path: &crate::semantic_coordinate::CheckedSemanticPath,
) {
    let bodies = choice
        .expression_owned_body_projections()
        .expect("closed Choice plan body projections");
    assert_eq!(bodies.len(), 3, "Timeout, Cancel, and OnSelect body roots");
    for (ordinal, body) in bodies.iter().enumerate() {
        let (role_ordinal, path) = match body.role() {
            HirExpressionOwnedBodyRole::ChoicePlanTimeoutBody { path } => (0, path),
            HirExpressionOwnedBodyRole::ChoicePlanCancelBody { path } => (1, path),
            HirExpressionOwnedBodyRole::ChoicePlanOnSelectBody { path } => (2, path),
            role => panic!("unexpected Choice body role: {role:?}"),
        };
        assert_eq!(role_ordinal, ordinal, "Choice body source order");
        assert_eq!(
            path.segments(),
            [HirNestedExpressionPathSegment::ChoicePlanItem {
                ordinal: u32::try_from(ordinal).expect("three plan rows"),
            }]
        );
        assert_eq!(body.kind(), HirBodyKind::Thread);
        assert_eq!(
            body.children().len(),
            if ordinal == 2 { 2 } else { 1 },
            "each plan body retains its checked statement children",
        );
        for child in body.children() {
            let HirBodyChild::Statement(owner) = child.child() else {
                panic!("plan body child is a statement");
            };
            assert!(
                report.statement(owner).is_some(),
                "checked plan body statement"
            );
            let path = coordinates
                .statement(owner)
                .expect("accepted statement path");
            assert!(path.path().is_at_or_below(match_path));
        }
    }
}

fn assert_compact_choice_plan_patterns(
    choice: &HirExprKind,
    report: &FinalSemanticAnalysis,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    match_path: &crate::semantic_coordinate::CheckedSemanticPath,
    expected_input: &TypeKind,
) {
    let mut cancel_pattern = false;
    let mut on_select_pattern = false;
    for edge in choice
        .expression_owned_child_edges()
        .expect("closed Choice plan child edges")
    {
        let HirExpressionOwnedChild::Pattern(owner) = edge.child() else {
            continue;
        };
        let checked = report.pattern(owner).expect("checked Choice plan pattern");
        let path = coordinates.pattern(owner).expect("accepted pattern path");
        assert!(path.path().is_at_or_below(match_path));
        match edge.role() {
            HirExpressionOwnedBodyRole::ChoicePlanCancelTrigger { path } => {
                assert_eq!(
                    path.segments(),
                    [HirNestedExpressionPathSegment::ChoicePlanItem { ordinal: 1 }]
                );
                assert_eq!(checked.ty(), expected_input);
                cancel_pattern = true;
            }
            HirExpressionOwnedBodyRole::ChoicePlanOnSelectPattern { path } => {
                assert_eq!(
                    path.segments(),
                    [HirNestedExpressionPathSegment::ChoicePlanItem { ordinal: 2 }]
                );
                assert_eq!(
                    checked.ty(),
                    &TypeKind::entity_ref(EntityKind::ChoiceOption)
                );
                on_select_pattern = true;
            }
            role => panic!("unexpected Choice plan pattern role: {role:?}"),
        }
    }
    assert!(
        cancel_pattern && on_select_pattern,
        "both plan patterns are checked"
    );
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
    let original = match_observations(&match_arm_let_source("1i64"));
    let changed = match_observations(&match_arm_let_source("2i64"));

    assert_eq!(original.len(), 1);
    assert_eq!(changed.len(), 1);
    assert_ne!(outermost(&original).digest, outermost(&changed).digest);
}

#[test]
fn checked_match_transcript_commits_nested_statement_body_meaning() {
    let digest = |body| outermost(&match_observations(&nested_if_statement_source(body))).digest;

    let first = digest("                let first = 1i64");
    let changed_value = digest("                let first = 2i64");
    let empty = digest("");
    let ordered = digest("                let first = 1i64\n                let second = 2i64");
    let reversed = digest("                let second = 2i64\n                let first = 1i64");

    assert_ne!(first, changed_value, "nested statement body value");
    assert_ne!(first, empty, "nonempty versus empty statement body");
    assert_ne!(ordered, reversed, "nested statement body order");
}

pub(super) fn assert_match_sensitivity_and_source_revision_invariance(
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
fn checked_match_transcript_commits_await_pending_body_and_ignores_source_revision() {
    let original = r#"
fn observe(need: Need<i64>) -> i64 {
    await need with {
        pending progress => {
            let selected = match true {
                true => 1i64
                false => 2i64
            }
        }
    }
}
"#;
    let changed = original.replace("true => 1i64", "true => 3i64");
    let revised = r#"
fn unrelated() -> i64 { 99i64 }
fn observe ( need : Need<i64> ) -> i64 {
    await need with {
        pending progress => {
            let selected = match true {
                true => 0x1_i64
                false => 2i64
            }
        }
    }
}
"#;

    assert_match_sensitivity_and_source_revision_invariance(
        "Await Pending body",
        original,
        &changed,
        revised,
    );
}

#[test]
fn checked_match_transcript_commits_dialogue_on_body_and_ignores_source_revision() {
    let original = r#"
pub character alice { display = "Alice" }
flow row() -> Unit {
    alice[before [mark @.release] after[p]] with {
        on mark(@.release) {
            let selected = match true {
                true => "Released"
                false => "Moved"
            }
            out "Released"
        }
    }
    return ()
}
"#;
    let changed = original.replace("true => \"Released\"", "true => \"Moved\"");
    let revised = r#"
fn unrelated() -> i64 { 99i64 }
pub character alice { display = "Alice" }
flow row ( ) -> Unit {
    alice[before [mark @.release] after[p]] with {
        on mark(@.release) {
            let selected = match true {
                true => "Released"
                false => "Moved"
            }
            out "Released"
        }
    }
return ()
}
"#;

    assert_match_sensitivity_and_source_revision_invariance(
        "dialogue On body",
        original,
        &changed,
        revised,
    );
}

#[test]
fn checked_match_transcript_commits_attached_default_and_ignores_source_revision() {
    let original = r#"
fn fallback(first: DialogueContent, second: DialogueContent)[body: DialogueContent = {
        match true {
            true => first
            false => second
        }
    }] -> DialogueContent { body }
"#;
    let changed = original.replace("true => first", "true => second");
    let revised = r#"
fn unrelated() -> i64 { 99i64 }
fn fallback(first: DialogueContent, second: DialogueContent)[body: DialogueContent = {
        match true {
            true => first
            false => second
        }
    }] -> DialogueContent { body }
"#;

    assert_match_sensitivity_and_source_revision_invariance(
        "attached-content default",
        original,
        &changed,
        revised,
    );
}

#[test]
fn checked_match_transcript_commits_trait_impl_method_body_and_ignores_source_revision() {
    let original = r#"
struct RouteInfo { label: String }
impl DisplayText for RouteInfo {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> {
        match true {
            true => Ok(fmt(self.label))
            false => Ok(fmt("alternate"))
        }
    }
}
fn render(value: RouteInfo) -> Content { fmt(value) }
"#;
    let changed = original.replace("fmt(\"alternate\")", "fmt(\"fallback\")");
    let revised = r#"
fn unrelated() -> i64 { 99i64 }
struct RouteInfo { label: String }
impl DisplayText for RouteInfo {
    fn display_text(self, ctx: DisplayContext) -> Result<Content, DisplayError> {
        match true {
            true => Ok(fmt(self.label))
            false => Ok(fmt("alternate"))
        }
    }
}
fn render(value: RouteInfo) -> Content { fmt(value) }
"#;

    assert_match_sensitivity_and_source_revision_invariance(
        "trait implementation method body",
        original,
        &changed,
        revised,
    );
}

#[test]
fn checked_match_transcript_commits_inherent_method_body_and_ignores_source_revision() {
    let original = r#"
struct Number { value: i64 }
impl Number {
    fn get(self, flag: bool) -> i64 {
        match flag {
            true => self.value
            false => 0i64
        }
    }
}
"#;
    let changed = original.replace("true => self.value", "true => 1i64");
    let revised = r#"
fn unrelated() -> i64 { 99i64 }
struct Number { value: i64 }
impl Number {
    fn get(self, flag: bool) -> i64 {
        match flag {
            true => self.value
            false => 0i64
        }
    }
}
"#;

    assert_match_sensitivity_and_source_revision_invariance(
        "inherent implementation method body",
        original,
        &changed,
        revised,
    );
}
