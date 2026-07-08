fn imq_is_available() -> bool {
    Command::new("imq")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn capture_native_png_report(source_path: &Path, png_path: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--out")
        .arg(png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native PNG");

    assert!(
        output.status.success(),
        "native PNG capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(png_path).expect("read native PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    serde_json::from_slice(&output.stdout).expect("native capture report is JSON")
}

fn observe_native_textbox_object_raw_report(
    source_path: &Path,
    raw_path: &Path,
    capture_kind: &str,
    extra_args: &[&str],
) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
    command
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64");
    command.args(extra_args);
    let output = command
        .output()
        .expect("arcw agent observe writes native textbox object raw crop");

    assert!(
        output.status.success(),
        "native textbox object {capture_kind} capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native textbox object raw report is JSON");
    let width = json["images"][0]["width"]
        .as_u64()
        .expect("native textbox object raw width is reported");
    let height = json["images"][0]["height"]
        .as_u64()
        .expect("native textbox object raw height is reported");
    assert_eq!(
        fs::read(raw_path)
            .expect("read native textbox object raw crop")
            .len(),
        4 * usize::try_from(width).expect("raw width fits usize")
            * usize::try_from(height).expect("raw height fits usize")
    );
    json
}

fn observe_native_rich_text_layer_report(source_path: &Path) -> serde_json::Value {
    observe_native_rich_text_layer_report_with_viewport_and_textbox_height(
        source_path,
        1280,
        720,
        0,
    )
}

fn observe_native_rich_text_layer_report_with_viewport_and_textbox_height(
    source_path: &Path,
    viewport_width: u32,
    viewport_height: u32,
    textbox_height: u32,
) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
    command
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--viewport-width")
        .arg(viewport_width.to_string())
        .arg("--viewport-height")
        .arg(viewport_height.to_string());
    if textbox_height > 0 {
        command
            .arg("--textbox-height")
            .arg(textbox_height.to_string());
    }
    let output = command
        .arg("--page")
        .arg("0")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("128")
        .output()
        .expect("arcw agent observe reports native rich-text layer");

    assert!(
        output.status.success(),
        "native rich-text layer observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("native rich-text layer report is JSON")
}

fn find_textbox_object(report: &serde_json::Value) -> &serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| object["role"] == "dialogue_textbox")
        .unwrap_or_else(|| panic!("textbox object should be observed: {report}"))
}

fn find_textbox_object_by_rich_text_line<'a>(
    report: &'a serde_json::Value,
    line: &str,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "dialogue_textbox"
                && observed_object_rich_text_frame(object)["line"] == line
        })
        .unwrap_or_else(|| {
            panic!("textbox object for rich-text line `{line}` should be observed: {report}")
        })
}

fn observed_object_rich_text_frame(object: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(object["content"]["kind"], "rich_text");
    assert!(
        object.get("rich_text").is_none(),
        "observed rich-text objects must use content.frame, not root rich_text: {object}"
    );
    &object["content"]["frame"]
}

fn rich_text_text_runs(textbox: &serde_json::Value) -> &[serde_json::Value] {
    observed_object_rich_text_frame(textbox)["display_map"]["text_runs"]
        .as_array()
        .unwrap_or_else(|| panic!("textbox display_map should expose text_runs: {textbox}"))
}

fn rich_text_text_run_has_effect(textbox: &serde_json::Value, id: &str) -> bool {
    rich_text_text_run_effect_count(textbox, id) > 0
}

