fn assert_native_strict_jlreq_style_split_geometry(writing_mode: &str) {
    let json = observe_native_strict_jlreq_style_split_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_strict_jlreq_style_split_opening(&json);
}

fn observe_native_strict_jlreq_style_split_fixture(writing_mode: &str) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-strict-jlreq-style-split"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地。[color red:「人]山川海[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp strict JLREQ style-split source");
    json
}

fn assert_native_strict_jlreq_style_split_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name =
        format!("agent-observe-native-{writing_mode}-strict-jlreq-style-split-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地。[color red:「人]山川海[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-strict-jlreq-style-split-{capture_kind}.rgba"
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
        .expect("arcw agent observe writes native strict JLREQ style-split raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} strict JLREQ style-split {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native strict JLREQ style-split report is JSON");
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

    let opening = assert_native_strict_jlreq_style_split_opening(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], opening["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], opening["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], opening["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], opening["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(opening),
            content_pixels,
            &format!("{writing_mode} strict JLREQ style-split object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native strict JLREQ style-split mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp strict JLREQ style-split source");
    fs::remove_dir_all(&dir).expect("remove temp strict JLREQ style-split dir");
}

fn assert_native_strict_jlreq_style_split_opening(json: &serde_json::Value) -> &serde_json::Value {
    let full_stop = find_rich_text_cluster_object(json, "。", 6, 9);
    let opening = find_rich_text_cluster_object(json, "「", 9, 12);
    let person = find_rich_text_cluster_object(json, "人", 12, 15);
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict style-split paragraph keeps adjacent closing/opening punctuation together",
    );
    assert_vertical_cluster_after(
        opening,
        person,
        "strict style-split opening punctuation keeps its following base",
    );
    assert_eq!(opening["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(
        opening["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    assert_rich_text_object_has_mask_capture(opening, "strict JLREQ style-split opening cluster");
    opening
}

fn assert_native_published_jlreq_european_numeral_sequence_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let json = observe_native_published_jlreq_european_numeral_sequence_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_european_numeral_sequence_objects(
        &json,
        writing_mode,
        next_column_moves_right,
    );
}

fn observe_native_published_jlreq_european_numeral_sequence_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-published-jlreq-european-numerals"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地2026502671234人[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp published JLREQ European numeral source");
    json
}

fn assert_native_published_jlreq_european_numeral_sequence_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-european-numerals-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地2026502671234人[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-european-numerals-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.5.18.19")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ European numeral raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ European numeral {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ European numeral report is JSON");
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

    let final_digit = assert_native_published_jlreq_european_numeral_sequence_objects(
        &json,
        writing_mode,
        next_column_moves_right,
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        final_digit["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        final_digit["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], final_digit["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], final_digit["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(final_digit),
            content_pixels,
            &format!("{writing_mode} published JLREQ European numeral object-id crop"),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native published JLREQ European numeral mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ European numeral source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ European numeral dir");
}

fn assert_native_published_jlreq_european_numeral_sequence_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    let earth = find_rich_text_cluster_object(json, "地", 3, 6);
    let first_digits = find_rich_text_cluster_object(json, "2026", 6, 10);
    let second_digits = find_rich_text_cluster_object(json, "5026", 10, 14);
    let third_digits = find_rich_text_cluster_object(json, "7123", 14, 18);
    let final_digit = find_rich_text_cluster_object(json, "4", 18, 19);
    let next_body = find_rich_text_cluster_object(json, "人", 19, 22);
    assert_eq!(
        first_digits["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        second_digits["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        third_digits["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(final_digit["rich_text_ref"]["orientation"], "sideways_cw");
    assert_next_paragraph_column(
        earth,
        first_digits,
        next_column_moves_right,
        "published JLREQ European numeral sequence should restart as a unit after body text",
    );
    assert_vertical_cluster_after(
        first_digits,
        second_digits,
        "second text-combine chunk should stay with the previous European numeral chunk",
    );
    assert_vertical_cluster_after(
        second_digits,
        third_digits,
        "third text-combine chunk should stay with the previous European numeral chunk",
    );
    assert_vertical_cluster_after(
        third_digits,
        final_digit,
        "final digit should stay with the preceding text-combine chunks",
    );
    assert_next_paragraph_column(
        final_digit,
        next_body,
        next_column_moves_right,
        "body text after the European numeral sequence should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        final_digit,
        &format!("{writing_mode} published JLREQ European numeral final digit"),
    );
    final_digit
}

fn assert_native_published_jlreq_numeric_separator_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    for case in native_published_jlreq_numeric_separator_cases() {
        let json = observe_native_published_jlreq_numeric_separator_fixture(writing_mode, case);
        assert_native_rich_text_layer_image_has_content(&json);
        assert_eq!(
            first_text_run_presentation_layout(&json)["writing_mode"],
            writing_mode
        );
        assert_native_published_jlreq_numeric_separator_objects(
            &json,
            writing_mode,
            next_column_moves_right,
            case,
        );
    }
}

fn observe_native_published_jlreq_numeric_separator_fixture(
    writing_mode: &str,
    case: NativeNumericSeparatorCase,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-numeric-separator-{}",
            case.label
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]{}[/][p]
}}
",
            case.text
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp published JLREQ numeric separator source");
    json
}

fn assert_native_published_jlreq_numeric_separator_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
) {
    for case in native_published_jlreq_numeric_separator_cases() {
        assert_native_published_jlreq_numeric_separator_case_raw_crop(
            writing_mode,
            next_column_moves_right,
            capture_kind,
            case,
        );
    }
}

