use crate::{
    code_actions::source_code_actions,
    format::{format_source, format_source_with_dialect},
    id_context::materialize_ids,
    model::{FormatOptions, ToolingError},
};
use arcweft_lang_syntax::parser::SourceDialect;

#[test]
fn default_format_preserves_sugar() {
    let source = "flow @flow.opening opening {\n    alice: hi[p]\n}\n";
    let report = format_source(source, FormatOptions::default()).expect("format report");
    assert!(!report.changed);
    assert_eq!(report.output, source);
}

#[test]
fn agent_format_accepts_awfagent_dialect_without_game_sugar() {
    let source = "agent @agent.opening {\n    let frame = try observe(@flow.opening)\n}\n";
    let report = format_source_with_dialect(source, SourceDialect::Agent, FormatOptions::default())
        .expect("agent format report");

    assert!(!report.changed);
    assert_eq!(report.output, source);
}

#[test]
fn agent_format_rejects_game_sugar_rewrites() {
    let source = "agent @agent.opening {\n}\n";
    let error = format_source_with_dialect(
        source,
        SourceDialect::Agent,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect_err("agent formatter rejects game sugar expansion");

    assert!(matches!(
        error,
        ToolingError::UnsupportedFormatOption {
            option: "expand_sugar",
            dialect: "Agent",
        }
    ));
}

#[test]
fn agent_format_preserves_comments_trivia_and_item_golden() {
    let source = r#"//! Agent formatter fixture
/// Investigates route behavior while preserving docs.
#[agent(version = 1)]

// Launch metadata must stay attached to the agent item.
#[launch(profile = "game.dev")]
#[bind(program = compatible)]
#[budget(timeout = 45s, steps = 192usize, captures = 8usize, rag_queries = 4usize)]
agent @agent.debug.opening_route investigate_opening_route()
effects {
    agent.observe,
    agent.act.semantic,
    agent.wait,
    agent.capture,
    agent.resource.read,
    debug.record,
    rag.query,
}
{
    // Observation and semantic action.
    let before = try observe()
    note(fmt("initial tick={before.tick} state={before.state_hash}"))
    let result = try choose(@choice.opening.listen)
    checkpoint("choice-dispatched")

    // Composite wait with entity refs and diagnostics.
    let after = try wait(
        any([
            signal(@signal.current_flow).eq(@flow.alice_intro),
            diagnostics().has_error(),
        ]),
        timeout = 8s,
    )

    if after.signals.get(@signal.current_flow) != @flow.alice_intro {
        let context = try rag.query(
            "opening listen choice did not reach alice_intro",
            roots = [@choice.opening.listen, @flow.alice_intro],
            graph_depth = 2u32,
            limit = 12usize,
        )
        note(context.summary())

        let latest = try read_resource("arcweft://session/cli/observation/latest.json")
        attach(latest)
        let image = try capture(viewport(), name = "route-failure")
        attach(image)

        expect(false, message = "opening route failed; investigation context attached")
    }

    expect(result.accepted)
    Ok(())
}
"#;

    assert_agent_format_golden(source);
}

#[test]
fn agent_format_is_idempotent_for_action_resource_and_rag_samples() {
    let samples = [
        include_str!("../../../samples/agent-script/cli-pointer-click-smoke.awfagent"),
        include_str!("../../../samples/agent-script/cli-attach-resource-smoke.awfagent"),
        include_str!("../../../samples/agent-script/failure-investigation.awfagent"),
    ];

    for sample in samples {
        assert_agent_format_golden(sample);
    }
}

fn assert_agent_format_golden(source: &str) {
    let first = format_source_with_dialect(source, SourceDialect::Agent, FormatOptions::default())
        .expect("agent format report");
    assert!(!first.changed);
    assert_eq!(first.output, source);
    assert!(
        first.diagnostics.is_empty(),
        "Agent formatter should not report diagnostics for golden fixture: {:?}",
        first.diagnostics
    );

    let second = format_source_with_dialect(
        &first.output,
        SourceDialect::Agent,
        FormatOptions::default(),
    )
    .expect("second agent format report");
    assert!(!second.changed);
    assert_eq!(second.output, first.output);
    assert!(
        second.diagnostics.is_empty(),
        "second Agent formatter pass should remain diagnostic-free: {:?}",
        second.diagnostics
    );
}

#[test]
fn expands_speaker_with_and_parent_sugar() {
    let source = "pub surface character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    alice: hi[p]\n    with:\n        log.info(\"x\")\n    goto parent::next\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect("format report");
    assert!(report.output.contains("alice.say()[hi[p]]"));
    assert!(report.output.contains("with {"));
    assert!(report.output.contains("    }"));
    assert!(report.output.contains("goto super::next"));
}

#[test]
fn expand_sugar_canonicalizes_redundant_decl_identity_only() {
    let source = "flow @flow.opening opening {\n}\nflow @flow.opening start {\n}\nsource @source.http_requests http_requests: Source<HttpRequest, HttpError> {\n}\ncharacter @character.alice alice {\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect("format report");

    assert!(report.output.contains("flow opening {"));
    assert!(report.output.contains("flow @flow.opening start {"));
    assert!(
        report
            .output
            .contains("source http_requests: Source<HttpRequest, HttpError>")
    );
    assert!(report.output.contains("character alice {"));
}

#[test]
fn expand_sugar_preserves_generated_decl_identity_surface() {
    let source = "#[generated]\nflow @flow.opening opening {\n}\n#[allow(style::redundant_decl_identity)]\nsource @source.http_requests http_requests: Source<HttpRequest, HttpError> {\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect("format report");

    assert!(!report.changed);
    assert!(report.output.contains("flow @flow.opening opening {"));
    assert!(
        report
            .output
            .contains("source @source.http_requests http_requests")
    );
}

#[test]
fn expand_sugar_preserves_source_generated_decl_identity_surface() {
    let source = "#![generated(tool)]\nflow @flow.generated generated {\n    alice: hi[p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect("format report");

    assert!(report.changed);
    assert!(report.output.contains("flow @flow.generated generated {"));
    assert!(report.output.contains("alice.say()[hi[p]]"));
}

#[test]
fn expand_sugar_preserves_source_allowed_decl_identity_surface() {
    let source = "#![allow(style::redundant_decl_identity)]\nflow @flow.generated generated {\n    alice: hi[p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect("format report");

    assert!(report.changed);
    assert!(report.output.contains("flow @flow.generated generated {"));
    assert!(report.output.contains("alice.say()[hi[p]]"));
}

#[test]
fn expand_sugar_nests_dotted_dialogue_defaults_assignments() {
    let source = "pub dialogue defaults @dialogue.defaults {\n    rich_text.ruby.size = 14px\n    rich_text.ruby.gap += 1px\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect("format report");

    assert!(report.changed);
    assert!(report.output.contains(
            "    rich_text {\n        ruby {\n            size = 14px\n            gap += 1px\n        }\n    }"
        ));
    assert!(!report.output.contains("rich_text.ruby.size"));
    assert!(!report.output.contains("rich_text.ruby.gap"));
}

#[test]
fn expands_speaker_presets_from_typed_tree_without_helper_false_positive() {
    let source = "pub surface character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    let alice2 = alice(voice=auto)\n    let helper = compute()\n    alice2: preset[p]\n    helper: helper[p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect("format report");

    assert!(report.output.contains("alice2[preset[p]]"));
    assert!(report.output.contains("helper.say()[helper[p]]"));
    assert!(!report.output.contains("helper[helper[p]]"));
}

#[test]
fn expands_chained_speaker_presets_from_typed_tree() {
    let source = "pub surface character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    let alice2 = alice(voice=auto)\n    let alice3 = alice2(face=smile)\n    alice3: chained[p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect("format report");

    assert!(report.output.contains("alice3[chained[p]]"));
}

#[test]
fn expands_dialogue_authoring_sugar_only_when_requested() {
    let source = "flow @flow.opening opening {\n    alice.say()[今日は｜変な夢《へんなゆめ》と|悪夢{あくむ}。$(name)[! flash()][.mark][w 500ms][page][em:夢][raw: [p]]]\n}\n";
    let preserved = format_source(source, FormatOptions::default()).expect("format report");
    assert_eq!(preserved.output, source);

    let expanded = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect("format report");
    assert!(expanded.output.contains("|[変な夢](へんなゆめ)"));
    assert!(expanded.output.contains("|[悪夢](あくむ)"));
    assert!(expanded.output.contains("#[name]"));
    assert!(expanded.output.contains("[call flash()]"));
    assert!(expanded.output.contains("[mark .mark]"));
    assert!(expanded.output.contains("[w time=500ms]"));
    assert!(expanded.output.contains("[p]"));
    assert!(expanded.output.contains("[em]夢[/em]"));
    assert!(expanded.output.contains("[raw][p][/raw]"));
}

#[test]
fn short_scalar_tag_sugar_emits_the_canonical_value_form() {
    let source = "flow @flow.opening opening {\n    alice: [color #a8b5ff:夜][p]\n}\n";
    let expanded = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect("format report");

    assert!(
        expanded
            .output
            .contains("[color value=\"#a8b5ff\"]夜[/color][p]")
    );
}

#[test]
fn expand_sugar_does_not_treat_dialogue_content_lines_as_speaker_sugar() {
    let source = "flow @flow.opening opening {\n    alice.say()[\n        cue: [raw: [p]や#[expr]をそのまま表示] と [! flash()][p]\n    ]\n}\n";
    let expanded = format_source(
        source,
        FormatOptions {
            expand_sugar: true,
            canonical_rich_text: false,
        },
    )
    .expect("format report");

    assert!(!expanded.output.contains("cue.say()"));
    assert!(
        expanded
            .output
            .contains("cue: [raw][p]や#[expr]をそのまま表示[/raw] と [call flash()][p]")
    );
}

#[test]
fn canonical_rich_text_expands_dot_inference_without_other_sugar() {
    let source = "flow @flow.opening opening {\n    alice: hi $(name)[.keyword][.sparkle amp=2px pattern=a,b,c]there[/][.host id=sparkle amp=1px]hosted[/][page]\n    let handles = alice.say()[[.vertical_rl]縦[/][p]] with: out handles\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains("$(name)"));
    assert!(report.output.contains("[mark .keyword]"));
    assert!(
        report
            .output
            .contains("[effect .sparkle amp=2px pattern=a,b,c]there[/effect]")
    );
    assert!(
        report
            .output
            .contains("[effect .host id=sparkle amp=1px]hosted[/effect]")
    );
    assert!(
        report
            .output
            .contains("[layout .vertical_rl]縦[/layout][p]")
    );
    assert!(report.output.contains("[page]"));
}

#[test]
fn canonical_rich_text_preserves_explicit_decoration_spans() {
    let source = "flow @flow.opening opening {\n    alice: [decorate .warning label=\"urgent warning\"]important[/decorate][.sparkle amp=2px]effect[/][p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[decorate .warning label=\"urgent warning\"]important[/decorate]")
    );
    assert!(
        report
            .output
            .contains("[effect .sparkle amp=2px]effect[/effect]")
    );
}

