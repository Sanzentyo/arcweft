use super::support::*;
use std::fmt::Write as _;

fn decoration_errors(source: &str) -> Vec<String> {
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("decoration fixture lowers");
    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    )
    .expect_err("decoration fixture should fail semantic validation")
    .into_iter()
    .map(|error| error.message().to_owned())
    .collect()
}

#[test]
fn decorations_accept_defaults_nested_composition_and_explicit_custom_arguments() {
    let tree = parse_ok(
        r##"
decoration accent(accent = "#ff4050", amplitude = 2px, ...effect_args) {
    strong()
    color(value=accent)
    effect(.wave, amp=amplitude, effect_args...)
}

decoration warning(label, ...effect_args) {
    decorate(.accent, effect_args...)
    effect(.shake, seed=label)
}

decoration delayed(delay = -250ms) {
    effect(.wave, delay=delay)
}

decoration localized(色 = "red", ...追加) {
    color(色)
    effect(.wave, 追加...)
}

flow @flow.opening opening {
    alice: [decorate .accent speed=2]Default[/decorate].[p]
    alice: [decorate .warning label=warning amount=4px]Nested[/decorate].[p]
    alice: [decorate .delayed delay=-1s]Negative duration[/decorate].[p]
    alice: [decorate .localized 色=青 揺れ=2px]Unicode bindings[/decorate].[p]
}
"##,
    );
    let hir = lower_to_hir(&tree).expect("valid decorations lower");

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    )
    .expect("valid decoration declarations and calls typecheck");
}

#[test]
fn expression_dialogue_calls_validate_typed_decoration_content_in_sema() {
    let errors = decoration_errors(
        r"
flow @flow.expression_dialogue expression_dialogue {
    let line = alice.say()[[decorate .missing]text[/decorate][p]]
}
",
    );

    assert!(
        errors
            .iter()
            .any(|message| message.contains("unknown decoration `missing`")),
        "{errors:?}"
    );
}

#[test]
fn expression_dialogue_calls_retain_text_mode_diagnostics_for_sema() {
    let errors = decoration_errors(
        r"
flow @flow.expression_dialogue expression_dialogue {
    let line = alice.say()[|bad base{ruby}[p]]
}
",
    );

    assert!(
        errors
            .iter()
            .any(|message| message.contains("invalid compact ruby")),
        "{errors:?}"
    );
}

#[test]
fn decoration_invocations_reject_character_literals_as_non_presentation_constants() {
    let errors = decoration_errors(
        r#"
decoration wave(seed) {
    effect(.wave, seed=seed)
}

flow @flow.character_value character_value {
    alice: [decorate .wave seed="x"c]text[/decorate][p]
}
"#,
    );

    assert!(
        errors.iter().any(|message| message
            .contains("decoration `wave` argument `seed` must be a compile-time closed value")),
        "{errors:?}"
    );
}

