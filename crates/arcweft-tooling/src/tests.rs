use crate::{
    canonicalize_source,
    code_actions::source_code_actions,
    format::{format_source, format_source_with_dialect},
    id_context::materialize_ids,
    model::{
        CanonicalizationInput, FormatOptions, ToolingCodeAction, ToolingEditReport, ToolingError,
    },
};
use arcweft_lang_hir::{
    lower::lower_to_hir,
    project::{HirProject, HirProjectModule},
};
use arcweft_lang_sema::{
    canonicalization::{
        CanonicalizationSourceSet, SemanticDataUnavailable, SemanticDocumentId,
        SemanticSourceIdentity,
    },
    check::analyze_project_types_for_canonicalization,
    env::TypeCheckEnv,
    types::{EntityKind, TypeKind},
};
use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModuleSegment},
    parser::{SourceDialect, parse_source},
};

fn with_checked_inventory<T>(
    source: &str,
    use_inventory: impl FnOnce(
        &arcweft_lang_sema::canonicalization::CheckedCanonicalizationInventory,
    ) -> T,
) -> T {
    let module = CanonicalModulePath::crate_root();
    with_checked_project_inventory(&[(module.clone(), source)], &module, use_inventory)
}

fn with_checked_project_inventory<T>(
    modules: &[(CanonicalModulePath, &str)],
    selected: &CanonicalModulePath,
    use_inventory: impl FnOnce(
        &arcweft_lang_sema::canonicalization::CheckedCanonicalizationInventory,
    ) -> T,
) -> T {
    let lowered = modules.iter().map(|(module, source)| {
        let parsed = parse_source(*source);
        let hir = lower_to_hir(parsed.typed_tree()).expect("tooling fixture must lower to HIR");
        HirProjectModule::new(module.clone(), hir)
    });
    let project = HirProject::new("tooling-tests", lowered).expect("tooling fixture project");
    let identities = modules
        .iter()
        .map(|(module, source)| {
            SemanticSourceIdentity::from_source(
                project.package().clone(),
                SemanticDocumentId::new(format!("memory:///{module}.arcw")),
                module.clone(),
                source,
            )
        })
        .collect::<Vec<_>>();
    let sources = CanonicalizationSourceSet::try_new(project.package().clone(), identities)
        .expect("exact source set");
    let env =
        TypeCheckEnv::standard().with_symbol("alice", TypeKind::entity_ref(EntityKind::Character));
    let report = analyze_project_types_for_canonicalization(&project, &env, &sources)
        .expect("checked project inventory");
    let identity = sources.source(selected).expect("selected source identity");
    let inventory = report
        .canonicalization_inventory(identity)
        .expect("module inventory");
    use_inventory(inventory)
}

fn canonicalize_for_test(source: &str) -> Result<ToolingEditReport, ToolingError> {
    with_checked_inventory(source, |inventory| {
        canonicalize_source(source, CanonicalizationInput::Checked(inventory))
    })
}

fn checked_source_code_actions(source: &str) -> Result<Vec<ToolingCodeAction>, ToolingError> {
    with_checked_inventory(source, |inventory| {
        source_code_actions(source, CanonicalizationInput::Checked(inventory))
    })
}

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
    let source = "pub character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    alice: hi[p]\n    with:\n        log.info(\"x\")\n    goto parent::next\n}\n";
    let report = canonicalize_for_test(source).expect("canonicalization report");
    assert!(report.output.contains("alice.say()[hi[p]]"));
    assert!(report.output.contains("with {"));
    assert!(report.output.contains("    }"));
    assert!(report.output.contains("goto super::next"));
}

#[test]
fn parent_path_expansion_uses_cst_path_ranges_only() {
    let source = concat!(
        "flow opening {\n",
        "    let 経路 = choose(parent::first, parent::second)\n",
        "    let normal = \"parent::normal-string\"\n",
        "    let raw = r\"parent::raw-string\"\n",
        "    // parent::line-comment\n",
        "    /* parent::block-comment */\n",
        "    alice: parent::dialogue-text[p]\n",
        "}\n",
    );

    let report = canonicalize_for_test(source).expect("typed sugar expansion");

    let expected = source
        .replacen("parent::first", "super::first", 1)
        .replacen("parent::second", "super::second", 1)
        .replacen(
            "alice: parent::dialogue-text[p]",
            "alice.say()[parent::dialogue-text[p]]",
            1,
        );
    assert_eq!(report.output, expected);

    let path_edit_starts = report
        .edits
        .iter()
        .filter(|edit| edit.replacement == "super")
        .map(|edit| edit.start)
        .collect::<Vec<_>>();
    let first = source.find("parent::first").expect("first path");
    let second = source.find("parent::second").expect("second path");
    assert_eq!(path_edit_starts, vec![first, second]);
    assert_eq!(&source[first..first + "parent".len()], "parent");
}

