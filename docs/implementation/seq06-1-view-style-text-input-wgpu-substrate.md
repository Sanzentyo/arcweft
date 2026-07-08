# seq06.1 View style, text input, and direct wgpu substrate

## Goal

Implement the seq06.1 Sans I/O UI/style/text-input substrate and renderer
contract for direct wgpu UI rendering. This note records the implementation
slice for the request file dated 2026-06-27.

## Implemented decisions

- Public display text remains a single `Text` view; rich, localized, and
  display-frame sources stay as retained `ViewTextSource` variants.
- Style authoring keeps Arcweft syntax as the default and models `.Css` as an
  explicit style syntax variant.
- External and embedded CSS are represented by typed `from file(...)` and
  `from embed(...)` source nodes rather than raw strings.
- Component `.style {}` and `.style(.Css) {}` overrides are retained as ordered
  `StyleOverrideLayer` values.
- Component internals are targetable through typed exported parts (`ViewPartId` and
  `StylePartId`) rather than structural selectors into private nodes.
- Dark mode, contrast, reduced motion, text scale, locale, and revision stay in
  `PresentationEnvironment`.
- Text input routing now carries session-scoped atomic `TextInput` batches in
  both `RawInputKind::Text` and `InputEventKind::Text`.
- TextField editing applies batches atomically after session validation; preedit
  composition updates affect the visual buffer only and do not mutate the
  committed document until commit or committed composition end.
- Secure text input can mark text batches as `TextInputPrivacy::Sensitive` so
  replay hashes include deterministic structure and lengths but not text payload
  bytes.
- Renderer-facing UI data lowers TextField selection/caret/composition geometry
  into `ViewScene` primitives without DOM/textarea or CPU-raster fallback paths.

## Explicit Non-Goals

- Full Takumi CSS/layout/stacking lowering remains seq06.2.
- Platform IME adapter implementations remain seq06.3 and do not enter
  presentation, UI, or render-wgpu crates.
- Product View compact resource codecs remain seq02.4.1.

## Verification Target

Run these commands at the review cut point:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-presentation -p arcweft-view -p arcweft-lang-syntax -p arcweft-render-wgpu
cargo test -p arcweft-presentation -p arcweft-view -p arcweft-lang-syntax -p arcweft-render-wgpu
cargo clippy -p arcweft-presentation -p arcweft-view -p arcweft-lang-syntax -p arcweft-render-wgpu --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

## Current Verification

This implementation was validated on the current checkout with:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-presentation -p arcweft-view -p arcweft-lang-syntax -p arcweft-render-wgpu
cargo test -p arcweft-presentation -p arcweft-view -p arcweft-lang-syntax -p arcweft-render-wgpu --quiet
cargo clippy -p arcweft-presentation -p arcweft-view -p arcweft-lang-syntax -p arcweft-render-wgpu --all-targets --all-features -- -D warnings
cargo test -p arcweft-cli --test regression_harness --quiet
cargo +nightly -Zscript tools/structure-audit.rs --root .
just test-workspace
```

The structural audit reported 0 errors and the existing warning set. The first
`just test-workspace` attempt hit a transient `arcweft-render-native --lib`
process access violation; rerunning that crate alone passed, and a full
`just test-workspace` rerun passed.

## Structural Audit Notes

The changed production Rust modules remain responsibility modules under the
existing crate boundaries:

- `arcweft-presentation::input`, `router`, `replay`, and `text_input` own pure
  input/focus/replay data. They do not depend on UI, renderer, OS, IME, or wgpu.
- `arcweft-view::style`, `style_authoring`, `program`, and `text_field` own
  authoring/retained View data and editor state. They do not perform rendering or
  platform I/O.
- `arcweft-lang-syntax::ast::{style, view}` owns syntax surfaces only.
- `arcweft-render-wgpu::view_scene` owns renderer-facing primitive data and has no
  platform adapter API.

No `unsafe`, DOM/textarea fallback, keydown-character insertion path, or CPU
raster surface path is introduced.

## Follow-Up Boundaries

seq06.2 should consume the `ViewScene` primitive contract and add Takumi lowering.
seq06.3 should translate TSF, Cocoa, UIKit, Android, Wayland, Web EditContext,
or other platform IME APIs into `TextInput` batches and host commands without
leaking those APIs into the shared crates.