fn rich_text_text_run_effect_count(textbox: &serde_json::Value, id: &str) -> usize {
    rich_text_text_runs(textbox)
        .iter()
        .flat_map(|run| {
            run["presentation"]["effects"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .filter(|effect| effect["id"] == id)
        .count()
}

fn rich_text_text_run_has_shader(textbox: &serde_json::Value, id: &str) -> bool {
    rich_text_text_runs(textbox).iter().any(|run| {
        run["presentation"]["shaders"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|shader| shader["id"] == id)
    })
}

fn rich_text_text_run_has_object_proxy(
    textbox: &serde_json::Value,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> bool {
    rich_text_text_runs(textbox).iter().any(|run| {
        run["presentation"]["object_proxies"]
            .as_array()
            .into_iter()
            .flatten()
            .any(&predicate)
    })
}

fn assert_full_grammar_text_object_proxy_hit_region(json: &serde_json::Value) {
    let proxy_run = find_rich_text_run_object(json, "proxy");
    assert_eq!(proxy_run["rich_text_ref"]["hit_test"], true);
    assert_eq!(proxy_run["rich_text_ref"]["object_depth"], 4000);
    let proxy_hit = rich_text_hit_region(
        proxy_run,
        "text_object_proxy",
        proxy_run["rich_text_ref"]["range"]["start"]
            .as_u64()
            .expect("proxy range start"),
        proxy_run["rich_text_ref"]["range"]["end"]
            .as_u64()
            .expect("proxy range end"),
    );
    assert_eq!(proxy_hit["proxy_id"], "hotspot");
    assert_eq!(proxy_hit["proxy_type"], "KeywordHit");
    assert_text_proxy_declaration(&proxy_hit["proxy_declaration"], "KeywordHit");
    assert_eq!(proxy_hit["proxy_role"], "keyword");
    assert!(proxy_hit["proxy_layer"].is_null());
    assert_eq!(proxy_hit["depth"], 4000);
}

fn assert_full_grammar_text_object_proxy_observed_object(
    source_path: &Path,
    json: &serde_json::Value,
) {
    let proxy_object = find_rich_text_proxy_object(json, "hotspot", "proxy");
    assert_eq!(proxy_object["role"], "rich_text_proxy");
    let parent_id = rich_text_proxy_parent_id(proxy_object);
    assert_eq!(proxy_object["parent_id"], parent_id);
    let proxy_object_id = proxy_object["id"].as_str().expect("proxy object id");
    assert_rich_text_proxy_presentation_tree_node(
        json,
        proxy_object_id,
        &parent_id,
        "hotspot",
        4000,
    );
    assert_eq!(proxy_object["text"], "proxy");
    assert_eq!(proxy_object["rich_text_ref"]["kind"], "text_object_proxy");
    assert_eq!(proxy_object["rich_text_ref"]["index"], 0);
    assert_eq!(proxy_object["rich_text_ref"]["hit_test"], true);
    assert_eq!(proxy_object["rich_text_ref"]["object_depth"], 4000);
    assert_eq!(
        proxy_object["rich_text_ref"]["presentation"]["object_proxies"]
            .as_array()
            .expect("proxy object presentation should expose object_proxies")
            .len(),
        1
    );
    let proxy = &proxy_object["rich_text_ref"]["presentation"]["object_proxies"][0];
    assert_eq!(proxy["id"], "hotspot");
    assert_eq!(proxy["type_name"], "KeywordHit");
    assert_text_proxy_declaration(&proxy["declaration"], "KeywordHit");
    assert_eq!(proxy["role"], "keyword");
    assert!(proxy["layer"].is_null());
    assert_eq!(proxy["depth"], 4000);
    assert_eq!(proxy["hit_test"], true);
    assert_agent_observe_object_capture_refs(proxy_object);
    let proxy_object_width = proxy_object["bbox"]["width"]
        .as_u64()
        .expect("proxy object bbox width");
    let proxy_object_height = proxy_object["bbox"]["height"]
        .as_u64()
        .expect("proxy object bbox height");
    let proxy_mask_uri =
        rich_text_object_capture_uri(proxy_object, "mask", "application/octet-stream");
    let proxy_mask_resource = assert_agent_read_uri_object_image_has_content(
        source_path,
        proxy_mask_uri,
        proxy_object_id,
        proxy_object_width,
        proxy_object_height,
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["kind"],
        "text_object_proxy"
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["parent_id"],
        parent_id
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["presentation"]["object_proxies"]
            [0]["declaration"]["struct_name"],
        "KeywordHit"
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["presentation"]["object_proxies"]
            [0]["params"]["channel"]["value"],
        "choice"
    );
    let proxy_object_id_uri =
        rich_text_object_capture_uri(proxy_object, "object_id", "application/octet-stream");
    assert_agent_read_uri_object_id_image_matches_object_color(
        source_path,
        proxy_object_id_uri,
        proxy_object,
        proxy_object_width,
        proxy_object_height,
    );
    let proxy_hit = rich_text_hit_region(
        proxy_object,
        "text_object_proxy",
        proxy_object["rich_text_ref"]["range"]["start"]
            .as_u64()
            .expect("proxy range start"),
        proxy_object["rich_text_ref"]["range"]["end"]
            .as_u64()
            .expect("proxy range end"),
    );
    assert_eq!(proxy_hit["proxy_id"], "hotspot");
    assert_eq!(proxy_hit["proxy_type"], "KeywordHit");
    assert_text_proxy_declaration(&proxy_hit["proxy_declaration"], "KeywordHit");
    assert_eq!(proxy_hit["proxy_role"], "keyword");
    assert!(proxy_hit["proxy_layer"].is_null());
    assert_eq!(proxy_hit["depth"], 4000);
}

fn assert_full_grammar_nested_text_object_proxies(source_path: &Path, json: &serde_json::Value) {
    let proxy_run = find_rich_text_run_object(json, "multi proxy");
    assert_eq!(proxy_run["rich_text_ref"]["hit_test"], true);
    assert_eq!(proxy_run["rich_text_ref"]["object_depth"], 7000);
    let proxies = proxy_run["rich_text_ref"]["presentation"]["object_proxies"]
        .as_array()
        .expect("nested proxy run should expose object_proxies");
    assert_eq!(
        proxies.len(),
        2,
        "nested proxy run should carry both authored proxy objects: {proxy_run}"
    );
    let keyword = proxies
        .iter()
        .find(|proxy| proxy["id"] == "hotspot2")
        .expect("nested proxy run should keep outer keyword proxy");
    assert_eq!(keyword["type_name"], "KeywordHit");
    assert_text_proxy_declaration(&keyword["declaration"], "KeywordHit");
    assert_eq!(keyword["role"], "keyword");
    assert!(keyword["layer"].is_null());
    assert_eq!(keyword["depth"], 4000);
    assert_eq!(keyword["hit_test"], true);
    assert_eq!(keyword["params"]["channel"]["value"], "inventory");
    let hover = proxies
        .iter()
        .find(|proxy| proxy["id"] == "hover")
        .expect("nested proxy run should keep inner hover proxy");
    assert_eq!(hover["type_name"], "HoverHit");
    assert_text_proxy_declaration(&hover["declaration"], "HoverHit");
    assert_eq!(hover["role"], "hover");
    assert_eq!(hover["layer"], "view");
    assert_eq!(hover["depth"], 7000);
    assert_eq!(hover["hit_test"], true);
    assert_eq!(hover["params"]["tone"]["value"], "alert");
    assert!(hover["params"]["layer"].is_null());

    let range_start = proxy_run["rich_text_ref"]["range"]["start"]
        .as_u64()
        .expect("nested proxy range start");
    let range_end = proxy_run["rich_text_ref"]["range"]["end"]
        .as_u64()
        .expect("nested proxy range end");
    let keyword_hit = rich_text_proxy_hit_region(proxy_run, "hotspot2", range_start, range_end);
    assert_eq!(keyword_hit["proxy_type"], "KeywordHit");
    assert_text_proxy_declaration(&keyword_hit["proxy_declaration"], "KeywordHit");
    assert_eq!(keyword_hit["proxy_role"], "keyword");
    assert_eq!(keyword_hit["depth"], 4000);
    let hover_hit = rich_text_proxy_hit_region(proxy_run, "hover", range_start, range_end);
    assert_eq!(hover_hit["proxy_type"], "HoverHit");
    assert_text_proxy_declaration(&hover_hit["proxy_declaration"], "HoverHit");
    assert_eq!(hover_hit["proxy_role"], "hover");
    assert_eq!(hover_hit["proxy_layer"], "view");
    assert_eq!(hover_hit["depth"], 7000);

    assert_full_grammar_nested_proxy_observed_object(
        source_path,
        json,
        "hotspot2",
        "KeywordHit",
        "keyword",
        4000,
        ("channel", "inventory"),
    );
    assert_full_grammar_nested_proxy_observed_object(
        source_path,
        json,
        "hover",
        "HoverHit",
        "hover",
        7000,
        ("tone", "alert"),
    );
}

fn assert_full_grammar_nested_proxy_observed_object(
    source_path: &Path,
    json: &serde_json::Value,
    proxy_id: &str,
    proxy_type: &str,
    proxy_role: &str,
    depth: i64,
    expected_param: (&str, &str),
) {
    let proxy_object = find_rich_text_proxy_object(json, proxy_id, "multi proxy");
    assert_eq!(proxy_object["role"], "rich_text_proxy");
    let parent_id = rich_text_proxy_parent_id(proxy_object);
    assert_eq!(proxy_object["parent_id"], parent_id);
    assert_eq!(proxy_object["rich_text_ref"]["kind"], "text_object_proxy");
    assert_eq!(proxy_object["rich_text_ref"]["hit_test"], true);
    assert_eq!(proxy_object["rich_text_ref"]["object_depth"], depth);
    let proxies = proxy_object["rich_text_ref"]["presentation"]["object_proxies"]
        .as_array()
        .expect("proxy object presentation should expose object_proxies");
    let [proxy] = proxies.as_slice() else {
        panic!("observed proxy object should carry only its selected proxy: {proxy_object}");
    };
    assert_eq!(proxy["id"], proxy_id);
    assert_eq!(proxy["type_name"], proxy_type);
    assert_text_proxy_declaration(&proxy["declaration"], proxy_type);
    assert_eq!(proxy["role"], proxy_role);
    if proxy_id == "hover" {
        assert_eq!(proxy["layer"], "view");
        assert_eq!(proxy_object["rich_text_ref"]["object_layer"], "view");
        assert_eq!(
            proxy_object["rich_text_ref"]["hit_regions"][0]["proxy_layer"],
            "view"
        );
    } else {
        assert!(proxy["layer"].is_null());
        assert!(proxy_object["rich_text_ref"]["object_layer"].is_null());
    }
    assert_eq!(proxy["depth"], depth);
    assert_eq!(proxy["hit_test"], true);
    assert_eq!(
        proxy["params"][expected_param.0]["value"], expected_param.1,
        "observed proxy object should keep typed params: {proxy_object}"
    );
    assert_agent_observe_object_capture_refs(proxy_object);
    let proxy_object_id = proxy_object["id"].as_str().expect("nested proxy object id");
    let proxy_object_width = proxy_object["bbox"]["width"]
        .as_u64()
        .expect("nested proxy object bbox width");
    let proxy_object_height = proxy_object["bbox"]["height"]
        .as_u64()
        .expect("nested proxy object bbox height");
    let proxy_object_id_uri =
        rich_text_object_capture_uri(proxy_object, "object_id", "application/octet-stream");
    assert_agent_read_uri_object_id_image_matches_object_color(
        source_path,
        proxy_object_id_uri,
        proxy_object,
        proxy_object_width,
        proxy_object_height,
    );
    let proxy_mask_uri =
        rich_text_object_capture_uri(proxy_object, "mask", "application/octet-stream");
    let proxy_mask_resource = assert_agent_read_uri_object_image_has_content(
        source_path,
        proxy_mask_uri,
        proxy_object_id,
        proxy_object_width,
        proxy_object_height,
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["kind"],
        "text_object_proxy"
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["parent_id"],
        parent_id
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["presentation"]["object_proxies"]
            [0]["params"][expected_param.0]["value"],
        expected_param.1
    );
}

fn assert_full_grammar_inferred_text_object_proxy(source_path: &Path, json: &serde_json::Value) {
    let proxy_run = find_rich_text_run_object(json, "typed proxy");
    assert_eq!(proxy_run["rich_text_ref"]["hit_test"], true);
    assert_eq!(proxy_run["rich_text_ref"]["object_depth"], 4000);
    let proxies = proxy_run["rich_text_ref"]["presentation"]["object_proxies"]
        .as_array()
        .expect("inferred proxy run should expose object_proxies");
    let [proxy] = proxies.as_slice() else {
        panic!("inferred proxy run should carry one proxy: {proxy_run}");
    };
    assert_eq!(proxy["id"], "KeywordHit");
    assert_eq!(proxy["type_name"], "KeywordHit");
    assert_eq!(proxy["role"], "keyword");
    assert_eq!(proxy["depth"], 4000);
    assert_eq!(proxy["hit_test"], true);
    assert_eq!(proxy["params"]["channel"]["value"], "typed");

    let proxy_object = find_rich_text_proxy_object(json, "KeywordHit", "typed proxy");
    assert_eq!(proxy_object["role"], "rich_text_proxy");
    assert_eq!(proxy_object["rich_text_ref"]["kind"], "text_object_proxy");
    assert_eq!(proxy_object["rich_text_ref"]["object_depth"], 4000);
    assert_agent_observe_object_capture_refs(proxy_object);
    let proxy_object_id = proxy_object["id"]
        .as_str()
        .expect("inferred proxy object id");
    let proxy_object_width = proxy_object["bbox"]["width"]
        .as_u64()
        .expect("inferred proxy object bbox width");
    let proxy_object_height = proxy_object["bbox"]["height"]
        .as_u64()
        .expect("inferred proxy object bbox height");
    let proxy_object_id_uri =
        rich_text_object_capture_uri(proxy_object, "object_id", "application/octet-stream");
    assert_agent_read_uri_object_id_image_matches_object_color(
        source_path,
        proxy_object_id_uri,
        proxy_object,
        proxy_object_width,
        proxy_object_height,
    );
    let proxy_mask_uri =
        rich_text_object_capture_uri(proxy_object, "mask", "application/octet-stream");
    let proxy_mask_resource = assert_agent_read_uri_object_image_has_content(
        source_path,
        proxy_mask_uri,
        proxy_object_id,
        proxy_object_width,
        proxy_object_height,
    );
    assert_eq!(
        proxy_mask_resource["image"]["object"]["rich_text_ref"]["presentation"]["object_proxies"]
            [0]["params"]["channel"]["value"],
        "typed"
    );
}

fn assert_full_grammar_presentation_scalar_depth(json: &serde_json::Value) {
    let run = find_rich_text_run_object(json, "z depth");
    assert_eq!(run["rich_text_ref"]["presentation"]["layer"], "hud");
    assert_eq!(run["rich_text_ref"]["presentation"]["z_index"], 3);
    assert_eq!(run["rich_text_ref"]["presentation"]["opacity"], 800);
    assert_eq!(
        run["rich_text_ref"]["presentation"]["params"]["role"]["value"],
        "caption"
    );
    assert_eq!(
        run["rich_text_ref"]["presentation"]["params"]["hover"]["value"],
        true
    );
    assert_eq!(run["rich_text_ref"]["object_layer"], "hud");
    assert_eq!(run["rich_text_ref"]["object_depth"], 3000);
}

fn assert_full_grammar_text_page_object_readback(source_path: &Path, json: &serde_json::Value) {
    let page_object = find_rich_text_page_object_by_line(json, "say.full.006", 0);
    assert_eq!(page_object["role"], "rich_text_page");
    assert_eq!(page_object["rich_text_ref"]["kind"], "text_page");
    assert_eq!(page_object["rich_text_ref"]["index"], 0);
    assert_eq!(
        page_object["rich_text_ref"]["page"].as_u64().unwrap_or(0),
        0
    );
    assert_eq!(page_object["rich_text_ref"]["hit_test"], true);
    assert_eq!(page_object["rich_text_ref"]["presentation"]["layer"], "hud");
    assert_eq!(
        page_object["rich_text_ref"]["presentation"]["params"]["role"]["value"],
        "caption"
    );
    assert_eq!(page_object["rich_text_ref"]["object_layer"], "view");
    assert_eq!(page_object["rich_text_ref"]["object_depth"], 7000);
    assert!(
        page_object["text"]
            .as_str()
            .is_some_and(|text| text.contains("明示family") && text.contains("proxy")),
        "full grammar text page object should expose the page text: {page_object}"
    );
    let range_start = page_object["rich_text_ref"]["range"]["start"]
        .as_u64()
        .expect("text page range start");
    let range_end = page_object["rich_text_ref"]["range"]["end"]
        .as_u64()
        .expect("text page range end");
    assert!(range_end > range_start);
    assert_rich_text_hit_region_matches_bbox(page_object, "text_page", range_start, range_end);
    assert_agent_observe_object_capture_refs(page_object);

    let page_object_id = page_object["id"].as_str().expect("text page object id");
    let page_object_width = page_object["bbox"]["width"]
        .as_u64()
        .expect("text page object bbox width");
    let page_object_height = page_object["bbox"]["height"]
        .as_u64()
        .expect("text page object bbox height");
    let page_mask_uri =
        rich_text_object_capture_uri(page_object, "mask", "application/octet-stream");
    assert_agent_read_uri_object_image_has_content(
        source_path,
        page_mask_uri,
        page_object_id,
        page_object_width,
        page_object_height,
    );
    let page_object_id_uri =
        rich_text_object_capture_uri(page_object, "object_id", "application/octet-stream");
    assert_agent_read_uri_object_id_image_matches_object_color(
        source_path,
        page_object_id_uri,
        page_object,
        page_object_width,
        page_object_height,
    );
}

fn assert_full_grammar_text_line_object_readback(source_path: &Path, json: &serde_json::Value) {
    let line_object = find_rich_text_line_object_by_line(json, "say.full.006", 0);
    assert_eq!(line_object["role"], "rich_text_line");
    assert_eq!(line_object["rich_text_ref"]["kind"], "text_line");
    assert_eq!(line_object["rich_text_ref"]["index"], 0);
    assert_eq!(
        line_object["rich_text_ref"]["page"].as_u64().unwrap_or(0),
        0
    );
    assert_eq!(line_object["rich_text_ref"]["hit_test"], true);
    assert_eq!(line_object["rich_text_ref"]["presentation"]["layer"], "hud");
    assert_eq!(
        line_object["rich_text_ref"]["presentation"]["params"]["role"]["value"],
        "caption"
    );
    assert_eq!(line_object["rich_text_ref"]["object_layer"], "view");
    assert_eq!(line_object["rich_text_ref"]["object_depth"], 7000);
    assert!(
        line_object["text"]
            .as_str()
            .is_some_and(|text| text.contains("明示family") && text.contains("proxy")),
        "full grammar text line object should expose the authored line text: {line_object}"
    );
    let range_start = line_object["rich_text_ref"]["range"]["start"]
        .as_u64()
        .expect("text line range start");
    let range_end = line_object["rich_text_ref"]["range"]["end"]
        .as_u64()
        .expect("text line range end");
    assert!(range_end > range_start);
    assert_rich_text_hit_region_matches_bbox(line_object, "text_line", range_start, range_end);
    assert_agent_observe_object_capture_refs(line_object);

    let line_object_id = line_object["id"].as_str().expect("text line object id");
    let line_object_width = line_object["bbox"]["width"]
        .as_u64()
        .expect("text line object bbox width");
    let line_object_height = line_object["bbox"]["height"]
        .as_u64()
        .expect("text line object bbox height");
    let line_mask_uri =
        rich_text_object_capture_uri(line_object, "mask", "application/octet-stream");
    assert_agent_read_uri_object_image_has_content(
        source_path,
        line_mask_uri,
        line_object_id,
        line_object_width,
        line_object_height,
    );
    let line_object_id_uri =
        rich_text_object_capture_uri(line_object, "object_id", "application/octet-stream");
    assert_agent_read_uri_object_id_image_matches_object_color(
        source_path,
        line_object_id_uri,
        line_object,
        line_object_width,
        line_object_height,
    );
}

fn rich_text_text_run_has_transform(
    textbox: &serde_json::Value,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> bool {
    rich_text_text_runs(textbox).iter().any(|run| {
        let transform = &run["presentation"]["transform"];
        !transform.is_null() && predicate(transform)
    })
}

fn assert_object_capture_ref_matches_image(
    object: &serde_json::Value,
    image: &serde_json::Value,
    kind: &str,
    mime_type: &str,
) {
    let capture = object["capture_refs"]["captures"]
        .as_array()
        .expect("object capture refs are reported")
        .iter()
        .find(|capture| capture["kind"] == kind && capture["mime_type"] == mime_type)
        .unwrap_or_else(|| panic!("object should expose {kind} {mime_type} capture ref: {object}"));
    assert_eq!(
        capture["width"], image["width"],
        "{kind} capture ref width should match actual image metadata"
    );
    assert_eq!(
        capture["height"], image["height"],
        "{kind} capture ref height should match actual image metadata"
    );
}

fn assert_native_rich_text_layer_image_has_content(report: &serde_json::Value) {
    let image = &report["images"][0];
    assert_eq!(image["kind"], "color");
    assert_eq!(image["renderer"], "native");
    assert_eq!(image["scope"]["kind"], "layer");
    assert_eq!(image["scope"]["id"], "dialogue.rich_text");
    assert_eq!(image["composition"], "isolated_regions");
    assert_eq!(image["mime_type"], "image/png");
    assert!(image["content_pixels"].as_u64().unwrap() > 0);
}

fn assert_agent_native_visual_diagnostic(report: &serde_json::Value, code: &str, effect_id: &str) {
    assert_agent_native_visual_diagnostic_in(&report["diagnostics"], code, effect_id);
}

fn assert_agent_native_visual_image_diagnostic(
    report: &serde_json::Value,
    code: &str,
    effect_id: &str,
) {
    assert_agent_native_visual_diagnostic_in(&report["images"][0]["diagnostics"], code, effect_id);
}

fn assert_agent_native_visual_resource_diagnostic(
    resource: &serde_json::Value,
    code: &str,
    effect_id: &str,
) {
    assert_agent_native_visual_diagnostic_in(&resource["image"]["diagnostics"], code, effect_id);
}

fn assert_agent_native_visual_diagnostic_in(
    diagnostics: &serde_json::Value,
    code: &str,
    effect_id: &str,
) {
    let diagnostic = diagnostics
        .as_array()
        .expect("diagnostics are reported")
        .iter()
        .find(|diagnostic| {
            diagnostic["source"] == "native_rich_text"
                && diagnostic["code"] == code
                && diagnostic["effect_id"] == effect_id
        })
        .unwrap_or_else(|| {
            panic!(
                "native rich-text diagnostic {code}/{effect_id} should be structured: {diagnostics}"
            )
        });
    assert_eq!(diagnostic["severity"], "warning");
    assert!(
        diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.contains(code)),
        "diagnostic message should retain the human-readable code: {diagnostic}"
    );
}

fn find_rich_text_run_object<'a>(
    report: &'a serde_json::Value,
    text: &str,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| object["role"] == "rich_text_run" && object["text"] == text)
        .unwrap_or_else(|| panic!("rich-text run `{text}` should be observed: {report}"))
}

fn find_rich_text_proxy_object<'a>(
    report: &'a serde_json::Value,
    proxy_id: &str,
    text: &str,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_proxy"
                && object["text"] == text
                && object["rich_text_ref"]["presentation"]["object_proxies"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|proxy| proxy["id"] == proxy_id)
        })
        .unwrap_or_else(|| {
            panic!("rich-text proxy `{proxy_id}` for `{text}` should be observed: {report}")
        })
}

