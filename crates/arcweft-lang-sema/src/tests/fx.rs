use super::support::*;

fn typecheck(source: &str) -> Result<(), Vec<crate::diagnostics::TypeCheckError>> {
    let tree = parse_ok(source);
    let hir = lower_document_to_hir(tree.document(), tree.typed_tree()).expect("Fx fixture lowers");
    typecheck_hir(&hir, &TypeCheckEnv::standard())
}

#[test]
fn fx_function_and_reactive_view_application_typecheck() {
    typecheck(
        r#"
#[fx]
fn notice(accent: Color, amplitude: Length = 2px) -> Fx {
    Fx.text(color = accent)
}

view Warning(state: WarningState) {
    Text("WARNING")
        .fx(notice(accent = state.warning_color))
}
"#,
    )
    .expect("typed Fx factory and reactive View binding typecheck");
}

#[test]
fn unregistered_function_attributes_do_not_register_fx_factories() {
    typecheck(
        r"
#[project_extension]
fn ordinary() -> Unit {}
",
    )
    .expect("an open extension attribute does not give an ordinary function Fx semantics");
}

#[test]
fn view_fx_rejects_unknown_definition() {
    let errors = typecheck(
        r#"
view Warning() {
    Text("WARNING").fx(missing(accent = "red"))
}
"#,
    )
    .expect_err("unknown View Fx is rejected");

    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("unknown Fx function `missing` in View `.fx(...)`")
    }));
}

#[test]
fn view_fx_rejects_missing_required_argument() {
    let errors = typecheck(
        r#"
#[fx]
fn notice(accent: Color) -> Fx {
    Fx.text(color = accent)
}

view Warning() {
    Text("WARNING").fx(notice())
}
"#,
    )
    .expect_err("missing required View Fx argument is rejected");

    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("Fx function `notice` is missing required argument `accent`")
    }));
}

#[test]
fn view_fx_rejects_positional_arguments_before_semantic_lowering() {
    let errors = parse_errors(
        r#"
#[fx]
fn notice(accent: Color) -> Fx {
    Fx.text(color = accent)
}

view Warning() {
    Text("WARNING").fx(notice("red"))
}
"#,
    );

    assert_eq!(
        errors
            .iter()
            .filter(|error| error.message().contains("named-only"))
            .count(),
        1
    );
    assert!(
        errors
            .iter()
            .all(|error| !error.message().contains("unsupported View modifier"))
    );
}

#[test]
fn fx_unit_arithmetic_accepts_scalar_scaling() {
    typecheck(
        r"
#[fx]
fn scaled(amplitude: Length, scale: f32) -> Fx {
    Fx.transform(
        target = .glyph,
        sample = |ctx| Transform2D {
            translate_y: sin(ctx.time) * scale * amplitude,
        },
    )
}
",
    )
    .expect("dimensionless scaling preserves the Length result");
}

#[test]
fn fx_unit_arithmetic_rejects_dimension_erasing_division_without_normalization() {
    let errors = typecheck(
        r"
#[fx]
fn ratio() -> Fx {
    Fx.transform(
        target = .glyph,
        sample = |ctx| Transform2D {
            translate_y: (2px / 1em) * 1px,
        },
    )
}
",
    )
    .expect_err("Length division requires an explicit normalization rule");

    assert!(
        errors
            .iter()
            .any(|error| { error.message().contains("found Length and Length") })
    );
}

#[test]
fn arithmetic_rejects_unimplemented_unit_and_duration_combinations() {
    for (expression, operands) in [
        ("2px + 1em", "Length and Length"),
        ("1s * 2s", "Duration and Duration"),
    ] {
        let errors = typecheck(&format!(
            r"
fn invalid() -> Unit {{
    let value = {expression}
}}
"
        ))
        .expect_err("unsupported dimensional arithmetic is rejected");

        assert!(
            errors
                .iter()
                .any(|error| { error.message().contains(&format!("found {operands}")) })
        );
    }
}

#[test]
fn ordinary_fx_call_uses_shared_signature_diagnostics_once() {
    let errors = typecheck(
        r"
#[fx]
fn notice(accent: Color) -> Fx {
    Fx.text(color = accent)
}

fn invalid() -> Fx {
    notice()
}
",
    )
    .expect_err("missing Fx argument is rejected");

    assert_eq!(
        errors
            .iter()
            .filter(|error| error
                .message()
                .contains("missing required argument `accent`"))
            .count(),
        1
    );
}

#[test]
fn point_controls_do_not_open_spans_inside_fx() {
    typecheck(
        r#"
character narrator {
    display = "Narrator"
    default_voice = auto
}

#[fx]
fn notice() -> Fx {
    Fx.text(weight = .strong)
}

flow main {
    narrator[ [fx notice()][w 500ms]warning[r][/fx][p] ]
}
"#,
    )
    .expect("point controls remain atomic inside an Fx span");
}

#[test]
fn explicit_rich_text_tags_are_not_reclassified_as_inferred_marks() {
    typecheck(
        r#"
character narrator {
    display = "Narrator"
    default_voice = auto
}

flow main {
    narrator[ [effect .wave][color #ff4050][strong][em][size 42]warning[/size][/em][/strong][/color][/effect][p] ]
}
"#,
    )
    .expect("explicit empty-attribute spans retain their authored nesting");
}

#[test]
fn view_and_rich_text_fx_calls_reject_incompatible_closed_arguments() {
    let errors = typecheck(
        r#"
#[fx]
fn notice(accent: Color) -> Fx {
    Fx.text(color = accent)
}

view Warning() {
    Text("WARNING").fx(notice(accent = "red"))
}

flow main {
    narrator[ [fx notice(accent="red")]warning[/fx][p] ]
}
"#,
    )
    .expect_err("closed Fx arguments are checked against the function schema");

    assert_eq!(
        errors
            .iter()
            .filter(|error| error.message().contains("must have type Color"))
            .count(),
        2
    );
}
