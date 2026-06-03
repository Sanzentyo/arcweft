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

Adapter completions, hover, and signature help are also Sans I/O. The LSP helper
consumes an already-resolved adapter manifest containing standard adapter facts,
project-local adapter manifests, and any profile-selected `arcweft-rust-abi`
metadata. It exposes manifest symbols, receiver methods, free functions, effect
capabilities, host calls, tooling docs, Rust exports, and Rust ADT names from the
same data source. It does not parse Rust source, query rust-analyzer, or run
Cargo by itself. Transport code refreshes metadata when the selected profile,
metadata JSON, adapter manifest, or Cargo build output changes, and can continue
showing the last valid metadata while reporting stale or missing metadata.

The helper also exposes adapter requirement diagnostics. The transport or
profile-aware compiler path supplies typed requirements collected from route
planning, runtime host tasks, or effect analysis, such as `http.respond` or
`fs.read_text`. `arcweft-verify-lsp` compares those requirements against the
resolved manifest and reports missing host calls or effect capabilities as
`arcweft-adapter` diagnostics. It does not add parser branches or implicit
fallback bindings for missing adapter features.

When the transport knows the selected runner, it should build an
`ArcweftLspContext` with both the resolved adapter manifest and a
`RuntimeHostCapabilities`:

```rust
let runtime_host = RuntimeHostCapabilities::standard_native();
let context = ArcweftLspProfileContextBuilder::new(&adapter)
    .with_runtime_host(&runtime_host)
    .build();
```

The combined helpers `profile_requirement_diagnostics`,
`profile_completions`, and `profile_hover` then expose both surfaces. Adapter
manifest diagnostics still report declarations missing from the selected
profile, while runtime-host diagnostics report declarations that type-check but
cannot be completed by the selected runner. The runtime-host set is a tooling
fact; it does not grant effects, add fallback bindings, or make unsupported
host calls executable.

Native and browser runners should use different presets. Native CLI/player
embeddings use `RuntimeHostCapabilities::standard_native()`, which includes
native virtual-file calls, host system information, and internal scheduler
markers. Browser embeddings use `RuntimeHostCapabilities::browser_web()`, which
keeps host system information and internal scheduler markers but excludes native
filesystem calls. If an embedding registers additional concrete host adapters,
it should extend the preset with the implemented adapter manifest:

```rust
let runtime_host = RuntimeHostCapabilities::browser_web()
    .with_adapter_manifest(&custom_web_adapter);
```

WebGPU and math acceleration are not treated as host-task capabilities by this
preset. Accelerator backends should add only the adapter manifests they actually
complete through the selected runner.
