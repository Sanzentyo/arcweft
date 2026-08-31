# Dialogue Line Manifest Schema

A dialogue line is the compiled unit connecting source text, localization key,
speaker, authored View target, voice, point actions, dialogue interpolation,
stage timeline, history, and Agent observation.

```json
{
  "schema_version": 1,
  "line_id": "say.opening.alice.002",
  "text_key": "text.opening.alice.002",
  "speaker": "character.alice",
  "view": "view.MainDialogue",
  "source_locale": "ja-JP",
  "source_text": "今日は少しだけ、変な夢を見たんだ。",
  "source_rich_text": "今日は少しだけ、#[fmt(\"変な夢\", color=dream_color, fallback=\"変な夢\")]を見たんだ。[p]",
  "source_hash": "b3:...",
  "source_anchor": "game/routes/opening.arcw:9:5-9:48",
  "flow": "flow.opening",
  "scope_path": ["dream"],
  "voice_policy": "auto",
  "voice_by_locale": {
    "ja-JP": "voice.ja-JP.alice.opening.002",
    "en-US": "voice.en-US.alice.opening.002"
  },
  "reveal": {
    "mode": "voice",
    "fallback": "chars",
    "chars_per_second": 40
  },
  "display": {
    "character_text_color": "#f7d7ff",
    "name_color": "#e070ff",
    "read_state_style": "builtin.read_state_color"
  },
  "interpolations": [
    {
      "kind": "fmt",
      "expr_id": "expr_dream_word",
      "requires_trait": "DisplayText",
      "source_span": "..."
    }
  ],
  "placeholders": [
    {
      "name": "player_name",
      "type": "String",
      "required": true
    }
  ],
  "text_fragments": [
    { "kind": "text", "value": "今日は少しだけ、" },
    { "kind": "expr", "expr_id": "expr_dream_word", "display": "fmt" },
    { "kind": "text", "value": "を見たんだ。" },
    { "kind": "page_wait" }
  ],
  "point_actions": [
    { "action": "page", "span": "..." }
  ],
  "hooks": [
    {
      "kind": "builtin",
      "hook": "builtin.read_state_color",
      "phase": "BeforeTextStyle"
    }
  ],
  "timeline": [
    {
      "anchor": { "kind": "time", "ms": 420 },
      "commands": [
        { "kind": "face", "target": "character.alice", "expression": "worried", "transition_ms": 120 }
      ]
    }
  ],
  "agent": {
    "observable": true,
    "action_targets": ["advance_text"],
    "view": "view.MainDialogue"
  }
}
```

## Required fields

| Field | Meaning |
|---|---|
| `line_id` | Stable narrative line entity |
| `text_key` | Localization key |
| `speaker` | Character or built-in narrator |
| `view` | Authored dialogue View target; defaults to the standard library resource |
| `source_locale` | Locale of inline/source text |
| `source_text` | Plain source text, without non-text point actions |
| `source_rich_text` | Source rich text including Ruby, dialogue interpolation, typed content calls, and point actions |
| `source_hash` | Hash used for stale translation detection |

`flow` and `scope_path` are recommended for generated manifests. They preserve
the context used to resolve relative IDs such as `id=@.comment` and to derive
text/voice keys. Fully qualified `line_id` and `text_key` remain the stable
registry identities.

Relative source IDs are resolved before manifest emission. Dialogue line IDs use
the current flow, speaker, and named-scope path:

```text
id=@.suffix
  -> @say.{flow}.{speaker}.{scope_path}.{suffix}
  -> @say.{flow}.{speaker}.{suffix} when scope_path is empty
```

The manifest should keep the resolver context as data so tooling can explain
where a generated ID came from:

```json
{
  "flow": "flow.opening",
  "speaker": "character.alice",
  "scope_path": ["dream"],
  "line_id": "say.opening.alice.dream.hint",
  "text_key": "text.opening.alice.dream.hint"
}
```

## Built-in narrator

Narration uses `speaker = "character.narrator"` by default. Source aliases such as `narrator:`, `地の文:`, and `地:` are resolved to the same entity unless project configuration overrides them.

## Source form

The manifest records the accepted Character content application and its typed
source coordinate. It does not serialize a method-shaped canonical form or a
sugar compatibility discriminator.

Relative source IDs are normalized before entering the manifest:

```arcw
scope rain {
    alice(id=@.comment, voice=auto):
        雨、強くなってきたね。[p]
}
```

```json
{
  "line_id": "say.opening.alice.rain.comment",
  "text_key": "text.opening.alice.rain.comment",
  "speaker": "character.alice",
  "flow": "flow.opening",
  "scope_path": ["rain"],
  "voice_by_locale": {
    "ja-JP": "voice.ja-JP.alice.opening.rain.comment"
  }
}
```

## View target

If no View is specified in source, the manifest records the resolved standard
or project resource.

```json
{ "view": "std.view.dialogue" }
```

Project Views are recorded by entity ID:

```json
{ "view": "view.PhoneMessage" }
```

## Text fragments

```json
{ "kind": "text", "value": "おはよう。" }
{ "kind": "line_wait" }
{ "kind": "page_wait" }
{ "kind": "line_break" }
{ "kind": "ruby", "base": "変な夢", "ruby": "へんなゆめ" }
{ "kind": "scope", "presentation": { "kind": "em" }, "children": [ ... ] }
{ "kind": "expr", "expr_id": "expr_...", "display": "DisplayText" }
{ "kind": "expr", "expr_id": "expr_...", "display": "fmt" }
{ "kind": "placeholder", "name": "player_name" }
{ "kind": "hook_dispatch", "hook": "hook.alice_line_emphasis" }
```

## Interpolations

Dialogue interpolation via `#[expr]` requires `DisplayText` unless explicitly wrapped by `fmt(...)`.

```json
{
  "kind": "display_text",
  "expr_id": "expr_player_name",
  "trait": "DisplayText"
}
```

Explicit formatting:

```json
{
  "kind": "fmt",
  "expr_id": "expr_score",
  "format_args": { "style": "number" }
}
```

Localization placeholders are separate from runtime interpolation:

```json
{
  "name": "player_name",
  "type": "String",
  "required": true
}
```

## Point actions and content applications

Zero-width actions are serialized by their closed typed action identity and
payload. Body-bearing presentation, Ruby, raw literal content, objects, and Fx
are structural content applications; the manifest never reconstructs them from
a flat open/close stream or an alias list.

## Timeline anchors

```json
{ "kind": "time", "ms": 420 }
{ "kind": "end_offset", "ms": -250 }
{ "kind": "marker", "name": "surprise" }
{ "kind": "phoneme", "value": "a" }
{ "kind": "char", "index": 12 }
{ "kind": "word", "index": 3 }
```

## Runtime behavior

The manifest compiles into:

```text
DialogueLine
  -> Character content application
  -> persistent View mount update
  -> AudioCommand / VoiceCue
  -> TextRevealPlan
  -> StageCommandTimeline
  -> DialogueHookDispatch
  -> AgentObservation metadata
  -> Replay trace event
```

