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
fn removed_presentation_callee_aliases_are_unknown_functions() {
    for (callee, call) in [
        (
            "background",
            "background(@asset:.bg.room, target = @target.scene)",
        ),
        (
            "image.show",
            "image.show(asset = @asset:.bg.room, id = \"image.room\")",
        ),
        ("viewport.fit", "viewport.fit(width = 1280px)"),
        ("player.viewport", "player.viewport(width = 1280px)"),
        ("player.viewport.fit", "player.viewport.fit(width = 1280px)"),
    ] {
        let tree = parse_ok(format!("flow main {{\n    {call}\n}}\n"));
        let hir = lower_to_hir(&tree).expect("removed callee fixture lowers");
        let errors = typecheck_hir(&hir, &TypeCheckEnv::new())
            .expect_err("removed presentation callee must be rejected");

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
fn removed_viewport_argument_aliases_are_unknown_arguments() {
    for (argument, value) in [
        ("design_width", "1280px"),
        ("design_height", "720px"),
        ("policy", "\"contain\""),
        ("scale_policy", "\"contain\""),
    ] {
        assert_unknown_presentation_argument(
            &format!("player_viewport({argument} = {value})"),
            "player_viewport",
            argument,
        );
    }
}

#[test]
fn hyphenated_viewport_argument_spellings_are_parser_errors() {
    for (argument, value) in [
        ("design-width", "1280px"),
        ("design-height", "720px"),
        ("scale-policy", "\"contain\""),
    ] {
        let errors = parse_errors(format!(
            "flow main {{\n    player_viewport({argument} = {value})\n}}\n"
        ));
        assert!(
            !errors.is_empty(),
            "hyphenated spelling must produce a parser error"
        );
        assert!(
            errors.iter().all(|error| error.code() == "syntax.parse"),
            "hyphenated spelling must remain a structured parser rejection: {errors:#?}"
        );
    }
}

#[test]
fn removed_image_and_background_argument_aliases_are_unknown_arguments() {
    for (argument, value) in [
        ("align.x", "0.5"),
        ("align.y", "500"),
        ("playback.start_time", "10ms"),
        ("playback.pause_at", "20ms"),
        ("playback.pinned_local_time", "30ms"),
    ] {
        assert_unknown_presentation_argument(
            &format!("image(asset = @asset:.bg.room, {argument} = {value})"),
            "image",
            argument,
        );
        assert_unknown_presentation_argument(
            &format!("bg(@asset:.bg.room, {argument} = {value})"),
            "bg",
            argument,
        );
    }
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
