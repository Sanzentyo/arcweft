# Independent cleanup inventory — 2026-07-10

## Policy

This inventory separates small ownership and duplication fixes from feature
work. Arcweft has no released consumer or persisted user data for the contracts
changed here. The implementation therefore replaces provisional shapes
directly: no deprecated aliases, dual readers, placeholder variants, or
migration-only schema versions are retained.

Compatibility work starts only when a released artifact, external consumer,
persisted user data, or an explicit compatibility requirement exists. This is
also recorded as a durable repository rule in `AGENTS.md`.

## Completed independent slices

### DataFormat inventory

`arcweft-data::DataFormat::ALL` is now the authoritative source-facing
inventory. Variant-name and codec-ID lookup, semantic builtin registration, and
runtime argument parsing consume the owning enum rather than a second list in
`arcweft-lang-sema`.

The unused noncanonical `yml`, `messagepack`, `arrow`, and `arcweft_binary`
lookups were removed. Canonical IDs remain `yaml`, `msgpack`, `arrow-ipc`, and
`arcweft-binary`. The new lower-layer dependency is
`arcweft-lang-sema -> arcweft-data`; it introduces no cycle and keeps data
formats below semantic analysis.

### Dialogue identity

`arcweft-lang-hir::dialogue_identity` now owns speaker slug normalization,
dialogue ID families, and `say.* -> text.*` derivation. HIR lowering and
ID-context materialization consume the same typed rules. Ordinary speaker case
is preserved, narrator aliases share `narrator`, and qualified/delimited entity
callees normalize consistently.

Absolute line IDs outside `say.*` and text keys outside `text.*` are rejected.
ID-context no longer derives a phantom text key from a wrong-family ID and only
materializes source positions identified as dialogue by the typed AST.

### Runtime collection conversions

`RuntimeValue` now owns collection-length construction and width-preserving
integer-to-index conversion. `RuntimeSeq` owns checked runtime indexing. The
pure and engine evaluators no longer carry parallel `len`, signed-index, and
unsigned-index helpers. Non-integer, negative, host-overflow, and out-of-bounds
indices deterministically yield `Unit` through one rule.

### Compact product-resource codec inventory

`ProductSectionCodecKind::ALL` contains exactly the 13 implemented compact
section codecs. Placeholder families (`Shader`, umbrella `View`, `Entity`,
`Contracts`, and `GraphIndex`) and JSON-temporary pseudo-codecs (`LocaleText`
and `DebugSymbols`) were removed together with the runtime migration-status
enum. Planning state remains documentation, not executable API.

Every remaining codec has a non-optional AWFB section owner. Codec tags are the
contiguous unpublished range 1-13, and one inventory test proves unique labels,
tags, magic values, section ownership, and both inverse mappings. No reader for
the discarded provisional tags exists.

### Session-save payload

The corrected unpublished session-save format is schema v1. The outer typed-save
envelope is the single owner of schema ID/version; the duplicate inner schema
object was removed. A generation stores one exclusive, complete artifact
binding:

- typed logical-bundle identity covering its manifest, source, executable, and
  runtime/presentation resources; or
- complete `ArtifactIdentity` for an AWFB container.

This leaves no root-only save path and makes contradictory identities
unrepresentable. The always-`Quiescent` payload field, speculative unsupported executor variants,
their conversion-only rejection test, an unnecessary executor-state `Box`, and
an unused codec-ID constant were also removed. Save creation itself continues
to reject non-quiescent sessions and unsupported live executor tiers.

### Collision-safe release test fixture

The broad route exposed a real test-infrastructure race: two parallel
`SuccessFileMirror` fixtures could receive the same process/timestamp path, and
one test's explicit cleanup could remove the other's staged publication. The
fixture now uses the shared `TempDir`, whose atomic sequence makes same-tick
paths unique and whose RAII cleanup also runs on early returns and panics. The
five-test `release_trust_json` binary passes in parallel.

## Ranked remaining slices

These candidates are independent of the completed work and should remain
separate reviewable commits.

