#[test]
fn agent_observe_native_renderer_reports_windows_fonts_sample_vertical_rl_geometry() {
    let source_path = workspace_root().join("samples/rich-text-windows-fonts.arcw");
    let json = observe_native_rich_text_layer_report(&source_path);

    assert_native_rich_text_layer_image_has_content(&json);
    let run = find_rich_text_run_object(
        &json,
        "縦書きの見本。吾輩は猫である。ABC 123 2026。春夏秋冬、朝昼夕夜、天地左右。",
    );
    assert_eq!(run["entity"], "sen.say");
    assert_eq!(observed_object_rich_text_frame(run)["line"], "say.windows_fonts.001");
    assert_eq!(run["rich_text_ref"]["range"]["start"], 0);
    assert_eq!(run["rich_text_ref"]["range"]["end"], 105);
    assert!(
        run["bbox"]["height"].as_u64().unwrap() >= 120,
        "sample vertical_rl run should occupy multiple vertical cells: {run}"
    );
    assert!(
        run["bbox"]["width"].as_u64().unwrap() <= 400,
        "sample vertical_rl run should be column-shaped rather than one long horizontal line: {run}"
    );
    assert!(
        run["capture_refs"]["captures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capture| capture["kind"] == "mask"
                && capture["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with(".mask.rgba")))
    );
    assert_windows_fonts_sample_vertical_cluster_readback(&source_path, &json);
}

#[test]
fn agent_observe_native_renderer_reports_full_grammar_sample_vertical_inference_geometry() {
    let source_path = workspace_root().join("samples/rich-text-full-grammar.arcw");
    let json = observe_native_rich_text_layer_report(&source_path);

    assert_native_rich_text_layer_image_has_content(&json);
    let textbox = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| {
            object["role"] == "dialogue_textbox" && observed_object_rich_text_frame(object)["line"] == "say.full.005"
        })
        .expect("target textbox object is observed");
    let vertical_rl_display_run = observed_object_rich_text_frame(textbox)["display_map"]["text_runs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|run| {
            run["range"]["start"].as_u64() == Some(27) && run["range"]["end"].as_u64() == Some(63)
        })
        .expect("vertical_rl display-map run is observed");
    assert_eq!(
        vertical_rl_display_run["presentation"]["layout"]["jlreq_strictness"],
        "strict"
    );
    let vertical_rl = find_rich_text_run_object(&json, "吾輩は猫である。ABC 123 2026");
    assert_eq!(vertical_rl["entity"], "bob.say");
    assert_eq!(observed_object_rich_text_frame(vertical_rl)["line"], "say.full.005");
    assert_eq!(vertical_rl["rich_text_ref"]["range"]["start"], 27);
    assert_eq!(vertical_rl["rich_text_ref"]["range"]["end"], 63);
    assert!(
        vertical_rl["bbox"]["height"].as_u64().unwrap() >= 120,
        "full grammar vertical_rl run should preserve column geometry: {vertical_rl}"
    );
    assert!(
        vertical_rl["bbox"]["width"].as_u64().unwrap() <= 260,
        "full grammar vertical_rl run should not flatten into a horizontal line: {vertical_rl}"
    );
    let first_vertical_cluster = find_rich_text_cluster_object(&json, "吾", 27, 30);
    assert_eq!(
        first_vertical_cluster["rich_text_ref"]["kind"],
        "glyph_cluster"
    );
    assert_eq!(first_vertical_cluster["rich_text_ref"]["source"], "text");
    assert_eq!(observed_object_rich_text_frame(first_vertical_cluster)["line"], "say.full.005");
    assert_eq!(first_vertical_cluster["bbox"]["width"], 30);
    assert_eq!(first_vertical_cluster["bbox"]["height"], 30);
    assert!(
        first_vertical_cluster["capture_refs"]["captures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capture| capture["kind"] == "mask"
                && capture["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with(".mask.rgba")))
    );
    let first_vertical_cluster_mask_uri =
        rich_text_object_capture_uri(first_vertical_cluster, "mask", "application/octet-stream");
    assert_agent_read_uri_object_image_has_content(
        &source_path,
        first_vertical_cluster_mask_uri,
        first_vertical_cluster["id"].as_str().unwrap(),
        30,
        30,
    );

    let sideways_latin = find_rich_text_cluster_object(&json, "ABC", 51, 54);
    assert_eq!(
        sideways_latin["rich_text_ref"]["orientation"],
        "sideways_cw"
    );
    assert_eq!(sideways_latin["rich_text_ref"]["vertical_form"], "none");
    assert!(
        sideways_latin["bbox"]["height"].as_u64().unwrap()
            > sideways_latin["bbox"]["width"].as_u64().unwrap(),
        "vertical sideways Latin should be observed as one rotated run-shaped cluster: {sideways_latin}"
    );

    let vertical_lr = find_rich_text_run_object(&json, "縦LR");
    assert_eq!(observed_object_rich_text_frame(vertical_lr)["line"], "say.full.005");
    assert_eq!(vertical_lr["rich_text_ref"]["range"]["start"], 66);
    assert_eq!(vertical_lr["rich_text_ref"]["range"]["end"], 71);
    assert!(
        vertical_lr["bbox"]["height"].as_u64().unwrap()
            > vertical_lr["bbox"]["width"].as_u64().unwrap(),
        "short vertical_lr sample run should be visibly vertical: {vertical_lr}"
    );
    assert_full_grammar_sample_vertical_lr_cluster_readback(&source_path, &json);
}

#[test]
fn agent_observe_native_renderer_reports_full_grammar_sample_rich_text_constructs() {
    let source_path = workspace_root().join("samples/rich-text-full-grammar.arcw");
    let json = observe_native_rich_text_layer_report(&source_path);

    assert_native_rich_text_layer_image_has_content(&json);
    assert!(
        json["diagnostics"].as_array().is_some_and(Vec::is_empty),
        "full-grammar sample should render without native diagnostics: {json}"
    );

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
        find_textbox_object_by_rich_text_line(&json, line);
    }

    let inferred = find_textbox_object_by_rich_text_line(&json, "say.full.005");
    assert!(rich_text_text_run_has_effect(inferred, "wave"));
    assert!(rich_text_text_run_has_effect(inferred, "shake"));
    assert!(rich_text_text_run_has_effect(inferred, "typewriter"));
    assert!(rich_text_text_run_has_effect(inferred, "arc"));
    assert!(
        rich_text_text_run_effect_count(inferred, "sparkle") >= 2,
        "custom and .host-dispatched sparkle effects should both survive lowering: {inferred}"
    );
    assert!(rich_text_text_run_has_transform(
        inferred,
        |transform| transform["translate"]["x"] == 4000
            && transform["translate"]["y"] == -2000
            && transform["origin"] == "baseline_start"
            && transform["target"] == "glyph",
    ));
    assert!(rich_text_text_run_has_transform(
        inferred,
        |transform| transform["rotate"]["degrees"] == 8000
            && transform["origin"] == "center"
            && transform["target"] == "run",
    ));
    assert!(rich_text_text_run_has_transform(
        inferred,
        |transform| transform["scale"]["x"] == 1200
            && transform["scale"]["y"] == 1200
            && transform["origin"] == "baseline_center"
            && transform["target"] == "run",
    ));
    assert_full_grammar_typewriter_capture_step_readback(&source_path, &json);
    assert_full_grammar_animated_effect_readbacks(&source_path, &json);

    let explicit = find_textbox_object_by_rich_text_line(&json, "say.full.006");
    assert!(rich_text_text_run_has_transform(
        explicit,
        |transform| transform["skew"]["x"] == 2000
            && transform["skew"]["y"] == 0
            && transform["origin"] == "glyph_center"
            && transform["target"] == "glyph",
    ));
    assert!(rich_text_text_run_has_effect(explicit, "jitter"));
    assert!(rich_text_text_run_has_shader(explicit, "soft_glow"));
    assert!(rich_text_text_run_has_object_proxy(explicit, |proxy| proxy
        ["id"]
        == "hotspot"
        && proxy["type_name"] == "KeywordHit"
        && proxy["role"] == "keyword"
        && proxy["depth"] == 4000
        && proxy["hit_test"] == true
        && proxy["params"]["channel"]["value"] == "choice",));
    assert_full_grammar_text_object_proxy_hit_region(&json);
    assert_full_grammar_text_object_proxy_observed_object(&source_path, &json);
    assert_full_grammar_nested_text_object_proxies(&source_path, &json);
    assert_full_grammar_inferred_text_object_proxy(&source_path, &json);
    assert_full_grammar_presentation_scalar_depth(&json);
    assert_full_grammar_text_page_object_readback(&source_path, &json);
    assert_full_grammar_text_line_object_readback(&source_path, &json);
    assert_full_grammar_soft_glow_shader_readback(&source_path, &json);

    let cue = find_textbox_object_by_rich_text_line(&json, "say.full.007");
    assert_eq!(cue["text"], "cue: 代替");
    let raw_short = find_textbox_object_by_rich_text_line(&json, "say.full.008");
    assert!(
        raw_short["text"]
            .as_str()
            .is_some_and(|text| text.contains("[p]や#[expr]をそのまま表示")),
        "raw shorthand text should render literally: {raw_short}"
    );
    let raw_block = find_textbox_object_by_rich_text_line(&json, "say.full.009");
    assert!(
        raw_block["text"]
            .as_str()
            .is_some_and(|text| text.contains("[.shake]") && text.contains("#[player_name]")),
        "raw block text should keep rich-text tags and interpolation literally: {raw_block}"
    );
}

#[test]
#[ignore = "milestone-only rich-text effects animation sample; runs many native object captures"]
fn agent_observe_native_renderer_captures_combined_typewriter_animation_sample() {
    let source_path = workspace_root().join("samples/rich-text-effects-animation.arcw");
    let json = observe_native_rich_text_layer_report(&source_path);

    assert_native_rich_text_layer_image_has_content(&json);
    assert!(
        json["diagnostics"].as_array().is_some_and(Vec::is_empty),
        "effects animation sample should render without native diagnostics: {json}"
    );

    for line in [
        "say.effects.shader",
        "say.effects.post",
        "say.effects.motion",
        "say.effects.reveal",
    ] {
        find_textbox_object_by_rich_text_line(&json, line);
    }

    let combo_run = find_rich_text_run_object(&json, "重ね掛けtypewriter");
    assert_eq!(observed_object_rich_text_frame(combo_run)["line"], "say.effects.reveal");
    for effect in ["typewriter", "wave", "shake", "sparkle"] {
        assert_rich_text_run_object_has_effect(combo_run, effect);
    }

    let object_id = combo_run["id"]
        .as_str()
        .expect("combined typewriter run object id is reported");
    let dir = temp_dir("agent-observe-rich-text-effects-animation-combo");
    let hidden_mask_path = dir.join("combo-hidden-mask.rgba");
    let visible_mask_path = dir.join("combo-visible-mask.rgba");
    let early_color_path = dir.join("combo-visible-color-4000.rgba");
    let late_color_path = dir.join("combo-visible-color-4500.rgba");

    let (hidden_mask, hidden_mask_bytes) = observe_full_grammar_typewriter_run_mask_at(
        &source_path,
        &hidden_mask_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "0"],
    );
    assert_eq!(hidden_mask["steps"], 3);
    assert_eq!(hidden_mask["capture_time_millis"], 0);
    assert_eq!(hidden_mask["images"][0]["capture_step"], 3);
    assert_eq!(hidden_mask["images"][0]["content_pixels"], 0);
    assert_eq!(opaque_pixel_count(&hidden_mask_bytes), 0);

    let (visible_mask, visible_mask_bytes) = observe_full_grammar_typewriter_run_mask_at(
        &source_path,
        &visible_mask_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    assert_eq!(visible_mask["steps"], 3);
    assert_eq!(visible_mask["capture_time_millis"], 4000);
    assert_eq!(visible_mask["images"][0]["capture_step"], 3);
    assert!(
        visible_mask["images"][0]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(
        opaque_pixel_count(&visible_mask_bytes) as u64,
        visible_mask["images"][0]["content_pixels"]
            .as_u64()
            .unwrap()
    );

    let (early_color, early_color_bytes) = observe_full_grammar_run_color_at(
        &source_path,
        &early_color_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    let (late_color, late_color_bytes) = observe_full_grammar_run_color_at(
        &source_path,
        &late_color_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4.5"],
    );
    assert_eq!(early_color["capture_time_millis"], 4000);
    assert_eq!(late_color["capture_time_millis"], 4500);
    assert_full_grammar_color_captures_differ(
        "combined typewriter + wave + shake + sparkle",
        &early_color,
        &early_color_bytes,
        &late_color,
        &late_color_bytes,
    );

    assert_effects_animation_function_motion_run_changes_over_time(&source_path, &json, &dir);
    assert_effects_animation_source_local_effect_run_changes_over_time(&source_path, &json, &dir);
    assert_effects_animation_source_local_effect_post_process_run_is_tinted(
        &source_path,
        &json,
        &dir,
    );
    assert_effects_animation_source_local_shader_run_is_tinted(&source_path, &json, &dir);
    assert_effects_animation_source_local_shader_post_process_run_is_tinted(
        &source_path,
        &json,
        &dir,
    );
    assert_effects_animation_warm_glow_shader_run_is_tinted(&source_path, &json, &dir);
    assert_effects_animation_color_sparkle_run_is_tinted(&source_path, &json, &dir);
    assert_effects_animation_post_process_effect_runs_execute(&source_path, &json, &dir);
    assert_effects_animation_spin_pulse_run_changes_over_time(&source_path, &json, &dir);
    assert_effects_animation_vertical_spin_pulse_run_changes_over_time(&source_path, &json, &dir);

    fs::remove_dir_all(&dir).expect("remove rich-text effects animation combo temp dir");
}

#[test]
fn agent_observe_native_rich_text_reports_structured_visual_diagnostics() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-structured-diagnostics",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [effect .missing_fx amp=2px]missing effect[/effect] and [effect .shader id=ghost_glow phase=run_offscreen_pass]missing shader[/effect][p]
}
",
    );

    let json = observe_native_rich_text_layer_report(&path);

    assert_native_rich_text_layer_image_has_content(&json);
    assert_agent_native_visual_diagnostic(&json, "missing_custom_effect", "missing_fx");
    assert_agent_native_visual_diagnostic(&json, "missing_shader", "ghost_glow");
    assert_agent_native_visual_image_diagnostic(&json, "missing_custom_effect", "missing_fx");
    assert_agent_native_visual_image_diagnostic(&json, "missing_shader", "ghost_glow");

    let read_uri = "arcweft://session/cli/frame/0/layer.dialogue.rich_text.png";
    let read_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--read-uri")
        .arg(read_uri)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("128")
        .output()
        .expect("arcw agent observe reads structured diagnostic image URI");
    fs::remove_file(&path).expect("remove temp structured diagnostics source");

    assert!(
        read_output.status.success(),
        "structured diagnostic read-uri should succeed, stderr: {}",
        String::from_utf8_lossy(&read_output.stderr)
    );
    let resource: serde_json::Value =
        serde_json::from_slice(&read_output.stdout).expect("read-uri resource is JSON");
    assert_eq!(resource["uri"], read_uri);
    assert_agent_native_visual_resource_diagnostic(
        &resource,
        "missing_custom_effect",
        "missing_fx",
    );
    assert_agent_native_visual_resource_diagnostic(&resource, "missing_shader", "ghost_glow");
}

#[test]
fn agent_observe_native_rich_text_reports_missing_motion_diagnostics_in_image_resources() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-missing-motion-diagnostics",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.motion fn=ghost_orbit amp=4px target=glyph]missing motion[/][p]
}
",
    );

    let json = observe_native_rich_text_layer_report(&path);

    assert_native_rich_text_layer_image_has_content(&json);
    assert_agent_native_visual_diagnostic(&json, "missing_motion_function", "motion");
    assert_agent_native_visual_image_diagnostic(&json, "missing_motion_function", "motion");

    let read_uri = "arcweft://session/cli/frame/0/layer.dialogue.rich_text.png";
    let read_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--read-uri")
        .arg(read_uri)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("128")
        .output()
        .expect("arcw agent observe reads missing motion diagnostic image URI");
    fs::remove_file(&path).expect("remove temp missing motion diagnostics source");

    assert!(
        read_output.status.success(),
        "missing motion diagnostic read-uri should succeed, stderr: {}",
        String::from_utf8_lossy(&read_output.stderr)
    );
    let resource: serde_json::Value =
        serde_json::from_slice(&read_output.stdout).expect("read-uri resource is JSON");
    assert_eq!(resource["uri"], read_uri);
    assert_agent_native_visual_resource_diagnostic(&resource, "missing_motion_function", "motion");
}

