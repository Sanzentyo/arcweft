use super::*;

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