fn assert_native_published_jlreq_numeric_separator_case_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
    case: NativeNumericSeparatorCase,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-numeric-separator-{}-{capture_kind}",
        case.label
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]{}[/][p]
}}
",
            case.text
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-numeric-separator-{}-{capture_kind}.rgba",
        case.label
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
        .arg(case.object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ numeric separator raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ numeric separator {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ numeric separator report is JSON");
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

    let crop_target = assert_native_published_jlreq_numeric_separator_objects(
        &json,
        writing_mode,
        next_column_moves_right,
        case,
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        crop_target["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        crop_target["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], crop_target["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], crop_target["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(crop_target),
            content_pixels,
            &format!(
                "{writing_mode} published JLREQ {} numeric separator {} object-id crop",
                case.description, case.crop_description
            ),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native published JLREQ numeric separator crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ numeric separator source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ numeric separator dir");
}

fn assert_native_published_jlreq_numeric_separator_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
    case: NativeNumericSeparatorCase,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let leading_digits =
        find_rich_text_cluster_object(json, case.leading_digits, 3, case.leading_digits_end);
    let preceding_digits = find_rich_text_cluster_object(
        json,
        case.preceding_digits,
        case.preceding_range.0,
        case.preceding_range.1,
    );
    let separator = case.separator_is_observed.then(|| {
        find_rich_text_cluster_object(
            json,
            case.separator,
            case.separator_range.0,
            case.separator_range.1,
        )
    });
    let following_digits = find_rich_text_cluster_object(
        json,
        case.following_digits,
        case.following_range.0,
        case.following_range.1,
    );
    let crop_target =
        find_rich_text_cluster_object(json, case.crop_text, case.crop_range.0, case.crop_range.1);
    let next_body = find_rich_text_cluster_object(json, "人", case.next_range.0, case.next_range.1);
    assert_vertical_cluster_after(
        body,
        leading_digits,
        "published JLREQ European numeral with separators should start after body text",
    );
    if let Some(separator) = separator {
        assert_vertical_cluster_after(
            preceding_digits,
            separator,
            &format!(
                "published JLREQ {} separator should stay with the preceding digit chunk",
                case.description
            ),
        );
        assert_vertical_cluster_after(
            separator,
            following_digits,
            &format!(
                "published JLREQ digits after a {} separator should stay attached",
                case.description
            ),
        );
        assert_rich_text_object_has_mask_capture(
            separator,
            &format!(
                "{writing_mode} published JLREQ numeric separator {}",
                case.description
            ),
        );
    } else {
        assert_vertical_cluster_after(
            preceding_digits,
            following_digits,
            &format!(
                "published JLREQ digits after a {} separator should stay attached",
                case.description
            ),
        );
        assert!(
            agent_json_bbox_y(&following_digits["bbox"])
                > agent_json_bbox_bottom(&preceding_digits["bbox"]),
            "published JLREQ {} separator should leave a visible layout gap between digit chunks",
            case.description
        );
    }
    assert_next_paragraph_column(
        following_digits,
        next_body,
        next_column_moves_right,
        "body text after the published JLREQ European numeral with separators should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        crop_target,
        &format!(
            "{writing_mode} published JLREQ numeric separator {} crop target",
            case.description
        ),
    );
    crop_target
}

#[derive(Clone, Copy)]
struct NativeNumericSeparatorCase {
    label: &'static str,
    text: &'static str,
    leading_digits: &'static str,
    leading_digits_end: u64,
    preceding_digits: &'static str,
    preceding_range: (u64, u64),
    separator: &'static str,
    separator_range: (u64, u64),
    separator_is_observed: bool,
    following_digits: &'static str,
    following_range: (u64, u64),
    crop_text: &'static str,
    crop_range: (u64, u64),
    object_id: &'static str,
    next_range: (u64, u64),
    description: &'static str,
    crop_description: &'static str,
}

const fn native_published_jlreq_numeric_separator_cases() -> [NativeNumericSeparatorCase; 3] {
    [
        NativeNumericSeparatorCase {
            label: "comma",
            text: "天1,234人",
            leading_digits: "1",
            leading_digits_end: 4,
            preceding_digits: "1",
            preceding_range: (3, 4),
            separator: ",",
            separator_range: (4, 5),
            separator_is_observed: true,
            following_digits: "234",
            following_range: (5, 8),
            crop_text: ",",
            crop_range: (4, 5),
            object_id: "object.dialogue.0.0.cluster.2.4.5",
            next_range: (8, 11),
            description: "comma",
            crop_description: "separator",
        },
        NativeNumericSeparatorCase {
            label: "decimal-point",
            text: "天1.23人",
            leading_digits: "1",
            leading_digits_end: 4,
            preceding_digits: "1",
            preceding_range: (3, 4),
            separator: ".",
            separator_range: (4, 5),
            separator_is_observed: true,
            following_digits: "23",
            following_range: (5, 7),
            crop_text: ".",
            crop_range: (4, 5),
            object_id: "object.dialogue.0.0.cluster.2.4.5",
            next_range: (7, 10),
            description: "decimal point",
            crop_description: "separator",
        },
        NativeNumericSeparatorCase {
            label: "space-place",
            text: "天12 345人",
            leading_digits: "12",
            leading_digits_end: 5,
            preceding_digits: "12",
            preceding_range: (3, 5),
            separator: " ",
            separator_range: (5, 6),
            separator_is_observed: false,
            following_digits: "345",
            following_range: (6, 9),
            crop_text: "345",
            crop_range: (6, 9),
            object_id: "object.dialogue.0.0.cluster.3.6.9",
            next_range: (9, 12),
            description: "space place",
            crop_description: "following digit chunk",
        },
    ]
}

fn assert_native_published_jlreq_numeric_abbreviation_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let prefix = observe_native_published_jlreq_numeric_abbreviation_fixture(writing_mode, "$");
    assert_native_rich_text_layer_image_has_content(&prefix);
    assert_eq!(
        first_text_run_presentation_layout(&prefix)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_numeric_prefix_objects(
        &prefix,
        writing_mode,
        next_column_moves_right,
    );
    let cent_prefix =
        observe_native_published_jlreq_numeric_abbreviation_fixture(writing_mode, "cent-prefix");
    assert_native_rich_text_layer_image_has_content(&cent_prefix);
    assert_eq!(
        first_text_run_presentation_layout(&cent_prefix)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_numeric_cent_prefix_objects(
        &cent_prefix,
        writing_mode,
        next_column_moves_right,
    );
    let ideographic_prefix = observe_native_published_jlreq_numeric_abbreviation_fixture(
        writing_mode,
        "prefix-ideographic",
    );
    assert_native_rich_text_layer_image_has_content(&ideographic_prefix);
    assert_eq!(
        first_text_run_presentation_layout(&ideographic_prefix)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_numeric_ideographic_prefix_objects(
        &ideographic_prefix,
        writing_mode,
        next_column_moves_right,
    );

    let suffix = observe_native_published_jlreq_numeric_abbreviation_fixture(writing_mode, "%");
    assert_native_rich_text_layer_image_has_content(&suffix);
    assert_eq!(
        first_text_run_presentation_layout(&suffix)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_numeric_suffix_objects(
        &suffix,
        writing_mode,
        next_column_moves_right,
    );
    let temperature_suffix = observe_native_published_jlreq_numeric_abbreviation_fixture(
        writing_mode,
        "temperature-suffix",
    );
    assert_native_rich_text_layer_image_has_content(&temperature_suffix);
    assert_eq!(
        first_text_run_presentation_layout(&temperature_suffix)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_numeric_temperature_suffix_objects(
        &temperature_suffix,
        writing_mode,
        next_column_moves_right,
    );
    let decomposed_temperature_suffix = observe_native_published_jlreq_numeric_abbreviation_fixture(
        writing_mode,
        "temperature-suffix-decomposed",
    );
    assert_native_rich_text_layer_image_has_content(&decomposed_temperature_suffix);
    assert_eq!(
        first_text_run_presentation_layout(&decomposed_temperature_suffix)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_numeric_decomposed_temperature_suffix_objects(
        &decomposed_temperature_suffix,
        writing_mode,
        next_column_moves_right,
    );
    let ideographic_suffix = observe_native_published_jlreq_numeric_abbreviation_fixture(
        writing_mode,
        "suffix-ideographic",
    );
    assert_native_rich_text_layer_image_has_content(&ideographic_suffix);
    assert_eq!(
        first_text_run_presentation_layout(&ideographic_suffix)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_numeric_ideographic_suffix_objects(
        &ideographic_suffix,
        writing_mode,
        next_column_moves_right,
    );
}

fn observe_native_published_jlreq_numeric_abbreviation_fixture(
    writing_mode: &str,
    label: &str,
) -> serde_json::Value {
    let text = native_published_jlreq_numeric_abbreviation_text(label);
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-numeric-abbreviation-{label}"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp published JLREQ numeric abbreviation source");
    json
}

fn assert_native_published_jlreq_numeric_abbreviation_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    mark: &str,
    label: &str,
    object_id: &str,
    capture_kind: &str,
) {
    let text = native_published_jlreq_numeric_abbreviation_text(label);
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-numeric-abbreviation-{label}-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-numeric-abbreviation-{label}-{capture_kind}.rgba"
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
        .arg(object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ numeric abbreviation raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ numeric abbreviation {label} {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ numeric abbreviation report is JSON");
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

    let target = assert_native_published_jlreq_numeric_abbreviation_target(
        &json,
        writing_mode,
        next_column_moves_right,
        mark,
        label,
    );
    assert_native_published_jlreq_numeric_abbreviation_crop_pixels(
        &json,
        target,
        &raw_path,
        writing_mode,
        label,
        capture_kind,
    );

    fs::remove_file(&path).expect("remove temp published JLREQ numeric abbreviation source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ numeric abbreviation dir");
}

fn assert_native_published_jlreq_numeric_abbreviation_target<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
    mark: &str,
    label: &str,
) -> &'report serde_json::Value {
    if mark == "$" {
        if label == "prefix-ideographic" {
            assert_native_published_jlreq_numeric_ideographic_prefix_objects(
                json,
                writing_mode,
                next_column_moves_right,
            )
        } else {
            assert_native_published_jlreq_numeric_prefix_objects(
                json,
                writing_mode,
                next_column_moves_right,
            )
        }
    } else if label == "cent-prefix" {
        assert_native_published_jlreq_numeric_cent_prefix_objects(
            json,
            writing_mode,
            next_column_moves_right,
        )
    } else if label == "temperature-suffix" {
        assert_native_published_jlreq_numeric_temperature_suffix_objects(
            json,
            writing_mode,
            next_column_moves_right,
        )
    } else if label == "temperature-suffix-decomposed" {
        assert_native_published_jlreq_numeric_decomposed_temperature_suffix_target(
            json,
            writing_mode,
            next_column_moves_right,
            mark,
        )
    } else if label == "suffix-ideographic" {
        assert_native_published_jlreq_numeric_ideographic_suffix_objects(
            json,
            writing_mode,
            next_column_moves_right,
        )
    } else {
        assert_native_published_jlreq_numeric_suffix_objects(
            json,
            writing_mode,
            next_column_moves_right,
        )
    }
}

fn assert_native_published_jlreq_numeric_abbreviation_crop_pixels(
    json: &serde_json::Value,
    target: &serde_json::Value,
    raw_path: &Path,
    writing_mode: &str,
    label: &str,
    capture_kind: &str,
) {
    assert_eq!(json["images"][0]["crop_origin"]["x"], target["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], target["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], target["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], target["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            raw_path,
            agent_object_id_color_from_json(target),
            content_pixels,
            &format!("{writing_mode} published JLREQ numeric abbreviation {label} object-id crop"),
        );
    } else {
        let bytes =
            fs::read(raw_path).expect("read native published JLREQ numeric abbreviation crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }
}

fn native_published_jlreq_numeric_abbreviation_text(label: &str) -> &'static str {
    match label {
        "$" | "prefix" => "天$123人",
        "cent-prefix" => "天¢123人",
        "%" | "suffix" => "天50%人",
        "temperature-suffix" => "天25℃人",
        "temperature-suffix-decomposed" => "天25°C人",
        "prefix-ideographic" => "天$五人",
        "suffix-ideographic" => "天五%人",
        _ => panic!("unknown native published JLREQ numeric abbreviation label {label}"),
    }
}

fn assert_native_published_jlreq_numeric_prefix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let prefix = find_rich_text_cluster_object(json, "$", 3, 4);
    let digits = find_rich_text_cluster_object(json, "123", 4, 7);
    let next_body = find_rich_text_cluster_object(json, "人", 7, 10);
    assert_vertical_cluster_after(
        body,
        prefix,
        "published JLREQ numeric prefix abbreviation should start after body text",
    );
    assert_vertical_cluster_after(
        prefix,
        digits,
        "published JLREQ digits should stay with the numeric prefix abbreviation",
    );
    assert_next_paragraph_column(
        digits,
        next_body,
        next_column_moves_right,
        "body text after the prefixed European numeral should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        prefix,
        &format!("{writing_mode} published JLREQ numeric prefix abbreviation"),
    );
    prefix
}

fn assert_native_published_jlreq_numeric_cent_prefix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let prefix = find_rich_text_cluster_object(json, "¢", 3, 5);
    let digits = find_rich_text_cluster_object(json, "123", 5, 8);
    let next_body = find_rich_text_cluster_object(json, "人", 8, 11);
    assert_vertical_cluster_after(
        body,
        prefix,
        "published JLREQ cent prefix abbreviation should start after body text",
    );
    assert_vertical_cluster_after(
        prefix,
        digits,
        "published JLREQ digits should stay with the cent prefix abbreviation",
    );
    assert_next_paragraph_column(
        digits,
        next_body,
        next_column_moves_right,
        "body text after the cent-prefixed European numeral should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        prefix,
        &format!("{writing_mode} published JLREQ cent prefix abbreviation"),
    );
    prefix
}

fn assert_native_published_jlreq_numeric_ideographic_prefix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let prefix = find_rich_text_cluster_object(json, "$", 3, 4);
    let ideographic_numeral = find_rich_text_cluster_object(json, "五", 4, 7);
    let next_body = find_rich_text_cluster_object(json, "人", 7, 10);
    assert_vertical_cluster_after(
        body,
        prefix,
        "published JLREQ numeric prefix abbreviation should start after body text",
    );
    assert_vertical_cluster_after(
        prefix,
        ideographic_numeral,
        "published JLREQ ideographic numeral should stay with the numeric prefix abbreviation",
    );
    assert_next_paragraph_column(
        ideographic_numeral,
        next_body,
        next_column_moves_right,
        "body text after the prefixed ideographic numeral should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        prefix,
        &format!("{writing_mode} published JLREQ numeric ideographic prefix abbreviation"),
    );
    prefix
}

fn assert_native_published_jlreq_numeric_suffix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let digits = find_rich_text_cluster_object(json, "50", 3, 5);
    let suffix = find_rich_text_cluster_object(json, "%", 5, 6);
    let next_body = find_rich_text_cluster_object(json, "人", 6, 9);
    assert_vertical_cluster_after(
        body,
        digits,
        "published JLREQ postfixed European numeral should start after body text",
    );
    assert_vertical_cluster_after(
        digits,
        suffix,
        "published JLREQ numeric suffix abbreviation should stay with the preceding digits",
    );
    assert_next_paragraph_column(
        suffix,
        next_body,
        next_column_moves_right,
        "body text after the postfixed European numeral should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        suffix,
        &format!("{writing_mode} published JLREQ numeric suffix abbreviation"),
    );
    suffix
}

fn assert_native_published_jlreq_numeric_temperature_suffix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let digits = find_rich_text_cluster_object(json, "25", 3, 5);
    let suffix = find_rich_text_cluster_object(json, "℃", 5, 8);
    let next_body = find_rich_text_cluster_object(json, "人", 8, 11);
    assert_vertical_cluster_after(
        body,
        digits,
        "published JLREQ temperature-suffixed numeral should start after body text",
    );
    assert_vertical_cluster_after(
        digits,
        suffix,
        "published JLREQ temperature suffix abbreviation should stay with the preceding digits",
    );
    assert_next_paragraph_column(
        suffix,
        next_body,
        next_column_moves_right,
        "body text after the temperature-suffixed European numeral should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        suffix,
        &format!("{writing_mode} published JLREQ temperature suffix abbreviation"),
    );
    suffix
}

fn assert_native_published_jlreq_numeric_decomposed_temperature_suffix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    assert_native_published_jlreq_numeric_decomposed_temperature_suffix_target(
        json,
        writing_mode,
        next_column_moves_right,
        "C",
    )
}

fn assert_native_published_jlreq_numeric_decomposed_temperature_suffix_target<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
    mark: &str,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let digits = find_rich_text_cluster_object(json, "25", 3, 5);
    let degree = find_rich_text_cluster_object(json, "°", 5, 7);
    let unit = find_rich_text_cluster_object(json, "C", 7, 8);
    let next_body = find_rich_text_cluster_object(json, "人", 8, 11);
    assert_vertical_cluster_after(
        body,
        digits,
        "published JLREQ decomposed temperature unit should start after body text",
    );
    assert_vertical_cluster_after(
        digits,
        degree,
        "published JLREQ degree suffix should stay with the preceding digits",
    );
    assert_vertical_cluster_after(
        degree,
        unit,
        "published JLREQ Latin temperature unit should stay with the degree suffix",
    );
    assert_next_paragraph_column(
        unit,
        next_body,
        next_column_moves_right,
        "body text after the decomposed temperature unit should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        degree,
        &format!("{writing_mode} published JLREQ decomposed temperature degree suffix"),
    );
    assert_rich_text_object_has_mask_capture(
        unit,
        &format!("{writing_mode} published JLREQ decomposed temperature unit tail"),
    );
    match mark {
        "°" => degree,
        "C" => unit,
        other => panic!("unsupported decomposed temperature suffix crop target: {other}"),
    }
}