fn assert_rich_text_run_object_has_effect<'a>(
    run: &'a serde_json::Value,
    id: &str,
) -> &'a serde_json::Value {
    run["rich_text_ref"]["presentation"]["effects"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|effect| effect["id"] == id)
        .unwrap_or_else(|| panic!("rich-text run object should carry effect `{id}`: {run}"))
}

fn assert_rich_text_run_object_has_shader<'a>(
    run: &'a serde_json::Value,
    id: &str,
) -> &'a serde_json::Value {
    run["rich_text_ref"]["presentation"]["shaders"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|shader| shader["id"] == id)
        .unwrap_or_else(|| panic!("rich-text run object should carry shader `{id}`: {run}"))
}

fn find_rich_text_cluster_object<'a>(
    report: &'a serde_json::Value,
    text: &str,
    range_start: u64,
    range_end: u64,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_cluster"
                && object["text"] == text
                && object["rich_text_ref"]["range"]["start"].as_u64() == Some(range_start)
                && object["rich_text_ref"]["range"]["end"].as_u64() == Some(range_end)
        })
        .unwrap_or_else(|| {
            panic!(
                "rich-text cluster `{text}` {range_start}..{range_end} should be observed: {report}"
            )
        })
}

fn find_rich_text_glyph_object<'a>(
    report: &'a serde_json::Value,
    text: &str,
    range_start: u64,
    range_end: u64,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_glyph"
                && object["text"] == text
                && object["rich_text_ref"]["range"]["start"].as_u64() == Some(range_start)
                && object["rich_text_ref"]["range"]["end"].as_u64() == Some(range_end)
        })
        .unwrap_or_else(|| {
            panic!(
                "rich-text glyph `{text}` {range_start}..{range_end} should be observed: {report}"
            )
        })
}

