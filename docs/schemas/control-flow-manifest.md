# Control Flow Manifest Schema

This schema summarizes the control-flow constructs recognized by the Arcweft compiler.

```json
{
  "schema_version": 1,
  "constructs": {
    "if": {
      "expression": true,
      "else_required_when_value": true
    },
    "match": {
      "expression": true,
      "exhaustive": true,
      "structured_bindings": true,
      "guard_keyword": "when"
    },
    "loop": {
      "expression": true,
      "break_value": true
    },
    "while": {
      "expression": false,
      "break_value": false
    },
    "while_let": {
      "expression": false,
      "pattern_binding": true
    },
    "let_else": {
      "else_must_diverge": true
    },
    "semicolon": {
      "required": false,
      "meaning": ["same_line_separator", "discard_expression_value"]
    }
  }
}
```
