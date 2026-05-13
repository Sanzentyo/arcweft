# Phase 0 / Phase 1 Workspace Status

## Workspace Layout

Implemented workspace members:

- `crates/arcweft-core`
- `crates/arcweft-id`
- `crates/arcweft-source`
- `crates/arcweft-need`
- `crates/arcweft-dialogue`
- `crates/arcweft-lang-syntax`
- `crates/arcweft-cli`

The workspace is intentionally dependency-light. Backend features are declared but empty in `arcweft-core` so adapter crates can be added later without changing the core boundary.

## Implemented Types

Identity and source:

- `EntityId`
- `PublicId`
- `TextKey`
- `SourceAnchor`

Async task state:

- `Need<T, E>`
- `Progress`

Dialogue surface model:

- `DialogueLine`
- `SpeakerRef`
- `TextBoxRef`
- `DialogueContent`
- `DialogueTag`
- `LinePlan`
- `TimelineAnchor`
- `CancelScope`
- `LineExit`

Supporting dialogue model types include speaker presets, voice references, content parts, line-plan steps, plan calls, and plan expressions. These are enough to represent the initial `alice2[...] with:` example as typed Rust data without implementing a parser.

Builder API:

- `SpeakerPreset`
- `SayOptions`
- `DialogueLineBuilder`
- `LinePlanBuilder`
- `TimelineCue`
- `CancelRule`
- `ReturnPayload`
- `CancelOnDrop`

The builder API supports fluent construction of a dialogue line with speaker defaults, line id, lossy dialogue content parsing, timeline cues, and input cancellation rules.

Syntax parser:

- `parse_source` and `parse_stub` now parse real `.awft` surface syntax into `SyntaxTree`.
- The parser records module/use headers, attributes, wiki links, flows, flow items, scenario commands, speaker lines, content calls, choice blocks, hooks, memo functions, parser items, line plans, and dialogue tokens.
- Diagnostics use structured `ParseError` values with spans, expected fragments, found text, recovery suggestions, and source anchors.
- `thiserror` is used for parser error integration.

## Deferred

Not implemented in this milestone:

- wgpu renderer
- Servo / DOM UI
- audio backend
- camera / capture devices
- USB / HID / gamepad backends
- MCP / agent protocol runtime
- Cranelift JIT
- type checking and HIR lowering
- semantic expression resolution beyond syntax-preserving CST fields

## Verification

Last verified during the Phase 0 / Phase 1 workspace pass:

```bash
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

Result:

- `cargo fmt`: passed
- `cargo clippy --workspace --all-targets --all-features`: passed
- `cargo test --workspace`: passed