#[test]
fn canonical_rich_text_preserves_closing_brackets_inside_quoted_arguments() {
    let source = "flow @flow.opening opening {\n    alice: [.sparkle note=\"contains ] safely\"]text[/][p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[effect .sparkle note=\"contains ] safely\"]text[/effect][p]")
    );
}

#[test]
fn canonical_rich_text_projects_indented_multiline_lf_and_crlf_edits() {
    let source_lf = "flow @flow.opening opening {\n    alice:\n        Intro\n        [.sparkle amp=2px]effect[/][p]\n}\n";
    for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
        let report = format_source(
            &source,
            FormatOptions {
                expand_sugar: false,
                canonical_rich_text: true,
            },
        )
        .expect("format report");

        assert!(
            report
                .output
                .contains("[effect .sparkle amp=2px]effect[/effect][p]")
        );
        assert!(report.output.contains("flow @flow.opening opening"));
        assert!(report.output.contains("        Intro"));
    }
}

#[test]
fn canonical_rich_text_visits_flow_else_branches() {
    let source_lf = "flow @flow.opening opening {\n    if ready {\n        alice: [.shake]then[/][p]\n    } else {\n        alice: [.pulse]else[/][p]\n    }\n    if let value = selected {\n        alice: [.wave]some[/][p]\n    } else {\n        alice: [.jitter]none[/][p]\n    }\n    if alternate {\n        alice: [.spin]first[/][p]\n    }\n    else {\n        alice: [.motion]second[/][p]\n    }\n}\n";
    for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
        let report = format_source(
            &source,
            FormatOptions {
                expand_sugar: false,
                canonical_rich_text: true,
            },
        )
        .expect("format report");

        assert!(
            report.output.contains("[effect .shake]then[/effect][p]"),
            "{}",
            report.output
        );
        assert!(report.output.contains("[effect .pulse]else[/effect][p]"));
        assert!(report.output.contains("[effect .wave]some[/effect][p]"));
        assert!(report.output.contains("[effect .jitter]none[/effect][p]"));
        assert!(report.output.contains("[effect .spin]first[/effect][p]"));
        assert!(report.output.contains("[effect .motion]second[/effect][p]"));
    }
}