fn assert_native_published_jlreq_numeric_ideographic_suffix_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let ideographic_numeral = find_rich_text_cluster_object(json, "五", 3, 6);
    let suffix = find_rich_text_cluster_object(json, "%", 6, 7);
    let next_body = find_rich_text_cluster_object(json, "人", 7, 10);
    assert_vertical_cluster_after(
        body,
        ideographic_numeral,
        "published JLREQ postfixed ideographic numeral should start after body text",
    );
    assert_vertical_cluster_after(
        ideographic_numeral,
        suffix,
        "published JLREQ numeric suffix abbreviation should stay with the preceding ideographic numeral",
    );
    assert_next_paragraph_column(
        suffix,
        next_body,
        next_column_moves_right,
        "body text after the postfixed ideographic numeral should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        suffix,
        &format!("{writing_mode} published JLREQ numeric ideographic suffix abbreviation"),
    );
    suffix
}

fn assert_native_published_jlreq_reference_mark_geometry(writing_mode: &str) {
    let json = observe_native_published_jlreq_reference_mark_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_reference_mark_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_reference_mark_fixture(writing_mode: &str) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-published-jlreq-reference-mark"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]本¹²。人[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp published JLREQ reference mark source");
    json
}

fn assert_native_published_jlreq_reference_mark_raw_crop(writing_mode: &str, capture_kind: &str) {
    for target in native_published_jlreq_reference_mark_targets() {
        assert_native_published_jlreq_reference_mark_target_raw_crop(
            writing_mode,
            capture_kind,
            target,
        );
    }
}

fn assert_native_published_jlreq_reference_mark_target_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
    target: NativeReferenceMarkTarget,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-reference-mark-{}-{capture_kind}",
        target.label
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]本¹²。人[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-reference-mark-{}-{capture_kind}.rgba",
        target.label
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
        .arg(target.object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ reference mark raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ reference mark {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ reference mark report is JSON");
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

    let crop_target =
        assert_native_published_jlreq_reference_mark_target(&json, writing_mode, target);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        crop_target["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        crop_target["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], crop_target["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], crop_target["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(crop_target),
            content_pixels,
            &format!(
                "{writing_mode} published JLREQ reference mark {} object-id crop",
                target.description
            ),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native published JLREQ reference mark crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ reference mark source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ reference mark dir");
}

fn assert_native_published_jlreq_reference_mark_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let full_stop = native_published_jlreq_reference_mark_targets()[1];
    assert_native_published_jlreq_reference_mark_target(json, writing_mode, full_stop)
}

fn assert_native_published_jlreq_reference_mark_target<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    target: NativeReferenceMarkTarget,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "本", 0, 3);
    let first_mark = find_rich_text_cluster_object(json, "¹", 3, 5);
    let second_mark = find_rich_text_cluster_object(json, "²", 5, 7);
    let full_stop = find_rich_text_cluster_object(json, "。", 7, 10);
    let crop_target =
        find_rich_text_cluster_object(json, target.text, target.range.0, target.range.1);
    let next_body = find_rich_text_cluster_object(json, "人", 10, 13);
    assert_vertical_cluster_after(
        body,
        first_mark,
        "published JLREQ reference mark should stay with the preceding main-text cluster",
    );
    assert_vertical_cluster_after(
        first_mark,
        second_mark,
        "published JLREQ reference mark digits should stay together",
    );
    assert_eq!(
        agent_json_bbox_x(&second_mark["bbox"]),
        agent_json_bbox_x(&full_stop["bbox"]),
        "published JLREQ full stop after a reference mark should stay in the reference mark column"
    );
    assert!(
        agent_json_bbox_bottom(&full_stop["bbox"]) > agent_json_bbox_y(&second_mark["bbox"]),
        "published JLREQ full stop after a reference mark should remain attached to the reference mark column"
    );
    assert_next_paragraph_column(
        full_stop,
        next_body,
        writing_mode == "vertical_lr",
        "body text after the published JLREQ reference mark sequence should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        second_mark,
        &format!("{writing_mode} published JLREQ reference mark second digit"),
    );
    assert_rich_text_object_has_mask_capture(
        full_stop,
        &format!("{writing_mode} published JLREQ reference mark full stop"),
    );
    crop_target
}