#[test]
fn decoration_builders_accept_only_their_canonical_argument_shapes() {
    let tree = parse_ok(
        r#"
decoration canonical(scalar = 2px, ...custom) {
    em()
    strong()
    color(scalar)
    font(value="serif")
    size(value=scalar)
    style(.italic)
    layout(.vertical_rl, jlreq=.strict)
    transform(.offset, x=scalar, custom...)
    effect(.wave, amp=scalar, custom...)
}

decoration bound_phase(phase) {
    effect(.wave, phase=phase)
}

decoration required_name_is_not_a_value(host_event) {
    effect(.wave, phase=host_event)
}

flow @flow.builder_shapes builder_shapes {
    alice: [decorate .bound_phase phase=glyph_transform]Visual[/decorate].[p]
    alice: [decorate .required_name_is_not_a_value host_event=glyph_transform]Named required parameter[/decorate].[p]
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("canonical decoration builders lower");

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    )
    .expect("canonical decoration builder argument shapes typecheck");
}

#[test]
fn decoration_builders_reject_malformed_argument_shapes_before_runtime_lowering() {
    let errors = decoration_errors(
        r#"
decoration leaf(...custom) {
    effect(.wave, custom...)
}

decoration malformed(scalar = 2px, fixed = 1, ...custom) {
    strong(1)
    strong(custom...)
    em(value=1)
    color()
    font(value="serif", value="mono")
    size(points=12px)
    style()
    layout(vertical_rl)
    transform(.offset, 1px)
    effect()
    effect(.wave, amp=1, amp=2)
    effect(.wave, fixed...)
    effect(.wave, custom..., custom...)
    decorate(.leaf, custom..., custom...)
    style(.unknown_style)
    layout(.unknown_layout)
    transform(.unknown_transform)
    effect(.unknown_effect)
}
"#,
    );

    for expected in [
        "builder `strong` does not accept arguments",
        "builder `em` does not accept arguments",
        "builder `color` requires exactly one scalar",
        "builder `font` repeats named argument `value`",
        "builder `font` requires exactly one scalar",
        "builder `size` requires exactly one scalar",
        "builder `style` requires one leading `.name` selector",
        "builder `layout` selector must use `.name` syntax",
        "builder `transform` does not accept positional arguments after its selector",
        "builder `effect` requires one leading `.name` selector",
        "builder `effect` repeats named argument `amp`",
        "parameter `fixed` is not a rest parameter",
        "builder `effect` may spread its rest parameter at most once",
        "nested decoration `leaf` in `malformed` may spread its rest parameter at most once",
        "builder `style` has unknown selector `.unknown_style`",
        "builder `layout` has unknown selector `.unknown_layout`",
        "builder `transform` has unknown selector `.unknown_transform`",
    ] {
        assert!(
            errors.iter().any(|message| message.contains(expected)),
            "missing `{expected}` in {errors:?}"
        );
    }
    assert!(
        errors
            .iter()
            .all(|message| !message.contains("unknown selector `.unknown_effect`")),
        "effect identifiers must remain registry-extensible: {errors:?}"
    );
}

#[test]
fn decoration_calls_validate_bound_values_through_nested_and_rest_expansion() {
    let errors = decoration_errors(
        r"
decoration dynamic_phase(phase) {
    effect(.wave, phase=phase)
}

decoration nested_phase(phase) {
    decorate(.dynamic_phase, phase=phase)
}

decoration rest_collision(...custom) {
    effect(.wave, amp=1, custom...)
}

flow @flow.opening opening {
    alice: [decorate .dynamic_phase phase=host_event]Invalid event[/decorate].[p]
    alice: [decorate .nested_phase phase=host_event]Invalid nested event[/decorate].[p]
    alice: [decorate .rest_collision amp=2]Duplicate explicit/rest sink[/decorate].[p]
}
",
    );

    assert!(
        errors
            .iter()
            .filter(|message| message
                .contains("decoration `dynamic_phase` expands an effect with `phase=host_event`"))
            .count()
            >= 2,
        "direct and nested bound host-event phases must both be rejected: {errors:?}"
    );
    assert!(errors.iter().any(|message| {
        message.contains(
            "decoration `rest_collision` builder `effect` receives duplicate bound argument `amp`",
        )
    }));
}

#[test]
fn decoration_declarations_reject_invalid_parameters_and_builder_bodies() {
    let errors = decoration_errors(
        r"
decoration duplicate(value, value) {
    strong()
}

decoration duplicate() {
    strong()
}

decoration empty() {
}

decoration invalid_rest(...custom, later) {
    effect(.wave, custom...)
}

decoration multiple_rest(...first, ...second) {
    effect(.wave, first...)
}

decoration defaulted_rest(...custom = raw) {
    effect(.wave, custom...)
}

decoration dynamic_default(accent = state.accent) {
    color(value=accent)
}

decoration invalid_body(value) {
    object(.hotspot)
    speed(24)
    p()
    unknown(value=value)
    color(value=missing)
    effect(.host, phase=host_event)
}
",
    );

    for expected in [
        "duplicate decoration declaration `duplicate`",
        "decoration `empty` must contain at least one visual builder layer",
        "duplicate parameter `value` in decoration `duplicate`",
        "rest parameter `custom` must be final",
        "can declare at most one rest parameter",
        "rest parameter `custom` cannot declare a default value",
        "default for decoration `dynamic_default` parameter `accent` must be a compile-time closed value",
        "decoration `invalid_body` cannot use `object`",
        "decoration `invalid_body` cannot use `speed`",
        "decoration `invalid_body` cannot use `p`",
        "unsupported decoration builder `unknown`",
        "unknown decoration parameter `missing`",
        "cannot hide an effect with `phase=host_event`",
    ] {
        assert!(
            errors.iter().any(|message| message.contains(expected)),
            "missing `{expected}` in {errors:?}"
        );
    }
}

#[test]
fn decorations_reject_unknown_nested_references_and_cycles() {
    let errors = decoration_errors(
        r#"
decoration missing_child() {
    decorate(.unknown)
}

decoration required_child(required) {
    strong()
}

decoration invalid_nested(...custom) {
    decorate(.required_child)
    decorate(.required_child, required="one", required="two")
    decorate(.required_child, unknown="value")
    decorate(.required_child, custom...)
}

decoration first() {
    decorate(.second)
}

decoration second() {
    decorate(.third)
}

decoration third() {
    decorate(.first)
}
"#,
    );

    assert!(errors.iter().any(|message| {
        message.contains("decoration `missing_child` references unknown decoration `unknown`")
    }));
    assert!(errors.iter().any(|message| {
        message.contains(
            "nested decoration `required_child` argument `required` was provided more than once",
        )
    }));
    assert!(errors.iter().any(|message| {
        message.contains("nested decoration `required_child` has no parameter named `unknown`")
    }));
    assert!(errors.iter().any(|message| {
        message.contains("nested decoration `required_child` in `invalid_nested` cannot accept this custom argument spread")
    }));
    assert!(errors.iter().any(|message| {
        message.contains("decoration `required_child` is missing required argument `required`")
    }));
    assert!(errors.iter().any(|message| {
        message.contains("decoration composition cycle")
            && message.contains("first")
            && message.contains("second")
            && message.contains("third")
    }));
}

#[test]
fn decoration_calls_reject_invalid_selectors_and_argument_binding() {
    let errors = decoration_errors(
        r##"
decoration strict(required, accent = "#ff4050") {
    strong()
    color(value=accent)
}

decoration flexible(required = ok, ...custom) {
    effect(.wave, custom...)
}

flow @flow.opening opening {
    alice: [decorate]Missing selector[/decorate].[p]
    alice: [decorate strict]Invalid selector[/decorate].[p]
    alice: [decorate ".strict"]Quoted selector[/decorate].[p]
    alice: [decorate .strict one two]Extra positional[/decorate].[p]
    alice: [decorate .strict required=one required=two]Duplicate[/decorate].[p]
    alice: [decorate .strict]Missing required[/decorate].[p]
    alice: [decorate .strict required=ok unknown=1]Unknown[/decorate].[p]
    alice: [decorate .strict required=state.value]Dynamic[/decorate].[p]
    alice: [decorate .flexible required=ok 色=状態.色]Unicode dynamic path[/decorate].[p]
    alice: [decorate .missing]Unknown decoration[/decorate].[p]
    alice: [decorate .flexible custom_name="kept"]Rest accepts custom[/decorate].[p]
}
"##,
    );

    for expected in [
        "`[decorate]` requires a `.name` selector",
        "decoration selector `strict` must use `.name` syntax",
        "must use unquoted `.name` syntax",
        "decoration `strict` does not accept positional arguments",
        "decoration `strict` argument `required` was provided more than once",
        "decoration `strict` is missing required argument `required`",
        "decoration `strict` has no parameter named `unknown`",
        "decoration `strict` argument `required` must be a compile-time closed value",
        "decoration `flexible` argument `色` must be a compile-time closed value",
        "unknown decoration `missing`",
    ] {
        assert!(
            errors.iter().any(|message| message.contains(expected)),
            "missing `{expected}` in {errors:?}"
        );
    }
    assert!(
        errors
            .iter()
            .all(|message| !message.contains("flexible") || !message.contains("custom_name")),
        "rest-bound custom argument should be accepted: {errors:?}"
    );
}

#[test]
fn decoration_calls_reject_non_identifier_custom_argument_names() {
    let errors = decoration_errors(
        r"
decoration flexible(...custom) {
    effect(.wave, custom...)
}

flow @flow.invalid_argument_names invalid_argument_names {
    alice: [decorate .flexible =x]Empty[/decorate].[p]
    alice: [decorate .flexible 1bad=x]Leading digit[/decorate].[p]
    alice: [decorate .flexible bad-name=x]Punctuation[/decorate].[p]
}
",
    );

    for invalid in ["", "1bad", "bad-name"] {
        let expected_name = format!("argument name `{invalid}`");
        assert!(
            errors.iter().any(|message| {
                message.contains(&expected_name) && message.contains("canonical identifier")
            }),
            "missing invalid argument-name diagnostic for {invalid:?}: {errors:?}"
        );
    }
}

#[test]
fn decoration_spans_reject_unmatched_and_unclosed_end_tags() {
    let errors = decoration_errors(
        r"
decoration strong_warning() {
    strong()
}

flow @flow.opening opening {
    alice: [decorate .strong_warning]Nested [decorate .strong_warning]ok[/decorate][/decorate].[p]
    alice: stray[/decorate].[p]
    alice: [decorate .strong_warning]unclosed[p]
}
",
    );

    assert!(
        errors
            .iter()
            .any(|message| message.contains("unmatched `[/decorate]`"))
    );
    assert!(
        errors
            .iter()
            .any(|message| message.contains("unclosed `[decorate .strong_warning]` span"))
    );
}

#[test]
fn decoration_spans_reject_crossing_closes_inferred_close_and_reset() {
    let errors = decoration_errors(
        r"
decoration atomic() {
    strong()
}

flow @flow.atomic_spans atomic_spans {
    alice: [decorate .atomic][strong]cross[/decorate][/strong].[p]
    alice: [decorate .atomic]internal[/strong][/decorate].[p]
    alice: [decorate .atomic]inferred[/][/decorate].[p]
    alice: [decorate .atomic]reset[reset][/decorate].[p]
    alice: [decorate .atomic][.wave]inferred style[/decorate][/].[p]
}
",
    );

    for expected in [
        "`[/decorate]` crosses a rich-text span opened inside the decoration",
        "`[/strong]` cannot close an internal layer of an open decoration",
        "an explicit `[decorate ...]` span must close with `[/decorate]`",
        "`[reset]` cannot clear styles from inside an open decoration span",
    ] {
        assert!(
            errors.iter().any(|message| message.contains(expected)),
            "missing `{expected}` in {errors:?}"
        );
    }
}

#[test]
fn decoration_span_state_uses_shared_inferred_aliases_and_ignores_speed_controls() {
    let tree = parse_ok(
        r"
decoration atomic() {
    strong()
}

flow @flow.atomic_aliases atomic_aliases {
    alice: [decorate .atomic][.italic]italic[/][/decorate].[p]
    alice: [decorate .atomic][.oblique]oblique[/][/decorate].[p]
    alice: [decorate .atomic][.vertical_rl]vertical[/][/decorate].[p]
    alice: [decorate .atomic][.skew x=2deg]skew[/skew][/decorate].[p]
    alice: [decorate .atomic][speed 24]speed control[/decorate].[p]
}
",
    );
    let hir = lower_to_hir(&tree).expect("valid decoration spans lower");

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    )
    .expect("canonical inferred spans and speed controls preserve decoration nesting");
}

