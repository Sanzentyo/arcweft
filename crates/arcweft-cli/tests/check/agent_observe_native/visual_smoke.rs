#[test]
fn visual_smoke_viewport_layer_and_object_captures_expose_selected_metadata() {
    let path = visual_smoke_source("agent-visual-smoke-selected-metadata");
    let dir = temp_dir("agent-visual-smoke-selected-metadata");

    let viewport_path = dir.join("visual-smoke-viewport.png");
    let viewport = visual_smoke_capture(&path, &viewport_path, "png", None, &[], &[]);
    let viewport_image = visual_smoke_image(&viewport);
    assert_visual_smoke_png(&viewport_path);
    assert_eq!(viewport_image["kind"], "color");
    assert_eq!(viewport_image["renderer"], "native");
    assert_eq!(viewport_image["scope"]["kind"], "viewport");
    assert_eq!(viewport_image["composition"], "framebuffer");
    assert_eq!(viewport_image["width"], 1280);
    assert_eq!(viewport_image["height"], 720);
    assert!(
        viewport_image["content_pixels"]
            .as_u64()
            .unwrap_or_default()
            > 0,
        "viewport visual smoke should render non-empty content: {viewport}"
    );
    assert!(
        viewport_image
            .get("selected_capture")
            .is_none_or(serde_json::Value::is_null),
        "viewport capture is full-frame metadata, not a selected object/layer capture: \
         {viewport_image}"
    );
    let dialogue_view_id = find_dialogue_view_object(&viewport)["id"]
        .as_str()
        .expect("observed dialogue_view id is a string")
        .to_owned();

    let layer_path = dir.join("visual-smoke-dialogue-layer.png");
    let layer = visual_smoke_capture(
        &path,
        &layer_path,
        "png",
        None,
        &["--layer", "dialogue"],
        &[],
    );
    let layer_image = visual_smoke_image(&layer);
    assert_visual_smoke_png(&layer_path);
    assert_eq!(layer_image["kind"], "color");
    assert_eq!(layer_image["scope"]["kind"], "layer");
    assert_eq!(layer_image["scope"]["id"], "dialogue");
    assert_eq!(layer_image["composition"], "masked_framebuffer_crop");
    assert!(layer_image["content_pixels"].as_u64().unwrap_or_default() > 0);
    assert_visual_smoke_selected_capture(
        layer_image,
        "layer",
        "dialogue",
        &["masked_framebuffer_crop"],
    );
    let layer_selected = visual_smoke_selected_capture(layer_image);
    assert_eq!(layer_selected["source"]["kind"], "layer");
    assert_eq!(layer_selected["source"]["id"], "dialogue");
    assert!(
        layer_selected["source"]["object_count"]
            .as_u64()
            .unwrap_or_default()
            > 0,
        "layer selected-capture source should summarize observed objects: {layer_selected}"
    );
    assert_visual_smoke_selected_crop_matches_image(layer_image);

    let object_path = dir.join("visual-smoke-dialogue-object.png");
    let object = visual_smoke_capture(
        &path,
        &object_path,
        "png",
        None,
        &["--object", dialogue_view_id.as_str()],
        &[],
    );
    let object_image = visual_smoke_image(&object);
    assert_visual_smoke_png(&object_path);
    assert_eq!(object_image["kind"], "color");
    assert_eq!(object_image["scope"]["kind"], "object");
    assert_eq!(object_image["scope"]["id"], dialogue_view_id);
    assert!(object_image["content_pixels"].as_u64().unwrap_or_default() > 0);
    assert_visual_smoke_selected_capture(
        object_image,
        "object",
        &dialogue_view_id,
        &[
            "framebuffer_crop",
            "isolated_regions",
            "masked_framebuffer_crop",
        ],
    );
    let object_selected = visual_smoke_selected_capture(object_image);
    assert_eq!(object_selected["source"]["kind"], "object");
    assert_eq!(object_selected["source"]["id"], dialogue_view_id);
    assert_eq!(object_selected["source"]["role"], "dialogue_view");
    assert_visual_smoke_selected_crop_matches_image(object_image);

    fs::remove_file(&path).expect("remove temp visual smoke source");
    fs::remove_dir_all(&dir).expect("remove temp visual smoke dir");
}