fn find_rich_text_ruby_object(report: &serde_json::Value, index: u64) -> &serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_ruby"
                && object["rich_text_ref"]["index"].as_u64() == Some(index)
        })
        .unwrap_or_else(|| panic!("rich-text ruby `{index}` should be observed: {report}"))
}

fn find_rich_text_page_object_by_line<'a>(
    report: &'a serde_json::Value,
    line: &str,
    page: u64,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_page"
                && observed_object_rich_text_frame(object)["line"] == line
                && object["rich_text_ref"]["page"].as_u64().unwrap_or(0) == page
        })
        .unwrap_or_else(|| {
            panic!("rich-text page `{line}` page {page} should be observed: {report}")
        })
}

fn find_rich_text_line_object_by_line<'a>(
    report: &'a serde_json::Value,
    line: &str,
    index: u64,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_line"
                && observed_object_rich_text_frame(object)["line"] == line
                && object["rich_text_ref"]["index"].as_u64() == Some(index)
        })
        .unwrap_or_else(|| {
            panic!("rich-text line `{line}` index {index} should be observed: {report}")
        })
}

fn first_text_run_presentation_layout(report: &serde_json::Value) -> &serde_json::Value {
    let textbox = report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| object["role"] == "dialogue_textbox")
        .unwrap_or_else(|| panic!("textbox object should be observed: {report}"));
    let run = observed_object_rich_text_frame(textbox)["display_map"]["text_runs"]
        .as_array()
        .expect("text runs are reported")
        .first()
        .unwrap_or_else(|| panic!("first text run should be observed: {report}"));
    &run["presentation"]["layout"]
}