#[derive(Clone, Copy)]
struct NativeReferenceMarkTarget {
    label: &'static str,
    text: &'static str,
    range: (u64, u64),
    object_id: &'static str,
    description: &'static str,
}

const fn native_published_jlreq_reference_mark_targets() -> [NativeReferenceMarkTarget; 2] {
    [
        NativeReferenceMarkTarget {
            label: "second-mark",
            text: "²",
            range: (5, 7),
            object_id: "object.dialogue.0.0.cluster.2.5.7",
            description: "second digit",
        },
        NativeReferenceMarkTarget {
            label: "full-stop",
            text: "。",
            range: (7, 10),
            object_id: "object.dialogue.0.0.cluster.3.7.10",
            description: "full stop",
        },
    ]
}

fn assert_native_published_jlreq_parenthesized_reference_mark_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let json = observe_native_published_jlreq_parenthesized_reference_mark_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_parenthesized_reference_mark_objects(
        &json,
        writing_mode,
        next_column_moves_right,
    );
}

fn observe_native_published_jlreq_parenthesized_reference_mark_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-parenthesized-reference-mark"
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]本⁽¹⁾。人[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path)
        .expect("remove temp published JLREQ parenthesized reference mark source");
    json
}

fn assert_native_published_jlreq_parenthesized_reference_mark_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
) {
    for target in native_published_jlreq_parenthesized_reference_mark_targets() {
        assert_native_published_jlreq_parenthesized_reference_mark_target_raw_crop(
            writing_mode,
            next_column_moves_right,
            capture_kind,
            target,
        );
    }
}

fn assert_native_published_jlreq_parenthesized_reference_mark_target_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
    target: NativeParenthesizedReferenceMarkTarget,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-parenthesized-reference-mark-{}-{capture_kind}",
        target.label
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]本⁽¹⁾。人[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-parenthesized-reference-mark-{}-{capture_kind}.rgba",
        target.label
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
        .arg(target.object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ parenthesized reference mark raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ parenthesized reference mark {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ parenthesized reference mark report is JSON");
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

    let crop_target = assert_native_published_jlreq_parenthesized_reference_mark_target(
        &json,
        writing_mode,
        next_column_moves_right,
        target,
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        crop_target["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        crop_target["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], crop_target["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], crop_target["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(crop_target),
            content_pixels,
            &format!(
                "{writing_mode} published JLREQ parenthesized reference mark {} object-id crop",
                target.description
            ),
        );
    } else {
        let bytes = fs::read(&raw_path)
            .expect("read native published JLREQ parenthesized reference mark crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path)
        .expect("remove temp published JLREQ parenthesized reference mark source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ parenthesized reference mark dir");
}

fn assert_native_published_jlreq_parenthesized_reference_mark_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    let full_stop = native_published_jlreq_parenthesized_reference_mark_targets()[1];
    assert_native_published_jlreq_parenthesized_reference_mark_target(
        json,
        writing_mode,
        next_column_moves_right,
        full_stop,
    )
}

fn assert_native_published_jlreq_parenthesized_reference_mark_target<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
    target: NativeParenthesizedReferenceMarkTarget,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "本", 0, 3);
    let open = find_rich_text_cluster_object(json, "⁽", 3, 6);
    let mark = find_rich_text_cluster_object(json, "¹", 6, 8);
    let close = find_rich_text_cluster_object(json, "⁾", 8, 11);
    let full_stop = find_rich_text_cluster_object(json, "。", 11, 14);
    let crop_target =
        find_rich_text_cluster_object(json, target.text, target.range.0, target.range.1);
    let next_body = find_rich_text_cluster_object(json, "人", 14, 17);
    assert_vertical_cluster_after(
        body,
        open,
        "published JLREQ parenthesized reference mark opening should stay with main text",
    );
    assert_vertical_cluster_after(
        open,
        mark,
        "published JLREQ parenthesized reference mark digit should stay with opening",
    );
    assert_vertical_cluster_after(
        mark,
        close,
        "published JLREQ parenthesized reference mark closing should stay with digit",
    );
    assert_eq!(
        close["bbox"]["x"], full_stop["bbox"]["x"],
        "published JLREQ full stop should stay in the parenthesized reference mark column"
    );
    assert!(
        agent_json_bboxes_intersect(&close["bbox"], &full_stop["bbox"]),
        "published JLREQ full stop should stay attached to the parenthesized reference mark bbox"
    );
    assert_next_paragraph_column(
        full_stop,
        next_body,
        next_column_moves_right,
        "body text after the parenthesized reference mark should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        close,
        &format!("{writing_mode} published JLREQ parenthesized reference mark closing"),
    );
    assert_rich_text_object_has_mask_capture(
        full_stop,
        &format!("{writing_mode} published JLREQ parenthesized reference mark full stop"),
    );
    crop_target
}

#[derive(Clone, Copy)]
struct NativeParenthesizedReferenceMarkTarget {
    label: &'static str,
    text: &'static str,
    range: (u64, u64),
    object_id: &'static str,
    description: &'static str,
}

const fn native_published_jlreq_parenthesized_reference_mark_targets()
-> [NativeParenthesizedReferenceMarkTarget; 2] {
    [
        NativeParenthesizedReferenceMarkTarget {
            label: "closing",
            text: "⁾",
            range: (8, 11),
            object_id: "object.dialogue.0.0.cluster.3.8.11",
            description: "closing",
        },
        NativeParenthesizedReferenceMarkTarget {
            label: "full-stop",
            text: "。",
            range: (11, 14),
            object_id: "object.dialogue.0.0.cluster.4.11.14",
            description: "full stop",
        },
    ]
}

fn assert_native_published_jlreq_latin_unit_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let json = observe_native_published_jlreq_latin_unit_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_latin_unit_objects(&json, writing_mode, next_column_moves_right);
}

fn observe_native_published_jlreq_latin_unit_fixture(writing_mode: &str) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-published-jlreq-latin-unit"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天kg人[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp published JLREQ Latin unit source");
    json
}

fn assert_native_published_jlreq_latin_unit_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
) {
    let fixture_name =
        format!("agent-observe-native-{writing_mode}-published-jlreq-latin-unit-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天kg人[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-latin-unit-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.2.4.5")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ Latin unit raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ Latin unit {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ Latin unit report is JSON");
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

    let unit_end = assert_native_published_jlreq_latin_unit_objects(
        &json,
        writing_mode,
        next_column_moves_right,
    );
    assert_eq!(json["images"][0]["crop_origin"]["x"], unit_end["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], unit_end["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], unit_end["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], unit_end["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(unit_end),
            content_pixels,
            &format!("{writing_mode} published JLREQ Latin unit object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native published JLREQ Latin unit crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ Latin unit source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ Latin unit dir");
}

fn assert_native_published_jlreq_latin_unit_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let unit_start = find_rich_text_cluster_object(json, "k", 3, 4);
    let unit_end = find_rich_text_cluster_object(json, "g", 4, 5);
    let next_body = find_rich_text_cluster_object(json, "人", 5, 8);
    assert_vertical_cluster_after(
        body,
        unit_start,
        "published JLREQ Latin unit should start after body text",
    );
    assert_vertical_cluster_after(
        unit_start,
        unit_end,
        "published JLREQ Latin unit letters should stay together",
    );
    assert_next_paragraph_column(
        unit_end,
        next_body,
        next_column_moves_right,
        "body text after the Latin unit symbol should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        unit_end,
        &format!("{writing_mode} published JLREQ Latin unit final letter"),
    );
    unit_end
}

fn assert_native_published_jlreq_western_word_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let json = observe_native_published_jlreq_western_word_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_western_word_objects(
        &json,
        writing_mode,
        next_column_moves_right,
    );
}

fn observe_native_published_jlreq_western_word_fixture(writing_mode: &str) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-published-jlreq-western-word"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天Web人[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp published JLREQ Western word source");
    json
}

fn assert_native_published_jlreq_western_word_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
) {
    let fixture_name =
        format!("agent-observe-native-{writing_mode}-published-jlreq-western-word-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天Web人[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-western-word-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.3.5.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ Western word raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ Western word {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ Western word report is JSON");
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

    let word_end = assert_native_published_jlreq_western_word_objects(
        &json,
        writing_mode,
        next_column_moves_right,
    );
    assert_eq!(json["images"][0]["crop_origin"]["x"], word_end["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], word_end["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], word_end["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], word_end["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(word_end),
            content_pixels,
            &format!("{writing_mode} published JLREQ Western word object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native published JLREQ Western word crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ Western word source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ Western word dir");
}

fn assert_native_published_jlreq_western_word_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let first = find_rich_text_cluster_object(json, "W", 3, 4);
    let second = find_rich_text_cluster_object(json, "e", 4, 5);
    let third = find_rich_text_cluster_object(json, "b", 5, 6);
    let next_body = find_rich_text_cluster_object(json, "人", 6, 9);
    assert_vertical_cluster_after(
        body,
        first,
        "published JLREQ Western word should start after body text",
    );
    assert_vertical_cluster_after(
        first,
        second,
        "published JLREQ Western word letters should stay together",
    );
    assert_vertical_cluster_after(
        second,
        third,
        "published JLREQ Western word final letter should stay attached",
    );
    assert_next_paragraph_column(
        third,
        next_body,
        next_column_moves_right,
        "body text after the Western word should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        third,
        &format!("{writing_mode} published JLREQ Western word final letter"),
    );
    third
}

fn assert_native_published_jlreq_numeric_unit_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    for case in native_published_jlreq_numeric_unit_cases() {
        let json = observe_native_published_jlreq_numeric_unit_fixture(writing_mode, case);
        assert_native_rich_text_layer_image_has_content(&json);
        assert_eq!(
            first_text_run_presentation_layout(&json)["writing_mode"],
            writing_mode
        );
        assert_native_published_jlreq_numeric_unit_objects(
            &json,
            writing_mode,
            next_column_moves_right,
            case,
        );
    }
}

