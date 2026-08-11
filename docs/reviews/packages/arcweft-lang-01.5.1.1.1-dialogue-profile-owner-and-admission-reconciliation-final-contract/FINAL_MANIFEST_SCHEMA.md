# Final manifest schema

## Common schema-1 envelope

The examples below are complete enough to decode as profile fixtures. The
`dialogue` fragment is the only portion varied.

```toml
schema = 1

[package]
id = "local.arcweft.dialogue-profile"
version = "0.0.0"

[profiles.dev]
kind = "game"
source = "src/main.arcw"
```

## Omitted profile

No dialogue table is authored:

```toml
schema = 1

[package]
id = "local.arcweft.dialogue-profile"
version = "0.0.0"

[profiles.dev]
kind = "game"
source = "src/main.arcw"
```

Resolved value:

```text
view = std.view.dialogue
style = none
inline_failure = fail_line
```

## View-only profile

```toml
[profiles.dev.dialogue]
view = "view.MobileDialogue"
```

The View must exist in the accepted View program and accept the canonical
`DialogueView` input role. Style remains absent and policy defaults to
`fail_line`.

## Style-only profile

```toml
[profiles.dev.dialogue]
style = "style.MobileDialogue"
```

The View remains `std.view.dialogue`. The Style sheet must exist in the same
accepted product/source revision.

## Policy-only profile

```toml
[profiles.dev.dialogue.inline-failure]
kind = "discard"
```

The View remains `std.view.dialogue`, Style is absent, and failed inline
interpolations are discarded according to the dialogue-owned policy.

## Complete profile with the ordinary failure policy

```toml
[profiles.dev.dialogue]
view = "view.MobileDialogue"
style = "style.MobileDialogue"

[profiles.dev.dialogue.inline-failure]
kind = "fail_line"
```

## Complete profile with a text fallback

```toml
[profiles.dev.dialogue]
view = "view.MobileDialogue"
style = "style.MobileDialogue"

[profiles.dev.dialogue.inline-failure]
kind = "fallback"

[profiles.dev.dialogue.inline-failure.fallback]
kind = "text"
text = "[missing]"

[profiles.dev.dialogue.inline-failure.fallback.style]
kind = "plain"
```

## Other exact policy discriminators

```toml
[profiles.dev.dialogue.inline-failure]
kind = "fallback"

[profiles.dev.dialogue.inline-failure.fallback]
kind = "expr_source"

[profiles.dev.dialogue.inline-failure.fallback.style]
kind = "inherit_surrounding"
```

```toml
[profiles.dev.dialogue.inline-failure]
kind = "fallback"

[profiles.dev.dialogue.inline-failure.fallback]
kind = "call_source"

[profiles.dev.dialogue.inline-failure.fallback.style]
kind = "plain"
```

```toml
[profiles.dev.dialogue.inline-failure]
kind = "fallback"

[profiles.dev.dialogue.inline-failure.fallback]
kind = "value_plain"
```

The `apply` style variant carries typed `CharacterDialogueStyleValue` entries.
It uses the current dialogue codec unchanged; this contract does not introduce
a second manifest-only representation.

## Rejected spelling

This is invalid:

```toml
[profiles.dev.dialogue]
inline_failure = { kind = "fail_line" }
```

It is rejected through the strict decoder's ordinary unknown table/field path;
no alias, migration warning, or compatibility reader is permitted.

## Nominal family rules

- View: authored `view.*` or engine-owned `std.view.*`
- Style: authored `style.*` or engine-owned `std.style.*`
- markers such as `#` and `@` are not accepted nominal identities

Malformed syntax uses `manifest.id.invalid`; a syntactically valid ID from the
wrong nominal family uses `manifest.id.family`. Because family checking occurs
at decode, a wrong-family value never reaches compiler admission as a
`ViewId`/`ViewStyleSheetId`.

## Strictness

Every schema record and every tagged policy level denies unknown fields. The
following are not accepted:

- `inline_failure` alias;
- legacy policy enum spellings;
- untagged fallback variants;
- unknown fields on unit variants;
- a second decode-only bridge enum exposed to consumers.
