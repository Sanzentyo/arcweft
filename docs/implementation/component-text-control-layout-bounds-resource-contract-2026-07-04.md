# Component text-control layout bounds resource contract implementation note

## Implemented

- Added `UiProgramResource::layout_bounds` as a dedicated typed bounds table.
- Added `UiLayoutBoundsResource`, `UiLayoutBoundsKind`, and `UiLogicalRect`.
- Added inherent APIs on Arcweft-owned types:
  - `UiProgramResource::text_control_bounds_for`;
  - `UiProgramResource::semantic_target_bounds_for`;
  - `UiLayoutBoundsResource::text_control`;
  - `UiLayoutBoundsResource::semantic_target`;
  - `UiInputKind::default_text_control_height_milli`.
- Changed `UiInputResource::runtime_text_controls` to prefer program-authored
  `TextControl` bounds and fall back to existing stacked slots only when no
  program layout record exists.
- Extended UI resource codec budget, canonicalization, public-id collection,
  record count, duplicate checking, and zero-size validation for layout bounds.
- Changed component/View lowering to derive deterministic layout bounds from
  existing `VStack`/`Column`, `HStack`/`Row`, `Stack`/`Surface`, `Fragment`,
  text, and button structure without new parser syntax.
- Changed action-button submit bounds to derive from the same component text
  control bounds table before falling back to legacy stacked bounds.
- Updated `tools/build-web-ime-player-rendered-fixture.rs` so the rendered IME
  fixture carries typed text-control layout records.

## Files Touched

- `crates/arcweft-bundle/src/resource_codec.rs`
- `crates/arcweft-bundle/src/resource_codec/ui/model.rs`
- `crates/arcweft-bundle/src/resource_codec/ui/codec.rs`
- `crates/arcweft-bundle/tests/ui_runtime_text_controls.rs`
- `crates/arcweft-bundle/tests/ui_resource_codecs.rs`
- `crates/arcweft-bundle/tests/ui_action_button_resources.rs`
- `crates/arcweft-bundle/tests/ui_focus_navigation_resources.rs`
- `crates/arcweft-cli/src/app/bundle.rs`
- `crates/arcweft-cli/src/app/bundle_view.rs`
- `tools/build-web-ime-player-rendered-fixture.rs`
- `docs/design/component-text-control-layout-bounds-resource-contract-2026-07-04.md`
- `docs/implementation/component-text-control-layout-bounds-resource-contract-2026-07-04.md`

## Runtime And Player Path

No new player renderer shape is introduced. The existing player path already
consumes `UiRuntimeTextControlBounds` and converts it into `HitRect`. This cut
changes the source of that runtime field from stacked defaults to the typed
program layout table when component bounds exist.

Action buttons still use the existing `UiRuntimeButtonBounds` and
`RenderActionButton` path. Their default submit placement now reads the target
text-control bounds from the same layout table, then falls back to the legacy
stacked bounds only for older resources.

## Tests Added Or Updated

- Component-only `TextField` deterministic runtime bounds.
- `TextArea` and `SecureField` deterministic runtime bounds.
- Compact UI resource round-trip preservation of layout bounds.
- Semantic target bounds agreement with text-control bounds.
- Missing bounds fallback to existing stacked slots.
- Invalid zero-size layout bounds rejection.
- Existing submit button placement with layout-bound target slots.
- Existing action button and focus navigation resource round trips.

## Non-Goals Retained

- Top-level `ui text_input`, `ui text_area`, and `ui secure_field`
  compatibility declarations remain removed.
- The submit action substrate from seq06.16/seq06.16.1 is not redesigned.
- Platform widget, DOM, CSS screenshot, and source-string fallback behavior are
  not introduced.
- Full style-driven layout resolution is deferred to the later style/runtime
  rendering cut.

## Validation

- `cargo test -p arcweft-bundle --test ui_runtime_text_controls -- --nocapture`
- `cargo test -p arcweft-bundle --test ui_resource_codecs -- --nocapture`
- `cargo test -p arcweft-bundle --test ui_action_button_resources -- --nocapture`
- `cargo test -p arcweft-bundle --test ui_focus_navigation_resources -- --nocapture`
- `cargo test -p arcweft-cli component_view_ -- --nocapture`
- `cargo test -p arcweft-runtime-driver text_submit -- --nocapture`
- `cargo test -p arcweft-player-scene --test runtime_text_controls -- --nocapture`
- `cargo fmt --all -- --check`
- `cargo run -p arcweft-cli -- bundle samples/modern-feedback-ui/src/main.arcw --output target/arcweft/modern-feedback-ui-layout-bounds.awfb`
- `cargo +nightly -Zscript tools/build-web-ime-player-rendered-fixture.rs --out target/arcweft/web-ime-player-rendered-layout-bounds.awfb`
- `cargo clippy -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene --all-targets -- -D warnings`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`

Structure audit completed as a dry run and reported the current workspace
baseline of `4 error(s), 127 warning(s)` without writing report files.