fn observe_native_published_jlreq_numeric_unit_fixture(
    writing_mode: &str,
    case: NativeNumericUnitCase,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-numeric-unit-{}",
            case.label
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]{}[/][p]
}}
",
            case.text
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp published JLREQ numeric unit source");
    json
}

fn assert_native_published_jlreq_numeric_unit_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
) {
    for case in native_published_jlreq_numeric_unit_cases() {
        assert_native_published_jlreq_numeric_unit_case_raw_crop(
            writing_mode,
            next_column_moves_right,
            capture_kind,
            case,
        );
    }
}

fn assert_native_published_jlreq_numeric_unit_case_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
    case: NativeNumericUnitCase,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-numeric-unit-{}-{capture_kind}",
        case.label
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]{}[/][p]
}}
",
            case.text
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-numeric-unit-{}-{capture_kind}.rgba",
        case.label
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
        .arg(case.object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ numeric unit raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ numeric unit {} {capture_kind} crop should succeed, stderr: {}",
        case.label,
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ numeric unit report is JSON");
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

    let unit_end = assert_native_published_jlreq_numeric_unit_objects(
        &json,
        writing_mode,
        next_column_moves_right,
        case,
    );
    assert_native_published_jlreq_numeric_unit_crop_pixels(
        &json,
        unit_end,
        &raw_path,
        writing_mode,
        capture_kind,
        case,
    );

    fs::remove_file(&path).expect("remove temp published JLREQ numeric unit source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ numeric unit dir");
}

fn assert_native_published_jlreq_numeric_unit_crop_pixels(
    json: &serde_json::Value,
    target: &serde_json::Value,
    raw_path: &Path,
    writing_mode: &str,
    capture_kind: &str,
    case: NativeNumericUnitCase,
) {
    assert_eq!(json["images"][0]["crop_origin"]["x"], target["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], target["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], target["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], target["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            raw_path,
            agent_object_id_color_from_json(target),
            content_pixels,
            &format!(
                "{writing_mode} published JLREQ numeric unit {} object-id crop",
                case.label
            ),
        );
    } else {
        let bytes = fs::read(raw_path).expect("read native published JLREQ numeric unit crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }
}

fn assert_native_published_jlreq_numeric_unit_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
    case: NativeNumericUnitCase,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let digits = find_rich_text_cluster_object(json, "3", 3, 4);
    let unit_start = find_rich_text_cluster_object(
        json,
        case.unit_start,
        case.unit_start_range.0,
        case.unit_start_range.1,
    );
    let unit_end = find_rich_text_cluster_object(
        json,
        case.unit_end,
        case.unit_end_range.0,
        case.unit_end_range.1,
    );
    let next_body = find_rich_text_cluster_object(json, "人", case.next_range.0, case.next_range.1);
    assert_vertical_cluster_after(
        body,
        digits,
        "published JLREQ numeric unit should start after body text",
    );
    assert_vertical_cluster_after(
        digits,
        unit_start,
        "published JLREQ unit symbol should stay with the preceding digit",
    );
    assert_vertical_cluster_after(
        unit_start,
        unit_end,
        "published JLREQ numeric unit letters should stay together",
    );
    assert_next_paragraph_column(
        unit_end,
        next_body,
        next_column_moves_right,
        "body text after the numeric unit symbol should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        unit_end,
        &format!("{writing_mode} published JLREQ numeric unit final letter"),
    );
    unit_end
}

#[derive(Clone, Copy)]
struct NativeNumericUnitCase {
    label: &'static str,
    text: &'static str,
    unit_start: &'static str,
    unit_start_range: (u64, u64),
    unit_end: &'static str,
    unit_end_range: (u64, u64),
    object_id: &'static str,
    next_range: (u64, u64),
}

const fn native_published_jlreq_numeric_unit_cases() -> [NativeNumericUnitCase; 2] {
    [
        NativeNumericUnitCase {
            label: "latin",
            text: "天3kg人",
            unit_start: "k",
            unit_start_range: (4, 5),
            unit_end: "g",
            unit_end_range: (5, 6),
            object_id: "object.dialogue.0.0.cluster.3.5.6",
            next_range: (6, 9),
        },
        NativeNumericUnitCase {
            label: "greek-latin",
            text: "天3μm人",
            unit_start: "μ",
            unit_start_range: (4, 6),
            unit_end: "m",
            unit_end_range: (6, 7),
            object_id: "object.dialogue.0.0.cluster.3.6.7",
            next_range: (7, 10),
        },
    ]
}

fn assert_native_published_jlreq_hyphenated_western_word_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let json = observe_native_published_jlreq_hyphenated_western_word_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_hyphenated_western_word_objects(
        &json,
        writing_mode,
        next_column_moves_right,
    );
}

fn observe_native_published_jlreq_hyphenated_western_word_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-published-jlreq-hyphenated-western-word"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天Web-Test人[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 900,
    );
    fs::remove_file(&path).expect("remove temp published JLREQ hyphenated Western word source");
    json
}

fn assert_native_published_jlreq_hyphenated_western_word_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-hyphenated-western-word-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天Web-Test人[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-hyphenated-western-word-{capture_kind}.rgba"
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
        .arg("--viewport-height")
        .arg("900")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.4.6.7")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ hyphenated Western word raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ hyphenated Western word {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ hyphenated Western word report is JSON");
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

    let hyphen = assert_native_published_jlreq_hyphenated_western_word_objects(
        &json,
        writing_mode,
        next_column_moves_right,
    );
    assert_eq!(json["images"][0]["crop_origin"]["x"], hyphen["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], hyphen["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], hyphen["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], hyphen["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(hyphen),
            content_pixels,
            &format!("{writing_mode} published JLREQ hyphenated Western word object-id crop"),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native published JLREQ hyphenated Western word crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ hyphenated Western word source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ hyphenated Western word dir");
}

fn assert_native_published_jlreq_hyphenated_western_word_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let first = find_rich_text_cluster_object(json, "W", 3, 4);
    let second = find_rich_text_cluster_object(json, "e", 4, 5);
    let before_hyphen = find_rich_text_cluster_object(json, "b", 5, 6);
    let hyphen = find_rich_text_cluster_object(json, "-", 6, 7);
    let after_hyphen = find_rich_text_cluster_object(json, "T", 7, 8);
    let after_hyphen_second = find_rich_text_cluster_object(json, "e", 8, 9);
    let after_hyphen_third = find_rich_text_cluster_object(json, "s", 9, 10);
    let last = find_rich_text_cluster_object(json, "t", 10, 11);
    let next_body = find_rich_text_cluster_object(json, "人", 11, 14);
    assert_vertical_cluster_after(
        body,
        first,
        "published JLREQ hyphenated Western word should start after body text",
    );
    assert_vertical_cluster_after(
        first,
        second,
        "published JLREQ hyphenated Western word first letters should stay together",
    );
    assert_vertical_cluster_after(
        second,
        before_hyphen,
        "published JLREQ hyphenated Western word letters before hyphen should stay together",
    );
    assert_vertical_cluster_after(
        before_hyphen,
        hyphen,
        "published JLREQ word-internal hyphen should stay with preceding letters",
    );
    assert_vertical_cluster_after(
        hyphen,
        after_hyphen,
        "published JLREQ letters after a word-internal hyphen should stay attached",
    );
    assert_vertical_cluster_after(
        after_hyphen,
        after_hyphen_second,
        "published JLREQ letters after a word-internal hyphen should stay together",
    );
    assert_vertical_cluster_after(
        after_hyphen_second,
        after_hyphen_third,
        "published JLREQ letters after a word-internal hyphen should stay together",
    );
    assert_vertical_cluster_after(
        after_hyphen_third,
        last,
        "published JLREQ letters after a word-internal hyphen should stay together",
    );
    assert_next_paragraph_column(
        last,
        next_body,
        next_column_moves_right,
        "body text after the hyphenated Western word should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        hyphen,
        &format!("{writing_mode} published JLREQ hyphenated Western word hyphen"),
    );
    hyphen
}

fn assert_native_published_jlreq_apostrophe_western_word_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    for case in native_published_jlreq_apostrophe_western_word_cases() {
        let json =
            observe_native_published_jlreq_apostrophe_western_word_fixture(writing_mode, case);
        assert_native_rich_text_layer_image_has_content(&json);
        assert_eq!(
            first_text_run_presentation_layout(&json)["writing_mode"],
            writing_mode
        );
        assert_native_published_jlreq_apostrophe_western_word_objects(
            &json,
            writing_mode,
            next_column_moves_right,
            case,
        );
    }
}