#[test]
fn canonical_rich_text_uses_the_dialogue_delimiters_when_content_repeats_in_callee() {
    let source = "flow @flow.opening opening {\n    let handles = try render(\"[.shake]effect[/][p]\")()[[.shake]effect[/][p]]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains("render(\"[.shake]effect[/][p]\")()"));
    assert!(
        report
            .output
            .contains("[[effect .shake]effect[/effect][p]]"),
        "{}",
        report.output
    );
    assert_eq!(report.output.matches("[effect .shake]").count(), 1);
}

#[test]
fn canonical_rich_text_projects_multiline_dialogue_call_expressions_across_crlf() {
    let source_lf = "flow @flow.opening opening {\n    let handles = alice.say()[\n        Intro\n        [.sparkle amp=2px]effect[/][p]\n    ]\n}\n";
    for source in [source_lf.to_owned(), source_lf.replace('\n', "\r\n")] {
        let report = format_source(
            &source,
            FormatOptions {
                expand_sugar: false,
                canonical_rich_text: true,
            },
        )
        .expect("format report");

        assert!(
            report
                .output
                .contains("[effect .sparkle amp=2px]effect[/effect][p]"),
            "{}",
            report.output
        );
        assert_eq!(report.output.contains("\r\n"), source.contains("\r\n"));
    }
}

