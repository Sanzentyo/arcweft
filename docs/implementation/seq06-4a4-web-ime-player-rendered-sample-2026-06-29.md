# Seq06.4a.4 Web IME Player-Rendered Sample - 2026-06-29

## Summary

The active Web IME sample is moved from visible DOM textbox/mirror/caret/status
UI to a normal Arcweft Web player sample. `web/ime-sample.html` remains as a
compatibility URL, but it is now a thin host for `web/ime-player-rendered.awfb`.

## Implemented boundary

```text
thin HTML host
  -> #arcweft-canvas
  -> startArcweftWebPlayer(...)
  -> product/runtime TextField/TextArea/SecureField fixture
  -> RuntimeTextControlLowerer
  -> SharedFramePlanner::prepare
  -> PreparedFrame::focused_text_input_target()
  -> WebPlayerTextInputBridge
  -> player-owned invisible EditContext glue
```

## Removed from active sample

- `div[role=textbox]`
- committed/composition mirror spans
- CSS `.caret` and caret variables
- visible status/selection/font output cards
- sample-owned `EditContext` or text model

## Renderer and geometry

Caret/selection evidence is produced by renderer text-control frame planning. Active preedit composition geometry is observed through runtime commands, and a renderer-visible underline should be enabled once composition ranges are carried in product/runtime control state.
`EditContext` candidate geometry is synchronized from the focused prepared text
input target and runtime geometry commands. DOM canvas bounds are only a client
coordinate transform/fallback, not the source of text-control geometry.

## Validation

Validated in the receiving checkout:

```bash
cargo +nightly -Zscript tools/build-web-ime-player-rendered-fixture.rs --out web/ime-player-rendered.awfb
cargo build -p arcweft-player-web --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir web/pkg --out-name arcweft_player_web target/wasm32-unknown-unknown/debug/arcweft_player_web.wasm
npm --prefix web run test:ime
node web/tests/editcontext-real-ime-harness.mjs --mode unsupported --output-dir target/web-editcontext-real-ime
just ime-sample-check
cargo fmt --all -- --check
cargo clippy -p arcweft-cli -p arcweft-player-native -p arcweft-player-scene -p arcweft-render-wgpu --features arcweft-cli/native-player --all-targets -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The local Playwright/WebGPU smoke reported `environment_blocked` because no
WebGPU adapter was available for the canvas surface in this environment. The
source gates, invisible `EditContext` glue units, wasm build/bindgen, generated
AWFB fixture, and non-interactive unsupported-mode real-IME harness passed.

Real Japanese IME validation still requires an interactive browser exposing
usable `EditContext` plus a real IME session. Trace output should be written
under `target/web-editcontext-real-ime/` unless it is deliberately reviewed and
promoted.