fn observe_native_published_jlreq_apostrophe_western_word_fixture(
    writing_mode: &str,
    case: NativeApostropheWesternWordCase,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-apostrophe-western-word-{}",
            case.label
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]{}[/][p]
}}
",
            case.text
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp published JLREQ apostrophe Western word source");
    json
}

fn assert_native_published_jlreq_apostrophe_western_word_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
) {
    for case in native_published_jlreq_apostrophe_western_word_cases() {
        assert_native_published_jlreq_apostrophe_western_word_case_raw_crop(
            writing_mode,
            next_column_moves_right,
            capture_kind,
            case,
        );
    }
}

fn assert_native_published_jlreq_apostrophe_western_word_case_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
    case: NativeApostropheWesternWordCase,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-apostrophe-western-word-{}-{capture_kind}",
        case.label
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]{}[/][p]
}}
",
            case.text
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-apostrophe-western-word-{}-{capture_kind}.rgba",
        case.label
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
        .arg(case.object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect(
            "arcw agent observe writes native published JLREQ apostrophe Western word raw crop",
        );

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ apostrophe Western word {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ apostrophe Western word report is JSON");
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

    let apostrophe = assert_native_published_jlreq_apostrophe_western_word_objects(
        &json,
        writing_mode,
        next_column_moves_right,
        case,
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        apostrophe["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        apostrophe["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], apostrophe["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], apostrophe["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(apostrophe),
            content_pixels,
            &format!("{writing_mode} published JLREQ apostrophe Western word object-id crop"),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native published JLREQ apostrophe Western word crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ apostrophe Western word source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ apostrophe Western word dir");
}

fn assert_native_published_jlreq_apostrophe_western_word_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
    case: NativeApostropheWesternWordCase,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let first = find_rich_text_cluster_object(json, "O", 3, 4);
    let apostrophe = find_rich_text_cluster_object(
        json,
        case.apostrophe,
        case.apostrophe_start,
        case.apostrophe_end,
    );
    let after_apostrophe =
        find_rich_text_cluster_object(json, "K", case.after_start, case.after_end);
    let next_body = find_rich_text_cluster_object(json, "人", case.next_start, case.next_end);
    assert_vertical_cluster_after(
        body,
        first,
        "published JLREQ apostrophe Western word should start after body text",
    );
    assert_vertical_cluster_after(
        first,
        apostrophe,
        "published JLREQ word-internal apostrophe should stay with preceding letters",
    );
    assert_vertical_cluster_after(
        apostrophe,
        after_apostrophe,
        "published JLREQ letters after a word-internal apostrophe should stay attached",
    );
    assert_next_paragraph_column(
        after_apostrophe,
        next_body,
        next_column_moves_right,
        "body text after the apostrophe Western word should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        apostrophe,
        &format!("{writing_mode} published JLREQ apostrophe Western word apostrophe"),
    );
    apostrophe
}

#[derive(Clone, Copy)]
struct NativeApostropheWesternWordCase {
    label: &'static str,
    text: &'static str,
    apostrophe: &'static str,
    object_id: &'static str,
    apostrophe_start: u64,
    apostrophe_end: u64,
    after_start: u64,
    after_end: u64,
    next_start: u64,
    next_end: u64,
}

const fn native_published_jlreq_apostrophe_western_word_cases()
-> [NativeApostropheWesternWordCase; 2] {
    [
        NativeApostropheWesternWordCase {
            label: "ascii",
            text: "天O'K人",
            apostrophe: "'",
            object_id: "object.dialogue.0.0.cluster.2.4.5",
            apostrophe_start: 4,
            apostrophe_end: 5,
            after_start: 5,
            after_end: 6,
            next_start: 6,
            next_end: 9,
        },
        NativeApostropheWesternWordCase {
            label: "typographic",
            text: "天O’K人",
            apostrophe: "’",
            object_id: "object.dialogue.0.0.cluster.2.4.7",
            apostrophe_start: 4,
            apostrophe_end: 7,
            after_start: 7,
            after_end: 8,
            next_start: 8,
            next_end: 11,
        },
    ]
}

fn assert_native_published_jlreq_accented_latin_word_geometry(writing_mode: &str) {
    let json = observe_native_published_jlreq_accented_latin_word_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_accented_latin_word_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_accented_latin_word_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-published-jlreq-accented-latin-word"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天café人[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report_with_viewport(
        &path, 1280, 720,
    );
    fs::remove_file(&path).expect("remove temp published JLREQ accented Latin word source");
    json
}

fn assert_native_published_jlreq_accented_latin_word_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-accented-latin-word-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天café人[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-accented-latin-word-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.4.6.8")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ accented Latin word raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ accented Latin word {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ accented Latin word report is JSON");
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

    let accented = assert_native_published_jlreq_accented_latin_word_objects(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], accented["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], accented["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], accented["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], accented["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(accented),
            content_pixels,
            &format!("{writing_mode} published JLREQ accented Latin word object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native published JLREQ accented Latin crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ accented Latin word source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ accented Latin word dir");
}

fn assert_native_published_jlreq_accented_latin_word_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let first = find_rich_text_cluster_object(json, "c", 3, 4);
    let second = find_rich_text_cluster_object(json, "a", 4, 5);
    let before_accent = find_rich_text_cluster_object(json, "f", 5, 6);
    let accented = find_rich_text_cluster_object(json, "é", 6, 8);
    let next_body = find_rich_text_cluster_object(json, "人", 8, 11);
    assert_vertical_cluster_after(
        body,
        first,
        "published JLREQ accented Latin word should start after body text",
    );
    assert_vertical_cluster_after(
        first,
        second,
        "published JLREQ accented Latin word letters should stay together",
    );
    assert_vertical_cluster_after(
        second,
        before_accent,
        "published JLREQ accented Latin word letters should stay together",
    );
    assert_vertical_cluster_after(
        before_accent,
        accented,
        "published JLREQ accented Latin grapheme should stay with preceding Latin letters",
    );
    assert_next_paragraph_column(
        accented,
        next_body,
        writing_mode == "vertical_lr",
        "body text after the accented Latin word should continue after the word",
    );
    assert_rich_text_object_has_mask_capture(
        accented,
        &format!("{writing_mode} published JLREQ accented Latin grapheme"),
    );
    accented
}

fn assert_native_published_jlreq_greek_latin_unit_geometry(writing_mode: &str) {
    let json = observe_native_published_jlreq_greek_latin_unit_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_published_jlreq_greek_latin_unit_objects(&json, writing_mode);
}

fn observe_native_published_jlreq_greek_latin_unit_fixture(
    writing_mode: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-published-jlreq-greek-latin-unit"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天μm人[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp published JLREQ Greek+Latin unit source");
    json
}

fn assert_native_published_jlreq_greek_latin_unit_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-greek-latin-unit-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天μm人[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-greek-latin-unit-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.2.5.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ Greek+Latin unit raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ Greek+Latin unit {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ Greek+Latin unit report is JSON");
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

    let latin_unit = assert_native_published_jlreq_greek_latin_unit_objects(&json, writing_mode);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        latin_unit["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        latin_unit["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], latin_unit["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], latin_unit["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(latin_unit),
            content_pixels,
            &format!("{writing_mode} published JLREQ Greek+Latin unit object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native published JLREQ Greek+Latin crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ Greek+Latin unit source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ Greek+Latin unit dir");
}

fn assert_native_published_jlreq_greek_latin_unit_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let greek_unit = find_rich_text_cluster_object(json, "μ", 3, 5);
    let latin_unit = find_rich_text_cluster_object(json, "m", 5, 6);
    let next_body = find_rich_text_cluster_object(json, "人", 6, 9);
    assert_vertical_cluster_after(
        body,
        greek_unit,
        "published JLREQ Greek+Latin unit should start after body text",
    );
    assert_vertical_cluster_after(
        greek_unit,
        latin_unit,
        "published JLREQ Latin unit suffix should stay attached to Greek unit symbol",
    );
    assert_next_paragraph_column(
        latin_unit,
        next_body,
        writing_mode == "vertical_lr",
        "body text after the Greek+Latin unit should continue after the unit",
    );
    assert_rich_text_object_has_mask_capture(
        latin_unit,
        &format!("{writing_mode} published JLREQ Greek+Latin unit suffix"),
    );
    latin_unit
}

fn assert_native_published_jlreq_subscript_object_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    for case in native_published_jlreq_subscript_object_cases() {
        let json = observe_native_published_jlreq_subscript_object_fixture(writing_mode, case);
        assert_native_rich_text_layer_image_has_content(&json);
        assert_eq!(
            first_text_run_presentation_layout(&json)["writing_mode"],
            writing_mode
        );
        assert_native_published_jlreq_subscript_object_objects(
            &json,
            writing_mode,
            next_column_moves_right,
            case,
        );
    }
}

fn observe_native_published_jlreq_subscript_object_fixture(
    writing_mode: &str,
    case: NativeSubscriptObjectCase,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!(
            "agent-observe-native-{writing_mode}-published-jlreq-subscript-object-{}",
            case.label
        ),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]{}[/][p]
}}
",
            case.text
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp published JLREQ subscript object source");
    json
}

fn assert_native_published_jlreq_subscript_object_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
) {
    for case in native_published_jlreq_subscript_object_cases() {
        assert_native_published_jlreq_subscript_object_case_raw_crop(
            writing_mode,
            next_column_moves_right,
            capture_kind,
            case,
        );
    }
}

