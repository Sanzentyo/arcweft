# Modern Feedback View Reveal And Scope Runtime Cut

Date: 2026-07-09

## Summary

This cut separates visual dialogue reveal from runtime dialogue advance and
uses normal lexical View scopes for the `modern-feedback-view` sample:

- `InputOutcome` now reports `DialogueProgress::{None, Reveal, Advance}` rather
  than a raw advance bool. Reveal wins when outcomes are merged, so a click,
  Enter, Backspace, or text submit cannot both finish the current line reveal
  and advance that same line in one input batch.
- Native and web players complete the current visual line when
  `DialogueProgress::Reveal` is returned. They queue runtime dialogue advance
  only for `DialogueProgress::Advance`.
- View-owned runtime controls now require a visible presentation handle before
  they are render-visible. Direct runtime controls without a View owner remain
  default-visible.
- Retain-style handle behavior is preserved: when a matching visible handle is
  present, older hidden/released records for the same resource do not hide it.
- `modern-feedback-view` now has separate one-line name and multi-line brief
  panels, each mounted by ordinary flow scope lifetime.

## Structural Audit

Command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-codex-reveal-scope
```

Result:

- Files scanned: 2554
- Rust files: 1180
- Rust physical LOC: 588831
- Violations: 2 errors, 153 warnings

Changed Rust file metrics:

| Path | Crate | Kind | Bytes | Physical LOC | Embedded tests | Responsibilities |
| --- | --- | --- | ---: | ---: | --- | --- |
| `crates/arcweft-player-scene/src/input.rs` | `arcweft-player-scene` | production | 105326 | 2960 | yes | Shared native/web input routing, focus, text editing, controller normalization, dialogue progress outcomes |
| `crates/arcweft-player-native/src/scene_windowed.rs` | `arcweft-player-native` | production | 68558 | 1854 | yes | Native window event loop, prepared frame application, visual clock, input outcome application |
| `crates/arcweft-player-web/src/app.rs` | `arcweft-player-web` | production | 32721 | 857 | no | Web player state loop, visual clock, input outcome application |
| `crates/arcweft-render-wgpu/src/geometry.rs` | `arcweft-render-wgpu` | production | 77354 | 2420 | no | Shared prepared frame geometry, dialogue paragraph reveal state, runtime control geometry |
| `crates/arcweft-runtime-driver/src/display.rs` | `arcweft-runtime-driver` | production | 53856 | 1432 | yes | Bundle presentation snapshot state and filtered text input replacement |
| `crates/arcweft-runtime-driver/src/presentation_handles.rs` | `arcweft-runtime-driver` | production | 40935 | 1212 | yes | Presentation handle lifecycle and render-visible filtering |
| `crates/arcweft-cli/tests/native_text_input_sample_sidecars.rs` | `arcweft-cli` | test | 4914 | 93 | no | Sample source assertions |
| `crates/arcweft-cli/tests/native_text_input_native_interactive_smoke.rs` | `arcweft-cli` | test | 6862 | 190 | no | Native text-input smoke sample assertions |
| `crates/arcweft-player-scene/tests/action_button_submit.rs` | `arcweft-player-scene` | test | 12975 | 338 | no | Action button submit regressions |

The audit reports error-level size violations for:

- `crates/arcweft-cli/src/app/bundle_view.rs`: 2590 physical LOC
- `crates/arcweft-player-scene/src/input.rs`: 2960 physical LOC

This cut intentionally does not split `input.rs`: the behavior change spans
pointer, keyboard, controller, and text-submit input paths, and splitting the
module in the same cut would mix a semantic runtime fix with a broad ownership
refactor. The next structural step should extract cohesive input modules such
as dialogue progress, pointer activation, text editing, controller
normalization, and tests without changing behavior.

## Validation

Commands run:

```bash
cargo fmt
cargo test -p arcweft-runtime-driver presentation_handles -- --nocapture
cargo test -p arcweft-player-scene dialogue -- --nocapture
cargo test -p arcweft-cli --test native_text_input_sample_sidecars -- --nocapture
cargo test -p arcweft-cli --test native_text_input_native_interactive_smoke -- --nocapture
target/debug/arcw.exe check samples/modern-feedback-view/src/main.arcw
target/debug/arcw.exe bundle samples/modern-feedback-view/src/main.arcw --output web/modern-feedback-view.awfb
cargo check -p arcweft-runtime-driver -p arcweft-player-web -p arcweft-player-native -p arcweft-render-wgpu -p arcweft-player-scene
cargo clippy -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-player-web -p arcweft-player-native -p arcweft-render-wgpu --all-targets --all-features
just web-player-refresh
```

Browser verification on
`http://127.0.0.1:4173/?bundle=./modern-feedback-view.awfb` confirmed:

- initial scope shows only the one-line name panel;
- clicking during reveal completes the line instead of entering receive;
- after advancing into receive, the Continue action reaches the anonymous name
  branch;
- advancing past that branch drops the name panel and shows only the multi-line
  brief panel;
- no browser console warnings or errors were observed during the checked path.

Known validation notes:

- `just web-player-refresh` still emits existing AWF0103 hints for
  `web/src/main.arcw` explicit image ids.
- Focused clippy exits successfully but reports existing warnings outside this
  cut, including syntax AST large enum variants, runtime-driver `Option<Option>`
  helpers, web text-input bool parameters, and native clipboard match style.
