# Localization Catalog Schema

Arcweft localization catalogs map stable `text.*` keys to localized rich text, voice cues, status, and review metadata.

## Long CSV schema

Recommended columns:

```csv
key,locale,speaker,source_text,target_text,status,source_hash,voice,context,notes
```

| Column | Meaning |
|---|---|
| `key` | `text.*` localization key |
| `locale` | BCP-47 style locale such as `ja-JP` or `en-US` |
| `speaker` | optional `character.*` ID |
| `source_text` | source plain text for reference |
| `target_text` | localized text or rich text markup |
| `status` | `missing`, `draft`, `translated`, `reviewed`, `stale` |
| `source_hash` | source hash used to detect stale translations |
| `voice` | optional locale-specific voice key |
| `context` | flow, choice, UI view, or other context ID |
| `notes` | translator/reviewer notes |

## `.arcwloc` schema

```arcw
locale en-US from ja-JP {
    line text.opening.alice.002 {
        speaker = @character.alice
        source = rich "今日は少しだけ、|[変な夢](へんなゆめ)を見たんだ。"
        text = "I had a strange dream today."
        status = translated
        source_hash = "b3:f8c0..."
        voice = @voice.en-US.alice.opening.002
        context = @flow.opening

        notes {
            translator = "Keep ominous tone."
            max_chars = 80
        }
    }
}
```

## Import validation

The importer validates:

```text
- key exists in .arcweft/dialogue-lines.toml unless importing new keys explicitly.
- target rich text parses successfully.
- placeholders are preserved.
- source_hash matches or row is marked stale.
- voice ID exists or missing voice policy allows it.
- untrusted locale files do not introduce effectful tags: [call], [hook], [signal].
- translated p/l/r/w control tags are allowed only if policy permits them.
```

## Runtime resolution

```text
DialogueLine.text_key
  -> current locale catalog
  -> fallback locale catalog
  -> source Japanese text
```

`voice auto` follows the same locale-aware fallback chain, then optional TTS fallback.