#[test]
fn canonical_rich_text_visits_statement_bodies_outside_flows() {
    let source = "fn render_notice() {\n    if ready {\n        let handles = alice.say()[[.shake]notice[/][p]]\n    }\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[[effect .shake]notice[/effect][p]]"),
        "{}",
        report.output
    );
}

#[test]
fn canonical_rich_text_expands_inferred_text_proxy_objects() {
    let source = "#[text_proxy(kind=\"keyword\", default_hit=true)]\npub struct KeywordHit {\n    channel: String\n}\n\nflow @flow.opening opening {\n    alice: [.hotspot type=KeywordHit channel=choice]proxy[/][.KeywordHit]typed[/][.sparkle amp=2px]effect[/][p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[object .hotspot type=KeywordHit channel=choice]proxy[/object]")
    );
    assert!(
        report
            .output
            .contains("[object .KeywordHit type=KeywordHit]typed[/object]")
    );
    assert!(
        report
            .output
            .contains("[effect .sparkle amp=2px]effect[/effect]")
    );
    assert!(!report.output.contains("[effect .hotspot"));
    assert!(!report.output.contains("[effect .KeywordHit"));
}

#[test]
fn canonical_rich_text_expands_inferred_rich_text_proxy_objects() {
    let source = "#[rich_text_proxy(kind=\"quest\", default_hit=true)]\npub struct QuestHit {\n    channel: String\n}\n\nflow @flow.opening opening {\n    alice: [.QuestHit channel=main]quest[/][.sparkle amp=2px]effect[/][p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[object .QuestHit type=QuestHit channel=main]quest[/object]")
    );
    assert!(
        report
            .output
            .contains("[effect .sparkle amp=2px]effect[/effect]")
    );
    assert!(!report.output.contains("[effect .QuestHit"));
}