#[test]
fn speaker_expansion_composes_contained_parent_path_edits() {
    let source = concat!(
        "flow opening {\n",
        "    alice(voice=parent::auto, look=parent::portrait): こんにちは[p]\n",
        "}\n",
    );

    let report = canonicalize_for_test(source)
        .expect("contained path edits compose into the speaker replacement");

    assert_eq!(
        report.output,
        concat!(
            "flow opening {\n",
            "    alice.say(voice=super::auto, look=super::portrait)[こんにちは[p]]\n",
            "}\n",
        )
    );
    assert_eq!(report.edits.len(), 1);
    assert!(
        report.edits[0]
            .replacement
            .contains("voice=super::auto, look=super::portrait")
    );
}

#[test]
fn await_expansion_composes_contained_parent_path_edits() {
    let source = "flow opening {\n    await? parent::next\n}\n";

    let report = canonicalize_for_test(source)
        .expect("contained path edit composes into the await replacement");

    assert_eq!(
        report.output,
        "flow opening {\n    try await super::next\n}\n"
    );
    assert_eq!(report.edits.len(), 1);
    assert_eq!(report.edits[0].replacement, "try await super::next");
}

#[test]
fn dialogue_defaults_expansion_composes_parent_paths_in_values() {
    let source = concat!(
        "pub dialogue defaults {\n",
        "    rich_text.ruby.size = parent::ruby_size\n",
        "}\n",
    );

    let report = canonicalize_for_test(source)
        .expect("contained path edit composes into the dialogue-defaults replacement");

    assert_eq!(
        report.output,
        concat!(
            "pub dialogue defaults {\n",
            "    rich_text {\n",
            "        ruby {\n",
            "            size = super::ruby_size\n",
            "        }\n",
            "    }\n",
            "}\n",
        )
    );
    assert_eq!(report.edits.len(), 1);
    assert!(report.edits[0].replacement.contains("super::ruby_size"));
}

#[test]
fn speaker_expansion_consumes_typed_statement_context() {
    let source = concat!(
        "pub struct SettingsInput {\n",
        "    text_speed: f32,\n",
        "}\n",
        "enum Event {\n",
        "    Settings {\n",
        "        text_speed: f32,\n",
        "    },\n",
        "}\n",
        "flow opening {\n",
        "    let settings: SettingsInput = SettingsInput {\n",
        "        text_speed: 1.0,\n",
        "    }\n",
        "    if ready {\n",
        "        alice(voice=auto): こんにちは[p]\n",
        "    }\n",
        "}\n",
    );

    let report = canonicalize_for_test(source).expect("typed speaker expansion");

    let expected = source.replacen(
        "alice(voice=auto): こんにちは[p]",
        "alice.say(voice=auto)[こんにちは[p]]",
        1,
    );
    assert_eq!(report.output, expected);
}

