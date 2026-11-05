use arcweft_presentation::fx::{
    FxApplication, FxTarget,
    builtin::{
        BUILTIN_FX_CALLABLE_CATALOG, BuiltinFxActiveAbiParameterSet, BuiltinFxCallableRowId,
        BuiltinFxParameterBinding, BuiltinFxParameterId, BuiltinFxParameterPresence,
        BuiltinFxSpecialization,
    },
};

#[test]
fn agent_observe_native_renderer_reports_full_grammar_sample_rich_text_constructs() {
    let fixture = NativeCliFixture::checked_in(
        workspace_root().join("samples/rich-text-full-grammar.arcw"),
        "entry.rich_text_full_grammar",
    );
    let mut session = NativeMcpDialogueSession::new();
    session.observe_fixture(&fixture, None, 128);
    let mut previous_tick = 0;
    for line in [
        "say.full.001",
        "say.full.001.extreme",
        "say.full.001.vertical_extreme",
        "say.full.002",
        "say.full.003",
        "say.full.004",
        "say.full.005",
        "say.full.006",
        "say.full.007",
        "say.full.008",
        "say.full.009",
    ] {
        // Each report belongs to its actual current frame. A second observe
        // would restart the sample; no aggregate of different frames is issued.
        let report = session.advance_to_line(line);
        let tick = report["tick"].as_u64().unwrap();
        assert!(tick > previous_tick);
        previous_tick = tick;
        let dialogue = find_dialogue_view_object_by_rich_text_line(&report, line);
        match line {
            "say.full.005" => {
                use BuiltinFxCallableRowId::*;
                for (text, row, target) in [
                    ("wave", WaveGlyphTransform, FxTarget::Glyph),
                    ("shake", ShakeGlyphTransform, FxTarget::Content),
                    ("typewriter", TypewriterGlyphMask, FxTarget::Content),
                    ("arc", ArcGlyphTransform, FxTarget::Glyph),
                    ("custom effect", SparkleGlyphTransform, FxTarget::Content),
                    ("sparkle", SparkleGlyphTransform, FxTarget::Content),
                ] {
                    assert_full_grammar_fx(&report, text, row, target, &[]);
                }
                for predicate in [
                    (|t: &serde_json::Value| {
                        t["translate"]["x"] == 4000
                            && t["translate"]["y"] == -2000
                            && t["origin"] == "baseline_start"
                            && t["target"] == "glyph"
                    }) as fn(&serde_json::Value) -> bool,
                    |t| {
                        t["rotate"]["degrees"] == 8000
                            && t["origin"] == "center"
                            && t["target"] == "content"
                    },
                    |t| {
                        t["scale"]["x"] == 1200
                            && t["scale"]["y"] == 1200
                            && t["origin"] == "baseline_center"
                            && t["target"] == "content"
                    },
                ] {
                    assert!(rich_text_text_run_has_transform(
                        &report, dialogue, predicate
                    ));
                }
                session.assert_rich_text_layer_capture_has_content();
            }
            "say.full.006" => {
                assert!(rich_text_text_run_has_transform(&report, dialogue, |t| {
                    t["skew"]["x"] == 2000
                        && t["skew"]["y"] == 0
                        && t["origin"] == "glyph_center"
                        && t["target"] == "glyph"
                }));
                assert_full_grammar_fx(
                    &report,
                    "effect",
                    BuiltinFxCallableRowId::JitterGlyphTransform,
                    FxTarget::Glyph,
                    &[],
                );
                let shader = assert_full_grammar_fx(
                    &report,
                    "shader",
                    BuiltinFxCallableRowId::ShaderOffscreenPass,
                    FxTarget::Content,
                    &[
                        BuiltinFxParameterId::Amount,
                        BuiltinFxParameterId::Direction,
                    ],
                );
                assert_eq!(shader["static_"][0]["kind"], "resource");
                assert_eq!(shader["static_"][0]["value"], "soft_glow");
                assert_full_grammar_proxy_metadata(&report);
                assert_full_grammar_page_line_metadata(&report);
                let run = find_rich_text_run_object(&report, "z depth");
                assert_eq!(run["rich_text_ref"]["presentation"]["layer"], "hud");
                assert_eq!(run["rich_text_ref"]["presentation"]["z_index"], 3);
                assert_eq!(run["rich_text_ref"]["presentation"]["opacity"], 800);
                assert_eq!(run["rich_text_ref"]["object_layer"], "hud");
                assert_eq!(run["rich_text_ref"]["object_depth"], 3000);
                session.assert_rich_text_layer_capture_has_content();
            }
            "say.full.007" => assert_eq!(dialogue["text"], "\ncue: 条件と代替の表示"),
            "say.full.008" => assert!(
                dialogue["text"]
                    .as_str()
                    .unwrap()
                    .contains("[p]や#[expr]をそのまま表示")
            ),
            "say.full.009" => {
                let text = dialogue["text"].as_str().unwrap();
                assert!(text.contains("[.shake]") && text.contains("#[player_name]"));
            }
            _ => {}
        }
    }
    // Positive Mask/ObjectId, animation samples and shader pixels are checked
    // by player_observation::full_grammar using the same local Native frame.
    // The MCP publication policy intentionally withholds non-color images.
    session.close();
}