#[test]
fn decoration_span_state_does_not_read_phase_assignments_from_quoted_values() {
    let tree = parse_ok(
        r#"
decoration atomic() {
    strong()
}

flow @flow.atomic_quoted_attrs atomic_quoted_attrs {
    alice: [decorate .atomic][effect .wave note="not phase=host_event"]explicit[/effect][/decorate].[p]
    alice: [decorate .atomic][.wave note="not phase=host_event"]inferred[/][/decorate].[p]
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("valid decoration spans lower");

    typecheck_hir(
        &hir,
        &TypeCheckEnv::new().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character)),
    )
    .expect("quoted custom values must not masquerade as host-event phase arguments");
}

#[test]
fn decoration_span_state_tracks_inferred_text_proxy_objects() {
    let errors = decoration_errors(
        r#"
#[text_proxy(kind="keyword", default_hit=true)]
pub struct KeywordHit {
    channel: String
}

decoration atomic() {
    strong()
}

flow @flow.atomic_proxy atomic_proxy {
    alice: [decorate .atomic][.KeywordHit]valid[/][/decorate].[p]
    alice: [decorate .atomic][.KeywordHit]cross[/decorate][/].[p]
}
"#,
    );

    assert!(errors.iter().any(|message| {
        message.contains("`[/decorate]` crosses a rich-text span opened inside the decoration")
    }));
}

