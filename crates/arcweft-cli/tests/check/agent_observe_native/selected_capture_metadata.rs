#[test]
#[ignore = "tier 2 MCP stdio E2E: requires native-capture feature subprocess"]
fn agent_mcp_stdio_lists_selected_capture_metadata() {
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.observe",
                "arguments": {
                    "source": rich_text_showcase_path().display().to_string(),
                    "steps": 4,
                    "max_ops": 128,
                    "resource": "all",
                    "mcp_format": "list"
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    assert!(
        output.status.success(),
        "agent mcp selected capture metadata list should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    let resources = responses[1]["result"]["content"]
        .as_array()
        .expect("observe tool resource links are array");
    let object_capture = resources
        .iter()
        .find(|resource| {
            resource["type"] == "resource_link"
                && resource["image"]["kind"] == "color"
                && resource["image"]["scope"]["kind"] == "object"
                && resource["image"]["selected_capture"]["scope"]["kind"] == "object"
                && resource["image"]["selected_capture"]["source"]["kind"] == "object"
                && resource["uri"].as_str().is_some_and(|uri| {
                    uri.starts_with("arcweft://moderated/")
                        && Path::new(uri)
                            .extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                })
        })
        .unwrap_or_else(|| panic!("selected object capture descriptor exists: {resources:?}"));
    let selected = &object_capture["image"]["selected_capture"];
    assert_eq!(selected["scope"]["kind"], "object");
    assert_eq!(selected["coordinate_basis"], "output");
    assert_eq!(selected["crop"]["basis"], "output");
    assert!(selected["crop"]["clipped"]["size"]["width"].as_f64().unwrap_or(0.0) > 0.0);
    assert!(selected["mask"]["has_alpha_mask"].as_bool().is_some());
    assert_eq!(selected["source"]["kind"], "object");
}