fn assert_full_grammar_fx<'a>(
    report: &'a serde_json::Value,
    text: &str,
    row: BuiltinFxCallableRowId,
    target: FxTarget,
    authored_optional: &[BuiltinFxParameterId],
) -> &'a serde_json::Value {
    let run = find_rich_text_run_object(report, text);
    let schema = BUILTIN_FX_CALLABLE_CATALOG.get(row).unwrap();
    let active = BuiltinFxActiveAbiParameterSet::from_parameters(
        schema
            .parameters()
            .iter()
            .filter(|parameter| {
                parameter.binding() == BuiltinFxParameterBinding::Abi
                    && matches!(
                        parameter.presence(),
                        BuiltinFxParameterPresence::Required
                            | BuiltinFxParameterPresence::Defaulted(_)
                    )
            })
            .map(|parameter| parameter.id())
            .chain(authored_optional.iter().copied()),
    );
    let expected = BuiltinFxSpecialization::try_new(row, target, None, active)
        .unwrap()
        .definition_id()
        .unwrap();
    let [application] = run["rich_text_ref"]["presentation"]["fx"]
        .as_array()
        .unwrap()
        .as_slice()
    else {
        panic!("one authored Fx application on {text}")
    };
    let typed: FxApplication = serde_json::from_value(application.clone()).unwrap();
    assert_eq!(typed.definition(), &expected);
    let tree = presentation_tree_node(report, run["id"].as_str().unwrap());
    assert_eq!(tree["role"], "rich_text_run");
    assert_eq!(tree["rich_text_kind"], "text_run");
    assert_eq!(tree["fx"][0]["id"], expected.to_string());
    assert_eq!(
        tree["fx"][0]["authored_ordinal"],
        application["authored_ordinal"]
    );
    application
}