fn assert_effects_animation_function_motion_run_changes_over_time(
    source_path: &Path,
    json: &serde_json::Value,
    dir: &Path,
) {
    let motion_run = find_rich_text_run_object(json, "関数motion");
    let motion_effect = assert_rich_text_run_object_has_effect(motion_run, "motion");
    assert_eq!(
        motion_effect["params"]["fn"]["value"], "breath_orbit",
        "motion run should carry the referenced Arcweft animation function id: {motion_run}"
    );
    let object_id = motion_run["id"]
        .as_str()
        .expect("function motion run object id is reported");
    let motion_node = presentation_tree_node(json, object_id);
    assert_eq!(motion_node["effects"][0]["id"], "motion");
    assert_eq!(motion_node["motion_function_ids"][0], "breath_orbit");
    let early_path = dir.join("function-motion-color-4000.rgba");
    let late_path = dir.join("function-motion-color-4500.rgba");
    let (early, early_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &early_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    let (late, late_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &late_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4.5"],
    );
    assert_full_grammar_color_captures_differ(
        "function-backed motion animation",
        &early,
        &early_bytes,
        &late,
        &late_bytes,
    );
}

fn assert_effects_animation_source_local_effect_run_changes_over_time(
    source_path: &Path,
    json: &serde_json::Value,
    dir: &Path,
) {
    let effect_run = find_rich_text_run_object(json, "関数effect");
    let effect = assert_rich_text_run_object_has_effect(effect_run, "source_drift");
    assert_eq!(
        effect["params"]["shape"]["value"], "elastic",
        "source-local effect should keep registry-owned raw params: {effect_run}"
    );
    let object_id = effect_run["id"]
        .as_str()
        .expect("source-local effect run object id is reported");
    let early_path = dir.join("source-local-effect-color-4000.rgba");
    let late_path = dir.join("source-local-effect-color-4500.rgba");
    let (early, early_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &early_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    let (late, late_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &late_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4.5"],
    );
    assert!(
        early["diagnostics"].as_array().is_some_and(Vec::is_empty),
        "source-local text effect should register before native capture: {early}"
    );
    assert!(
        late["diagnostics"].as_array().is_some_and(Vec::is_empty),
        "source-local text effect should register before native capture: {late}"
    );
    assert_full_grammar_color_captures_differ(
        "source-local pure text effect",
        &early,
        &early_bytes,
        &late,
        &late_bytes,
    );
}

fn assert_effects_animation_source_local_effect_post_process_run_is_tinted(
    source_path: &Path,
    json: &serde_json::Value,
    dir: &Path,
) {
    let effect_run = find_rich_text_run_object(json, "source post effect");
    let effect = assert_rich_text_run_object_has_effect(effect_run, "source_drift");
    assert_eq!(
        effect["phase"], "post_process",
        "source-local effect should keep its post_process phase: {effect_run}"
    );
    let object_id = effect_run["id"]
        .as_str()
        .expect("source-local post-process effect run object id is reported");
    let color_path = dir.join("source-local-effect-post-process-rgba-4000.rgba");
    let (capture, bytes) = observe_full_grammar_run_color_at(
        source_path,
        &color_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    assert!(
        capture["diagnostics"].as_array().is_some_and(Vec::is_empty),
        "source-local post-process text effect should register before native capture: {capture}"
    );
    assert!(
        bytes.chunks_exact(4).any(|pixel| {
            pixel[0] > pixel[1].saturating_add(15)
                && pixel[2] > pixel[1].saturating_add(10)
                && pixel[3] > 0
        }),
        "source-local post-process effect should tint the object crop with magenta pixels: {capture}"
    );
}

fn assert_effects_animation_warm_glow_shader_run_is_tinted(
    source_path: &Path,
    json: &serde_json::Value,
    dir: &Path,
) {
    let shader_run = find_rich_text_run_object(json, "warm glow shader");
    let shader = assert_rich_text_run_object_has_shader(shader_run, "warm_glow");
    assert_eq!(
        shader["phase"], "run_offscreen_pass",
        "warm_glow should be a run_offscreen_pass shader: {shader_run}"
    );
    let object_id = shader_run["id"]
        .as_str()
        .expect("warm glow shader run object id is reported");
    let color_path = dir.join("warm-glow-shader-rgba-4000.rgba");
    let (capture, bytes) = observe_full_grammar_run_color_at(
        source_path,
        &color_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    assert!(capture["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        bytes.chunks_exact(4).any(|pixel| {
            pixel[0] > pixel[1].saturating_add(25)
                && pixel[1] > pixel[2].saturating_add(20)
                && pixel[3] > 0
        }),
        "registered warm_glow shader should tint the object crop with warm pixels: {capture}"
    );
}

fn assert_effects_animation_source_local_shader_run_is_tinted(
    source_path: &Path,
    json: &serde_json::Value,
    dir: &Path,
) {
    let shader_run = find_rich_text_run_object(json, "source shader");
    let shader = assert_rich_text_run_object_has_shader(shader_run, "source_glow");
    assert_eq!(
        shader["phase"], "glyph_color",
        "source-local shader should keep its glyph_color phase: {shader_run}"
    );
    let object_id = shader_run["id"]
        .as_str()
        .expect("source-local shader run object id is reported");
    let color_path = dir.join("source-local-shader-rgba-4000.rgba");
    let (capture, bytes) = observe_full_grammar_run_color_at(
        source_path,
        &color_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    assert!(
        capture["diagnostics"].as_array().is_some_and(Vec::is_empty),
        "source-local text shader should register before native capture: {capture}"
    );
    assert!(
        bytes.chunks_exact(4).any(|pixel| {
            pixel[2] > pixel[0].saturating_add(15)
                && pixel[2] > pixel[1].saturating_add(10)
                && pixel[3] > 0
        }),
        "source-local shader should tint the object crop with blue/purple pixels: {capture}"
    );
}

fn assert_effects_animation_source_local_shader_post_process_run_is_tinted(
    source_path: &Path,
    json: &serde_json::Value,
    dir: &Path,
) {
    let shader_run = find_rich_text_run_object(json, "source post shader");
    let shader = assert_rich_text_run_object_has_shader(shader_run, "source_glow");
    assert_eq!(
        shader["phase"], "post_process",
        "source-local shader should keep its post_process phase: {shader_run}"
    );
    let object_id = shader_run["id"]
        .as_str()
        .expect("source-local post-process shader run object id is reported");
    let color_path = dir.join("source-local-shader-post-process-rgba-4000.rgba");
    let (capture, bytes) = observe_full_grammar_run_color_at(
        source_path,
        &color_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    assert!(
        capture["diagnostics"].as_array().is_some_and(Vec::is_empty),
        "source-local post-process text shader should register before native capture: {capture}"
    );
    assert!(
        bytes.chunks_exact(4).any(|pixel| {
            pixel[2] > pixel[0].saturating_add(20)
                && pixel[1] > pixel[0].saturating_add(15)
                && pixel[3] > 0
        }),
        "source-local post-process shader should tint the object crop with cyan/blue pixels: {capture}"
    );
}

fn assert_effects_animation_color_sparkle_run_is_tinted(
    source_path: &Path,
    json: &serde_json::Value,
    dir: &Path,
) {
    let color_run = find_rich_text_run_object(json, "色sparkle");
    let effect = assert_rich_text_run_object_has_effect(color_run, "sparkle");
    assert_eq!(
        effect["phase"], "glyph_color",
        "color sparkle should keep its glyph_color phase: {color_run}"
    );
    let object_id = color_run["id"]
        .as_str()
        .expect("color sparkle run object id is reported");
    let color_path = dir.join("color-sparkle-rgba-4000.rgba");
    let (capture, bytes) = observe_full_grammar_run_color_at(
        source_path,
        &color_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    assert!(capture["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        bytes.chunks_exact(4).any(|pixel| {
            pixel[0] > pixel[1].saturating_add(35)
                && pixel[0] > pixel[2].saturating_add(20)
                && pixel[3] > 0
        }),
        "glyph_color custom sparkle should tint the object crop: {capture}"
    );
}

fn assert_effects_animation_post_process_effect_runs_execute(
    source_path: &Path,
    json: &serde_json::Value,
    dir: &Path,
) {
    let wave_run = find_rich_text_run_object(json, "post wave effect");
    let wave = assert_rich_text_run_object_has_effect(wave_run, "wave");
    assert_eq!(
        wave["phase"], "post_process",
        "post wave effect should keep its post_process phase: {wave_run}"
    );
    let wave_object_id = wave_run["id"]
        .as_str()
        .expect("post wave run object id is reported");
    let wave_path = dir.join("post-wave-effect-rgba-4000.rgba");
    let (wave_capture, wave_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &wave_path,
        wave_object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    assert!(
        wave_capture["images"][0]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        wave_capture["diagnostics"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "post_process builtin effect should execute without native diagnostics: {wave_capture}"
    );
    assert!(
        wave_bytes.chunks_exact(4).any(|pixel| pixel[3] > 0),
        "post_process wave effect crop should contain rendered pixels: {wave_capture}"
    );

    let sparkle_run = find_rich_text_run_object(json, "post sparkle");
    let sparkle = assert_rich_text_run_object_has_effect(sparkle_run, "sparkle");
    assert_eq!(
        sparkle["phase"], "post_process",
        "post sparkle should keep its post_process phase: {sparkle_run}"
    );
    let sparkle_object_id = sparkle_run["id"]
        .as_str()
        .expect("post sparkle run object id is reported");
    let sparkle_path = dir.join("post-sparkle-rgba-4000.rgba");
    let (sparkle_capture, sparkle_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &sparkle_path,
        sparkle_object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    assert!(
        sparkle_capture["images"][0]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        sparkle_capture["diagnostics"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "post_process registry effect should execute without native diagnostics: {sparkle_capture}"
    );
    assert!(
        sparkle_bytes.chunks_exact(4).any(|pixel| {
            pixel[0] > pixel[1].saturating_add(20)
                && pixel[2] > pixel[1].saturating_add(10)
                && pixel[3] > 0
        }),
        "post_process sparkle effect should tint the object crop: {sparkle_capture}"
    );
}

fn assert_effects_animation_spin_pulse_run_changes_over_time(
    source_path: &Path,
    json: &serde_json::Value,
    dir: &Path,
) {
    let spin_pulse_run = find_rich_text_run_object(json, "回転して膨らむ文字");
    for effect in ["spin", "pulse"] {
        assert_rich_text_run_object_has_effect(spin_pulse_run, effect);
    }
    let spin_pulse_object_id = spin_pulse_run["id"]
        .as_str()
        .expect("spin/pulse run object id is reported");
    let spin_pulse_early_path = dir.join("spin-pulse-color-4000.rgba");
    let spin_pulse_late_path = dir.join("spin-pulse-color-4500.rgba");
    let (spin_pulse_early, spin_pulse_early_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &spin_pulse_early_path,
        spin_pulse_object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    let (spin_pulse_late, spin_pulse_late_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &spin_pulse_late_path,
        spin_pulse_object_id,
        &["--capture-step", "3", "--capture-time", "4.5"],
    );
    assert_full_grammar_color_captures_differ(
        "spin + pulse affine animation",
        &spin_pulse_early,
        &spin_pulse_early_bytes,
        &spin_pulse_late,
        &spin_pulse_late_bytes,
    );
}

fn assert_effects_animation_vertical_spin_pulse_run_changes_over_time(
    source_path: &Path,
    json: &serde_json::Value,
    dir: &Path,
) {
    let vertical_run = find_rich_text_run_object(json, "縦回転");
    assert_eq!(
        vertical_run["rich_text_ref"]["presentation"]["layout"]["writing_mode"], "vertical_rl",
        "vertical spin/pulse run should keep vertical_rl layout metadata: {vertical_run}"
    );
    for effect in ["spin", "pulse"] {
        assert_rich_text_run_object_has_effect(vertical_run, effect);
    }
    let object_id = vertical_run["id"]
        .as_str()
        .expect("vertical spin/pulse run object id is reported");
    let early_path = dir.join("vertical-spin-pulse-color-4000.rgba");
    let late_path = dir.join("vertical-spin-pulse-color-4500.rgba");
    let (early, early_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &early_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4"],
    );
    let (late, late_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &late_path,
        object_id,
        &["--capture-step", "3", "--capture-time", "4.5"],
    );
    assert_full_grammar_color_captures_differ(
        "vertical spin + pulse affine animation",
        &early,
        &early_bytes,
        &late,
        &late_bytes,
    );
}

fn assert_windows_fonts_sample_vertical_cluster_readback(
    source_path: &Path,
    json: &serde_json::Value,
) {
    let first_vertical_cluster = find_rich_text_cluster_object(json, "縦", 0, 3);
    assert_eq!(
        first_vertical_cluster["rich_text_ref"]["kind"],
        "glyph_cluster"
    );
    assert_eq!(first_vertical_cluster["rich_text_ref"]["source"], "text");
    assert_eq!(
        first_vertical_cluster["rich_text_ref"]["orientation"],
        "upright"
    );
    assert_eq!(
        first_vertical_cluster["rich_text_ref"]["vertical_form"],
        "none"
    );
    assert_eq!(first_vertical_cluster["bbox"]["width"], 30);
    assert_eq!(first_vertical_cluster["bbox"]["height"], 30);
    for (kind, mime_type) in [
        ("mask", "application/octet-stream"),
        ("object_id", "application/octet-stream"),
    ] {
        let uri = rich_text_object_capture_uri(first_vertical_cluster, kind, mime_type);
        assert_agent_read_uri_object_image_has_content(
            source_path,
            uri,
            first_vertical_cluster["id"].as_str().unwrap(),
            30,
            30,
        );
    }
}

fn assert_full_grammar_sample_vertical_lr_cluster_readback(
    source_path: &Path,
    json: &serde_json::Value,
) {
    let first_vertical_lr_cluster = find_rich_text_cluster_object(json, "縦", 66, 69);
    assert_eq!(
        first_vertical_lr_cluster["rich_text_ref"]["kind"],
        "glyph_cluster"
    );
    assert_eq!(first_vertical_lr_cluster["rich_text_ref"]["source"], "text");
    assert_eq!(first_vertical_lr_cluster["bbox"]["width"], 30);
    assert_eq!(first_vertical_lr_cluster["bbox"]["height"], 30);
    let first_vertical_lr_cluster_mask_uri = rich_text_object_capture_uri(
        first_vertical_lr_cluster,
        "mask",
        "application/octet-stream",
    );
    assert_agent_read_uri_object_image_has_content(
        source_path,
        first_vertical_lr_cluster_mask_uri,
        first_vertical_lr_cluster["id"].as_str().unwrap(),
        30,
        30,
    );
    let first_vertical_lr_cluster_object_id_uri = rich_text_object_capture_uri(
        first_vertical_lr_cluster,
        "object_id",
        "application/octet-stream",
    );
    assert_agent_read_uri_object_image_has_content(
        source_path,
        first_vertical_lr_cluster_object_id_uri,
        first_vertical_lr_cluster["id"].as_str().unwrap(),
        30,
        30,
    );
}

fn assert_full_grammar_typewriter_capture_step_readback(
    source_path: &Path,
    json: &serde_json::Value,
) {
    let typewriter_run = find_rich_text_run_object(json, "typewriter");
    assert_eq!(
        observed_object_rich_text_frame(typewriter_run)["line"],
        "say.full.005"
    );
    let object_id = typewriter_run["id"]
        .as_str()
        .expect("typewriter run object id is reported");
    let dir = temp_dir("agent-observe-full-grammar-typewriter-capture-step");
    let step_path = dir.join("full-grammar-typewriter-step-mask.rgba");
    let zero_path = dir.join("full-grammar-typewriter-zero-mask.rgba");

    let (step_json, step_bytes) = observe_full_grammar_typewriter_run_mask_at(
        source_path,
        &step_path,
        object_id,
        &["--capture-step", "1"],
    );
    assert_eq!(step_json["steps"], 1);
    assert_eq!(step_json["capture_time_millis"], 1000);
    assert_eq!(step_json["images"][0]["capture_step"], 1);
    assert_eq!(step_json["images"][0]["capture_time_millis"], 1000);
    assert!(step_json["images"][0]["content_pixels"].as_u64().unwrap() > 0);

    let (zero_json, zero_bytes) = observe_full_grammar_typewriter_run_mask_at(
        source_path,
        &zero_path,
        object_id,
        &["--capture-step", "1", "--capture-time", "0"],
    );
    assert_eq!(zero_json["steps"], 1);
    assert_eq!(zero_json["capture_time_millis"], 0);
    assert_eq!(zero_json["images"][0]["capture_step"], 1);
    assert_eq!(
        zero_json["images"][0]["capture_time_millis"],
        serde_json::Value::Null
    );
    assert_eq!(zero_json["images"][0]["content_pixels"], 0);
    assert_ne!(
        step_bytes, zero_bytes,
        "full grammar typewriter raw masks should differ between capture-step default time and explicit zero capture-time"
    );

    fs::remove_file(&step_path).expect("remove full grammar typewriter step raw crop");
    fs::remove_file(&zero_path).expect("remove full grammar typewriter zero raw crop");
    fs::remove_dir_all(&dir).expect("remove full grammar typewriter capture-step temp dir");
}

fn observe_full_grammar_typewriter_run_mask_at(
    source_path: &Path,
    raw_path: &Path,
    object_id: &str,
    timing_args: &[&str],
) -> (serde_json::Value, Vec<u8>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
    command
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg(object_id)
        .arg("--out")
        .arg(raw_path)
        .arg("--page")
        .arg("0")
        .arg("--mode")
        .arg("drain")
        .arg("--max-ops")
        .arg("128");
    command.args(timing_args);
    let output = command
        .output()
        .expect("arcw agent observe writes full grammar typewriter raw crop");
    assert!(
        output.status.success(),
        "full grammar typewriter raw crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("typewriter raw report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "object");
    assert_eq!(json["images"][0]["scope"]["id"], object_id);
    assert_eq!(json["images"][0]["composition"], "mask_attachment");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    let bytes = fs::read(raw_path).expect("read full grammar typewriter raw crop");
    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    assert_eq!(
        bytes.len() as u64,
        width.saturating_mul(height).saturating_mul(4),
        "raw crop length should match image metadata"
    );
    (json, bytes)
}

fn assert_full_grammar_animated_effect_readbacks(source_path: &Path, json: &serde_json::Value) {
    let dir = temp_dir("agent-observe-full-grammar-animated-effects");

    let wave_run = find_rich_text_run_object(json, "wave");
    assert_eq!(
        observed_object_rich_text_frame(wave_run)["line"],
        "say.full.005"
    );
    let wave_object_id = wave_run["id"].as_str().unwrap();
    let wave_early_path = dir.join("full-grammar-wave-time-0125.rgba");
    let wave_late_path = dir.join("full-grammar-wave-time-0375.rgba");
    let (wave_early, wave_early_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &wave_early_path,
        wave_object_id,
        &["--capture-step", "1", "--capture-time", "0.125"],
    );
    let (wave_late, wave_late_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &wave_late_path,
        wave_object_id,
        &["--capture-step", "1", "--capture-time", "0.375"],
    );
    assert_eq!(wave_early["steps"], 1);
    assert_eq!(wave_late["steps"], 1);
    assert_eq!(wave_early["capture_time_millis"], 125);
    assert_eq!(wave_late["capture_time_millis"], 375);
    assert_eq!(wave_early["images"][0]["capture_step"], 1);
    assert_eq!(wave_late["images"][0]["capture_step"], 1);
    assert_full_grammar_color_captures_differ(
        "wave glyph-transform",
        &wave_early,
        &wave_early_bytes,
        &wave_late,
        &wave_late_bytes,
    );

    let shake_run = find_rich_text_run_object(json, "shake");
    assert_eq!(
        observed_object_rich_text_frame(shake_run)["line"],
        "say.full.005"
    );
    assert_full_grammar_step_motion_differs(
        source_path,
        &dir,
        "shake",
        shake_run["id"].as_str().unwrap(),
    );

    let sparkle_run = find_rich_text_run_object(json, "custom effect");
    assert_eq!(
        observed_object_rich_text_frame(sparkle_run)["line"],
        "say.full.005"
    );
    assert_full_grammar_step_motion_differs(
        source_path,
        &dir,
        "custom-sparkle",
        sparkle_run["id"].as_str().unwrap(),
    );

    let host_run = find_rich_text_run_object(json, "host");
    assert_eq!(
        observed_object_rich_text_frame(host_run)["line"],
        "say.full.005"
    );
    assert_full_grammar_step_motion_differs(
        source_path,
        &dir,
        "host-sparkle",
        host_run["id"].as_str().unwrap(),
    );

    fs::remove_dir_all(&dir).expect("remove full grammar animated effects temp dir");
}

fn assert_full_grammar_step_motion_differs(
    source_path: &Path,
    dir: &Path,
    label: &str,
    object_id: &str,
) {
    let step_1_path = dir.join(format!("full-grammar-{label}-step-1.rgba"));
    let step_2_path = dir.join(format!("full-grammar-{label}-step-2.rgba"));
    let (step_1, step_1_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &step_1_path,
        object_id,
        &["--capture-step", "1"],
    );
    let (step_2, step_2_bytes) = observe_full_grammar_run_color_at(
        source_path,
        &step_2_path,
        object_id,
        &["--capture-step", "2"],
    );
    assert_eq!(step_1["steps"], 1);
    assert_eq!(step_2["steps"], 2);
    assert_eq!(step_1["capture_time_millis"], 1000);
    assert_eq!(step_2["capture_time_millis"], 2000);
    assert_eq!(step_1["images"][0]["capture_step"], 1);
    assert_eq!(step_2["images"][0]["capture_step"], 2);
    assert_eq!(step_1["images"][0]["capture_time_millis"], 1000);
    assert_eq!(step_2["images"][0]["capture_time_millis"], 2000);
    assert_full_grammar_color_captures_differ(
        label,
        &step_1,
        &step_1_bytes,
        &step_2,
        &step_2_bytes,
    );
}

fn observe_full_grammar_run_color_at(
    source_path: &Path,
    raw_path: &Path,
    object_id: &str,
    timing_args: &[&str],
) -> (serde_json::Value, Vec<u8>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
    command
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("color")
        .arg("--object")
        .arg(object_id)
        .arg("--out")
        .arg(raw_path)
        .arg("--page")
        .arg("0")
        .arg("--mode")
        .arg("drain")
        .arg("--max-ops")
        .arg("128");
    command.args(timing_args);
    let output = command
        .output()
        .expect("arcw agent observe writes full grammar animated effect raw crop");
    assert!(
        output.status.success(),
        "full grammar animated effect raw crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("animated effect raw report is JSON");
    let image = &json["images"][0];
    assert_eq!(image["kind"], "color");
    assert_eq!(image["renderer"], "native");
    assert_eq!(image["scope"]["kind"], "object");
    assert_eq!(image["scope"]["id"], object_id);
    assert_eq!(image["composition"], "isolated_regions");
    assert_eq!(image["mime_type"], "application/octet-stream");
    assert!(image["content_pixels"].as_u64().unwrap() > 0);

    let bytes = fs::read(raw_path).expect("read full grammar animated effect raw crop");
    let width = image["width"].as_u64().unwrap();
    let height = image["height"].as_u64().unwrap();
    assert_eq!(
        bytes.len() as u64,
        width.saturating_mul(height).saturating_mul(4),
        "raw crop length should match image metadata"
    );
    (json, bytes)
}

fn assert_full_grammar_color_captures_differ(
    label: &str,
    first: &serde_json::Value,
    first_bytes: &[u8],
    second: &serde_json::Value,
    second_bytes: &[u8],
) {
    assert_ne!(
        first_bytes, second_bytes,
        "{label} color raw captures should differ between animation samples: first={first}, second={second}"
    );
}

fn assert_full_grammar_soft_glow_shader_readback(source_path: &Path, json: &serde_json::Value) {
    let shader_run = find_rich_text_run_object(json, "shader");
    assert_eq!(
        observed_object_rich_text_frame(shader_run)["line"],
        "say.full.006"
    );
    let object_id = shader_run["id"]
        .as_str()
        .expect("shader run object id is reported");
    let shader_node = presentation_tree_node(json, object_id);
    assert_eq!(shader_node["role"], "rich_text_run");
    assert_eq!(shader_node["rich_text_kind"], "text_run");
    assert_eq!(shader_node["shaders"][0]["id"], "soft_glow");
    assert_eq!(shader_node["shaders"][0]["phase"], "run_offscreen_pass");
    let dir = temp_dir("agent-observe-full-grammar-soft-glow-shader");
    let raw_path = dir.join("full-grammar-soft-glow-shader.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("color")
        .arg("--object")
        .arg(object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--page")
        .arg("0")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("128")
        .output()
        .expect("arcw agent observe writes full grammar soft-glow raw crop");
    assert!(
        output.status.success(),
        "full grammar soft-glow raw crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("soft-glow raw report is JSON");
    let image = &report["images"][0];
    assert_eq!(image["kind"], "color");
    assert_eq!(image["renderer"], "native");
    assert_eq!(image["scope"]["kind"], "object");
    assert_eq!(image["scope"]["id"], object_id);
    assert_eq!(image["mime_type"], "application/octet-stream");
    assert_eq!(image["written"], "full-grammar-soft-glow-shader.rgba");
    assert!(image["content_pixels"].as_u64().unwrap() > 0);

    let bytes = fs::read(&raw_path).expect("read full grammar soft-glow raw crop");
    let width = image["width"].as_u64().unwrap();
    let height = image["height"].as_u64().unwrap();
    assert_eq!(
        bytes.len() as u64,
        width.saturating_mul(height).saturating_mul(4),
        "raw crop length should match image metadata"
    );
    let blue_glow_pixels = count_soft_glow_blue_pixels(&bytes);
    assert!(
        blue_glow_pixels > 32,
        "soft_glow shader should add visible blue-tinted pixels, got {blue_glow_pixels}: image={image}"
    );

    fs::remove_file(&raw_path).expect("remove full grammar soft-glow raw crop");
    fs::remove_dir_all(&dir).expect("remove full grammar soft-glow temp dir");
}

fn count_soft_glow_blue_pixels(rgba: &[u8]) -> usize {
    rgba.chunks_exact(4)
        .filter(|pixel| {
            let [red, green, blue, alpha] = [pixel[0], pixel[1], pixel[2], pixel[3]];
            alpha > 0 && blue > red.saturating_add(20) && blue > green.saturating_add(5)
        })
        .count()
}

#[test]
fn agent_observe_native_renderer_writes_sample_full_frame_png_vertical_captures() {
    let cases = [
        (
            "windows-fonts",
            workspace_root().join("samples/rich-text-windows-fonts.arcw"),
            "縦書きの見本。吾輩は猫である。ABC 123 2026。春夏秋冬、朝昼夕夜、天地左右。",
            120,
            500,
        ),
        (
            "full-grammar",
            workspace_root().join("samples/rich-text-full-grammar.arcw"),
            "吾輩は猫である。ABC 123 2026",
            120,
            260,
        ),
    ];
    let dir = temp_dir("agent-observe-native-sample-full-frame-png");
    for (label, source_path, run_text, min_height, max_width) in cases {
        let png_path = dir.join(format!("{label}-full-frame.png"));
        let json = capture_native_png_report(&source_path, &png_path);
        assert_native_capture_has_content(&json, &format!("{label}-full-frame.png"));
        let run = find_rich_text_run_object(&json, run_text);
        assert!(
            run["bbox"]["height"].as_u64().unwrap() >= min_height,
            "{label} full-frame PNG report should preserve vertical run height: {run}"
        );
        assert!(
            run["bbox"]["width"].as_u64().unwrap() <= max_width,
            "{label} full-frame PNG report should preserve column-shaped width: {run}"
        );
        assert!(
            json["images"][0]["content_bbox"]["height"]
                .as_u64()
                .is_some_and(|height| height > 0),
            "{label} full-frame PNG should contain rendered native pixels: {json}"
        );
    }
    fs::remove_dir_all(&dir).expect("remove temp native sample full-frame dir");
}

#[test]
fn agent_observe_native_renderer_writes_dialogue_layer_framebuffer_crop() {
    let path = temp_arcw(
        "agent-observe-native-dialogue-layer",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Hello native layer[p]
}
",
    );
    let dir = temp_dir("agent-observe-native-dialogue-layer");
    let png_path = dir.join("native-dialogue-layer.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native dialogue layer PNG");

    assert!(
        output.status.success(),
        "native dialogue layer crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&png_path).expect("read native dialogue layer PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native dialogue layer report is JSON");
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(json["images"][0]["scope"]["id"], "dialogue");
    assert_eq!(json["images"][0]["composition"], "framebuffer_crop");
    assert_eq!(json["images"][0]["width"], 1088);
    assert_eq!(json["images"][0]["height"], 124);
    assert_eq!(json["images"][0]["crop_origin"]["space"], "viewport");
    assert_eq!(json["images"][0]["crop_origin"]["x"], 96);
    assert_eq!(json["images"][0]["crop_origin"]["y"], 548);
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(json["images"][0]["written"], "native-dialogue-layer.png");

    fs::remove_file(&path).expect("remove temp native dialogue layer source");
    fs::remove_dir_all(&dir).expect("remove temp native dialogue layer dir");
}

#[test]
fn agent_observe_native_renderer_reports_capture_step_metadata() {
    let path = temp_arcw(
        "agent-observe-native-capture-step",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Step pinned capture[p]
}
",
    );
    let dir = temp_dir("agent-observe-native-capture-step");
    let png_path = dir.join("native-step.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--capture-step")
        .arg("3")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes capture-step native PNG");

    assert!(
        output.status.success(),
        "native capture-step crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native capture-step report is JSON");
    assert_eq!(json["steps"], 3);
    assert_eq!(json["capture_time_millis"], 3000);
    assert_eq!(json["images"][0]["capture_step"], 3);
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["written"], "native-step.png");
    let bytes = fs::read(&png_path).expect("read native capture-step PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");

    fs::remove_file(&path).expect("remove temp native capture-step source");
    fs::remove_dir_all(&dir).expect("remove temp native capture-step dir");
}

#[test]
fn agent_observe_native_capture_step_defaults_capture_time_for_typewriter() {
    let path = temp_arcw(
        "agent-observe-native-capture-step-typewriter-time",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl][.typewriter cps=1]吾輩[/][/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-capture-step-typewriter-time");
    let step_path = dir.join("native-step-typewriter-mask.rgba");
    let explicit_zero_path = dir.join("native-step-typewriter-zero-mask.rgba");

    let step_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.0.0.3")
        .arg("--out")
        .arg(&step_path)
        .arg("--mode")
        .arg("drain")
        .arg("--capture-step")
        .arg("2")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes capture-step typewriter mask");
    assert!(
        step_output.status.success(),
        "capture-step typewriter mask should succeed, stderr: {}",
        String::from_utf8_lossy(&step_output.stderr)
    );
    let step_json: serde_json::Value = serde_json::from_slice(&step_output.stdout)
        .expect("capture-step typewriter report is JSON");
    assert_eq!(step_json["steps"], 2);
    assert_eq!(step_json["capture_time_millis"], 2000);
    assert_eq!(step_json["images"][0]["capture_step"], 2);
    assert_eq!(step_json["images"][0]["capture_time_millis"], 2000);
    assert!(step_json["images"][0]["content_pixels"].as_u64().unwrap() > 0);

    let zero_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.0.0.3")
        .arg("--out")
        .arg(&explicit_zero_path)
        .arg("--mode")
        .arg("drain")
        .arg("--capture-step")
        .arg("2")
        .arg("--capture-time")
        .arg("0")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes explicit zero typewriter mask");
    assert!(
        zero_output.status.success(),
        "explicit zero typewriter mask should succeed, stderr: {}",
        String::from_utf8_lossy(&zero_output.stderr)
    );
    let zero_json: serde_json::Value = serde_json::from_slice(&zero_output.stdout)
        .expect("explicit zero typewriter report is JSON");
    assert_eq!(zero_json["steps"], 2);
    assert_eq!(zero_json["capture_time_millis"], 0);
    assert_eq!(zero_json["images"][0]["capture_step"], 2);
    assert_eq!(
        zero_json["images"][0]["capture_time_millis"],
        serde_json::Value::Null
    );
    assert_eq!(zero_json["images"][0]["content_pixels"], 0);

    fs::remove_file(&path).expect("remove temp native capture-step typewriter source");
    fs::remove_dir_all(&dir).expect("remove temp native capture-step typewriter dir");
}

#[test]
fn agent_observe_native_renderer_reports_custom_effect_diagnostics() {
    let path = temp_arcw(
        "agent-observe-native-custom-effect-diagnostic",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.unknown_custom_effect amp=2px]custom effect[/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-custom-effect-diagnostic");
    let png_path = dir.join("native-custom-effect.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reports native custom effect diagnostics");

    assert!(
        output.status.success(),
        "native custom effect diagnostic capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native custom effect report is JSON");
    assert!(
        json["diagnostics"].as_array().is_some_and(|diagnostics| {
            diagnostics.iter().any(|diagnostic| {
                diagnostic["severity"] == "warning"
                    && diagnostic["message"]
                        .as_str()
                        .is_some_and(|message| message.contains("missing_custom_effect"))
            })
        }),
        "native custom effect capture should surface renderer diagnostics: {json}"
    );
    let bytes = fs::read(&png_path).expect("read native custom effect PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");

    fs::remove_file(&path).expect("remove temp native custom effect source");
    fs::remove_dir_all(&dir).expect("remove temp native custom effect dir");
}

#[test]
fn agent_observe_native_renderer_applies_shader_glyph_color_phase() {
    let path = temp_arcw(
        "agent-observe-native-shader-glyph-color-phase",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [effect .shader id=soft_glow phase=glyph_color amount=1 dir=1,0]shader phase[/effect][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-shader-glyph-color-phase");
    let png_path = dir.join("native-shader-glyph-color-phase.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe applies native shader glyph_color");

    assert!(
        output.status.success(),
        "native shader glyph_color capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native shader report is JSON");
    assert!(
        json["diagnostics"].as_array().is_none_or(|diagnostics| {
            diagnostics.iter().all(|diagnostic| {
                diagnostic["message"]
                    .as_str()
                    .is_none_or(|message| !message.contains("unsupported_shader_phase"))
            })
        }),
        "native shader glyph_color should execute without unsupported phase diagnostics: {json}"
    );
    assert!(
        json["images"][0]["content_pixels"]
            .as_u64()
            .is_some_and(|pixels| pixels > 0),
        "native shader glyph_color capture should contain rendered glyph pixels: {json}"
    );
    let bytes = fs::read(&png_path).expect("read native shader glyph_color PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");

    fs::remove_file(&path).expect("remove temp native shader glyph_color source");
    fs::remove_dir_all(&dir).expect("remove temp native shader glyph_color dir");
}

#[test]
fn agent_observe_native_renderer_applies_shader_post_process_phase() {
    let path = temp_arcw(
        "agent-observe-native-shader-post-process-phase",
        r##"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [effect .shader id=screen_tint phase=post_process amount=1 color="#ff2020"]post process shader[/effect][p]
}
"##,
    );
    let dir = temp_dir("agent-observe-native-shader-post-process-phase");
    let raw_path = dir.join("native-shader-post-process-phase.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe applies native shader post_process");

    assert!(
        output.status.success(),
        "native shader post_process capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native shader post_process report is JSON");
    assert!(
        json["diagnostics"].as_array().is_none_or(|diagnostics| {
            diagnostics.iter().all(|diagnostic| {
                diagnostic["message"]
                    .as_str()
                    .is_none_or(|message| !message.contains("unsupported_shader_phase"))
            })
        }),
        "native shader post_process should execute without unsupported phase diagnostics: {json}"
    );
    let width = usize::try_from(json["images"][0]["width"].as_u64().expect("image width"))
        .expect("image width fits usize");
    let height = usize::try_from(json["images"][0]["height"].as_u64().expect("image height"))
        .expect("image height fits usize");
    let bytes = fs::read(&raw_path).expect("read native shader post_process RGBA");
    assert_eq!(bytes.len(), width.saturating_mul(height).saturating_mul(4));
    assert!(
        bytes.chunks_exact(4).any(|pixel| {
            pixel[0] > pixel[1].saturating_add(100)
                && pixel[0] > pixel[2].saturating_add(100)
                && pixel[3] > 0
        }),
        "native shader post_process should red-tint rendered glyph pixels: {json}"
    );

    fs::remove_file(&path).expect("remove temp native shader post_process source");
    fs::remove_dir_all(&dir).expect("remove temp native shader post_process dir");
}

#[test]
fn agent_observe_native_renderer_applies_builtin_effect_post_process_phase() {
    let path = temp_arcw(
        "agent-observe-native-builtin-effect-post-process-phase",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [effect .wave phase=post_process amp=12px period=48px dir=1,0]wave phase[/effect][p]
}
",
    );
    let baseline = temp_arcw(
        "agent-observe-native-builtin-effect-post-process-baseline",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: wave phase[p]
}
",
    );
    let dir = temp_dir("agent-observe-native-builtin-effect-post-process-phase");
    let raw_path = dir.join("native-builtin-effect-post-process-phase.rgba");
    let baseline_raw_path = dir.join("native-builtin-effect-post-process-baseline.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--capture-time")
        .arg("0.25")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe applies native builtin effect post_process");

    assert!(
        output.status.success(),
        "native builtin effect post_process capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let baseline_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&baseline)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&baseline_raw_path)
        .arg("--capture-time")
        .arg("0.25")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe captures native builtin effect baseline");
    assert!(
        baseline_output.status.success(),
        "native builtin effect baseline capture should succeed, stderr: {}",
        String::from_utf8_lossy(&baseline_output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native builtin effect post_process report is JSON");
    assert!(
        json["diagnostics"].as_array().is_none_or(|diagnostics| {
            diagnostics.iter().all(|diagnostic| {
                diagnostic["message"]
                    .as_str()
                    .is_none_or(|message| !message.contains("unsupported_builtin_effect_phase"))
            })
        }),
        "native builtin effect post_process should execute without unsupported phase diagnostics: {json}"
    );
    let bytes = fs::read(&raw_path).expect("read native builtin effect post_process RGBA");
    let baseline_bytes =
        fs::read(&baseline_raw_path).expect("read native builtin effect baseline RGBA");
    assert_ne!(
        bytes, baseline_bytes,
        "native builtin effect post_process should alter raw framebuffer pixels: {json}"
    );

    fs::remove_file(&path).expect("remove temp native builtin effect post_process source");
    fs::remove_file(&baseline).expect("remove temp native builtin effect baseline source");
    fs::remove_dir_all(&dir).expect("remove temp native builtin effect post_process dir");
}

#[test]
fn agent_observe_reports_host_event_phase_effects() {
    let path = temp_arcw(
        "agent-observe-host-event-phase-effect",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.host id=sparkle phase=host_event channel=debug]host cue[/][effect .wave phase=host_event amp=4px]wave cue[/effect][p]
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
        .expect("arcw agent observe reports host_event phase effects");

    assert!(
        output.status.success(),
        "host_event phase effect observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("host_event phase report is JSON");
    let object = json["objects"]
        .as_array()
        .expect("objects array")
        .iter()
        .find(|object| object["role"] == "dialogue_textbox")
        .expect("textbox object");
    let host_events = observed_object_rich_text_frame(object)["host_events"]
        .as_array()
        .expect("host events array");
    assert!(
        host_events.iter().any(|event| {
            event["kind"] == "effect"
                && event["id"] == "sparkle"
                && event["attrs"]
                    .as_str()
                    .is_some_and(|attrs| attrs.contains("channel=debug"))
        }),
        "host_event .host should be observed as a typed effect event: {json}"
    );
    assert!(
        host_events
            .iter()
            .any(|event| event["kind"] == "effect" && event["id"] == "wave"),
        "host_event builtin effect should be observed as a typed effect event: {json}"
    );
    assert_eq!(object["text"], "host cuewave cue");

    fs::remove_file(&path).expect("remove temp host_event phase source");
}

#[test]
fn agent_observe_native_renderer_dispatches_host_effect_registry() {
    let path = temp_arcw(
        "agent-observe-native-host-effect-registry",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.host id=sparkle amp=2px seed=custom]host effect[/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-host-effect-registry");
    let png_path = dir.join("native-host-effect.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe dispatches host effect registry");

    assert!(
        output.status.success(),
        "native host effect capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native host effect report is JSON");
    assert!(
        json["diagnostics"].as_array().is_some_and(Vec::is_empty),
        "registered host effect should not emit diagnostics: {json}"
    );
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    let bytes = fs::read(&png_path).expect("read native host effect PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");

    fs::remove_file(&path).expect("remove temp native host effect source");
    fs::remove_dir_all(&dir).expect("remove temp native host effect dir");
}

#[test]
fn agent_observe_native_renderer_writes_rich_text_layer_png_crop() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-layer",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let dir = temp_dir("agent-observe-native-rich-text-layer");
    let png_path = dir.join("native-rich-text-layer.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native rich-text layer PNG");

    assert!(
        output.status.success(),
        "native rich-text layer crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&png_path).expect("read native rich-text layer PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native rich-text layer crop report is JSON");
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(json["images"][0]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["images"][0]["composition"], "isolated_regions");
    assert_eq!(json["images"][0]["mime_type"], "image/png");
    assert!(
        json["images"][0]["width"].as_u64().unwrap() < 1088,
        "rich-text layer crop should be narrower than the textbox"
    );
    assert!(
        json["images"][0]["height"].as_u64().unwrap() < 124,
        "rich-text layer crop should be shorter than the textbox"
    );
    assert_eq!(json["images"][0]["crop_origin"]["space"], "viewport");
    assert!(
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap() >= 96,
        "rich-text layer crop origin should map to viewport coordinates"
    );
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(json["images"][0]["written"], "native-rich-text-layer.png");

    fs::remove_file(&path).expect("remove temp native rich-text layer source");
    fs::remove_dir_all(&dir).expect("remove temp native rich-text layer dir");
}

#[test]
fn agent_observe_native_renderer_handles_clear_in_rich_text_layer_capture() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-clear-layer",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
    let dir = temp_dir("agent-observe-native-rich-text-clear-layer");
    let png_path = dir.join("native-rich-text-clear-layer.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native clear rich-text layer PNG");

    assert!(
        output.status.success(),
        "native clear rich-text layer crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&png_path).expect("read native clear rich-text layer PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native clear rich-text layer crop report is JSON");
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(json["images"][0]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["images"][0]["composition"], "isolated_regions");
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(
        json["images"][0]["content_viewport_bbox"]["x"]
            .as_u64()
            .unwrap(),
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap()
            + json["images"][0]["content_bbox"]["x"].as_u64().unwrap()
    );
    let textbox = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["role"] == "dialogue_textbox")
        .expect("textbox object is observed");
    assert_eq!(textbox["text"], "BeforeAfter");
    assert!(
        observed_object_rich_text_frame(textbox)["display_map"]["controls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|control| control["control"]["kind"] == "clear")
    );

    fs::remove_file(&path).expect("remove temp native clear rich-text layer source");
    fs::remove_dir_all(&dir).expect("remove temp native clear rich-text layer dir");
}

#[test]
fn agent_observe_native_renderer_captures_clear_after_page_layer() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-page-layer",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
    let dir = temp_dir("agent-observe-native-rich-text-page-layer");
    let png_path = dir.join("native-rich-text-page-layer.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--page")
        .arg("1")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes page-selected native rich-text layer PNG");

    assert!(
        output.status.success(),
        "native page-selected rich-text layer crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&png_path).expect("read native page-selected rich-text layer PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native page-selected rich-text layer report is JSON");
    assert_page_selected_native_rich_text_layer_report(&json);

    fs::remove_file(&path).expect("remove temp native rich-text page layer source");
    fs::remove_dir_all(&dir).expect("remove temp native rich-text page layer dir");
}

fn assert_page_selected_native_rich_text_layer_report(json: &serde_json::Value) {
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["page"], 1);
    assert_eq!(json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(json["images"][0]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["images"][0]["composition"], "isolated_regions");
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    let run_object = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.run.1")
        .expect("page-selected run object is observed");
    assert_eq!(run_object["rich_text_ref"]["page"], 1);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"], run_object["bbox"]["x"],
        "page-selected layer bbox should use the visible page child x bound"
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"], run_object["bbox"]["y"],
        "page-selected layer bbox should use the visible page child y bound"
    );
    assert_eq!(
        json["images"][0]["width"], run_object["bbox"]["width"],
        "page-selected layer crop width should match the visible page child"
    );
    assert_eq!(
        json["images"][0]["height"], run_object["bbox"]["height"],
        "page-selected layer crop height should match the visible page child"
    );
}

#[test]
fn agent_observe_native_renderer_captures_clear_after_page_object() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-page-object",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
    let dir = temp_dir("agent-observe-native-rich-text-page-object");
    let png_path = dir.join("native-rich-text-page-object.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--object")
        .arg("object.dialogue.0.0.run.1")
        .arg("--page")
        .arg("1")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes page-selected native rich-text object PNG");

    assert!(
        output.status.success(),
        "native page-selected rich-text object crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&png_path).expect("read native page-selected rich-text object PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native page-selected rich-text object report is JSON");
    let page_capture_uris = assert_page_selected_native_rich_text_object_report(&json);

    for (object_id, page_capture_uri) in page_capture_uris {
        assert_agent_read_uri_page_capture_ref(&path, &page_capture_uri, &object_id);
    }

    fs::remove_file(&path).expect("remove temp native rich-text page object source");
    fs::remove_dir_all(&dir).expect("remove temp native rich-text page object dir");
}

fn assert_page_selected_native_rich_text_object_report(
    json: &serde_json::Value,
) -> Vec<(String, String)> {
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["page"], 1);
    assert_eq!(json["images"][0]["scope"]["kind"], "object");
    assert_eq!(
        json["images"][0]["scope"]["id"],
        "object.dialogue.0.0.run.1"
    );
    assert_eq!(json["images"][0]["composition"], "isolated_regions");
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        json["images"][0]["width"].as_u64().unwrap() < 1088,
        "page-selected run crop should be narrower than the textbox"
    );
    assert!(
        json["objects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|object| object["id"] == "object.dialogue.0.0.run.1")
    );
    let run_object = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.run.1")
        .expect("page-selected run object is observed");
    assert_eq!(run_object["rich_text_ref"]["page"], 1);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"], run_object["bbox"]["x"],
        "page-selected child bbox should use the same native x bound as the capture"
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"], run_object["bbox"]["y"],
        "page-selected child bbox should use the same native y bound as the capture"
    );
    assert_eq!(
        json["images"][0]["width"], run_object["bbox"]["width"],
        "page-selected child bbox width should match the native crop width"
    );
    assert_eq!(
        json["images"][0]["height"], run_object["bbox"]["height"],
        "page-selected child bbox height should match the native crop height"
    );
    let page_capture_uri = run_object["capture_refs"]["captures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capture| capture["kind"] == "color" && capture["mime_type"] == "image/png")
        .expect("page-selected run object has a color PNG capture ref");
    assert_eq!(page_capture_uri["page"], 1);
    let page_capture_uri = page_capture_uri["uri"]
        .as_str()
        .expect("page-selected run object color PNG capture ref has a URI")
        .to_owned();
    assert!(
        page_capture_uri.ends_with("/object.object.dialogue.0.0.run.1.png?page=1"),
        "page-selected rich-text child capture ref should encode page query: {page_capture_uri}"
    );
    let page_object = page_selected_rich_text_object(json, "object.dialogue.0.0.page.1");
    assert_eq!(page_object["role"], "rich_text_page");
    assert_eq!(page_object["rich_text_ref"]["kind"], "text_page");
    assert_eq!(page_object["rich_text_ref"]["page"], 1);
    assert_eq!(page_object["text"], "After");
    assert_rich_text_hit_region_matches_bbox(page_object, "text_page", 6, 11);
    let page_object_capture_uri =
        assert_page_selected_object_color_capture_ref(page_object, "object.dialogue.0.0.page.1");

    let line_object = page_selected_rich_text_object(json, "object.dialogue.0.0.line.1");
    assert_eq!(line_object["role"], "rich_text_line");
    assert_eq!(line_object["rich_text_ref"]["kind"], "text_line");
    assert_eq!(line_object["rich_text_ref"]["page"], 1);
    assert_eq!(line_object["text"], "After");
    assert_rich_text_hit_region_matches_bbox(line_object, "text_line", 6, 11);
    let line_object_capture_uri =
        assert_page_selected_object_color_capture_ref(line_object, "object.dialogue.0.0.line.1");

    vec![
        ("object.dialogue.0.0.run.1".to_owned(), page_capture_uri),
        (
            "object.dialogue.0.0.page.1".to_owned(),
            page_object_capture_uri,
        ),
        (
            "object.dialogue.0.0.line.1".to_owned(),
            line_object_capture_uri,
        ),
    ]
}

fn page_selected_rich_text_object<'a>(
    json: &'a serde_json::Value,
    object_id: &str,
) -> &'a serde_json::Value {
    json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["id"] == object_id)
        .unwrap_or_else(|| panic!("page-selected object should be observed: {object_id}"))
}

fn assert_page_selected_object_color_capture_ref(
    object: &serde_json::Value,
    object_id: &str,
) -> String {
    let capture = object["capture_refs"]["captures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capture| capture["kind"] == "color" && capture["mime_type"] == "image/png")
        .unwrap_or_else(|| panic!("page-selected object has a color PNG capture ref: {object}"));
    assert_eq!(capture["page"], 1);
    let uri = capture["uri"]
        .as_str()
        .expect("page-selected object color PNG capture ref has a URI")
        .to_owned();
    assert!(
        uri.ends_with(&format!("/object.{object_id}.png?page=1")),
        "page-selected object capture ref should encode page query: {uri}"
    );
    uri
}

fn assert_agent_read_uri_page_capture_ref(path: &Path, page_capture_uri: &str, object_id: &str) {
    let read_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--json")
        .arg("--read-uri")
        .arg(page_capture_uri)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads page-selected rich-text capture ref");
    assert!(
        read_output.status.success(),
        "page-selected rich-text capture ref read should succeed, stderr: {}",
        String::from_utf8_lossy(&read_output.stderr)
    );
    let resource: serde_json::Value = serde_json::from_slice(&read_output.stdout)
        .expect("page-selected rich-text capture ref read is JSON");
    assert_eq!(resource["kind"], "image");
    assert_eq!(resource["uri"], page_capture_uri);
    assert_eq!(resource["image"]["renderer"], "native");
    assert_eq!(resource["image"]["page"], 1);
    assert_eq!(resource["image"]["scope"]["kind"], "object");
    assert_eq!(resource["image"]["scope"]["id"], object_id);
    assert!(resource["image"]["content_pixels"].as_u64().unwrap() > 0);
}

fn assert_agent_read_uri_object_image_has_content(
    path: &Path,
    uri: &str,
    object_id: &str,
    width: u64,
    height: u64,
) -> serde_json::Value {
    let resource = read_agent_observe_object_image_resource(path, uri);
    assert_agent_read_uri_object_image_metadata(&resource, uri, object_id, width, height);
    assert!(resource["image"]["content_pixels"].as_u64().unwrap() > 0);
    resource
}

fn assert_agent_read_uri_object_id_image_matches_object_color(
    path: &Path,
    uri: &str,
    object: &serde_json::Value,
    width: u64,
    height: u64,
) {
    let object_id = object["id"].as_str().expect("object id");
    let resource = read_agent_observe_object_image_resource(path, uri);
    assert_agent_read_uri_object_image_metadata(&resource, uri, object_id, width, height);
    let content_pixels = resource["image"]["content_pixels"]
        .as_u64()
        .expect("object-id resource content pixels");
    assert!(content_pixels > 0);
    let bytes = raw_bytes_from_agent_image_resource(&resource, "object-id read-uri resource");
    assert_eq!(
        bytes.len() as u64,
        width * height * 4,
        "object-id read-uri raw image should match bbox RGBA dimensions"
    );
    assert_raw_object_id_tint_bytes(
        &bytes,
        agent_object_id_color_from_json(object),
        content_pixels,
        "object-id read-uri resource",
    );
}

fn read_agent_observe_object_image_resource(path: &Path, uri: &str) -> serde_json::Value {
    let read_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--json")
        .arg("--read-uri")
        .arg(uri)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("128")
        .output()
        .expect("arcw agent observe reads object image capture ref");
    assert!(
        read_output.status.success(),
        "object image capture ref read should succeed, stderr: {}",
        String::from_utf8_lossy(&read_output.stderr)
    );
    serde_json::from_slice(&read_output.stdout).expect("object image capture ref read is JSON")
}

fn assert_agent_read_uri_object_image_metadata(
    resource: &serde_json::Value,
    uri: &str,
    object_id: &str,
    width: u64,
    height: u64,
) {
    assert_eq!(resource["kind"], "image");
    assert_eq!(resource["uri"], uri);
    assert_eq!(resource["image"]["renderer"], "native");
    assert_eq!(resource["image"]["scope"]["kind"], "object");
    assert_eq!(resource["image"]["scope"]["id"], object_id);
    assert_eq!(resource["image"]["object"]["id"], object_id);
    assert!(
        resource["image"]["object"]["role"].as_str().is_some(),
        "object image metadata should preserve the observed object role: {resource}"
    );
    assert!(
        resource["image"]["object"]["layer"].as_str().is_some(),
        "object image metadata should preserve the observed object layer: {resource}"
    );
    assert_eq!(resource["image"]["width"], width);
    assert_eq!(resource["image"]["height"], height);
}

fn raw_bytes_from_agent_image_resource(resource: &serde_json::Value, context: &str) -> Vec<u8> {
    assert_eq!(
        resource["mime_type"], "application/octet-stream",
        "{context} should be a raw byte resource"
    );
    assert_eq!(
        resource["body"]["body_kind"], "bytes_base64",
        "{context} should use the bytes_base64 body envelope"
    );
    assert_eq!(
        resource["body"]["body"]["encoding"], "base64",
        "{context} should use base64 body encoding"
    );
    let data = resource["body"]["body"]["data"]
        .as_str()
        .unwrap_or_else(|| panic!("{context} should carry base64 image data"));
    general_purpose::STANDARD
        .decode(data)
        .unwrap_or_else(|error| panic!("{context} should decode as base64: {error}"))
}

#[test]
fn agent_observe_read_uri_preserves_animated_image_object_frame_metadata() {
    let source_path = workspace_root().join("samples/image-animation.arcw");
    let observe =
        observe_image_animation_sample_flow_at(&source_path, "image_sprite_overlay", "0.15");
    let image_object = observe["objects"]
        .as_array()
        .expect("image animation sample reports objects")
        .iter()
        .find(|object| {
            object["content"]["kind"] == "image"
                && object["content"]["object"] == "image.sample.pulse_sprite"
        })
        .expect("bounded animated image object is observed");
    let object_id = image_object["id"]
        .as_str()
        .expect("bounded image object id");
    let raw_uri = image_object["capture_refs"]["captures"]
        .as_array()
        .expect("bounded image object reports capture refs")
        .iter()
        .find(|capture| {
            capture["kind"] == "color" && capture["mime_type"] == "application/octet-stream"
        })
        .and_then(|capture| capture["uri"].as_str())
        .expect("bounded image object has raw color capture ref");

    let resource = read_image_animation_sample_resource_at(
        &source_path,
        "image_sprite_overlay",
        "0.15",
        raw_uri,
    );

    assert_agent_read_uri_object_image_metadata(&resource, raw_uri, object_id, 360, 180);
    assert_eq!(resource["image"]["composition"], "framebuffer_crop");
    assert_eq!(resource["image"]["crop_origin"]["x"], 120);
    assert_eq!(resource["image"]["crop_origin"]["y"], 84);
    assert_eq!(resource["image"]["content_pixels"], 64_800);
    let image_ref = &resource["image"]["object"]["image_ref"];
    assert_eq!(image_ref["source"], "image.sample.pulse_sprite");
    assert_eq!(image_ref["object"], "image.sample.pulse_sprite");
    assert_eq!(image_ref["target"], "target.sample.pulse_sprite");
    assert_eq!(image_ref["asset"], "asset.bg.pulse");
    assert_eq!(image_ref["frame_index"], 0);
    assert_eq!(image_ref["local_time_millis"], 50);
    assert_eq!(image_ref["opacity_milli"], 500);
    assert_eq!(image_ref["fit"], "stretch");
    assert_eq!(image_ref["alignment"]["x_milli"], 500);
    assert_eq!(image_ref["alignment"]["y_milli"], 500);
    assert_eq!(image_ref["intrinsic_width"], 2);
    assert_eq!(image_ref["intrinsic_height"], 1);
    assert_eq!(image_ref["actions"][0], "action.inspect.pulse_sprite");
    assert_eq!(
        image_ref["params"]["param.role"]["value"],
        "animated-hotspot"
    );
    assert_eq!(image_ref["proxies"][0]["id"], "proxy.pulse_sprite.hotspot");
    assert_eq!(image_ref["proxies"][0]["depth"], 2600);
    assert_eq!(image_ref["proxies"][0]["hit_test"], true);

    let bytes =
        raw_bytes_from_agent_image_resource(&resource, "animated image object read-uri resource");
    assert_eq!(bytes.len(), 360 * 180 * 4);
    assert_eq!(
        &bytes[..4],
        &[5, 26, 161, 127],
        "object-local pinned playback should return the active textured-quad pixel"
    );
    assert!(
        bytes.chunks_exact(4).any(|pixel| pixel[3] == 127),
        "half-opacity animated image resource should preserve frame alpha"
    );
}

#[test]
fn agent_observe_reports_missing_scope_for_released_image_handle_object() {
    let source_path = workspace_root().join("samples/image-animation.arcw");
    let object_id = "object.image.image.sample.pulse_sprite";
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&source_path)
        .arg("--entry")
        .arg("image_sprite_released")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .arg("--mode")
        .arg("drain")
        .arg("--capture-time")
        .arg("0.15")
        .arg("--json")
        .arg("--object")
        .arg(object_id)
        .output()
        .expect("arcw agent observe runs released image handle sample");

    assert!(
        output.status.success(),
        "released image handle observe should succeed with a structured diagnostic, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("released image observe output is JSON");

    assert!(
        json["objects"]
            .as_array()
            .expect("objects are reported")
            .iter()
            .all(|object| object["id"] != object_id),
        "released image object should be absent from observed objects: {json}"
    );
    assert!(json["diagnostics"].as_array().is_some_and(|diagnostics| {
        diagnostics.iter().any(|diagnostic| {
            diagnostic["code"] == "AGENT_CAPTURE_MISSING_SCOPE"
                && diagnostic["message"]
                    .as_str()
                    .is_some_and(|message| message.contains(object_id))
        })
    }));
}

#[test]
fn agent_observe_reports_authored_scroll_view_capture_and_release_filtering() {
    let path = temp_arcw(
        "agent-observe-authored-scroll-view",
        AUTHORED_SCROLL_AGENT_SOURCE,
    );

    let live = agent_observe_json_for_path(
        &path,
        &authored_scroll_live_observe_args(),
        "authored Scroll live observe",
    );
    assert_authored_scroll_live_observation(&live);
    let view_raw_uri = authored_scroll_view_raw_uri(&live);
    let view_resource = read_agent_observe_image_resource_for_path(
        &path,
        &view_raw_uri,
        &authored_scroll_live_read_uri_args(),
        "authored Scroll view read-uri",
    );
    assert_authored_scroll_view_resource(&view_resource, &view_raw_uri);

    let clipped = agent_observe_json_for_path(
        &path,
        &[
            "--flow",
            "scroll_agent_live",
            "--steps",
            "3",
            "--max-ops",
            "64",
            "--mode",
            "drain",
            "--json",
            "--object",
            "button.below_scroll",
        ],
        "authored Scroll clipped object observe",
    );
    assert_agent_missing_scope_diagnostic(&clipped, "button.below_scroll", "object");
    assert_released_authored_scroll_missing_scopes(&path);

    fs::remove_file(&path).expect("remove temp authored Scroll observe source");
}

const AUTHORED_SCROLL_AGENT_SOURCE: &str = r#"
pub action feedback.submit(value: String)

entry game @entry.scroll_agent_parity {
  goto @flow.scroll_agent_live
}

pub view ScrollPanel() {
  let feedback = input.text(@input:.feedback, initial = "Ada")
  Panel {
    Scroll {
      TextField(feedback)
        .label("Feedback")
        .on_submit {
          action.invoke(@action:.feedback.submit, value = feedback.text)
        }
      Button(@button:.send, label = "Send")
        .on_click {
          action.invoke(@action:.feedback.submit, value = feedback.text)
        }
      Button(@button:.more, label = "More")
        .on_click {
          action.invoke(@action:.feedback.submit, value = feedback.text)
        }
      Button(@button:.below_scroll, label = "Below")
        .on_click {
          action.invoke(@action:.feedback.submit, value = feedback.text)
        }
    }
  }
}

flow scroll_agent_live {
  let panel = view(@view:.ScrollPanel, lifetime = .manual)
  let event = receive action(@action:.feedback.submit)
  return event.value
}

flow scroll_agent_released {
  let panel = view(@view:.ScrollPanel, lifetime = .manual)
  panel.release()
  return "released"
}

flow scroll_agent_unmounted {
  let panel = view(@view:.ScrollPanel, lifetime = .manual)
  panel.unmount()
  return "unmounted"
}

flow scroll_agent_destroyed {
  let panel = view(@view:.ScrollPanel, lifetime = .manual)
  panel.destroy()
  return "destroyed"
}
"#;

fn assert_released_authored_scroll_missing_scopes(path: &Path) {
    let released_view = agent_observe_json_for_path(
        path,
        &[
            "--flow",
            "scroll_agent_released",
            "--steps",
            "4",
            "--max-ops",
            "64",
            "--mode",
            "drain",
            "--json",
            "--view",
            "view.ScrollPanel",
        ],
        "released authored Scroll view observe",
    );
    assert!(released_view["objects"].as_array().is_some_and(Vec::is_empty));
    assert!(released_view["views"].as_array().is_some_and(Vec::is_empty));
    assert_agent_missing_scope_diagnostic(&released_view, "view.ScrollPanel", "view");

    let released_object = agent_observe_json_for_path(
        path,
        &[
            "--flow",
            "scroll_agent_released",
            "--steps",
            "4",
            "--max-ops",
            "64",
            "--mode",
            "drain",
            "--json",
            "--object",
            "input.feedback",
        ],
        "released authored Scroll object observe",
    );
    assert_agent_missing_scope_diagnostic(&released_object, "input.feedback", "object");

    let unmounted_view = agent_observe_json_for_path(
        path,
        &[
            "--flow",
            "scroll_agent_unmounted",
            "--steps",
            "4",
            "--max-ops",
            "64",
            "--mode",
            "drain",
            "--json",
            "--view",
            "view.ScrollPanel",
        ],
        "unmounted authored Scroll view observe",
    );
    assert!(unmounted_view["objects"].as_array().is_some_and(Vec::is_empty));
    assert!(unmounted_view["views"].as_array().is_some_and(Vec::is_empty));
    assert_agent_missing_scope_diagnostic(&unmounted_view, "view.ScrollPanel", "view");

    let destroyed_view = agent_observe_json_for_path(
        path,
        &[
            "--flow",
            "scroll_agent_destroyed",
            "--steps",
            "4",
            "--max-ops",
            "64",
            "--mode",
            "drain",
            "--json",
            "--view",
            "view.ScrollPanel",
        ],
        "destroyed authored Scroll view observe",
    );
    assert!(destroyed_view["objects"].as_array().is_some_and(Vec::is_empty));
    assert!(destroyed_view["views"].as_array().is_some_and(Vec::is_empty));
    assert_agent_missing_scope_diagnostic(&destroyed_view, "view.ScrollPanel", "view");
}

fn assert_authored_scroll_live_observation(live: &serde_json::Value) {
    let view = authored_scroll_view(live);
    assert_eq!(view["visible"], true);
    assert_eq!(view["object_count"], 3);
    assert_eq!(view["bbox"]["x"], 48);
    assert_eq!(view["bbox"]["y"], 48);
    assert_eq!(view["bbox"]["width"], 420);
    assert_eq!(view["bbox"]["height"], 168);
    assert_eq!(
        view["object_refs"]
            .as_array()
            .expect("view reports object refs")
            .iter()
            .map(|value| value.as_str().expect("object ref is a string"))
            .collect::<Vec<_>>(),
        vec!["input.feedback", "button.send", "button.more"]
    );

    let objects = live["objects"]
        .as_array()
        .expect("live observe reports objects");
    let input = objects
        .iter()
        .find(|object| object["id"] == "input.feedback")
        .unwrap_or_else(|| panic!("Scroll-owned input should be observed: {live}"));
    assert_eq!(input["parent_id"], "view.ScrollPanel");
    assert_eq!(input["role"], "text_field");
    assert_eq!(input["text"], "Ada");
    assert_eq!(input["bbox"]["x"], 48);
    assert_eq!(input["bbox"]["y"], 48);
    assert_eq!(input["bbox"]["width"], 420);
    assert_eq!(input["bbox"]["height"], 48);
    assert!(objects.iter().any(|object| {
        object["id"] == "button.more"
            && object["parent_id"] == "view.ScrollPanel"
            && object["bbox"]["y"] == 172
    }));
    assert!(
        objects.iter().all(|object| object["id"] != "button.below_scroll"),
        "button fully outside the authored Scroll viewport must be absent: {live}"
    );
}

fn assert_authored_scroll_view_resource(resource: &serde_json::Value, expected_uri: &str) {
    assert_eq!(resource["kind"], "image");
    assert_eq!(resource["uri"], expected_uri);
    assert_eq!(resource["image"]["renderer"], "native");
    assert_eq!(resource["image"]["scope"]["kind"], "view");
    assert_eq!(resource["image"]["scope"]["id"], "view.ScrollPanel");
    assert_eq!(resource["image"]["width"], 420);
    assert_eq!(resource["image"]["height"], 168);
    assert!(resource["image"]["content_pixels"].as_u64().unwrap() > 0);
}

fn authored_scroll_view_raw_uri(live: &serde_json::Value) -> String {
    authored_scroll_view(live)["capture_refs"]["captures"]
        .as_array()
        .expect("view reports capture refs")
        .iter()
        .find(|capture| {
            capture["kind"] == "color" && capture["mime_type"] == "application/octet-stream"
        })
        .and_then(|capture| capture["uri"].as_str())
        .expect("view reports raw color capture ref")
        .to_owned()
}

fn authored_scroll_view(live: &serde_json::Value) -> &serde_json::Value {
    live["views"]
        .as_array()
        .expect("live observe reports views")
        .iter()
        .find(|view| view["id"] == "view.ScrollPanel")
        .unwrap_or_else(|| panic!("live Scroll view should be observed: {live}"))
}

fn authored_scroll_live_observe_args() -> [&'static str; 9] {
    [
        "--flow",
        "scroll_agent_live",
        "--steps",
        "3",
        "--max-ops",
        "64",
        "--mode",
        "drain",
        "--json",
    ]
}

fn authored_scroll_live_read_uri_args() -> [&'static str; 8] {
    [
        "--flow",
        "scroll_agent_live",
        "--steps",
        "3",
        "--max-ops",
        "64",
        "--mode",
        "drain",
    ]
}

fn agent_observe_json_for_path(path: &Path, args: &[&str], context: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("arcw agent observe runs {context}: {error}"));

    assert!(
        output.status.success(),
        "{context} should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{context} output is JSON: {error}"))
}

fn read_agent_observe_image_resource_for_path(
    path: &Path,
    uri: &str,
    args: &[&str],
    context: &str,
) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--json")
        .arg("--read-uri")
        .arg(uri)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("arcw agent observe reads {context}: {error}"));

    assert!(
        output.status.success(),
        "{context} should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{context} resource is JSON: {error}"))
}

fn assert_agent_missing_scope_diagnostic(
    report: &serde_json::Value,
    expected_id: &str,
    expected_kind: &str,
) {
    assert!(
        report["diagnostics"]
            .as_array()
            .expect("diagnostics are reported")
            .iter()
            .any(|diagnostic| {
                diagnostic["code"] == "AGENT_CAPTURE_MISSING_SCOPE"
                    && diagnostic["message"].as_str().is_some_and(|message| {
                        message.contains(expected_id)
                            && message.contains(&format!("observed {expected_kind}"))
                    })
            }),
        "expected missing-scope diagnostic for {expected_kind} `{expected_id}`: {report}"
    );
}

#[test]
fn agent_observe_read_uri_preserves_animated_image_layer_frame_pixels() {
    let source_path = workspace_root().join("samples/image-animation.arcw");
    let observe =
        observe_image_animation_sample_flow_at(&source_path, "image_sprite_overlay", "0.15");
    let foreground_layer = observe["layers"]
        .as_array()
        .expect("image animation sample reports layers")
        .iter()
        .find(|layer| layer["id"] == "layer.foreground")
        .expect("foreground image layer is observed");
    let raw_uri = foreground_layer["capture_refs"]["captures"]
        .as_array()
        .expect("foreground layer reports capture refs")
        .iter()
        .find(|capture| {
            capture["kind"] == "color" && capture["mime_type"] == "application/octet-stream"
        })
        .and_then(|capture| capture["uri"].as_str())
        .expect("foreground layer has raw color capture ref");

    let resource = read_image_animation_sample_resource_at(
        &source_path,
        "image_sprite_overlay",
        "0.15",
        raw_uri,
    );

    assert_eq!(resource["kind"], "image");
    assert_eq!(resource["uri"], raw_uri);
    assert_eq!(resource["image"]["renderer"], "native");
    assert_eq!(resource["image"]["scope"]["kind"], "layer");
    assert_eq!(resource["image"]["scope"]["id"], "layer.foreground");
    assert_eq!(resource["image"]["composition"], "framebuffer_crop");
    assert_eq!(resource["image"]["width"], 360);
    assert_eq!(resource["image"]["height"], 180);
    assert_eq!(resource["image"]["crop_origin"]["x"], 120);
    assert_eq!(resource["image"]["crop_origin"]["y"], 84);
    assert_eq!(resource["image"]["content_pixels"], 64_800);
    assert!(resource["image"].get("object").is_none());

    let bytes =
        raw_bytes_from_agent_image_resource(&resource, "animated image layer read-uri resource");
    assert_eq!(bytes.len(), 360 * 180 * 4);
    assert_eq!(
        &bytes[..4],
        &[5, 26, 161, 127],
        "layer read-uri should use the same object-local pinned animated frame as object capture"
    );
}

#[test]
fn agent_observe_mcp_tool_result_preserves_animated_image_object_metadata_and_raw_blob() {
    let source_path = workspace_root().join("samples/image-animation.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&source_path)
        .arg("--flow")
        .arg("image_sprite_overlay")
        .arg("--steps")
        .arg("2")
        .arg("--capture-time")
        .arg("0.15")
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/0/object.object.image.layer.foreground.0.1.rgba")
        .arg("--mcp")
        .arg("--mcp-format")
        .arg("tool-result")
        .arg("--json")
        .output()
        .expect("arcw agent observe reads animated image object raw URI as MCP tool result");

    assert!(
        output.status.success(),
        "animated image object MCP tool-result should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tool_result: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("MCP tool-result output is JSON");
    assert_eq!(tool_result["isError"], false);
    assert_eq!(tool_result["content"][0]["type"], "text");
    let metadata: serde_json::Value = serde_json::from_str(
        tool_result["content"][0]["text"]
            .as_str()
            .expect("MCP tool-result metadata is text"),
    )
    .expect("MCP tool-result metadata is JSON");
    assert_eq!(metadata["mime_type"], "application/octet-stream");
    assert_eq!(metadata["image"]["scope"]["kind"], "object");
    assert_eq!(
        metadata["image"]["scope"]["id"],
        "object.image.layer.foreground.0.1"
    );
    assert_eq!(metadata["image"]["width"], 360);
    assert_eq!(metadata["image"]["height"], 180);
    assert_eq!(
        metadata["image"]["object"]["image_ref"]["asset"],
        "asset.bg.pulse"
    );
    assert_eq!(metadata["image"]["object"]["image_ref"]["frame_index"], 0);
    assert_eq!(
        metadata["image"]["object"]["image_ref"]["local_time_millis"],
        50
    );
    assert_eq!(
        metadata["image"]["object"]["image_ref"]["params"]["param.role"]["value"],
        "animated-hotspot"
    );
    assert_eq!(
        metadata["image"]["object"]["image_ref"]["proxies"][0]["id"],
        "proxy.pulse_sprite.hotspot"
    );

    assert_eq!(tool_result["content"][1]["type"], "resource");
    assert_eq!(
        tool_result["content"][1]["resource"]["mimeType"],
        "application/octet-stream"
    );
    let blob = tool_result["content"][1]["resource"]["blob"]
        .as_str()
        .expect("MCP raw image resource carries base64 blob");
    let bytes = general_purpose::STANDARD
        .decode(blob)
        .expect("MCP raw image blob decodes");
    assert_eq!(bytes.len(), 360 * 180 * 4);
    assert_eq!(
        &bytes[..4],
        &[5, 26, 161, 127],
        "MCP raw blob should preserve the pinned animated image pixel and opacity"
    );
}

#[test]
fn agent_hit_test_reports_animated_image_object_proxy_metadata() {
    let source_path = workspace_root().join("samples/image-animation.arcw");
    let hit =
        hit_test_image_animation_sample_at(&source_path, "image_sprite_overlay", "0.15", 300, 174);

    assert_eq!(hit["status"], "ok");
    assert_eq!(hit["top_object_id"], "object.image.layer.foreground.0.1");
    let top_hit = &hit["hits"][0];
    assert_eq!(top_hit["object_id"], "object.image.layer.foreground.0.1");
    assert_eq!(top_hit["layer"], "layer.hit");
    assert_eq!(top_hit["object"]["layer"], "layer.foreground");
    assert_eq!(top_hit["region"]["kind"], "object_proxy");
    assert_eq!(top_hit["region"]["proxy_id"], "proxy.pulse_sprite.hotspot");
    assert_eq!(top_hit["region"]["proxy_type"], "PulseSpriteHotspot");
    assert_eq!(top_hit["region"]["proxy_role"], "inspect");
    assert_eq!(top_hit["region"]["proxy_layer"], "layer.hit");
    assert_eq!(top_hit["region"]["depth"], 2600);
    assert_eq!(
        top_hit["region"]["proxy_params"]["param.channel"]["value"],
        "preview"
    );
    let image_ref = &top_hit["object"]["image_ref"];
    assert_eq!(image_ref["asset"], "asset.bg.pulse");
    assert_eq!(image_ref["frame_index"], 0);
    assert_eq!(image_ref["local_time_millis"], 50);
    assert_eq!(image_ref["fit"], "stretch");
    assert_eq!(image_ref["alignment"]["x_milli"], 500);
    assert_eq!(image_ref["proxies"][0]["id"], "proxy.pulse_sprite.hotspot");
}

#[test]
fn agent_hit_test_capture_time_updates_unpinned_animated_image_frame_metadata() {
    let source_path = workspace_root().join("samples/image-animation.arcw");
    let early = hit_test_image_animation_sample_at(&source_path, "image_animated_gif", "0", 10, 10);
    let late =
        hit_test_image_animation_sample_at(&source_path, "image_animated_gif", "0.15", 10, 10);

    assert_eq!(early["status"], "ok");
    assert_eq!(late["status"], "ok");
    assert_eq!(early["top_object_id"], "object.image.layer.background.0.0");
    assert_eq!(late["top_object_id"], "object.image.layer.background.0.0");
    let early_hit = &early["hits"][0];
    let late_hit = &late["hits"][0];
    assert_eq!(early_hit["region"]["kind"], "object");
    assert_eq!(late_hit["region"]["kind"], "object");
    assert_eq!(early_hit["object"]["entity"], "image.background.default");
    assert_eq!(late_hit["object"]["entity"], "image.background.default");

    let early_ref = &early_hit["object"]["image_ref"];
    let late_ref = &late_hit["object"]["image_ref"];
    assert_eq!(early_ref["asset"], "asset.bg.pulse");
    assert_eq!(late_ref["asset"], "asset.bg.pulse");
    assert_eq!(early_ref["frame_index"], 0);
    assert_eq!(early_ref["local_time_millis"], 0);
    assert_eq!(late_ref["frame_index"], 1);
    assert_eq!(late_ref["local_time_millis"], 150);
    assert_eq!(
        early_ref["object"], late_ref["object"],
        "capture_time must update animated frame metadata without changing object identity"
    );
}

#[test]
fn agent_observe_image_alignment_sample_uses_authored_alignment_geometry() {
    let source_path = workspace_root().join("samples/image-animation.arcw");
    let observe =
        observe_image_animation_sample_flow_at(&source_path, "image_alignment_object", "0");
    let image_object = observe["objects"]
        .as_array()
        .expect("image alignment sample reports objects")
        .iter()
        .find(|object| {
            object["content"]["kind"] == "image"
                && object["content"]["object"] == "image.sample.aligned_poster"
        })
        .expect("aligned static image object is observed");

    assert_eq!(image_object["bbox"]["x"], 454);
    assert_eq!(image_object["bbox"]["y"], 251);
    assert_eq!(image_object["bbox"]["width"], 2);
    assert_eq!(image_object["bbox"]["height"], 1);
    assert_eq!(image_object["content"]["fit"], "intrinsic");
    assert_eq!(image_object["content"]["alignment"]["x_milli"], 1_000);
    assert_eq!(image_object["content"]["alignment"]["y_milli"], 1_000);
    assert_eq!(
        image_object["content"]["asset"], "asset.bg.poster",
        "alignment sample should preserve static image asset metadata"
    );
}

#[test]
fn agent_observe_native_captures_clipped_animated_image_object() {
    let source_path = workspace_root().join("samples/image-animation.arcw");
    let observe =
        observe_image_animation_sample_flow_at(&source_path, "image_clipped_object", "0.15");
    let image_object = observe["objects"]
        .as_array()
        .expect("image clipped sample reports objects")
        .iter()
        .find(|object| {
            object["content"]["kind"] == "image"
                && object["content"]["object"] == "image.sample.clipped_pulse"
        })
        .expect("clipped animated image object is observed");
    let object_id = image_object["id"]
        .as_str()
        .expect("clipped animated image object id");
    assert_eq!(object_id, "object.image.layer.clipped.0.1");
    assert_eq!(image_object["bbox"]["x"], 1184);
    assert_eq!(image_object["bbox"]["y"], 48);
    assert_eq!(image_object["bbox"]["width"], 96);
    assert_eq!(image_object["bbox"]["height"], 96);
    assert_eq!(image_object["content"]["frame_index"], 1);
    assert_eq!(image_object["content"]["local_time_millis"], 150);
    assert_eq!(image_object["content"]["opacity_milli"], 500);

    let dir = temp_dir("agent-observe-clipped-animated-image-object");
    let color_path = dir.join("clipped-image-color.rgba");
    let (color, color_bytes) = capture_image_animation_sample_object_raw_at(
        &source_path,
        "image_clipped_object",
        "0.15",
        object_id,
        "color",
        &color_path,
    );
    assert_clipped_animated_image_capture_metadata(&color, object_id, "color", "framebuffer_crop");
    assert_eq!(color_bytes.len(), 96 * 96 * 4);
    assert_eq!(
        &color_bytes[..4],
        &[176, 131, 11, 127],
        "clipped object color capture should start with the pinned animated frame pixel"
    );
    assert!(
        color_bytes.chunks_exact(4).all(|pixel| pixel[3] == 127),
        "clipped half-opacity image color capture should preserve object alpha"
    );

    let object_id_path = dir.join("clipped-image-object-id.rgba");
    let (object_id_capture, object_id_bytes) = capture_image_animation_sample_object_raw_at(
        &source_path,
        "image_clipped_object",
        "0.15",
        object_id,
        "object-id",
        &object_id_path,
    );
    assert_clipped_animated_image_capture_metadata(
        &object_id_capture,
        object_id,
        "object_id",
        "object_id_attachment",
    );
    assert_eq!(object_id_bytes.len(), 96 * 96 * 4);
    assert_eq!(
        &object_id_bytes[..4],
        &[113, 59, 100, 127],
        "clipped object-id capture should use the deterministic image debug tint"
    );
    assert!(
        object_id_bytes.chunks_exact(4).all(|pixel| pixel[3] == 127),
        "clipped object-id capture should preserve the half-opacity image alpha"
    );

    fs::remove_dir_all(&dir).expect("remove clipped animated image capture dir");
}

fn observe_image_animation_sample_flow_at(
    path: &Path,
    flow: &str,
    capture_time: &str,
) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--flow")
        .arg(flow)
        .arg("--steps")
        .arg("2")
        .arg("--capture-time")
        .arg(capture_time)
        .arg("--json")
        .output()
        .expect("arcw agent observe runs image animation sample flow");
    assert!(
        output.status.success(),
        "image animation sample observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("image animation sample observe output is JSON")
}

fn read_image_animation_sample_resource_at(
    path: &Path,
    flow: &str,
    capture_time: &str,
    uri: &str,
) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--flow")
        .arg(flow)
        .arg("--steps")
        .arg("2")
        .arg("--capture-time")
        .arg(capture_time)
        .arg("--json")
        .arg("--read-uri")
        .arg(uri)
        .output()
        .expect("arcw agent observe reads image animation sample resource");
    assert!(
        output.status.success(),
        "image animation sample read-uri should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("image animation sample resource is JSON")
}

fn hit_test_image_animation_sample_at(
    path: &Path,
    flow: &str,
    capture_time: &str,
    x: u32,
    y: u32,
) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("hit-test")
        .arg(path)
        .arg("--flow")
        .arg(flow)
        .arg("--steps")
        .arg("2")
        .arg("--capture-time")
        .arg(capture_time)
        .arg("--x")
        .arg(x.to_string())
        .arg("--y")
        .arg(y.to_string())
        .arg("--json")
        .output()
        .expect("arcw agent hit-test runs image animation sample");
    assert!(
        output.status.success(),
        "image animation sample hit-test should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("image animation sample hit-test is JSON")
}

fn capture_image_animation_sample_object_raw_at(
    path: &Path,
    flow: &str,
    capture_time: &str,
    object_id: &str,
    capture_kind: &str,
    raw_path: &Path,
) -> (serde_json::Value, Vec<u8>) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--flow")
        .arg(flow)
        .arg("--steps")
        .arg("2")
        .arg("--capture-time")
        .arg(capture_time)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg(object_id)
        .arg("--out")
        .arg(raw_path)
        .output()
        .expect("arcw agent observe captures image animation object");
    assert!(
        output.status.success(),
        "image animation object {capture_kind} capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("image animation object capture is JSON");
    let bytes = fs::read(raw_path).expect("read image animation object raw capture");
    (json, bytes)
}

fn assert_clipped_animated_image_capture_metadata(
    capture: &serde_json::Value,
    object_id: &str,
    kind: &str,
    composition: &str,
) {
    assert_eq!(capture["images"][0]["kind"], kind);
    assert_eq!(capture["images"][0]["renderer"], "native");
    assert_eq!(capture["images"][0]["scope"]["kind"], "object");
    assert_eq!(capture["images"][0]["scope"]["id"], object_id);
    assert_eq!(capture["images"][0]["composition"], composition);
    assert_eq!(capture["images"][0]["capture_time_millis"], 150);
    assert_eq!(capture["images"][0]["width"], 96);
    assert_eq!(capture["images"][0]["height"], 96);
    assert_eq!(capture["images"][0]["crop_origin"]["x"], 1184);
    assert_eq!(capture["images"][0]["crop_origin"]["y"], 48);
    assert_eq!(capture["images"][0]["content_bbox"]["x"], 0);
    assert_eq!(capture["images"][0]["content_bbox"]["y"], 0);
    assert_eq!(capture["images"][0]["content_bbox"]["width"], 96);
    assert_eq!(capture["images"][0]["content_bbox"]["height"], 96);
    assert_eq!(capture["images"][0]["content_viewport_bbox"]["x"], 1184);
    assert_eq!(capture["images"][0]["content_viewport_bbox"]["y"], 48);
    assert_eq!(capture["images"][0]["content_pixels"], 96 * 96);
    assert_eq!(
        capture["images"][0]["object"]["image_ref"]["object"],
        "image.sample.clipped_pulse"
    );
    assert_eq!(
        capture["images"][0]["object"]["image_ref"]["frame_index"],
        1
    );
}

#[test]
fn agent_observe_read_uri_returns_latest_native_layer_image() {
    let path = temp_arcw(
        "agent-observe-native-layer-read-uri",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/0/layer.dialogue.rich_text.png")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads latest native layer image");

    fs::remove_file(&path).expect("remove temp native layer read-uri source");
    assert!(
        output.status.success(),
        "native layer read-uri should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native layer read-uri resource is JSON");
    assert_eq!(
        json["uri"],
        "arcweft://session/cli/frame/0/layer.dialogue.rich_text.png"
    );
    assert_eq!(json["image"]["kind"], "color");
    assert_eq!(json["image"]["renderer"], "native");
    assert_eq!(json["image"]["scope"]["kind"], "layer");
    assert_eq!(json["image"]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["image"]["composition"], "isolated_regions");
    assert!(json["image"]["width"].as_u64().unwrap() < 1088);
    assert!(json["image"]["height"].as_u64().unwrap() < 124);
    assert_eq!(json["image"]["crop_origin"]["space"], "viewport");
    assert_eq!(json["body"]["body_kind"], "bytes_base64");
    assert_eq!(json["body"]["body"]["encoding"], "base64");
    assert!(
        json["body"]["body"]["data"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );
}

#[test]
fn agent_observe_read_uri_uses_native_renderer_without_selected_image() {
    let path = temp_arcw(
        "agent-observe-native-read-uri-renderer",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/0/layer.dialogue.rich_text.png")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads native layer image by URI");

    fs::remove_file(&path).expect("remove temp native read-uri renderer source");
    assert!(
        output.status.success(),
        "native read-uri renderer should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native read-uri renderer resource is JSON");
    assert_eq!(
        json["uri"],
        "arcweft://session/cli/frame/0/layer.dialogue.rich_text.png"
    );
    assert_eq!(json["image"]["renderer"], "native");
    assert_eq!(json["image"]["scope"]["kind"], "layer");
    assert_eq!(json["image"]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["image"]["composition"], "isolated_regions");
    assert!(json["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        json["body"]["body"]["data"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );
}

#[test]
fn agent_observe_native_renderer_writes_ruby_mask_raw_crop() {
    let path = temp_arcw(
        "agent-observe-native-ruby-mask",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let dir = temp_dir("agent-observe-native-ruby-mask");
    let raw_path = dir.join("native-ruby-mask.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0.ruby.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native ruby mask raw crop");

    assert!(
        output.status.success(),
        "native ruby mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native ruby mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");
    assert!(json["images"][0]["width"].as_u64().unwrap() < 180);
    assert!(json["images"][0]["height"].as_u64().unwrap() < 120);
    assert_eq!(json["images"][0]["crop_origin"]["space"], "viewport");
    let objects = json["objects"].as_array().expect("objects are listed");
    let ruby_object = objects
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.ruby.0")
        .expect("ruby object is observed");
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        ruby_object["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        ruby_object["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], ruby_object["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby_object["bbox"]["height"]);
    assert!(
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap() >= 96,
        "native ruby crop origin should be in textbox viewport bounds"
    );
    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    let content_bbox = &json["images"][0]["content_bbox"];
    let content_viewport_bbox = &json["images"][0]["content_viewport_bbox"];
    assert_eq!(
        content_viewport_bbox["x"].as_u64().unwrap(),
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap()
            + content_bbox["x"].as_u64().unwrap()
    );
    assert_eq!(
        content_viewport_bbox["y"].as_u64().unwrap(),
        json["images"][0]["crop_origin"]["y"].as_u64().unwrap()
            + content_bbox["y"].as_u64().unwrap()
    );
    assert!(
        content_viewport_bbox["x"].as_u64().unwrap() >= ruby_object["bbox"]["x"].as_u64().unwrap()
    );
    assert!(
        content_viewport_bbox["y"].as_u64().unwrap() >= ruby_object["bbox"]["y"].as_u64().unwrap()
    );
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native ruby mask raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    assert!(opaque > 0);
    assert!(transparent > 0);

    fs::remove_file(&path).expect("remove temp native ruby mask source");
    fs::remove_dir_all(&dir).expect("remove temp native ruby mask dir");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_ruby_mask_raw_crop() {
    assert_native_vertical_lr_ruby_raw_crop("mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_ruby_object_id_raw_crop() {
    assert_native_vertical_lr_ruby_raw_crop("object-id");
}

fn assert_native_vertical_lr_ruby_raw_crop(capture_kind: &str) {
    let fixture_name = format!("agent-observe-native-vertical-lr-ruby-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]天地|[夢](ゆめ)星[/][p]
}
",
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!("native-vertical-lr-ruby-{capture_kind}.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.ruby.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native vertical_lr ruby raw crop");

    assert!(
        output.status.success(),
        "native vertical_lr ruby {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native vertical_lr ruby report is JSON");
    match capture_kind {
        "mask" => {
            assert_eq!(json["images"][0]["kind"], "mask");
            assert_eq!(json["images"][0]["composition"], "mask_attachment");
        }
        "object-id" => {
            assert_eq!(json["images"][0]["kind"], "object_id");
            assert_eq!(json["images"][0]["composition"], "object_id_attachment");
        }
        other => panic!("unsupported vertical_lr ruby capture kind: {other}"),
    }
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");

    let ruby = assert_native_vertical_lr_ruby_geometry(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], ruby["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], ruby["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], ruby["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(ruby),
            content_pixels,
            "vertical_lr ruby object-id crop",
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native vertical_lr ruby mask raw crop");
        let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native vertical_lr ruby source");
    fs::remove_dir_all(&dir).expect("remove temp native vertical_lr ruby dir");
}

fn assert_native_vertical_lr_ruby_geometry(json: &serde_json::Value) -> &serde_json::Value {
    let ruby = find_rich_text_ruby_object(json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ゆめ");
    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    assert!(
        agent_json_bbox_center_x_twice(annotation) < agent_json_bbox_center_x_twice(base),
        "vertical_lr ruby annotation should render on the left side of the base: {ruby}"
    );
    assert!(
        agent_json_bbox_x(&json["images"][0]["content_viewport_bbox"])
            >= agent_json_bbox_x(annotation)
            && agent_json_bbox_right(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_right(base),
        "vertical_lr ruby mask content should stay within the ruby base/annotation union: {ruby}"
    );
    ruby
}

#[test]
fn agent_observe_native_renderer_writes_long_vertical_ruby_mask_raw_crop() {
    assert_native_long_vertical_ruby_mask_raw_crop("vertical_rl", true);
    assert_native_long_vertical_ruby_mask_raw_crop("vertical_lr", false);
}

#[test]
fn agent_observe_native_renderer_writes_long_vertical_ruby_object_id_raw_crop() {
    assert_native_long_vertical_ruby_object_id_raw_crop("vertical_rl", true);
    assert_native_long_vertical_ruby_object_id_raw_crop("vertical_lr", false);
}

#[test]
fn agent_observe_native_renderer_writes_overheight_vertical_ruby_raw_crops() {
    assert_native_overheight_vertical_ruby_raw_crop("vertical_rl", true, "mask");
    assert_native_overheight_vertical_ruby_raw_crop("vertical_lr", false, "mask");
    assert_native_overheight_vertical_ruby_raw_crop("vertical_rl", true, "object-id");
    assert_native_overheight_vertical_ruby_raw_crop("vertical_lr", false, "object-id");
}

fn assert_native_long_vertical_ruby_mask_raw_crop(writing_mode: &str, ruby_on_right: bool) {
    let path = temp_arcw(
        &format!("agent-observe-native-long-{writing_mode}-ruby-mask"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬|[夢](ながいながいよみ)人外[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-long-{writing_mode}-ruby-mask"
    ));
    let raw_path = dir.join(format!("native-long-{writing_mode}-ruby-mask.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0.ruby.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native long vertical ruby mask raw crop");

    assert!(
        output.status.success(),
        "native long {writing_mode} ruby mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native long vertical ruby mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");

    let ruby = find_rich_text_ruby_object(&json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ながいながいよみ");
    assert_eq!(json["images"][0]["crop_origin"]["x"], ruby["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], ruby["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], ruby["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby["bbox"]["height"]);

    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    let base_cluster = find_rich_text_cluster_object(&json, "夢", 18, 21);
    assert!(
        agent_json_bbox_height(base) > agent_json_bbox_height(&base_cluster["bbox"]) * 2,
        "long {writing_mode} ruby mask should observe expanded base geometry: {ruby}"
    );
    if ruby_on_right {
        assert!(
            agent_json_bbox_center_x_twice(annotation) > agent_json_bbox_center_x_twice(base),
            "vertical_rl long ruby annotation should be on the right side of the base: {ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_center_x_twice(annotation) < agent_json_bbox_center_x_twice(base),
            "vertical_lr long ruby annotation should be on the left side of the base: {ruby}"
        );
    }
    assert!(
        agent_json_bbox_x(&json["images"][0]["content_viewport_bbox"])
            >= agent_json_bbox_x(&ruby["bbox"])
            && agent_json_bbox_right(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_right(&ruby["bbox"])
            && agent_json_bbox_y(&json["images"][0]["content_viewport_bbox"])
                >= agent_json_bbox_y(&ruby["bbox"])
            && agent_json_bbox_bottom(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_bottom(&ruby["bbox"]),
        "long {writing_mode} ruby mask content should stay inside the expanded ruby object bbox: {ruby}"
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native long vertical ruby mask raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(transparent > 0);

    fs::remove_file(&path).expect("remove temp native long vertical ruby mask source");
    fs::remove_dir_all(&dir).expect("remove temp native long vertical ruby mask dir");
}

fn assert_native_long_vertical_ruby_object_id_raw_crop(writing_mode: &str, ruby_on_right: bool) {
    let path = temp_arcw(
        &format!("agent-observe-native-long-{writing_mode}-ruby-object-id"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬|[夢](ながいながいよみ)人外[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-long-{writing_mode}-ruby-object-id"
    ));
    let raw_path = dir.join(format!("native-long-{writing_mode}-ruby-object-id.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("object-id")
        .arg("--object")
        .arg("object.dialogue.0.0.ruby.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native long vertical ruby object-id raw crop");

    assert!(
        output.status.success(),
        "native long {writing_mode} ruby object-id crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native long vertical ruby object-id report is JSON");
    assert_eq!(json["images"][0]["kind"], "object_id");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "object_id_attachment");

    let ruby = find_rich_text_ruby_object(&json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ながいながいよみ");
    assert_eq!(json["images"][0]["crop_origin"]["x"], ruby["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], ruby["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], ruby["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby["bbox"]["height"]);

    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    let base_cluster = find_rich_text_cluster_object(&json, "夢", 18, 21);
    assert!(
        agent_json_bbox_height(base) > agent_json_bbox_height(&base_cluster["bbox"]) * 2,
        "long {writing_mode} ruby object-id should observe expanded base geometry: {ruby}"
    );
    if ruby_on_right {
        assert!(
            agent_json_bbox_center_x_twice(annotation) > agent_json_bbox_center_x_twice(base),
            "vertical_rl long ruby annotation should be on the right side of the base: {ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_center_x_twice(annotation) < agent_json_bbox_center_x_twice(base),
            "vertical_lr long ruby annotation should be on the left side of the base: {ruby}"
        );
    }
    assert!(
        agent_json_bbox_x(&json["images"][0]["content_viewport_bbox"])
            >= agent_json_bbox_x(&ruby["bbox"])
            && agent_json_bbox_right(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_right(&ruby["bbox"])
            && agent_json_bbox_y(&json["images"][0]["content_viewport_bbox"])
                >= agent_json_bbox_y(&ruby["bbox"])
            && agent_json_bbox_bottom(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_bottom(&ruby["bbox"]),
        "long {writing_mode} ruby object-id content should stay inside the expanded ruby object bbox: {ruby}"
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);

    let expected = agent_object_id_color_from_json(ruby);
    assert_raw_object_id_tint(
        &raw_path,
        expected,
        content_pixels,
        &format!("{writing_mode} long ruby object-id crop"),
    );

    fs::remove_file(&path).expect("remove temp native long vertical ruby object-id source");
    fs::remove_dir_all(&dir).expect("remove temp native long vertical ruby object-id dir");
}

fn assert_native_overheight_vertical_ruby_raw_crop(
    writing_mode: &str,
    ruby_on_right: bool,
    capture_kind: &str,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-overheight-{writing_mode}-ruby-{capture_kind}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地|[夢](あいうえおかきくけこさしすせそたちつてと)人外[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-overheight-{writing_mode}-ruby-{capture_kind}"
    ));
    let raw_path = dir.join(format!(
        "native-overheight-{writing_mode}-ruby-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.ruby.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native over-height vertical ruby raw crop");

    assert!(
        output.status.success(),
        "native over-height {writing_mode} ruby {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native over-height vertical ruby report is JSON");
    let expected_kind = capture_kind.replace('-', "_");
    assert_eq!(json["images"][0]["kind"], expected_kind);
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let ruby = assert_native_overheight_vertical_ruby_geometry(&json, writing_mode, ruby_on_right);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(ruby),
            content_pixels,
            &format!("{writing_mode} over-height ruby object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native over-height vertical ruby mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native over-height vertical ruby source");
    fs::remove_dir_all(&dir).expect("remove temp native over-height vertical ruby dir");
}

fn assert_native_overheight_vertical_ruby_geometry<'a>(
    json: &'a serde_json::Value,
    writing_mode: &str,
    ruby_on_right: bool,
) -> &'a serde_json::Value {
    let ruby = find_rich_text_ruby_object(json, 0);
    assert_eq!(
        ruby["rich_text_ref"]["ruby"],
        "あいうえおかきくけこさしすせそたちつてと"
    );
    assert_eq!(json["images"][0]["crop_origin"]["x"], ruby["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], ruby["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], ruby["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby["bbox"]["height"]);

    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    assert!(
        agent_json_bbox_width(annotation) > 24,
        "over-height {writing_mode} ruby annotation should union split tracks: {ruby}"
    );
    if ruby_on_right {
        assert!(
            agent_json_bbox_center_x_twice(annotation) > agent_json_bbox_center_x_twice(base),
            "vertical_rl over-height ruby annotation should stay on the right side of the base: {ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_center_x_twice(annotation) < agent_json_bbox_center_x_twice(base),
            "vertical_lr over-height ruby annotation should stay on the left side of the base: {ruby}"
        );
    }
    assert!(
        agent_json_bbox_x(&ruby["bbox"]) <= agent_json_bbox_x(base)
            && agent_json_bbox_x(&ruby["bbox"]) <= agent_json_bbox_x(annotation)
            && agent_json_bbox_right(&ruby["bbox"]) >= agent_json_bbox_right(base)
            && agent_json_bbox_right(&ruby["bbox"]) >= agent_json_bbox_right(annotation)
            && agent_json_bbox_y(&ruby["bbox"]) <= agent_json_bbox_y(base)
            && agent_json_bbox_y(&ruby["bbox"]) <= agent_json_bbox_y(annotation)
            && agent_json_bbox_bottom(&ruby["bbox"]) >= agent_json_bbox_bottom(base)
            && agent_json_bbox_bottom(&ruby["bbox"]) >= agent_json_bbox_bottom(annotation),
        "over-height {writing_mode} ruby object bbox should union base and split annotation geometry: {ruby}"
    );
    assert!(
        agent_json_bbox_x(&json["images"][0]["content_viewport_bbox"])
            >= agent_json_bbox_x(&ruby["bbox"])
            && agent_json_bbox_right(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_right(&ruby["bbox"])
            && agent_json_bbox_y(&json["images"][0]["content_viewport_bbox"])
                >= agent_json_bbox_y(&ruby["bbox"])
            && agent_json_bbox_bottom(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_bottom(&ruby["bbox"]),
        "over-height {writing_mode} ruby content should stay inside the authored ruby object bbox: {ruby}"
    );
    ruby
}

fn agent_object_id_color_from_json(object: &serde_json::Value) -> [u8; 4] {
    let color = &object["capture_refs"]["object_id_color"];
    [
        u8::try_from(color["red"].as_u64().expect("object-id red"))
            .expect("object-id red fits in u8"),
        u8::try_from(color["green"].as_u64().expect("object-id green"))
            .expect("object-id green fits in u8"),
        u8::try_from(color["blue"].as_u64().expect("object-id blue"))
            .expect("object-id blue fits in u8"),
        u8::try_from(color["alpha"].as_u64().expect("object-id alpha"))
            .expect("object-id alpha fits in u8"),
    ]
}

fn assert_raw_object_id_tint(
    raw_path: &Path,
    expected: [u8; 4],
    content_pixels: u64,
    context: &str,
) {
    let bytes = fs::read(raw_path).expect("read native long vertical ruby object-id raw crop");
    assert_raw_object_id_tint_bytes(&bytes, expected, content_pixels, context);
}

fn assert_raw_object_id_tint_bytes(
    bytes: &[u8],
    expected: [u8; 4],
    content_pixels: u64,
    context: &str,
) {
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let tinted_color = bytes
        .chunks_exact(4)
        .filter(|pixel| {
            pixel[3] >= 128
                && pixel[0].abs_diff(expected[0]) <= 24
                && pixel[1].abs_diff(expected[1]) <= 24
                && pixel[2].abs_diff(expected[2]) <= 24
        })
        .count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(
        tinted_color > 0,
        "{context} should contain the observed object color tint"
    );
}

#[test]
fn agent_observe_native_renderer_writes_text_combine_mask_raw_crop() {
    let path = temp_arcw(
        "agent-observe-native-text-combine-mask",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]A 2026 B[/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-text-combine-mask");
    let raw_path = dir.join("native-text-combine-mask.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.2.2.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native text-combine mask raw crop");

    assert!(
        output.status.success(),
        "native text-combine mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native text-combine mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");

    let text_combine = find_rich_text_cluster_object(&json, "2026", 2, 6);
    assert_eq!(
        text_combine["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        text_combine["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        text_combine["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], text_combine["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], text_combine["bbox"]["height"]);
    assert_eq!(
        json["images"][0]["content_viewport_bbox"]["x"]
            .as_u64()
            .unwrap(),
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap()
            + json["images"][0]["content_bbox"]["x"].as_u64().unwrap()
    );
    assert_eq!(
        json["images"][0]["content_viewport_bbox"]["y"]
            .as_u64()
            .unwrap(),
        json["images"][0]["crop_origin"]["y"].as_u64().unwrap()
            + json["images"][0]["content_bbox"]["y"].as_u64().unwrap()
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native text-combine mask raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(transparent > 0);

    fs::remove_file(&path).expect("remove temp native text-combine mask source");
    fs::remove_dir_all(&dir).expect("remove temp native text-combine mask dir");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_text_combine_mask_raw_crop() {
    let path = temp_arcw(
        "agent-observe-native-vertical-lr-text-combine-mask",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]A 2026 B[/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-vertical-lr-text-combine-mask");
    let raw_path = dir.join("native-vertical-lr-text-combine-mask.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.2.2.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native vertical_lr text-combine mask raw crop");

    assert!(
        output.status.success(),
        "native vertical_lr text-combine mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native vertical_lr text-combine mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");

    let text_combine = find_rich_text_cluster_object(&json, "2026", 2, 6);
    assert_eq!(
        text_combine["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        text_combine["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        text_combine["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], text_combine["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], text_combine["bbox"]["height"]);
    assert_eq!(
        json["images"][0]["content_viewport_bbox"]["x"]
            .as_u64()
            .unwrap(),
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap()
            + json["images"][0]["content_bbox"]["x"].as_u64().unwrap()
    );
    assert_eq!(
        json["images"][0]["content_viewport_bbox"]["y"]
            .as_u64()
            .unwrap(),
        json["images"][0]["crop_origin"]["y"].as_u64().unwrap()
            + json["images"][0]["content_bbox"]["y"].as_u64().unwrap()
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native vertical_lr text-combine mask raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(transparent > 0);

    fs::remove_file(&path).expect("remove temp native vertical_lr text-combine mask source");
    fs::remove_dir_all(&dir).expect("remove temp native vertical_lr text-combine mask dir");
}

#[test]
fn agent_observe_native_renderer_writes_text_combine_object_id_raw_crop() {
    assert_native_text_combine_object_id_raw_crop("vertical_rl", "text-combine-object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_text_combine_object_id_raw_crop() {
    assert_native_text_combine_object_id_raw_crop(
        "vertical_lr",
        "vertical-lr-text-combine-object-id",
    );
}

fn assert_native_text_combine_object_id_raw_crop(writing_mode: &str, label: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-{label}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]A 2026 B[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&format!("agent-observe-native-{label}"));
    let raw_path = dir.join(format!("native-{label}.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("object-id")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.2.2.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native text-combine object-id raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} text-combine object-id crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native text-combine object-id report is JSON");
    assert_eq!(json["images"][0]["kind"], "object_id");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "object_id_attachment");

    let text_combine = find_rich_text_cluster_object(&json, "2026", 2, 6);
    assert_eq!(
        text_combine["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        text_combine["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        text_combine["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], text_combine["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], text_combine["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);

    let color = &text_combine["capture_refs"]["object_id_color"];
    let expected = [
        u8::try_from(color["red"].as_u64().expect("object-id red"))
            .expect("object-id red fits in u8"),
        u8::try_from(color["green"].as_u64().expect("object-id green"))
            .expect("object-id green fits in u8"),
        u8::try_from(color["blue"].as_u64().expect("object-id blue"))
            .expect("object-id blue fits in u8"),
        u8::try_from(color["alpha"].as_u64().expect("object-id alpha"))
            .expect("object-id alpha fits in u8"),
    ];
    let bytes = fs::read(&raw_path).expect("read native text-combine object-id raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let exact_color = bytes
        .chunks_exact(4)
        .filter(|pixel| *pixel == expected)
        .count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(
        exact_color > 0,
        "{writing_mode} text-combine object-id crop should contain the observed object color"
    );

    fs::remove_file(&path).expect("remove temp native text-combine object-id source");
    fs::remove_dir_all(&dir).expect("remove temp native text-combine object-id dir");
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_compressed_punctuation_mask_raw_crop() {
    assert_native_jlreq_compressed_punctuation_raw_crop("vertical_rl", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_compressed_punctuation_object_id_raw_crop() {
    assert_native_jlreq_compressed_punctuation_raw_crop("vertical_rl", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_compressed_punctuation_mask_raw_crop() {
    assert_native_jlreq_compressed_punctuation_raw_crop("vertical_lr", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_compressed_punctuation_object_id_crop() {
    assert_native_jlreq_compressed_punctuation_raw_crop("vertical_lr", "object-id");
}

fn assert_native_jlreq_compressed_punctuation_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name =
        format!("agent-observe-native-{writing_mode}-jlreq-compressed-punctuation-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天、。・人[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-jlreq-compressed-punctuation-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.1.3.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ punctuation raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} JLREQ punctuation {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native JLREQ punctuation crop report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let comma = assert_native_jlreq_compressed_punctuation_chain(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], comma["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], comma["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], comma["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], comma["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(comma),
            content_pixels,
            &format!("{writing_mode} JLREQ compressed punctuation object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ punctuation mask raw crop");
        let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native JLREQ punctuation source");
    fs::remove_dir_all(&dir).expect("remove temp native JLREQ punctuation dir");
}

fn assert_native_jlreq_compressed_punctuation_chain(
    json: &serde_json::Value,
) -> &serde_json::Value {
    let comma = find_rich_text_cluster_object(json, "、", 3, 6);
    let period = find_rich_text_cluster_object(json, "。", 6, 9);
    let middle_dot = find_rich_text_cluster_object(json, "・", 9, 12);
    let person = find_rich_text_cluster_object(json, "人", 12, 15);
    assert_eq!(comma["rich_text_ref"]["orientation"], "upright");
    assert_eq!(comma["rich_text_ref"]["vertical_form"], "upright_alternate");
    assert_eq!(middle_dot["rich_text_ref"]["orientation"], "upright");
    assert_eq!(middle_dot["rich_text_ref"]["vertical_form"], "none");
    assert_eq!(
        agent_json_bbox_y(&period["bbox"]) - agent_json_bbox_y(&comma["bbox"]),
        21,
        "compressed comma should advance by half a body cell"
    );
    assert_eq!(
        agent_json_bbox_y(&middle_dot["bbox"]) - agent_json_bbox_y(&period["bbox"]),
        21,
        "middle dot should continue the compressed punctuation chain"
    );
    assert_eq!(
        agent_json_bbox_y(&person["bbox"]) - agent_json_bbox_y(&middle_dot["bbox"]),
        21,
        "following text should consume the space left by punctuation compression"
    );
    comma
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_opening_punctuation_mask_raw_crop() {
    assert_native_jlreq_opening_punctuation_raw_crop("vertical_rl", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_opening_punctuation_object_id_raw_crop() {
    assert_native_jlreq_opening_punctuation_raw_crop("vertical_rl", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_opening_punctuation_mask_raw_crop() {
    assert_native_jlreq_opening_punctuation_raw_crop("vertical_lr", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_opening_punctuation_object_id_raw_crop() {
    assert_native_jlreq_opening_punctuation_raw_crop("vertical_lr", "object-id");
}

fn assert_native_jlreq_opening_punctuation_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name =
        format!("agent-observe-native-{writing_mode}-jlreq-opening-punctuation-{capture_kind}");
    let source = format!(
        r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春「人外[/][p]
}}
"
    );
    let path = temp_arcw(&fixture_name, &source);
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-jlreq-opening-punctuation-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.3.9.12")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ opening punctuation raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} JLREQ opening {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native JLREQ opening punctuation report is JSON");
    match capture_kind {
        "mask" => {
            assert_eq!(json["images"][0]["kind"], "mask");
            assert_eq!(json["images"][0]["composition"], "mask_attachment");
        }
        "object-id" => {
            assert_eq!(json["images"][0]["kind"], "object_id");
            assert_eq!(json["images"][0]["composition"], "object_id_attachment");
        }
        other => panic!("unsupported JLREQ opening punctuation capture kind: {other}"),
    }
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");

    let opening_bracket = assert_native_jlreq_opening_punctuation_geometry(&json, writing_mode);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        opening_bracket["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        opening_bracket["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], opening_bracket["bbox"]["width"]);
    assert_eq!(
        json["images"][0]["height"],
        opening_bracket["bbox"]["height"]
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(opening_bracket),
            content_pixels,
            &format!("{writing_mode} JLREQ opening punctuation object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ opening punctuation mask crop");
        let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native JLREQ opening punctuation source");
    fs::remove_dir_all(&dir).expect("remove temp native JLREQ opening punctuation dir");
}

fn assert_native_jlreq_opening_punctuation_geometry<'a>(
    json: &'a serde_json::Value,
    writing_mode: &str,
) -> &'a serde_json::Value {
    let spring = find_rich_text_cluster_object(json, "春", 6, 9);
    let opening_bracket = find_rich_text_cluster_object(json, "「", 9, 12);
    let person = find_rich_text_cluster_object(json, "人", 12, 15);
    assert_eq!(
        opening_bracket["rich_text_ref"]["orientation"],
        "sideways_cw"
    );
    assert_eq!(
        opening_bracket["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    match writing_mode {
        "vertical_rl" => assert!(
            agent_json_bbox_x(&opening_bracket["bbox"]) < agent_json_bbox_x(&spring["bbox"]),
            "line-end-prohibited opening punctuation should move to the next vertical_rl column"
        ),
        "vertical_lr" => assert!(
            agent_json_bbox_x(&opening_bracket["bbox"]) > agent_json_bbox_x(&spring["bbox"]),
            "line-end-prohibited opening punctuation should move to the next vertical_lr column"
        ),
        other => panic!("unsupported writing mode for JLREQ opening punctuation crop: {other}"),
    }
    assert!(
        agent_json_bbox_y(&opening_bracket["bbox"]) < agent_json_bbox_y(&spring["bbox"]),
        "opening punctuation moved from a column end should restart near the column top"
    );
    assert_vertical_cluster_after(
        opening_bracket,
        person,
        "text after opening punctuation should continue in the same moved column",
    );
    opening_bracket
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_hanging_punctuation_mask_raw_crop() {
    assert_native_vertical_lr_jlreq_hanging_punctuation_raw_crop("mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_hanging_punctuation_object_id_raw_crop() {
    assert_native_vertical_lr_jlreq_hanging_punctuation_raw_crop("object-id");
}

fn assert_native_vertical_lr_jlreq_hanging_punctuation_raw_crop(capture_kind: &str) {
    let fixture_name = format!("agent-observe-native-vertical-lr-jlreq-hanging-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]天地、人人[/][p]
}
",
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-vertical-lr-jlreq-hanging-punctuation-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.2.6.9")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native vertical_lr JLREQ hanging raw crop");

    assert!(
        output.status.success(),
        "native vertical_lr JLREQ hanging {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native vertical_lr JLREQ hanging report is JSON");
    match capture_kind {
        "mask" => {
            assert_eq!(json["images"][0]["kind"], "mask");
            assert_eq!(json["images"][0]["composition"], "mask_attachment");
        }
        "object-id" => {
            assert_eq!(json["images"][0]["kind"], "object_id");
            assert_eq!(json["images"][0]["composition"], "object_id_attachment");
        }
        other => panic!("unsupported vertical_lr JLREQ hanging capture kind: {other}"),
    }
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");

    let comma = assert_native_vertical_lr_jlreq_hanging_punctuation_geometry(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], comma["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], comma["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], comma["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], comma["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(comma),
            content_pixels,
            "vertical_lr JLREQ hanging punctuation object-id crop",
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native vertical_lr JLREQ hanging mask raw crop");
        let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native vertical_lr JLREQ hanging source");
    fs::remove_dir_all(&dir).expect("remove temp native vertical_lr JLREQ hanging dir");
}

fn assert_native_vertical_lr_jlreq_hanging_punctuation_geometry(
    json: &serde_json::Value,
) -> &serde_json::Value {
    let earth = find_rich_text_cluster_object(json, "地", 3, 6);
    let comma = find_rich_text_cluster_object(json, "、", 6, 9);
    let next_person = find_rich_text_cluster_object(json, "人", 9, 12);
    assert_eq!(comma["rich_text_ref"]["orientation"], "upright");
    assert_eq!(comma["rich_text_ref"]["vertical_form"], "upright_alternate");
    assert_eq!(
        earth["bbox"]["x"], comma["bbox"]["x"],
        "vertical_lr hanging punctuation should remain in the previous column"
    );
    assert!(
        agent_json_bbox_y(&comma["bbox"]) > agent_json_bbox_y(&earth["bbox"]),
        "vertical_lr hanging punctuation should sit after the previous cluster"
    );
    assert!(
        agent_json_bbox_x(&next_person["bbox"]) > agent_json_bbox_x(&comma["bbox"])
            && agent_json_bbox_y(&next_person["bbox"]) < agent_json_bbox_y(&comma["bbox"]),
        "text after vertical_lr hanging punctuation should start the next column"
    );
    comma
}

fn observe_native_typewriter_cluster_mask_at(
    source_path: &Path,
    raw_path: &Path,
    object_id: &str,
    capture_time: &str,
) -> (serde_json::Value, Vec<u8>) {
    observe_native_typewriter_cluster_raw_at(source_path, raw_path, object_id, capture_time, "mask")
}

fn observe_native_typewriter_cluster_raw_at(
    source_path: &Path,
    raw_path: &Path,
    object_id: &str,
    capture_time: &str,
    capture_kind: &str,
) -> (serde_json::Value, Vec<u8>) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg(object_id)
        .arg("--capture-time")
        .arg(capture_time)
        .arg("--out")
        .arg(raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native typewriter raw crop");

    assert!(
        output.status.success(),
        "native typewriter {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native typewriter raw report is JSON");
    match capture_kind {
        "mask" => {
            assert_eq!(json["images"][0]["kind"], "mask");
            assert_eq!(json["images"][0]["composition"], "mask_attachment");
        }
        "object-id" => {
            assert_eq!(json["images"][0]["kind"], "object_id");
            assert_eq!(json["images"][0]["composition"], "object_id_attachment");
        }
        other => panic!("unsupported native typewriter capture kind: {other}"),
    }
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    let bytes = fs::read(raw_path).expect("read native typewriter raw crop");
    (json, bytes)
}

fn opaque_pixel_count(bytes: &[u8]) -> usize {
    bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count()
}

#[test]
fn agent_observe_native_renderer_writes_vertical_goal_clear_smoke_raw_crops() {
    let source_path = vertical_goal_clear_smoke_fixture_path();
    let dir = temp_dir("agent-observe-native-vertical-goal-clear-smoke-raw-crops");
    let object_id = "object.dialogue.0.0.cluster.17.38.42";

    let hidden_mask_path = dir.join("vertical-goal-clear-hidden-mask.rgba");
    let visible_mask_path = dir.join("vertical-goal-clear-visible-mask.rgba");
    let (hidden_mask, hidden_mask_bytes) = observe_native_goal_clear_object_raw_at(
        &source_path,
        &hidden_mask_path,
        object_id,
        "mask",
        "0",
    );
    let (visible_mask, visible_mask_bytes) = observe_native_goal_clear_object_raw_at(
        &source_path,
        &visible_mask_path,
        object_id,
        "mask",
        "60",
    );
    let hidden_cluster = find_rich_text_cluster_object(&hidden_mask, "2026", 38, 42);
    let visible_cluster = find_rich_text_cluster_object(&visible_mask, "2026", 38, 42);
    assert_eq!(
        hidden_cluster["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(hidden_cluster["bbox"], visible_cluster["bbox"]);
    assert_eq!(
        hidden_mask["images"][0]["crop_origin"],
        visible_mask["images"][0]["crop_origin"]
    );
    assert_eq!(hidden_mask["images"][0]["content_pixels"], 0);
    assert!(
        visible_mask["images"][0]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(opaque_pixel_count(&hidden_mask_bytes), 0);
    assert_eq!(
        opaque_pixel_count(&visible_mask_bytes) as u64,
        visible_mask["images"][0]["content_pixels"]
            .as_u64()
            .unwrap()
    );

    for capture_kind in ["color", "object-id"] {
        let raw_path = dir.join(format!("vertical-goal-clear-visible-{capture_kind}.rgba"));
        let (json, bytes) = observe_native_goal_clear_object_raw_at(
            &source_path,
            &raw_path,
            object_id,
            capture_kind,
            "60",
        );
        let cluster = find_rich_text_cluster_object(&json, "2026", 38, 42);
        assert_eq!(json["images"][0]["width"], cluster["bbox"]["width"]);
        assert_eq!(json["images"][0]["height"], cluster["bbox"]["height"]);
        assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
        assert_eq!(
            opaque_pixel_count(&bytes) as u64,
            json["images"][0]["content_pixels"].as_u64().unwrap()
        );
        if capture_kind == "object-id" {
            assert_raw_object_id_tint(
                &raw_path,
                agent_object_id_color_from_json(cluster),
                json["images"][0]["content_pixels"].as_u64().unwrap(),
                "vertical goal-clear text-combine object-id crop",
            );
        }
    }

    assert_vertical_goal_clear_ruby_raw_crops(&source_path, &dir);

    fs::remove_dir_all(&dir).expect("remove temp vertical goal-clear raw crop dir");
}

fn assert_vertical_goal_clear_ruby_raw_crops(source_path: &Path, dir: &Path) {
    for (ruby_index, object_id, description) in [
        (
            0,
            "object.dialogue.0.0.ruby.0",
            "vertical goal-clear vertical_rl ruby",
        ),
        (
            1,
            "object.dialogue.0.0.ruby.1",
            "vertical goal-clear vertical_lr ruby",
        ),
    ] {
        for capture_kind in ["mask", "object-id"] {
            let raw_path = dir.join(format!(
                "vertical-goal-clear-ruby-{ruby_index}-{capture_kind}.rgba"
            ));
            let (json, bytes) = observe_native_goal_clear_object_raw_at(
                source_path,
                &raw_path,
                object_id,
                capture_kind,
                "60",
            );
            assert_vertical_goal_clear_ruby_raw_crop(
                &json,
                &bytes,
                &raw_path,
                ruby_index,
                capture_kind,
                description,
            );
        }
    }
}

fn assert_vertical_goal_clear_ruby_raw_crop(
    json: &serde_json::Value,
    bytes: &[u8],
    raw_path: &Path,
    ruby_index: u64,
    capture_kind: &str,
    description: &str,
) {
    let ruby = find_rich_text_ruby_object(json, ruby_index);
    assert_eq!(json["images"][0]["width"], ruby["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby["bbox"]["height"]);
    assert_eq!(json["images"][0]["crop_origin"]["x"], ruby["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], ruby["bbox"]["y"]);

    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    if ruby_index == 0 {
        assert!(
            agent_json_bbox_center_x_twice(annotation) > agent_json_bbox_center_x_twice(base),
            "{description} annotation should stay on the right side of its base: {ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_center_x_twice(annotation) < agent_json_bbox_center_x_twice(base),
            "{description} annotation should stay on the left side of its base: {ruby}"
        );
    }

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    assert_eq!(opaque_pixel_count(bytes) as u64, content_pixels);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            raw_path,
            agent_object_id_color_from_json(ruby),
            content_pixels,
            &format!("{description} object-id crop"),
        );
    } else {
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert!(transparent > 0);
    }
}

fn observe_native_goal_clear_object_raw_at(
    source_path: &Path,
    raw_path: &Path,
    object_id: &str,
    capture_kind: &str,
    capture_time: &str,
) -> (serde_json::Value, Vec<u8>) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg(object_id)
        .arg("--capture-time")
        .arg(capture_time)
        .arg("--out")
        .arg(raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes vertical goal-clear raw crop");

    assert!(
        output.status.success(),
        "vertical goal-clear {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("vertical goal-clear raw report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    match capture_kind {
        "color" => assert_eq!(json["images"][0]["composition"], "isolated_regions"),
        "mask" => assert_eq!(json["images"][0]["composition"], "mask_attachment"),
        "object-id" => assert_eq!(json["images"][0]["composition"], "object_id_attachment"),
        other => panic!("unsupported vertical goal-clear capture kind: {other}"),
    }
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    let bytes = fs::read(raw_path).expect("read vertical goal-clear raw crop");
    (json, bytes)
}

#[test]
fn agent_observe_native_typewriter_capture_time_changes_visibility_without_relayout() {
    let path = temp_arcw(
        "agent-observe-native-typewriter-capture-time",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl][.typewriter cps=1]吾輩[/][/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-typewriter-capture-time");
    let hidden_path = dir.join("native-typewriter-hidden-mask.rgba");
    let visible_path = dir.join("native-typewriter-visible-mask.rgba");

    let (hidden, hidden_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &hidden_path,
        "object.dialogue.0.0.cluster.0.0.3",
        "0",
    );
    let (visible, visible_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &visible_path,
        "object.dialogue.0.0.cluster.0.0.3",
        "4",
    );
    let hidden_cluster = find_rich_text_cluster_object(&hidden, "吾", 0, 3);
    let visible_cluster = find_rich_text_cluster_object(&visible, "吾", 0, 3);
    assert_eq!(
        hidden_cluster["rich_text_ref"]["presentation"]["effects"][0]["id"],
        "typewriter"
    );
    assert_eq!(
        hidden_cluster["rich_text_ref"]["presentation"]["effects"][0]["phase"],
        "glyph_mask"
    );
    assert_eq!(hidden_cluster["bbox"], visible_cluster["bbox"]);
    assert_eq!(
        hidden["images"][0]["crop_origin"],
        visible["images"][0]["crop_origin"]
    );
    assert_eq!(hidden["images"][0]["width"], visible["images"][0]["width"]);
    assert_eq!(
        hidden["images"][0]["height"],
        visible["images"][0]["height"]
    );
    assert_eq!(hidden["images"][0]["content_pixels"], 0);
    assert!(visible["images"][0]["content_pixels"].as_u64().unwrap() > 0);

    let hidden_opaque = opaque_pixel_count(&hidden_bytes);
    let visible_opaque = opaque_pixel_count(&visible_bytes);
    assert_eq!(hidden_opaque, 0);
    assert_eq!(
        visible_opaque as u64,
        visible["images"][0]["content_pixels"].as_u64().unwrap()
    );

    fs::remove_file(&path).expect("remove temp native typewriter source");
    fs::remove_dir_all(&dir).expect("remove temp native typewriter dir");
}

#[test]
fn agent_observe_native_typewriter_capture_time_controls_object_id() {
    let path = temp_arcw(
        "agent-observe-native-typewriter-object-id-capture-time",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl][.typewriter cps=1]吾輩[/][/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-typewriter-object-id-capture-time");
    let hidden_path = dir.join("native-typewriter-hidden-object-id.rgba");
    let visible_path = dir.join("native-typewriter-visible-object-id.rgba");

    let (hidden, hidden_bytes) = observe_native_typewriter_cluster_raw_at(
        &path,
        &hidden_path,
        "object.dialogue.0.0.cluster.0.0.3",
        "0",
        "object-id",
    );
    let (visible, _visible_bytes) = observe_native_typewriter_cluster_raw_at(
        &path,
        &visible_path,
        "object.dialogue.0.0.cluster.0.0.3",
        "4",
        "object-id",
    );
    let hidden_cluster = find_rich_text_cluster_object(&hidden, "吾", 0, 3);
    let visible_cluster = find_rich_text_cluster_object(&visible, "吾", 0, 3);
    assert_eq!(hidden_cluster["bbox"], visible_cluster["bbox"]);
    assert_eq!(
        hidden["images"][0]["crop_origin"],
        visible["images"][0]["crop_origin"]
    );
    assert_eq!(hidden["images"][0]["width"], visible["images"][0]["width"]);
    assert_eq!(
        hidden["images"][0]["height"],
        visible["images"][0]["height"]
    );
    assert_eq!(hidden["images"][0]["content_pixels"], 0);
    let visible_pixels = visible["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(visible_pixels > 0);

    assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
    assert_raw_object_id_tint(
        &visible_path,
        agent_object_id_color_from_json(visible_cluster),
        visible_pixels,
        "typewriter cluster object-id capture-time crop",
    );

    fs::remove_file(&path).expect("remove temp native typewriter object-id source");
    fs::remove_dir_all(&dir).expect("remove temp native typewriter object-id dir");
}

#[test]
fn agent_observe_native_typewriter_text_combine_capture_time_controls_all_glyphs() {
    assert_native_typewriter_text_combine_capture_time_controls_all_glyphs(
        "vertical_rl",
        "typewriter-text-combine",
    );
}

#[test]
fn agent_observe_native_vertical_lr_typewriter_text_combine_capture_time_controls_all_glyphs() {
    assert_native_typewriter_text_combine_capture_time_controls_all_glyphs(
        "vertical_lr",
        "vertical-lr-typewriter-text-combine",
    );
}

fn assert_native_typewriter_text_combine_capture_time_controls_all_glyphs(
    writing_mode: &str,
    label: &str,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{label}-capture-time"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}][.typewriter cps=1]2026[/][/][p]
}}
",
        ),
    );
    let dir = temp_dir(&format!("agent-observe-native-{label}-capture-time"));
    let hidden_path = dir.join(format!("native-{label}-hidden-mask.rgba"));
    let visible_path = dir.join(format!("native-{label}-visible-mask.rgba"));

    let (hidden, hidden_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &hidden_path,
        "object.dialogue.0.0.cluster.0.0.4",
        "0",
    );
    let (visible, visible_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &visible_path,
        "object.dialogue.0.0.cluster.0.0.4",
        "4",
    );
    let hidden_cluster = find_rich_text_cluster_object(&hidden, "2026", 0, 4);
    let visible_cluster = find_rich_text_cluster_object(&visible, "2026", 0, 4);
    assert_eq!(
        hidden_cluster["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        visible_cluster["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(hidden_cluster["bbox"], visible_cluster["bbox"]);
    assert_eq!(
        hidden["images"][0]["crop_origin"],
        visible["images"][0]["crop_origin"]
    );
    assert_eq!(hidden["images"][0]["width"], visible["images"][0]["width"]);
    assert_eq!(
        hidden["images"][0]["height"],
        visible["images"][0]["height"]
    );
    assert_eq!(hidden["images"][0]["content_pixels"], 0);
    assert!(visible["images"][0]["content_pixels"].as_u64().unwrap() > 0);

    let hidden_opaque = opaque_pixel_count(&hidden_bytes);
    let visible_opaque = opaque_pixel_count(&visible_bytes);
    assert_eq!(hidden_opaque, 0);
    assert_eq!(
        visible_opaque as u64,
        visible["images"][0]["content_pixels"].as_u64().unwrap()
    );

    fs::remove_file(&path).expect("remove temp native typewriter text-combine source");
    fs::remove_dir_all(&dir).expect("remove temp native typewriter text-combine dir");
}

#[test]
fn agent_observe_native_typewriter_text_combine_capture_time_controls_object_id() {
    assert_native_typewriter_text_combine_capture_time_controls_object_id(
        "vertical_rl",
        "typewriter-text-combine",
    );
}

#[test]
fn agent_observe_native_vertical_lr_typewriter_text_combine_capture_time_controls_object_id() {
    assert_native_typewriter_text_combine_capture_time_controls_object_id(
        "vertical_lr",
        "vertical-lr-typewriter-text-combine",
    );
}

fn assert_native_typewriter_text_combine_capture_time_controls_object_id(
    writing_mode: &str,
    label: &str,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{label}-object-id-capture-time"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}][.typewriter cps=1]2026[/][/][p]
}}
",
        ),
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-{label}-object-id-capture-time"
    ));
    let hidden_path = dir.join(format!("native-{label}-hidden-object-id.rgba"));
    let visible_path = dir.join(format!("native-{label}-visible-object-id.rgba"));

    let (hidden, hidden_bytes) = observe_native_typewriter_cluster_raw_at(
        &path,
        &hidden_path,
        "object.dialogue.0.0.cluster.0.0.4",
        "0",
        "object-id",
    );
    let (visible, _visible_bytes) = observe_native_typewriter_cluster_raw_at(
        &path,
        &visible_path,
        "object.dialogue.0.0.cluster.0.0.4",
        "4",
        "object-id",
    );
    let hidden_cluster = find_rich_text_cluster_object(&hidden, "2026", 0, 4);
    let visible_cluster = find_rich_text_cluster_object(&visible, "2026", 0, 4);
    assert_eq!(
        hidden_cluster["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        visible_cluster["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(hidden_cluster["bbox"], visible_cluster["bbox"]);
    assert_eq!(
        hidden["images"][0]["crop_origin"],
        visible["images"][0]["crop_origin"]
    );
    assert_eq!(hidden["images"][0]["width"], visible["images"][0]["width"]);
    assert_eq!(
        hidden["images"][0]["height"],
        visible["images"][0]["height"]
    );
    assert_eq!(hidden["images"][0]["content_pixels"], 0);
    let visible_pixels = visible["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(visible_pixels > 0);

    assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
    assert_raw_object_id_tint(
        &visible_path,
        agent_object_id_color_from_json(visible_cluster),
        visible_pixels,
        "typewriter text-combine object-id capture-time crop",
    );

    fs::remove_file(&path).expect("remove temp native typewriter text-combine object-id source");
    fs::remove_dir_all(&dir).expect("remove temp native typewriter text-combine object-id dir");
}

#[test]
fn agent_observe_native_typewriter_ruby_capture_time_controls_base_and_annotation() {
    assert_native_typewriter_ruby_capture_time_controls_base_and_annotation(
        "vertical_rl",
        "typewriter-ruby",
        true,
    );
}

#[test]
fn agent_observe_native_vertical_lr_typewriter_ruby_capture_time_controls_base_and_annotation() {
    assert_native_typewriter_ruby_capture_time_controls_base_and_annotation(
        "vertical_lr",
        "vertical-lr-typewriter-ruby",
        false,
    );
}

fn assert_native_typewriter_ruby_capture_time_controls_base_and_annotation(
    writing_mode: &str,
    label: &str,
    ruby_annotation_on_right: bool,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{label}-capture-time"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬[.typewriter cps=1]|[夢](ながいながいよみ)人外[/][/][p]
}}
",
        ),
    );
    let dir = temp_dir(&format!("agent-observe-native-{label}-capture-time"));
    let hidden_path = dir.join(format!("native-{label}-hidden-mask.rgba"));
    let visible_path = dir.join(format!("native-{label}-visible-mask.rgba"));

    let (hidden, hidden_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &hidden_path,
        "object.dialogue.0.0.ruby.0",
        "0",
    );
    let (visible, visible_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &visible_path,
        "object.dialogue.0.0.ruby.0",
        "4",
    );
    assert_native_typewriter_ruby_capture_time_geometry(
        &hidden,
        &visible,
        ruby_annotation_on_right,
    );
    assert_eq!(hidden["images"][0]["content_pixels"], 0);
    assert!(visible["images"][0]["content_pixels"].as_u64().unwrap() > 0);

    assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
    assert_eq!(
        opaque_pixel_count(&visible_bytes) as u64,
        visible["images"][0]["content_pixels"].as_u64().unwrap()
    );

    fs::remove_file(&path).expect("remove temp native typewriter ruby source");
    fs::remove_dir_all(&dir).expect("remove temp native typewriter ruby dir");
}

#[test]
fn agent_observe_native_typewriter_ruby_capture_time_controls_object_id() {
    assert_native_typewriter_ruby_capture_time_controls_object_id(
        "vertical_rl",
        "typewriter-ruby",
        true,
    );
}

#[test]
fn agent_observe_native_vertical_lr_typewriter_ruby_capture_time_controls_object_id() {
    assert_native_typewriter_ruby_capture_time_controls_object_id(
        "vertical_lr",
        "vertical-lr-typewriter-ruby",
        false,
    );
}

fn assert_native_typewriter_ruby_capture_time_controls_object_id(
    writing_mode: &str,
    label: &str,
    ruby_annotation_on_right: bool,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{label}-object-id-capture-time"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬[.typewriter cps=1]|[夢](ながいながいよみ)人外[/][/][p]
}}
",
        ),
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-{label}-object-id-capture-time"
    ));
    let hidden_path = dir.join(format!("native-{label}-hidden-object-id.rgba"));
    let visible_path = dir.join(format!("native-{label}-visible-object-id.rgba"));

    let (hidden, hidden_bytes) = observe_native_typewriter_cluster_raw_at(
        &path,
        &hidden_path,
        "object.dialogue.0.0.ruby.0",
        "0",
        "object-id",
    );
    let (visible, _visible_bytes) = observe_native_typewriter_cluster_raw_at(
        &path,
        &visible_path,
        "object.dialogue.0.0.ruby.0",
        "4",
        "object-id",
    );
    let visible_ruby = assert_native_typewriter_ruby_capture_time_geometry(
        &hidden,
        &visible,
        ruby_annotation_on_right,
    );
    assert_eq!(hidden["images"][0]["content_pixels"], 0);
    let visible_pixels = visible["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(visible_pixels > 0);

    assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
    assert_raw_object_id_tint(
        &visible_path,
        agent_object_id_color_from_json(visible_ruby),
        visible_pixels,
        "typewriter ruby object-id capture-time crop",
    );

    fs::remove_file(&path).expect("remove temp native typewriter ruby object-id source");
    fs::remove_dir_all(&dir).expect("remove temp native typewriter ruby object-id dir");
}

fn assert_native_typewriter_ruby_capture_time_geometry<'a>(
    hidden: &serde_json::Value,
    visible: &'a serde_json::Value,
    ruby_annotation_on_right: bool,
) -> &'a serde_json::Value {
    let hidden_ruby = find_rich_text_ruby_object(hidden, 0);
    let visible_ruby = find_rich_text_ruby_object(visible, 0);
    assert_eq!(
        visible_ruby["rich_text_ref"]["presentation"]["effects"][0]["id"],
        "typewriter"
    );
    assert_eq!(
        visible_ruby["rich_text_ref"]["presentation"]["effects"][0]["phase"],
        "glyph_mask"
    );
    assert_eq!(hidden_ruby["bbox"], visible_ruby["bbox"]);
    assert_eq!(
        hidden_ruby["rich_text_ref"]["ruby_base_bbox"],
        visible_ruby["rich_text_ref"]["ruby_base_bbox"]
    );
    assert_eq!(
        hidden_ruby["rich_text_ref"]["ruby_annotation_bbox"],
        visible_ruby["rich_text_ref"]["ruby_annotation_bbox"]
    );
    assert_eq!(
        hidden["images"][0]["crop_origin"],
        visible["images"][0]["crop_origin"]
    );
    assert_eq!(hidden["images"][0]["width"], visible["images"][0]["width"]);
    assert_eq!(
        hidden["images"][0]["height"],
        visible["images"][0]["height"]
    );
    let base = &visible_ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &visible_ruby["rich_text_ref"]["ruby_annotation_bbox"];
    if ruby_annotation_on_right {
        assert!(
            agent_json_bbox_center_x_twice(annotation) > agent_json_bbox_center_x_twice(base),
            "vertical_rl typewriter ruby annotation should stay on the right side of the base: {visible_ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_center_x_twice(annotation) < agent_json_bbox_center_x_twice(base),
            "vertical_lr typewriter ruby annotation should stay on the left side of the base: {visible_ruby}"
        );
    }
    visible_ruby
}

#[test]
fn agent_observe_native_renderer_writes_rich_text_layer_mask_attachment() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-layer-mask",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let dir = temp_dir("agent-observe-native-rich-text-layer-mask");
    let raw_path = dir.join("native-rich-text-layer-mask.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native rich-text layer mask raw crop");

    assert!(
        output.status.success(),
        "native rich-text layer mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native rich-text layer mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(json["images"][0]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");
    assert_eq!(json["images"][0]["crop_origin"]["space"], "viewport");
    assert!(json["images"][0]["width"].as_u64().unwrap() < 1088);
    assert!(json["images"][0]["height"].as_u64().unwrap() < 124);
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_bbox = &json["images"][0]["content_bbox"];
    let content_viewport_bbox = &json["images"][0]["content_viewport_bbox"];
    assert_eq!(
        content_viewport_bbox["x"].as_u64().unwrap(),
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap()
            + content_bbox["x"].as_u64().unwrap()
    );
    assert_eq!(
        content_viewport_bbox["y"].as_u64().unwrap(),
        json["images"][0]["crop_origin"]["y"].as_u64().unwrap()
            + content_bbox["y"].as_u64().unwrap()
    );
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native rich-text layer mask raw crop");
    assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] > 0));
    assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] == 0));

    fs::remove_file(&path).expect("remove temp native rich-text layer mask source");
    fs::remove_dir_all(&dir).expect("remove temp native rich-text layer mask dir");
}

#[test]
fn agent_observe_native_renderer_writes_rich_text_layer_object_id_attachment() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-layer-object-id",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let dir = temp_dir("agent-observe-native-rich-text-layer-object-id");
    let raw_path = dir.join("native-rich-text-layer-object-id.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("object-id")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native rich-text layer object-id raw crop");

    assert!(
        output.status.success(),
        "native rich-text layer object-id crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native rich-text layer object-id report is JSON");
    assert_eq!(json["images"][0]["kind"], "object_id");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(json["images"][0]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["images"][0]["composition"], "object_id_attachment");
    assert_eq!(json["images"][0]["crop_origin"]["space"], "viewport");
    assert!(json["images"][0]["width"].as_u64().unwrap() < 1088);
    assert!(json["images"][0]["height"].as_u64().unwrap() < 124);
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native rich-text layer object-id raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(transparent > 0);

    fs::remove_file(&path).expect("remove temp native rich-text layer object-id source");
    fs::remove_dir_all(&dir).expect("remove temp native rich-text layer object-id dir");
}
