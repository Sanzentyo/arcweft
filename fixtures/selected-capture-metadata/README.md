# Selected capture metadata fixtures

These fixtures illustrate the stable JSON shape produced by the seq06.5 patch series. They are not golden image fixtures; image body data is intentionally tiny placeholder base64.

- `object-color-resource.json`: selected rich-text child object color capture.
- `layer-mask-resource.json`: selected layer mask raw-rgba capture.
- `mcp-resource-list-descriptor.json`: MCP `resources/list` descriptor carrying `image.selected_capture` without image bytes.
- `mcp-resource-read-blob.json`: MCP `resources/read` blob content carrying the same metadata shape.