fn assert_rich_text_cluster_metadata(
    report: &serde_json::Value,
    text: &str,
    range_start: u64,
    range_end: u64,
    orientation: &str,
    vertical_form: &str,
) {
    let cluster = find_rich_text_cluster_object(report, text, range_start, range_end);
    assert_eq!(cluster["rich_text_ref"]["orientation"], orientation);
    assert_eq!(cluster["rich_text_ref"]["vertical_form"], vertical_form);
}

fn assert_rich_text_hit_region_matches_bbox(
    object: &serde_json::Value,
    kind: &str,
    range_start: u64,
    range_end: u64,
) {
    let region = rich_text_hit_region(object, kind, range_start, range_end);
    assert_eq!(
        region["bbox"], object["bbox"],
        "{kind} hit region should match the observed object bbox: {object}"
    );
}

fn assert_rich_text_hit_region_matches_ref_bbox(
    object: &serde_json::Value,
    kind: &str,
    ref_bbox_key: &str,
    range_start: u64,
    range_end: u64,
) {
    let region = rich_text_hit_region(object, kind, range_start, range_end);
    assert_eq!(
        region["bbox"], object["rich_text_ref"][ref_bbox_key],
        "{kind} hit region should match {ref_bbox_key}: {object}"
    );
}

fn rich_text_hit_region<'a>(
    object: &'a serde_json::Value,
    kind: &str,
    range_start: u64,
    range_end: u64,
) -> &'a serde_json::Value {
    object["rich_text_ref"]["hit_regions"]
        .as_array()
        .unwrap_or_else(|| panic!("rich-text object should expose hit_regions: {object}"))
        .iter()
        .find(|region| {
            region["kind"] == kind
                && region["range"]["start"].as_u64() == Some(range_start)
                && region["range"]["end"].as_u64() == Some(range_end)
        })
        .unwrap_or_else(|| {
            panic!("rich-text object should expose {kind} hit region {range_start}..{range_end}: {object}")
        })
}

