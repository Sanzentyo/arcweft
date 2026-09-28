use super::*;
use crate::final_analysis::CheckedRecordPatternOwner;
use arcweft_lang_hir::pattern::{HirPatternChild, HirPatternKind, HirPatternSequenceRest};

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

fn entry_reference_match_source(first: &str, second: &str) -> String {
    format!(
        "flow done() -> String {{ return \"done\" }}\nflow @flow.references references {{\n    let selected = match true {{\n        true => {first}\n        false => {second}\n    }}\n}}\nentry cli @entry.cli.primary {{ goto @flow.done }}\nentry cli @entry.cli.alternate {{ goto @flow.done }}\n"
    )
}

fn project_item_match_source(first: &str, second: &str) -> String {
    format!(
        "pub character alternate {{}}\nfn root(flag: bool) -> Ref<Character> {{\n    match flag {{\n        true => {first}\n        false => {second}\n    }}\n}}\n"
    )
}

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
        "signal ready: bool\n",
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
            "signal ready: bool\n",
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

fn first_arm_pattern_digest(source: &str) -> [u8; 32] {
    checked_match_pattern_observation(source, 0).arm_pattern_digest
}

fn sequence_pattern_source(pattern: &str) -> String {
    format!(
        "fn root(values: Vec<bool>) -> i64 {{\n    match values {{\n        {pattern} => 1i64\n        _ => 0i64\n    }}\n}}\n"
    )
}

fn tuple_pattern_source(pattern: &str) -> String {
    format!(
        "fn root(pair: (bool, bool)) -> i64 {{\n    match pair {{\n        {pattern} => 1i64\n        _ => 0i64\n    }}\n}}\n"
    )
}

fn result_pattern_source(ok_pattern: &str) -> String {
    format!(
        "fn root(value: Result<bool, String>) -> i64 {{\n    match value {{\n        .Ok({ok_pattern}) => 1i64\n        .Err(error) => 2i64\n    }}\n}}\n"
    )
}

fn choice_pattern_source(first: &str, second: &str) -> String {
    format!(
        "fn root(value: String | Bytes) -> i64 {{\n    match value {{\n        text: {first} => 1i64\n        bytes: {second} => 2i64\n    }}\n}}\n"
    )
}

fn record_variant_pattern_source(field_type: &str) -> String {
    format!(
        "enum Event {{\n    Start,\n    Empty {{}},\n    ChoiceSelected {{ id: {field_type} }},\n}}\nfn root(event: Event) -> i64 {{\n    match event {{\n        .Start => 0i64\n        .Empty {{}} => 1i64\n        .ChoiceSelected {{ id }} => 2i64\n    }}\n}}\n"
    )
}

fn mutable_binding_pattern_source(pattern: &str) -> String {
    format!(
        "fn root(value: bool) -> i64 {{\n    match value {{\n        {pattern} => 1i64\n    }}\n}}\n"
    )
}

fn entity_reference_pattern_source(pattern: &str) -> String {
    format!(
        "flow @flow.primary primary {{}}\nflow @flow.alternate alternate {{}}\nfn root(value: Ref<Flow>) -> i64 {{\n    match value {{\n        {pattern} => 1i64\n        _ => 0i64\n    }}\n}}\n"
    )
}

fn whole_binding_pattern_source(pattern: &str) -> String {
    format!(
        "enum Event {{\n    Start,\n    Empty {{}},\n    ChoiceSelected {{ id: i64 }},\n}}\nfn root(event: Event) -> i64 {{\n    match event {{\n        {pattern} => 2i64\n        .Start => 0i64\n        .Empty {{}} => 1i64\n    }}\n}}\n"
    )
}

struct CheckedMatchPatternFact {
    hir_kind: HirPatternKind,
    checked: CheckedPattern,
}

struct CheckedMatchPatternObservation {
    semantic_digest: [u8; 32],
    arm_pattern_digest: [u8; 32],
    scrutinee_type: TypeKind,
    facts: Vec<CheckedMatchPatternFact>,
}

fn checked_match_pattern_observation(
    source: &str,
    arm_ordinal: usize,
) -> CheckedMatchPatternObservation {
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
    let HirExprKind::Match(authored) = module.resolve_expr(owner).expect("Match owner").kind()
    else {
        panic!("selected expression is a Match");
    };
    let arm = authored
        .arms()
        .get(arm_ordinal)
        .expect("selected Match arm");
    let product = super::checked_match_product(&report, project, module, &world.symbols, owner);
    let arm_product = product
        .arms()
        .get(arm_ordinal)
        .expect("checked Match arm product");
    let mut pending = vec![arm.pattern()];
    let mut visited = std::collections::BTreeSet::new();
    let mut facts = Vec::new();
    while let Some(pattern_owner) = pending.pop() {
        if !visited.insert(pattern_owner) {
            continue;
        }
        let hir = module
            .resolve_pattern(pattern_owner)
            .expect("accepted Match pattern owner");
        let checked = report
            .pattern(pattern_owner)
            .expect("checked Match pattern fact");
        pending.extend(hir.kind().child_edges().into_iter().filter_map(
            |edge| match edge.child() {
                HirPatternChild::Pattern(child) => Some(child),
                HirPatternChild::Local(_) | HirPatternChild::Type(_) => None,
            },
        ));
        facts.push(CheckedMatchPatternFact {
            hir_kind: hir.kind().clone(),
            checked: checked.clone(),
        });
    }

    CheckedMatchPatternObservation {
        semantic_digest: *product.semantic_digest().as_bytes(),
        arm_pattern_digest: *arm_product.pattern().as_bytes(),
        scrutinee_type: report
            .expression(authored.scrutinee())
            .and_then(CheckedExpression::value_type)
            .expect("checked Match scrutinee type")
            .clone(),
        facts,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatternCorpusDisposition {
    Accepted,
    RejectOnly,
    #[allow(
        dead_code,
        reason = "no live pattern family has been proven unreachable"
    )]
    ProvenUnreachable,
}