#[test]
fn text_edit_planning_preserves_structured_edit_errors() {
    use crate::{
        edit::{SourceEditOverlay, apply_text_edits},
        model::TextEdit,
    };

    let utf8_error = apply_text_edits(
        "é",
        &[TextEdit {
            start: 1,
            end: 1,
            replacement: "x".to_owned(),
        }],
    )
    .expect_err("mid-codepoint edit must fail");
    assert_eq!(
        utf8_error,
        ToolingError::InvalidCharBoundary { start: 1, end: 1 }
    );

    let overlap_error = apply_text_edits(
        "abcd",
        &[
            TextEdit {
                start: 0,
                end: 2,
                replacement: String::new(),
            },
            TextEdit {
                start: 1,
                end: 3,
                replacement: String::new(),
            },
        ],
    )
    .expect_err("overlap must fail");
    assert_eq!(
        overlap_error,
        ToolingError::OverlappingEdit { start: 1, end: 3 }
    );

    let range_error = apply_text_edits(
        "abc",
        &[TextEdit {
            start: 2,
            end: 4,
            replacement: String::new(),
        }],
    )
    .expect_err("out-of-range edit must fail");
    assert_eq!(
        range_error,
        ToolingError::RangeOutOfBounds {
            start: 2,
            end: 4,
            len: 3,
        }
    );

    let mut overlay = SourceEditOverlay::new(vec![TextEdit {
        start: 0,
        end: "parent".len(),
        replacement: "super".to_owned(),
    }]);
    assert_eq!(
        overlay
            .rewrite_range("parent::next", 0..3)
            .expect("non-containing rewrite remains valid"),
        "par"
    );
    let mut partial_overlap = vec![TextEdit {
        start: 0,
        end: 3,
        replacement: "prefix".to_owned(),
    }];
    partial_overlap.extend(overlay.into_unconsumed_edits());
    assert_eq!(
        apply_text_edits("parent::next", &partial_overlap)
            .expect_err("partially contained overlay must remain an overlap"),
        ToolingError::OverlappingEdit {
            start: 0,
            end: "parent".len(),
        }
    );
}

#[test]
fn dialogue_tokenizer_canonical_edits_are_valid_utf8_plans() {
    use crate::{
        dialogue_sugar::{DialogueSugarContext, DialogueSugarMode, dialogue_text_canonical_edits},
        edit::apply_text_edits,
    };
    let corpus = [
        "plain 日本語[p]",
        "[.shake amp=2px]揺れる[/][p]",
        "nested [b]太字と[.wave]波[/][/][p]",
        "escaped \\[ bracket and {name}[p]",
    ];
    for text in corpus {
        let edits = dialogue_text_canonical_edits(
            text,
            DialogueSugarMode::All,
            &DialogueSugarContext::default(),
        );
        apply_text_edits(text, &edits).expect("tokenizer edits form a valid UTF-8 plan");
    }
}

#[test]
fn canonicalization_removes_redundant_decl_identity_only() {
    let source = "flow @flow.opening opening {\n}\nflow @flow.opening start {\n}\nsource @source.http_requests http_requests: Source<HttpRequest, HttpError> {\n}\ncharacter @character.alice alice {\n}\n";
    let report = canonicalize_for_test(source).expect("canonicalization report");

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
fn canonicalization_preserves_generated_decl_identity_surface() {
    let source = "#[generated]\nflow @flow.opening opening {\n}\n#[allow(style::redundant_decl_identity)]\nsource @source.http_requests http_requests: Source<HttpRequest, HttpError> {\n}\n";
    let report = canonicalize_for_test(source).expect("canonicalization report");

    assert!(!report.changed);
    assert!(report.output.contains("flow @flow.opening opening {"));
    assert!(
        report
            .output
            .contains("source @source.http_requests http_requests")
    );
}

#[test]
fn canonicalization_preserves_source_generated_decl_identity_surface() {
    let source = "#![generated(tool)]\nflow @flow.generated generated {\n    alice: hi[p]\n}\n";
    let report = canonicalize_for_test(source).expect("canonicalization report");

    assert!(report.changed);
    assert!(report.output.contains("flow @flow.generated generated {"));
    assert!(report.output.contains("alice.say()[hi[p]]"));
}

#[test]
fn canonicalization_preserves_source_allowed_decl_identity_surface() {
    let source = "#![allow(style::redundant_decl_identity)]\nflow @flow.generated generated {\n    alice: hi[p]\n}\n";
    let report = canonicalize_for_test(source).expect("canonicalization report");

    assert!(report.changed);
    assert!(report.output.contains("flow @flow.generated generated {"));
    assert!(report.output.contains("alice.say()[hi[p]]"));
}

#[test]
fn canonicalization_nests_dotted_dialogue_defaults_assignments() {
    let source = "pub dialogue defaults {\n    rich_text.ruby.size = 14px\n    rich_text.ruby.gap += 1px\n}\n";
    let report = canonicalize_for_test(source).expect("canonicalization report");

    assert!(report.changed);
    assert!(report.output.contains(
            "    rich_text {\n        ruby {\n            size = 14px\n            gap += 1px\n        }\n    }"
        ));
    assert!(!report.output.contains("rich_text.ruby.size"));
    assert!(!report.output.contains("rich_text.ruby.gap"));
}

#[test]
fn helper_returns_and_deceptive_names_use_semantic_types() {
    let source = "fn factory() -> SpeakerPreset<Character> {\n    SpeakerPreset.new(@character.alice)\n}\nfn SpeakerPresetFactory() -> i32 { 1 }\nflow @flow.opening opening {\n    let alice2 = factory()\n    let deceptive = SpeakerPresetFactory()\n    alice2: preset[p]\n    deceptive: helper[p]\n}\n";
    let report = canonicalize_for_test(source).expect("canonicalization report");

    assert!(report.output.contains("alice2[preset[p]]"));
    assert!(report.output.contains("deceptive: helper[p]"));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "AWT-CANON-004")
    );
}

