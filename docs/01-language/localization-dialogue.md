# Localization for Dialogue, Voice, and Text Keys

Arcweft is Japanese-first by default: ordinary `.arcw` source can be written directly in Japanese. The compiler extracts each dialogue line, narration line, choice label, and UI label into stable text units with generated IDs shown by LSP inlay hints.

Related:

- [Flow-Integrated Scenario Syntax](scenario-surface-syntax.md)
- [Dialogue Control Tags, Ruby, Interpolation, and Inline Hooks](dialogue-control-tags-and-ruby.md)
- [Dialogue Line Manifest](../schemas/dialogue-line-manifest.md)
- [Localization Catalog](../schemas/localization-catalog.md)

---

## Source locale policy

```toml
# project.arcw.toml
[locale]
source = "ja-JP"
default = "ja-JP"
fallback = ["ja-JP"]

[locale.extraction]
default_mode = "inline_source"
id_storage = "registry"      # registry | inline | hybrid
show_inlay = true
```

Meaning:

```text
source = "ja-JP"
  Text written directly in .arcw is Japanese source text.

id_storage = "registry"
  Text IDs are stored in .arcweft/dialogue-lines.toml instead of cluttering source.

show_inlay = true
  LSP shows line IDs, text keys, translation status, and voice keys as inlay hints.
```

---

## Source stays concise

```arcw
pub flow @flow.opening opening(state: GameState) -> Result<FlowExit, FlowError> {
    bg(@asset.bg.room, fade = 300ms)
    show(@character.alice, .smile, at = .center)

    地の文: 扉の向こうから、雨の音がした。[p]
    alice: おはよう。[l]
    alice(voice=auto): 今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]

    choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" -> @flow.alice_intro
        @choice.opening.silent "黙っている" -> @flow.quiet_intro
    }
}
```

LSP inlay view:

```text
地の文: 扉の向こうから、雨の音がした。[p]
        @say.opening.narrator.001 / text.opening.narrator.001

alice: おはよう。[l]
       @say.opening.alice.001 / text.opening.alice.001 / voice.ja-JP.alice.opening.001

alice(voice=auto): 今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
                   @say.opening.alice.002 / text.opening.alice.002 / voice.ja-JP.alice.opening.002
```

---

## Identity model

A dialogue line has separate identities:

```text
LineId      @say.opening.alice.002        stable narrative line entity
TextKey     @text.opening.alice.002       localization key
VoiceKey    @voice.ja-JP.alice.opening.002  locale/speaker voice cue
Speaker     @character.alice
```

`LineId` remains stable through text edits. `source_hash` changes and marks translations as stale.

The canonical dialogue form is `speaker.say(...)[...]`; `speaker(...):` is sugar. Dialogue and narration use the same line options:

```arcw
alice.say(
    id = @say.opening.alice.greeting,
    text_key = @text.opening.alice.greeting,
    voice = auto,
    args = { player_name = state.player_name },
)[
    {player_name}、おはよう。[p]
]

alice(
    id = @say.opening.alice.greeting,
    text_key = @text.opening.alice.greeting,
    voice = auto,
    args = { player_name = state.player_name },
):
    {player_name}、おはよう。[p]
```

`text_key` is optional. If omitted, it is derived from the narrative line ID:

```text
id = @say.opening.alice.greeting
  -> text_key = @text.opening.alice.greeting
  -> voice_key = @voice.{runtime_locale}.alice.opening.greeting
```

Line IDs may be written as relative IDs when the surrounding flow and speaker
provide the stable prefix:

```arcw
alice(id=@.greeting, voice=auto):
    おはよう。[p]

地の文(id=@.rain):
    扉の向こうから、雨の音がした。[p]
```

Relative line IDs normalize to full `@say...` IDs:

```text
alice(id=@.greeting)
  -> @say.opening.alice.greeting
  -> @text.opening.alice.greeting

地の文(id=@.rain)
  -> @say.opening.narrator.rain
  -> @text.opening.narrator.rain
```

Named `scope` blocks become part of generated and relative IDs:

```arcw
scope rain {
    地の文(id=@.sound):
        扉の向こうから、雨の音がした。[p]

    alice(id=@.comment):
        雨、強くなってきたね。[p]
}
```

```text
@say.opening.narrator.rain.sound
@text.opening.narrator.rain.sound
@say.opening.alice.rain.comment
@text.opening.alice.rain.comment
```

If `id` is omitted, the generated stable ID still includes the named scope path:

```text
scope rain { 地の文: ... }
  -> @say.opening.narrator.rain.001
  -> @text.opening.narrator.rain.001
```

Narration is the built-in narrator speaker alias and accepts the same options:

```arcw
地の文(id=@say.opening.narrator.rain):
    扉の向こうから、雨の音がした。[p]

narrator(id=@say.opening.narrator.rain):
    扉の向こうから、雨の音がした。[p]
```