// The inventory and its dispositions come from one list. The `of` matches
// below remain exhaustive against the live HIR and checked owner enums.
macro_rules! pattern_family_inventory {
    ($family:ident { $($variant:ident => $disposition:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        enum $family {
            $($variant),+
        }

        impl $family {
            const INVENTORY: &'static [(Self, PatternCorpusDisposition)] = &[
                $((Self::$variant, PatternCorpusDisposition::$disposition)),+
            ];
        }
    };
}

pattern_family_inventory!(PatternShapeFamily {
    Binding => Accepted,
    MutableBinding => Accepted,
    Literal => Accepted,
    EntityReference => Accepted,
    Variant => Accepted,
    Discard => Accepted,
    Tuple => Accepted,
    Record => Accepted,
    BracketSequence => Accepted,
    WholeBinding => Accepted,
    Or => Accepted,
    TypedBinding => Accepted,
    Error => RejectOnly,
});

impl PatternShapeFamily {
    fn of(kind: &HirPatternKind) -> Self {
        match kind {
            HirPatternKind::Binding(_) => Self::Binding,
            HirPatternKind::MutableBinding(_) => Self::MutableBinding,
            HirPatternKind::Literal(_) => Self::Literal,
            HirPatternKind::EntityReference(_) => Self::EntityReference,
            HirPatternKind::Variant(_) => Self::Variant,
            HirPatternKind::Discard => Self::Discard,
            HirPatternKind::Tuple { .. } => Self::Tuple,
            HirPatternKind::Record { .. } => Self::Record,
            HirPatternKind::BracketSequence { .. } => Self::BracketSequence,
            HirPatternKind::WholeBinding { .. } => Self::WholeBinding,
            HirPatternKind::Or { .. } => Self::Or,
            HirPatternKind::TypedBinding { .. } => Self::TypedBinding,
            HirPatternKind::Error(_) => Self::Error,
        }
    }
}

pattern_family_inventory!(PatternResolutionFamily {
    Structural => Accepted,
    Literal => Accepted,
    Entity => Accepted,
    Record => Accepted,
    Variant => Accepted,
    TypedBinding => Accepted,
});

impl PatternResolutionFamily {
    fn of(resolution: &CheckedPatternResolution) -> Self {
        match resolution {
            CheckedPatternResolution::Structural => Self::Structural,
            CheckedPatternResolution::Literal(_) => Self::Literal,
            CheckedPatternResolution::Entity(_) => Self::Entity,
            CheckedPatternResolution::Record(_) => Self::Record,
            CheckedPatternResolution::Variant(_) => Self::Variant,
            CheckedPatternResolution::TypedBinding(_) => Self::TypedBinding,
        }
    }
}

struct MatchPatternCorpusObservation {
    shapes: BTreeSet<PatternShapeFamily>,
    resolutions: BTreeSet<PatternResolutionFamily>,
}

/// Collect checked pattern facts whose accepted owner path descends from the
/// selected Match expression, including all arms and nested pattern children.
fn accepted_match_pattern_corpus_observation(source: &str) -> MatchPatternCorpusObservation {
    let world = super::fixture(source, None);
    let report = super::analyze(&world).expect("pattern corpus source should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let match_owners = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    let [match_owner] = match_owners.as_slice() else {
        panic!("each pattern corpus row has exactly one Match expression");
    };
    let product =
        super::checked_match_product(&report, project, module, &world.symbols, *match_owner);
    assert!(
        !product.arms().is_empty(),
        "accepted Match has checked arms"
    );

    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let root_path = coordinates
        .expression(*match_owner)
        .expect("accepted Match root path");
    let mut shapes = BTreeSet::new();
    let mut resolutions = BTreeSet::new();
    for (owner, hir) in module.patterns() {
        let Some(checked) = report.pattern(owner) else {
            continue;
        };
        let coordinate = coordinates
            .pattern(owner)
            .expect("checked pattern owner path");
        let path = coordinate.path();
        if path.root() != root_path.root() || !path.steps().starts_with(root_path.steps()) {
            continue;
        }
        shapes.insert(PatternShapeFamily::of(hir.kind()));
        resolutions.insert(PatternResolutionFamily::of(checked.resolution()));
    }
    assert!(
        !shapes.is_empty(),
        "Match root has checked pattern descendants"
    );
    MatchPatternCorpusObservation {
        shapes,
        resolutions,
    }
}

#[test]
fn checked_match_pattern_corpus_tracks_accepted_root_families() {
    struct Row {
        name: &'static str,
        source: String,
        shapes: &'static [PatternShapeFamily],
        resolutions: &'static [PatternResolutionFamily],
    }
    let rows = [
        Row {
            name: "tuple and Or",
            source: tuple_pattern_source("(true | false, true)"),
            shapes: &[
                PatternShapeFamily::Tuple,
                PatternShapeFamily::Or,
                PatternShapeFamily::Literal,
                PatternShapeFamily::Discard,
            ],
            resolutions: &[
                PatternResolutionFamily::Structural,
                PatternResolutionFamily::Literal,
            ],
        },
        Row {
            name: "Result variant and binding",
            source: result_pattern_source("value"),
            shapes: &[PatternShapeFamily::Variant, PatternShapeFamily::Binding],
            resolutions: &[PatternResolutionFamily::Variant],
        },
        Row {
            name: "Choice typed binding",
            source: choice_pattern_source("String", "Bytes"),
            shapes: &[PatternShapeFamily::TypedBinding],
            resolutions: &[PatternResolutionFamily::TypedBinding],
        },
        Row {
            name: "sequence rest",
            source: sequence_pattern_source("[true, ..]"),
            shapes: &[PatternShapeFamily::BracketSequence],
            resolutions: &[PatternResolutionFamily::Structural],
        },
        Row {
            name: "project record variant",
            source: record_variant_pattern_source("i64"),
            shapes: &[PatternShapeFamily::Record, PatternShapeFamily::Variant],
            resolutions: &[
                PatternResolutionFamily::Record,
                PatternResolutionFamily::Variant,
            ],
        },
        Row {
            name: "mutable binding",
            source: mutable_binding_pattern_source("mut selected"),
            shapes: &[PatternShapeFamily::MutableBinding],
            resolutions: &[PatternResolutionFamily::Structural],
        },
        Row {
            name: "checked entity reference",
            source: entity_reference_pattern_source("@flow.primary"),
            shapes: &[
                PatternShapeFamily::EntityReference,
                PatternShapeFamily::Discard,
            ],
            resolutions: &[
                PatternResolutionFamily::Entity,
                PatternResolutionFamily::Structural,
            ],
        },
        Row {
            name: "whole binding",
            source: whole_binding_pattern_source("whole .ChoiceSelected { id }"),
            shapes: &[
                PatternShapeFamily::WholeBinding,
                PatternShapeFamily::Variant,
                PatternShapeFamily::Record,
            ],
            resolutions: &[
                PatternResolutionFamily::Structural,
                PatternResolutionFamily::Variant,
                PatternResolutionFamily::Record,
            ],
        },
    ];
    let mut observed_shapes = BTreeSet::new();
    let mut observed_resolutions = BTreeSet::new();
    for row in rows {
        let observation = accepted_match_pattern_corpus_observation(&row.source);
        for required in row.shapes {
            assert!(
                observation.shapes.contains(required),
                "{} should contain {required:?}",
                row.name,
            );
        }
        for required in row.resolutions {
            assert!(
                observation.resolutions.contains(required),
                "{} should contain {required:?}",
                row.name,
            );
        }
        observed_shapes.extend(observation.shapes);
        observed_resolutions.extend(observation.resolutions);
    }
    for (family, disposition) in PatternShapeFamily::INVENTORY {
        assert_eq!(
            observed_shapes.contains(family),
            *disposition == PatternCorpusDisposition::Accepted,
            "HIR pattern {family:?} corpus disposition {disposition:?}",
        );
    }
    for (family, disposition) in PatternResolutionFamily::INVENTORY {
        assert_eq!(
            observed_resolutions.contains(family),
            *disposition == PatternCorpusDisposition::Accepted,
            "checked pattern {family:?} corpus disposition {disposition:?}",
        );
    }
}

#[test]
fn checked_match_pattern_corpus_excludes_unrelated_declaration_patterns() {
    let source = format!(
        "fn unrelated() -> bool {{ let outside = true; outside }}\n{}",
        tuple_pattern_source("(true | false, true)")
    );
    let world = super::fixture(&source, None);
    let report = super::analyze(&world).expect("unrelated binding should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    assert!(module.patterns().any(|(owner, pattern)| {
        matches!(pattern.kind(), HirPatternKind::Binding(_)) && report.pattern(owner).is_some()
    }));
    let observation = accepted_match_pattern_corpus_observation(&source);
    assert!(observation.shapes.contains(&PatternShapeFamily::Tuple));
    assert!(
        !observation.shapes.contains(&PatternShapeFamily::Binding),
        "a checked binding outside the Match root must not enter its corpus",
    );
}

