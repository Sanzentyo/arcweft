# TextBox Manifest Schema

A TextBox is the target object for dialogue output. If no target is specified, `@textbox.main` is used.

```json
{
  "schema_version": 1,
  "textbox_id": "textbox.main",
  "public_id": "textbox.main",
  "layer": "layer.view.dialogue",
  "role": "DialogueTextBox",
  "layout": {
    "anchor": "bottom",
    "rect": [80, 520, 1120, 160],
    "safe_area": true
  },
  "page_policy": "wait_then_clear",
  "reveal": {
    "mode": "typewriter",
    "chars_per_second": 40
  },
  "theme": "style.textbox.default",
  "agent": {
    "observable": true,
    "action_targets": ["advance_text", "skip_line"],
    "bbox_source": "ViewLayoutExact"
  }
}
```

## Built-ins

```text
textbox.main    default main dialogue textbox
textbox.narrator optional narration textbox
textbox.system  system/debug message textbox
```

## Runtime state

A TextBox runtime state contains:

```text
current_line_id
speaker
visible_text_range
full_rich_text
reveal_cursor
wait_state
read_state
active_voice
active_hooks
bbox / polygon / mask
```

## Agent observation

TextBox objects expose their current state through Agent Debug Bus so LLM tools can determine whether text is visible, partially revealed, waiting for input, or blocked by a cancellation policy.