Locale is split into source locale and runtime locale. `source` in project config describes the language of text written directly in `.arcw`. Runtime locale comes from engine state/config and is not normally a line option. Per-line `source_locale` is only for exceptional embedded-language lines:

```arcw
alice(id=@say.opening.alice.english_quote, source_locale=en-US):
    Good morning.[p]
```

For a scoped override, use a lexical source locale block:

```arcw
source locale en-US {
    alice(id=@say.opening.alice.english_quote):
        Good morning.[p]
}
```

---

## Dialogue line registry

The extracted line registry is versioned and stored under `.arcweft/`.

```toml
# .arcweft/dialogue-lines.toml

[lines."ent_01JABC_OPENING_NARRATOR_001"]
kind = "Narration"
public_id = "say.opening.narrator.001"
text_key = "text.opening.narrator.001"
speaker = "character.narrator"
source_locale = "ja-JP"
source_text = "扉の向こうから、雨の音がした。"
source_rich_text = "扉の向こうから、雨の音がした。[p]"
source_hash = "b3:91a2..."
source_anchor = "game/routes/opening.arcw:7:5-7:32"
flow = "flow.opening"

[lines."ent_01JABC_OPENING_ALICE_002"]
kind = "Dialogue"
public_id = "say.opening.alice.002"
text_key = "text.opening.alice.002"
speaker = "character.alice"
source_locale = "ja-JP"
source_text = "今日は少しだけ、変な夢を見たんだ。"
source_rich_text = "今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]"
source_hash = "b3:f8c0..."
source_anchor = "game/routes/opening.arcw:9:5-9:48"
voice_key = "voice.alice.opening.002"
flow = "flow.opening"

[lines."ent_01JABC_CHOICE_LISTEN"]
kind = "ChoiceLabel"
public_id = "choice.opening.listen"
text_key = "text.choice.opening.listen"
source_locale = "ja-JP"
source_text = "聞いてみる"
source_hash = "b3:12cd..."
source_anchor = "game/routes/opening.arcw:12:9-12:15"
flow = "flow.opening"
```

Static choice labels are localization extraction targets. Their text key is derived from the choice option ID unless explicitly provided:

```text
@choice.opening.listen
  -> @text.choice.opening.listen
```

Dynamic option labels are extracted only when the label value is `LocalizedText`, `TextKey`, or rich text with a stable text key. Plain runtime `String` labels are displayed but are not extractable; LSP should report `CHOICE_DYNAMIC_LABEL_NOT_LOCALIZABLE` for user-facing strings that need translation.

---

## ID generation rules

Default generated keys:

```text
flow.opening
  say.opening.narrator.001
  text.opening.narrator.001

  say.opening.alice.001
  text.opening.alice.001
  voice.ja-JP.alice.opening.001

  say.opening.alice.002
  text.opening.alice.002
  voice.ja-JP.alice.opening.002

  text.choice.opening.listen
  text.choice.opening.silent
```

Counters are stable. Inserting a new line between `.001` and `.002` creates `.003`; it does not renumber old lines.

Renumbering is explicit:

```bash
arcw locale renumber --flow flow.opening --preview
arcw locale renumber --flow flow.opening --apply
```

---

## CSV localization

Arcweft supports CSV import/export for translators, spreadsheets, and LLM batch workflows.

### Long CSV, recommended

```csv
key,locale,speaker,source_text,target_text,status,source_hash,voice,context,notes
text.opening.alice.001,en-US,character.alice,おはよう。,Good morning.,translated,b3:91a2...,voice.en-US.alice.opening.001,flow.opening,
text.opening.alice.002,en-US,character.alice,"今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。",I had a strange dream today.,draft,b3:f8c0...,voice.en-US.alice.opening.002,flow.opening,
text.choice.opening.listen,en-US,,聞いてみる,Ask her about it,translated,b3:12cd...,,choice.opening.first,
```

### Wide CSV, optional

```csv
key,speaker,ja-JP,en-US,zh-CN,status,source_hash,context,notes
text.opening.alice.001,character.alice,おはよう。,Good morning.,早上好。,translated,b3:91a2...,flow.opening,
text.opening.alice.002,character.alice,"今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。",I had a strange dream today.,今天我做了一个奇怪的梦。,draft,b3:f8c0...,flow.opening,
```

Recommended policy:

```text
standard import/export: long CSV
small manual review: wide CSV
structured rich text / voice / notes: .arcwloc
```

---

## `.arcwloc` localization file

CSV is intentionally simple. `.arcwloc` is used for structured localization metadata.

```arcw
locale en-US from ja-JP {
    line text.opening.alice.001 {
        speaker = @character.alice
        source = "おはよう。"
        text = "Good morning."
        status = translated
        source_hash = "b3:91a2..."
        voice = @voice.en-US.alice.opening.001
        context = @flow.opening
    }

    line text.opening.alice.002 {
        speaker = @character.alice
        source = rich "今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。"
        text = "I had a strange dream today."
        status = draft
        source_hash = "b3:f8c0..."
        voice = @voice.en-US.alice.opening.002

        notes {
            translator = "Keep this line slightly ominous."
            max_chars = 80
        }
    }

    line text.choice.opening.listen {
        source = "聞いてみる"
        text = "Ask her about it"
        status = translated
        context = @choice.opening.first
    }
}
```

