use super::support::*;

fn assert_unknown_presentation_argument(call: &str, command: &str, argument: &str) {
    let tree = parse_ok(format!("flow main {{\n    {call}\n}}\n"));
    let hir = lower_to_hir(&tree).expect("presentation fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("unknown presentation argument must be rejected");

    assert!(
        errors.iter().any(|error| {
            matches!(
                error.kind(),
                TypeCheckErrorKind::UnknownPresentationArgument {
                    command: actual_command,
                    argument: actual_argument,
                } if actual_command == command && actual_argument == argument
            ) && error.stable_code() == "sema.presentation.unknown_argument"
        }),
        "missing structured unknown-argument diagnostic for `{call}`: {errors:#?}"
    );
}

#[test]
fn canonical_presentation_command_and_argument_spellings_typecheck() {
    let tree = parse_ok(
        r#"
flow main {
    player_viewport(width = 1280px, height = 720px, fit = "contain")
    bg(@asset:.bg.room, fade = 100ms, fit = "cover", alignment.x = 0.5, alignment.y = 500, opacity = 0.75, playback.start = 10ms, playback.paused_at = 20ms, playback.local_time = 30ms, playback.rate = 1.0)
    image(asset = @asset:.bg.pulse, id = "image.pulse", target = @target.pulse, layer = @layer.foreground, x = 1px, y = 2px, width = 3px, height = 4px, fit = "stretch", alignment.x = 0.25, alignment.y = 750, opacity = 0.5, playback.start = 10ms, playback.paused_at = 20ms, playback.local_time = 30ms, playback.rate = 0.5, transform.tx = 1px, transform.ty = 2px, transform.m11 = 1000, transform.m12 = 0, transform.m21 = 0, transform.m22 = 1000, depth = 10, enabled = true, visible = true, action = "action.inspect", actions = "action.inspect", param.role = "sprite", proxy.id = "proxy.pulse", proxy.type = "Pulse", proxy.role = "inspect", proxy.layer = @layer.hit, proxy.depth = 11, proxy.hit_test = true, proxy.param.channel = "preview", lifetime = .manual, focus = .Pass, input_capture = .None, owner = "main", drop = .release)
    view(@view:.Panel, layer = @layer.ui, depth = 20, visible = true, enabled = true, lifetime = .manual, focus = .Trap, input_capture = .Modal, owner = "main", drop = .release)
    show(@character.alice, .smile, at = .center, fade = 200ms, scale = 1.05, z = 10, layer = @layer.characters)
    hide(@character.alice, fade = 180ms)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("canonical presentation fixture lowers");

    typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect("canonical presentation commands and arguments typecheck");
}

#[test]
fn arbitrary_unknown_presentation_callees_use_normal_resolution_errors() {
    for (callee, call) in [
        (
            "mystery_presentation",
            "mystery_presentation(@asset:.bg.room, target = @target.scene)",
        ),
        (
            "mystery.image",
            "mystery.image(asset = @asset:.bg.room, id = \"image.room\")",
        ),
    ] {
        let tree = parse_ok(format!("flow main {{\n    {call}\n}}\n"));
        let hir = lower_to_hir(&tree).expect("unknown callee fixture lowers");
        let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
            .expect_err("unknown presentation callee must be rejected");

        assert!(
            errors.iter().any(|error| {
                matches!(error.kind(), TypeCheckErrorKind::Message)
                    && (error.message().contains("unknown symbol")
                        || error.message().contains("unknown function")
                        || error.message().contains("unknown method"))
            }),
            "missing normal unresolved-callee diagnostic for `{callee}` in `{call}`: {errors:#?}"
        );
    }
}

#[test]
fn malformed_presentation_argument_name_is_a_parser_error() {
    let errors = parse_errors("flow main {\n    player_viewport(mystery-name = true)\n}\n");
    assert!(!errors.is_empty(), "malformed argument must be rejected");
    assert!(
        errors.iter().all(|error| error.code() == "syntax.parse"),
        "malformed argument must remain a structured parser rejection: {errors:#?}"
    );
}

#[test]
fn arbitrary_unknown_arguments_use_the_same_presentation_diagnostic() {
    for (call, command) in [
        ("bg(@asset:.bg.room, mystery = true)", "bg"),
        ("image(asset = @asset:.bg.room, mystery = true)", "image"),
        ("player_viewport(mystery = true)", "player_viewport"),
    ] {
        assert_unknown_presentation_argument(call, command, "mystery");
    }
}

#[test]
fn presentation_named_numeric_arguments_preserve_schema_types_in_authored_order() {
    let tree = parse_ok(
        r"
flow main {
    image(asset = @asset:.bg.pulse, depth = 7, opacity = 0.5, param.count = 9, proxy.depth = 11)
}
",
    );
    let hir = lower_to_hir(&tree).expect("mixed numeric presentation fixture lowers");
    let report = analyze_types(&hir, &TypeCheckEnv::new());
    assert!(
        report.diagnostics.is_empty(),
        "mixed numeric presentation arguments must typecheck: {:#?}",
        report.diagnostics
    );

    let resolved = report
        .typed_lowering_evidence
        .iter()
        .filter_map(|evidence| match &evidence.kind {
            TypedLoweringEvidenceKind::ResolvedNumericType { target } => {
                Some((evidence.expression_id.index(), target.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        resolved
            .iter()
            .map(|(_, target)| target)
            .collect::<Vec<_>>(),
        [
            &TypeKind::I32,
            &TypeKind::F64,
            &TypeKind::I64,
            &TypeKind::I32
        ],
        "each named argument must retain its own schema/domain numeric type"
    );
    assert!(
        resolved.windows(2).all(|pair| pair[0].0 < pair[1].0),
        "numeric evidence must remain aligned with authored argument order: {resolved:?}"
    );
}

#[test]
fn presentation_named_numeric_arguments_reject_cross_slot_types() {
    let tree = parse_ok(
        r"
flow main {
    image(asset = @asset:.bg.pulse, depth = 0.5, opacity = true)
}
",
    );
    let hir = lower_to_hir(&tree).expect("invalid numeric presentation fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("cross-slot presentation numeric types must be rejected");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("argument `depth`")),
        "missing depth mismatch: {errors:#?}"
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("argument `opacity`")),
        "missing opacity mismatch: {errors:#?}"
    );
}

#[test]
fn presentation_public_id_arguments_accept_typed_locals_entities_and_strings() {
    let tree = parse_ok(
        r#"
flow main {
    let typed_target = @target.pulse
    let typed_layer = @layer.foreground
    image(asset = @asset:.bg.pulse, target = typed_target, layer = typed_layer)
    image(asset = @asset:.bg.pulse, target = @target.pulse, layer = @layer.foreground)
    image(asset = @asset:.bg.pulse, target = "target.pulse", layer = "layer.foreground")
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("public-id presentation fixture lowers");

    typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect("typed locals, entity references, and explicit public-id strings typecheck");
}

#[test]
fn presentation_public_id_arguments_reject_unknown_bare_paths() {
    let tree = parse_ok(
        r"
flow main {
    image(asset = @asset:.bg.pulse, target = totally_unknown_target, layer = totally_unknown_layer)
}
",
    );
    let hir = lower_to_hir(&tree).expect("unknown public-id path fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("unknown bare target/layer paths must not become string atoms");

    for name in ["totally_unknown_target", "totally_unknown_layer"] {
        assert!(
            errors.iter().any(|error| error.message().contains(name)),
            "missing normal name-resolution failure for `{name}`: {errors:#?}"
        );
    }
}

#[test]
fn presentation_token_scalar_policies_keep_intentional_bare_tokens() {
    let tree = parse_ok(
        r#"
flow main {
    player_viewport(width = automatic, height = .automatic, fit = "contain")
    bg(@asset:.bg.room, alignment.x = center, playback.start = beginning, opacity = inherited)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("token-scalar presentation fixture lowers");

    typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect("ratio, time, and dimension policies retain authored bare tokens");
}

#[test]
fn presentation_token_scalar_policies_typecheck_resolved_numeric_locals() {
    let tree = parse_ok(
        r"
flow main {
    let viewport_width: i32 = 1280
    let image_opacity: f64 = 0.5
    player_viewport(width = viewport_width, height = 720px)
    image(asset = @asset:.bg.pulse, opacity = image_opacity)
}
",
    );
    let hir = lower_to_hir(&tree).expect("typed token-scalar local fixture lowers");

    typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect("resolved numeric locals must use the token-scalar expected type");
}

#[test]
fn presentation_token_scalar_policies_reject_resolved_non_scalar_locals() {
    let tree = parse_ok(
        r"
flow main {
    let enabled: Bool = true
    image(asset = @asset:.bg.pulse, opacity = enabled)
}
",
    );
    let hir = lower_to_hir(&tree).expect("invalid token-scalar local fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("resolved Bool local must not be reinterpreted as an authored scalar token");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("argument `opacity`")),
        "missing resolved-local opacity mismatch: {errors:#?}"
    );
}

#[test]
fn presentation_token_scalar_policies_reject_other_resolved_non_scalar_paths() {
    for (label, declarations, value) in [
        (
            "function value",
            "fn resolved_flag() -> Bool { true }\n",
            "resolved_flag",
        ),
        ("builtin state root", "", "state"),
        ("builtin asset root", "", "asset"),
        ("known dotted target", "", "state.enabled"),
    ] {
        let tree = parse_ok(format!(
            "{declarations}flow main {{\n    image(asset = @asset:.bg.pulse, opacity = {value})\n}}\n"
        ));
        let hir = lower_to_hir(&tree).expect("resolved token-scalar path fixture lowers");
        let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
            .expect_err("resolved non-scalar path must not become an authored scalar token");

        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("argument `opacity`")),
            "missing {label} opacity mismatch: {errors:#?}"
        );
    }
}

#[test]
fn presentation_token_scalar_policies_reject_resolved_short_variant_symbols() {
    let tree = parse_ok(
        r"
flow main {
    player_viewport(width = .automatic, height = 720px)
}
",
    );
    let hir = lower_to_hir(&tree).expect("resolved short-variant token fixture lowers");
    let env = TypeCheckEnv::new().with_symbol(".automatic", TypeKind::Bool);
    let errors = typecheck_hir(&hir, &env)
        .expect_err("resolved short-variant symbol must keep its registered type");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("argument `width`")),
        "missing resolved short-variant width mismatch: {errors:#?}"
    );
}

#[test]
fn presentation_token_scalar_policies_reject_registered_function_values() {
    let tree = parse_ok(
        r"
flow main {
    image(asset = @asset:.bg.pulse, opacity = external_flag)
}
",
    );
    let hir = lower_to_hir(&tree).expect("registered function token fixture lowers");
    let env = TypeCheckEnv::new().with_function("external_flag", TypeKind::Bool);
    let errors = typecheck_hir(&hir, &env)
        .expect_err("registered function value must keep its normal Bool type");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("external_flag")),
        "missing normal registered-function resolution failure: {errors:#?}"
    );
}

#[test]
fn presentation_token_scalar_policies_reject_global_entity_alias_member_paths() {
    let tree = parse_ok(
        r"
character @character.flag Flag as global_flag {}

flow main {
    image(asset = @asset:.bg.pulse, opacity = global_flag.custom_token)
}
",
    );
    let hir = lower_to_hir(&tree).expect("global entity alias member fixture lowers");
    let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
        .expect_err("global entity alias member path must use normal path checking");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("argument `opacity`")),
        "missing global-alias opacity mismatch: {errors:#?}"
    );
}
