# seq06.4i renderer-backed TextEditorLayout geometry

Focused TextField/TextArea geometry is now sourced from Arcweft renderer/text-layout glyph output instead of DOM mirror estimates or production monospaced assumptions.

## Implemented decisions

- `arcweft-presentation` owns the Sans I/O geometry contract and secure redaction.
- `arcweft-render-wgpu` owns conversion from `arcweft-text-layout::LaidOutText` glyph clusters into `TextEditorLayout`.
- `TextEditorLayout` is source-tagged as `Renderer` or `MonospacedFixture`; production publishing requires the renderer source.
- `TextInputGeometrySnapshot` carries caret, selection, composition, control, and character/cluster bounds in the spaces required by adapters.
- Web EditContext glue consumes runtime geometry for control, selection, character bounds, and pointer hit-testing before any DOM fallback.

## Validation target

```bash
cargo fmt --all -- --check
cargo check -p arcweft-presentation -p arcweft-text-layout -p arcweft-render-wgpu -p arcweft-ui -p arcweft-runtime-host -p arcweft-player-web --all-targets
cargo test -p arcweft-presentation text_editor -- --nocapture
cargo test -p arcweft-render-wgpu text_editor_geometry -- --nocapture
cargo test -p arcweft-ui text_input_geometry -- --nocapture
cargo test -p arcweft-runtime-host text_input -- --nocapture
cargo clippy -p arcweft-presentation -p arcweft-render-wgpu -p arcweft-ui -p arcweft-runtime-host -p arcweft-player-web --all-targets --all-features -- -D warnings
npm run test:ime
cargo +nightly -Zscript tools/structure-audit.rs --root .
```
