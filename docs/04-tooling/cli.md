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
speaker(args): text   -> speaker.say(args)[text] for character refs
speaker_preset(args): text
                      -> speaker_preset(args)[text]
await? expr with ...  -> try await expr with ...
parent::path          -> super::path
```

The expansion must preserve the callee kind. A lexical `SpeakerPreset` remains a
callable speaker value, so `alice2(voice=auto): text` expands to
`alice2(voice=auto)[text]`, not to `alice2.say(voice=auto)[text]`.

The command must preserve IDs, source anchors where possible, comments, and stable child entity slots. It must never renumber dialogue or choice IDs as a side effect of formatting.

Relative IDs are not expanded by default because they are author-facing source
syntax. A separate materialization command may rewrite relative IDs to their
fully normalized registry IDs when a project wants explicit IDs in source:

```bash
arcw ids materialize game/routes/opening.awft
arcw ids materialize --write game/routes/
```

Materialization resolves only ID-bearing contexts such as line IDs, text keys,
choice IDs, and choice option IDs. It must not rewrite ordinary entity
references, and it must not invent support for ambiguous forms such as
`goto @.next`.
