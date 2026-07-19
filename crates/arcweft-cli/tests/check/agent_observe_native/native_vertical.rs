#[test]
#[ignore = "tier 2 Agent observe resource matrix: slow multi-subprocess image/resource coverage"]
#[allow(clippy::too_many_lines)]
fn agent_observe_writes_layer_png_and_object_raw_images() {
    let path = temp_arcw(
        "agent-observe-image-capture",
        r##"
entry cli @entry.main { goto @flow.main }

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
    let dir = temp_dir("agent-observe-image-capture");
    let png_path = dir.join("dialogue.png");
    let object_id_path = dir.join("dialogue-object-id.png");
    let raw_path = dir.join("object.rgba");
    let mask_path = dir.join("object-mask.rgba");

    let png_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
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
        .expect("arcw agent observe writes layer PNG");

    assert!(
        png_output.status.success(),
        "agent observe PNG capture should succeed, stderr: {}",
        String::from_utf8_lossy(&png_output.stderr)
    );
    let png_bytes = fs::read(&png_path).expect("read captured PNG");
    assert_eq!(&png_bytes[..8], b"\x89PNG\r\n\x1a\n");
    let png_json: serde_json::Value =
        serde_json::from_slice(&png_output.stdout).expect("PNG capture report is JSON");
    assert_eq!(png_json["images"][0]["kind"], "color");
    assert_eq!(png_json["images"][0]["renderer"], "native");
    assert_eq!(png_json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(png_json["images"][0]["scope"]["id"], "dialogue");
    assert_eq!(
        png_json["images"][0]["composition"],
        "masked_framebuffer_crop"
    );
    assert_eq!(png_json["images"][0]["mime_type"], "image/png");
    assert_eq!(png_json["images"][0]["width"], 1166);
    assert_eq!(png_json["images"][0]["height"], 203);
    assert_eq!(png_json["images"][0]["crop_origin"]["space"], "viewport");
    assert_eq!(png_json["images"][0]["crop_origin"]["x"], 57);
    assert_eq!(png_json["images"][0]["crop_origin"]["y"], 460);
    assert!(png_json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        png_json["images"][0]["content_bbox"]["width"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(png_json["images"][0]["written"], "dialogue.png");
    assert!(
        png_json["images"][0]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/layer.dialogue.png"))
    );
    let layers = png_json["layers"]
        .as_array()
        .expect("observation reports layers");
    let dialogue_layer = layers
        .iter()
        .find(|layer| layer["id"] == "dialogue")
        .expect("dialogue layer is observed");
    assert_eq!(dialogue_layer["bbox"]["space"], "viewport");
    assert_eq!(dialogue_layer["bbox"]["x"], 57);
    assert_eq!(dialogue_layer["bbox"]["y"], 460);
    assert_eq!(dialogue_layer["bbox"]["width"], 1166);
    assert_eq!(dialogue_layer["bbox"]["height"], 203);
    assert!(
        dialogue_layer["capture_refs"]["captures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capture| capture["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/layer.dialogue.mask.rgba"))
                && capture["mime_type"] == "application/octet-stream")
    );

    let object_id_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--capture")
        .arg("object-id")
        .arg("--layer")
        .arg("dialogue")
        .arg("--out")
        .arg(&object_id_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes object-id PNG");

    assert!(
        object_id_output.status.success(),
        "agent observe object-id capture should succeed, stderr: {}",
        String::from_utf8_lossy(&object_id_output.stderr)
    );
    let object_id_bytes = fs::read(&object_id_path).expect("read captured object-id PNG");
    assert_eq!(&object_id_bytes[..8], b"\x89PNG\r\n\x1a\n");
    let object_id_json: serde_json::Value =
        serde_json::from_slice(&object_id_output.stdout).expect("object-id report is JSON");
    assert_eq!(object_id_json["images"][0]["kind"], "object_id");
    assert_eq!(object_id_json["images"][0]["mime_type"], "image/png");
    assert!(
        object_id_json["images"][0]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        object_id_json["images"][0]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/layer.dialogue.object-id.png"))
    );

    let image_resource_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue")
        .arg("--resource")
        .arg("image")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe returns image resource");

    assert!(
        image_resource_output.status.success(),
        "agent observe image resource should succeed, stderr: {}",
        String::from_utf8_lossy(&image_resource_output.stderr)
    );
    let image_resource: serde_json::Value = serde_json::from_slice(&image_resource_output.stdout)
        .expect("image resource output is JSON");
    assert_eq!(image_resource["kind"], "image");
    assert_eq!(image_resource["mime_type"], "image/png");
    assert_eq!(image_resource["body"]["body_kind"], "bytes_base64");
    assert_eq!(image_resource["body"]["body"]["encoding"], "base64");
    assert!(
        image_resource["body"]["body"]["data"]
            .as_str()
            .is_some_and(|data| data.starts_with("iVBORw0KGgo"))
    );

    let mcp_image_resource_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue")
        .arg("--resource")
        .arg("image")
        .arg("--mcp")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe returns MCP image resource");

    assert!(
        mcp_image_resource_output.status.success(),
        "agent observe MCP image resource should succeed, stderr: {}",
        String::from_utf8_lossy(&mcp_image_resource_output.stderr)
    );
    let mcp_image_resource: serde_json::Value =
        serde_json::from_slice(&mcp_image_resource_output.stdout)
            .expect("MCP image resource output is JSON");
    assert_eq!(mcp_image_resource["contents"][0]["mimeType"], "image/png");
    assert!(
        mcp_image_resource["contents"][0]["blob"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );

    let presentation_tree_resource_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--resource")
        .arg("presentation-tree")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe returns presentation tree resource");

    assert!(
        presentation_tree_resource_output.status.success(),
        "agent observe presentation tree resource should succeed, stderr: {}",
        String::from_utf8_lossy(&presentation_tree_resource_output.stderr)
    );
    let presentation_tree_resource: serde_json::Value =
        serde_json::from_slice(&presentation_tree_resource_output.stdout)
            .expect("presentation tree resource output is JSON");
    assert_eq!(presentation_tree_resource["kind"], "presentation_tree");
    assert_eq!(
        presentation_tree_resource["uri"],
        "arcweft://session/cli/frame/3/presentation-tree.json"
    );
    assert_eq!(
        presentation_tree_resource["body"]["body"]["root"],
        "presentation.root"
    );
    assert!(
        presentation_tree_resource["body"]["body"]["nodes"]
            .as_array()
            .expect("presentation tree nodes are returned")
            .iter()
            .any(|node| node["role"] == "rich_text_ruby")
    );

    let presentation_tree_read_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/3/presentation-tree.json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads presentation tree resource");

    assert!(
        presentation_tree_read_output.status.success(),
        "agent observe presentation tree read-uri should succeed, stderr: {}",
        String::from_utf8_lossy(&presentation_tree_read_output.stderr)
    );
    let presentation_tree_read: serde_json::Value =
        serde_json::from_slice(&presentation_tree_read_output.stdout)
            .expect("presentation tree read-uri output is JSON");
    assert_eq!(presentation_tree_read["kind"], "presentation_tree");
    assert_eq!(
        presentation_tree_read["body"]["body"]["root"],
        "presentation.root"
    );

    let presentation_tree_filtered_read_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/3/presentation-tree.json?rich_text_kind=ruby")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads filtered presentation tree resource");

    assert!(
        presentation_tree_filtered_read_output.status.success(),
        "agent observe filtered presentation tree read-uri should succeed, stderr: {}",
        String::from_utf8_lossy(&presentation_tree_filtered_read_output.stderr)
    );
    let presentation_tree_filtered_read: serde_json::Value =
        serde_json::from_slice(&presentation_tree_filtered_read_output.stdout)
            .expect("filtered presentation tree read-uri output is JSON");
    let filtered_nodes = presentation_tree_filtered_read["body"]["body"]["nodes"]
        .as_array()
        .expect("filtered presentation tree nodes are returned");
    assert!(
        filtered_nodes
            .iter()
            .any(|node| node["rich_text_kind"] == "ruby")
    );
    assert!(
        !filtered_nodes
            .iter()
            .any(|node| node["rich_text_kind"] == "text_object_proxy")
    );

    let presentation_tree_filtered_mcp_read_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/3/presentation-tree.json?rich_text_kind=ruby")
        .arg("--mcp")
        .arg("--mcp-format")
        .arg("read")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads filtered presentation tree as MCP resource");

    assert!(
        presentation_tree_filtered_mcp_read_output.status.success(),
        "agent observe filtered presentation tree MCP read should succeed, stderr: {}",
        String::from_utf8_lossy(&presentation_tree_filtered_mcp_read_output.stderr)
    );
    let presentation_tree_filtered_mcp_read: serde_json::Value =
        serde_json::from_slice(&presentation_tree_filtered_mcp_read_output.stdout)
            .expect("filtered presentation tree MCP read output is JSON");
    assert!(
        presentation_tree_filtered_mcp_read["contents"][0]["uri"]
            .as_str()
            .is_some_and(|uri| uri.starts_with("arcweft://moderated/")
                && uri
                    .rsplit_once('.')
                    .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("json")))
    );
    assert_eq!(
        presentation_tree_filtered_mcp_read["contents"][0]["mimeType"],
        "application/json"
    );
    let filtered_tree_mcp_text = presentation_tree_filtered_mcp_read["contents"][0]["text"]
        .as_str()
        .expect("filtered presentation tree MCP text content is present");
    let filtered_tree_mcp_json: serde_json::Value =
        serde_json::from_str(filtered_tree_mcp_text).expect("filtered tree MCP text is JSON");
    assert!(
        filtered_tree_mcp_json["nodes"]
            .as_array()
            .expect("MCP filtered tree nodes are returned")
            .iter()
            .any(|node| node["rich_text_kind"] == "ruby")
    );

    let mcp_resource_list_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--image")
        .arg("png")
        .arg("--capture")
        .arg("object-id")
        .arg("--layer")
        .arg("dialogue")
        .arg("--resource")
        .arg("all")
        .arg("--mcp")
        .arg("--mcp-format")
        .arg("list")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe returns MCP resource list");

    assert!(
        mcp_resource_list_output.status.success(),
        "agent observe MCP resource list should succeed, stderr: {}",
        String::from_utf8_lossy(&mcp_resource_list_output.stderr)
    );
    let mcp_resource_list: serde_json::Value =
        serde_json::from_slice(&mcp_resource_list_output.stdout)
            .expect("MCP resource list output is JSON");
    let resources = mcp_resource_list["resources"]
        .as_array()
        .expect("MCP resource list contains resources");
    assert!(!resources.is_empty());
    assert!(resources.iter().all(|resource| {
        let Some(uri) = resource["uri"].as_str() else {
            return false;
        };
        let Some(name) = resource["name"].as_str() else {
            return false;
        };
        uri.starts_with("arcweft://moderated/")
            && uri.rsplit('/').next() == Some(name)
            && matches!(
                resource["mimeType"].as_str(),
                Some("application/json" | "application/x-ndjson")
            )
    }));
    assert!(
        resources
            .iter()
            .any(|resource| resource["mimeType"] == "application/json")
    );
    assert!(
        resources
            .iter()
            .any(|resource| resource["mimeType"] == "application/x-ndjson")
    );

    let mcp_tool_image_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue")
        .arg("--resource")
        .arg("image")
        .arg("--mcp")
        .arg("--mcp-format")
        .arg("tool-result")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe returns MCP image tool result");

    assert!(
        mcp_tool_image_output.status.success(),
        "agent observe MCP image tool result should succeed, stderr: {}",
        String::from_utf8_lossy(&mcp_tool_image_output.stderr)
    );
    let mcp_tool_image: serde_json::Value = serde_json::from_slice(&mcp_tool_image_output.stdout)
        .expect("MCP image tool result output is JSON");
    assert_eq!(mcp_tool_image["isError"], false);
    assert_eq!(mcp_tool_image["content"][0]["type"], "text");
    let mcp_tool_image_metadata: serde_json::Value =
        serde_json::from_str(mcp_tool_image["content"][0]["text"].as_str().unwrap())
            .expect("image metadata content is JSON");
    assert_eq!(mcp_tool_image_metadata["image"]["width"], 1166);
    assert_eq!(mcp_tool_image_metadata["image"]["height"], 203);
    assert_eq!(mcp_tool_image_metadata["image"]["renderer"], "native");
    assert_eq!(mcp_tool_image_metadata["image"]["scope"]["kind"], "layer");
    assert!(
        mcp_tool_image_metadata["image"]["scope"]["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("layer."))
    );
    assert_eq!(
        mcp_tool_image_metadata["image"]["scope"]["id"],
        mcp_tool_image_metadata["image"]["selected_capture"]["scope"]["id"]
    );
    assert_eq!(
        mcp_tool_image_metadata["image"]["scope"]["id"],
        mcp_tool_image_metadata["image"]["selected_capture"]["source"]["id"]
    );
    assert_eq!(
        mcp_tool_image_metadata["image"]["composition"],
        "masked_framebuffer_crop"
    );
    assert_eq!(
        mcp_tool_image_metadata["image"]["crop_origin"]["space"],
        "viewport"
    );
    assert_eq!(mcp_tool_image_metadata["image"]["crop_origin"]["x"], 57);
    assert_eq!(mcp_tool_image_metadata["image"]["crop_origin"]["y"], 460);
    assert!(
        mcp_tool_image_metadata["image"]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        mcp_tool_image_metadata["image"]["content_bbox"]["width"]
            .as_u64()
            .is_some_and(|width| width > 0)
    );
    assert!(
        mcp_tool_image_metadata["image"]["content_bbox"]["height"]
            .as_u64()
            .is_some_and(|height| height > 0)
    );
    assert_eq!(mcp_tool_image["content"][1]["type"], "image");
    assert_eq!(mcp_tool_image["content"][1]["mimeType"], "image/png");
    assert!(
        mcp_tool_image["content"][1]["data"]
            .as_str()
            .is_some_and(|data| data.starts_with("iVBORw0KGgo"))
    );

    let read_mask_uri_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/3/object.object.dialogue.0.0.mask.rgba")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads object mask resource URI");

    assert!(
        read_mask_uri_output.status.success(),
        "agent observe read-uri mask should succeed, stderr: {}",
        String::from_utf8_lossy(&read_mask_uri_output.stderr)
    );
    let read_mask_resource: serde_json::Value =
        serde_json::from_slice(&read_mask_uri_output.stdout)
            .expect("read-uri mask resource output is JSON");
    assert_eq!(read_mask_resource["kind"], "image");
    assert_eq!(
        read_mask_resource["uri"],
        "arcweft://session/cli/frame/3/object.object.dialogue.0.0.mask.rgba"
    );
    assert_eq!(read_mask_resource["mime_type"], "application/octet-stream");
    assert_eq!(read_mask_resource["body"]["body"]["encoding"], "base64");
    assert!(
        read_mask_resource["body"]["body"]["data"]
            .as_str()
            .is_some_and(|data| !data.is_empty())
    );

    let read_object_id_uri_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/3/object.object.dialogue.0.0.object-id.rgba")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads object object-id resource URI");

    assert!(
        read_object_id_uri_output.status.success(),
        "agent observe read-uri object-id should succeed, stderr: {}",
        String::from_utf8_lossy(&read_object_id_uri_output.stderr)
    );
    let read_object_id_resource: serde_json::Value =
        serde_json::from_slice(&read_object_id_uri_output.stdout)
            .expect("read-uri object-id resource output is JSON");
    assert_eq!(read_object_id_resource["kind"], "image");
    assert_eq!(
        read_object_id_resource["uri"],
        "arcweft://session/cli/frame/3/object.object.dialogue.0.0.object-id.rgba"
    );
    assert_eq!(
        read_object_id_resource["mime_type"],
        "application/octet-stream"
    );
    assert_eq!(
        read_object_id_resource["body"]["body"]["encoding"],
        "base64"
    );
    assert!(
        read_object_id_resource["body"]["body"]["data"]
            .as_str()
            .is_some_and(|data| !data.is_empty())
    );

    let mcp_read_object_image_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/3/object.object.dialogue.0.0.png")
        .arg("--mcp")
        .arg("--mcp-format")
        .arg("tool-result")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads object PNG as MCP tool result");

    assert!(
        mcp_read_object_image_output.status.success(),
        "agent observe read-uri MCP image should succeed, stderr: {}",
        String::from_utf8_lossy(&mcp_read_object_image_output.stderr)
    );
    let mcp_read_object_image: serde_json::Value =
        serde_json::from_slice(&mcp_read_object_image_output.stdout)
            .expect("read-uri MCP image output is JSON");
    assert_eq!(mcp_read_object_image["content"][0]["type"], "text");
    let mcp_read_object_metadata: serde_json::Value = serde_json::from_str(
        mcp_read_object_image["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .expect("read-uri image metadata content is JSON");
    assert_eq!(mcp_read_object_metadata["image"]["width"], 1166);
    assert_eq!(mcp_read_object_metadata["image"]["height"], 203);
    assert!(
        mcp_read_object_metadata["image"]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(mcp_read_object_image["content"][1]["type"], "image");
    assert_eq!(mcp_read_object_image["content"][1]["mimeType"], "image/png");
    assert!(
        mcp_read_object_image["content"][1]["data"]
            .as_str()
            .is_some_and(|data| data.starts_with("iVBORw0KGgo"))
    );

    let raw_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
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
        .expect("arcw agent observe writes object raw RGBA");

    assert!(
        raw_output.status.success(),
        "agent observe raw capture should succeed, stderr: {}",
        String::from_utf8_lossy(&raw_output.stderr)
    );
    let raw_bytes = fs::read(&raw_path).expect("read captured raw RGBA");
    let raw_json: serde_json::Value =
        serde_json::from_slice(&raw_output.stdout).expect("raw capture report is JSON");
    let width = raw_json["images"][0]["width"]
        .as_u64()
        .expect("raw capture width is integer");
    let height = raw_json["images"][0]["height"]
        .as_u64()
        .expect("raw capture height is integer");
    assert_eq!(raw_json["images"][0]["kind"], "color");
    assert_eq!(
        raw_json["images"][0]["mime_type"],
        "application/octet-stream"
    );
    assert_eq!(raw_json["images"][0]["written"], "object.rgba");
    assert!(
        raw_json["images"][0]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/object.object.dialogue.0.0.rgba"))
    );
    assert_eq!(
        raw_bytes.len(),
        usize::try_from(width * height * 4).expect("raw capture byte count fits usize")
    );
    let raw_content_pixels = raw_bytes
        .chunks_exact(4)
        .filter(|pixel| pixel[3] > 0)
        .count();
    assert_eq!(
        u64::try_from(raw_content_pixels).expect("raw content pixel count fits u64"),
        raw_json["images"][0]["content_pixels"]
            .as_u64()
            .expect("raw capture content pixel count is integer")
    );

    let mask_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&mask_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes object mask raw RGBA");

    assert!(
        mask_output.status.success(),
        "agent observe mask capture should succeed, stderr: {}",
        String::from_utf8_lossy(&mask_output.stderr)
    );
    let mask_bytes = fs::read(&mask_path).expect("read captured mask RGBA");
    let mask_json: serde_json::Value =
        serde_json::from_slice(&mask_output.stdout).expect("mask capture report is JSON");
    assert_eq!(mask_json["images"][0]["kind"], "mask");
    assert_eq!(
        mask_json["images"][0]["mime_type"],
        "application/octet-stream"
    );
    assert!(
        mask_json["images"][0]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/object.object.dialogue.0.0.mask.rgba"))
    );
    assert!(
        mask_bytes
            .chunks_exact(4)
            .any(|pixel| pixel == [255, 255, 255, 255]),
        "object mask crop should include selected native geometry"
    );
    assert!(
        mask_bytes
            .chunks_exact(4)
            .all(|pixel| pixel == [255, 255, 255, 255] || pixel == [0, 0, 0, 0]),
        "native object mask should remain binary"
    );

    let ruby_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--entry")
        .arg("entry.main")
        .arg("--content-policy-mode")
        .arg("local-dev")
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/3/object.object.dialogue.0.0.ruby.0.png")
        .arg("--mcp")
        .arg("--mcp-format")
        .arg("tool-result")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads rich text ruby child PNG");
    assert!(
        ruby_output.status.success(),
        "agent observe read-uri ruby image should succeed, stderr: {}",
        String::from_utf8_lossy(&ruby_output.stderr)
    );
    let ruby_json: serde_json::Value =
        serde_json::from_slice(&ruby_output.stdout).expect("ruby image tool result is JSON");
    assert_eq!(ruby_json["content"][0]["type"], "text");
    let ruby_metadata: serde_json::Value =
        serde_json::from_str(ruby_json["content"][0]["text"].as_str().unwrap())
            .expect("ruby image metadata is JSON");
    assert_eq!(ruby_metadata["image"]["kind"], "color");
    assert!(ruby_metadata["image"]["width"].as_u64().unwrap() > 0);
    assert!(ruby_metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(ruby_json["content"][1]["type"], "image");
    assert_eq!(ruby_json["content"][1]["mimeType"], "image/png");
    assert!(
        ruby_json["content"][1]["data"]
            .as_str()
            .is_some_and(|data| data.starts_with("iVBORw0KGgo"))
    );

    fs::remove_file(&path).expect("remove temp agent observe source");
    fs::remove_dir_all(&dir).expect("remove temp capture dir");
}

#[test]
fn agent_observe_native_dialogue_view_capture_bounds_include_ruby_extents() {
    let path = temp_arcw(
        "agent-observe-native-dialogue_view-ruby-crop-bounds",
        r"
entry cli @entry.main { goto @flow.main }

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.ruby_over ruby_size=14px ruby_gap=12px]|[夢](ゆめ)[/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-dialogue_view-ruby-crop-bounds");
    let raw_path = dir.join("dialogue_view-ruby-mask.rgba");

    let json = observe_native_dialogue_view_object_raw_report(
        &path,
        &raw_path,
        "mask",
        &["--entry", "entry.main"],
    );
    let image = &json["images"][0];
    let dialogue_view = find_dialogue_view_object(&json);
    let ruby = find_rich_text_ruby_object(&json, 0);
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];

    assert_eq!(image["kind"], "mask");
    assert_eq!(image["scope"]["kind"], "object");
    assert_eq!(image["scope"]["id"], "object.dialogue.0.0");
    assert!(
        image["crop_origin"]["y"].as_u64().unwrap() <= agent_json_bbox_y(annotation),
        "dialogue_view object capture should start above the measured ruby annotation: {json}"
    );
    assert!(
        image["crop_origin"]["y"].as_u64().unwrap() + image["height"].as_u64().unwrap()
            >= agent_json_bbox_bottom(annotation),
        "dialogue_view object capture should include the measured ruby annotation: {json}"
    );
    assert!(
        agent_json_bbox_y(&dialogue_view["bbox"]) <= agent_json_bbox_y(annotation)
            && agent_json_bbox_bottom(&dialogue_view["bbox"]) >= agent_json_bbox_bottom(annotation),
        "dialogue_view reserved layout bounds should include the measured ruby annotation: {json}"
    );
    assert_object_capture_ref_matches_image(dialogue_view, image, "mask", "application/octet-stream");

    fs::remove_file(&path).expect("remove temp native dialogue_view ruby crop source");
    fs::remove_dir_all(&dir).expect("remove temp native dialogue_view ruby crop dir");
}

#[test]
fn agent_observe_native_dialogue_view_capture_bounds_include_vertical_columns() {
    let path = temp_arcw(
        "agent-observe-native-dialogue_view-vertical-crop-bounds",
        r"
entry cli @entry.main { goto @flow.main }

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]吾輩は猫である。ABC 123 2026。[/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-dialogue_view-vertical-crop-bounds");
    let raw_path = dir.join("dialogue_view-vertical-mask.rgba");

    let json = observe_native_dialogue_view_object_raw_report(
        &path,
        &raw_path,
        "mask",
        &["--entry", "entry.main"],
    );
    let image = &json["images"][0];
    let dialogue_view = find_dialogue_view_object(&json);
    let vertical_bottom = json["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .filter(|object| object["role"] == "rich_text_cluster")
        .map(|object| agent_json_bbox_bottom(&object["bbox"]))
        .max()
        .expect("vertical cluster objects are reported");

    assert_eq!(image["kind"], "mask");
    assert_eq!(image["scope"]["kind"], "object");
    assert_eq!(image["scope"]["id"], "object.dialogue.0.0");
    assert!(
        image["crop_origin"]["y"].as_u64().unwrap() + image["height"].as_u64().unwrap()
            >= vertical_bottom,
        "dialogue_view object capture should include measured vertical cluster extents: {json}"
    );
    assert!(
        agent_json_bbox_bottom(&dialogue_view["bbox"]) >= vertical_bottom,
        "dialogue_view reserved layout bounds should include measured vertical cluster extents: {json}"
    );
    assert_object_capture_ref_matches_image(dialogue_view, image, "mask", "application/octet-stream");

    fs::remove_file(&path).expect("remove temp native dialogue_view vertical crop source");
    fs::remove_dir_all(&dir).expect("remove temp native dialogue_view vertical crop dir");
}

#[test]
fn agent_observe_native_vertical_capture_matches_imq_reference() {
    if !imq_is_available() {
        eprintln!("skipping native vertical capture imq comparison: imq is not available");
        return;
    }

    assert_repeated_native_capture_matches_imq_reference(
        "vertical-rl-mixed",
        r"
entry cli @entry.main { goto @flow.main }

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]吾輩は猫である。ABC 123 2026。[/][p]
}
",
    );
    assert_repeated_native_capture_matches_imq_reference(
        "vertical-lr-ruby-text-combine",
        r"
entry cli @entry.main { goto @flow.main }

character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]縦 |[夢](ゆめ)[r] 2026 ABC。[/][p]
}
",
    );
}

fn assert_repeated_native_capture_matches_imq_reference(label: &str, source: &str) {
    let path = temp_arcw(&format!("agent-observe-native-{label}-imq"), source);
    let dir = temp_dir(&format!("agent-observe-native-{label}-imq"));
    let reference_path = dir.join(format!("{label}-reference.png"));
    let candidate_path = dir.join(format!("{label}-candidate.png"));
    let entry =
        EntryRuntimeId::from_source_entity_body("entry.main").expect("test entry ID is valid");

    let reference_json = capture_native_png_report(&path, &entry, &reference_path);
    let candidate_json = capture_native_png_report(&path, &entry, &candidate_path);
    assert_native_capture_has_content(&reference_json, &format!("{label}-reference.png"));
    assert_native_capture_has_content(&candidate_json, &format!("{label}-candidate.png"));

    let imq_output = Command::new("imq")
        .arg("image")
        .arg(&reference_path)
        .arg(&candidate_path)
        .arg("--format")
        .arg("json")
        .output()
        .expect("imq compares native vertical captures");
    assert!(
        imq_output.status.success(),
        "{label} imq comparison should succeed, stderr: {}",
        String::from_utf8_lossy(&imq_output.stderr)
    );
    let imq_json: serde_json::Value =
        serde_json::from_slice(&imq_output.stdout).expect("imq output is JSON");
    assert_eq!(imq_json["dimensions"]["width"], 1280);
    assert_eq!(imq_json["dimensions"]["height"], 720);
    assert_metric_close(
        metric_score(&imq_json, "mse"),
        0.0,
        0.0,
        &format!("{label} mse"),
    );
    assert_metric_close(
        metric_score(&imq_json, "mae"),
        0.0,
        0.0,
        &format!("{label} mae"),
    );
    assert_metric_close(
        metric_score(&imq_json, "maxae"),
        0.0,
        0.0,
        &format!("{label} maxae"),
    );
    assert!(
        metric_score(&imq_json, "ssim") >= 0.999_999,
        "{label} ssim should report identical native captures: {imq_json}"
    );
    assert_metric_close(
        metric_detail(&imq_json, "psnr", "mse"),
        0.0,
        0.0,
        &format!("{label} psnr.mse"),
    );

    fs::remove_file(&path).expect("remove temp native vertical source");
    fs::remove_dir_all(&dir).expect("remove temp native vertical dir");
}

#[test]
fn native_checked_in_visual_golden_artifacts_are_well_formed() {
    let tutr = include_bytes!("../../../../../tests/fixtures/native_capture/vertical_tutr_golden.png");
    let loose = include_bytes!(
        "../../../../../tests/fixtures/native_capture/vertical_jlreq_preset_loose_golden.png"
    );
    let normal = include_bytes!(
        "../../../../../tests/fixtures/native_capture/vertical_jlreq_preset_normal_golden.png"
    );
    let vertical_lr_ruby_text_combine = include_bytes!(
        "../../../../../tests/fixtures/native_capture/vertical_lr_ruby_text_combine_golden.png"
    );
    for (label, golden) in [
        ("vertical Tu/Tr", tutr.as_slice()),
        ("loose JLREQ preset", loose.as_slice()),
        ("normal JLREQ preset", normal.as_slice()),
        (
            "vertical_lr ruby/text-combine",
            vertical_lr_ruby_text_combine.as_slice(),
        ),
    ] {
        assert_checked_in_native_png_golden(label, golden);
    }
    assert_ne!(
        loose.as_slice(),
        normal.as_slice(),
        "loose and normal JLREQ preset visual goldens should capture different column plans"
    );
}

fn assert_checked_in_native_png_golden(label: &str, golden: &[u8]) {
    assert_eq!(&golden[..8], b"\x89PNG\r\n\x1a\n", "{label}");
    assert_eq!(
        png_dimensions(golden),
        Some((1280, 720)),
        "checked-in {label} golden should stay at the Agent capture size"
    );
    assert!(
        golden.len() > 1024,
        "checked-in {label} golden should contain image data"
    );
}

const EXACT_NATIVE_GOLDEN_METRIC_SET: &str = "psnr,ssim,mse,mae,maxae";
const EXACT_NATIVE_GOLDEN_VIEWPORT_WIDTH: u32 = 1280;
const EXACT_NATIVE_GOLDEN_VIEWPORT_HEIGHT: u32 = 720;

#[derive(Clone, Copy, Debug)]
struct NativeExactGoldenFixture {
    id: &'static str,
    label: &'static str,
    source_filename: &'static str,
    entry_source_body: &'static str,
    golden_filename: &'static str,
    max_mse: f64,
    max_mae: f64,
}

impl NativeExactGoldenFixture {
    fn source_path(&self, fixture_dir: &Path) -> PathBuf {
        fixture_dir.join(self.source_filename)
    }

    fn golden_path(&self, fixture_dir: &Path) -> PathBuf {
        fixture_dir.join(self.golden_filename)
    }

    fn entry(&self) -> EntryRuntimeId {
        EntryRuntimeId::from_source_entity_body(self.entry_source_body)
            .expect("native exact golden entry ID is valid")
    }
}

#[derive(Debug)]
struct NativeExactGoldenArtifactPaths {
    artifact_dir: PathBuf,
    candidate_path: PathBuf,
    observe_path: PathBuf,
    metrics_path: PathBuf,
    fingerprint_path: PathBuf,
}

impl NativeExactGoldenArtifactPaths {
    fn for_fixture(fixture: NativeExactGoldenFixture) -> Self {
        let artifact_dir = workspace_root()
            .join("target/arcweft-native-golden-drift/test-visual-golden")
            .join(fixture.id);
        Self {
            candidate_path: artifact_dir.join(format!("{}.candidate.png", fixture.id)),
            observe_path: artifact_dir.join(format!("{}.observe.json", fixture.id)),
            metrics_path: artifact_dir.join(format!("{}.imq.json", fixture.id)),
            fingerprint_path: artifact_dir.join(format!("{}.environment.json", fixture.id)),
            artifact_dir,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct NativeExactGoldenEnvironmentBlocker {
    classification: &'static str,
    code: &'static str,
    message: &'static str,
}

const NATIVE_EXACT_GOLDEN_FIXTURES: &[NativeExactGoldenFixture] = &[
    NativeExactGoldenFixture {
        id: "vertical_tutr_golden",
        label: "vertical Tu/Tr",
        source_filename: "vertical_tutr_golden.arcw",
        entry_source_body: "entry.vertical_tutr_golden",
        golden_filename: "vertical_tutr_golden.png",
        max_mse: 0.002,
        max_mae: 0.003,
    },
    NativeExactGoldenFixture {
        id: "vertical_jlreq_preset_loose_golden",
        label: "loose JLREQ preset",
        source_filename: "vertical_jlreq_preset_loose_golden.arcw",
        entry_source_body: "entry.vertical_jlreq_preset_loose_golden",
        golden_filename: "vertical_jlreq_preset_loose_golden.png",
        max_mse: 0.002,
        max_mae: 0.003,
    },
    NativeExactGoldenFixture {
        id: "vertical_jlreq_preset_normal_golden",
        label: "normal JLREQ preset",
        source_filename: "vertical_jlreq_preset_normal_golden.arcw",
        entry_source_body: "entry.vertical_jlreq_preset_normal_golden",
        golden_filename: "vertical_jlreq_preset_normal_golden.png",
        max_mse: 0.002,
        max_mae: 0.003,
    },
    NativeExactGoldenFixture {
        id: "vertical_lr_ruby_text_combine_golden",
        label: "vertical_lr ruby/text-combine",
        source_filename: "vertical_lr_ruby_text_combine_golden.arcw",
        entry_source_body: "entry.vertical_lr_ruby_text_combine_golden",
        golden_filename: "vertical_lr_ruby_text_combine_golden.png",
        max_mse: 0.002,
        max_mae: 0.003,
    },
];

#[test]
#[ignore = "tier 2 visual regression: exact PNG/imq golden is environment-sensitive"]
fn agent_observe_native_renderer_matches_checked_in_imq_golden_fixture_vertical_tutr() {
    assert_checked_in_native_imq_golden("vertical_tutr_golden");
}

#[test]
#[ignore = "tier 2 visual regression: exact PNG/imq golden is environment-sensitive"]
fn agent_observe_native_renderer_matches_checked_in_imq_golden_fixture_vertical_jlreq_preset_loose()
{
    assert_checked_in_native_imq_golden("vertical_jlreq_preset_loose_golden");
}

#[test]
#[ignore = "tier 2 visual regression: exact PNG/imq golden is environment-sensitive"]
fn agent_observe_native_renderer_matches_checked_in_imq_golden_fixture_vertical_jlreq_preset_normal(
) {
    assert_checked_in_native_imq_golden("vertical_jlreq_preset_normal_golden");
}

#[test]
#[ignore = "tier 2 visual regression: exact PNG/imq golden is environment-sensitive"]
fn agent_observe_native_renderer_matches_checked_in_imq_golden_fixture_vertical_lr_ruby_text_combine(
) {
    assert_checked_in_native_imq_golden("vertical_lr_ruby_text_combine_golden");
}

fn assert_checked_in_native_imq_golden(fixture_id: &str) {
    let fixture = native_exact_golden_fixture(fixture_id);
    let fixture_dir = workspace_root().join("tests/fixtures/native_capture");
    let source_path = fixture.source_path(&fixture_dir);
    let golden_path = fixture.golden_path(&fixture_dir);
    let paths = NativeExactGoldenArtifactPaths::for_fixture(fixture);
    reset_native_exact_golden_artifacts(&paths);

    if handle_exact_native_golden_environment_blocker(fixture, &paths) {
        return;
    }

    let golden_bytes = fs::read(&golden_path).expect("read checked-in native visual golden");
    assert_checked_in_native_png_golden(fixture.label, &golden_bytes);
    capture_native_exact_golden_candidate(fixture, &source_path, &paths);
    let imq_json = run_exact_native_golden_imq(fixture, &golden_path, &paths);
    let mse = metric_score(&imq_json, "mse");
    let mae = metric_score(&imq_json, "mae");
    let status = exact_native_golden_status(fixture, &imq_json, mse, mae);
    write_exact_native_golden_fingerprint(fixture, &paths, status, None);
    assert_exact_native_golden_metrics(fixture, &golden_path, &paths, &imq_json, mse, mae);
}

fn reset_native_exact_golden_artifacts(paths: &NativeExactGoldenArtifactPaths) {
    fs::create_dir_all(&paths.artifact_dir)
        .expect("create native exact golden artifact directory");
    for stale_path in [
        &paths.candidate_path,
        &paths.observe_path,
        &paths.metrics_path,
        &paths.fingerprint_path,
    ] {
        let _ = fs::remove_file(stale_path);
    }
}

fn handle_exact_native_golden_environment_blocker(
    fixture: NativeExactGoldenFixture,
    paths: &NativeExactGoldenArtifactPaths,
) -> bool {
    let Some(blocker) = exact_native_golden_environment_blocker() else {
        return false;
    };
    write_exact_native_golden_fingerprint(fixture, paths, blocker.classification, Some(&blocker));
    let message = format!(
        "{} exact native golden {}: fixture={}, fingerprint={}, {}",
        blocker.classification,
        blocker.code,
        fixture.id,
        paths.fingerprint_path.display(),
        blocker.message
    );
    assert!(!exact_native_golden_required(), "{message}");
    eprintln!("{message}");
    true
}

fn capture_native_exact_golden_candidate(
    fixture: NativeExactGoldenFixture,
    source_path: &Path,
    paths: &NativeExactGoldenArtifactPaths,
) {
    let candidate_json =
        capture_native_png_report(source_path, &fixture.entry(), &paths.candidate_path);
    fs::write(
        &paths.observe_path,
        serde_json::to_vec_pretty(&candidate_json).expect("serialize native golden observe JSON"),
    )
    .expect("write native exact golden observe JSON");
    let candidate_name = paths
        .candidate_path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("candidate path has UTF-8 file name");
    assert_native_exact_capture_has_content(&candidate_json, candidate_name);
    assert_eq!(fixture.id, candidate_name.trim_end_matches(".candidate.png"));
}

fn run_exact_native_golden_imq(
    fixture: NativeExactGoldenFixture,
    golden_path: &Path,
    paths: &NativeExactGoldenArtifactPaths,
) -> serde_json::Value {
    let imq_output = Command::new("imq")
        .arg("image")
        .arg(golden_path)
        .arg(&paths.candidate_path)
        .arg("--metrics")
        .arg(EXACT_NATIVE_GOLDEN_METRIC_SET)
        .arg("--format")
        .arg("json")
        .output()
        .expect("imq compares checked-in native visual golden");
    fs::write(&paths.metrics_path, &imq_output.stdout)
        .expect("write checked-in native visual golden imq metrics");
    if !imq_output.status.success() {
        write_exact_native_golden_fingerprint(
            fixture,
            paths,
            "hard_visual_regression",
            None,
        );
        panic!(
            "fixture={} imq checked-in golden comparison should succeed, reference={}, candidate={}, observe={}, metrics={}, environment={}, stderr: {}",
            fixture.id,
            golden_path.display(),
            paths.candidate_path.display(),
            paths.observe_path.display(),
            paths.metrics_path.display(),
            paths.fingerprint_path.display(),
            String::from_utf8_lossy(&imq_output.stderr)
        );
    }
    match serde_json::from_slice(&imq_output.stdout) {
        Ok(json) => json,
        Err(error) => {
            write_exact_native_golden_fingerprint(
                fixture,
                paths,
                "hard_visual_regression",
                None,
            );
            panic!(
                "fixture={} imq output should be JSON, reference={}, candidate={}, observe={}, metrics={}, environment={}, error={error}",
                fixture.id,
                golden_path.display(),
                paths.candidate_path.display(),
                paths.observe_path.display(),
                paths.metrics_path.display(),
                paths.fingerprint_path.display()
            );
        }
    }
}

fn exact_native_golden_status(
    fixture: NativeExactGoldenFixture,
    imq_json: &serde_json::Value,
    mse: f64,
    mae: f64,
) -> &'static str {
    let width = imq_json["dimensions"]["width"].as_u64().unwrap_or_default();
    let height = imq_json["dimensions"]["height"].as_u64().unwrap_or_default();
    if width != u64::from(EXACT_NATIVE_GOLDEN_VIEWPORT_WIDTH)
        || height != u64::from(EXACT_NATIVE_GOLDEN_VIEWPORT_HEIGHT)
    {
        "hard_visual_regression"
    } else if mse <= fixture.max_mse && mae <= fixture.max_mae {
        "passed"
    } else {
        "baseline_drift"
    }
}

fn assert_exact_native_golden_metrics(
    fixture: NativeExactGoldenFixture,
    golden_path: &Path,
    paths: &NativeExactGoldenArtifactPaths,
    imq_json: &serde_json::Value,
    mse: f64,
    mae: f64,
) {
    assert_eq!(
        imq_json["dimensions"]["width"],
        EXACT_NATIVE_GOLDEN_VIEWPORT_WIDTH,
        "fixture={} visual golden width should match, reference={}, candidate={}, observe={}, metrics={}, environment={}: {imq_json}",
        fixture.id,
        golden_path.display(),
        paths.candidate_path.display(),
        paths.observe_path.display(),
        paths.metrics_path.display(),
        paths.fingerprint_path.display()
    );
    assert_eq!(
        imq_json["dimensions"]["height"],
        EXACT_NATIVE_GOLDEN_VIEWPORT_HEIGHT,
        "fixture={} visual golden height should match, reference={}, candidate={}, observe={}, metrics={}, environment={}: {imq_json}",
        fixture.id,
        golden_path.display(),
        paths.candidate_path.display(),
        paths.observe_path.display(),
        paths.metrics_path.display(),
        paths.fingerprint_path.display()
    );
    assert!(
        mse <= fixture.max_mse,
        "fixture={} visual golden mse drift should stay bounded, max_mse={}, actual_mse={}, actual_mae={}, reference={}, candidate={}, observe={}, metrics={}, environment={}: {imq_json}",
        fixture.id,
        fixture.max_mse,
        mse,
        mae,
        golden_path.display(),
        paths.candidate_path.display(),
        paths.observe_path.display(),
        paths.metrics_path.display(),
        paths.fingerprint_path.display()
    );
    assert!(
        mae <= fixture.max_mae,
        "fixture={} visual golden mae drift should stay bounded, max_mae={}, actual_mse={}, actual_mae={}, reference={}, candidate={}, observe={}, metrics={}, environment={}: {imq_json}",
        fixture.id,
        fixture.max_mae,
        mse,
        mae,
        golden_path.display(),
        paths.candidate_path.display(),
        paths.observe_path.display(),
        paths.metrics_path.display(),
        paths.fingerprint_path.display()
    );
    assert_metric_close(
        metric_detail(imq_json, "psnr", "mse"),
        mse,
        0.0,
        "psnr.mse",
    );
}

fn assert_native_exact_capture_has_content(report: &serde_json::Value, written_name: &str) {
    assert_eq!(report["images"][0]["kind"], "color");
    assert_eq!(report["images"][0]["renderer"], "native");
    assert_eq!(report["images"][0]["composition"], "framebuffer");
    assert_eq!(report["images"][0]["mime_type"], "image/png");
    assert_eq!(report["images"][0]["width"], EXACT_NATIVE_GOLDEN_VIEWPORT_WIDTH);
    assert_eq!(report["images"][0]["height"], EXACT_NATIVE_GOLDEN_VIEWPORT_HEIGHT);
    assert!(report["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    let written = report["images"][0]["written"]
        .as_str()
        .expect("native exact capture report includes written path");
    assert_eq!(native_capture_written_file_name(written), written_name);
}

fn native_capture_written_file_name(written: &str) -> &str {
    written
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(written)
}

fn native_exact_golden_fixture(fixture_id: &str) -> NativeExactGoldenFixture {
    NATIVE_EXACT_GOLDEN_FIXTURES
        .iter()
        .copied()
        .find(|fixture| fixture.id == fixture_id)
        .unwrap_or_else(|| panic!("unknown native exact golden fixture `{fixture_id}`"))
}

fn exact_native_golden_environment_blocker() -> Option<NativeExactGoldenEnvironmentBlocker> {
    if exact_native_golden_required() && std::env::var_os("ARW_EXACT_NATIVE_GOLDEN_PINNED").is_none()
    {
        return Some(NativeExactGoldenEnvironmentBlocker {
            classification: "environment_not_pinned",
            code: "missing_required_pin",
            message: "required exact native golden job must set ARW_EXACT_NATIVE_GOLDEN_PINNED=1",
        });
    }
    if !cfg!(windows) {
        return Some(NativeExactGoldenEnvironmentBlocker {
            classification: if exact_native_golden_required() {
                "environment_blocker"
            } else {
                "expected_skip"
            },
            code: "unsupported_os",
            message: "checked-in native exact goldens are Windows MS Mincho fixtures",
        });
    }
    if !imq_is_available() {
        return Some(NativeExactGoldenEnvironmentBlocker {
            classification: "environment_blocker",
            code: "missing_imq",
            message: "imq is required for exact native golden metrics",
        });
    }
    if !pinned_native_golden_font_available() {
        return Some(NativeExactGoldenEnvironmentBlocker {
            classification: "environment_blocker",
            code: "missing_pinned_font",
            message: "MS Mincho font probe failed for the pinned exact native golden environment",
        });
    }
    if let Ok(backend) = std::env::var("ARW_EXACT_NATIVE_GOLDEN_BACKEND")
        && backend != "native_rich_text_observer"
    {
        return Some(NativeExactGoldenEnvironmentBlocker {
            classification: "environment_blocker",
            code: "unsupported_backend",
            message: "ARW_EXACT_NATIVE_GOLDEN_BACKEND must be native_rich_text_observer",
        });
    }
    None
}

fn exact_native_golden_required() -> bool {
    std::env::var_os("ARW_EXACT_NATIVE_GOLDEN_REQUIRED").is_some()
}

fn pinned_native_golden_font_available() -> bool {
    pinned_native_golden_font_path().is_some_and(|path| path.exists())
}

fn pinned_native_golden_font_path() -> Option<PathBuf> {
    std::env::var_os("WINDIR").map(|windir| PathBuf::from(windir).join("Fonts").join("msmincho.ttc"))
}

fn write_exact_native_golden_fingerprint(
    fixture: NativeExactGoldenFixture,
    paths: &NativeExactGoldenArtifactPaths,
    status: &str,
    blocker: Option<&NativeExactGoldenEnvironmentBlocker>,
) {
    let root = workspace_root();
    let fixture_dir = root.join("tests/fixtures/native_capture");
    let source_path = fixture.source_path(&fixture_dir);
    let golden_path = fixture.golden_path(&fixture_dir);
    let font_path = pinned_native_golden_font_path();
    let fingerprint = serde_json::json!({
        "schema": "arcweft.exact_native_golden.environment.v1",
        "fixture": {
            "id": fixture.id,
            "label": fixture.label,
            "source": source_path.display().to_string(),
            "reference": golden_path.display().to_string(),
            "thresholds": {
                "max_mse": fixture.max_mse,
                "max_mae": fixture.max_mae,
            },
        },
        "status": status,
        "blocker": blocker.map(|blocker| serde_json::json!({
            "classification": blocker.classification,
            "code": blocker.code,
            "message": blocker.message,
        })),
        "environment": {
            "required": exact_native_golden_required(),
            "pinned": std::env::var_os("ARW_EXACT_NATIVE_GOLDEN_PINNED").is_some(),
            "os": {
                "family": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "version_family": native_exact_golden_os_version(),
            },
            "renderer": {
                "backend_path": "native_rich_text_observer",
                "backend_env": std::env::var("ARW_EXACT_NATIVE_GOLDEN_BACKEND").ok(),
                "arcw_binary": env!("CARGO_BIN_EXE_arcw"),
            },
            "font": {
                "requested_family": "MS Mincho",
                "fallback_policy": "fixture source pins MS Mincho; exact baseline acceptance is blocked if the pinned family probe fails",
                "windows_font_file": font_path.as_ref().map(|path| path.display().to_string()),
                "windows_font_file_exists": font_path.as_ref().is_some_and(|path| path.exists()),
            },
            "viewport": {
                "width": EXACT_NATIVE_GOLDEN_VIEWPORT_WIDTH,
                "height": EXACT_NATIVE_GOLDEN_VIEWPORT_HEIGHT,
                "device_scale": 1.0,
            },
            "png": {
                "format": "png",
                "capture_command_format": "arcw agent observe --image png",
                "color_format": "PNG bytes emitted by native Agent capture",
            },
            "arcweft": {
                "commit": native_exact_golden_git_commit(&root),
                "dirty": native_exact_golden_git_dirty(&root),
                "source_hash": native_exact_golden_git_hash_object(&root, &source_path),
                "reference_hash": native_exact_golden_git_hash_object(&root, &golden_path),
            },
            "imq": {
                "available": imq_is_available(),
                "version": native_exact_golden_imq_version(),
                "metrics": ["psnr", "ssim", "mse", "mae", "maxae"],
            },
        },
        "artifacts": {
            "artifact_dir": paths.artifact_dir.display().to_string(),
            "reference": artifact_status_json(&golden_path),
            "candidate": artifact_status_json(&paths.candidate_path),
            "observe_json": artifact_status_json(&paths.observe_path),
            "metrics_json": artifact_status_json(&paths.metrics_path),
            "fingerprint_json": paths.fingerprint_path.display().to_string(),
        },
    });
    fs::write(
        &paths.fingerprint_path,
        serde_json::to_vec_pretty(&fingerprint).expect("serialize exact native golden fingerprint"),
    )
    .expect("write exact native golden fingerprint");
}

fn artifact_status_json(path: &Path) -> serde_json::Value {
    serde_json::json!({
        "path": path.display().to_string(),
        "exists": path.exists(),
    })
}

fn native_exact_golden_os_version() -> Option<String> {
    if cfg!(windows) {
        native_exact_golden_command_stdout(Command::new("cmd").arg("/C").arg("ver"))
    } else if cfg!(target_os = "macos") {
        native_exact_golden_command_stdout(Command::new("sw_vers").arg("-productVersion"))
    } else {
        native_exact_golden_command_stdout(Command::new("uname").arg("-srv"))
    }
}

fn native_exact_golden_git_commit(root: &Path) -> Option<String> {
    native_exact_golden_command_stdout(
        Command::new("git")
            .arg("-C")
            .arg(root)
            .arg("rev-parse")
            .arg("HEAD"),
    )
}

fn native_exact_golden_git_dirty(root: &Path) -> Option<bool> {
    native_exact_golden_command_stdout(
        Command::new("git")
            .arg("-C")
            .arg(root)
            .arg("status")
            .arg("--short"),
    )
    .map(|status| !status.is_empty())
}

fn native_exact_golden_git_hash_object(root: &Path, path: &Path) -> Option<String> {
    native_exact_golden_command_stdout(
        Command::new("git")
            .arg("-C")
            .arg(root)
            .arg("hash-object")
            .arg(path),
    )
}

fn native_exact_golden_imq_version() -> Option<String> {
    native_exact_golden_command_stdout(Command::new("imq").arg("--version")).or_else(|| {
        native_exact_golden_command_stdout(Command::new("imq").arg("--help"))
            .and_then(|help| help.lines().next().map(str::to_owned))
    })
}

fn native_exact_golden_command_stdout(command: &mut Command) -> Option<String> {
    let output = command.stderr(Stdio::null()).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    Some(stdout.trim().to_owned())
}

#[test]
fn agent_observe_native_renderer_reports_vertical_lr_ruby_text_combine_geometry() {
    let path = temp_arcw(
        "agent-observe-native-vertical-lr-ruby-combine",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]縦 |[夢](ゆめ)[r] 2026 ABC。[/][p]
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reports native vertical_lr rich-text geometry");

    fs::remove_file(&path).expect("remove temp native vertical_lr source");
    assert!(
        output.status.success(),
        "native vertical_lr rich-text observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native vertical_lr report is JSON");
    assert_native_vertical_lr_ruby_text_combine_report(&json);
}

fn assert_native_vertical_lr_ruby_text_combine_report(json: &serde_json::Value) {
    let image = &json["images"][0];
    assert_eq!(image["renderer"], "native");
    assert_eq!(image["scope"]["kind"], "layer");
    assert_eq!(image["scope"]["id"], "dialogue.rich_text");
    assert_eq!(image["composition"], "masked_framebuffer_crop");
    assert!(image["content_pixels"].as_u64().unwrap() > 0);

    let dialogue_view = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["role"] == "dialogue_view")
        .expect("dialogue_view object is observed");
    let text_runs = observed_object_rich_text_frame(dialogue_view)["display_map"]["text_runs"]
        .as_array()
        .expect("text runs are reported");
    assert!(
        text_runs.iter().all(|run| {
            run["presentation"]["layout"]["writing_mode"]
                .as_str()
                .is_some_and(|mode| mode == "vertical_lr")
        }),
        "all display-map runs in the sample should preserve vertical_lr presentation"
    );
    assert!(
        text_runs.iter().any(|run| {
            run["range"]["start"].as_u64() == Some(8) && run["range"]["end"].as_u64() == Some(20)
        }),
        "the run containing 2026 should remain observable for text-combine geometry"
    );

    let objects = json["objects"].as_array().unwrap();
    let digit_run = objects
        .iter()
        .find(|object| object["role"] == "rich_text_run" && object["text"] == " 2026 ABC。")
        .expect("vertical text-combine run object is observed");
    assert_eq!(digit_run["rich_text_ref"]["source"], "text");
    assert_eq!(
        digit_run["rich_text_ref"]["presentation"]["layout"]["writing_mode"],
        "vertical_lr"
    );
    assert_rich_text_hit_region_matches_bbox(digit_run, "text_run", 8, 20);
    assert!(
        digit_run["bbox"]["height"].as_u64().unwrap()
            > digit_run["bbox"]["width"].as_u64().unwrap(),
        "vertical_lr text-combine run geometry should be column-oriented"
    );
    let text_combine = find_rich_text_cluster_object(json, "2026", 9, 13);
    assert_eq!(text_combine["rich_text_ref"]["kind"], "glyph_cluster");
    assert_rich_text_hit_region_matches_bbox(text_combine, "glyph_cluster", 9, 13);
    let next_latin = find_rich_text_cluster_object(json, "A", 14, 15);
    assert!(
        text_combine["bbox"]["width"].as_u64().unwrap()
            <= next_latin["bbox"]["width"].as_u64().unwrap()
            && text_combine["bbox"]["height"].as_u64().unwrap()
                <= next_latin["bbox"]["height"].as_u64().unwrap(),
        "4-digit text-combine cluster should occupy one vertical cell: {text_combine}"
    );
    assert!(
        next_latin["bbox"]["x"].as_u64().unwrap() > text_combine["bbox"]["x"].as_u64().unwrap(),
        "vertical_lr text after a text-combine cluster should advance to the next column"
    );
    assert!(
        text_combine["capture_refs"]["captures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capture| capture["kind"] == "mask"
                && capture["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with(".mask.rgba"))),
        "text-combine cluster should expose native mask capture refs"
    );

    let ruby = objects
        .iter()
        .find(|object| object["role"] == "rich_text_ruby")
        .expect("vertical ruby child object is observed");
    assert_eq!(ruby["rich_text_ref"]["kind"], "ruby");
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ゆめ");
    assert_rich_text_hit_region_matches_bbox(ruby, "ruby_object", 4, 7);
    assert_rich_text_hit_region_matches_ref_bbox(ruby, "ruby_base", "ruby_base_bbox", 4, 7);
    assert_rich_text_hit_region_matches_ref_bbox(
        ruby,
        "ruby_annotation",
        "ruby_annotation_bbox",
        4,
        7,
    );
    assert!(ruby["bbox"]["width"].as_u64().unwrap() > 0);
    assert!(ruby["bbox"]["height"].as_u64().unwrap() > 0);
}

#[test]
fn agent_observe_native_renderer_reports_vertical_goal_clear_smoke_geometry() {
    let source_path = vertical_goal_clear_smoke_fixture_path();
    let dir = temp_dir("agent-observe-native-vertical-goal-clear-smoke-geometry");
    let png_path = dir.join("vertical-goal-clear-smoke.png");

    let entry = EntryRuntimeId::from_source_entity_body("entry.vertical_goal_clear_smoke")
        .expect("vertical goal-clear entry ID is valid");
    let json = capture_native_png_report(&source_path, &entry, &png_path);
    assert_native_capture_has_content(&json, "vertical-goal-clear-smoke.png");
    assert_eq!(
        png_dimensions(&fs::read(&png_path).expect("read vertical goal-clear smoke png")),
        Some((1280, 720))
    );
    assert_native_vertical_goal_clear_smoke_report(&json);

    fs::remove_dir_all(&dir).expect("remove temp vertical goal-clear smoke geometry dir");
}

fn vertical_goal_clear_smoke_fixture_path() -> PathBuf {
    workspace_root().join("tests/fixtures/native_capture/vertical_goal_clear_smoke.arcw")
}

fn assert_native_vertical_goal_clear_smoke_report(json: &serde_json::Value) {
    let image = &json["images"][0];
    assert_eq!(image["renderer"], "native");
    assert_eq!(image["kind"], "color");
    assert_eq!(image["composition"], "framebuffer");
    assert!(image["content_pixels"].as_u64().unwrap() > 0);

    assert_rich_text_cluster_metadata(json, "吾", 0, 3, "upright", "none");
    assert_rich_text_cluster_metadata(json, "2026", 9, 13, "text_combine_upright", "none");
    assert_rich_text_cluster_metadata(json, "ABC", 13, 16, "sideways_cw", "none");
    assert_rich_text_cluster_metadata(json, "縦", 29, 32, "upright", "none");
    assert_rich_text_cluster_metadata(json, "2026", 38, 42, "text_combine_upright", "none");
    assert_rich_text_cluster_metadata(json, "XYZ", 42, 45, "sideways_cw", "none");

    let rl_start = find_rich_text_cluster_object(json, "吾", 0, 3);
    let rl_second = find_rich_text_cluster_object(json, "輩", 3, 6);
    let rl_text_combine = find_rich_text_cluster_object(json, "2026", 9, 13);
    let rl_sideways = find_rich_text_cluster_object(json, "ABC", 13, 16);
    let lr_start = find_rich_text_cluster_object(json, "縦", 29, 32);
    let lr_second = find_rich_text_cluster_object(json, "夢", 32, 35);
    let lr_text_combine = find_rich_text_cluster_object(json, "2026", 38, 42);
    let lr_after_text_combine = find_rich_text_cluster_object(json, "XYZ", 42, 45);
    assert_rich_text_hit_region_matches_bbox(rl_text_combine, "glyph_cluster", 9, 13);
    assert_rich_text_hit_region_matches_bbox(lr_text_combine, "glyph_cluster", 38, 42);
    assert_rich_text_object_has_mask_capture(lr_text_combine, "vertical goal-clear text-combine");
    assert!(
        agent_json_bbox_y(&rl_second["bbox"]) > agent_json_bbox_y(&rl_start["bbox"]),
        "vertical_rl inline progression should move later same-column content down: {rl_second}"
    );
    assert!(
        agent_json_bbox_x(&rl_text_combine["bbox"]) < agent_json_bbox_x(&rl_start["bbox"]),
        "vertical_rl should move the unbroken text-combine/Latin word into the next left column: {rl_text_combine}"
    );
    assert_eq!(
        agent_json_bbox_x(&rl_sideways["bbox"]),
        agent_json_bbox_x(&rl_text_combine["bbox"]),
        "vertical_rl text-combine and following Latin word must stay in one column"
    );
    assert!(
        agent_json_bbox_y(&rl_sideways["bbox"])
            > agent_json_bbox_y(&rl_text_combine["bbox"]),
        "vertical_rl sideways Latin should follow text-combine inline: {rl_sideways}"
    );
    assert!(
        agent_json_bbox_y(&lr_second["bbox"]) > agent_json_bbox_y(&lr_start["bbox"]),
        "vertical_lr inline progression should move later same-column content down: {lr_second}"
    );
    assert!(
        agent_json_bbox_x(&lr_text_combine["bbox"]) > agent_json_bbox_x(&lr_start["bbox"]),
        "vertical_lr should move the unbroken text-combine/Latin word into the next right column: {lr_text_combine}"
    );
    assert_eq!(
        agent_json_bbox_x(&lr_after_text_combine["bbox"]),
        agent_json_bbox_x(&lr_text_combine["bbox"]),
        "vertical_lr text-combine and following Latin word must stay in one column"
    );
    assert!(
        agent_json_bbox_y(&lr_after_text_combine["bbox"])
            > agent_json_bbox_y(&lr_text_combine["bbox"]),
        "vertical_lr sideways Latin should follow text-combine inline: {lr_after_text_combine}"
    );

    let rl_ruby = find_rich_text_ruby_object(json, 0);
    assert_eq!(rl_ruby["rich_text_ref"]["ruby"], "はい");
    assert_rich_text_object_has_mask_capture(rl_ruby, "vertical goal-clear vertical_rl ruby");
    assert_rich_text_hit_region_matches_bbox(rl_ruby, "ruby_object", 3, 6);
    assert_rich_text_hit_region_matches_ref_bbox(rl_ruby, "ruby_base", "ruby_base_bbox", 3, 6);
    assert_rich_text_hit_region_matches_ref_bbox(
        rl_ruby,
        "ruby_annotation",
        "ruby_annotation_bbox",
        3,
        6,
    );
    assert!(
        agent_json_bbox_center_x_twice(&rl_ruby["rich_text_ref"]["ruby_annotation_bbox"])
            > agent_json_bbox_center_x_twice(&rl_ruby["rich_text_ref"]["ruby_base_bbox"]),
        "vertical_rl ruby annotation should be on the right side of its base: {rl_ruby}"
    );

    let lr_ruby = find_rich_text_ruby_object(json, 1);
    assert_eq!(lr_ruby["rich_text_ref"]["ruby"], "ゆめ");
    assert_rich_text_object_has_mask_capture(lr_ruby, "vertical goal-clear vertical_lr ruby");
    assert_rich_text_hit_region_matches_bbox(lr_ruby, "ruby_object", 32, 35);
    assert_rich_text_hit_region_matches_ref_bbox(lr_ruby, "ruby_base", "ruby_base_bbox", 32, 35);
    assert_rich_text_hit_region_matches_ref_bbox(
        lr_ruby,
        "ruby_annotation",
        "ruby_annotation_bbox",
        32,
        35,
    );
    assert!(
        agent_json_bbox_center_x_twice(&lr_ruby["rich_text_ref"]["ruby_annotation_bbox"])
            < agent_json_bbox_center_x_twice(&lr_ruby["rich_text_ref"]["ruby_base_bbox"]),
        "vertical_lr ruby annotation should be on the left side of its base: {lr_ruby}"
    );
}

#[test]
fn agent_observe_native_renderer_reports_vertical_column_progression_direction() {
    assert_native_vertical_column_progression_direction("vertical_rl", false);
    assert_native_vertical_column_progression_direction("vertical_lr", true);
}

fn assert_native_vertical_column_progression_direction(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-column-progression"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬月火水木金土[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native vertical progression source");
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );

    let first_column_start = find_rich_text_cluster_object(&json, "天", 0, 3);
    let next_column_start = json["objects"]
        .as_array()
        .expect("native observation objects are an array")
        .iter()
        .filter(|object| object["role"] == "rich_text_cluster")
        .filter(|object| object["rich_text_ref"]["range"]["start"].as_u64().unwrap_or_default() > 0)
        .filter(|object| {
            agent_json_bbox_x(&object["bbox"])
                != agent_json_bbox_x(&first_column_start["bbox"])
        })
        .min_by_key(|object| {
            object["rich_text_ref"]["range"]["start"]
                .as_u64()
                .unwrap_or(u64::MAX)
        })
        .expect("vertical sample should advance to another column");
    assert!(
        agent_json_bbox_y(&first_column_start["bbox"])
            .abs_diff(agent_json_bbox_y(&next_column_start["bbox"]))
            <= 1,
        "{writing_mode} next column should restart near the top inline origin"
    );
    if next_column_moves_right {
        assert!(
            agent_json_bbox_x(&next_column_start["bbox"])
                > agent_json_bbox_x(&first_column_start["bbox"]),
            "{writing_mode} next column should advance rightward: {first_column_start} / {next_column_start}"
        );
    } else {
        assert!(
            agent_json_bbox_x(&next_column_start["bbox"])
                < agent_json_bbox_x(&first_column_start["bbox"]),
            "{writing_mode} next column should advance leftward: {first_column_start} / {next_column_start}"
        );
    }
}

#[test]
fn agent_observe_native_renderer_reports_vertical_cluster_orientation_metadata() {
    let path = temp_arcw(
        "agent-observe-native-vertical-cluster-metadata",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]A。ー12[/][p]
}
",
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native vertical cluster metadata source");
    assert_native_rich_text_layer_image_has_content(&json);

    assert_rich_text_cluster_metadata(&json, "A", 0, 1, "sideways_cw", "none");
    assert_rich_text_cluster_metadata(&json, "。", 1, 4, "upright", "upright_alternate");
    assert_rich_text_cluster_metadata(&json, "ー", 4, 7, "sideways_cw", "rotated_alternate");
    assert_rich_text_cluster_metadata(&json, "12", 7, 9, "text_combine_upright", "none");
}

#[test]
fn agent_observe_native_renderer_reports_vertical_grapheme_cluster_metadata() {
    assert_native_vertical_grapheme_cluster_metadata("vertical_rl");
    assert_native_vertical_grapheme_cluster_metadata("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_grapheme_cluster_raw_crops() {
    assert_native_vertical_grapheme_cluster_raw_crop("vertical_rl", "mask");
    assert_native_vertical_grapheme_cluster_raw_crop("vertical_rl", "object-id");
    assert_native_vertical_grapheme_cluster_raw_crop("vertical_lr", "mask");
    assert_native_vertical_grapheme_cluster_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_text_glyph_raw_crops() {
    assert_native_vertical_text_glyph_raw_crop("vertical_rl", "mask");
    assert_native_vertical_text_glyph_raw_crop("vertical_rl", "object-id");
    assert_native_vertical_text_glyph_raw_crop("vertical_lr", "mask");
    assert_native_vertical_text_glyph_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_vertical_zwj_cluster_metadata() {
    assert_native_vertical_zwj_cluster_metadata("vertical_rl");
    assert_native_vertical_zwj_cluster_metadata("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_zwj_cluster_raw_crops() {
    assert_native_vertical_zwj_cluster_raw_crop("vertical_rl", "mask");
    assert_native_vertical_zwj_cluster_raw_crop("vertical_rl", "object-id");
    assert_native_vertical_zwj_cluster_raw_crop("vertical_lr", "mask");
    assert_native_vertical_zwj_cluster_raw_crop("vertical_lr", "object-id");
}

fn assert_native_vertical_grapheme_cluster_metadata(writing_mode: &str) {
    let json = observe_native_vertical_grapheme_cluster(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );

    let kana = assert_native_vertical_grapheme_cluster_geometry(&json);
    let latin = find_rich_text_cluster_object(&json, "A", 6, 7);
    assert_eq!(latin["rich_text_ref"]["orientation"], "sideways_cw");
    assert_vertical_cluster_after(
        kana,
        latin,
        "Latin cluster should follow the decomposed kana grapheme in the same column",
    );
}

fn observe_native_vertical_grapheme_cluster(writing_mode: &str) -> serde_json::Value {
    let text = "か\u{3099}A";
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-grapheme-cluster"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native grapheme-cluster source");
    json
}

fn assert_native_vertical_grapheme_cluster_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name =
        format!("agent-observe-native-{writing_mode}-grapheme-cluster-{capture_kind}");
    let text = "か\u{3099}A";
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-grapheme-cluster-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.0.0.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native grapheme-cluster raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} grapheme-cluster {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native grapheme-cluster report is JSON");
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

    let kana = assert_native_vertical_grapheme_cluster_geometry(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], kana["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], kana["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], kana["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], kana["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(kana),
            content_pixels,
            &format!("{writing_mode} grapheme-cluster object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native grapheme-cluster mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native grapheme-cluster source");
    fs::remove_dir_all(&dir).expect("remove temp native grapheme-cluster dir");
}

fn assert_native_vertical_grapheme_cluster_geometry(
    json: &serde_json::Value,
) -> &serde_json::Value {
    let kana = find_rich_text_cluster_object(json, "か\u{3099}", 0, 6);
    assert_eq!(kana["rich_text_ref"]["kind"], "glyph_cluster");
    assert_eq!(kana["rich_text_ref"]["orientation"], "upright");
    assert_eq!(kana["rich_text_ref"]["vertical_form"], "none");
    assert_eq!(kana["rich_text_ref"]["range"]["start"], 0);
    assert_eq!(kana["rich_text_ref"]["range"]["end"], 6);
    assert_rich_text_object_has_mask_capture(kana, "decomposed kana grapheme cluster");
    kana
}

fn assert_native_vertical_text_glyph_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name = format!("agent-observe-native-{writing_mode}-text-glyph-{capture_kind}");
    let text = "か\u{3099}A";
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-text-glyph-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.glyph.0.0.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native text-glyph raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} text-glyph {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native text-glyph report is JSON");
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

    let glyph = assert_native_vertical_text_glyph_geometry(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], glyph["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], glyph["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], glyph["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], glyph["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(glyph),
            content_pixels,
            &format!("{writing_mode} text-glyph object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native text-glyph mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native text-glyph source");
    fs::remove_dir_all(&dir).expect("remove temp native text-glyph dir");
}

fn assert_native_vertical_text_glyph_geometry(json: &serde_json::Value) -> &serde_json::Value {
    let glyph = find_rich_text_glyph_object(json, "か\u{3099}", 0, 6);
    assert_eq!(glyph["rich_text_ref"]["kind"], "text_glyph");
    assert_eq!(glyph["rich_text_ref"]["orientation"], "upright");
    assert_eq!(glyph["rich_text_ref"]["vertical_form"], "none");
    assert_eq!(glyph["rich_text_ref"]["range"]["start"], 0);
    assert_eq!(glyph["rich_text_ref"]["range"]["end"], 6);
    assert_rich_text_object_has_mask_capture(glyph, "decomposed kana text glyph");
    glyph
}

fn assert_native_vertical_zwj_cluster_metadata(writing_mode: &str) {
    let json = observe_native_vertical_zwj_cluster(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );

    let cluster = assert_native_vertical_zwj_cluster_geometry(&json);
    let latin = find_rich_text_cluster_object(&json, "A", 11, 12);
    assert_eq!(latin["rich_text_ref"]["orientation"], "sideways_cw");
    assert_vertical_cluster_after(
        cluster,
        latin,
        "Latin cluster should follow the ZWJ grapheme in the same column",
    );
}

fn observe_native_vertical_zwj_cluster(writing_mode: &str) -> serde_json::Value {
    let text = "👩‍💻A";
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-zwj-cluster"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]{text}[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native ZWJ-cluster source");
    json
}

fn assert_native_vertical_zwj_cluster_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name = format!("agent-observe-native-{writing_mode}-zwj-cluster-{capture_kind}");
    let text = "👩‍💻A";
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]{text}[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-zwj-cluster-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.0.0.11")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native ZWJ-cluster raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} ZWJ-cluster {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native ZWJ-cluster report is JSON");
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

    let cluster = assert_native_vertical_zwj_cluster_geometry(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], cluster["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], cluster["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], cluster["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], cluster["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(cluster),
            content_pixels,
            &format!("{writing_mode} ZWJ-cluster object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native ZWJ-cluster mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native ZWJ-cluster source");
    fs::remove_dir_all(&dir).expect("remove temp native ZWJ-cluster dir");
}

fn assert_native_vertical_zwj_cluster_geometry(json: &serde_json::Value) -> &serde_json::Value {
    let cluster = find_rich_text_cluster_object(json, "👩‍💻", 0, 11);
    assert_eq!(cluster["rich_text_ref"]["kind"], "glyph_cluster");
    assert_eq!(cluster["rich_text_ref"]["orientation"], "upright");
    assert_eq!(cluster["rich_text_ref"]["vertical_form"], "none");
    assert_eq!(cluster["rich_text_ref"]["range"]["start"], 0);
    assert_eq!(cluster["rich_text_ref"]["range"]["end"], 11);
    assert_rich_text_object_has_mask_capture(cluster, "ZWJ grapheme cluster");
    cluster
}

#[test]
fn agent_observe_native_renderer_reports_vertical_ruby_collision_geometry() {
    assert_native_vertical_ruby_collision_geometry("vertical_rl", true);
    assert_native_vertical_ruby_collision_geometry("vertical_lr", false);
}

#[test]
fn agent_observe_native_renderer_reports_vertical_ruby_under_geometry() {
    assert_native_vertical_ruby_under_geometry("vertical_rl", false);
    assert_native_vertical_ruby_under_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_vertical_ruby_under_raw_crops() {
    assert_native_vertical_ruby_under_raw_crop("vertical_rl", false, "mask");
    assert_native_vertical_ruby_under_raw_crop("vertical_rl", false, "object-id");
    assert_native_vertical_ruby_under_raw_crop("vertical_lr", true, "mask");
    assert_native_vertical_ruby_under_raw_crop("vertical_lr", true, "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_vertical_inter_character_ruby_geometry() {
    assert_native_vertical_inter_character_ruby_geometry("vertical_rl");
    assert_native_vertical_inter_character_ruby_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_inter_character_ruby_raw_crops() {
    assert_native_vertical_inter_character_ruby_raw_crop("vertical_rl", "mask");
    assert_native_vertical_inter_character_ruby_raw_crop("vertical_rl", "object-id");
    assert_native_vertical_inter_character_ruby_raw_crop("vertical_lr", "mask");
    assert_native_vertical_inter_character_ruby_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_long_vertical_ruby_expansion_geometry() {
    assert_native_long_vertical_ruby_expansion_geometry("vertical_rl", true);
    assert_native_long_vertical_ruby_expansion_geometry("vertical_lr", false);
}

fn assert_native_vertical_ruby_under_geometry(writing_mode: &str, annotation_on_right: bool) {
    let json = observe_native_vertical_ruby_under_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_native_vertical_ruby_under_object(&json, writing_mode, annotation_on_right);
}

fn observe_native_vertical_ruby_under_fixture(writing_mode: &str) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-ruby-under"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}][.ruby_under]|[夢](ゆめ)[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native ruby_under source");
    json
}

fn assert_native_vertical_ruby_under_raw_crop(
    writing_mode: &str,
    annotation_on_right: bool,
    capture_kind: &str,
) {
    let fixture_name = format!("agent-observe-native-{writing_mode}-ruby-under-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}][.ruby_under]|[夢](ゆめ)[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-ruby-under-{capture_kind}.rgba"
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
        .expect("arcw agent observe writes native ruby_under raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} ruby_under {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native ruby_under report is JSON");
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

    let ruby = assert_native_vertical_ruby_under_object(&json, writing_mode, annotation_on_right);
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
            &format!("{writing_mode} ruby_under object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native ruby_under raw crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native ruby_under source");
    fs::remove_dir_all(&dir).expect("remove temp native ruby_under dir");
}

fn assert_native_vertical_ruby_under_object<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    annotation_on_right: bool,
) -> &'report serde_json::Value {
    let ruby = find_rich_text_ruby_object(json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ゆめ");
    assert_eq!(
        first_text_run_presentation_layout(json)["writing_mode"],
        writing_mode
    );
    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    if annotation_on_right {
        assert!(
            agent_json_bbox_center_x_twice(annotation) > agent_json_bbox_center_x_twice(base),
            "{writing_mode} ruby_under annotation should render on the right side of the base: {ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_center_x_twice(annotation) < agent_json_bbox_center_x_twice(base),
            "{writing_mode} ruby_under annotation should render on the left side of the base: {ruby}"
        );
    }
    assert_rich_text_hit_region_matches_ref_bbox(ruby, "ruby_base", "ruby_base_bbox", 0, 3);
    assert_rich_text_hit_region_matches_ref_bbox(
        ruby,
        "ruby_annotation",
        "ruby_annotation_bbox",
        0,
        3,
    );
    ruby
}

fn assert_native_vertical_inter_character_ruby_geometry(writing_mode: &str) {
    let json = observe_native_vertical_inter_character_ruby_fixture(writing_mode);
    assert_native_rich_text_layer_image_has_content(&json);
    assert_native_vertical_inter_character_ruby_object(&json, writing_mode);
}

fn observe_native_vertical_inter_character_ruby_fixture(writing_mode: &str) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-ruby-inter-character"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}][.ruby_inter_character]|[夢星](ゆめ)[/]人[p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native ruby_inter_character source");
    json
}

fn assert_native_vertical_inter_character_ruby_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name =
        format!("agent-observe-native-{writing_mode}-ruby-inter-character-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}][.ruby_inter_character]|[夢星](ゆめ)[/]人[p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-ruby-inter-character-{capture_kind}.rgba"
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
        .expect("arcw agent observe writes native ruby_inter_character raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} ruby_inter_character {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native ruby_inter_character report is JSON");
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

    let ruby = assert_native_vertical_inter_character_ruby_object(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], ruby["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], ruby["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], ruby["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels <= width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(ruby),
            content_pixels,
            &format!("{writing_mode} ruby_inter_character object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native ruby_inter_character raw crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert_eq!(transparent as u64, width * height - content_pixels);
    }

    fs::remove_file(&path).expect("remove temp native ruby_inter_character source");
    fs::remove_dir_all(&dir).expect("remove temp native ruby_inter_character dir");
}

fn assert_native_vertical_inter_character_ruby_object<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    let ruby = find_rich_text_ruby_object(json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ゆめ");
    let layout = first_text_run_presentation_layout(json);
    assert_eq!(layout["writing_mode"], writing_mode);
    assert_eq!(layout["ruby_position"], "inter_character");

    let dream = find_rich_text_cluster_object(json, "夢", 0, 3);
    let star = find_rich_text_cluster_object(json, "星", 3, 6);
    let person = find_rich_text_cluster_object(json, "人", 6, 9);
    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    if writing_mode == "vertical_rl" {
        assert!(
            agent_json_bbox_center_x_twice(annotation) > agent_json_bbox_center_x_twice(base),
            "vertical_rl inter-character ruby should use the over track on the right: {ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_center_x_twice(annotation) < agent_json_bbox_center_x_twice(base),
            "vertical_lr inter-character ruby should use the over track on the left: {ruby}"
        );
    }
    assert_eq!(
        agent_json_bbox_x(&star["bbox"]),
        agent_json_bbox_x(&dream["bbox"]),
        "{writing_mode} ruby_inter_character base clusters should remain in the same column"
    );
    assert!(
        agent_json_bbox_y(&star["bbox"]).saturating_add(1)
            >= agent_json_bbox_bottom(&dream["bbox"]),
        "{writing_mode} ruby_inter_character should not enter vertical inline flow: {ruby}"
    );
    assert!(
        agent_json_bbox_y(&person["bbox"]).saturating_add(1)
            >= agent_json_bbox_bottom(&star["bbox"]),
        "{writing_mode} content following ruby should continue normal vertical inline flow: {ruby}"
    );
    assert_rich_text_hit_region_matches_ref_bbox(ruby, "ruby_base", "ruby_base_bbox", 0, 6);
    assert_rich_text_hit_region_matches_ref_bbox(
        ruby,
        "ruby_annotation",
        "ruby_annotation_bbox",
        0,
        6,
    );
    assert_rich_text_object_has_mask_capture(ruby, "ruby_inter_character object");
    ruby
}

fn assert_native_long_vertical_ruby_expansion_geometry(writing_mode: &str, ruby_on_right: bool) {
    let path = temp_arcw(
        &format!("agent-observe-native-long-{writing_mode}-ruby-expansion"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬|[夢](ながいながいよみ)人外[/][p]
}}
"
        ),
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native long vertical ruby source");
    assert_native_rich_text_layer_image_has_content(&json);

    let ruby = find_rich_text_ruby_object(&json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ながいながいよみ");
    assert_rich_text_object_has_mask_capture(ruby, "long vertical ruby object");

    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    let base_cluster = find_rich_text_cluster_object(&json, "夢", 18, 21);
    assert!(
        agent_json_bbox_height(base) > agent_json_bbox_height(&base_cluster["bbox"]) * 2,
        "long vertical ruby should expand base allocation along inline progression: {ruby}"
    );
    assert!(
        agent_json_bbox_height(annotation) >= agent_json_bbox_height(base),
        "long vertical ruby annotation should share the expanded inline extent: {ruby}"
    );
    assert!(
        agent_json_bbox_y(base) < agent_json_bbox_y(&base_cluster["bbox"]),
        "expanded ruby base should be observable beyond the base glyph cell: {ruby}"
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
        agent_json_bbox_x(&ruby["bbox"]) <= agent_json_bbox_x(base)
            && agent_json_bbox_right(&ruby["bbox"]) >= agent_json_bbox_right(annotation)
            && agent_json_bbox_y(&ruby["bbox"]) <= agent_json_bbox_y(base)
            && agent_json_bbox_bottom(&ruby["bbox"]) >= agent_json_bbox_bottom(base),
        "ruby object bbox should cover expanded base and annotation geometry: {ruby}"
    );
}

#[test]
fn agent_observe_native_renderer_reports_short_vertical_rl_ruby_at_edge() {
    let path = temp_arcw(
        "agent-observe-native-short-vertical-rl-ruby-edge",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天地春夏秋冬|[夢](ゆめ)[/][p]
}
",
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native short vertical_rl ruby edge source");
    assert_native_rich_text_layer_image_has_content(&json);

    let ruby = find_rich_text_ruby_object(&json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ゆめ");
    assert_rich_text_object_has_mask_capture(ruby, "short vertical_rl ruby object");

    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    assert!(
        agent_json_bbox_center_x_twice(annotation) > agent_json_bbox_center_x_twice(base),
        "short vertical_rl ruby annotation should stay on the right side of the base: {ruby}"
    );
    assert!(
        agent_json_bbox_right(annotation)
            <= json["viewport"]["width"].as_u64().expect("viewport width"),
        "short vertical_rl ruby annotation should remain inside the viewport: {ruby}"
    );
}

fn assert_native_vertical_ruby_collision_geometry(writing_mode: &str, ruby_on_right: bool) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-ruby-collision"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬|[夢](ながいよみ)|[星](ながいよみ)[/][p]
}}
"
        ),
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native ruby collision source");
    assert_native_rich_text_layer_image_has_content(&json);

    let first = find_rich_text_ruby_object(&json, 0);
    let second = find_rich_text_ruby_object(&json, 1);
    let first_annotation = &first["rich_text_ref"]["ruby_annotation_bbox"];
    let second_annotation = &second["rich_text_ref"]["ruby_annotation_bbox"];
    assert!(
        !agent_json_bboxes_intersect(first_annotation, second_annotation),
        "{writing_mode} adjacent ruby annotation bboxes should be separated: {first} / {second}"
    );

    for ruby in [first, second] {
        let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
        let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
        if ruby_on_right {
            assert!(
                agent_json_bbox_center_x_twice(annotation) > agent_json_bbox_center_x_twice(base),
                "vertical_rl ruby annotation should be on the right side of the base: {ruby}"
            );
        } else {
            assert!(
                agent_json_bbox_center_x_twice(annotation) < agent_json_bbox_center_x_twice(base),
                "vertical_lr ruby annotation should be on the left side of the base: {ruby}"
            );
        }
    }
}

#[test]
fn agent_observe_native_renderer_reports_expanded_jlreq_pair_geometry() {
    let path = temp_arcw(
        "agent-observe-native-expanded-jlreq-pairs",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=normal]天地春夏秋冬月火…人[/][p]
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reports expanded JLREQ pair geometry");

    fs::remove_file(&path).expect("remove temp expanded JLREQ source");
    assert!(
        output.status.success(),
        "native expanded JLREQ observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("expanded JLREQ report is JSON");
    assert_native_rich_text_layer_image_has_content(&json);

    let dialogue_view = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["role"] == "dialogue_view")
        .expect("dialogue_view object is observed");
    let run = observed_object_rich_text_frame(dialogue_view)["display_map"]["text_runs"]
        .as_array()
        .unwrap()
        .first()
        .expect("text run is observed");
    assert_eq!(run["presentation"]["layout"]["jlreq_strictness"], "normal");

    let fire = find_rich_text_cluster_object(&json, "火", 21, 24);
    let leader = find_rich_text_cluster_object(&json, "…", 24, 27);
    let person = find_rich_text_cluster_object(&json, "人", 27, 30);
    assert_eq!(
        fire["bbox"]["x"], leader["bbox"]["x"],
        "leader should stay in the same native-layout column as the previous cluster"
    );
    assert_eq!(
        leader["bbox"]["x"], person["bbox"]["x"],
        "following text should remain in the same observed column after the leader"
    );
    assert!(
        leader["bbox"]["y"].as_u64().unwrap() > fire["bbox"]["y"].as_u64().unwrap(),
        "leader should advance after the previous cluster within the column"
    );
    assert!(
        person["bbox"]["y"].as_u64().unwrap() > leader["bbox"]["y"].as_u64().unwrap(),
        "text after the leader should advance after the leader within the column"
    );
    assert!(
        leader["capture_refs"]["captures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capture| capture["kind"] == "mask"
                && capture["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with(".mask.rgba"))),
        "expanded JLREQ cluster should expose native mask capture refs"
    );
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_leader_mark_raw_crops() {
    assert_native_jlreq_leader_mark_raw_crop("vertical_rl", "mask");
    assert_native_jlreq_leader_mark_raw_crop("vertical_rl", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_leader_mark_mask_raw_crop() {
    assert_native_jlreq_leader_mark_raw_crop("vertical_lr", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_leader_mark_object_id_raw_crop() {
    assert_native_jlreq_leader_mark_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_vertical_presentation_leader_chain_geometry() {
    assert_native_vertical_presentation_leader_chain_geometry(
        "vertical_rl",
        false,
        "︙",
        "vertical-presentation-leader",
        "vertical presentation leader",
    );
    assert_native_vertical_presentation_leader_chain_geometry(
        "vertical_lr",
        true,
        "︙",
        "vertical-presentation-leader",
        "vertical presentation leader",
    );
}

#[test]
fn agent_observe_native_renderer_writes_vertical_presentation_leader_raw_crops() {
    assert_native_vertical_presentation_leader_raw_crop(
        "vertical_rl",
        false,
        "︙",
        "vertical-presentation-leader",
        "vertical presentation leader",
        "mask",
    );
    assert_native_vertical_presentation_leader_raw_crop(
        "vertical_rl",
        false,
        "︙",
        "vertical-presentation-leader",
        "vertical presentation leader",
        "object-id",
    );
    assert_native_vertical_presentation_leader_raw_crop(
        "vertical_lr",
        true,
        "︙",
        "vertical-presentation-leader",
        "vertical presentation leader",
        "mask",
    );
    assert_native_vertical_presentation_leader_raw_crop(
        "vertical_lr",
        true,
        "︙",
        "vertical-presentation-leader",
        "vertical presentation leader",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_vertical_two_dot_leader_chain_geometry() {
    assert_native_vertical_presentation_leader_chain_geometry(
        "vertical_rl",
        false,
        "︰",
        "vertical-two-dot-leader",
        "vertical two-dot leader",
    );
    assert_native_vertical_presentation_leader_chain_geometry(
        "vertical_lr",
        true,
        "︰",
        "vertical-two-dot-leader",
        "vertical two-dot leader",
    );
}

#[test]
fn agent_observe_native_renderer_writes_vertical_two_dot_leader_raw_crops() {
    assert_native_vertical_presentation_leader_raw_crop(
        "vertical_rl",
        false,
        "︰",
        "vertical-two-dot-leader",
        "vertical two-dot leader",
        "mask",
    );
    assert_native_vertical_presentation_leader_raw_crop(
        "vertical_rl",
        false,
        "︰",
        "vertical-two-dot-leader",
        "vertical two-dot leader",
        "object-id",
    );
    assert_native_vertical_presentation_leader_raw_crop(
        "vertical_lr",
        true,
        "︰",
        "vertical-two-dot-leader",
        "vertical two-dot leader",
        "mask",
    );
    assert_native_vertical_presentation_leader_raw_crop(
        "vertical_lr",
        true,
        "︰",
        "vertical-two-dot-leader",
        "vertical two-dot leader",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_dash_mark_raw_crops() {
    assert_native_jlreq_dash_mark_raw_crop("vertical_rl", "mask");
    assert_native_jlreq_dash_mark_raw_crop("vertical_rl", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_dash_mark_mask_raw_crop() {
    assert_native_jlreq_dash_mark_raw_crop("vertical_lr", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_dash_mark_object_id_raw_crop() {
    assert_native_jlreq_dash_mark_raw_crop("vertical_lr", "object-id");
}

fn assert_native_jlreq_leader_mark_raw_crop(writing_mode: &str, capture_kind: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-jlreq-leader-mark-{capture_kind}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地春夏秋冬月火…人[/][p]
}}
",
        ),
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-{writing_mode}-jlreq-leader-mark-{capture_kind}"
    ));
    let raw_path = dir.join(format!(
        "native-{writing_mode}-jlreq-leader-mark-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.8.24.27")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ leader-mark raw crop");

    assert!(
        output.status.success(),
        "native JLREQ leader-mark {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native JLREQ leader-mark report is JSON");
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

    let leader = assert_native_jlreq_leader_mark_geometry(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], leader["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], leader["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], leader["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], leader["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(leader),
            content_pixels,
            "JLREQ leader-mark object-id crop",
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ leader-mark mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp JLREQ leader-mark source");
    fs::remove_dir_all(&dir).expect("remove temp JLREQ leader-mark dir");
}

fn assert_native_jlreq_leader_mark_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let fire = find_rich_text_cluster_object(json, "火", 21, 24);
    let leader = find_rich_text_cluster_object(json, "…", 24, 27);
    let person = find_rich_text_cluster_object(json, "人", 27, 30);
    assert_eq!(leader["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(leader["rich_text_ref"]["vertical_form"], "none");
    assert_vertical_cluster_after(
        fire,
        leader,
        "leader mark should stay with the previous cluster",
    );
    assert_vertical_cluster_after(
        leader,
        person,
        "text after leader mark should continue in the same column",
    );
    leader
}

fn assert_native_vertical_presentation_leader_chain_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
    leader: &str,
    label: &str,
    description: &str,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-{label}-chain"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天{leader}{leader}人[/][p]
}}
",
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp vertical presentation leader source");
    assert_native_rich_text_layer_image_has_content(&json);
    assert_native_vertical_presentation_leader_geometry(
        &json,
        writing_mode,
        next_column_moves_right,
        leader,
        description,
    );
}

fn assert_native_vertical_presentation_leader_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    leader: &str,
    label: &str,
    description: &str,
    capture_kind: &str,
) {
    let fixture_name = format!("agent-observe-native-{writing_mode}-{label}-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天{leader}{leader}人[/][p]
}}
",
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!("native-{writing_mode}-{label}-{capture_kind}.rgba"));

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
        .expect("arcw agent observe writes native vertical presentation leader raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} {description} {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native vertical presentation leader report is JSON");
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

    let leader = assert_native_vertical_presentation_leader_geometry(
        &json,
        writing_mode,
        next_column_moves_right,
        leader,
        description,
    );
    assert_eq!(json["images"][0]["crop_origin"]["x"], leader["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], leader["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], leader["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], leader["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(leader),
            content_pixels,
            &format!("{writing_mode} {description} object-id crop"),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native vertical presentation leader mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native vertical presentation leader source");
    fs::remove_dir_all(&dir).expect("remove temp native vertical presentation leader dir");
}

fn assert_native_vertical_presentation_leader_geometry<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
    leader: &str,
    description: &str,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["writing_mode"],
        writing_mode
    );
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let first_leader = find_rich_text_cluster_object(json, leader, 3, 6);
    let second_leader = find_rich_text_cluster_object(json, leader, 6, 9);
    let person = find_rich_text_cluster_object(json, "人", 9, 12);
    assert_eq!(second_leader["rich_text_ref"]["orientation"], "upright");
    assert_eq!(second_leader["rich_text_ref"]["vertical_form"], "none");
    assert_vertical_cluster_after(
        first_leader,
        second_leader,
        &format!("{description}s should stay together"),
    );
    assert_next_paragraph_column(
        second_leader,
        person,
        next_column_moves_right,
        "text after vertical presentation leader suffix should start the next column",
    );
    second_leader
}

fn assert_native_jlreq_dash_mark_raw_crop(writing_mode: &str, capture_kind: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-jlreq-dash-mark-{capture_kind}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地春夏秋冬月火――人[/][p]
}}
",
        ),
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-{writing_mode}-jlreq-dash-mark-{capture_kind}"
    ));
    let raw_path = dir.join(format!(
        "native-{writing_mode}-jlreq-dash-mark-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.9.27.30")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ dash-mark raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} JLREQ dash-mark {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native JLREQ dash-mark report is JSON");
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

    let second_dash = assert_native_jlreq_dash_mark_geometry(&json);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        second_dash["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        second_dash["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], second_dash["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], second_dash["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(second_dash),
            content_pixels,
            &format!("{writing_mode} JLREQ dash-mark object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ dash-mark mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp JLREQ dash-mark source");
    fs::remove_dir_all(&dir).expect("remove temp JLREQ dash-mark dir");
}

fn assert_native_jlreq_dash_mark_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let fire = find_rich_text_cluster_object(json, "火", 21, 24);
    let first_dash = find_rich_text_cluster_object(json, "―", 24, 27);
    let second_dash = find_rich_text_cluster_object(json, "―", 27, 30);
    let person = find_rich_text_cluster_object(json, "人", 30, 33);
    assert_eq!(second_dash["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(second_dash["rich_text_ref"]["vertical_form"], "none");
    assert_vertical_cluster_after(
        fire,
        first_dash,
        "dash mark should stay with the previous cluster",
    );
    assert_vertical_cluster_after(
        first_dash,
        second_dash,
        "repeated dash marks should stay together",
    );
    assert_next_paragraph_column(
        second_dash,
        person,
        first_text_run_presentation_layout(json)["writing_mode"] == "vertical_lr",
        "text after repeated dash marks should continue in the next column",
    );
    second_dash
}

#[test]
fn agent_observe_native_renderer_reports_expanded_jlreq_normal_pair_geometry() {
    let path = temp_arcw(
        "agent-observe-native-expanded-jlreq-normal-pairs",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=normal]天地春夏秋冬山々人「」川あっいおーえ[/][p]
}
",
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp expanded normal JLREQ source");
    assert_native_rich_text_layer_image_has_content(&json);

    let dialogue_view = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["role"] == "dialogue_view")
        .expect("dialogue_view object is observed");
    let run = observed_object_rich_text_frame(dialogue_view)["display_map"]["text_runs"]
        .as_array()
        .unwrap()
        .first()
        .expect("text run is observed");
    assert_eq!(run["presentation"]["layout"]["jlreq_strictness"], "normal");

    let mountain = find_rich_text_cluster_object(&json, "山", 18, 21);
    let iteration = find_rich_text_cluster_object(&json, "々", 21, 24);
    let person = find_rich_text_cluster_object(&json, "人", 24, 27);
    assert_vertical_cluster_after(
        mountain,
        iteration,
        "iteration mark should stay with the previous cluster",
    );
    assert_vertical_cluster_after(
        iteration,
        person,
        "text after iteration mark should continue in the same column",
    );

    let open = find_rich_text_cluster_object(&json, "「", 27, 30);
    let close = find_rich_text_cluster_object(&json, "」", 30, 33);
    let river = find_rich_text_cluster_object(&json, "川", 33, 36);
    assert_vertical_cluster_after(open, close, "compact bracket pair should stay together");
    assert_vertical_cluster_after(
        close,
        river,
        "text after compact bracket pair should stay in the same column",
    );

    let large_kana = find_rich_text_cluster_object(&json, "あ", 36, 39);
    let small_kana = find_rich_text_cluster_object(&json, "っ", 39, 42);
    let next_kana = find_rich_text_cluster_object(&json, "い", 42, 45);
    assert_vertical_cluster_after(
        large_kana,
        small_kana,
        "small kana should stay out of a column head",
    );
    assert_vertical_cluster_after(
        small_kana,
        next_kana,
        "text after small kana should continue in the same column",
    );

    let vowel = find_rich_text_cluster_object(&json, "お", 45, 48);
    let prolonged_sound = find_rich_text_cluster_object(&json, "ー", 48, 51);
    let after_dash = find_rich_text_cluster_object(&json, "え", 51, 54);
    assert_vertical_cluster_after(
        vowel,
        prolonged_sound,
        "prolonged sound mark should stay with the previous cluster",
    );
    assert_vertical_cluster_after(
        prolonged_sound,
        after_dash,
        "text after prolonged sound mark should continue in the same column",
    );
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_prolonged_sound_raw_crops() {
    assert_native_jlreq_prolonged_sound_raw_crop("vertical_rl", "mask");
    assert_native_jlreq_prolonged_sound_raw_crop("vertical_rl", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_prolonged_sound_mask_raw_crop() {
    assert_native_jlreq_prolonged_sound_raw_crop("vertical_lr", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_prolonged_sound_object_id_raw_crop() {
    assert_native_jlreq_prolonged_sound_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_halfwidth_middle_dot_and_prolonged_sound_geometry() {
    for (mark, label, description) in [
        ("･", "halfwidth-middle-dot", "halfwidth middle dot"),
        (
            "ｰ",
            "halfwidth-prolonged-sound",
            "halfwidth prolonged-sound mark",
        ),
    ] {
        assert_native_halfwidth_suffix_mark_geometry("vertical_rl", mark, label, description);
        assert_native_halfwidth_suffix_mark_geometry("vertical_lr", mark, label, description);
    }
}

#[test]
fn agent_observe_native_renderer_writes_halfwidth_middle_dot_and_prolonged_sound_raw_crops() {
    for (mark, label, description) in [
        ("･", "halfwidth-middle-dot", "halfwidth middle dot"),
        (
            "ｰ",
            "halfwidth-prolonged-sound",
            "halfwidth prolonged-sound mark",
        ),
    ] {
        for writing_mode in ["vertical_rl", "vertical_lr"] {
            for capture_kind in ["mask", "object-id"] {
                assert_native_halfwidth_suffix_mark_raw_crop(
                    writing_mode,
                    mark,
                    label,
                    description,
                    capture_kind,
                );
            }
        }
    }
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_small_kana_raw_crops() {
    assert_native_jlreq_small_kana_raw_crop("vertical_rl", "mask");
    assert_native_jlreq_small_kana_raw_crop("vertical_rl", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_small_kana_mask_raw_crop() {
    assert_native_jlreq_small_kana_raw_crop("vertical_lr", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_small_kana_object_id_raw_crop() {
    assert_native_jlreq_small_kana_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_halfwidth_small_kana_geometry() {
    assert_native_halfwidth_suffix_mark_geometry(
        "vertical_rl",
        "ｯ",
        "halfwidth-small-kana",
        "halfwidth small kana",
    );
    assert_native_halfwidth_suffix_mark_geometry(
        "vertical_lr",
        "ｯ",
        "halfwidth-small-kana",
        "halfwidth small kana",
    );
}

#[test]
fn agent_observe_native_renderer_writes_halfwidth_small_kana_raw_crops() {
    for writing_mode in ["vertical_rl", "vertical_lr"] {
        for capture_kind in ["mask", "object-id"] {
            assert_native_halfwidth_suffix_mark_raw_crop(
                writing_mode,
                "ｯ",
                "halfwidth-small-kana",
                "halfwidth small kana",
                capture_kind,
            );
        }
    }
}

#[test]
fn agent_observe_native_renderer_reports_katakana_phonetic_extension_small_kana_geometry() {
    assert_native_katakana_phonetic_extension_small_kana_geometry("vertical_rl");
    assert_native_katakana_phonetic_extension_small_kana_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_katakana_phonetic_extension_small_kana_raw_crops() {
    for writing_mode in ["vertical_rl", "vertical_lr"] {
        for capture_kind in ["mask", "object-id"] {
            assert_native_katakana_phonetic_extension_small_kana_raw_crop(
                writing_mode,
                capture_kind,
            );
        }
    }
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_iteration_mark_raw_crops() {
    assert_native_jlreq_iteration_mark_raw_crop("vertical_rl", "mask");
    assert_native_jlreq_iteration_mark_raw_crop("vertical_rl", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_iteration_mark_mask_raw_crop() {
    assert_native_jlreq_iteration_mark_raw_crop("vertical_lr", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_iteration_mark_object_id_raw_crop() {
    assert_native_jlreq_iteration_mark_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_compact_bracket_raw_crops() {
    assert_native_jlreq_compact_bracket_raw_crop("vertical_rl", "mask");
    assert_native_jlreq_compact_bracket_raw_crop("vertical_rl", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_compact_bracket_mask_raw_crop() {
    assert_native_jlreq_compact_bracket_raw_crop("vertical_lr", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_compact_bracket_object_id_raw_crop() {
    assert_native_jlreq_compact_bracket_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_vertical_presentation_compact_bracket_geometry() {
    assert_native_vertical_presentation_compact_bracket_geometry("vertical_rl");
    assert_native_vertical_presentation_compact_bracket_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_presentation_compact_bracket_raw_crops() {
    assert_native_vertical_presentation_compact_bracket_raw_crop("vertical_rl", "mask");
    assert_native_vertical_presentation_compact_bracket_raw_crop("vertical_rl", "object-id");
    assert_native_vertical_presentation_compact_bracket_raw_crop("vertical_lr", "mask");
    assert_native_vertical_presentation_compact_bracket_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_vertical_presentation_curly_and_square_bracket_geometry() {
    for (open, close, label, description) in [
        (
            "︷",
            "︸",
            "vertical-presentation-curly-bracket",
            "vertical presentation curly bracket",
        ),
        (
            "︹",
            "︺",
            "vertical-presentation-tortoise-shell-bracket",
            "vertical presentation tortoise shell bracket",
        ),
        (
            "︻",
            "︼",
            "vertical-presentation-lenticular-bracket",
            "vertical presentation lenticular bracket",
        ),
        (
            "︽",
            "︾",
            "vertical-presentation-double-angle-bracket",
            "vertical presentation double angle bracket",
        ),
        (
            "︿",
            "﹀",
            "vertical-presentation-angle-bracket",
            "vertical presentation angle bracket",
        ),
        (
            "﹁",
            "﹂",
            "vertical-presentation-corner-bracket",
            "vertical presentation corner bracket",
        ),
        (
            "﹃",
            "﹄",
            "vertical-presentation-white-corner-bracket",
            "vertical presentation white corner bracket",
        ),
        (
            "﹇",
            "﹈",
            "vertical-presentation-square-bracket",
            "vertical presentation square bracket",
        ),
    ] {
        assert_native_vertical_presentation_bracket_geometry(
            "vertical_rl",
            open,
            close,
            label,
            description,
        );
        assert_native_vertical_presentation_bracket_geometry(
            "vertical_lr",
            open,
            close,
            label,
            description,
        );
    }
}

#[test]
fn agent_observe_native_renderer_writes_vertical_presentation_curly_and_square_bracket_raw_crops() {
    for (open, close, label, description) in [
        (
            "︷",
            "︸",
            "vertical-presentation-curly-bracket",
            "vertical presentation curly bracket",
        ),
        (
            "︹",
            "︺",
            "vertical-presentation-tortoise-shell-bracket",
            "vertical presentation tortoise shell bracket",
        ),
        (
            "︻",
            "︼",
            "vertical-presentation-lenticular-bracket",
            "vertical presentation lenticular bracket",
        ),
        (
            "︽",
            "︾",
            "vertical-presentation-double-angle-bracket",
            "vertical presentation double angle bracket",
        ),
        (
            "︿",
            "﹀",
            "vertical-presentation-angle-bracket",
            "vertical presentation angle bracket",
        ),
        (
            "﹁",
            "﹂",
            "vertical-presentation-corner-bracket",
            "vertical presentation corner bracket",
        ),
        (
            "﹃",
            "﹄",
            "vertical-presentation-white-corner-bracket",
            "vertical presentation white corner bracket",
        ),
        (
            "﹇",
            "﹈",
            "vertical-presentation-square-bracket",
            "vertical presentation square bracket",
        ),
    ] {
        for writing_mode in ["vertical_rl", "vertical_lr"] {
            for capture_kind in ["mask", "object-id"] {
                assert_native_vertical_presentation_bracket_raw_crop(
                    writing_mode,
                    open,
                    close,
                    label,
                    description,
                    capture_kind,
                );
            }
        }
    }
}

#[test]
fn agent_observe_native_renderer_reports_fullwidth_bracket_pair_geometry() {
    for (open, close, label, description) in [
        ("（", "）", "fullwidth-parenthesis", "fullwidth parenthesis"),
        (
            "［",
            "］",
            "fullwidth-square-bracket",
            "fullwidth square bracket",
        ),
        (
            "｛",
            "｝",
            "fullwidth-curly-bracket",
            "fullwidth curly bracket",
        ),
        (
            "｟",
            "｠",
            "fullwidth-white-parenthesis",
            "fullwidth white parenthesis",
        ),
    ] {
        assert_native_rotated_bracket_geometry("vertical_rl", open, close, label, description);
        assert_native_rotated_bracket_geometry("vertical_lr", open, close, label, description);
    }
}

#[test]
fn agent_observe_native_renderer_writes_fullwidth_bracket_pair_raw_crops() {
    for (open, close, label, description) in [
        ("（", "）", "fullwidth-parenthesis", "fullwidth parenthesis"),
        (
            "［",
            "］",
            "fullwidth-square-bracket",
            "fullwidth square bracket",
        ),
        (
            "｛",
            "｝",
            "fullwidth-curly-bracket",
            "fullwidth curly bracket",
        ),
        (
            "｟",
            "｠",
            "fullwidth-white-parenthesis",
            "fullwidth white parenthesis",
        ),
    ] {
        for writing_mode in ["vertical_rl", "vertical_lr"] {
            for capture_kind in ["mask", "object-id"] {
                assert_native_rotated_bracket_raw_crop(
                    writing_mode,
                    open,
                    close,
                    label,
                    description,
                    capture_kind,
                );
            }
        }
    }
}

#[test]
fn agent_observe_native_renderer_reports_halfwidth_corner_bracket_geometry() {
    assert_native_halfwidth_corner_bracket_geometry("vertical_rl");
    assert_native_halfwidth_corner_bracket_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_halfwidth_corner_bracket_raw_crops() {
    assert_native_halfwidth_corner_bracket_raw_crop("vertical_rl", "mask");
    assert_native_halfwidth_corner_bracket_raw_crop("vertical_rl", "object-id");
    assert_native_halfwidth_corner_bracket_raw_crop("vertical_lr", "mask");
    assert_native_halfwidth_corner_bracket_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_small_parenthesis_geometry() {
    assert_native_rotated_bracket_geometry(
        "vertical_rl",
        "﹙",
        "﹚",
        "small-parenthesis",
        "small parenthesis",
    );
    assert_native_rotated_bracket_geometry(
        "vertical_lr",
        "﹙",
        "﹚",
        "small-parenthesis",
        "small parenthesis",
    );
}

#[test]
fn agent_observe_native_renderer_writes_small_parenthesis_raw_crops() {
    assert_native_rotated_bracket_raw_crop(
        "vertical_rl",
        "﹙",
        "﹚",
        "small-parenthesis",
        "small parenthesis",
        "mask",
    );
    assert_native_rotated_bracket_raw_crop(
        "vertical_rl",
        "﹙",
        "﹚",
        "small-parenthesis",
        "small parenthesis",
        "object-id",
    );
    assert_native_rotated_bracket_raw_crop(
        "vertical_lr",
        "﹙",
        "﹚",
        "small-parenthesis",
        "small parenthesis",
        "mask",
    );
    assert_native_rotated_bracket_raw_crop(
        "vertical_lr",
        "﹙",
        "﹚",
        "small-parenthesis",
        "small parenthesis",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_small_curly_and_tortoise_bracket_geometry() {
    for (open, close, label, description) in [
        ("﹛", "﹜", "small-curly-bracket", "small curly bracket"),
        (
            "﹝",
            "﹞",
            "small-tortoise-shell-bracket",
            "small tortoise shell bracket",
        ),
    ] {
        assert_native_rotated_bracket_geometry("vertical_rl", open, close, label, description);
        assert_native_rotated_bracket_geometry("vertical_lr", open, close, label, description);
    }
}

#[test]
fn agent_observe_native_renderer_writes_small_curly_and_tortoise_bracket_raw_crops() {
    for (open, close, label, description) in [
        ("﹛", "﹜", "small-curly-bracket", "small curly bracket"),
        (
            "﹝",
            "﹞",
            "small-tortoise-shell-bracket",
            "small tortoise shell bracket",
        ),
    ] {
        for writing_mode in ["vertical_rl", "vertical_lr"] {
            for capture_kind in ["mask", "object-id"] {
                assert_native_rotated_bracket_raw_crop(
                    writing_mode,
                    open,
                    close,
                    label,
                    description,
                    capture_kind,
                );
            }
        }
    }
}

fn assert_native_jlreq_prolonged_sound_raw_crop(writing_mode: &str, capture_kind: &str) {
    let source = format!(
        r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地春夏秋冬山々人「」川あっいおーえ[/][p]
}}
"
    );
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-jlreq-prolonged-sound-{capture_kind}"),
        &source,
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-{writing_mode}-jlreq-prolonged-sound-{capture_kind}"
    ));
    let raw_path = dir.join(format!(
        "native-{writing_mode}-jlreq-prolonged-sound-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.16.48.51")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ prolonged-sound raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} JLREQ prolonged-sound {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native JLREQ prolonged-sound report is JSON");
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

    let prolonged_sound = assert_native_jlreq_prolonged_sound_geometry(&json);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        prolonged_sound["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        prolonged_sound["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], prolonged_sound["bbox"]["width"]);
    assert_eq!(
        json["images"][0]["height"],
        prolonged_sound["bbox"]["height"]
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(prolonged_sound),
            content_pixels,
            &format!("{writing_mode} JLREQ prolonged-sound object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ prolonged-sound mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp JLREQ prolonged-sound source");
    fs::remove_dir_all(&dir).expect("remove temp JLREQ prolonged-sound dir");
}

fn assert_native_jlreq_small_kana_raw_crop(writing_mode: &str, capture_kind: &str) {
    let source = format!(
        r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地春夏秋冬山々人「」川あっいおーえ[/][p]
}}
"
    );
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-jlreq-small-kana-{capture_kind}"),
        &source,
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-{writing_mode}-jlreq-small-kana-{capture_kind}"
    ));
    let raw_path = dir.join(format!(
        "native-{writing_mode}-jlreq-small-kana-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.13.39.42")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ small-kana raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} JLREQ small-kana {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native JLREQ small-kana report is JSON");
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

    let small_kana = assert_native_jlreq_small_kana_geometry(&json);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        small_kana["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        small_kana["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], small_kana["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], small_kana["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(small_kana),
            content_pixels,
            &format!("{writing_mode} JLREQ small-kana object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ small-kana mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp JLREQ small-kana source");
    fs::remove_dir_all(&dir).expect("remove temp JLREQ small-kana dir");
}

fn assert_native_jlreq_iteration_mark_raw_crop(writing_mode: &str, capture_kind: &str) {
    let source = format!(
        r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地春夏秋冬山々人「」川あっいおーえ[/][p]
}}
"
    );
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-jlreq-iteration-mark-{capture_kind}"),
        &source,
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-{writing_mode}-jlreq-iteration-mark-{capture_kind}"
    ));
    let raw_path = dir.join(format!(
        "native-{writing_mode}-jlreq-iteration-mark-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.7.21.24")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ iteration-mark raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} JLREQ iteration-mark {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native JLREQ iteration-mark report is JSON");
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

    let iteration = assert_native_jlreq_iteration_mark_geometry(&json);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        iteration["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        iteration["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], iteration["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], iteration["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(iteration),
            content_pixels,
            &format!("{writing_mode} JLREQ iteration-mark object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ iteration-mark mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp JLREQ iteration-mark source");
    fs::remove_dir_all(&dir).expect("remove temp JLREQ iteration-mark dir");
}

fn assert_native_jlreq_compact_bracket_raw_crop(writing_mode: &str, capture_kind: &str) {
    let source = format!(
        r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地春夏秋冬山々人「」川あっいおーえ[/][p]
}}
"
    );
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-jlreq-compact-bracket-{capture_kind}"),
        &source,
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-{writing_mode}-jlreq-compact-bracket-{capture_kind}"
    ));
    let raw_path = dir.join(format!(
        "native-{writing_mode}-jlreq-compact-bracket-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.10.30.33")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ compact-bracket raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} JLREQ compact-bracket {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native JLREQ compact-bracket report is JSON");
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

    let close = assert_native_jlreq_compact_bracket_geometry(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], close["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], close["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], close["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], close["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(close),
            content_pixels,
            &format!("{writing_mode} JLREQ compact-bracket object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ compact-bracket mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp JLREQ compact-bracket source");
    fs::remove_dir_all(&dir).expect("remove temp JLREQ compact-bracket dir");
}

fn assert_native_jlreq_prolonged_sound_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let vowel = find_rich_text_cluster_object(json, "お", 45, 48);
    let prolonged_sound = find_rich_text_cluster_object(json, "ー", 48, 51);
    let after_dash = find_rich_text_cluster_object(json, "え", 51, 54);
    assert_eq!(
        prolonged_sound["rich_text_ref"]["orientation"],
        "sideways_cw"
    );
    assert_eq!(
        prolonged_sound["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    assert_vertical_cluster_after(
        vowel,
        prolonged_sound,
        "prolonged sound mark should stay with the previous cluster",
    );
    assert_vertical_cluster_after(
        prolonged_sound,
        after_dash,
        "text after prolonged sound mark should continue in the same column",
    );
    prolonged_sound
}

fn assert_native_jlreq_small_kana_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let large_kana = find_rich_text_cluster_object(json, "あ", 36, 39);
    let small_kana = find_rich_text_cluster_object(json, "っ", 39, 42);
    let next_kana = find_rich_text_cluster_object(json, "い", 42, 45);
    assert_eq!(small_kana["rich_text_ref"]["orientation"], "upright");
    assert_eq!(
        small_kana["rich_text_ref"]["vertical_form"],
        "upright_alternate"
    );
    assert_vertical_cluster_after(
        large_kana,
        small_kana,
        "small kana should stay out of a column head",
    );
    assert_vertical_cluster_after(
        small_kana,
        next_kana,
        "text after small kana should continue in the same column",
    );
    small_kana
}

fn assert_native_jlreq_iteration_mark_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let mountain = find_rich_text_cluster_object(json, "山", 18, 21);
    let iteration = find_rich_text_cluster_object(json, "々", 21, 24);
    let person = find_rich_text_cluster_object(json, "人", 24, 27);
    assert_eq!(iteration["rich_text_ref"]["orientation"], "upright");
    assert_eq!(iteration["rich_text_ref"]["vertical_form"], "none");
    assert_vertical_cluster_after(
        mountain,
        iteration,
        "iteration mark should stay with the previous cluster",
    );
    assert_vertical_cluster_after(
        iteration,
        person,
        "text after iteration mark should continue in the same column",
    );
    iteration
}

fn assert_native_jlreq_compact_bracket_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let open = find_rich_text_cluster_object(json, "「", 27, 30);
    let close = find_rich_text_cluster_object(json, "」", 30, 33);
    let river = find_rich_text_cluster_object(json, "川", 33, 36);
    assert_eq!(close["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(close["rich_text_ref"]["vertical_form"], "rotated_alternate");
    assert_vertical_cluster_after(open, close, "compact bracket pair should stay together");
    assert_vertical_cluster_after(
        close,
        river,
        "text after compact bracket pair should stay in the same column",
    );
    close
}

fn assert_native_vertical_presentation_compact_bracket_geometry(writing_mode: &str) {
    assert_native_vertical_presentation_bracket_geometry(
        writing_mode,
        "︵",
        "︶",
        "vertical-presentation-compact-bracket",
        "vertical presentation compact bracket",
    );
}

fn assert_native_vertical_presentation_compact_bracket_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    assert_native_vertical_presentation_bracket_raw_crop(
        writing_mode,
        "︵",
        "︶",
        "vertical-presentation-compact-bracket",
        "vertical presentation compact bracket",
        capture_kind,
    );
}

fn assert_native_vertical_presentation_bracket_geometry(
    writing_mode: &str,
    open: &str,
    close: &str,
    label: &str,
    description: &str,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-{label}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天{open}{close}人[/][p]
}}
",
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp vertical presentation compact bracket source");
    assert_native_rich_text_layer_image_has_content(&json);
    assert_native_vertical_presentation_bracket_object(
        &json,
        writing_mode,
        open,
        close,
        description,
    );
}

fn assert_native_vertical_presentation_bracket_raw_crop(
    writing_mode: &str,
    open: &str,
    close: &str,
    label: &str,
    description: &str,
    capture_kind: &str,
) {
    let fixture_name = format!("agent-observe-native-{writing_mode}-{label}-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天{open}{close}人[/][p]
}}
",
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!("native-{writing_mode}-{label}-{capture_kind}.rgba"));

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
        .expect("arcw agent observe writes native vertical presentation compact-bracket raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} {description} {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native vertical presentation compact-bracket report is JSON");
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

    let close_object = assert_native_vertical_presentation_bracket_object(
        &json,
        writing_mode,
        open,
        close,
        description,
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        close_object["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        close_object["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], close_object["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], close_object["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(close_object),
            content_pixels,
            &format!("{writing_mode} {description} object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path)
            .expect("read native vertical presentation compact-bracket mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path)
        .expect("remove temp native vertical presentation compact bracket source");
    fs::remove_dir_all(&dir).expect("remove temp native vertical presentation compact bracket dir");
}

fn assert_native_vertical_presentation_bracket_object<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    open: &str,
    close: &str,
    description: &str,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["writing_mode"],
        writing_mode
    );
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let open_object = find_rich_text_cluster_object(json, open, 3, 6);
    let close_object = find_rich_text_cluster_object(json, close, 6, 9);
    let person = find_rich_text_cluster_object(json, "人", 9, 12);
    assert_eq!(close_object["rich_text_ref"]["orientation"], "upright");
    assert_eq!(close_object["rich_text_ref"]["vertical_form"], "none");
    assert_vertical_cluster_after(
        body,
        open_object,
        &format!("{description} opening mark should sit after body text"),
    );
    assert_vertical_cluster_after(
        open_object,
        close_object,
        &format!("{description} pair should stay together"),
    );
    assert_vertical_cluster_after(
        close_object,
        person,
        &format!("text after {description} pair should continue in the same column"),
    );
    close_object
}

fn assert_native_halfwidth_corner_bracket_geometry(writing_mode: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-halfwidth-corner-bracket"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天｢｣人[/][p]
}}
",
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp halfwidth corner bracket source");
    assert_native_rich_text_layer_image_has_content(&json);
    assert_native_halfwidth_corner_bracket_object(&json, writing_mode);
}

fn assert_native_halfwidth_corner_bracket_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name =
        format!("agent-observe-native-{writing_mode}-halfwidth-corner-bracket-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天｢｣人[/][p]
}}
",
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-halfwidth-corner-bracket-{capture_kind}.rgba"
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
        .expect("arcw agent observe writes native halfwidth corner-bracket raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} halfwidth corner-bracket {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native halfwidth corner-bracket report is JSON");
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

    let close = assert_native_halfwidth_corner_bracket_object(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], close["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], close["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], close["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], close["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(close),
            content_pixels,
            &format!("{writing_mode} halfwidth corner-bracket object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native halfwidth corner-bracket mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native halfwidth corner bracket source");
    fs::remove_dir_all(&dir).expect("remove temp native halfwidth corner bracket dir");
}

fn assert_native_halfwidth_corner_bracket_object<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["writing_mode"],
        writing_mode
    );
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let open = find_rich_text_cluster_object(json, "｢", 3, 6);
    let close = find_rich_text_cluster_object(json, "｣", 6, 9);
    let person = find_rich_text_cluster_object(json, "人", 9, 12);
    assert_eq!(close["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(close["rich_text_ref"]["vertical_form"], "none");
    assert_vertical_cluster_after(
        body,
        open,
        "halfwidth opening corner bracket should sit after body text",
    );
    assert_vertical_cluster_after(
        open,
        close,
        "halfwidth corner bracket pair should stay together",
    );
    assert_vertical_cluster_after(
        close,
        person,
        "text after halfwidth corner bracket pair should continue in the same column",
    );
    close
}

fn assert_native_rotated_bracket_geometry(
    writing_mode: &str,
    open: &str,
    close: &str,
    label: &str,
    description: &str,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-{label}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天{open}{close}人[/][p]
}}
",
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp rotated bracket source");
    assert_native_rich_text_layer_image_has_content(&json);
    assert_native_rotated_bracket_object(&json, writing_mode, open, close, description);
}

fn assert_native_rotated_bracket_raw_crop(
    writing_mode: &str,
    open: &str,
    close: &str,
    label: &str,
    description: &str,
    capture_kind: &str,
) {
    let fixture_name = format!("agent-observe-native-{writing_mode}-{label}-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天{open}{close}人[/][p]
}}
",
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!("native-{writing_mode}-{label}-{capture_kind}.rgba"));

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
        .expect("arcw agent observe writes native rotated-bracket raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} {description} {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native rotated-bracket report is JSON");
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

    let close_object =
        assert_native_rotated_bracket_object(&json, writing_mode, open, close, description);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        close_object["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        close_object["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], close_object["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], close_object["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(close_object),
            content_pixels,
            &format!("{writing_mode} {description} object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native rotated-bracket mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native rotated bracket source");
    fs::remove_dir_all(&dir).expect("remove temp native rotated bracket dir");
}

fn assert_native_rotated_bracket_object<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    open: &str,
    close: &str,
    description: &str,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["writing_mode"],
        writing_mode
    );
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let body = find_rich_text_cluster_object(json, "天", 0, 3);
    let open_object = find_rich_text_cluster_object(json, open, 3, 6);
    let close_object = find_rich_text_cluster_object(json, close, 6, 9);
    let person = find_rich_text_cluster_object(json, "人", 9, 12);
    assert_eq!(close_object["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(
        close_object["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    assert_vertical_cluster_after(
        body,
        open_object,
        &format!("{description} opening mark should sit after body text"),
    );
    assert_vertical_cluster_after(
        open_object,
        close_object,
        &format!("{description} pair should stay together"),
    );
    assert_vertical_cluster_after(
        close_object,
        person,
        &format!("text after {description} pair should continue in the same column"),
    );
    close_object
}

fn assert_native_halfwidth_suffix_mark_geometry(
    writing_mode: &str,
    mark: &str,
    label: &str,
    description: &str,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-{label}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地{mark}人[/][p]
}}
",
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp halfwidth suffix-mark source");
    assert_native_rich_text_layer_image_has_content(&json);
    assert_native_halfwidth_suffix_mark_object(&json, writing_mode, mark, description);
}

fn assert_native_halfwidth_suffix_mark_raw_crop(
    writing_mode: &str,
    mark: &str,
    label: &str,
    description: &str,
    capture_kind: &str,
) {
    let fixture_name = format!("agent-observe-native-{writing_mode}-{label}-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地{mark}人[/][p]
}}
",
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!("native-{writing_mode}-{label}-{capture_kind}.rgba"));

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
        .expect("arcw agent observe writes native halfwidth suffix-mark raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} {description} {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native halfwidth suffix-mark report is JSON");
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

    let mark_object =
        assert_native_halfwidth_suffix_mark_object(&json, writing_mode, mark, description);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        mark_object["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        mark_object["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], mark_object["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], mark_object["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(mark_object),
            content_pixels,
            &format!("{writing_mode} {description} object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native halfwidth suffix-mark mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native halfwidth suffix-mark source");
    fs::remove_dir_all(&dir).expect("remove temp native halfwidth suffix-mark dir");
}

fn assert_native_halfwidth_suffix_mark_object<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    mark: &str,
    description: &str,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["writing_mode"],
        writing_mode
    );
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let earth = find_rich_text_cluster_object(json, "地", 3, 6);
    let mark_object = find_rich_text_cluster_object(json, mark, 6, 9);
    let person = find_rich_text_cluster_object(json, "人", 9, 12);
    assert_eq!(mark_object["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(mark_object["rich_text_ref"]["vertical_form"], "none");
    assert_vertical_cluster_after(
        earth,
        mark_object,
        &format!("{description} should stay after the previous cluster"),
    );
    if mark_object["bbox"]["x"] == person["bbox"]["x"] {
        assert!(
            agent_json_bbox_y(&person["bbox"]) > agent_json_bbox_y(&mark_object["bbox"]),
            "text after halfwidth suffix mark should advance within the same observed column"
        );
    } else {
        assert!(
            agent_json_bbox_y(&person["bbox"]) < agent_json_bbox_y(&mark_object["bbox"]),
            "text after halfwidth suffix mark should restart near the column top"
        );
    }
    mark_object
}

fn assert_native_katakana_phonetic_extension_small_kana_geometry(writing_mode: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-katakana-phonetic-extension-small-kana"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地ㇰ人[/][p]
}}
",
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp katakana phonetic extension small-kana source");
    assert_native_rich_text_layer_image_has_content(&json);
    assert_native_katakana_phonetic_extension_small_kana_object(&json, writing_mode);
}

fn assert_native_katakana_phonetic_extension_small_kana_raw_crop(
    writing_mode: &str,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-katakana-phonetic-extension-small-kana-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=normal]天地ㇰ人[/][p]
}}
",
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-katakana-phonetic-extension-small-kana-{capture_kind}.rgba"
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
        .expect("arcw agent observe writes native katakana phonetic extension small-kana raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} katakana phonetic extension small-kana {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native katakana phonetic extension small-kana report is JSON");
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

    let mark = assert_native_katakana_phonetic_extension_small_kana_object(&json, writing_mode);
    assert_eq!(json["images"][0]["crop_origin"]["x"], mark["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], mark["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], mark["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], mark["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(mark),
            content_pixels,
            &format!("{writing_mode} katakana phonetic extension small-kana object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path)
            .expect("read native katakana phonetic extension small-kana mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path)
        .expect("remove temp native katakana phonetic extension small-kana source");
    fs::remove_dir_all(&dir)
        .expect("remove temp native katakana phonetic extension small-kana dir");
}

fn assert_native_katakana_phonetic_extension_small_kana_object<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["writing_mode"],
        writing_mode
    );
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let earth = find_rich_text_cluster_object(json, "地", 3, 6);
    let mark = find_rich_text_cluster_object(json, "ㇰ", 6, 9);
    let person = find_rich_text_cluster_object(json, "人", 9, 12);
    assert_eq!(mark["rich_text_ref"]["orientation"], "upright");
    assert_eq!(mark["rich_text_ref"]["vertical_form"], "upright_alternate");
    assert_vertical_cluster_after(
        earth,
        mark,
        "katakana phonetic extension small kana should stay after the previous cluster",
    );
    if mark["bbox"]["x"] == person["bbox"]["x"] {
        assert!(
            agent_json_bbox_y(&person["bbox"]) > agent_json_bbox_y(&mark["bbox"]),
            "text after katakana phonetic extension small kana should advance within the same observed column"
        );
    } else {
        assert!(
            agent_json_bbox_y(&person["bbox"]) < agent_json_bbox_y(&mark["bbox"]),
            "text after katakana phonetic extension small kana should restart near the column top"
        );
    }
    mark
}

#[test]
fn agent_observe_native_renderer_reports_strict_jlreq_middle_dot_pair_geometry() {
    let path = temp_arcw(
        "agent-observe-native-strict-jlreq-middle-dot-pair",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=strict]天地春夏秋冬月火中・外[/][p]
}
",
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp strict JLREQ source");
    assert_native_rich_text_layer_image_has_content(&json);

    let dialogue_view = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["role"] == "dialogue_view")
        .expect("dialogue_view object is observed");
    let run = observed_object_rich_text_frame(dialogue_view)["display_map"]["text_runs"]
        .as_array()
        .unwrap()
        .first()
        .expect("text run is observed");
    assert_eq!(run["presentation"]["layout"]["jlreq_strictness"], "strict");

    let inside = find_rich_text_cluster_object(&json, "中", 24, 27);
    let middle_dot = find_rich_text_cluster_object(&json, "・", 27, 30);
    let outside = find_rich_text_cluster_object(&json, "外", 30, 33);
    assert_vertical_cluster_after(
        inside,
        middle_dot,
        "strict middle-dot pair should stay in the same native-layout column",
    );
    assert_vertical_cluster_after(
        middle_dot,
        outside,
        "text after strict middle dot should remain in the same observed column",
    );
}

#[test]
fn agent_observe_native_renderer_reports_strict_jlreq_middle_dot_opening_pair_geometry() {
    assert_native_jlreq_middle_dot_opening_pair_plan("vertical_rl", false);
    assert_native_jlreq_middle_dot_opening_pair_plan("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_strict_jlreq_middle_dot_raw_crops() {
    assert_native_strict_jlreq_middle_dot_raw_crop("vertical_rl", "mask");
    assert_native_strict_jlreq_middle_dot_raw_crop("vertical_rl", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_strict_jlreq_middle_dot_mask_raw_crop() {
    assert_native_strict_jlreq_middle_dot_raw_crop("vertical_lr", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_strict_jlreq_middle_dot_object_id_raw_crop() {
    assert_native_strict_jlreq_middle_dot_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_strict_jlreq_middle_dot_opening_raw_crops() {
    assert_native_strict_jlreq_middle_dot_opening_raw_crop("vertical_rl", false, "mask");
    assert_native_strict_jlreq_middle_dot_opening_raw_crop("vertical_rl", false, "object-id");
    assert_native_strict_jlreq_middle_dot_opening_raw_crop("vertical_lr", true, "mask");
    assert_native_strict_jlreq_middle_dot_opening_raw_crop("vertical_lr", true, "object-id");
}

fn assert_native_jlreq_middle_dot_opening_pair_plan(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let loose = observe_native_jlreq_middle_dot_opening_fixture(writing_mode, "loose");
    let strict = observe_native_jlreq_middle_dot_opening_fixture(writing_mode, "strict");
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

    let loose_middle_dot = find_rich_text_cluster_object(&loose, "・", 6, 9);
    let loose_open = find_rich_text_cluster_object(&loose, "「", 9, 12);
    assert_next_paragraph_column(
        loose_middle_dot,
        loose_open,
        next_column_moves_right,
        "loose native paragraph plan may break between middle dot and opening punctuation",
    );

    assert_native_strict_jlreq_middle_dot_opening_geometry(
        &strict,
        writing_mode,
        next_column_moves_right,
    );
}

fn observe_native_jlreq_middle_dot_opening_fixture(
    writing_mode: &str,
    strictness: &str,
) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-jlreq-middle-dot-opening-{strictness}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq={strictness}]天地・「人山川海[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp middle-dot/opening JLREQ source");
    json
}

fn assert_native_strict_jlreq_middle_dot_opening_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    capture_kind: &str,
) {
    let fixture_name = format!(
        "agent-observe-native-{writing_mode}-strict-jlreq-middle-dot-opening-{capture_kind}"
    );
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地・「人山川海[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-strict-jlreq-middle-dot-opening-{capture_kind}.rgba"
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
        .expect("arcw agent observe writes native strict JLREQ middle-dot/opening raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} strict JLREQ middle-dot/opening {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native strict JLREQ middle-dot/opening report is JSON");
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

    let open = assert_native_strict_jlreq_middle_dot_opening_geometry(
        &json,
        writing_mode,
        next_column_moves_right,
    );
    assert_eq!(json["images"][0]["crop_origin"]["x"], open["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], open["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], open["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], open["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(open),
            content_pixels,
            &format!("{writing_mode} strict JLREQ middle-dot/opening object-id crop"),
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native strict JLREQ middle-dot/opening mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp strict JLREQ middle-dot/opening source");
    fs::remove_dir_all(&dir).expect("remove temp strict JLREQ middle-dot/opening dir");
}

fn assert_native_strict_jlreq_middle_dot_raw_crop(writing_mode: &str, capture_kind: &str) {
    let source = format!(
        r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode} jlreq=strict]天地春夏秋冬月火中・外[/][p]
}}
"
    );
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-strict-jlreq-middle-dot-{capture_kind}"),
        &source,
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-{writing_mode}-strict-jlreq-middle-dot-{capture_kind}"
    ));
    let raw_path = dir.join(format!(
        "native-{writing_mode}-strict-jlreq-middle-dot-{capture_kind}.rgba"
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
        .arg("object.dialogue.0.0.cluster.9.27.30")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native strict JLREQ middle-dot raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} strict JLREQ middle-dot {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native strict JLREQ middle-dot report is JSON");
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

    let middle_dot = assert_native_strict_jlreq_middle_dot_geometry(&json);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        middle_dot["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        middle_dot["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], middle_dot["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], middle_dot["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(middle_dot),
            content_pixels,
            &format!("{writing_mode} strict JLREQ middle-dot object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native strict JLREQ middle-dot mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp strict JLREQ middle-dot source");
    fs::remove_dir_all(&dir).expect("remove temp strict JLREQ middle-dot dir");
}

fn assert_native_strict_jlreq_middle_dot_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "strict"
    );
    let inside = find_rich_text_cluster_object(json, "中", 24, 27);
    let middle_dot = find_rich_text_cluster_object(json, "・", 27, 30);
    let outside = find_rich_text_cluster_object(json, "外", 30, 33);
    assert_eq!(middle_dot["rich_text_ref"]["orientation"], "upright");
    assert_eq!(middle_dot["rich_text_ref"]["vertical_form"], "none");
    assert_vertical_cluster_after(
        inside,
        middle_dot,
        "strict middle-dot pair should stay in the same native-layout column",
    );
    assert_vertical_cluster_after(
        middle_dot,
        outside,
        "text after strict middle dot should remain in the same observed column",
    );
    middle_dot
}

fn assert_native_strict_jlreq_middle_dot_opening_geometry<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["writing_mode"],
        writing_mode
    );
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "strict"
    );
    let earth = find_rich_text_cluster_object(json, "地", 3, 6);
    let middle_dot = find_rich_text_cluster_object(json, "・", 6, 9);
    let open = find_rich_text_cluster_object(json, "「", 9, 12);
    let person = find_rich_text_cluster_object(json, "人", 12, 15);
    let mountain = find_rich_text_cluster_object(json, "山", 15, 18);
    assert_eq!(middle_dot["rich_text_ref"]["orientation"], "upright");
    assert_eq!(middle_dot["rich_text_ref"]["vertical_form"], "none");
    assert_eq!(open["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(open["rich_text_ref"]["vertical_form"], "rotated_alternate");
    assert_vertical_cluster_after(
        earth,
        middle_dot,
        "strict middle-dot/opening fixture should keep middle dot after body text",
    );
    assert_vertical_cluster_after(
        middle_dot,
        open,
        "strict middle-dot/opening pair should stay in the same native-layout column",
    );
    assert_vertical_cluster_after(
        open,
        person,
        "text after strict middle-dot/opening pair should remain in the same observed column",
    );
    assert_next_paragraph_column(
        person,
        mountain,
        next_column_moves_right,
        "strict middle-dot/opening paragraph should continue in the next column after the attached base",
    );
    open
}

#[test]
fn agent_observe_native_renderer_reports_jlreq_punctuation_compression_and_hanging() {
    let hanging_path = temp_arcw(
        "agent-observe-native-jlreq-hanging-punctuation",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天地、人人[/][p]
}
",
    );
    let hanging = observe_native_rich_text_layer_report(&hanging_path);
    fs::remove_file(&hanging_path).expect("remove temp hanging punctuation source");
    assert_native_rich_text_layer_image_has_content(&hanging);

    let earth = find_rich_text_cluster_object(&hanging, "地", 3, 6);
    let comma = find_rich_text_cluster_object(&hanging, "、", 6, 9);
    let next_person = find_rich_text_cluster_object(&hanging, "人", 9, 12);
    assert_eq!(
        earth["bbox"]["x"], comma["bbox"]["x"],
        "hanging punctuation should remain in the previous column"
    );
    assert!(
        agent_json_bbox_y(&comma["bbox"]) > agent_json_bbox_y(&earth["bbox"]),
        "hanging punctuation should sit after the previous cluster"
    );
    assert!(
        agent_json_bbox_x(&next_person["bbox"]) < agent_json_bbox_x(&comma["bbox"])
            && agent_json_bbox_y(&next_person["bbox"]) < agent_json_bbox_y(&comma["bbox"]),
        "text after hanging punctuation should start the next vertical_rl column"
    );

    let compression_path = temp_arcw(
        "agent-observe-native-jlreq-punctuation-compression",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天、。・人[/][p]
}
",
    );
    let compression = observe_native_rich_text_layer_report(&compression_path);
    fs::remove_file(&compression_path).expect("remove temp punctuation compression source");
    assert_native_rich_text_layer_image_has_content(&compression);

    let first = find_rich_text_cluster_object(&compression, "天", 0, 3);
    let comma = find_rich_text_cluster_object(&compression, "、", 3, 6);
    let period = find_rich_text_cluster_object(&compression, "。", 6, 9);
    let middle_dot = find_rich_text_cluster_object(&compression, "・", 9, 12);
    let person = find_rich_text_cluster_object(&compression, "人", 12, 15);
    assert_eq!(first["bbox"]["x"], comma["bbox"]["x"]);
    assert_eq!(comma["bbox"]["x"], period["bbox"]["x"]);
    assert_eq!(period["bbox"]["x"], middle_dot["bbox"]["x"]);
    assert_eq!(middle_dot["bbox"]["x"], person["bbox"]["x"]);
    let body_advance = agent_json_bbox_y(&comma["bbox"]) - agent_json_bbox_y(&first["bbox"]);
    let compressed_advance = agent_json_bbox_y(&period["bbox"]) - agent_json_bbox_y(&comma["bbox"]);
    assert_eq!(
        compressed_advance * 2,
        body_advance,
        "compressed punctuation should advance by half a body cell"
    );
    assert_eq!(
        agent_json_bbox_y(&middle_dot["bbox"]) - agent_json_bbox_y(&period["bbox"]),
        compressed_advance,
        "middle dot should continue the compressed punctuation chain"
    );
    assert_eq!(
        agent_json_bbox_y(&person["bbox"]) - agent_json_bbox_y(&middle_dot["bbox"]),
        compressed_advance,
        "following text should consume the space left by punctuation compression"
    );
}

#[test]
fn agent_observe_native_renderer_reports_fullwidth_question_mark_hanging_punctuation() {
    assert_native_closing_punctuation_hanging_geometry(
        "vertical_rl",
        false,
        "？",
        "fullwidth-question",
        "fullwidth question mark",
    );
    assert_native_closing_punctuation_hanging_geometry(
        "vertical_lr",
        true,
        "？",
        "fullwidth-question",
        "fullwidth question mark",
    );
}

#[test]
fn agent_observe_native_renderer_reports_fullwidth_colon_semicolon_hanging_punctuation() {
    for (mark, label, description) in [
        ("：", "fullwidth-colon", "fullwidth colon"),
        ("；", "fullwidth-semicolon", "fullwidth semicolon"),
    ] {
        assert_native_closing_punctuation_hanging_geometry(
            "vertical_rl",
            false,
            mark,
            label,
            description,
        );
        assert_native_closing_punctuation_hanging_geometry(
            "vertical_lr",
            true,
            mark,
            label,
            description,
        );
    }
}

#[test]
fn agent_observe_native_renderer_reports_halfwidth_full_stop_hanging_punctuation() {
    assert_native_closing_punctuation_hanging_geometry(
        "vertical_rl",
        false,
        "｡",
        "halfwidth-full-stop",
        "halfwidth full stop",
    );
    assert_native_closing_punctuation_hanging_geometry(
        "vertical_lr",
        true,
        "｡",
        "halfwidth-full-stop",
        "halfwidth full stop",
    );
}

#[test]
fn agent_observe_native_renderer_reports_halfwidth_ideographic_comma_hanging_punctuation() {
    assert_native_closing_punctuation_hanging_geometry(
        "vertical_rl",
        false,
        "､",
        "halfwidth-ideographic-comma",
        "halfwidth ideographic comma",
    );
    assert_native_closing_punctuation_hanging_geometry(
        "vertical_lr",
        true,
        "､",
        "halfwidth-ideographic-comma",
        "halfwidth ideographic comma",
    );
}

#[test]
fn agent_observe_native_renderer_writes_fullwidth_question_mark_raw_crops() {
    assert_native_closing_punctuation_raw_crop(
        "vertical_rl",
        false,
        "？",
        "fullwidth-question",
        "fullwidth question mark",
        "mask",
    );
    assert_native_closing_punctuation_raw_crop(
        "vertical_rl",
        false,
        "？",
        "fullwidth-question",
        "fullwidth question mark",
        "object-id",
    );
    assert_native_closing_punctuation_raw_crop(
        "vertical_lr",
        true,
        "？",
        "fullwidth-question",
        "fullwidth question mark",
        "mask",
    );
    assert_native_closing_punctuation_raw_crop(
        "vertical_lr",
        true,
        "？",
        "fullwidth-question",
        "fullwidth question mark",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_writes_fullwidth_colon_semicolon_raw_crops() {
    for (mark, label, description) in [
        ("：", "fullwidth-colon", "fullwidth colon"),
        ("；", "fullwidth-semicolon", "fullwidth semicolon"),
    ] {
        for (writing_mode, next_column_moves_right) in
            [("vertical_rl", false), ("vertical_lr", true)]
        {
            for capture_kind in ["mask", "object-id"] {
                assert_native_closing_punctuation_raw_crop(
                    writing_mode,
                    next_column_moves_right,
                    mark,
                    label,
                    description,
                    capture_kind,
                );
            }
        }
    }
}

#[test]
fn agent_observe_native_renderer_writes_halfwidth_ideographic_comma_raw_crops() {
    assert_native_closing_punctuation_raw_crop(
        "vertical_rl",
        false,
        "､",
        "halfwidth-ideographic-comma",
        "halfwidth ideographic comma",
        "mask",
    );
    assert_native_closing_punctuation_raw_crop(
        "vertical_rl",
        false,
        "､",
        "halfwidth-ideographic-comma",
        "halfwidth ideographic comma",
        "object-id",
    );
    assert_native_closing_punctuation_raw_crop(
        "vertical_lr",
        true,
        "､",
        "halfwidth-ideographic-comma",
        "halfwidth ideographic comma",
        "mask",
    );
    assert_native_closing_punctuation_raw_crop(
        "vertical_lr",
        true,
        "､",
        "halfwidth-ideographic-comma",
        "halfwidth ideographic comma",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_writes_halfwidth_full_stop_raw_crops() {
    assert_native_closing_punctuation_raw_crop(
        "vertical_rl",
        false,
        "｡",
        "halfwidth-full-stop",
        "halfwidth full stop",
        "mask",
    );
    assert_native_closing_punctuation_raw_crop(
        "vertical_rl",
        false,
        "｡",
        "halfwidth-full-stop",
        "halfwidth full stop",
        "object-id",
    );
    assert_native_closing_punctuation_raw_crop(
        "vertical_lr",
        true,
        "｡",
        "halfwidth-full-stop",
        "halfwidth full stop",
        "mask",
    );
    assert_native_closing_punctuation_raw_crop(
        "vertical_lr",
        true,
        "｡",
        "halfwidth-full-stop",
        "halfwidth full stop",
        "object-id",
    );
}

fn assert_native_closing_punctuation_hanging_geometry(
    writing_mode: &str,
    next_column_moves_right: bool,
    mark: &str,
    label: &str,
    description: &str,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-{label}-hanging"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地{mark}人人[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp closing punctuation source");
    assert_native_rich_text_layer_image_has_content(&json);
    assert_native_closing_punctuation_geometry(
        &json,
        writing_mode,
        next_column_moves_right,
        mark,
        description,
    );
}

fn assert_native_closing_punctuation_raw_crop(
    writing_mode: &str,
    next_column_moves_right: bool,
    mark: &str,
    label: &str,
    description: &str,
    capture_kind: &str,
) {
    let fixture_name = format!("agent-observe-native-{writing_mode}-{label}-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地{mark}人人[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!("native-{writing_mode}-{label}-{capture_kind}.rgba"));

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
        .expect("arcw agent observe writes native closing punctuation raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} {description} {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native closing punctuation report is JSON");
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

    let punctuation = assert_native_closing_punctuation_geometry(
        &json,
        writing_mode,
        next_column_moves_right,
        mark,
        description,
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        punctuation["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        punctuation["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], punctuation["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], punctuation["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(punctuation),
            content_pixels,
            &format!("{writing_mode} {description} object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native closing punctuation mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native closing punctuation source");
    fs::remove_dir_all(&dir).expect("remove temp native closing punctuation dir");
}

fn assert_native_closing_punctuation_geometry<'report>(
    json: &'report serde_json::Value,
    writing_mode: &str,
    next_column_moves_right: bool,
    mark: &str,
    description: &str,
) -> &'report serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["writing_mode"],
        writing_mode
    );
    let earth = find_rich_text_cluster_object(json, "地", 3, 6);
    let punctuation = find_rich_text_cluster_object(json, mark, 6, 9);
    let person = find_rich_text_cluster_object(json, "人", 9, 12);
    assert_eq!(
        earth["bbox"]["x"], punctuation["bbox"]["x"],
        "{description} should remain in the current {writing_mode} column"
    );
    assert!(
        agent_json_bbox_y(&punctuation["bbox"]) > agent_json_bbox_y(&earth["bbox"]),
        "{description} should sit after the previous cluster"
    );
    assert_next_paragraph_column(
        punctuation,
        person,
        next_column_moves_right,
        "text after closing punctuation should start the next column",
    );
    punctuation
}

#[test]
fn agent_observe_native_renderer_reports_jlreq_line_end_prohibited_opening_punctuation() {
    let path = temp_arcw(
        "agent-observe-native-jlreq-line-end-opening-punctuation",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天地春「人外[/][p]
}
",
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp JLREQ opening punctuation source");
    assert_native_rich_text_layer_image_has_content(&json);

    let spring = find_rich_text_cluster_object(&json, "春", 6, 9);
    let opening_bracket = find_rich_text_cluster_object(&json, "「", 9, 12);
    let person = find_rich_text_cluster_object(&json, "人", 12, 15);
    assert!(
        agent_json_bbox_x(&opening_bracket["bbox"]) < agent_json_bbox_x(&spring["bbox"]),
        "line-end-prohibited opening punctuation should move to the next vertical_rl column"
    );
    assert!(
        agent_json_bbox_y(&opening_bracket["bbox"]) < agent_json_bbox_y(&spring["bbox"]),
        "opening punctuation moved from a column end should restart near the column top"
    );
    assert_vertical_cluster_after(
        opening_bracket,
        person,
        "text after opening punctuation should continue in the same moved column",
    );
    assert_eq!(
        opening_bracket["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    assert_rich_text_object_has_mask_capture(opening_bracket, "opening punctuation cluster");
}

#[test]
fn agent_observe_native_renderer_reports_vertical_lr_jlreq_edge_geometry() {
    let opening_path = temp_arcw(
        "agent-observe-native-vertical-lr-jlreq-opening-punctuation",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]天地春「人外[/][p]
}
",
    );
    let opening = observe_native_rich_text_layer_report(&opening_path);
    fs::remove_file(&opening_path).expect("remove temp vertical_lr JLREQ opening source");
    assert_native_rich_text_layer_image_has_content(&opening);

    let spring = find_rich_text_cluster_object(&opening, "春", 6, 9);
    let opening_bracket = find_rich_text_cluster_object(&opening, "「", 9, 12);
    let person = find_rich_text_cluster_object(&opening, "人", 12, 15);
    assert!(
        agent_json_bbox_x(&opening_bracket["bbox"]) > agent_json_bbox_x(&spring["bbox"]),
        "line-end-prohibited opening punctuation should move to the next vertical_lr column"
    );
    assert!(
        agent_json_bbox_y(&opening_bracket["bbox"]) < agent_json_bbox_y(&spring["bbox"]),
        "opening punctuation moved from a vertical_lr column end should restart near the column top"
    );
    assert_vertical_cluster_after(
        opening_bracket,
        person,
        "text after vertical_lr opening punctuation should continue in the same moved column",
    );
    assert_rich_text_object_has_mask_capture(opening_bracket, "vertical_lr opening punctuation");

    let hanging_path = temp_arcw(
        "agent-observe-native-vertical-lr-jlreq-hanging-punctuation",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]天地、人人[/][p]
}
",
    );
    let hanging = observe_native_rich_text_layer_report(&hanging_path);
    fs::remove_file(&hanging_path).expect("remove temp vertical_lr JLREQ hanging source");
    assert_native_rich_text_layer_image_has_content(&hanging);

    let earth = find_rich_text_cluster_object(&hanging, "地", 3, 6);
    let comma = find_rich_text_cluster_object(&hanging, "、", 6, 9);
    let next_person = find_rich_text_cluster_object(&hanging, "人", 9, 12);
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

    let leader_path = temp_arcw(
        "agent-observe-native-vertical-lr-jlreq-leader-chain",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr jlreq=normal]天地………終[/][p]
}
",
    );
    let leader = observe_native_rich_text_layer_report(&leader_path);
    fs::remove_file(&leader_path).expect("remove temp vertical_lr JLREQ leader source");
    assert_native_rich_text_layer_image_has_content(&leader);

    assert_eq!(
        first_text_run_presentation_layout(&leader)["jlreq_strictness"],
        "normal"
    );
    let first_leader = find_rich_text_cluster_object(&leader, "…", 6, 9);
    let second_leader = find_rich_text_cluster_object(&leader, "…", 9, 12);
    let ending = find_rich_text_cluster_object(&leader, "終", 15, 18);
    assert_vertical_cluster_after(
        first_leader,
        second_leader,
        "vertical_lr repeated leaders stay together in one trailing suffix",
    );
    assert!(
        agent_json_bbox_x(&ending["bbox"]) > agent_json_bbox_x(&second_leader["bbox"]),
        "vertical_lr text after a partially clipped overhanging leader chain should continue in the next column"
    );
    assert!(
        agent_json_bbox_y(&ending["bbox"]) < agent_json_bbox_y(&second_leader["bbox"]),
        "vertical_lr text after a leader chain should restart near the column top"
    );
}

#[test]
fn agent_observe_native_renderer_reports_jlreq_preset_specific_column_geometry() {
    let loose = observe_native_jlreq_preset_fixture("loose", "preset-loose");
    let normal = observe_native_jlreq_preset_fixture("normal", "preset-normal");
    assert_native_rich_text_layer_image_has_content(&loose);
    assert_native_rich_text_layer_image_has_content(&normal);

    assert_eq!(
        first_text_run_presentation_layout(&loose)["jlreq_strictness"],
        "loose"
    );
    assert_eq!(
        first_text_run_presentation_layout(&normal)["jlreq_strictness"],
        "normal"
    );

    assert_eq!(
        rich_text_vertical_column_start_byte_offsets(&loose),
        vec![0, 15, 27],
        "loose should expose the accepted UAX-permitted three-column plan"
    );
    assert_eq!(
        rich_text_vertical_column_start_byte_offsets(&normal),
        vec![0, 12, 21, 33],
        "normal should expose the accepted UAX-permitted four-column plan"
    );
}

#[test]
fn agent_observe_native_renderer_reports_strict_jlreq_closing_opening_column_plan() {
    assert_native_jlreq_closing_opening_column_plan("vertical_rl", false);
    assert_native_jlreq_closing_opening_column_plan("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_strict_jlreq_closing_opening_raw_crops() {
    assert_native_strict_jlreq_closing_opening_raw_crop("vertical_rl", "mask");
    assert_native_strict_jlreq_closing_opening_raw_crop("vertical_rl", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_strict_jlreq_closing_opening_raw_crops() {
    assert_native_strict_jlreq_closing_opening_raw_crop("vertical_lr", "mask");
    assert_native_strict_jlreq_closing_opening_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_jlreq_paragraph_column_geometry() {
    let path = temp_arcw(
        "agent-observe-native-jlreq-paragraph-column-geometry",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=normal]天地春夏秋冬月火、山々人「川」あっいおーえ―中・外………終[/][p]
}
",
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp JLREQ paragraph source");
    assert_native_jlreq_paragraph_overview(&json);
    assert_native_jlreq_paragraph_compression_and_iteration(&json, false);
    assert_native_jlreq_paragraph_grouping_and_leaders(&json, false);
}

#[test]
fn agent_observe_native_renderer_reports_vertical_lr_jlreq_paragraph_column_geometry() {
    let path = temp_arcw(
        "agent-observe-native-vertical-lr-jlreq-paragraph-column-geometry",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr jlreq=normal]天地春夏秋冬月火、山々人「川」あっいおーえ―中・外………終[/][p]
}
",
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp vertical_lr JLREQ paragraph source");
    assert_native_jlreq_paragraph_overview(&json);
    assert_native_jlreq_paragraph_compression_and_iteration(&json, true);
    assert_native_jlreq_paragraph_grouping_and_leaders(&json, true);
}

#[test]
fn agent_observe_native_renderer_reports_strict_jlreq_paragraph_class_mix_geometry() {
    assert_native_strict_jlreq_paragraph_class_mix_geometry("vertical_rl", false);
    assert_native_strict_jlreq_paragraph_class_mix_geometry("vertical_lr", true);
}

#[test]
fn agent_observe_native_renderer_writes_strict_jlreq_paragraph_class_mix_raw_crops() {
    assert_native_strict_jlreq_paragraph_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_strict_jlreq_paragraph_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_strict_jlreq_paragraph_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_strict_jlreq_paragraph_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_plain_western_word_class_mix_geometry() {
    assert_native_published_jlreq_plain_western_word_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_plain_western_word_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_plain_western_word_class_mix_raw_crops() {
    assert_native_published_jlreq_plain_western_word_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_plain_western_word_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_published_jlreq_plain_western_word_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_plain_western_word_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_western_word_class_mix_geometry() {
    assert_native_published_jlreq_western_word_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_western_word_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_western_word_class_mix_raw_crops() {
    assert_native_published_jlreq_western_word_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_western_word_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_published_jlreq_western_word_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_western_word_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_apostrophe_western_word_class_mix_geometry()
 {
    assert_native_published_jlreq_apostrophe_western_word_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_apostrophe_western_word_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_apostrophe_western_word_class_mix_raw_crops()
 {
    assert_native_published_jlreq_apostrophe_western_word_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_apostrophe_western_word_class_mix_raw_crop(
        "vertical_rl",
        "object-id",
    );
    assert_native_published_jlreq_apostrophe_western_word_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_apostrophe_western_word_class_mix_raw_crop(
        "vertical_lr",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_accented_latin_word_class_mix_geometry() {
    assert_native_published_jlreq_accented_latin_word_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_accented_latin_word_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_accented_latin_word_class_mix_raw_crops() {
    assert_native_published_jlreq_accented_latin_word_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_published_jlreq_accented_latin_word_class_mix_raw_crop(
        "vertical_rl",
        "object-id",
    );
    assert_native_published_jlreq_accented_latin_word_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_published_jlreq_accented_latin_word_class_mix_raw_crop(
        "vertical_lr",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_published_jlreq_decomposed_accented_latin_word_class_mix_geometry()
 {
    assert_native_published_jlreq_decomposed_accented_latin_word_class_mix_geometry("vertical_rl");
    assert_native_published_jlreq_decomposed_accented_latin_word_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_published_jlreq_decomposed_accented_latin_word_class_mix_raw_crops()
 {
    assert_native_published_jlreq_decomposed_accented_latin_word_class_mix_raw_crop(
        "vertical_rl",
        "mask",
    );
    assert_native_published_jlreq_decomposed_accented_latin_word_class_mix_raw_crop(
        "vertical_rl",
        "object-id",
    );
    assert_native_published_jlreq_decomposed_accented_latin_word_class_mix_raw_crop(
        "vertical_lr",
        "mask",
    );
    assert_native_published_jlreq_decomposed_accented_latin_word_class_mix_raw_crop(
        "vertical_lr",
        "object-id",
    );
}

#[test]
fn agent_observe_native_renderer_reports_zwj_grapheme_strict_class_mix_geometry() {
    assert_native_zwj_grapheme_strict_class_mix_geometry("vertical_rl");
    assert_native_zwj_grapheme_strict_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_zwj_grapheme_strict_class_mix_raw_crops() {
    assert_native_zwj_grapheme_strict_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_zwj_grapheme_strict_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_zwj_grapheme_strict_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_zwj_grapheme_strict_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_decomposed_kana_strict_class_mix_geometry() {
    assert_native_decomposed_kana_strict_class_mix_geometry("vertical_rl");
    assert_native_decomposed_kana_strict_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_decomposed_kana_strict_class_mix_raw_crops() {
    assert_native_decomposed_kana_strict_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_decomposed_kana_strict_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_decomposed_kana_strict_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_decomposed_kana_strict_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_variation_selector_strict_class_mix_geometry() {
    assert_native_variation_selector_strict_class_mix_geometry("vertical_rl");
    assert_native_variation_selector_strict_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_variation_selector_strict_class_mix_raw_crops() {
    assert_native_variation_selector_strict_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_variation_selector_strict_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_variation_selector_strict_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_variation_selector_strict_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_emoji_modifier_strict_class_mix_geometry() {
    assert_native_emoji_modifier_strict_class_mix_geometry("vertical_rl");
    assert_native_emoji_modifier_strict_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_emoji_modifier_strict_class_mix_raw_crops() {
    assert_native_emoji_modifier_strict_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_emoji_modifier_strict_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_emoji_modifier_strict_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_emoji_modifier_strict_class_mix_raw_crop("vertical_lr", "object-id");
}

#[test]
fn agent_observe_native_renderer_reports_regional_indicator_strict_class_mix_geometry() {
    assert_native_regional_indicator_strict_class_mix_geometry("vertical_rl");
    assert_native_regional_indicator_strict_class_mix_geometry("vertical_lr");
}

#[test]
fn agent_observe_native_renderer_writes_regional_indicator_strict_class_mix_raw_crops() {
    assert_native_regional_indicator_strict_class_mix_raw_crop("vertical_rl", "mask");
    assert_native_regional_indicator_strict_class_mix_raw_crop("vertical_rl", "object-id");
    assert_native_regional_indicator_strict_class_mix_raw_crop("vertical_lr", "mask");
    assert_native_regional_indicator_strict_class_mix_raw_crop("vertical_lr", "object-id");
}