#[test]
fn shared_helper_corpus_matches_the_adapter_contract() {
    let source = include_str!("../tests/fixtures/canonicalization/aw-ah-003-helper.arcw");
    let expected =
        include_str!("../tests/fixtures/canonicalization/aw-ah-003-helper.expected.arcw");

    let report = canonicalize_for_test(source).expect("shared helper canonicalization");

    assert_eq!(report.output, expected);
    assert!(report.diagnostics.is_empty());
}

#[test]
fn closure_returns_use_the_checked_result_type() {
    let source = "flow main {\n  let factory = || -> SpeakerPreset<Character> {\n    SpeakerPreset.new(@character.alice)\n  }\n  let from_closure = factory()\n  from_closure: closure return\n}\n";

    let report = canonicalize_for_test(source).expect("closure canonicalization");

    assert!(report.output.contains("from_closure[closure return]"));
    assert!(report.diagnostics.is_empty());
}

#[test]
fn direct_aliases_and_presets_use_checked_semantic_types() {
    let source = "fn make_direct() -> SpeakerPreset<Character> {\n  SpeakerPreset.new(@character.alice)\n}\n\nflow main {\n  let preset = SpeakerPreset.new(@character.alice)\n  preset: direct preset\n\n  let alice = @character.alice\n  alice: character alias\n}\n";
    let expected = "fn make_direct() -> SpeakerPreset<Character> {\n  SpeakerPreset.new(@character.alice)\n}\n\nflow main {\n  let preset = SpeakerPreset.new(@character.alice)\n  preset[direct preset]\n\n  let alice = @character.alice\n  alice.say()[character alias]\n}\n";

    let report = canonicalize_for_test(source).expect("direct canonicalization");

    assert_eq!(report.output, expected);
    assert!(report.diagnostics.is_empty());
}

#[test]
fn branch_return_types_drive_speaker_canonicalization() {
    let source = "fn factory() -> SpeakerPreset<Character> {\n  SpeakerPreset.new(@character.alice)\n}\n\nfn from_block() -> SpeakerPreset<Character> {\n  {\n    let value = factory()\n    value\n  }\n}\n\nfn from_if(flag: Bool) -> SpeakerPreset<Character> {\n  if flag {\n    factory()\n  } else {\n    from_block()\n  }\n}\n\nfn from_if_let(maybe: Option<SpeakerPreset<Character>>) -> SpeakerPreset<Character> {\n  if let .Some(value) = maybe {\n    value\n  } else {\n    factory()\n  }\n}\n\nfn from_match(maybe: Option<SpeakerPreset<Character>>) -> SpeakerPreset<Character> {\n  match maybe {\n    .Some(value) => value\n    .None => factory()\n  }\n}\n\nflow main {\n  let block_value = from_block()\n  block_value: block\n  let if_value = from_if(true)\n  if_value: if\n  let if_let_value = from_if_let(.None)\n  if_let_value: if let\n  let match_value = from_match(.None)\n  match_value: match\n}\n";

    let report = canonicalize_for_test(source).expect("branch canonicalization");

    for canonical in [
        "block_value[block]",
        "if_value[if]",
        "if_let_value[if let]",
        "match_value[match]",
    ] {
        assert!(report.output.contains(canonical), "missing `{canonical}`");
    }
}

