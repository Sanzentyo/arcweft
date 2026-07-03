# DSL text input resources and submit flow sample

This cut moves the native text-input sample's input controls out of
`scene-contract.json` / product UI JSON sidecars and into Arcweft DSL.

## Implemented

- Added `ui text_input`, `ui text_area`, and `ui secure_field` top-level DSL
  declarations with typed `@input.*` IDs.
- Added `EntityKind::Input` so input IDs are indexed, resolved, and type
  checked as a real entity family rather than `Other("input")`.
- Added `let PAT = text_submit @input.id` as a flow statement lowered to the
  suspending host call `ui.text_input.await_submit`.
- Wired `BundleSession` so submitted text-control write-backs resume pending
  `text_submit` host calls with the submitted `String`.
- Added `String.len()` to sema and pure runtime evaluation, returning character
  count as `usize`.
- Fixed flow `if count < 5 { ... }` parsing: CST block detection no longer lets
  comparison `<` hide the following block `{`.
- Updated `samples/native-text-input/src/main.arcw` to declare its three text
  controls in DSL and removed the old input/program/text/scene-contract JSON
  files.
- Added `samples/text-submit-flow/`, a separate sample that waits for
  Enter/IME send, branches by submitted text length, uses the submitted string
  in dialogue, and returns it.

## Intentional follow-up

The new sample supports Enter and the platform IME send/done action. A visible
Arcweft-rendered submit button is not implemented in this cut because it needs
a typed UI action/handler contract that can synthesize the same text-control
submit write-back on web and native without HTML/DOM or compatibility shim
fallbacks. That is split into
`docs/reviews/requests/2026-07-03-seq-06.16-player-rendered-text-submit-button-package.md`.

## Verification

- `cargo run -p arcweft-cli -- check --manifest-path samples/native-text-input/arcw.toml`
- `cargo run -p arcweft-cli -- check --manifest-path samples/text-submit-flow/arcw.toml`
- `cargo run -p arcweft-cli -- run --runner headless samples/text-submit-flow/src/main.arcw --steps 2 --mode drain --max-ops 64`
- `cargo test -p arcweft-lang-syntax flow_if_comparison_condition_is_structured --test parser_p1`
- `cargo test -p arcweft-cli --test native_text_input_sample_sidecars`
- `cargo check -p arcweft-core -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-runtime-driver -p arcweft-cli --all-targets`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

The structure audit completed as a dry run and reported the current workspace
baseline of `3 error(s), 126 warning(s)` without writing report files.
