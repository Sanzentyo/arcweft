# Error Trace Manifest Schema

```json
{
  "schema_version": 1,
  "error": {
    "kind": "AssetMissing",
    "message": "failed to load background image",
    "code": "ASSET_MISSING"
  },
  "trace": {
    "tick": 182,
    "state_hash": "b3:8a12...",
    "replay_cursor": "trace:182",
    "frames": [
      {
        "kind": "Flow",
        "flow": "flow.opening",
        "source": {
          "file": "game/routes/opening.awft",
          "line": 10,
          "column": 1,
          "byte_start": 212,
          "byte_end": 420
        }
      },
      {
        "kind": "DialogueLine",
        "line": "say.opening.narration.001",
        "text_key": "text.opening.narration.001",
        "speaker": "character.narrator",
        "source": {
          "file": "game/routes/opening.awft",
          "line": 12,
          "column": 5
        }
      },
      {
        "kind": "Await",
        "task": "task_0042",
        "awaited": "asset.image(asset.bg.room)",
        "source": {
          "file": "game/routes/opening.awft",
          "line": 15,
          "column": 18
        }
      }
    ]
  },
  "contexts": [
    "while loading opening background",
    "while entering flow.opening"
  ],
  "related": [
    {
      "kind": "AssetRef",
      "id": "asset.bg.room"
    }
  ]
}
```

## Required fields

```text
error.kind
error.message
trace.frames
```

## Recommended fields

```text
error.code
trace.tick
trace.state_hash
trace.replay_cursor
source file/line/column/byte span
flow id
line id
text key
speaker id
hook id
task id
related IDs
```

## Display policy

Tools may redact source text and IDs in product builds, but the crash bundle should preserve the full manifest when allowed by privacy configuration.
