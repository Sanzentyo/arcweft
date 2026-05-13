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
    "block": {
      "expression": true,
      "final_expression_value": true,
      "semicolon_discards_final_value": true
    },
    "named_scope": {
      "syntax": "scope ident { ... }",
      "expression": true,
      "lexical_scope": true,
      "id_namespace": [
        "dialogue_line",
        "text_key",
        "choice",
        "choice_option"
      ],
      "trace_name": true
    },
    "relative_id": {
      "syntax": ".suffix",
      "valid_contexts": [
        "dialogue_line_id",
        "text_key",
        "choice_id",
        "choice_option_id"
      ],
      "not_general_entity_reference": true
    },
    "module_path": {
      "canonical_roots": ["crate", "self", "super"],
      "reserved_aliases": {
        "parent": "super"
      },
      "relative_id_syntax_allowed": false
    },
    "semicolon": {
      "required": false,
      "meaning": ["same_line_separator", "discard_expression_value"]
    }
  }
}
```
