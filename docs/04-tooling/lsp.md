# LSP

## 基本機能

- diagnostics
- hover
- completion
- go to definition
- find references
- semantic tokens
- inlay hints
  - inferred `let` expressions containing numeric fallback show their stable
    resolved type (`: i32`, `: f64`, `: Vec<i32>`, and so on), including unary,
    binary, and compact-sequence expressions; explicit ascriptions do not get a
    duplicate hint
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
- Rich-text canonicalization code action:
  - command/action id: `arcweft.canonicalRichText`
  - rewrites inferred dot selectors such as `[.shake]...[/]` into explicit
    family tags such as `[effect .shake]...[/effect]`
  - rewrites unknown dot selectors to `[mark .name]`
  - rewrites inferred text proxy object selectors to `[object ...]` when the
    selector references a visible `#[text_proxy]` / `#[rich_text_proxy]` struct
    through `type=`, `struct=`, `proxy=`, or by using the struct name itself
  - does not expand unrelated dialogue sugar such as `$(expr)`, ruby shorthand,
    `[page]`, or speaker-line sugar
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
- View preview

## Effective presentation context

Style/defaults navigation is based on the effective presentation context at the
cursor position, not only lexical scope. A context may include the selected
project/build profile, module, flow, named scope path, current dialogue line or
content call, speaker preset, character, authored dialogue View style, selected dialogue
defaults profile, and inline rich-text span stack.

The dialogue RichText cascade is:

```text
inline rich-text span
  -> line options
  -> speaker preset options
  -> character dialogue_style
  -> authored dialogue View style
  -> selected dialogue defaults
  -> engine defaults
```

Lowering must preserve provenance for each effective setting:

```text
ResolvedSetting {
  path,
  value,
  winner,
  contributions,
}
```

Each contribution records the cascade layer, source kind, assignment operator,
path range, value range, displayed value, whether it is active, and the winning
contribution that shadows it when applicable. Source kinds include Arcweft
source files, project manifests, build profiles, and engine defaults.

LSP features use that shared index:

- hover shows the winning value and cascade contributors for fields such as
  `rich_text.ruby.size`
- go to definition on an effective style field jumps to the winning assignment
  value range; when the winner comes from a profile-selected dialogue defaults
  profile, the result also includes the manifest value that selected that
  defaults profile
- peek cascade shows shadowed and unset layers
- find all contributors lists declarations and inline spans that can affect the
  field in the current entry profile, including the manifest value that selected
  a dialogue defaults profile when that profile contributes to the field
- go to active profile selection jumps to the manifest or build profile that
  selected `@dialogue.mobile`
- code actions can extract an override to a line option, speaker preset,
  character `dialogue_style`, authored View style, or dialogue defaults profile

Generated or fully elaborated source is also surfaced through diagnostics.
Domain lint names are used in user-authored attributes, while stable numeric
codes are displayed in LSP diagnostics:

```text
AWF0101 style::redundant_decl_identity
AWF0102 identity::decl_binding_mismatch
AWF0103 style::explicit_decl_id
AWF0104 style::generated_surface_form
```

LSP severities follow the same default policy: redundant declaration identity is
a warning, declaration binding mismatch is an error, explicit declaration IDs
are hints, and generated surface forms are informational diagnostics.
Item-level `#[generated]` / `#[allow(...)]` and source-level
`#![generated(...)]` / `#![allow(...)]` use the same lint policy as CLI checks
and formatter expansion.

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
`lsp-types` diagnostics and code actions. It does not own stdio, sockets, open
document state, file watching, request cancellation, or client capability
negotiation.

`arcweft-lsp` is the transport crate. It uses `lsp-server` with synchronous stdio
transport and keeps the language-server session, FULL text-document cache,
client capability negotiation, publish-diagnostics notifications, and request
dispatch outside the verifier helper. MVP document sync is
`TextDocumentSyncKind::FULL`; incremental sync and `ropey` remain future work.

It also exposes source-level helpers backed by `arcweft-tooling`: sugar
expansion actions, relative-ID materialization actions, and inferred-ID inlay
hints. These helpers return `lsp-types` data only; opening documents, applying
workspace edits, watching files, and resolving editor capabilities remain
transport-adapter responsibilities.

Actual LSP ranges must not treat byte offsets as `Position.character` values.
`arcweft-verify-lsp` exposes `LspPositionMapper`, while `arcweft-lsp` owns a
source-aware `LineIndex` that maps Arcweft UTF-8 byte spans to the negotiated LSP
encoding. UTF-16 remains the default, and UTF-8 is selected only when the client
advertises it through initialize capabilities.

Source-level code actions return `WorkspaceEdit` values when the server can map
the current document snapshot. Sugar expansion and ID materialization edits are
computed by `arcweft-tooling`, converted through `LspPositionMapper`, and sent
as LSP text edits. Command-backed edits use a single structured
`workspace/executeCommand` argument:

```json
{
  "uri": "file:///story.arcw",
  "edit": {
    "start": 0,
    "end": 0,
    "replacement": "// generated\n"
  }
}
```

The server does not accept older positional command argument shapes; removed
syntax and removed tooling protocols should fail through the normal request
result path instead of transitional protocol branches. The command still returns a
`WorkspaceEdit`, so the server never writes files directly.

Adapter completions, hover, and signature help are also Sans I/O. The LSP helper
consumes an already-resolved adapter manifest containing standard adapter facts,
project-local adapter manifests, and any profile-selected `arcweft-rust-abi`
metadata. It exposes manifest symbols, receiver methods, free functions, effect
capabilities, host calls, tooling docs, Rust exports, and Rust ADT names from the
same data source. Rust ADT display uses `arcweft-rust-abi` metadata formatting,
so struct fields, enum variants, newtype inners, and nested `Vec` / `Option` /
`Result` / tuple references are visible in completion detail and hover without
the LSP parsing Rust source, querying rust-analyzer, or running Cargo by itself.
Borrowed Rust references are not exported through this metadata surface: the
Rust ABI macros reject borrowed function parameters, borrowed return values, and
borrowed ADT fields before metadata is generated.
Transport code refreshes metadata when the selected profile, metadata JSON,
adapter manifest, or Cargo build output changes, and can continue showing the
last valid metadata while reporting stale or missing metadata.

The stdio transport resolves project metadata from `arcw.toml` near each opened
document and caches the resolved profile per document URI. On `didOpen` and
`didSave` it refreshes that document's selected launch profile, loads
profile-local adapter manifests, applies profile-selected Rust ABI JSON to the
selected adapter, and publishes profile diagnostics together with source
diagnostics. `workspace/didChangeWatchedFiles` and
`workspace/didChangeConfiguration` refresh metadata for all open documents, so
adapter manifest and Rust ABI changes become visible to completion, hover, and
signature help without restarting the server. File reads and URI-to-path
conversion stay in `arcweft-lsp`; `arcweft-verify-lsp` continues to receive only
typed adapter/runtime facts. Profile diagnostics carry the profile id and a
profile-relative resource label, never host absolute paths. Missing and invalid
Rust ABI metadata are reported as `profile.rust_metadata.read` and
`profile.rust_metadata.parse`, and watched-file/configuration notifications
refresh Rust metadata for every open document. A project-local manifest is
treated as a declared profile surface, not as proof that the selected runner
implements its host calls; conformance diagnostics compare the declared manifest
against the runner capability preset.

Source diagnostics use the same profile-aware semantic pipeline as CLI checks:
syntax parse, HIR lowering, HIR reference resolution, typecheck readiness,
profile-selected adapter/Rust ABI type analysis, and then verifier diagnostics
with the same `TypeCheckEnv`. Later phases are skipped when an earlier phase
fails, so LSP diagnostics do not report verifier obligations for source that
has not passed profile-aware type checking.

Workspace edits are negotiated in the transport. If the client advertises
`workspace.workspaceEdit.documentChanges`, edit-bearing code actions and
`workspace/executeCommand` results are returned as versioned
`documentChanges`; otherwise they fall back to the plain `changes` map. The
server still never writes files directly.

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
let context = ArcweftLspProfileContextBuilder::new(&adapter)
    .with_runner_kind(RuntimeHostRunnerKind::Native)
    .build();
```

The combined helpers `profile_requirement_diagnostics`,
`profile_completions`, and `profile_hover` then expose both surfaces. Adapter
manifest diagnostics still report declarations missing from the selected
profile, while runtime-host diagnostics report declarations that type-check but
cannot be completed by the selected runner. The runtime-host set is a tooling
fact; it does not grant effects, add fallback bindings, or make unsupported
host calls executable.

For profile-level checks, transports can compare adapter manifests against the
selected runner through `RuntimeHostCapabilities::check_adapter_manifest` or the
LSP wrapper `profile_manifest_conformance_diagnostics`. The underlying report is
typed in `arcweft-runtime-host`, so LSP, CLI, and CI checks can share the same
host-call conformance rule.

Native and browser runners should use different presets. Native CLI/player
embeddings use `RuntimeHostCapabilities::standard_native()`, which includes
native virtual-file calls, host system information, and internal scheduler
markers. Browser embeddings use `RuntimeHostCapabilities::browser_web()`, which
keeps host system information and internal scheduler markers but excludes native
filesystem calls. If an embedding registers additional concrete host adapters,
it should extend the preset with the implemented adapter manifest:

```rust
let context = ArcweftLspProfileContextBuilder::new(&adapter)
    .with_runner_kind(RuntimeHostRunnerKind::BrowserWeb)
    .with_implemented_adapter_manifest(&custom_web_adapter)
    .build();
```

WebGPU and math acceleration are not treated as host-task capabilities by this
preset. Accelerator backends should add only the adapter manifests they actually
complete through the selected runner.