fn assert_native_published_jlreq_subscript_object_case_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
    case: NativeSubscriptObjectCase,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-published-jlreq-subscript-object-{}-{capture_kind}",
        case.label
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]{}[/][p]
}}
",
            case.text
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-published-jlreq-subscript-object-{}-{capture_kind}.rgba",
        case.label
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
        .arg(case.object_id)
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native published JLREQ subscript object raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} published JLREQ subscript object {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native published JLREQ subscript object report is JSON");
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

    let following_base = assert_native_published_jlreq_subscript_object_objects(
        &json,
        writing_mode,
        next_column_moves_right,
        case,
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        following_base["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        following_base["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], following_base["bbox"]["width"]);
    assert_eq!(
        json["images"][0]["height"],
        following_base["bbox"]["height"]
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(following_base),
            content_pixels,
            &format!("{writing_mode} published JLREQ subscript object object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native published JLREQ subscript object crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp published JLREQ subscript object source");
    fs::remove_dir_all(&dir).expect("remove temp published JLREQ subscript object dir");
}

fn assert_native_published_jlreq_subscript_object_objects<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
    case: NativeSubscriptObjectCase,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let base = find_rich_text_cluster_object(json, case.base, case.base_range.0, case.base_range.1);
    let mark = find_rich_text_cluster_object(json, case.mark, case.mark_range.0, case.mark_range.1);
    let following_base = find_rich_text_cluster_object(
        json,
        case.following_base,
        case.following_base_range.0,
        case.following_base_range.1,
    );
    let next_body = find_rich_text_cluster_object(json, "人", case.next_range.0, case.next_range.1);
    assert_vertical_cluster_after(
        body,
        base,
        "published JLREQ sub/superscript object should start after body text",
    );
    assert_vertical_cluster_after(
        base,
        mark,
        "published JLREQ sub/superscript mark should stay with the preceding base character",
    );
    assert_vertical_cluster_after(
        mark,
        following_base,
        "published JLREQ following base should stay attached to the sub/superscript object",
    );
    assert_next_paragraph_column(
        following_base,
        next_body,
        next_column_moves_right,
        "body text after the sub/superscript object should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(
        following_base,
        &format!("{writing_mode} published JLREQ sub/superscript object following base"),
    );
    following_base
}

#[derive(Clone, Copy)]
struct NativeSubscriptObjectCase {
    label: &'static str,
    text: &'static str,
    base: &'static str,
    base_range: (u64, u64),
    mark: &'static str,
    mark_range: (u64, u64),
    following_base: &'static str,
    following_base_range: (u64, u64),
    object_id: &'static str,
    next_range: (u64, u64),
}

const fn native_published_jlreq_subscript_object_cases() -> [NativeSubscriptObjectCase; 3] {
    [
        NativeSubscriptObjectCase {
            label: "ascii",
            text: "天H₂O人",
            base: "H",
            base_range: (3, 4),
            mark: "₂",
            mark_range: (4, 7),
            following_base: "O",
            following_base_range: (7, 8),
            object_id: "object.dialogue.0.0.cluster.3.7.8",
            next_range: (8, 11),
        },
        NativeSubscriptObjectCase {
            label: "greek",
            text: "天α₂β人",
            base: "α",
            base_range: (3, 5),
            mark: "₂",
            mark_range: (5, 8),
            following_base: "β",
            following_base_range: (8, 10),
            object_id: "object.dialogue.0.0.cluster.3.8.10",
            next_range: (10, 13),
        },
        NativeSubscriptObjectCase {
            label: "greek-superscript",
            text: "天α²β人",
            base: "α",
            base_range: (3, 5),
            mark: "²",
            mark_range: (5, 7),
            following_base: "β",
            following_base_range: (7, 9),
            object_id: "object.dialogue.0.0.cluster.3.7.9",
            next_range: (9, 12),
        },
    ]
}

fn assert_native_strict_jlreq_ruby_text_combine_geometry(writing_mode: &str, ruby_on_right: bool) {
    let json = observe_native_strict_jlreq_ruby_text_combine_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_strict_jlreq_ruby_text_combine_objects(&json, ruby_on_right);
}

fn observe_native_strict_jlreq_ruby_text_combine_fixture(writing_mode: &str) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-strict-jlreq-ruby-text-combine"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]|[夢](ゆめ)2026。「人山川海[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp strict JLREQ ruby/text-combine source");
    json
}

fn assert_native_strict_jlreq_ruby_text_combine_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-strict-jlreq-ruby-text-combine-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]|[夢](ゆめ)2026。「人山川海[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-strict-jlreq-ruby-text-combine-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.1.3.7")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native strict JLREQ ruby/text-combine raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} strict JLREQ ruby/text-combine {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native strict JLREQ ruby/text-combine report is JSON");
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

    let text_combine =
        assert_native_strict_jlreq_ruby_text_combine_objects(&json, writing_mode == "vertical_rl");
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
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(text_combine),
            content_pixels,
            &format!("{writing_mode} strict JLREQ ruby/text-combine object-id crop"),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native strict JLREQ ruby/text-combine mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp strict JLREQ ruby/text-combine source");
    fs::remove_dir_all(&dir).expect("remove temp strict JLREQ ruby/text-combine dir");
}

fn assert_native_strict_jlreq_ruby_text_combine_objects(
    json: &serde_json::Value,
    ruby_on_right: bool,
) -> &serde_json::Value {
    let ruby = find_rich_text_ruby_object(json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ゆめ");
    assert_rich_text_object_has_mask_capture(ruby, "strict JLREQ ruby/text-combine ruby object");
    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    if ruby_on_right {
        assert!(
            agent_json_bbox_center_x_twice(annotation) > agent_json_bbox_center_x_twice(base),
            "vertical_rl strict JLREQ ruby annotation should stay on the right side: {ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_center_x_twice(annotation) < agent_json_bbox_center_x_twice(base),
            "vertical_lr strict JLREQ ruby annotation should stay on the left side: {ruby}"
        );
    }

    let text_combine = find_rich_text_cluster_object(json, "2026", 3, 7);
    assert_eq!(
        text_combine["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(text_combine["rich_text_ref"]["vertical_form"], "none");
    assert_rich_text_object_has_mask_capture(
        text_combine,
        "strict JLREQ paragraph text-combine object",
    );

    let full_stop = find_rich_text_cluster_object(json, "。", 7, 10);
    let opening = find_rich_text_cluster_object(json, "「", 10, 13);
    let person = find_rich_text_cluster_object(json, "人", 13, 16);
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict ruby/text-combine paragraph keeps adjacent closing/opening punctuation together",
    );
    assert_vertical_cluster_after(
        opening,
        person,
        "strict ruby/text-combine opening punctuation keeps its following base",
    );
    text_combine
}

fn assert_native_strict_jlreq_hard_break_segment_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-strict-jlreq-hard-break-segment"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地。[r]「人外[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp strict JLREQ hard-break segment source");
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );
    assert_native_strict_jlreq_hard_break_segment_attached_opening(&json, next_column_moves_right);
}