#[test]
fn lexical_shadowing_preserves_only_the_non_speaker_line() {
    let source = "fn factory() -> SpeakerPreset<Character> {\n  SpeakerPreset.new(@character.alice)\n}\n\nflow main {\n  let speaker = factory()\n  speaker: outer before\n  scope {\n    let speaker = 1\n    speaker: inner non-speaker remains unchanged\n  }\n  speaker: outer after\n}\n";

    let report = canonicalize_for_test(source).expect("shadowing canonicalization");

    assert!(report.output.contains("speaker[outer before]"));
    assert!(
        report
            .output
            .contains("speaker: inner non-speaker remains unchanged")
    );
    assert!(report.output.contains("speaker[outer after]"));
    assert_eq!(report.status, "partial");
    assert!(
        report.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.code.as_str(),
            "AWT-CANON-003" | "AWT-CANON-004"
        ))
    );
}

#[test]
fn unresolved_line_is_left_unchanged_without_blocking_proven_lines() {
    let source = "flow main {\n  alice: checked speaker\n  missing: unresolved speaker\n}\n";

    let report = canonicalize_for_test(source).expect("partial canonicalization");

    assert!(report.output.contains("alice.say()[checked speaker]"));
    assert!(report.output.contains("missing: unresolved speaker"));
    assert_eq!(report.status, "partial");
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "AWT-CANON-003"
            && diagnostic.arguments.get("reference").map(String::as_str) == Some("missing")
            && diagnostic.arguments.get("state").map(String::as_str) == Some("unresolved")
    }));
}

#[test]
fn imported_callable_aliases_resolve_by_canonical_declaration() {
    let root = CanonicalModulePath::crate_root();
    let helpers = root.join(ModuleSegment::new("helpers").expect("valid module segment"));
    let other = root.join(ModuleSegment::new("other").expect("valid module segment"));
    let root_source = "use helpers.neutral_name as build\nuse helpers.misleading_preset_name as SpeakerPresetFactory\n\nflow main {\n  let imported = build()\n  imported: imported helper return\n\n  let qualified = helpers.neutral_name()\n  qualified: qualified helper return\n\n  let deceptive = SpeakerPresetFactory()\n  deceptive: non-preset remains unchanged\n\n  let collision = other.neutral_name()\n  collision: same-spelling non-preset remains unchanged\n}\n";
    let helper_source = "pub fn neutral_name() -> SpeakerPreset<Character> {\n  SpeakerPreset.new(@character.alice)\n}\n\npub fn misleading_preset_name() -> i32 {\n  1\n}\n";
    let other_source = "pub fn neutral_name() -> i32 {\n  1\n}\n";

    let report = with_checked_project_inventory(
        &[
            (root.clone(), root_source),
            (helpers, helper_source),
            (other, other_source),
        ],
        &root,
        |inventory| canonicalize_source(root_source, CanonicalizationInput::Checked(inventory)),
    )
    .expect("import canonicalization");

    assert!(report.output.contains("imported[imported helper return]"));
    assert!(report.output.contains("qualified[qualified helper return]"));
    assert!(
        report
            .output
            .contains("deceptive: non-preset remains unchanged")
    );
    assert!(
        report
            .output
            .contains("collision: same-spelling non-preset remains unchanged")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "AWT-CANON-004")
    );
}

#[test]
fn unicode_identifier_and_crlf_ranges_are_preserved() {
    let source = "flow main {\r\n  let 話者 = @character.alice\r\n  話者: こんにちは\r\n}\r\n";
    let expected =
        "flow main {\r\n  let 話者 = @character.alice\r\n  話者.say()[こんにちは]\r\n}\r\n";

    let report = canonicalize_for_test(source).expect("Unicode CRLF canonicalization");

    assert_eq!(report.output, expected);
}

#[test]
fn stale_and_unavailable_semantics_are_hard_errors() {
    let source = "flow main {\n  alice: hi\n}\n";
    with_checked_inventory(source, |inventory| {
        let error = canonicalize_source(
            &format!("{source}\n"),
            CanonicalizationInput::Checked(inventory),
        )
        .expect_err("changed source must reject stale inventory");
        assert_eq!(error.code(), "AWT-CANON-002");
    });

    let unavailable = SemanticDataUnavailable::new(
        SemanticDocumentId::new("memory:///unavailable.arcw"),
        "project analysis failed",
    );
    let error = canonicalize_source(source, CanonicalizationInput::Unavailable(&unavailable))
        .expect_err("unavailable semantic data must stop canonicalization");
    assert_eq!(error.code(), "AWT-CANON-001");
}

