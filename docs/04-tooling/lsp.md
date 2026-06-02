# LSP

## 基本機能

- diagnostics
- hover
- completion
- go to definition
- find references
- semantic tokens
- inlay hints
- code actions
- formatter
- rename
- prepare rename

## DSL固有

- ID inference / materialize / rename
- Ref resolution
- Sugar expansion code actions:
  - `with:` → `with { ... }`
  - `speaker:` → `speaker.say()[...]`
  - `speaker(args):` → `speaker.say(args)[...]` for character refs
  - `speaker_preset(args):` → `speaker_preset(args)[...]`
  - `await? expr with ...` → `try await expr with ...`
  - `parent::path` → `super::path`
  - preserve the callee kind when expanding dialogue sugar, so lexical speaker
    presets are not rewritten into forced `.say(...)` calls
- ID code actions:
  - materialize dialogue `id=@.suffix` and `text_key=@.suffix` options as
    normalized `@say...` / `@text...` IDs
  - insert omitted dialogue `id=` / `text_key=` for colon, bracket-call, and
    flat `=== line ... ===` dialogue heads
  - materialize `choice @.suffix` and relative option IDs as normalized `@choice...` IDs
  - keep `@.suffix` / `@..suffix` / `@...suffix` and `@super...` relative IDs by default during formatting
- `Need` unhandled diagnostics
- naked await in flow diagnostics
- borrow crosses await diagnostics
- Option/Result unhandled diagnostics
- contract hover / counterexample display
- parser preview
- shader diagnostics
- audio cue / signal / BGM completion
- UI component preview

## Custom requests

```text
arcweft/getNodeAtPosition
arcweft/getGraphSlice
arcweft/getNodeHistory
arcweft/previewGraphPatch
arcweft/applyGraphPatch
arcweft/getRagContext
arcweft/renderRouteMap
arcweft/parseInput
arcweft/expandSugar
arcweft/shaderPreview
arcweft/audioCuePreview
```

## Agent-oriented JSON

CLI/LSP は `arcweft-verify` の machine-readable diagnostics を共有する。

```json
{
  "id": "diagnostic.obligation.0001",
  "severity": "error",
  "message": "lifetime promotion to `'flow` requires proof or audit",
  "source": { "start": 120, "end": 155 },
  "obligation": "obligation.0001",
  "related_ids": ["'flow"],
  "actions": [
    {
      "id": "action.generate_proof_stub",
      "label": "Generate proof stub",
      "kind": "generate_proof_stub"
    }
  ]
}
```

`arcweft-verify-lsp` is a Sans I/O helper crate. It converts verifier reports into
`lsp-types` diagnostics and code actions. A transport server can wrap this
crate later without changing verifier semantics.

It also exposes source-level helpers backed by `arcweft-tooling`: sugar
expansion actions, relative-ID materialization actions, and inferred-ID inlay
hints. These helpers return `lsp-types` data only; opening documents, applying
workspace edits, watching files, and resolving editor capabilities remain
transport-adapter responsibilities.

Rust adapter completions, hover, and signature help are also Sans I/O. The LSP
helper consumes an already-resolved adapter manifest containing standard
adapter facts plus any profile-selected `arcweft-rust-abi` metadata. It does
not parse Rust source, query rust-analyzer, or run Cargo by itself. Transport
code refreshes metadata when the selected profile, metadata JSON, or Cargo build
output changes, and can continue showing the last valid metadata while
reporting stale or missing metadata.