fn assert_native_strict_jlreq_hard_break_segment_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
    next_column_moves_right: bool,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-strict-jlreq-hard-break-segment-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地。[r]「人外[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-strict-jlreq-hard-break-segment-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.3.10.13")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native strict JLREQ hard-break raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} strict JLREQ hard-break {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native strict JLREQ hard-break report is JSON");
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

    let opening = assert_native_strict_jlreq_hard_break_segment_attached_opening(
        &json,
        next_column_moves_right,
    );
    assert_eq!(json["images"][0]["crop_origin"]["x"], opening["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], opening["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], opening["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], opening["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(opening),
            content_pixels,
            &format!("{writing_mode} strict JLREQ hard-break object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native strict JLREQ hard-break mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp strict JLREQ hard-break source");
    fs::remove_dir_all(&dir).expect("remove temp strict JLREQ hard-break dir");
}

fn assert_native_strict_jlreq_hard_break_segment_attached_opening(
    json: &serde_json::Value,
    next_column_moves_right: bool,
) -> &serde_json::Value {
    let full_stop = find_rich_text_cluster_object(json, "。", 6, 9);
    let opening = find_rich_text_cluster_object(json, "「", 10, 13);
    let person = find_rich_text_cluster_object(json, "人", 13, 16);
    assert_next_paragraph_column(
        full_stop,
        opening,
        next_column_moves_right,
        "hard line break should reset the strict JLREQ paragraph segment",
    );
    assert_vertical_cluster_after(
        opening,
        person,
        "text after hard-break opening punctuation should stay in its new segment column",
    );
    assert_eq!(opening["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(
        opening["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    opening
}

fn assert_native_jlreq_paragraph_overview(json: &serde_json::Value) {
    assert_native_rich_text_layer_image_has_content(json);
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let run = find_rich_text_run_object(
        json,
        "天地春夏秋冬月火、山々人「川」あっいおーえ―中・外………終",
    );
    assert!(
        run["bbox"]["width"].as_u64().unwrap() >= 300
            && run["bbox"]["height"].as_u64().unwrap() >= 120,
        "published-style JLREQ paragraph fixture should span multiple vertical columns: {run}"
    );
    assert!(
        rich_text_cluster_column_count(json) >= 6,
        "JLREQ paragraph fixture should expose a multi-column native plan: {json}"
    );
}

fn assert_native_jlreq_paragraph_compression_and_iteration(
    json: &serde_json::Value,
    next_column_moves_right: bool,
) {
    let fire = find_rich_text_cluster_object(json, "火", 21, 24);
    let comma = find_rich_text_cluster_object(json, "、", 24, 27);
    let mountain = find_rich_text_cluster_object(json, "山", 27, 30);
    assert_vertical_cluster_after(fire, comma, "paragraph comma follows body text");
    assert_eq!(
        comma["bbox"]["x"], mountain["bbox"]["x"],
        "text after a compressed comma should remain in the same planned column"
    );
    assert_eq!(
        (agent_json_bbox_y(&mountain["bbox"]) - agent_json_bbox_y(&comma["bbox"])) * 2,
        agent_json_bbox_y(&comma["bbox"]) - agent_json_bbox_y(&fire["bbox"]),
        "paragraph comma compression should be visible in native cluster geometry"
    );

    let iteration = find_rich_text_cluster_object(json, "々", 30, 33);
    let person = find_rich_text_cluster_object(json, "人", 33, 36);
    assert_vertical_cluster_after(
        mountain,
        iteration,
        "iteration mark stays attached in paragraph context",
    );
    assert_next_paragraph_column(
        iteration,
        person,
        next_column_moves_right,
        "text after an overhanging iteration mark should continue in the next paragraph column",
    );
}

fn assert_native_jlreq_paragraph_grouping_and_leaders(
    json: &serde_json::Value,
    next_column_moves_right: bool,
) {
    let open = find_rich_text_cluster_object(json, "「", 36, 39);
    let river = find_rich_text_cluster_object(json, "川", 39, 42);
    let close = find_rich_text_cluster_object(json, "」", 42, 45);
    assert_vertical_cluster_after(
        open,
        river,
        "paragraph bracket base follows opening bracket",
    );
    assert_vertical_cluster_after(
        river,
        close,
        "paragraph closing bracket stays with its base",
    );

    let large_kana = find_rich_text_cluster_object(json, "あ", 45, 48);
    let small_kana = find_rich_text_cluster_object(json, "っ", 48, 51);
    let next_kana = find_rich_text_cluster_object(json, "い", 51, 54);
    assert_vertical_cluster_after(
        large_kana,
        small_kana,
        "small kana stays out of a column head in paragraph context",
    );
    assert_vertical_cluster_after(
        small_kana,
        next_kana,
        "text after small kana continues in the same paragraph column",
    );

    let syllable = find_rich_text_cluster_object(json, "え", 60, 63);
    let dash = find_rich_text_cluster_object(json, "―", 63, 66);
    let center = find_rich_text_cluster_object(json, "中", 66, 69);
    assert_vertical_cluster_after(
        syllable,
        dash,
        "paragraph dash mark stays with its preceding cluster",
    );
    assert_next_paragraph_column(
        dash,
        center,
        next_column_moves_right,
        "paragraph text after an overhanging dash-mark suffix should continue in the next column",
    );

    let middle_dot = find_rich_text_cluster_object(json, "・", 69, 72);
    let outside = find_rich_text_cluster_object(json, "外", 72, 75);
    assert_eq!(
        middle_dot["bbox"]["x"], outside["bbox"]["x"],
        "text after a middle dot should remain in the same paragraph column"
    );
    assert!(
        agent_json_bbox_y(&outside["bbox"]) > agent_json_bbox_y(&middle_dot["bbox"]),
        "middle-dot compression should still advance paragraph text downward"
    );

    let first_leader = find_rich_text_cluster_object(json, "…", 75, 78);
    let second_leader = find_rich_text_cluster_object(json, "…", 78, 81);
    let ending = find_rich_text_cluster_object(json, "終", 84, 87);
    assert_vertical_cluster_after(
        first_leader,
        second_leader,
        "repeated leaders stay together in paragraph context",
    );
    assert_next_paragraph_column(
        second_leader,
        ending,
        next_column_moves_right,
        "paragraph text after a partially clipped overhanging leader chain should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(first_leader, "paragraph leader cluster");
}

fn assert_next_paragraph_column(
    previous: &serde_json::Value,
    next: &serde_json::Value,
    next_column_moves_right: bool,
    context: &str,
) {
    if next_column_moves_right {
        assert!(
            agent_json_bbox_x(&next["bbox"]) > agent_json_bbox_x(&previous["bbox"]),
            "{context}: next column should advance rightward"
        );
    } else {
        assert!(
            agent_json_bbox_x(&next["bbox"]) < agent_json_bbox_x(&previous["bbox"]),
            "{context}: next column should advance leftward"
        );
    }
    assert!(
        agent_json_bbox_y(&next["bbox"]) < agent_json_bbox_y(&previous["bbox"]),
        "{context}: next column should restart near the column top"
    );
}

fn observe_native_jlreq_preset_fixture(strictness: &str, label: &str) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-jlreq-{label}"),
        &format!(
            r##"
entry cli @entry.main {{ goto @flow.main }}

pub character @character.alice Alice as alice {{
    default_voice = auto
    dialogue_style {{
        font = "MS Mincho"
        text_color = "#d9f2ff"
        text_size = 30
    }}
}}

flow @flow.main main {{
    alice: [.vertical_rl jlreq={strictness}][font "MS Mincho"]天地。」人山川海。『火水木[/font][/][p]
}}
"##
        ),
    );
    let entry =
        EntryRuntimeId::from_source_entity_body("entry.main").expect("test entry ID is valid");
    let json = observe_native_rich_text_layer_report_at_entry(&path, &entry);
    fs::remove_file(&path).expect("remove temp preset JLREQ source");
    json
}

fn rich_text_vertical_column_start_byte_offsets(report: &serde_json::Value) -> Vec<u64> {
    let mut clusters = report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .filter(|object| object["role"] == "rich_text_cluster")
        .collect::<Vec<_>>();
    clusters.sort_by_key(|object| {
        object["rich_text_ref"]["range"]["start"]
            .as_u64()
            .expect("rich-text cluster source range start is reported")
    });

    let mut previous_column = None;
    clusters
        .into_iter()
        .filter_map(|object| {
            let column = agent_json_bbox_x(&object["bbox"]);
            if previous_column.replace(column) == Some(column) {
                None
            } else {
                Some(
                    object["rich_text_ref"]["range"]["start"]
                        .as_u64()
                        .expect("rich-text cluster source range start is reported"),
                )
            }
        })
        .collect()
}

fn assert_native_jlreq_closing_opening_column_plan(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let loose = observe_native_jlreq_closing_opening_fixture(writing_mode, "loose");
    let strict = observe_native_jlreq_closing_opening_fixture(writing_mode, "strict");
    assert_native_rich_text_layer_image_has_content(&loose);
    assert_native_rich_text_layer_image_has_content(&strict);

    assert_eq!(
        first_text_run_presentation_layout(&loose)["jlreq_strictness"],
        "loose"
    );
    assert_eq!(
        first_text_run_presentation_layout(&strict)["jlreq_strictness"],
        "strict"
    );

    let loose_full_stop = find_rich_text_cluster_object(&loose, "。", 6, 9);
    let loose_open = find_rich_text_cluster_object(&loose, "「", 9, 12);
    assert_next_paragraph_column(
        loose_full_stop,
        loose_open,
        next_column_moves_right,
        "loose native paragraph plan may break between adjacent closing/opening punctuation",
    );

    assert_native_strict_jlreq_closing_opening_geometry(&strict, writing_mode);
}

fn observe_native_jlreq_closing_opening_fixture(
    writing_mode: &str,
    strictness: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-jlreq-closing-opening-{strictness}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq={strictness}]天地。「人山川海[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp closing/opening JLREQ source");
    json
}

fn assert_native_strict_jlreq_closing_opening_raw_crop(writing_mode: &str, capture_kind: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-strict-jlreq-closing-opening-{capture_kind}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地。「人山川海[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-{writing_mode}-strict-jlreq-closing-opening-{capture_kind}"
    ));
    let raw_path = dir.join(format!(
        "native-{writing_mode}-strict-jlreq-closing-opening-{capture_kind}.rgba"
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
        .expect("arcw agent observe writes native strict JLREQ closing/opening raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} strict JLREQ closing/opening {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native strict JLREQ report is JSON");
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

    let opening = assert_native_strict_jlreq_closing_opening_geometry(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], opening["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], opening["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], opening["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], opening["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(opening),
            content_pixels,
            &format!("{writing_mode} strict JLREQ closing/opening object-id crop"),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native strict JLREQ closing/opening mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp strict JLREQ closing/opening source");
    fs::remove_dir_all(&dir).expect("remove temp strict JLREQ closing/opening dir");
}

fn assert_native_strict_jlreq_closing_opening_geometry<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "strict"
    );
    assert_eq!(
        first_text_run_presentation_layout(json)["writing_mode"],
        writing_mode
    );
    let full_stop = find_rich_text_cluster_object(json, "。", 6, 9);
    let opening = find_rich_text_cluster_object(json, "「", 9, 12);
    assert_eq!(opening["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(
        opening["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    assert_vertical_cluster_after(
        full_stop,
        opening,
        "strict native paragraph plan should keep adjacent closing/opening punctuation together",
    );
    opening
}
