fn assert_agent_observe_object_capture_refs(object: &serde_json::Value) {
    assert_eq!(object["capture_refs"]["object_id_color"]["alpha"], 255);
    let object_id = object["id"].as_str().expect("object id is present");
    let captures = object["capture_refs"]["captures"]
        .as_array()
        .expect("object capture refs are listed");
    assert!(captures.iter().any(|capture| {
        capture["kind"] == "color"
            && capture["mime_type"] == "image/png"
            && capture["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with(&format!("/object.{object_id}.png")))
    }));
    assert!(captures.iter().any(|capture| {
        capture["kind"] == "object_id"
            && capture["mime_type"] == "image/png"
            && capture["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with(&format!("/object.{object_id}.object-id.png")))
    }));
    assert!(captures.iter().any(|capture| {
        capture["kind"] == "mask"
            && capture["mime_type"] == "application/octet-stream"
            && capture["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with(&format!("/object.{object_id}.mask.rgba")))
    }));
}

fn assert_agent_observe_rich_text_display_map(object: &serde_json::Value) {
    let rich_text = observed_object_rich_text_frame(object);
    let text_runs = rich_text["display_map"]["text_runs"].as_array().unwrap();
    assert!(text_runs.iter().any(|run| run["source"] == "interpolation"
        && run["range"]["start"] == 6
        && run["range"]["end"] == 9));
    assert!(text_runs.iter().any(|run| run["source"] == "ruby_base"
        && run["range"]["start"] == 10
        && run["range"]["end"] == 13));
    assert!(
        text_runs
            .iter()
            .any(|run| run["source"] == "control_hard_break"
                && run["range"]["start"] == 13
                && run["range"]["end"] == 14)
    );
    assert!(
        rich_text["display_map"]["ruby_annotations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|ruby| ruby["ruby"] == "ゆめ"
                && ruby["base_range"]["start"] == 10
                && ruby["base_range"]["end"] == 13)
    );
}

fn assert_agent_observe_rich_text_display_report(json: &serde_json::Value) {
    assert_eq!(json["status"], "ok");
    assert_eq!(json["viewport"]["width"], 1280);
    assert_eq!(json["images"][0]["kind"], "overlay_svg");
    assert!(
        json["images"][0]["uri"]
            .as_str()
            .is_some_and(|uri| uri.starts_with("arcweft://session/cli/frame/"))
    );
    assert!(
        json["overlay_svg"]
            .as_str()
            .is_some_and(|svg| svg.contains("Hello Aoi"))
    );
    let object = &json["objects"][0];
    let rich_text = observed_object_rich_text_frame(object);
    assert_eq!(object["role"], "dialogue_view");
    assert_eq!(object["bbox"]["space"], "viewport");
    assert_eq!(object["text"], "Hello Aoi 夢\n");
    assert_agent_observe_object_capture_refs(object);
    assert_eq!(rich_text["base_styles"].as_array().unwrap().len(), 4);
    assert!(
        rich_text["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["kind"] == "ruby")
    );
    assert_agent_observe_rich_text_display_map(object);
    assert!(
        rich_text["host_events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "voice")
    );
    assert_eq!(json["actions"][0]["action"], "advance_text");
    let objects = json["objects"].as_array().expect("objects are listed");
    assert_agent_observe_rich_text_child_objects(objects);
}

fn assert_agent_observe_rich_text_child_objects(objects: &[serde_json::Value]) {
    let page_object = objects
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.page.0")
        .expect("text page is observable as an element");
    assert_eq!(page_object["role"], "rich_text_page");
    assert_eq!(page_object["layer"], "dialogue.rich_text");
    assert_eq!(page_object["text"], "Hello Aoi 夢\n");
    assert_eq!(page_object["rich_text_ref"]["kind"], "text_page");
    assert_eq!(page_object["rich_text_ref"]["index"], 0);
    assert_eq!(
        page_object["rich_text_ref"]["page"].as_u64().unwrap_or(0),
        0
    );
    assert_eq!(page_object["rich_text_ref"]["range"]["start"], 0);
    assert_eq!(
        page_object["rich_text_ref"]["range"]["end"],
        page_object["text"].as_str().expect("page text").len() as u64
    );
    assert_eq!(page_object["rich_text_ref"]["hit_test"], true);
    assert_rich_text_hit_region_matches_bbox(
        page_object,
        "text_page",
        0,
        page_object["text"].as_str().expect("page text").len() as u64,
    );
    assert_agent_observe_object_capture_refs(page_object);

    let line_object = objects
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.line.0")
        .expect("text line is observable as an element");
    assert_eq!(line_object["role"], "rich_text_line");
    assert_eq!(line_object["layer"], "dialogue.rich_text");
    assert!(
        line_object["text"]
            .as_str()
            .is_some_and(|text| text.contains("Hello Aoi")),
        "line object should expose resolved line text: {line_object}"
    );
    assert_eq!(line_object["rich_text_ref"]["kind"], "text_line");
    assert_eq!(line_object["rich_text_ref"]["index"], 0);
    assert_eq!(
        line_object["rich_text_ref"]["page"].as_u64().unwrap_or(0),
        0
    );
    assert_eq!(line_object["rich_text_ref"]["hit_test"], true);
    let line_start = line_object["rich_text_ref"]["range"]["start"]
        .as_u64()
        .expect("line range start");
    let line_end = line_object["rich_text_ref"]["range"]["end"]
        .as_u64()
        .expect("line range end");
    assert!(line_end > line_start);
    assert_rich_text_hit_region_matches_bbox(line_object, "text_line", line_start, line_end);
    assert_agent_observe_object_capture_refs(line_object);

    let run_object = objects
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.run.1")
        .expect("interpolation run is observable as an element");
    assert_eq!(run_object["role"], "rich_text_run");
    assert_eq!(run_object["layer"], "dialogue.rich_text");
    assert_eq!(run_object["text"], "Aoi");
    assert_eq!(run_object["rich_text_ref"]["kind"], "text_run");
    assert_eq!(run_object["rich_text_ref"]["index"], 1);
    assert_eq!(run_object["rich_text_ref"]["source"], "interpolation");
    assert_eq!(run_object["rich_text_ref"]["range"]["start"], 6);
    assert_eq!(run_object["rich_text_ref"]["range"]["end"], 9);

    let glyph_object = objects
        .iter()
        .find(|object| object["role"] == "rich_text_glyph" && object["text"] == "H")
        .expect("individual text glyph is observable as an element");
    assert_eq!(glyph_object["layer"], "dialogue.rich_text");
    assert_eq!(glyph_object["rich_text_ref"]["kind"], "text_glyph");
    assert_eq!(glyph_object["rich_text_ref"]["range"]["start"], 0);
    assert_eq!(glyph_object["rich_text_ref"]["range"]["end"], 1);
    assert_rich_text_hit_region_matches_bbox(glyph_object, "text_glyph", 0, 1);
    assert_agent_observe_object_capture_refs(glyph_object);

    let cluster_object = objects
        .iter()
        .find(|object| object["role"] == "rich_text_cluster" && object["text"] == "H")
        .expect("glyph cluster remains observable as an element");
    assert_eq!(cluster_object["rich_text_ref"]["kind"], "glyph_cluster");
    assert_rich_text_hit_region_matches_bbox(cluster_object, "glyph_cluster", 0, 1);
    assert_agent_observe_object_capture_refs(cluster_object);

    let ruby_object = objects
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.ruby.0")
        .expect("ruby annotation is observable as an element");
    assert_eq!(ruby_object["role"], "rich_text_ruby");
    assert_eq!(ruby_object["text"], "夢 (ゆめ)");
    assert_eq!(ruby_object["rich_text_ref"]["kind"], "ruby");
    assert_eq!(ruby_object["rich_text_ref"]["index"], 0);
    assert_eq!(ruby_object["rich_text_ref"]["ruby"], "ゆめ");
    assert_eq!(ruby_object["rich_text_ref"]["range"]["start"], 10);
    assert_eq!(ruby_object["rich_text_ref"]["range"]["end"], 13);
    assert_agent_observe_object_capture_refs(ruby_object);
}

#[test]
fn agent_observe_json_reports_rich_text_display_objects() {
    let path = temp_arcw(
        "agent-observe-rich-text",
        r##"
pub dialogue defaults {
    font = serif
    text_color = rgb("#101112")
    inline_error = InlineFailure.fallback("?")
}

character @character.alice Alice as alice {
    dialogue_style {
        text_color = rgb("#202122")
    }
}

flow @flow.main main {
    let player = "Aoi"
    alice(color=rgb("#303132")): Hello #[player] |[夢](ゆめ)[r][voice auto][p]
}
"##,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("overlay")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe runs rich text source");
    fs::remove_file(&path).expect("remove temp agent observe source");

    assert!(
        output.status.success(),
        "agent observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "agent observe JSON should not leak absolute temp paths: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("agent observe output is structured JSON");
    assert_agent_observe_rich_text_display_report(&json);
}

#[test]
fn agent_hit_test_reports_depth_sorted_rich_text_proxy() {
    let path = temp_arcw(
        "agent-hit-test-rich-text-proxy",
        r#"
#[text_proxy(kind="keyword", default_hit=true, depth=4, channel=choice)]
pub struct KeywordHit {
    channel: String
}

#[text_proxy(kind="hover", default_hit=true, depth=7, layer=view)]
pub struct HoverHit {
    layer: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [object .hotspot type=KeywordHit][object .hover type=HoverHit]Hit[/object][/object][p]
}
"#,
    );
    let observe = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe runs hit-test source");
    assert!(
        observe.status.success(),
        "agent observe for hit-test should succeed, stderr: {}",
        String::from_utf8_lossy(&observe.stderr)
    );
    let observe_json: serde_json::Value =
        serde_json::from_slice(&observe.stdout).expect("observe output is JSON");
    let hover = find_rich_text_proxy_object(&observe_json, "hover", "Hit");
    assert_eq!(hover["rich_text_ref"]["object_layer"], "view");
    assert_eq!(
        hover["rich_text_ref"]["presentation"]["object_proxies"][0]["layer"],
        "view"
    );
    assert_rich_text_page_and_line_aggregate_proxy_metadata(&observe_json);
    let x = agent_json_bbox_x(&hover["bbox"]) + agent_json_bbox_width(&hover["bbox"]) / 2;
    let y = agent_json_bbox_y(&hover["bbox"]) + agent_json_bbox_height(&hover["bbox"]) / 2;

    let hit_test = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("hit-test")
        .arg(&path)
        .arg("--x")
        .arg(x.to_string())
        .arg("--y")
        .arg(y.to_string())
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent hit-test runs rich text source");
    fs::remove_file(&path).expect("remove temp agent hit-test source");

    assert!(
        hit_test.status.success(),
        "agent hit-test should succeed, stderr: {}",
        String::from_utf8_lossy(&hit_test.stderr)
    );
    let hit_json: serde_json::Value =
        serde_json::from_slice(&hit_test.stdout).expect("hit-test output is JSON");
    assert_eq!(hit_json["status"], "ok");
    assert_eq!(hit_json["x"], x);
    assert_eq!(hit_json["y"], y);
    assert_eq!(hit_json["top_object_id"], hover["id"]);
    let top = &hit_json["hits"][0];
    assert_top_hover_rich_text_proxy_hit(top, hover);
    let keyword_hit = hit_json["hits"]
        .as_array()
        .expect("hits are listed")
        .iter()
        .find(|hit| hit["region"]["proxy_id"] == "hotspot" && hit["depth"] == 4000)
        .unwrap_or_else(|| {
            panic!("shallower nested proxy should remain visible as a lower-ranked hit: {hit_json}")
        });
    assert_eq!(
        keyword_hit["region"]["proxy_params"]["channel"]["value"],
        "choice"
    );
}

fn assert_agent_hit_capture_refs_match(hit: &serde_json::Value, object: &serde_json::Value) {
    assert_eq!(hit["object"]["id"], object["id"]);
    assert_eq!(hit["object"]["layer"], object["layer"]);
    assert_eq!(hit["object"]["role"], object["role"]);
    assert_eq!(hit["object"]["bbox"], object["bbox"]);
    assert_eq!(hit["object"]["polygon"], object["polygon"]);
    assert_eq!(hit["object"]["capture_refs"], object["capture_refs"]);
    assert_eq!(
        hit["object"]["rich_text_ref"]["kind"],
        object["rich_text_ref"]["kind"]
    );
    assert_eq!(hit["polygon"], object["polygon"]);
    assert_eq!(
        hit["capture_refs"]["object_id_color"],
        object["capture_refs"]["object_id_color"]
    );
}

fn assert_top_hover_rich_text_proxy_hit(top: &serde_json::Value, hover: &serde_json::Value) {
    assert_eq!(top["rank"], 0);
    assert_eq!(top["object_id"], hover["id"]);
    assert_eq!(top["role"], "rich_text_proxy");
    assert_eq!(top["region"]["kind"], "text_object_proxy");
    assert_eq!(top["region"]["proxy_id"], "hover");
    assert_eq!(top["region"]["proxy_type"], "HoverHit");
    assert_text_proxy_declaration(&top["region"]["proxy_declaration"], "HoverHit");
    assert_eq!(top["region"]["proxy_role"], "hover");
    assert_eq!(top["region"]["proxy_layer"], "view");
    assert_eq!(top["layer"], "view");
    assert_agent_hit_capture_refs_match(top, hover);
    assert_eq!(top["rich_text_ref"]["object_layer"], "view");
    assert_eq!(top["depth"], 7000);
}

fn assert_text_proxy_declaration(declaration: &serde_json::Value, struct_name: &str) {
    assert_text_proxy_declaration_with_attribute(declaration, struct_name, "text_proxy");
}

fn assert_text_proxy_declaration_with_attribute(
    declaration: &serde_json::Value,
    struct_name: &str,
    attribute: &str,
) {
    assert_eq!(declaration["struct_name"], struct_name);
    assert_eq!(declaration["attribute"], attribute);
}

#[test]
fn agent_observe_infers_text_proxy_struct_shorthand() {
    let path = inferred_text_proxy_struct_shorthand_source();
    let observe = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe runs inferred proxy source");
    assert!(
        observe.status.success(),
        "agent observe for inferred proxy should succeed, stderr: {}",
        String::from_utf8_lossy(&observe.stderr)
    );
    let observe_json: serde_json::Value =
        serde_json::from_slice(&observe.stdout).expect("observe output is JSON");
    assert_inferred_text_proxy_struct_shorthand_observe(&path, &observe_json);
    let hover = find_rich_text_proxy_object(&observe_json, "HoverHit", "Hit");

    let hit_json = hit_test_center_of_observed_object(&path, hover);
    fs::remove_file(&path).expect("remove temp inferred proxy source");

    assert_eq!(hit_json["status"], "ok");
    assert_eq!(hit_json["top_object_id"], hover["id"]);
    assert_eq!(hit_json["hits"][0]["role"], "rich_text_proxy");
    assert_eq!(hit_json["hits"][0]["region"]["kind"], "text_object_proxy");
    assert_eq!(hit_json["hits"][0]["region"]["proxy_id"], "HoverHit");
    assert_eq!(hit_json["hits"][0]["region"]["proxy_type"], "HoverHit");
    assert_text_proxy_declaration(
        &hit_json["hits"][0]["region"]["proxy_declaration"],
        "HoverHit",
    );
    assert_eq!(hit_json["hits"][0]["region"]["proxy_layer"], "view");
    assert_eq!(
        hit_json["hits"][0]["region"]["proxy_params"]["tone"]["value"],
        "alert"
    );
    assert_eq!(hit_json["hits"][0]["depth"], 7000);
}

#[test]
fn agent_observe_infers_rich_text_proxy_struct_attribute_family() {
    let path = temp_arcw(
        "agent-observe-inferred-rich-text-proxy-attribute-family",
        r#"
#[rich_text_proxy(kind="quest", default_hit=true, depth=6, layer=hud, channel=quest)]
pub struct QuestHit {
    channel: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.QuestHit state=active]Quest[/][p]
}
"#,
    );
    let observe = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe runs rich_text_proxy source");
    assert!(
        observe.status.success(),
        "agent observe for rich_text_proxy should succeed, stderr: {}",
        String::from_utf8_lossy(&observe.stderr)
    );
    let observe_json: serde_json::Value =
        serde_json::from_slice(&observe.stdout).expect("observe output is JSON");

    let dialogue_view = find_dialogue_view_object(&observe_json);
    assert!(rich_text_text_run_has_object_proxy(dialogue_view, |proxy| {
        proxy["id"] == "QuestHit"
            && proxy["type_name"] == "QuestHit"
            && proxy["declaration"]["struct_name"] == "QuestHit"
            && proxy["declaration"]["attribute"] == "rich_text_proxy"
            && proxy["role"] == "quest"
            && proxy["layer"] == "hud"
            && proxy["depth"] == 6000
            && proxy["hit_test"] == true
            && proxy["params"]["channel"]["value"] == "quest"
            && proxy["params"]["state"]["value"] == "active"
    }));

    let proxy = find_rich_text_proxy_object(&observe_json, "QuestHit", "Quest");
    assert_eq!(proxy["role"], "rich_text_proxy");
    assert_eq!(proxy["rich_text_ref"]["kind"], "text_object_proxy");
    assert_eq!(proxy["rich_text_ref"]["object_layer"], "hud");
    assert_eq!(proxy["rich_text_ref"]["object_depth"], 6000);
    assert_text_proxy_declaration_with_attribute(
        &proxy["rich_text_ref"]["presentation"]["object_proxies"][0]["declaration"],
        "QuestHit",
        "rich_text_proxy",
    );
    assert_agent_observe_proxy_object_image_metadata_carries_struct_declaration(
        &path, proxy, "hud", 6000,
    );

    assert_agent_presentation_tree_filters_rich_text_proxy_struct(&path);

    let hit_json = hit_test_center_of_observed_object(&path, proxy);
    fs::remove_file(&path).expect("remove temp rich_text_proxy source");

    assert_eq!(hit_json["status"], "ok");
    assert_eq!(hit_json["top_object_id"], proxy["id"]);
    assert_eq!(hit_json["hits"][0]["role"], "rich_text_proxy");
    assert_eq!(hit_json["hits"][0]["region"]["kind"], "text_object_proxy");
    assert_eq!(hit_json["hits"][0]["region"]["proxy_id"], "QuestHit");
    assert_eq!(hit_json["hits"][0]["region"]["proxy_type"], "QuestHit");
    assert_text_proxy_declaration_with_attribute(
        &hit_json["hits"][0]["region"]["proxy_declaration"],
        "QuestHit",
        "rich_text_proxy",
    );
    assert_eq!(hit_json["hits"][0]["region"]["proxy_role"], "quest");
    assert_eq!(hit_json["hits"][0]["region"]["proxy_layer"], "hud");
    assert_eq!(
        hit_json["hits"][0]["region"]["proxy_params"]["state"]["value"],
        "active"
    );
    assert_eq!(hit_json["hits"][0]["depth"], 6000);
}

fn assert_agent_observe_proxy_object_image_metadata_carries_struct_declaration(
    path: &Path,
    proxy: &serde_json::Value,
    object_layer: &str,
    object_depth: i32,
) {
    assert_agent_observe_object_image_metadata_carries_object_layer(
        path,
        proxy,
        object_layer,
        object_depth,
    );
}

fn assert_agent_presentation_tree_filters_rich_text_proxy_struct(path: &Path) {
    let proxy_struct_tree = read_agent_presentation_tree_resource(
        path,
        "arcweft://session/cli/frame/0/presentation-tree.json?proxy_struct=QuestHit",
        "rich-text-proxy-struct",
    );
    assert!(presentation_tree_has_object_proxy(
        &proxy_struct_tree,
        |proxy| {
            proxy["type_name"] == "QuestHit"
                && proxy["declaration"]["attribute"] == "rich_text_proxy"
                && proxy["params"]["channel"]["value"] == "quest"
        }
    ));
    let proxy_param_tree = read_agent_presentation_tree_resource(
        path,
        "arcweft://session/cli/frame/0/presentation-tree.json?proxy_param.state=active",
        "rich-text-proxy-param",
    );
    assert!(presentation_tree_has_object_proxy(
        &proxy_param_tree,
        |proxy| {
            proxy["type_name"] == "QuestHit" && proxy["params"]["state"]["value"] == "active"
        }
    ));
}

#[test]
fn agent_observe_reports_text_presentation_z_index_depth() {
    let path = temp_arcw(
        "agent-observe-rich-text-z-index-depth",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.layer hud][.z_index 7][.opacity 0.5][.meta role=caption hover=true weight=2]Depth|[夢](ゆめ)[r][/][/][/][/] plain[p]
}
",
    );
    let observe = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe runs z-index source");
    assert!(
        observe.status.success(),
        "agent observe for z-index text should succeed, stderr: {}",
        String::from_utf8_lossy(&observe.stderr)
    );
    let observe_json: serde_json::Value =
        serde_json::from_slice(&observe.stdout).expect("observe output is JSON");
    let hud_layer = observe_json["layers"]
        .as_array()
        .expect("layers are reported")
        .iter()
        .find(|layer| layer["id"] == "hud")
        .expect("presentation object layer is observed as a layer");
    assert!(
        hud_layer["object_count"].as_u64().unwrap_or(0) >= 3,
        "hud layer should include run/page/line rich text objects: {hud_layer}"
    );
    let run = find_rich_text_run_object(&observe_json, "Depth");
    assert_eq!(run["rich_text_ref"]["presentation"]["layer"], "hud");
    assert_eq!(run["rich_text_ref"]["presentation"]["z_index"], 7);
    assert_eq!(run["rich_text_ref"]["presentation"]["opacity"], 500);
    assert_eq!(
        run["rich_text_ref"]["presentation"]["params"]["role"]["value"],
        "caption"
    );
    assert_eq!(
        run["rich_text_ref"]["presentation"]["params"]["hover"]["value"],
        true
    );
    assert_eq!(
        run["rich_text_ref"]["presentation"]["params"]["weight"]["value"],
        2
    );
    assert_eq!(run["rich_text_ref"]["object_layer"], "hud");
    assert_eq!(run["rich_text_ref"]["object_depth"], 7000);
    assert_eq!(run["rich_text_ref"]["hit_test"], false);

    let page = observe_json["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| object["role"] == "rich_text_page")
        .expect("rich-text page object is observed");
    assert_eq!(page["rich_text_ref"]["presentation"]["layer"], "hud");
    assert_eq!(
        page["rich_text_ref"]["presentation"]["params"]["role"]["value"],
        "caption"
    );
    assert_eq!(page["rich_text_ref"]["object_layer"], "hud");
    assert_eq!(page["rich_text_ref"]["object_depth"], 7000);

    let line = observe_json["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| object["role"] == "rich_text_line")
        .expect("rich-text line object is observed");
    assert_eq!(line["rich_text_ref"]["presentation"]["layer"], "hud");
    assert_eq!(
        line["rich_text_ref"]["presentation"]["params"]["role"]["value"],
        "caption"
    );
    assert_eq!(line["rich_text_ref"]["object_layer"], "hud");
    assert_eq!(line["rich_text_ref"]["object_depth"], 7000);

    assert_agent_observe_captures_presentation_layer(&path);
    assert_agent_observe_object_image_metadata_carries_object_layer(&path, line, "hud", 7000);
    assert_agent_observe_child_text_object_image_metadata_carries_object_layer(
        &path,
        &observe_json,
        "hud",
        7000,
    );

    assert_agent_observe_line_hit_test_carries_object_layer(&path, line, "hud", 7000);
    fs::remove_file(&path).expect("remove temp z-index source");
}

fn assert_agent_observe_line_hit_test_carries_object_layer(
    path: &Path,
    line: &serde_json::Value,
    object_layer: &str,
    object_depth: i32,
) {
    let hit = hit_test_center_of_observed_object(path, line);
    assert_eq!(hit["status"], "ok");
    assert_eq!(hit["top_object_id"], line["id"]);
    assert_eq!(hit["hits"][0]["role"], "rich_text_line");
    assert_eq!(hit["hits"][0]["layer"], object_layer);
    assert_agent_hit_capture_refs_match(&hit["hits"][0], line);
    assert_eq!(
        hit["hits"][0]["rich_text_ref"]["object_layer"],
        object_layer
    );
    assert_eq!(hit["hits"][0]["depth"], object_depth);
}

fn assert_agent_observe_child_text_object_image_metadata_carries_object_layer(
    path: &Path,
    observe_json: &serde_json::Value,
    object_layer: &str,
    object_depth: i32,
) {
    let glyph = find_rich_text_glyph_object(observe_json, "D", 0, 1);
    assert_eq!(glyph["rich_text_ref"]["object_layer"], object_layer);
    assert_eq!(glyph["rich_text_ref"]["object_depth"], object_depth);
    assert_agent_observe_object_image_metadata_carries_object_layer(
        path,
        glyph,
        object_layer,
        object_depth,
    );
    let cluster = find_rich_text_cluster_object(observe_json, "D", 0, 1);
    assert_eq!(cluster["rich_text_ref"]["object_layer"], object_layer);
    assert_eq!(cluster["rich_text_ref"]["object_depth"], object_depth);
    assert_agent_observe_object_image_metadata_carries_object_layer(
        path,
        cluster,
        object_layer,
        object_depth,
    );
    let ruby = find_rich_text_ruby_object(observe_json, 0);
    assert_eq!(ruby["rich_text_ref"]["object_layer"], object_layer);
    assert_eq!(ruby["rich_text_ref"]["object_depth"], object_depth);
    assert_agent_observe_object_image_metadata_carries_object_layer(
        path,
        ruby,
        object_layer,
        object_depth,
    );
}

fn assert_agent_observe_captures_presentation_layer(path: &Path) {
    let layer_dir = temp_dir("agent-observe-rich-text-layer-capture");
    let layer_png = layer_dir.join("hud-layer.png");
    let layer_capture = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("hud")
        .arg("--out")
        .arg(&layer_png)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe captures presentation object layer");
    assert!(
        layer_capture.status.success(),
        "agent observe should capture rich-text presentation object layer, stderr: {}",
        String::from_utf8_lossy(&layer_capture.stderr)
    );
    let layer_png_bytes = fs::read(&layer_png).expect("read rich-text presentation layer PNG");
    assert_eq!(&layer_png_bytes[..8], b"\x89PNG\r\n\x1a\n");
}

fn assert_agent_observe_object_image_metadata_carries_object_layer(
    path: &Path,
    object: &serde_json::Value,
    object_layer: &str,
    object_depth: i32,
) {
    let object_id = object["id"].as_str().expect("observed object id");
    let dir = temp_dir("agent-observe-rich-text-object-image-metadata");
    let object_png = dir.join("object.png");
    let capture = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--object")
        .arg(object_id)
        .arg("--out")
        .arg(&object_png)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe captures rich-text object image metadata");
    assert!(
        capture.status.success(),
        "agent observe should capture rich-text object image metadata, stderr: {}",
        String::from_utf8_lossy(&capture.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&capture.stdout).expect("object capture output is JSON");
    assert_eq!(json["images"][0]["scope"]["kind"], "object");
    assert_eq!(json["images"][0]["object"]["id"], object_id);
    assert_eq!(json["images"][0]["object"]["role"], object["role"]);
    assert_eq!(json["images"][0]["object"]["bbox"], object["bbox"]);
    assert_eq!(json["images"][0]["object"]["polygon"], object["polygon"]);
    assert_agent_observe_object_capture_refs(&json["images"][0]["object"]);
    assert_eq!(
        json["images"][0]["object"]["capture_refs"]["object_id_color"],
        object["capture_refs"]["object_id_color"]
    );
    assert_eq!(json["images"][0]["object"]["object_layer"], object_layer);
    assert_eq!(json["images"][0]["object"]["object_depth"], object_depth);
    assert_eq!(
        json["images"][0]["object"]["rich_text_ref"]["object_layer"],
        object_layer
    );
    assert_eq!(
        json["images"][0]["object"]["rich_text_ref"]["object_depth"],
        object_depth
    );
    assert_eq!(
        json["images"][0]["object"]["rich_text_ref"],
        object["rich_text_ref"]
    );
    assert!(
        json["images"][0]["content_pixels"].as_u64().unwrap_or(0) > 0,
        "rich-text object image should contain visible pixels: {json}"
    );
    let object_png_bytes = fs::read(&object_png).expect("read rich-text object PNG");
    assert_eq!(&object_png_bytes[..8], b"\x89PNG\r\n\x1a\n");
    fs::remove_dir_all(&dir).expect("remove temp rich-text object metadata dir");
}

fn inferred_text_proxy_struct_shorthand_source() -> PathBuf {
    temp_arcw(
        "agent-observe-inferred-rich-text-proxy",
        r#"
#[text_proxy(kind="keyword", default_hit=true, depth=4, channel=choice)]
pub struct KeywordHit {
    channel: String
}

#[text_proxy(kind="hover", default_hit=true, depth=7, layer=view)]
pub struct HoverHit {
    layer: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.hotspot type=KeywordHit channel=inventory][.HoverHit tone=alert]Hit[/][/][.sparkle amp=2px]FX[/][p]
}
"#,
    )
}

fn assert_inferred_text_proxy_struct_shorthand_observe(
    path: &Path,
    observe_json: &serde_json::Value,
) {
    let dialogue_view = find_dialogue_view_object(observe_json);
    assert!(rich_text_text_run_has_object_proxy(dialogue_view, |proxy| proxy
        ["id"]
        == "hotspot"
        && proxy["type_name"] == "KeywordHit"
        && proxy["declaration"]["struct_name"] == "KeywordHit"
        && proxy["declaration"]["attribute"] == "text_proxy"
        && proxy["role"] == "keyword"
        && proxy["depth"] == 4000
        && proxy["hit_test"] == true
        && proxy["params"]["channel"]["value"] == "inventory",));
    assert!(rich_text_text_run_has_object_proxy(dialogue_view, |proxy| proxy
        ["id"]
        == "HoverHit"
        && proxy["type_name"] == "HoverHit"
        && proxy["declaration"]["struct_name"] == "HoverHit"
        && proxy["declaration"]["attribute"] == "text_proxy"
        && proxy["role"] == "hover"
        && proxy["layer"] == "view"
        && proxy["depth"] == 7000
        && proxy["hit_test"] == true
        && proxy["params"]["tone"]["value"] == "alert",));
    assert!(rich_text_text_run_has_effect(dialogue_view, "sparkle"));

    assert_inferred_text_proxy_presentation_tree_indexes(path, observe_json);

    let hover = find_rich_text_proxy_object(observe_json, "HoverHit", "Hit");
    assert_eq!(hover["role"], "rich_text_proxy");
    assert_eq!(hover["rich_text_ref"]["kind"], "text_object_proxy");
    assert_eq!(hover["rich_text_ref"]["object_layer"], "view");
    assert_eq!(hover["rich_text_ref"]["object_depth"], 7000);
    let hover_uri = rich_text_object_capture_uri(hover, "object_id", "application/octet-stream");
    let hover_width = hover["bbox"]["width"]
        .as_u64()
        .expect("inferred proxy bbox width");
    let hover_height = hover["bbox"]["height"]
        .as_u64()
        .expect("inferred proxy bbox height");
    assert_agent_read_uri_object_id_image_matches_object_color(
        path,
        hover_uri,
        hover,
        hover_width,
        hover_height,
    );
}

fn assert_inferred_text_proxy_presentation_tree_indexes(
    path: &Path,
    observe_json: &serde_json::Value,
) {
    let proxy_type_nodes = observe_json["presentation_tree"]["nodes"]
        .as_array()
        .expect("presentation tree nodes are reported")
        .iter()
        .filter(|node| rich_text_proxy_tree_node_indexes_keyword_hit(node))
        .count();
    assert!(
        proxy_type_nodes >= 2,
        "presentation tree should index KeywordHit on run and proxy objects: {observe_json}"
    );

    let proxy_type_tree = read_agent_presentation_tree_resource(
        path,
        "arcweft://session/cli/frame/0/presentation-tree.json?proxy_type=KeywordHit",
        "proxy-type",
    );
    assert!(presentation_tree_has_object_proxy(
        &proxy_type_tree,
        |proxy| { proxy["type_name"] == "KeywordHit" }
    ));

    let proxy_param_tree = read_agent_presentation_tree_resource(
        path,
        "arcweft://session/cli/frame/0/presentation-tree.json?proxy_param.channel=inventory",
        "proxy-param",
    );
    assert!(presentation_tree_has_object_proxy(
        &proxy_param_tree,
        |proxy| { proxy["params"]["channel"]["value"] == "inventory" }
    ));
}

fn rich_text_proxy_tree_node_indexes_keyword_hit(node: &serde_json::Value) -> bool {
    node["object_proxies"].as_array().is_some_and(|proxies| {
        proxies.iter().any(|proxy| {
            proxy["id"] == "hotspot"
                && proxy["type_name"] == "KeywordHit"
                && proxy["role"] == "keyword"
                && proxy["declaration"]["struct_name"] == "KeywordHit"
                && proxy["declaration"]["attribute"] == "text_proxy"
                && proxy["params"]["channel"]["value"] == "inventory"
        })
    })
}

fn read_agent_presentation_tree_resource(
    path: &Path,
    uri: &str,
    context: &str,
) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--read-uri")
        .arg(uri)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .unwrap_or_else(|error| panic!("arcw agent observe reads {context} tree: {error}"));
    assert!(
        output.status.success(),
        "agent observe {context} presentation tree read should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{context} tree read is JSON: {error}"))
}

fn presentation_tree_has_object_proxy(
    tree: &serde_json::Value,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> bool {
    tree["body"]["body"]["nodes"]
        .as_array()
        .expect("presentation tree nodes are returned")
        .iter()
        .any(|node| {
            node["object_proxies"]
                .as_array()
                .is_some_and(|proxies| proxies.iter().any(&predicate))
        })
}

fn hit_test_center_of_observed_object(
    path: &Path,
    object: &serde_json::Value,
) -> serde_json::Value {
    let center_x = agent_json_bbox_x(&object["bbox"]) + agent_json_bbox_width(&object["bbox"]) / 2;
    let center_y = agent_json_bbox_y(&object["bbox"]) + agent_json_bbox_height(&object["bbox"]) / 2;
    let hit_test = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("hit-test")
        .arg(path)
        .arg("--x")
        .arg(center_x.to_string())
        .arg("--y")
        .arg(center_y.to_string())
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent hit-test runs inferred rich text source");

    assert!(
        hit_test.status.success(),
        "agent hit-test for inferred proxy should succeed, stderr: {}",
        String::from_utf8_lossy(&hit_test.stderr)
    );
    serde_json::from_slice(&hit_test.stdout).expect("hit-test output is JSON")
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native observe and hit-test coverage"]
fn agent_mcp_stdio_hit_test_reports_depth_sorted_rich_text_proxy() {
    let path = temp_arcw(
        "agent-mcp-hit-test-rich-text-proxy",
        r#"
#[text_proxy(kind="keyword", default_hit=true, depth=4, channel=choice)]
pub struct KeywordHit {
    channel: String
}

#[text_proxy(kind="hover", default_hit=true, depth=7, layer=view)]
pub struct HoverHit {
    layer: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [object .hotspot type=KeywordHit][object .hover type=HoverHit]Hit[/object][/object][p]
}
"#,
    );
    let observe = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe runs MCP hit-test source");
    assert!(
        observe.status.success(),
        "agent observe for MCP hit-test should succeed, stderr: {}",
        String::from_utf8_lossy(&observe.stderr)
    );
    let observe_json: serde_json::Value =
        serde_json::from_slice(&observe.stdout).expect("observe output is JSON");
    let hover = find_rich_text_proxy_object(&observe_json, "hover", "Hit");
    let x = agent_json_bbox_x(&hover["bbox"]) + agent_json_bbox_width(&hover["bbox"]) / 2;
    let y = agent_json_bbox_y(&hover["bbox"]) + agent_json_bbox_height(&hover["bbox"]) / 2;
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.hit_test",
                "arguments": {
                    "source": path.display().to_string(),
                    "x": x,
                    "y": y,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp MCP hit-test source");
    assert!(
        output.status.success(),
        "agent MCP hit-test should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[1]["result"]["content"][0]["type"], "text");
    let hit_json = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "MCP hit-test result is JSON",
    );
    assert_eq!(hit_json["status"], "ok");
    assert_eq!(hit_json["top_object_id"], hover["id"]);
    let top = &hit_json["hits"][0];
    assert_eq!(top["rank"], 0);
    assert_eq!(top["object_id"], hover["id"]);
    assert_eq!(top["role"], "rich_text_proxy");
    assert_eq!(top["region"]["kind"], "text_object_proxy");
    assert_eq!(top["region"]["proxy_id"], "hover");
    assert_eq!(top["region"]["proxy_type"], "HoverHit");
    assert_text_proxy_declaration(&top["region"]["proxy_declaration"], "HoverHit");
    assert_eq!(top["region"]["proxy_layer"], "view");
    assert_eq!(top["layer"], "view");
    assert_agent_hit_capture_refs_match(top, hover);
    assert_eq!(top["rich_text_ref"]["object_layer"], "view");
    assert_eq!(top["depth"], 7000);
    let keyword_hit = hit_json["hits"]
        .as_array()
        .expect("MCP hit-test hits are listed")
        .iter()
        .find(|hit| hit["region"]["proxy_id"] == "hotspot" && hit["depth"] == 4000)
        .unwrap_or_else(|| {
            panic!("MCP hit-test should keep the lower-ranked proxy hit with params: {hit_json}")
        });
    assert_eq!(
        keyword_hit["region"]["proxy_params"]["channel"]["value"],
        "choice"
    );
}

#[test]
fn agent_hit_test_capture_time_follows_animated_text_proxy_bounds() {
    let path = temp_arcw(
        "agent-hit-test-animated-rich-text-proxy",
        r#"
#[text_proxy(kind="keyword", default_hit=true, depth=4, channel=choice)]
pub struct KeywordHit {
    channel: String
}

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [object .hotspot type=KeywordHit][.wave amp=60px dir=0,1 target=run]Hit[/][/object][p]
}
"#,
    );
    let early = observe_animated_proxy_hit_test_source(&path, "0");
    let late = observe_animated_proxy_hit_test_source(&path, "0.25");
    let early_proxy = find_rich_text_proxy_object(&early, "hotspot", "Hit");
    let late_proxy = find_rich_text_proxy_object(&late, "hotspot", "Hit");
    let early_bottom = agent_json_bbox_bottom(&early_proxy["bbox"]);
    let late_center_x =
        agent_json_bbox_x(&late_proxy["bbox"]) + agent_json_bbox_width(&late_proxy["bbox"]) / 2;
    let late_center_y =
        agent_json_bbox_y(&late_proxy["bbox"]) + agent_json_bbox_height(&late_proxy["bbox"]) / 2;
    assert!(
        late_center_y > early_bottom,
        "late wave proxy center should move outside the early proxy bbox: early={early_proxy} late={late_proxy}"
    );

    let late_hit = hit_test_animated_proxy_source_at(&path, late_center_x, late_center_y, "0.25");
    assert_eq!(late_hit["status"], "ok");
    assert_eq!(late_hit["top_object_id"], late_proxy["id"]);
    assert_eq!(late_hit["hits"][0]["region"]["kind"], "text_object_proxy");
    assert_eq!(late_hit["hits"][0]["region"]["proxy_id"], "hotspot");
    assert_eq!(
        late_hit["hits"][0]["region"]["proxy_params"]["channel"]["value"],
        "choice"
    );

    let early_hit = hit_test_animated_proxy_source_at(&path, late_center_x, late_center_y, "0");
    assert!(
        early_hit["hits"]
            .as_array()
            .expect("early hit-test hits are listed")
            .iter()
            .all(|hit| hit["region"]["proxy_id"] != "hotspot"),
        "early hit-test should not use the late animated proxy bounds: {early_hit}"
    );

    let dir = temp_dir("agent-hit-test-animated-rich-text-proxy-capture");
    let late_object_id = late_proxy["id"]
        .as_str()
        .expect("late animated proxy object id is reported");
    let mask_path = dir.join("animated-proxy-mask.rgba");
    let (mask_json, mask_bytes) =
        capture_animated_proxy_raw_at(&path, late_object_id, "mask", &mask_path, "0.25");
    assert_animated_proxy_capture_uses_late_bounds(
        &mask_json,
        &mask_bytes,
        early_proxy,
        late_proxy,
        "mask_attachment",
    );
    assert_eq!(
        mask_json["images"][0]["object"]["rich_text_ref"]["presentation"]["object_proxies"][0]["params"]
            ["channel"]["value"],
        "choice"
    );

    let object_id_path = dir.join("animated-proxy-object-id.rgba");
    let (object_id_json, object_id_bytes) =
        capture_animated_proxy_raw_at(&path, late_object_id, "object-id", &object_id_path, "0.25");
    assert_animated_proxy_capture_uses_late_bounds(
        &object_id_json,
        &object_id_bytes,
        early_proxy,
        late_proxy,
        "object_id_attachment",
    );
    assert_raw_object_id_tint_bytes(
        &object_id_bytes,
        agent_object_id_color_from_json(late_proxy),
        object_id_json["images"][0]["content_pixels"]
            .as_u64()
            .expect("animated proxy object-id content pixels"),
        "animated proxy object-id capture-time crop",
    );

    fs::remove_dir_all(&dir).expect("remove temp animated proxy capture dir");
    fs::remove_file(&path).expect("remove temp animated hit-test source");
}

fn observe_animated_proxy_hit_test_source(path: &Path, capture_time: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .arg("--capture-time")
        .arg(capture_time)
        .output()
        .expect("arcw agent observe runs animated hit-test source");
    assert!(
        output.status.success(),
        "agent observe for animated hit-test should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("animated hit-test observe output is JSON")
}

fn capture_animated_proxy_raw_at(
    path: &Path,
    object_id: &str,
    capture_kind: &str,
    raw_path: &Path,
    capture_time: &str,
) -> (serde_json::Value, Vec<u8>) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg(object_id)
        .arg("--out")
        .arg(raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .arg("--capture-time")
        .arg(capture_time)
        .output()
        .expect("arcw agent observe captures animated rich text proxy");
    assert!(
        output.status.success(),
        "animated rich text proxy {capture_kind} capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("animated proxy capture output is JSON");
    let bytes = fs::read(raw_path).expect("read animated proxy raw capture");
    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    assert_eq!(bytes.len() as u64, width * height * 4);
    (json, bytes)
}

fn assert_animated_proxy_capture_uses_late_bounds(
    json: &serde_json::Value,
    bytes: &[u8],
    early_proxy: &serde_json::Value,
    late_proxy: &serde_json::Value,
    composition: &str,
) {
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "object");
    assert_eq!(json["images"][0]["scope"]["id"], late_proxy["id"]);
    assert_eq!(json["images"][0]["composition"], composition);
    assert_eq!(
        json["images"][0]["object"]["rich_text_ref"]["kind"],
        "text_object_proxy"
    );
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        bytes.chunks_exact(4).any(|pixel| pixel[3] > 0),
        "animated proxy {composition} raw capture should contain selected pixels"
    );
    let content = &json["images"][0]["content_viewport_bbox"];
    assert!(
        agent_json_bboxes_intersect(content, &late_proxy["bbox"]),
        "animated proxy capture content should overlap the late observed bbox: image={} late={late_proxy}",
        json["images"][0]
    );
    assert!(
        !agent_json_bboxes_intersect(content, &early_proxy["bbox"]),
        "animated proxy capture content should not use the stale early bbox: image={} early={early_proxy}",
        json["images"][0]
    );
}

fn hit_test_animated_proxy_source_at(
    path: &Path,
    x: u64,
    y: u64,
    capture_time: &str,
) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("hit-test")
        .arg(path)
        .arg("--x")
        .arg(x.to_string())
        .arg("--y")
        .arg(y.to_string())
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .arg("--capture-time")
        .arg(capture_time)
        .output()
        .expect("arcw agent hit-test runs animated rich text source");
    assert!(
        output.status.success(),
        "agent hit-test for animated rich text should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("animated hit-test output is JSON")
}

fn assert_rich_text_page_and_line_aggregate_proxy_metadata(observe_json: &serde_json::Value) {
    let objects = observe_json["objects"]
        .as_array()
        .expect("observed objects are listed");
    let page = objects
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.page.0")
        .expect("proxy text page object is observed");
    assert_eq!(page["rich_text_ref"]["object_layer"], "view");
    assert_eq!(page["rich_text_ref"]["object_depth"], 7000);
    let page_hover_hit = rich_text_proxy_hit_region(page, "hover", 0, 3);
    assert_eq!(page_hover_hit["proxy_type"], "HoverHit");
    assert_text_proxy_declaration(&page_hover_hit["proxy_declaration"], "HoverHit");
    assert_eq!(page_hover_hit["proxy_layer"], "view");
    assert_eq!(page_hover_hit["depth"], 7000);

    let line = objects
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.line.0")
        .expect("proxy text line object is observed");
    assert_eq!(line["rich_text_ref"]["object_layer"], "view");
    assert_eq!(line["rich_text_ref"]["object_depth"], 7000);
    let line_keyword_hit = rich_text_proxy_hit_region(line, "hotspot", 0, 3);
    assert_eq!(line_keyword_hit["proxy_type"], "KeywordHit");
    assert_text_proxy_declaration(&line_keyword_hit["proxy_declaration"], "KeywordHit");
    assert_eq!(line_keyword_hit["depth"], 4000);
    assert_eq!(
        line_keyword_hit["proxy_params"]["channel"]["value"],
        "choice"
    );
}

#[test]
fn agent_observe_profile_selected_dialogue_defaults_drive_native_debug_output() {
    let dir = temp_dir("agent-observe-profile-dialogue-defaults");
    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).expect("create profiled observe source dir");
    let source_path = src_dir.join("main.arcw");
    fs::write(&source_path, profiled_observe_source()).expect("write profiled observe source");
    let manifest_path = dir.join("arcw.toml");
    fs::write(&manifest_path, profiled_observe_manifest())
        .expect("write profiled observe manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg("--manifest")
        .arg(&manifest_path)
        .arg("--profile")
        .arg("mobile")
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("128")
        .output()
        .expect("arcw agent observe runs profiled source");

    fs::remove_dir_all(&dir).expect("remove profiled observe fixture");
    assert!(
        output.status.success(),
        "profiled agent observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("profiled observe output is JSON");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["source"], "main.arcw");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["mime_type"], "image/png");
    assert!(
        json["images"][0]["content_pixels"]
            .as_u64()
            .is_some_and(|pixels| pixels > 0),
        "profiled native image should contain rendered pixels: {json}"
    );

    let dialogue_view = find_dialogue_view_object(&json);
    let base_styles = observed_object_rich_text_frame(dialogue_view)["base_styles"]
        .as_array()
        .expect("base styles are reported");
    assert!(
        base_styles.iter().any(|style| {
            style["kind"] == "font"
                && style["family"]["kind"] == "named"
                && style["family"]["name"] == "Meiryo"
        }),
        "mobile dialogue defaults should select the Meiryo base font: {dialogue_view}"
    );
    assert!(
        base_styles
            .iter()
            .any(|style| style["kind"] == "size" && style["raw"] == "24px"),
        "mobile dialogue defaults should select the 24px base text size: {dialogue_view}"
    );

    let contributions = observed_object_rich_text_frame(dialogue_view)["style_contributions"]
        .as_array()
        .expect("style contributions are reported");
    assert!(contributions.iter().any(|contribution| {
        contribution["path"] == "view"
            && contribution["value"] == "@view.MobileDialogue"
            && contribution["active"] == true
            && contribution["source"]["item_id"] == "dialogue.mobile"
    }));
    assert!(contributions.iter().any(|contribution| {
        contribution["path"] == "rich_text.ruby.gap"
            && contribution["value"] == "1px"
            && contribution["active"] == true
            && contribution["source"]["item_id"] == "dialogue.mobile"
    }));
    assert!(contributions.iter().any(|contribution| {
        contribution["path"] == "rich_text.effect"
            && contribution["value"] == "shake"
            && contribution["active"] == true
    }));
}

fn profiled_observe_source() -> &'static str {
    r##"
pub view DesktopDialogue(dialogue: DialogueView) {
    Panel {
        Text(dialogue.speaker)
        RichText(dialogue.content)
    }
}

pub view MobileDialogue(dialogue: DialogueView) {
    Panel {
        Text(dialogue.speaker)
        RichText(dialogue.content)
    }
}

pub dialogue defaults {
    view = @view.DesktopDialogue
    rich_text {
        text {
            font = "Yu Gothic"
            size = 30px
            color = rgb("#d9f2ff")
        }
        ruby {
            size = 14px
            gap = 2px
        }
    }
}

pub dialogue defaults @dialogue.mobile {
    view = @view.MobileDialogue
    rich_text {
        text {
            font = "Meiryo"
            size = 24px
            color = rgb("#f0f8ff")
        }
        ruby {
            size = 10px
            gap = 1px
        }
    }
}

pub character alice {
    display = "Alice"
}

entry cli @entry.main {
    goto @flow.main
}

flow main {
    alice.say(id=@say.profiled.001)[
        Profile defaults: |[星影](ほしかげ) と [.shake amp=1px seed=profiled]揺れる文字[/]。[p]
    ]

    return "done"
}
"##
}

fn profiled_observe_manifest() -> &'static str {
    r#"
[package]
name = "profiled-agent-observe"

[profiles.desktop]
kind = "cli"
entry = "entry.main"
source = "src/main.arcw"
adapter = "sans-io"

[profiles.mobile]
kind = "cli"
entry = "entry.main"
source = "src/main.arcw"
adapter = "sans-io"
dialogue_defaults = "dialogue.mobile"
"#
}

#[test]
fn agent_observe_json_reports_rich_text_reset_controls_and_host_markers() {
    let path = temp_arcw(
        "agent-observe-rich-text-controls",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [color red]Hot[reset]Cool[w 500ms][mark .sync][clear][voice auto][p]
}
",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe runs rich text controls source");
    fs::remove_file(&path).expect("remove temp agent observe controls source");

    assert!(
        output.status.success(),
        "agent observe controls should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("agent observe controls output is JSON");
    let object = &json["objects"][0];
    assert_eq!(object["text"], "HotCool");
    let runs = observed_object_rich_text_frame(object)["display_map"]["text_runs"]
        .as_array()
        .expect("text runs are listed");
    let hot = runs
        .iter()
        .find(|run| run["range"]["start"] == 0 && run["range"]["end"] == 3)
        .expect("styled Hot run is reported");
    assert!(
        hot["styles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|style| style["kind"] == "color")
    );
    let cool = runs
        .iter()
        .find(|run| run["range"]["start"] == 3 && run["range"]["end"] == 7)
        .expect("post-reset Cool run is reported");
    assert!(
        cool["styles"].as_array().unwrap().is_empty(),
        "reset should clear active inline styles for following display runs"
    );
    let controls = observed_object_rich_text_frame(object)["display_map"]["controls"]
        .as_array()
        .expect("control markers are listed");
    assert!(
        controls
            .iter()
            .any(|control| control["control"]["kind"] == "reset")
    );
    assert!(controls.iter().any(|control| {
        control["control"]["kind"] == "timed_wait" && control["control"]["value"] == "time=500ms"
    }));
    assert!(controls.iter().any(|control| {
        control["control"]["kind"] == "mark" && control["control"]["name"] == ".sync"
    }));
    assert!(
        controls
            .iter()
            .any(|control| control["control"]["kind"] == "clear")
    );
    assert!(
        controls
            .iter()
            .any(|control| control["control"]["kind"] == "page")
    );
    assert!(
        observed_object_rich_text_frame(object)["display_map"]["host_events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["event"]["kind"] == "voice")
    );
}
