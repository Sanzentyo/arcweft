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
- Expression syntax now has an `Expr` AST for entity references, literals, tuples, calls, named arguments, method calls, dialogue calls, indexes, pipes, binary comparisons, and placeholders.
- Dialogue `#[...]` expressions, timed-cue anchors/bodies, line-plan options, line-plan `let`/`return`, choice conditions, and `await ... with` carry parsed expressions for later type checking and HIR lowering.
- Flow `let`/`return`/`goto` statements now lower to structured `Stmt` and `Pattern` values instead of opaque strings.
- `lower_to_hir` verifies that parsed edge-case flow syntax can be converted to HIR-facing structures and rejects raw syntax that still needs parser coverage.
- `collect_symbol_uses` walks HIR without reparsing source snippets so name resolution can see dialogue callees, entity references, paths, calls, methods, dialogue text expressions, timed cues, and choice-condition references.
- `validate_typecheck_ready` rejects lowered HIR that still contains raw expression fragments before the future type checker sees it.
- `typecheck_hir` provides a minimal semantic checker over HIR with an explicit environment. It validates entity reference families, dialogue callees, `Need<T, E>` awaits, `Duration` timeline anchors, indexed expressions, calls, and methods for parser/HIR integration tests.

## Deferred

Not implemented in this milestone:

- wgpu renderer
- Servo / DOM UI
- audio backend
- camera / capture devices
- USB / HID / gamepad backends
- MCP / agent protocol runtime
- Cranelift JIT
- full HIR ownership/region model
- full type environment, name resolution, and type checking
- inference, overload resolution, traits, generics, contracts, and effect checking
- full semantic expression resolution and type-directed ambiguity resolution

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