fn assert_full_grammar_proxy_metadata(report: &serde_json::Value) {
    let catalog = serde_json::from_value(report["text_contexts"].clone()).unwrap();
    for (text, id, ty, role, layer, depth, field, value, start, end, index) in [
        (
            "proxy",
            "hotspot",
            "KeywordHit",
            "keyword",
            None,
            4000,
            "channel",
            "choice",
            62,
            67,
            0,
        ),
        (
            "multi proxy",
            "hotspot2",
            "KeywordHit",
            "keyword",
            None,
            4000,
            "channel",
            "inventory",
            70,
            81,
            0,
        ),
        (
            "multi proxy",
            "hover",
            "HoverHit",
            "hover",
            Some("view"),
            7000,
            "tone",
            "alert",
            70,
            81,
            1,
        ),
        (
            "typed proxy",
            "KeywordHit",
            "KeywordHit",
            "keyword",
            None,
            4000,
            "channel",
            "typed",
            84,
            95,
            0,
        ),
    ] {
        let run = find_rich_text_run_object(report, text);
        checked_observed_text_element(&catalog, run);
        let run_proxy = &run["rich_text_ref"]["presentation"]["object_proxies"][index];
        let object = find_rich_text_proxy_object(report, id, text);
        // Resolve the canonical parent through its issued catalog, never by
        // splitting the opaque object ID or borrowing an ID from another frame.
        checked_observed_text_element(&catalog, object);
        assert_eq!(object["parent_id"], run["id"]);
        let proxies = object["rich_text_ref"]["presentation"]["object_proxies"]
            .as_array()
            .unwrap();
        let [proxy] = proxies.as_slice() else {
            panic!("one selected proxy")
        };
        assert_eq!(proxy, run_proxy);
        assert_eq!(proxy["id"], id);
        assert_eq!(proxy["type_name"], ty);
        assert_text_proxy_declaration(&proxy["declaration"], ty);
        assert_eq!(proxy["role"], role);
        assert_eq!(proxy["layer"].as_str(), layer);
        assert_eq!(proxy["depth"], depth);
        assert_eq!(proxy["hit_test"], true);
        assert_eq!(text_proxy_field(proxy, field)["kind"], "text");
        assert_eq!(text_proxy_field(proxy, field)["value"], value);
        assert_eq!(object["rich_text_ref"]["index"], index);
        assert_eq!(object["rich_text_ref"]["range"]["start"], start);
        assert_eq!(object["rich_text_ref"]["range"]["end"], end);
        assert_eq!(object["rich_text_ref"]["hit_test"], true);
        assert_eq!(object["rich_text_ref"]["object_depth"], depth);
        assert_eq!(object["rich_text_ref"]["object_layer"].as_str(), layer);
        assert_agent_observe_object_capture_refs(object);
        for hit_owner in [run, object] {
            let hit = rich_text_proxy_hit_region(hit_owner, id, start, end);
            assert_eq!(hit["proxy_type"], ty);
            assert_text_proxy_declaration(&hit["proxy_declaration"], ty);
            assert_eq!(hit["proxy_role"], role);
            assert_eq!(hit["proxy_layer"].as_str(), layer);
            assert_eq!(hit["depth"], depth);
        }
        assert_rich_text_proxy_presentation_tree_node(
            report,
            object["id"].as_str().unwrap(),
            run["id"].as_str().unwrap(),
            id,
            depth,
        );
    }
    let nested = find_rich_text_run_object(report, "multi proxy");
    assert_eq!(nested["rich_text_ref"]["object_depth"], 7000);
    assert_eq!(
        nested["rich_text_ref"]["presentation"]["object_proxies"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

fn assert_full_grammar_page_line_metadata(report: &serde_json::Value) {
    let catalog = serde_json::from_value(report["text_contexts"].clone()).unwrap();
    for (object, kind, start, end) in [
        (
            find_rich_text_page_object_by_line(report, "say.full.006", 0),
            "text_page",
            0,
            108,
        ),
        (
            find_rich_text_line_object_by_line(report, "say.full.006", 0),
            "text_line",
            1,
            102,
        ),
    ] {
        checked_observed_text_element(&catalog, object);
        let element = &object["rich_text_ref"];
        assert_eq!(element["kind"], kind);
        assert_eq!(element["index"], 0);
        assert_eq!(element["range"]["start"], start);
        assert_eq!(element["range"]["end"], end);
        assert_eq!(element["hit_test"], true);
        assert_eq!(element["presentation"]["layer"], "hud");
        assert_eq!(element["object_layer"], "view");
        assert_eq!(element["object_depth"], 7000);
        assert!(object["text"].as_str().unwrap().contains("明示family"));
        assert!(object["text"].as_str().unwrap().contains("proxy"));
        assert_rich_text_hit_region_matches_bbox(object, kind, start, end);
        assert_agent_observe_object_capture_refs(object);
    }
}