| Rank | Candidate | Evidence | Target shape |
| ---: | --- | --- | --- |
| 1 | `ViewElementKind` inventory | `arcweft-bundle/src/resource_codec/view/model.rs`; repeated labels and source-name maps in `runtime_control_style.rs` and `arcweft-cli/src/app/bundle_view.rs` | Put `ALL`, source names, runtime labels, and lookup on the owning enum; migrate CLI/runtime consumers. |
| 2 | Pipe-placeholder AST traversal | Near-duplicate traversal in `arcweft-lang-sema/src/checker/expr/pipe.rs` and `arcweft-runtime-plan/src/expr/desugar.rs` | One HIR/syntax-owned visitor for query and substitution, with sema/runtime-plan parity tests. |
| 3 | REPL trace-policy wrapper and labels | `ReplCommandEndpointTracePolicy` immediately converts to `ReplTracePolicy`; MCP/LSP repeat conversions; JSON and CLI formatters duplicate enum label matches | Store the owning policy directly, delete the conversion-only test, and expose labels from their owning enums. Keep true MCP/LSP wire enums only at serialization boundaries. |
| 4 | Takumi gradient rules | Duplicate stop fallback, offset, and angle logic in `takumi-adapter/src/lowering.rs` and `paint_extractor.rs`; failure behavior has diverged | One crate-local gradient responsibility module and parity tests across both callers. |
| 5 | Empty LSP feature shells | `lsp/src/features/rename.rs` and `semantic_tokens.rs` contain comments only; no capability is advertised | Delete the public placeholder modules and add real modules only with handlers/capabilities. |
| 6 | Player-scene milli conversion | Four copies in `action_buttons.rs`, `control_style.rs`, `text_controls.rs`, and `frame.rs`, with one differing signed fallback | One crate-local unit type or numeric owner with explicit overflow policy. |
| 7 | Renderer viewport clipping | Identical action-button/text-control clipping in `render-wgpu/src/geometry`; a second layout-rect conversion is also duplicated | Move geometry conversions to the existing common geometry owner. |
| 8 | Bundle kind naming | Structured bundle kind and AWFB artifact kind are both named `BundleKind`, forcing aliases and a conversion helper | Directly rename the two concepts (for example, source bundle vs artifact kind) without compatibility aliases; treat as a medium-size slice after the smaller changes. |
| 9 | Multiple dialogue display frames | `runtime-driver/src/display.rs::resolve_display_frames` collects every dialogue event, but `BundlePresentationSnapshot::update` selects `resolution.frames.last()` | Either enforce a checked single-frame-per-step invariant or preserve all frames with explicit textbox and ordering semantics; never silently discard earlier frames. |
| 10 | Stream/source unsupported-statement no-ops | `runtime-plan/src/stream.rs::lower_stream_stmt` and `runtime-plan/src/source.rs::lower_source_stmt` use wildcard `StreamOp::Noop` / `SourceOp::Noop` fallbacks after general statement type checking | Make lowering fallible, report structured unsupported-statement diagnostics, and remove fallback no-ops that erase authored behavior. |
| 11 | Lossy executable-expression lowering | `runtime-plan/src/expr.rs::lower_runtime_expr` converts unsupported expressions to `RuntimeValue::String(expr_label(expr))`; flow/source/stream callers can fall back to it after strict lowering fails | Use checked lowering in every executable position and propagate one structured runtime-plan error contract instead of changing expression meaning. |
| 12 | Duplicate dialogue content model | `arcweft-dialogue/src/lib.rs` owns `DialogueTag` and `DialogueContent::parse_lossy`, while the compiler uses syntax/HIR/runtime-plan/render-text types | Audit facade consumers, then remove the duplicate or make one model authoritative without conversion wrappers or compatibility aliases. |
| 13 | Unowned generic text policies | `arcweft-layout::TextOverflowPolicy::Page` is exercised only by layout contract tests; `ViewTextRevealPolicy` and `reveal_policies` are serialized/merged but have no player consumer | Define and implement a genuine generic View owner or remove the unused variants/bindings. Do not reuse them as aliases for dialogue logical-page or reveal state. |

## Feature boundary found during dialogue playback cleanup

Mark and dialogue host-event dispatch is deliberately not ranked as a cleanup
slice. `RichTextControl::Mark` and `DialogueHostEvent` remain projected into
display stages, but exactly-once reveal-time dispatch, capability enforcement,
cancellation, save/restore, and executor parity require a separately designed
feature slice. The implemented control boundary and all independent follow-ups
are recorded in
[Dialogue control playback — 2026-07-10](dialogue-control-playback-2026-07-10.md).

Do not turn `LocaleText`, `DebugSymbols`, or other planned resource families
back into codec variants merely to reserve names. Each joins the runtime
inventory only when its compact codec and AWFB product path are implemented.

## Validation

Focused validation completed for the changed owners:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-core --lib tests::value
cargo test -p arcweft-core --lib tests::pure
cargo test -p arcweft-core --lib tests::flow
cargo test -p arcweft-data --test data_format
cargo test -p arcweft-lang-hir
cargo test -p arcweft-lang-sema dialogue
cargo test -p arcweft-lang-sema \
  tests::typecheck::typechecks_every_authoritative_data_format_variant -- --exact
cargo test -p arcweft-tooling
cargo test -p arcweft-runtime-accelerator data_external_call --lib
cargo test -p arcweft-bundle \
  --test resource_codec_common \
  --test runtime_resource_codecs \
  --test product_catalog_resource_codecs \
  --test view_resource_codecs \
  --test product_awbc_only
cargo test -p arcweft-runtime-driver \
  --test session --test awbc_product_session
cargo test -p arcweft-cli --test release_trust_json -- --nocapture
```

The first broad `just test-workspace` attempt reached its external 15-minute
command limit during compilation and is not counted. A warmed attempt exposed
the release fixture collision above. After the fix, the five-test binary passed
five consecutive runs and the final complete `just test-workspace` route
passed.
