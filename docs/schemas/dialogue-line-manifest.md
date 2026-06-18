# Dialogue Line Manifest Schema

A dialogue line is the compiled unit connecting source text, localization key, speaker, window target, voice, control tags, dialogue interpolation, stage timeline, history, and agent observation.

```json
{
  "schema_version": 3,
  "line_id": "say.opening.alice.002",
  "text_key": "text.opening.alice.002",
  "speaker": "character.alice",
  "window": "textbox.main",
  "source_locale": "ja-JP",
  "source_text": "今日は少しだけ、変な夢を見たんだ。",
  "source_rich_text": "今日は少しだけ、#[fmt(\"変な夢\", color=dream_color, fallback=\"変な夢\")]を見たんだ。[p]",
  "source_hash": "b3:...",
  "source_anchor": "game/routes/opening.arcw:9:5-9:48",
  "flow": "flow.opening",
  "scope_path": ["dream"],
  "canonical_form": "character.say",
  "sugar_source": "speaker_colon",
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
  "control_tags": [
    { "tag": "p", "span": "...", "kind": "page_wait" }
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
    "window": "textbox.main"
  }
}
```

## Required fields

| Field | Meaning |
|---|---|
| `line_id` | Stable narrative line entity |
| `text_key` | Localization key |
| `speaker` | Character or built-in narrator |
| `window` | Dialogue window target; defaults to `textbox.main` |
| `source_locale` | Locale of inline/source text |
| `source_text` | Plain source text, without non-text control tags |
| `source_rich_text` | Source rich text including ruby, dialogue interpolation, and permitted control tags |
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

## Canonical form

Arcweft records whether the source was written as the canonical method form or as sugar.

```json
{ "canonical_form": "character.say", "sugar_source": "speaker_colon" }
```

Valid `sugar_source` values:

```text
none
speaker_colon
speaker_colon_indented
narrator_alias
```

There is no `script` form in the manifest.

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

## Window target

If no window is specified in source, the manifest records the resolved target.

```json
{ "window": "textbox.main" }
```

Custom textboxes are recorded by entity ID:

```json
{ "window": "textbox.phone_message" }
```

## Text fragments

```json
{ "kind": "text", "value": "おはよう。" }
{ "kind": "line_wait" }
{ "kind": "page_wait" }
{ "kind": "line_break" }
{ "kind": "ruby", "base": "変な夢", "ruby": "へんなゆめ" }
{ "kind": "style_start", "style": "em" }
{ "kind": "style_end", "style": "em" }
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

## Control tags

Control tags are parsed only in dialogue text mode. Reserved built-ins include `p`, `l`, `r`, `br`, `w`, `ruby`, `voice`, `face`, `pose`, `show`, `hide`, `move`, `anim`, `hook`, `call`, `signal`, `if`, `else`, `endif`, `raw`, and `fmt`.

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
  -> Character.say command
  -> Window target update
  -> AudioCommand / VoiceCue
  -> TextRevealPlan
  -> StageCommandTimeline
  -> DialogueHookDispatch
  -> AgentObservation metadata
  -> Replay trace event
```