fn rich_text_proxy_hit_region<'a>(
    object: &'a serde_json::Value,
    proxy_id: &str,
    range_start: u64,
    range_end: u64,
) -> &'a serde_json::Value {
    object["rich_text_ref"]["hit_regions"]
        .as_array()
        .unwrap_or_else(|| panic!("rich-text object should expose hit_regions: {object}"))
        .iter()
        .find(|region| {
            region["kind"] == "text_object_proxy"
                && region["proxy_id"] == proxy_id
                && region["range"]["start"].as_u64() == Some(range_start)
                && region["range"]["end"].as_u64() == Some(range_end)
        })
        .unwrap_or_else(|| {
            panic!(
                "rich-text object should expose text_object_proxy hit region for {proxy_id} {range_start}..{range_end}: {object}"
            )
        })
}

fn rich_text_proxy_parent_id(object: &serde_json::Value) -> String {
    let object_id = object["id"]
        .as_str()
        .unwrap_or_else(|| panic!("rich-text object id is reported: {object}"));
    let (textbox_id, suffix) = object_id
        .split_once(".proxy.")
        .unwrap_or_else(|| panic!("rich-text proxy object id should include .proxy.: {object_id}"));
    let run_index = suffix.split('.').next().unwrap_or_else(|| {
        panic!("rich-text proxy object id should include run index: {object_id}")
    });
    format!("{textbox_id}.run.{run_index}")
}