---

## Rich text, control tags, and translations

Japanese source may contain ruby and control tags:

```arcw
alice: 今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
```

Exported source fields distinguish plain text and rich text:

```text
source_text      今日は少しだけ、変な夢を見たんだ。
source_rich_text 今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。[p]
```

Import validation checks:

```text
- placeholders are preserved.
- rich text markup is well-formed.
- ruby syntax is valid if present.
- untrusted locale files cannot add [call], [hook], [signal], or other effectful tags.
- p/l/r/w tags appear only if locale policy allows translated control marks.
```

---

## Placeholders and runtime interpolation

Arcweft separates localization placeholders from runtime interpolation.

Runtime interpolation with `#[expr]` is not exported as a translator-editable placeholder by default:

```arcw
narrator: #[state.player_name]は鍵を手に入れた。[p]
```

A localizable placeholder uses `{name}` and must be supplied by line args or context:

```arcw
narrator.say(args={ player_name = state.player_name })[
    {player_name}は鍵を手に入れた。[p]
]
```

Registry metadata:

```toml
text_key = "text.item.key_obtained"
placeholders = [
  { name = "player_name", type = "PlayerName" }
]
```

CSV:

```csv
key,locale,source_text,target_text,placeholders,status,source_hash
text.item.key_obtained,en-US,{player_name}は鍵を手に入れた。,{player_name} got the key.,player_name:PlayerName,translated,b3:...
```

Missing placeholders are errors:

```text
error[LOCALE_PLACEHOLDER_MISSING]:
  translation for text.item.key_obtained is missing placeholder {player_name}
```

Runtime interpolation still requires `DisplayText` or explicit `fmt(...)`:

```arcw
narrator: スコアは#[fmt(score, style="number", on_error=InlineFailure.fallback("?"))]点です。[p]
```

---

## Voice per locale

`voice auto` resolves by locale, speaker, and line ID.

```text
voice.{locale}.{speaker}.{line_id}
  ↓
voice.{source_locale}.{speaker}.{line_id}
  ↓
TTS fallback or no voice
```

CSV can specify localized voice:

```csv
key,locale,target_text,voice,status,source_hash
text.opening.alice.002,en-US,I had a strange dream today.,voice.en-US.alice.opening.002,translated,b3:f8c0...
```

`.arcwloc` can also set voice:

```arcw
line text.opening.alice.002 {
    text = "I had a strange dream today."
    voice = @voice.en-US.alice.opening.002
}
```

TTS fallback:

```toml
[localization.voice]
missing_voice = "tts"  # silent | warn | error | tts

[tts."en-US"]
speaker.alice = "en-US-female-soft"
```

---

## Stale translation detection

If the Japanese source changes, `source_hash` changes.

```text
registry source_hash: b3:a921...
locale row source_hash: b3:f8c0...
=> translation is stale
```

Diagnostic:

```text
warning[LOCALE_STALE]:
  en-US translation for text.opening.alice.002 was translated from an older source.
```

CSV export marks stale rows:

```csv
key,locale,source_text,target_text,status,source_hash
text.opening.alice.002,en-US,今日は少しだけ、変な夢を見たんだ。,I had a strange dream today.,stale,b3:a921...
```

---

## CLI

```bash
arcw locale extract
arcw locale check
arcw locale stale
arcw locale stats

arcw locale export --locale en-US --format csv --out locales/en-US.csv
arcw locale export --format wide-csv --out locales/all.csv
arcw locale export --flow flow.opening --locale en-US --format csv

arcw locale import locales/en-US.csv
arcw locale import locales/en-US.arcwloc

arcw locale missing --locale en-US
arcw locale open text.opening.alice.002
arcw locale attach-voice text.opening.alice.002 --locale en-US --file voice/en/alice/opening_002.ogg
```

---

## LSP features

Inlay hints:

```text
alice: おはよう。[l]
       @say.opening.alice.001 / text.opening.alice.001 / en-US: translated / zh-CN: missing
```

Hover:

```text
text.opening.alice.002
source locale: ja-JP
source hash: b3:f8c0...
en-US: draft
zh-CN: missing
voice ja-JP: present
voice en-US: missing
```

Code actions:

```text
- Materialize text ID
- Open locale entry
- Export this flow to CSV
- Generate missing locale rows
- Mark translation stale
- Attach voice file
- Generate TTS placeholder
```

Diagnostics:

```text
- missing translation
- stale translation
- missing placeholder
- invalid rich text markup
- forbidden translated control tag
- missing voice for locale
- text too long for textbox
- choice label exceeds layout constraints
```