#[test]
fn decoration_expansion_rejects_excessive_composition_depth_without_recursive_cycle_walk() {
    let mut source = String::from("decoration chain_0() { strong() }\n");
    for index in 1..2_048 {
        writeln!(
            &mut source,
            "decoration chain_{index}() {{ decorate(.chain_{}) }}",
            index - 1
        )
        .expect("writing a decoration fixture to String cannot fail");
    }

    let errors = decoration_errors(&source);
    assert!(
        errors.iter().any(|message| {
            message.contains("expansion exceeds maximum composition depth of 64")
        })
    );
}

#[test]
fn decoration_expansion_rejects_excessive_expanded_layers() {
    let mut source = String::from("decoration layer_0() { strong() }\n");
    for index in 1..=13 {
        writeln!(
            &mut source,
            "decoration layer_{index}() {{\ndecorate(.layer_{0})\ndecorate(.layer_{0})\n}}",
            index - 1
        )
        .expect("writing a decoration fixture to String cannot fail");
    }

    let errors = decoration_errors(&source);
    assert!(errors.iter().any(|message| {
        message.contains("expansion exceeds maximum expanded layer count of 4096")
    }));
}

#[test]
fn decoration_expansion_rejects_excessive_repeated_visits_independently_of_layers() {
    let mut source = String::from("decoration visit_chain_0() { strong() }\n");
    for index in 1..=62 {
        writeln!(
            &mut source,
            "decoration visit_chain_{index}() {{ decorate(.visit_chain_{}) }}",
            index - 1
        )
        .expect("writing a decoration fixture to String cannot fail");
    }
    source.push_str("decoration visit_root() {\n");
    for _ in 0..300 {
        source.push_str("decorate(.visit_chain_62)\n");
    }
    source.push_str("}\n");

    let errors = decoration_errors(&source);
    assert!(
        errors
            .iter()
            .any(|message| { message.contains("expansion exceeds maximum visit count of 16384") })
    );
}