fn presentation_tree_node<'a>(
    report: &'a serde_json::Value,
    node_id: &str,
) -> &'a serde_json::Value {
    report["presentation_tree"]["nodes"]
        .as_array()
        .unwrap_or_else(|| panic!("presentation_tree nodes are reported: {report}"))
        .iter()
        .find(|node| node["id"] == node_id)
        .unwrap_or_else(|| panic!("presentation_tree should contain node {node_id}: {report}"))
}

fn assert_rich_text_proxy_presentation_tree_node(
    report: &serde_json::Value,
    proxy_object_id: &str,
    parent_id: &str,
    proxy_id: &str,
    depth: i64,
) {
    let parent_node = presentation_tree_node(report, parent_id);
    assert!(
        parent_node["children"]
            .as_array()
            .expect("parent tree node should expose children")
            .iter()
            .any(|child| child == proxy_object_id),
        "presentation tree parent should list proxy child {proxy_object_id}: {parent_node}"
    );
    let proxy_node = presentation_tree_node(report, proxy_object_id);
    assert_eq!(proxy_node["kind"], "object");
    assert_eq!(proxy_node["parent_id"], parent_id);
    assert_eq!(proxy_node["object_id"], proxy_object_id);
    assert_eq!(proxy_node["role"], "rich_text_proxy");
    assert_eq!(proxy_node["rich_text_kind"], "text_object_proxy");
    assert_eq!(proxy_node["object_depth"], depth);
    assert_eq!(proxy_node["object_proxy_ids"][0], proxy_id);
}

fn rich_text_cluster_column_count(report: &serde_json::Value) -> usize {
    let mut columns = report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .filter(|object| object["role"] == "rich_text_cluster")
        .map(|object| agent_json_bbox_x(&object["bbox"]))
        .collect::<Vec<_>>();
    columns.sort_unstable();
    columns.dedup();
    columns.len()
}