#[test]
fn visual_smoke_object_id_and_mask_captures_have_debug_pixels_and_metadata() {
    let path = visual_smoke_source("agent-visual-smoke-debug-attachments");
    let dir = temp_dir("agent-visual-smoke-debug-attachments");

    let object_id_path = dir.join("visual-smoke-dialogue-layer-object-id.rgba");
    let object_id = visual_smoke_capture(
        &path,
        &object_id_path,
        "raw-rgba",
        Some("object-id"),
        &["--layer", "dialogue"],
        &[],
    );
    let object_id_image = visual_smoke_image(&object_id);
    assert_eq!(object_id_image["kind"], "object_id");
    assert_eq!(object_id_image["mime_type"], "application/octet-stream");
    assert_visual_smoke_selected_capture(
        object_id_image,
        "layer",
        "dialogue",
        &["object_id_attachment"],
    );
    assert_visual_smoke_raw_len_matches_image(&object_id_path, object_id_image);
    let object_id_selected = visual_smoke_selected_capture(object_id_image);
    assert!(
        object_id_selected["mask"]["has_object_id_attachment"]
            .as_bool()
            .unwrap_or(false),
        "object-id smoke should describe the object-id attachment: {object_id_selected}"
    );
    let object_ids = object_id_selected["mask"]["object_ids"]
        .as_array()
        .expect("object-id metadata carries object ids");
    assert_visual_smoke_metadata_ids_are_unique(object_ids);
    let object_id_bytes = fs::read(&object_id_path).expect("read visual smoke object-id RGBA");
    assert_eq!(
        visual_smoke_non_transparent_color_count(&object_id_bytes),
        object_ids.len(),
        "object-id attachment should encode one debug color per selected object: {object_id}"
    );
    let dialogue_view_id = find_dialogue_view_object(&object_id)["id"]
        .as_str()
        .expect("observed dialogue_view id is a string")
        .to_owned();

    let mask_path = dir.join("visual-smoke-dialogue-object-mask.rgba");
    let mask = visual_smoke_capture(
        &path,
        &mask_path,
        "raw-rgba",
        Some("mask"),
        &["--object", dialogue_view_id.as_str()],
        &[],
    );
    let mask_image = visual_smoke_image(&mask);
    assert_eq!(mask_image["kind"], "mask");
    assert_eq!(mask_image["mime_type"], "application/octet-stream");
    assert_visual_smoke_selected_capture(mask_image, "object", &dialogue_view_id, &["mask_attachment"]);
    assert_visual_smoke_raw_len_matches_image(&mask_path, mask_image);
    let mask_bytes = fs::read(&mask_path).expect("read visual smoke mask RGBA");
    let opaque_pixels = mask_bytes
        .chunks_exact(4)
        .filter(|pixel| pixel[3] > 0)
        .count();
    assert!(
        opaque_pixels > 0,
        "mask smoke should contain selected opaque pixels: {mask}"
    );
    assert_eq!(
        u64::try_from(opaque_pixels).expect("opaque pixel count fits u64"),
        mask_image["content_pixels"]
            .as_u64()
            .expect("mask content pixel count is reported"),
        "mask attachment metadata should count its opaque selected pixels: {mask}"
    );
    let mask_selected = visual_smoke_selected_capture(mask_image);
    assert!(
        mask_selected["mask"]["has_alpha_mask"]
            .as_bool()
            .unwrap_or(false),
        "mask smoke should describe alpha-mask availability: {mask_selected}"
    );
    assert_eq!(mask_selected["mask"]["availability"], "available");
    assert_eq!(mask_selected["source"]["kind"], "object");
    assert_eq!(mask_selected["source"]["id"], dialogue_view_id);
    assert_eq!(mask_selected["source"]["role"], "dialogue_view");
    assert_eq!(
        mask_selected["mask"]["object_ids"],
        serde_json::json!([dialogue_view_id])
    );
    assert_visual_smoke_selected_crop_matches_image(mask_image);

    fs::remove_file(&path).expect("remove temp visual smoke debug source");
    fs::remove_dir_all(&dir).expect("remove temp visual smoke debug dir");
}

fn visual_smoke_source(label: &str) -> PathBuf {
    temp_arcw(
        label,
        r#"
entry cli @entry.main { goto @flow.main }

character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r][br][.vertical_rl]吾輩は猫である。ABC 123 2026。[/][p]
}
"#,
    )
}

