# Dialogue View Manifest Schema

Dialogue presentation uses an ordinary authored View whose input type carries
the standard dialogue role. The manifest records that typed relationship; it
does not create a second dialogue-specific presentation entity.

```json
{
  "schema_version": 1,
  "view_id": "view.MainDialogue",
  "parameter": {
    "name": "dialogue",
    "type": "DialogueView",
    "role": "dialogue_view",
    "required_fields": {
      "speaker": "String",
      "content": "DialogueContent",
      "occurrence": "DialogueOccurrenceId",
      "stage": "DialogueStage",
      "reveal": "DialogueReveal",
      "primary_action": "DialogueAction"
    }
  },
  "program": "view-program.MainDialogue",
  "styles": ["style.main_dialogue"],
  "text_consumers": [
    { "node": "speaker", "source": "dialogue.speaker", "kind": "Text" },
    { "node": "content", "source": "dialogue.content", "kind": "RichText" }
  ],
  "agent": {
    "observable": true,
    "action_targets": ["advance_text", "skip_line"],
    "geometry_source": "PreparedViewText"
  }
}
```

`DialogueView` is a public nominal standard-prelude record. Type checking and
LSP tooling resolve it like any other public type. A custom nominal record may
carry the `dialogue_view` role when the compiler verifies its required fields.

The standard library includes the reserved minimal manifest-backed resource
`std.view.dialogue`; project declarations cannot override it. An omitted
project `view` selection links to that resource through the same View program
and mount evaluator as an explicitly selected project View.

## Runtime mount state

Each active dialogue target is a persistent View mount. Save/load and Agent
observation retain:

```text
view_mount_id
view_definition_id
dialogue_presentation_id
dialogue_occurrence_id
current_line_id
speaker
display_stage_index
logical_page_index
visible_text_range
full_rich_text
reveal_cursor
wait_state
read_state
active_voice
active_hooks
prepared geometry
```

Logical-page behavior is not a View-manifest option. Dialogue controls define
the state transition: `[p]` closes a logical page after its user wait, `[l]`
keeps that page open after its user wait, and a terminal `[p]` releases the line
without creating an empty page. The View may style or animate that transition,
but cannot change its semantics.

Agent geometry is derived from the same prepared View text used for rendering,
interaction, accessibility, and capture.