#[test]
fn checked_match_pattern_transcript_commits_mutable_entity_and_whole_binding_owners() {
    let mutable =
        checked_match_pattern_observation(&mutable_binding_pattern_source("mut selected"), 0);
    let immutable =
        checked_match_pattern_observation(&mutable_binding_pattern_source("selected"), 0);
    assert_ne!(
        mutable.semantic_digest, immutable.semantic_digest,
        "mutable binding mode reaches the Match transcript",
    );
    assert!(mutable.facts.iter().any(|fact| matches!(
        (&fact.hir_kind, fact.checked.resolution()),
        (
            HirPatternKind::MutableBinding(_),
            CheckedPatternResolution::Structural
        )
    )));

    let primary =
        checked_match_pattern_observation(&entity_reference_pattern_source("@flow.primary"), 0);
    let alternate =
        checked_match_pattern_observation(&entity_reference_pattern_source("@flow.alternate"), 0);
    let checked_entity = |observation: &CheckedMatchPatternObservation| {
        observation
            .facts
            .iter()
            .find_map(|fact| match (&fact.hir_kind, fact.checked.resolution()) {
                (HirPatternKind::EntityReference(_), CheckedPatternResolution::Entity(item)) => {
                    Some(item.semantic_id())
                }
                _ => None,
            })
            .expect("entity pattern resolves to an accepted project item")
    };
    assert_ne!(checked_entity(&primary), checked_entity(&alternate));
    assert_ne!(
        primary.semantic_digest, alternate.semantic_digest,
        "checked entity target meaning reaches the Match transcript",
    );

    let whole = checked_match_pattern_observation(
        &whole_binding_pattern_source("whole .ChoiceSelected { id }"),
        0,
    );
    let variant = checked_match_pattern_observation(
        &whole_binding_pattern_source(".ChoiceSelected { id }"),
        0,
    );
    assert_ne!(
        whole.semantic_digest, variant.semantic_digest,
        "whole-pattern binding reaches the Match transcript",
    );
    assert!(whole.facts.iter().any(|fact| matches!(
        (&fact.hir_kind, fact.checked.resolution()),
        (
            HirPatternKind::WholeBinding { .. },
            CheckedPatternResolution::Structural
        )
    )));
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

#[test]
fn checked_match_pattern_transcript_distinguishes_exact_and_rest_sequences() {
    assert_ne!(
        first_arm_pattern_digest(&sequence_pattern_source("[true]")),
        first_arm_pattern_digest(&sequence_pattern_source("[true, ..]")),
    );
    assert_ne!(
        first_arm_pattern_digest(&sequence_pattern_source("[]")),
        first_arm_pattern_digest(&sequence_pattern_source("[..]")),
    );
}

#[test]
fn checked_match_pattern_transcript_carries_tuple_and_or_targets() {
    let tuple_or =
        checked_match_pattern_observation(&tuple_pattern_source("(true | false, true)"), 0);
    let changed =
        checked_match_pattern_observation(&tuple_pattern_source("(true | false, false)"), 0);

    assert_ne!(
        tuple_or.semantic_digest, changed.semantic_digest,
        "a checked tuple element change reaches the Match digest",
    );
    assert!(matches!(
        &tuple_or.scrutinee_type,
        TypeKind::Tuple(elements)
            if elements.as_slice() == [TypeKind::Bool, TypeKind::Bool]
    ));
    assert!(tuple_or.facts.iter().any(|fact| matches!(
        &fact.hir_kind,
        HirPatternKind::Tuple { .. }
    ) && matches!(
        fact.checked.resolution(),
        CheckedPatternResolution::Structural
    ) && matches!(fact.checked.ty(), TypeKind::Tuple(_))));
    assert!(
        tuple_or
            .facts
            .iter()
            .any(|fact| matches!(&fact.hir_kind, HirPatternKind::Or { .. })
                && matches!(
                    fact.checked.resolution(),
                    CheckedPatternResolution::Structural
                )
                && matches!(fact.checked.ty(), TypeKind::Bool))
    );
}

#[test]
fn checked_match_pattern_transcript_carries_result_and_choice_targets() {
    let result = checked_match_pattern_observation(&result_pattern_source("value"), 0);
    let error = checked_match_pattern_observation(&result_pattern_source("value"), 1);
    let changed_result = checked_match_pattern_observation(&result_pattern_source("_"), 0);
    assert_ne!(
        result.semantic_digest, changed_result.semantic_digest,
        "a Result payload binding change reaches the Match digest",
    );
    let selected_result_cases = result
        .facts
        .iter()
        .chain(&error.facts)
        .filter_map(|fact| match (&fact.hir_kind, fact.checked.resolution()) {
            (HirPatternKind::Variant(_), CheckedPatternResolution::Variant(variant))
                if matches!(
                    variant.owner().kind(),
                    CheckedVariantOwnerKind::Result { ok, error }
                        if ok == &TypeKind::Bool && error == &TypeKind::String
                ) =>
            {
                variant.selected().diagnostic_name().map(str::to_owned)
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        selected_result_cases,
        ["Err".to_owned(), "Ok".to_owned()].into_iter().collect()
    );

    let choice = checked_match_pattern_observation(&choice_pattern_source("String", "Bytes"), 0);
    let changed_choice =
        checked_match_pattern_observation(&choice_pattern_source("Bytes", "String"), 0);
    assert_ne!(
        choice.semantic_digest, changed_choice.semantic_digest,
        "checked Choice alternative selection reaches the Match digest",
    );
    assert!(matches!(
        &choice.scrutinee_type,
        TypeKind::Choice(alternatives) if alternatives.len() == 2
    ));
    let typed_binding = choice
        .facts
        .iter()
        .find_map(|fact| match (&fact.hir_kind, fact.checked.resolution()) {
            (
                HirPatternKind::TypedBinding { .. },
                CheckedPatternResolution::TypedBinding(binding),
            ) => Some(binding),
            _ => None,
        })
        .expect("checked Choice typed binding reaches the Match arm");
    assert_eq!(typed_binding.annotation(), &TypeKind::String);
    assert_eq!(typed_binding.choice_alternatives().len(), 1);
}

#[test]
fn checked_match_pattern_transcript_carries_sequence_target_and_rest_mode() {
    let source = |first: &str| {
        format!(
            "fn root(items: Vec<bool>) -> i64 {{\n    match items {{\n        [{first}, ..] => 1i64\n        _ => 0i64\n    }}\n}}\n"
        )
    };
    let sequence = checked_match_pattern_observation(&source("true"), 0);
    let changed = checked_match_pattern_observation(&source("false"), 0);

    assert_ne!(
        sequence.semantic_digest, changed.semantic_digest,
        "a checked sequence element change reaches the Match digest",
    );
    assert!(matches!(
        &sequence.scrutinee_type,
        TypeKind::Vec(item) if item.as_ref() == &TypeKind::Bool
    ));
    assert!(sequence.facts.iter().any(|fact| matches!(
        &fact.hir_kind,
        HirPatternKind::BracketSequence {
            rest: HirPatternSequenceRest::Unbound,
            ..
        }
    ) && matches!(
        fact.checked.resolution(),
        CheckedPatternResolution::Structural
    ) && matches!(
        fact.checked.ty(),
        TypeKind::Vec(item) if item.as_ref() == &TypeKind::Bool
    )));
}

#[test]
fn checked_match_pattern_transcript_carries_project_record_variant_owner() {
    let record_variant =
        checked_match_pattern_observation(&record_variant_pattern_source("i64"), 2);
    let changed = checked_match_pattern_observation(&record_variant_pattern_source("i32"), 2);

    assert_ne!(
        record_variant.semantic_digest, changed.semantic_digest,
        "the project record-variant payload schema reaches the Match digest",
    );
    let carries_payload_type = |observation: &CheckedMatchPatternObservation,
                                expected: &TypeKind| {
        observation.facts.iter().any(|fact| {
            matches!(
                (&fact.hir_kind, fact.checked.resolution()),
                (HirPatternKind::Record { .. }, CheckedPatternResolution::Record(record))
                    if matches!(
                        record.owner(),
                        CheckedRecordPatternOwner::VariantPayload { .. }
                    ) && record.fields().len() == 1
                        && record.fields()[0].field_type() == expected
            )
        })
    };
    assert!(carries_payload_type(&record_variant, &TypeKind::I64));
    assert!(carries_payload_type(&changed, &TypeKind::I32));
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpressionCorpusDisposition {
    Accepted,
    Pending,
    RejectOnly,
    #[allow(
        dead_code,
        reason = "no live expression family has been proven unreachable"
    )]
    ProvenUnreachable,
}

// Each list generates both the corpus disposition and an exhaustive classifier
// against its live HIR or checked owner enum.
macro_rules! expression_family_inventory {
    ($family:ident for $owner:ty, { $($pattern:pat => $variant:ident => $disposition:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        enum $family {
            $($variant),+
        }

        impl $family {
            const INVENTORY: &'static [(Self, ExpressionCorpusDisposition)] = &[
                $((Self::$variant, ExpressionCorpusDisposition::$disposition)),+
            ];

            fn of(owner: &$owner) -> Self {
                match owner {
                    $($pattern => Self::$variant),+
                }
            }
        }
    };
}

expression_family_inventory!(ExprShapeFamily for HirExprKind, {
    HirExprKind::Unit => Unit => Accepted,
    HirExprKind::Literal(_) => Literal => Accepted,
    HirExprKind::EntityReference(_) => EntityReference => Accepted,
    HirExprKind::LifetimePath(_) => LifetimePath => Pending,
    HirExprKind::Path(_) => Path => Accepted,
    HirExprKind::ShortVariant(_) => ShortVariant => Pending,
    HirExprKind::Placeholder(_) => Placeholder => Accepted,
    HirExprKind::Tuple(_) => Tuple => Accepted,
    HirExprKind::BracketSequence(_) => BracketSequence => Pending,
    HirExprKind::NumericBracketSequence(_) => NumericBracketSequence => Accepted,
    HirExprKind::ArrayRepeat(_) => ArrayRepeat => Pending,
    HirExprKind::Call(_) => Call => Accepted,
    HirExprKind::Select(_) => Select => Accepted,
    HirExprKind::Index(_) => Index => Pending,
    HirExprKind::Pipe(_) => Pipe => Accepted,
    HirExprKind::Try(_) => Try => Accepted,
    HirExprKind::Await(_) => Await => Pending,
    HirExprKind::Thread(_) => Thread => Accepted,
    HirExprKind::Choice(_) => Choice => Accepted,
    HirExprKind::Range(_) => Range => Pending,
    HirExprKind::Record(_) => Record => Pending,
    HirExprKind::RecordLiteral(_) => RecordLiteral => Pending,
    HirExprKind::Binary(_) => Binary => Accepted,
    HirExprKind::Borrow(_) => Borrow => Pending,
    HirExprKind::Dereference(_) => Dereference => Pending,
    HirExprKind::Closure(_) => Closure => Accepted,
    HirExprKind::Unary(_) => Unary => Pending,
    HirExprKind::Block(_) => Block => Accepted,
    HirExprKind::ComputationBlock(_) => ComputationBlock => Pending,
    HirExprKind::NamedBlock(_) => NamedBlock => Pending,
    HirExprKind::Loop(_) => Loop => Pending,
    HirExprKind::If(_) => If => Pending,
    HirExprKind::IfLet(_) => IfLet => Pending,
    HirExprKind::Match(_) => Match => Accepted,
    HirExprKind::AttachedContentApplication(_) => AttachedContentApplication => Pending,
    HirExprKind::PostfixBracket(_) => PostfixBracket => Pending,
    HirExprKind::Error(_) => Error => RejectOnly,
    HirExprKind::ForSynthetic(_) => ForSynthetic => Pending,
});

expression_family_inventory!(ExpressionResolutionFamily for CheckedExpressionResolution, {
    CheckedExpressionResolution::Structural => Structural => Accepted,
    CheckedExpressionResolution::Scope(_) => Scope => Pending,
    CheckedExpressionResolution::Literal(_) => Literal => Accepted,
    CheckedExpressionResolution::Value(_) => Value => Accepted,
    CheckedExpressionResolution::Select(_) => Select => Accepted,
    CheckedExpressionResolution::Nominal(_) => Nominal => Pending,
    CheckedExpressionResolution::Variant(_) => Variant => Pending,
    CheckedExpressionResolution::CompileTimeEnum(_) => CompileTimeEnum => Pending,
    CheckedExpressionResolution::StageLook(_) => StageLook => Pending,
    CheckedExpressionResolution::Effect(_) => Effect => Pending,
    CheckedExpressionResolution::Call => Call => Accepted,
    CheckedExpressionResolution::Await(_) => Await => Pending,
    CheckedExpressionResolution::Choice(_) => Choice => Accepted,
    CheckedExpressionResolution::Try(_) => Try => Accepted,
    CheckedExpressionResolution::ImplicitCallable(_) => ImplicitCallable => Pending,
    CheckedExpressionResolution::Closure(_) => Closure => Accepted,
    CheckedExpressionResolution::ImplicitParameter(_) => ImplicitParameter => Pending,
    CheckedExpressionResolution::Pipe(_) => Pipe => Accepted,
    CheckedExpressionResolution::PipeLeft(_) => PipeLeft => Accepted,
    CheckedExpressionResolution::ViewCall(_) => ViewCall => Accepted,
    CheckedExpressionResolution::ViewFxApplication(_) => ViewFxApplication => Pending,
    CheckedExpressionResolution::StyleValue(_) => StyleValue => Pending,
    CheckedExpressionResolution::CompileTimeCallee(_) => CompileTimeCallee => Accepted,
    CheckedExpressionResolution::TypeValue(_) => TypeValue => Pending,
    CheckedExpressionResolution::CompileTimeScalar(_) => CompileTimeScalar => Pending,
    CheckedExpressionResolution::DialogueLineReference(_) => DialogueLineReference => Pending,
    CheckedExpressionResolution::DialogueLineCoordinate(_) => DialogueLineCoordinate => Pending,
    CheckedExpressionResolution::DialogueTextKeyCoordinate(_) => DialogueTextKeyCoordinate => Pending,
    CheckedExpressionResolution::CharacterDialogueFactory(_) => CharacterDialogueFactory => Pending,
    CheckedExpressionResolution::CharacterDialogueReconfigure(_) => CharacterDialogueReconfigure => Pending,
    CheckedExpressionResolution::DialogueApplication { .. } => DialogueApplication => Pending,
    CheckedExpressionResolution::ContentApplication(_) => ContentApplication => Pending,
    CheckedExpressionResolution::PostfixBracket(_) => PostfixBracket => Pending,
});

expression_family_inventory!(ValueResolutionFamily for CheckedValueResolution, {
    CheckedValueResolution::Local(_) => Local => Accepted,
    CheckedValueResolution::LineContext => LineContext => Pending,
    CheckedValueResolution::CharacterField { .. } => CharacterField => Pending,
    CheckedValueResolution::ProjectCallable(_) => ProjectCallable => Accepted,
    CheckedValueResolution::ProjectItem(_) => ProjectItem => Accepted,
    CheckedValueResolution::Entry(_) => Entry => Accepted,
    CheckedValueResolution::Registered(_) => Registered => Pending,
    CheckedValueResolution::Constant(_) => Constant => Pending,
});

expression_family_inventory!(SelectResolutionFamily for CheckedSelectResolution, {
    CheckedSelectResolution::Method(_) => Method => Accepted,
    CheckedSelectResolution::DialogueView { .. } => DialogueView => Accepted,
    CheckedSelectResolution::AgentField { .. } => AgentField => Pending,
    CheckedSelectResolution::ProgressField { .. } => ProgressField => Pending,
    CheckedSelectResolution::Field(_) => Field => Accepted,
});

#[derive(Default)]
struct MatchExpressionCorpusObservation {
    shapes: BTreeSet<ExprShapeFamily>,
    resolutions: BTreeSet<ExpressionResolutionFamily>,
    values: BTreeSet<ValueResolutionFamily>,
    selects: BTreeSet<SelectResolutionFamily>,
    value_facts: Vec<(CheckedValueResolution, TypeKind)>,
    semantic_digest: Option<[u8; 32]>,
}

impl MatchExpressionCorpusObservation {
    fn record_value(&mut self, value: &CheckedValueResolution) {
        self.values.insert(ValueResolutionFamily::of(value));
        if let CheckedValueResolution::CharacterField { receiver, .. } = value {
            self.record_value(receiver);
        }
    }

    fn record_resolution(&mut self, resolution: &CheckedExpressionResolution) {
        self.resolutions
            .insert(ExpressionResolutionFamily::of(resolution));
        if let CheckedExpressionResolution::Value(value) = resolution {
            self.record_value(value);
        }
        if let CheckedExpressionResolution::Select(select) = resolution {
            self.selects.insert(SelectResolutionFamily::of(select));
        }
        if let CheckedExpressionResolution::CompileTimeScalar(scalar) = resolution {
            self.record_resolution(scalar.original());
        }
        if let CheckedExpressionResolution::ImplicitCallable(callable) = resolution
            && let CheckedImplicitCallableBody::Plain(inner) = callable.body()
        {
            self.record_resolution(inner);
        }
    }
}

/// Traverse only expressions whose accepted path descends from one Match root.
/// Checked wrappers may retain a nested Value/Select resolution at that owner.
fn accepted_match_expression_corpus_observation(source: &str) -> MatchExpressionCorpusObservation {
    let world = super::fixture(source, None);
    accepted_match_expression_corpus_observation_for_fixture(&world)
}

fn accepted_match_expression_corpus_observation_for_fixture(
    world: &super::Fixture,
) -> MatchExpressionCorpusObservation {
    let report = super::analyze(world).expect("expression corpus source should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let match_owners = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    let [match_owner] = match_owners.as_slice() else {
        panic!("each expression corpus row has exactly one Match expression");
    };
    let product =
        super::checked_match_product(&report, project, module, &world.symbols, *match_owner);
    assert!(
        !product.arms().is_empty(),
        "accepted Match has checked arms"
    );

    let coordinates = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let root_path = coordinates
        .expression(*match_owner)
        .expect("accepted Match root path");
    let mut observation = MatchExpressionCorpusObservation {
        shapes: BTreeSet::new(),
        resolutions: BTreeSet::new(),
        values: BTreeSet::new(),
        selects: BTreeSet::new(),
        value_facts: Vec::new(),
        semantic_digest: Some(*product.semantic_digest().as_bytes()),
    };
    for (owner, hir) in module.expressions() {
        let Some(checked) = report.expression(owner) else {
            continue;
        };
        let path = coordinates
            .expression(owner)
            .expect("checked expression owner path");
        if path.root() != root_path.root() || !path.steps().starts_with(root_path.steps()) {
            continue;
        }
        observation.shapes.insert(ExprShapeFamily::of(hir.kind()));
        observation.record_resolution(checked.resolution());
        if let CheckedExpressionResolution::Value(value) = checked.resolution() {
            observation.value_facts.push((
                value.clone(),
                checked
                    .value_type()
                    .expect("checked Value expression has a type")
                    .clone(),
            ));
        }
    }
    assert!(observation.shapes.contains(&ExprShapeFamily::Match));
    observation
}

fn assert_expression_corpus_inventory<T: Copy + Ord + std::fmt::Debug>(
    axis: &str,
    observed: &BTreeSet<T>,
    inventory: &[(T, ExpressionCorpusDisposition)],
) {
    for (family, disposition) in inventory {
        assert_eq!(
            observed.contains(family),
            *disposition == ExpressionCorpusDisposition::Accepted,
            "{axis} {family:?} corpus disposition {disposition:?}",
        );
    }
}

struct ExpressionCorpusRow {
    name: &'static str,
    source: String,
    fixture: ExpressionCorpusFixture,
    shapes: &'static [ExprShapeFamily],
    resolutions: &'static [ExpressionResolutionFamily],
    values: &'static [ValueResolutionFamily],
    selects: &'static [SelectResolutionFamily],
}

#[derive(Clone, Copy)]
enum ExpressionCorpusFixture {
    Standard,
    ExternalCharacter,
}

impl ExpressionCorpusFixture {
    fn build(self, source: &str) -> super::Fixture {
        match self {
            Self::Standard => super::fixture(source, None),
            Self::ExternalCharacter => super::external_character_fixture(source),
        }
    }
}

fn view_method_corpus_row() -> ExpressionCorpusRow {
    ExpressionCorpusRow {
        name: "selected view method",
        source: method_match_source("", "Button().on_click { dialogue.primary_action }"),
        fixture: ExpressionCorpusFixture::Standard,
        shapes: &[
            ExprShapeFamily::Call,
            ExprShapeFamily::Select,
            ExprShapeFamily::Closure,
        ],
        resolutions: &[
            ExpressionResolutionFamily::Select,
            ExpressionResolutionFamily::Call,
            ExpressionResolutionFamily::Closure,
            ExpressionResolutionFamily::ViewCall,
            ExpressionResolutionFamily::CompileTimeCallee,
        ],
        values: &[],
        selects: &[
            SelectResolutionFamily::Method,
            SelectResolutionFamily::DialogueView,
        ],
    }
}

fn expression_corpus_rows() -> Vec<ExpressionCorpusRow> {
    let mut rows = vec![
        ExpressionCorpusRow {
            name: "binary scalar",
            source: bool_match_i64_source("1i64 + 2i64"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Match,
                ExprShapeFamily::Binary,
                ExprShapeFamily::Literal,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Structural,
                ExpressionResolutionFamily::Literal,
            ],
            values: &[ValueResolutionFamily::Local],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "compact numeric sequence",
            source: numeric_sequence_match_source("[1i64, 2i64]"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::NumericBracketSequence],
            resolutions: &[ExpressionResolutionFamily::Structural],
            values: &[ValueResolutionFamily::Local],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "local and project callable values",
            source: callable_value_match_source("saved"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::Block, ExprShapeFamily::Path],
            resolutions: &[ExpressionResolutionFamily::Value],
            values: &[
                ValueResolutionFamily::Local,
                ValueResolutionFamily::ProjectCallable,
            ],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "record field",
            source: field_match_source("left"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::Select],
            resolutions: &[ExpressionResolutionFamily::Select],
            values: &[],
            selects: &[SelectResolutionFamily::Field],
        },
        view_method_corpus_row(),
        ExpressionCorpusRow {
            name: "checked Try carrier",
            source: try_match_source("first"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::Try],
            resolutions: &[ExpressionResolutionFamily::Try],
            values: &[],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "pipeline placeholders",
            source: pipe_match_source("identity"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Pipe,
                ExprShapeFamily::Placeholder,
                ExprShapeFamily::Tuple,
                ExprShapeFamily::Call,
            ],
            resolutions: &[
                ExpressionResolutionFamily::Pipe,
                ExpressionResolutionFamily::PipeLeft,
                ExpressionResolutionFamily::Call,
            ],
            values: &[ValueResolutionFamily::ProjectCallable],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "Choice plan with thread value",
            source: choice_match_source("", "with { window = thread {} }"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[
                ExprShapeFamily::Choice,
                ExprShapeFamily::Thread,
                ExprShapeFamily::Unit,
            ],
            resolutions: &[ExpressionResolutionFamily::Choice],
            values: &[],
            selects: &[],
        },
    ];
    rows.extend(checked_owner_expression_corpus_rows());
    rows
}

fn checked_owner_expression_corpus_rows() -> Vec<ExpressionCorpusRow> {
    vec![
        ExpressionCorpusRow {
            name: "checked Entry references",
            source: entry_reference_match_source("@entry.cli.primary", "@entry.cli.alternate"),
            fixture: ExpressionCorpusFixture::Standard,
            shapes: &[ExprShapeFamily::Match, ExprShapeFamily::EntityReference],
            resolutions: &[ExpressionResolutionFamily::Structural],
            values: &[ValueResolutionFamily::Entry],
            selects: &[],
        },
        ExpressionCorpusRow {
            name: "checked external and project Character items",
            source: project_item_match_source("@character.akane", "@character.alternate"),
            fixture: ExpressionCorpusFixture::ExternalCharacter,
            shapes: &[ExprShapeFamily::Match, ExprShapeFamily::EntityReference],
            resolutions: &[ExpressionResolutionFamily::Structural],
            values: &[ValueResolutionFamily::ProjectItem],
            selects: &[],
        },
    ]
}

#[test]
fn checked_match_expression_corpus_tracks_accepted_root_families() {
    let mut observed = MatchExpressionCorpusObservation::default();
    for row in expression_corpus_rows() {
        let world = row.fixture.build(&row.source);
        let found = accepted_match_expression_corpus_observation_for_fixture(&world);
        for required in row.shapes {
            assert!(
                found.shapes.contains(required),
                "{} needs {required:?}",
                row.name
            );
        }
        for required in row.resolutions {
            assert!(
                found.resolutions.contains(required),
                "{} needs {required:?}",
                row.name
            );
        }
        for required in row.values {
            assert!(
                found.values.contains(required),
                "{} needs {required:?}",
                row.name
            );
        }
        for required in row.selects {
            assert!(
                found.selects.contains(required),
                "{} needs {required:?}",
                row.name
            );
        }
        observed.shapes.extend(found.shapes);
        observed.resolutions.extend(found.resolutions);
        observed.values.extend(found.values);
        observed.selects.extend(found.selects);
    }
    assert_expression_corpus_inventory(
        "HIR expression",
        &observed.shapes,
        ExprShapeFamily::INVENTORY,
    );
    assert_expression_corpus_inventory(
        "checked expression",
        &observed.resolutions,
        ExpressionResolutionFamily::INVENTORY,
    );
    assert_expression_corpus_inventory(
        "checked Value",
        &observed.values,
        ValueResolutionFamily::INVENTORY,
    );
    assert_expression_corpus_inventory(
        "checked Select",
        &observed.selects,
        SelectResolutionFamily::INVENTORY,
    );
}

#[test]
fn checked_match_transcript_commits_entry_value_owner() {
    let entry_world = ExpressionCorpusFixture::Standard.build(&entry_reference_match_source(
        "@entry.cli.primary",
        "@entry.cli.alternate",
    ));
    let entries = accepted_match_expression_corpus_observation_for_fixture(&entry_world);
    let checked_entries = entries
        .value_facts
        .iter()
        .filter_map(|(value, ty)| match value {
            CheckedValueResolution::Entry(entry) => Some((entry, ty)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let entry_ids = checked_entries
        .iter()
        .map(|(entry, ty)| {
            assert_eq!(*ty, &TypeKind::entity_ref(EntityKind::Entry));
            assert_eq!(
                entry.value_type(),
                TypeKind::entity_ref(EntityKind::Entry)
                    .semantic_identity_digest()
                    .expect("Entry reference type has a stable identity"),
            );
            entry.diagnostic_public_id().as_str().to_owned()
        })
        .collect::<BTreeSet<_>>();
    let entry_bindings = checked_entries
        .iter()
        .map(|(entry, _)| *entry.binding().as_bytes())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entry_ids,
        ["entry.cli.primary", "entry.cli.alternate"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        "both arm values resolve through the accepted Entry catalog",
    );
    assert_eq!(
        entry_bindings.len(),
        2,
        "each Entry has its own checked binding"
    );

    let same_entry_world = ExpressionCorpusFixture::Standard.build(&entry_reference_match_source(
        "@entry.cli.primary",
        "@entry.cli.primary",
    ));
    let same_entry = accepted_match_expression_corpus_observation_for_fixture(&same_entry_world);
    assert_ne!(
        entries.semantic_digest, same_entry.semantic_digest,
        "changing one Match arm's checked Entry owner changes its transcript",
    );
}

#[test]
fn checked_match_transcript_commits_project_item_value_owner() {
    let project_item_world = ExpressionCorpusFixture::ExternalCharacter.build(
        &project_item_match_source("@character.akane", "@character.alternate"),
    );
    let project_items =
        accepted_match_expression_corpus_observation_for_fixture(&project_item_world);
    let checked_items = project_items
        .value_facts
        .iter()
        .filter_map(|(value, ty)| match value {
            CheckedValueResolution::ProjectItem(item) => Some((item, ty)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let character_type = TypeKind::entity_ref(EntityKind::Character);
    let project_item_ids = checked_items
        .iter()
        .map(|(item, ty)| {
            assert_eq!(*ty, &character_type);
            assert_eq!(
                item.value_type(),
                character_type
                    .semantic_identity_digest()
                    .expect("Character reference type has a stable identity")
            );
            item.semantic_id()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        project_item_ids.len(),
        2,
        "the arms retain distinct checked Character owners"
    );
    let external = checked_items
        .iter()
        .find(|(item, _)| item.public_id().as_str() == "character.akane")
        .expect("external registered Character item");
    assert!(external.0.external_declaration().is_some());
    let retained = checked_items
        .iter()
        .find(|(item, _)| item.public_id().as_str() == "character.alternate")
        .expect("retained project Character item");
    assert!(retained.0.retained_owner().is_some());

    let same_project_item_world = ExpressionCorpusFixture::ExternalCharacter.build(
        &project_item_match_source("@character.akane", "@character.akane"),
    );
    let same_project_item =
        accepted_match_expression_corpus_observation_for_fixture(&same_project_item_world);
    assert_ne!(
        project_items.semantic_digest, same_project_item.semantic_digest,
        "changing one Match arm's checked ProjectItem owner changes its transcript",
    );
}

#[test]
fn checked_match_expression_corpus_excludes_unrelated_declaration_expressions() {
    let source = format!(
        "fn unrelated() -> i64 {{ let outside = 0i64..1i64; 0i64 }}\n{}",
        bool_match_i64_source("1i64 + 2i64"),
    );
    let world = super::fixture(&source, None);
    let report = super::analyze(&world).expect("unrelated range should check");
    let project = world.project.analysis_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    assert!(module.expressions().any(|(owner, expression)| {
        matches!(expression.kind(), HirExprKind::Range(_)) && report.expression(owner).is_some()
    }));
    let observation = accepted_match_expression_corpus_observation(&source);
    assert!(observation.shapes.contains(&ExprShapeFamily::Binary));
    assert!(
        !observation.shapes.contains(&ExprShapeFamily::Range),
        "an unrelated checked Range must not enter the Match corpus",
    );
}