fn visual_smoke_capture(
    source_path: &Path,
    out_path: &Path,
    image_kind: &str,
    capture_kind: Option<&str>,
    scope_args: &[&str],
    extra_args: &[&str],
) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
    command
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--json")
        .arg("--image")
        .arg(image_kind);
    if let Some(capture_kind) = capture_kind {
        command.arg("--capture").arg(capture_kind);
    }
    command
        .args(scope_args)
        .args(extra_args)
        .arg("--out")
        .arg(out_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("128");
    let output = command
        .output()
        .expect("arcw agent observe writes visual smoke capture");
    assert!(
        output.status.success(),
        "visual smoke capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("visual smoke capture report is JSON")
}

fn visual_smoke_image(report: &serde_json::Value) -> &serde_json::Value {
    &report["images"]
        .as_array()
        .expect("visual smoke report has images")[0]
}

fn visual_smoke_selected_capture(image: &serde_json::Value) -> &serde_json::Value {
    image
        .get("selected_capture")
        .filter(|value| !value.is_null())
        .unwrap_or_else(|| panic!("selected_capture metadata should be present: {image}"))
}

fn assert_visual_smoke_png(path: &Path) {
    let bytes = fs::read(path).expect("read visual smoke PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
}

fn assert_visual_smoke_raw_len_matches_image(path: &Path, image: &serde_json::Value) {
    let bytes = fs::read(path).expect("read visual smoke raw RGBA");
    let width = usize::try_from(image["width"].as_u64().expect("image width is integer"))
        .expect("image width fits usize");
    let height = usize::try_from(image["height"].as_u64().expect("image height is integer"))
        .expect("image height fits usize");
    assert_eq!(
        bytes.len(),
        width.saturating_mul(height).saturating_mul(4),
        "raw RGBA byte length should match visual smoke metadata: {image}"
    );
}

fn assert_visual_smoke_selected_capture(
    image: &serde_json::Value,
    expected_scope_kind: &str,
    expected_scope_id: &str,
    expected_compositions: &[&str],
) {
    let selected = visual_smoke_selected_capture(image);
    assert_eq!(selected["renderer"], "shared_wgpu_prepared_frame");
    assert_eq!(selected["scope"]["kind"], expected_scope_kind);
    assert_eq!(selected["scope"]["id"], expected_scope_id);
    let composition = selected["composition"]
        .as_str()
        .expect("selected capture composition is a string");
    assert!(
        expected_compositions.contains(&composition),
        "unexpected selected capture composition `{composition}`: {selected}"
    );
    assert_eq!(selected["coordinate_basis"], "output");
    assert_eq!(selected["crop"]["basis"], "output");
    assert_eq!(selected["fit_transform"]["policy"], "raw");
    assert!(
        selected["mask"].is_object(),
        "selected capture should expose mask/object-id metadata even for color smoke captures: \
         {selected}"
    );
}

fn assert_visual_smoke_selected_crop_matches_image(image: &serde_json::Value) {
    let selected = visual_smoke_selected_capture(image);
    let clipped = &selected["crop"]["clipped"];
    let origin = &clipped["origin"];
    let size = &clipped["size"];
    assert_eq!(
        visual_smoke_json_number(&origin["x"]),
        visual_smoke_json_number(&image["crop_origin"]["x"]),
        "selected capture clipped x should match image crop origin: {image}"
    );
    assert_eq!(
        visual_smoke_json_number(&origin["y"]),
        visual_smoke_json_number(&image["crop_origin"]["y"]),
        "selected capture clipped y should match image crop origin: {image}"
    );
    assert_eq!(
        visual_smoke_json_number(&size["width"]),
        visual_smoke_json_number(&image["width"]),
        "selected capture clipped width should match image width: {image}"
    );
    assert_eq!(
        visual_smoke_json_number(&size["height"]),
        visual_smoke_json_number(&image["height"]),
        "selected capture clipped height should match image height: {image}"
    );
}

fn visual_smoke_json_number(value: &serde_json::Value) -> Option<String> {
    value.as_f64().map(|value| format!("{value:.3}"))
}

fn visual_smoke_non_transparent_color_count(rgba: &[u8]) -> usize {
    let mut colors = std::collections::BTreeSet::new();
    for pixel in rgba.chunks_exact(4).filter(|pixel| pixel[3] > 0) {
        colors.insert([pixel[0], pixel[1], pixel[2], pixel[3]]);
    }
    colors.len()
}

fn assert_visual_smoke_metadata_ids_are_unique(ids: &[serde_json::Value]) {
    let mut unique = std::collections::BTreeSet::new();
    for id in ids {
        let id = id.as_str().expect("metadata id is a string");
        assert!(unique.insert(id), "metadata id should be unique: {ids:?}");
    }
    assert!(!unique.is_empty(), "metadata id set should not be empty");
}