fn assert_rich_text_object_has_mask_capture(object: &serde_json::Value, context: &str) {
    assert!(
        object["capture_refs"]["captures"]
            .as_array()
            .expect("rich-text object captures are reported")
            .iter()
            .any(|capture| capture["kind"] == "mask"
                && capture["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with(".mask.rgba"))),
        "{context} should expose native mask capture refs: {object}"
    );
}

fn agent_json_bboxes_intersect(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    agent_json_bbox_x(left) < agent_json_bbox_right(right)
        && agent_json_bbox_x(right) < agent_json_bbox_right(left)
        && agent_json_bbox_y(left) < agent_json_bbox_bottom(right)
        && agent_json_bbox_y(right) < agent_json_bbox_bottom(left)
}

fn agent_json_bbox_x(bbox: &serde_json::Value) -> u64 {
    bbox["x"].as_u64().expect("bbox x is reported")
}

fn agent_json_bbox_y(bbox: &serde_json::Value) -> u64 {
    bbox["y"].as_u64().expect("bbox y is reported")
}

fn agent_json_bbox_height(bbox: &serde_json::Value) -> u64 {
    bbox["height"].as_u64().expect("bbox height is reported")
}

fn agent_json_bbox_width(bbox: &serde_json::Value) -> u64 {
    bbox["width"].as_u64().expect("bbox width is reported")
}

fn agent_json_bbox_center_x_twice(bbox: &serde_json::Value) -> u64 {
    agent_json_bbox_x(bbox)
        .saturating_mul(2)
        .saturating_add(agent_json_bbox_width(bbox))
}

fn agent_json_bbox_right(bbox: &serde_json::Value) -> u64 {
    agent_json_bbox_x(bbox) + bbox["width"].as_u64().expect("bbox width is reported")
}

fn agent_json_bbox_bottom(bbox: &serde_json::Value) -> u64 {
    agent_json_bbox_y(bbox) + bbox["height"].as_u64().expect("bbox height is reported")
}

fn assert_vertical_cluster_after(
    previous: &serde_json::Value,
    next: &serde_json::Value,
    context: &str,
) {
    assert_eq!(
        previous["bbox"]["x"], next["bbox"]["x"],
        "{context}: clusters should share the same vertical column"
    );
    let previous_y = previous["bbox"]["y"]
        .as_i64()
        .expect("previous cluster y is numeric");
    let next_y = next["bbox"]["y"]
        .as_i64()
        .expect("next cluster y is numeric");
    assert!(
        next_y > previous_y,
        "{context}: next cluster should advance downward within the column"
    );
}

fn rich_text_object_capture_uri<'a>(
    object: &'a serde_json::Value,
    kind: &str,
    mime_type: &str,
) -> &'a str {
    object["capture_refs"]["captures"]
        .as_array()
        .expect("rich-text object has capture refs")
        .iter()
        .find(|capture| capture["kind"] == kind && capture["mime_type"] == mime_type)
        .and_then(|capture| capture["uri"].as_str())
        .unwrap_or_else(|| {
            panic!("rich-text object should have {kind}/{mime_type} capture URI: {object}")
        })
}

fn assert_native_capture_has_content(report: &serde_json::Value, written_name: &str) {
    assert_eq!(report["images"][0]["kind"], "color");
    assert_eq!(report["images"][0]["renderer"], "native");
    assert_eq!(report["images"][0]["composition"], "framebuffer");
    assert_eq!(report["images"][0]["mime_type"], "image/png");
    assert_eq!(report["images"][0]["width"], 1280);
    assert_eq!(report["images"][0]["height"], 720);
    assert!(report["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(report["images"][0]["written"], written_name);
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    (bytes.len() >= 24 && &bytes[..8] == PNG_SIGNATURE && &bytes[12..16] == b"IHDR").then(|| {
        let width = u32::from_be_bytes(bytes[16..20].try_into().expect("IHDR width bytes"));
        let height = u32::from_be_bytes(bytes[20..24].try_into().expect("IHDR height bytes"));
        (width, height)
    })
}

fn metric_score(report: &serde_json::Value, metric_name: &str) -> f64 {
    metric_entry(report, metric_name)["score"]
        .as_f64()
        .unwrap_or_else(|| panic!("{metric_name} score should be numeric: {report}"))
}

fn metric_detail(report: &serde_json::Value, metric_name: &str, detail_name: &str) -> f64 {
    metric_entry(report, metric_name)["details"][detail_name]
        .as_f64()
        .unwrap_or_else(|| panic!("{metric_name}.{detail_name} should be numeric: {report}"))
}

fn metric_entry<'a>(report: &'a serde_json::Value, metric_name: &str) -> &'a serde_json::Value {
    report["metrics"]
        .as_array()
        .and_then(|metrics| {
            metrics
                .iter()
                .find(|metric| metric["name"].as_str() == Some(metric_name))
        })
        .unwrap_or_else(|| panic!("{metric_name} metric should be present: {report}"))
}

fn assert_metric_close(actual: f64, expected: f64, epsilon: f64, label: &str) {
    assert!(
        (actual - expected).abs() <= epsilon,
        "{label} should be {expected}, got {actual}"
    );
}