#[test]
fn canonical_rich_text_expands_nested_inferred_text_proxy_objects() {
    let source = "#[text_proxy(kind=\"keyword\", default_hit=true)]\npub struct KeywordHit {\n    channel: String\n}\n\n#[text_proxy(kind=\"hover\", default_hit=false)]\npub struct HoverHit {\n    layer: String\n}\n\nflow @flow.opening opening {\n    alice: [.hotspot type=KeywordHit channel=inventory][.HoverHit tone=alert]multi[/][/][.sparkle amp=2px]effect[/][p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains(
            "[object .hotspot type=KeywordHit channel=inventory][object .HoverHit type=HoverHit tone=alert]multi[/object][/object]"
        ));
    assert!(
        report
            .output
            .contains("[effect .sparkle amp=2px]effect[/effect]")
    );
    assert!(!report.output.contains("[effect .hotspot"));
    assert!(!report.output.contains("[effect .HoverHit"));
    assert!(!report.output.contains("[/]"));
}

#[test]
fn canonical_rich_text_removes_marker_like_inferred_close() {
    let source =
        "flow @flow.opening opening {\n    alice: [.keyword]word[/][.shake]there[/][p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[mark .keyword]word[effect .shake]there[/effect]")
    );
    assert!(!report.output.contains("[/]"));
}

#[test]
fn canonical_rich_text_uses_the_shared_reserved_marker_classification() {
    let source = "flow @flow.opening opening {\n    alice: [.mark ignored=value]word[/][p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(report.output.contains("[mark .mark]word[p]"));
    assert!(!report.output.contains("[effect .mark"));
    assert!(!report.output.contains("[/]"));
}

#[test]
fn source_code_actions_include_canonical_rich_text_edits() {
    let source = "flow @flow.opening opening {\n    alice: [.keyword][.vertical_rl]縦[/]\n}\n";
    let actions = source_code_actions(source);

    let action = actions
        .iter()
        .find(|action| action.id == "arcweft.canonicalRichText")
        .expect("canonical rich-text action");
    let edit = action.edit.as_ref().expect("canonical action has edit");

    assert_eq!(action.label, "Canonicalize inferred rich-text tags");
    assert_eq!(edit.start, 0);
    assert_eq!(edit.end, source.len());
    assert!(
        edit.replacement
            .contains("[layout .vertical_rl]縦[/layout]")
    );
    assert!(edit.replacement.contains("[mark .keyword]"));
    assert!(!edit.replacement.contains("[/]"));
}

#[test]
fn source_code_actions_group_expand_sugar_rewrites() {
    let source = "flow @flow.opening opening {\n    alice: hi $(name)[.shake]there[/][page]\n}\n";
    let actions = source_code_actions(source);

    let action = actions
        .iter()
        .find(|action| action.id == "arcweft.expandSugar")
        .expect("expand action");
    let edit = action.edit.as_ref().expect("expand action has edit");

    assert_eq!(edit.start, 0);
    assert_eq!(edit.end, source.len());
    assert!(edit.replacement.contains("alice.say()["));
    assert!(edit.replacement.contains("#[name]"));
    assert!(edit.replacement.contains("[effect .shake]there[/effect]"));
    assert!(edit.replacement.contains("[p]"));
}

