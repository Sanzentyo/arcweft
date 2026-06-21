use arcweft_agent_mcp::{model::McpToolDescriptor, tools::agent_tool_descriptors};
use std::collections::BTreeSet;

#[test]
fn tool_descriptors_have_unique_non_empty_names() {
    let tools = agent_tool_descriptors();
    assert!(!tools.is_empty(), "at least one MCP tool must be described");

    let mut names = BTreeSet::new();
    for tool in &tools {
        assert!(!tool.name.trim().is_empty(), "tool name must not be empty");
        assert!(
            names.insert(tool.name.as_str()),
            "duplicate MCP tool descriptor: {}",
            tool.name
        );
        assert_non_empty_metadata(tool);
    }
}

#[test]
fn required_schema_properties_are_declared() {
    for tool in agent_tool_descriptors() {
        assert_eq!(
            tool.input_schema
                .get("type")
                .and_then(serde_json::Value::as_str),
            Some("object"),
            "{} must expose an object input schema",
            tool.name
        );

        let properties = tool
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .unwrap_or_else(|| panic!("{} must declare schema properties", tool.name));

        let required = tool
            .input_schema
            .get("required")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten();
        for required_name in required {
            let required_name = required_name
                .as_str()
                .unwrap_or_else(|| panic!("{} required entries must be strings", tool.name));
            assert!(
                properties.contains_key(required_name),
                "{} requires undeclared property {required_name}",
                tool.name
            );
        }
    }
}

fn assert_non_empty_metadata(tool: &McpToolDescriptor) {
    assert!(
        tool.title
            .as_deref()
            .is_none_or(|title| !title.trim().is_empty()),
        "{} has an empty title",
        tool.name
    );
    assert!(
        !tool.description.trim().is_empty(),
        "{} has an empty description",
        tool.name
    );
}