#[test]
fn checked_canonicalization_is_deterministic() {
    let source = "flow main {\n  let alice2 = alice(voice=auto)\n  alice2: hi[p]\n}\n";

    let first = canonicalize_for_test(source).expect("first canonicalization");
    let second = canonicalize_for_test(source).expect("second canonicalization");

    assert_eq!(first, second);
}

#[test]
fn inconsistent_semantic_record_multiplicity_and_surface_are_diagnostics() {
    let source = "flow main {\n  alice: first\n  alice: second\n}\n";
    with_checked_inventory(source, |inventory| {
        let parsed = parse_source(source);
        let lines = crate::dialogue_content::collect_speaker_lines(&parsed);
        let records = inventory.speaker_lines();
        assert_eq!(lines.len(), 2);
        assert_eq!(records.len(), 2);

        for (matches, expected_reason) in [
            (Vec::new(), "missing_record"),
            (vec![&records[0], &records[0]], "duplicate_record"),
            (vec![&records[1]], "surface_mismatch"),
        ] {
            let diagnostic = crate::canonicalization::speaker_record_diagnostic_for_matches(
                source, lines[0], &matches,
            )
            .expect("inconsistent record must diagnose");
            assert_eq!(diagnostic.code, "AWT-CANON-005");
            assert_eq!(
                diagnostic.arguments.get("reason").map(String::as_str),
                Some(expected_reason)
            );
        }
    });
}

#[test]
fn canonicalizes_chained_speaker_presets_from_checked_types() {
    let source = "pub character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    let alice2 = alice(voice=auto)\n    let alice3 = alice2(face=smile)\n    alice3: chained[p]\n}\n";
    let report = canonicalize_for_test(source).expect("canonicalization report");

    assert!(report.output.contains("alice3[chained[p]]"));
}

#[test]
fn expands_dialogue_authoring_sugar_only_when_requested() {
    let source = "flow @flow.opening opening {\n    alice.say()[今日は｜変な夢《へんなゆめ》と|悪夢{あくむ}。$(name)[! flash()][.mark][w 500ms][page][em:夢][raw: [p]]]\n}\n";
    let preserved = format_source(source, FormatOptions::default()).expect("format report");
    assert_eq!(preserved.output, source);

    let expanded = canonicalize_for_test(source).expect("canonicalization report");
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
    let expanded = canonicalize_for_test(source).expect("canonicalization report");

    assert!(
        expanded
            .output
            .contains("[color value=\"#a8b5ff\"]夜[/color][p]")
    );
}

#[test]
fn canonicalization_does_not_treat_dialogue_content_lines_as_speaker_sugar() {
    let source = "flow @flow.opening opening {\n    alice.say()[\n        cue: [raw: [p]や#[expr]をそのまま表示] と [! flash()][p]\n    ]\n}\n";
    let expanded = canonicalize_for_test(source).expect("canonicalization report");

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
fn canonical_rich_text_preserves_explicit_fx_spans() {
    let source = "flow @flow.opening opening {\n    alice: [fx warning(label=\"urgent warning\")]important[/fx][.sparkle amp=2px]effect[/][p]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[fx warning(label=\"urgent warning\")]important[/fx]")
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
    let actions = checked_source_code_actions(source).expect("source code actions");

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
fn source_code_actions_group_semantic_canonicalization_rewrites() {
    let source = "flow @flow.opening opening {\n    alice: hi $(name)[.shake]there[/][page]\n}\n";
    let actions = checked_source_code_actions(source).expect("source code actions");

    let action = actions
        .iter()
        .find(|action| action.id == "arcweft.canonicalizeSugar")
        .expect("canonicalization action");
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
    let actions = checked_source_code_actions(source).expect("source code actions");
    let action = actions
        .iter()
        .find(|action| action.id == "arcweft.canonicalizeSugar")
        .expect("canonicalization action");
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
    let source = "flow @flow.opening opening {\n    let handles = alice.say()[don't [fx warning()]stop[/fx] [.shake]now[/][p]]\n}\n";
    let report = format_source(
        source,
        FormatOptions {
            canonical_rich_text: true,
        },
    )
    .expect("format report");

    assert!(
        report
            .output
            .contains("[don't [fx warning()]stop[/fx] [effect .shake]now[/effect][p]]")
    );
}
