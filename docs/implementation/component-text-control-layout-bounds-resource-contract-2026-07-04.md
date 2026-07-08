# Component text-control layout bounds resource contract implementation note

## Implemented

- Added `ViewProgramResource::layout_bounds` as a dedicated typed bounds table.
- Added `ViewLayoutBoundsResource`, `ViewLayoutBoundsKind`, and `ViewLogicalRect`.
- Added inherent APIs on Arcweft-owned types:
  - `ViewProgramResource::text_control_bounds_for`;
  - `ViewProgramResource::semantic_target_bounds_for`;
  - `ViewLayoutBoundsResource::text_control`;
  - `ViewLayoutBoundsResource::semantic_target`;
  - `ViewInputKind::default_text_control_height_milli`.
- Changed `ViewInputResource::runtime_text_controls` to prefer program-authored
  `TextControl` bounds and fall back to existing stacked slots only when no
  program layout record exists.
- Extended View resource codec budget, canonicalization, public-id collection,
  record count, duplicate checking, and zero-size validation for layout bounds.
- Changed View lowering to derive deterministic layout bounds from
  current `Column`, `Row`, `Stack`/`Panel`, `Fragment`,
  text, and button structure without new parser syntax.
- Changed action-button submit bounds to derive from the same View text
  control bounds table before falling back to legacy stacked bounds.
- Updated `tools/build-web-ime-player-rendered-fixture.rs` so the rendered IME
  fixture carries typed text-control layout records.

## Files Touched

- `crates/arcweft-bundle/src/resource_codec.rs`
- `crates/arcweft-bundle/src/resource_codec/view/model.rs`
- `crates/arcweft-bundle/src/resource_codec/view/codec.rs`
- `crates/arcweft-bundle/tests/view_runtime_text_controls.rs`
- `crates/arcweft-bundle/tests/view_resource_codecs.rs`
- `crates/arcweft-bundle/tests/view_action_button_resources.rs`
- `crates/arcweft-bundle/tests/view_focus_navigation_resources.rs`
- `crates/arcweft-cli/src/app/bundle.rs`
- `crates/arcweft-cli/src/app/bundle_view.rs`
- `tools/build-web-ime-player-rendered-fixture.rs`
- `docs/design/component-text-control-layout-bounds-resource-contract-2026-07-04.md`
- `docs/implementation/component-text-control-layout-bounds-resource-contract-2026-07-04.md`

## Runtime And Player Path

No new player renderer shape is introduced. The existing player path already
consumes `ViewRuntimeTextControlBounds` and converts it into `HitRect`. This cut
changes the source of that runtime field from stacked defaults to the typed
program layout table when View bounds exist.

Action buttons still use the existing `ViewRuntimeButtonBounds` and
`RenderActionButton` path. Their default submit placement now reads the target
text-control bounds from the same layout table, then falls back to the legacy
stacked bounds only for older resources.

## Tests Added Or Updated

- Component-only `TextField` deterministic runtime bounds.
- `TextArea` and `SecureField` deterministic runtime bounds.
- Compact View resource round-trip preservation of layout bounds.
- Semantic target bounds agreement with text-control bounds.
- Missing bounds fallback to existing stacked slots.
- Invalid zero-size layout bounds rejection.
- Existing submit button placement with layout-bound target slots.
- Existing action button and focus navigation resource round trips.

## Non-Goals Retained

- Removed top-level text-control declarations are not reintroduced.
- The submit action substrate from seq06.16/seq06.16.1 is not redesigned.
- Platform widget, DOM, CSS screenshot, and source-string fallback behavior are
  not introduced.
- Full style-driven layout resolution is deferred to the later style/runtime
  rendering cut.

## Validation

- `cargo test -p arcweft-bundle --test view_runtime_text_controls -- --nocapture`
- `cargo test -p arcweft-bundle --test view_resource_codecs -- --nocapture`
- `cargo test -p arcweft-bundle --test view_action_button_resources -- --nocapture`
- `cargo test -p arcweft-bundle --test view_focus_navigation_resources -- --nocapture`
- `cargo test -p arcweft-cli view_ -- --nocapture`
- `cargo test -p arcweft-runtime-driver text_submit -- --nocapture`
- `cargo test -p arcweft-player-scene --test runtime_text_controls -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo run -p arcweft-cli -- bundle samples/modern-feedback-view/src/main.arcw --output target/arcweft/modern-feedback-view-layout-bounds.awfb`
- `cargo +nightly -Zscript tools/build-web-ime-player-rendered-fixture.rs --out target/arcweft/web-ime-player-rendered-layout-bounds.awfb`
- `cargo clippy -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene --all-targets -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

Structure audit completed as a dry run and reported the current workspace
baseline of `4 error(s), 127 warning(s)` without writing report files.