#[test]
fn source_code_actions_include_decl_identity_rewrite_only_when_linted() {
    let source =
        "flow @flow.opening opening {\n}\n#[generated]\nflow @flow.generated generated {\n}\n";
    let actions = source_code_actions(source);
    let action = actions
        .iter()
        .find(|action| action.id == "arcweft.expandSugar")
        .expect("expand action");
    let edit = action.edit.as_ref().expect("expand action has edit");

    assert_eq!(edit.start, 0);
    assert_eq!(edit.end, source.len());
    assert!(edit.replacement.contains("flow opening {"));
    assert!(
        edit.replacement
            .contains("#[generated]\nflow @flow.generated generated {")
    );
}

#[test]
fn materializes_top_level_and_choice_ids() {
    let source = "flow @flow.opening opening {\n    choice @.first {\n        @.listen \"Listen\" -> @flow.next\n    }\n}\ntest @.smoke scenario {}\n";
    let report = materialize_ids(source).expect("materialize report");
    assert!(report.output.contains("choice @choice.opening.first"));
    assert!(report.output.contains("@choice.opening.first.listen"));
    assert!(report.output.contains("test @test.smoke scenario"));
}

#[test]
fn materializes_dialogue_line_option_ids() {
    let source = "flow @flow.opening opening {\n    scope outer {\n        scope rain {\n            地の文(id=@say:.sound):\n                雨の音。[p]\n            alice(id=@.comment, text_key=@.comment_text):\n                Good morning.[p]\n            alice.say(id=@...shared, text_key=@super.inner_text)[\n                Shared.[p]\n            ]\n        }\n    }\n}\n";
    let report = materialize_ids(source).expect("materialize report");

    assert!(report.output.contains(
            "地の文(id=@say.opening.narrator.outer.rain.sound, text_key=@text.opening.narrator.outer.rain.sound):"
        ));
    assert!(report.output.contains(
            "alice(id=@say.opening.alice.outer.rain.comment, text_key=@text.opening.alice.outer.rain.comment_text):"
        ));
    assert!(report.output.contains(
        "alice.say(id=@say.opening.alice.shared, text_key=@text.opening.alice.outer.inner_text)["
    ));
}

#[test]
fn materializes_omitted_dialogue_ids_in_colon_call_and_flat_fences() {
    let source = "flow @flow.opening opening {\n    alice:\n        Hi[p]\n    alice.say()[\n        Again[p]\n    ]\n=== scope rain ===\n=== line 地の文 ===\n雨。[p]\n=== with ===\nwait(mark(.done))\n=== /with ===\n=== /line ===\n=== /scope ===\n}\n";
    let report = materialize_ids(source).expect("materialize report");

    assert!(
        report
            .output
            .contains("alice(id=@say.opening.alice.001, text_key=@text.opening.alice.001):")
    );
    assert!(
        report
            .output
            .contains("alice.say(id=@say.opening.alice.002, text_key=@text.opening.alice.002)[")
    );
    assert!(report.output.contains(
            "=== line 地の文(id=@say.opening.narrator.rain.001, text_key=@text.opening.narrator.rain.001) ==="
        ));
    assert!(report.output.contains("=== with ==="));
}

#[test]
fn canonical_rich_text_keeps_dialogue_call_ranges_after_natural_apostrophes() {
    let source = "flow @flow.opening opening {\n    let handles = alice.say()[don't [decorate .warning]stop[/decorate] [.shake]now[/][p]]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            expand_sugar: false,
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[don't [decorate .warning]stop[/decorate] [effect .shake]now[/effect][p]]")
    );
}
