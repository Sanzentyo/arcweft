# CLI

The CLI should expose syntax-normalization tools without forcing formatter users to give up script-friendly source.

## Syntax Expansion

Default formatting preserves indentation sugar such as `with:`. Expansion is explicit:

```bash
arcw fmt game/routes/opening.awft
arcw fmt --expand-sugar game/routes/opening.awft
arcw fmt --expand-sugar --write game/routes/
```

Expansion rewrites source-level sugar to canonical forms:

```text
with:                 -> with { ... }
speaker: text         -> speaker.say()[text]
speaker(args): text   -> speaker.say(args)[text]
await? expr with ...  -> try await expr with ...
```

The command must preserve IDs, source anchors where possible, comments, and stable child entity slots. It must never renumber dialogue or choice IDs as a side effect of formatting.
