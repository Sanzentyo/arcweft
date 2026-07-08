# seq06.4k.1 Real Text Control Schema To PreparedFrame - 2026-06-29

## Summary

This cut applies the seq06.4k.1 package as a production implementation of the renderer/player text-input boundary. `PreparedFrame::focused_text_input_target()` no longer returns a hardcoded `None`; it returns a real `PreparedTextInputTarget` when the focused frame contains a typed `RenderTextInputControl`.

## Implemented

- Added `SemanticRole::SecureTextField` and role-owned text-input option normalization.
- Added `TextEditorState::from_text_control` for validated product/runtime supplied value and selection.
- Added `RenderTextInputControl` to `RenderScene`.
- Added renderer preparation of text-control semantics, hit rectangles, visible text blocks, focus ring, and focused IME snapshot/geometry through `TextEditorGeometryPump::layout_from_laid_out_text`.
- Split renderer text-control lowering into `crates/arcweft-render-wgpu/src/geometry/text_controls.rs` so `geometry.rs` stays below the structural warning threshold after this cut.
- Added player-owned live text editor state to `InputController`, including `activate_text_control`, `apply_live_text_control_state`, and `TextInput` mutation of the focused editor.
- Updated native and web player loops to propagate text editor errors from platform edits.
- Updated the seq06.4k source gate so hardcoded `PreparedFrame` text-input `None` is now rejected.
- Tightened the wasm `EditContext` rectangle helper to construct a real `DOMRect` when available.

## Non-Goals And Follow-Up

The normal product/runtime View schema still does not emit real `RenderTextInputControl` values into story frames. Existing native/web scene builders therefore pass `text_inputs: Vec::new()` until a typed product/runtime text-control collection is exposed.

Follow-up request:

- `docs/reviews/requests/2026-06-29-seq-06.4k.1.1-product-runtime-text-control-emission.md`

That follow-up must wire authored/runtime View text controls into `RenderScene::text_inputs` and use `InputController::apply_live_text_control_state` before frame planning.

## Validation

Commands run:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-render-wgpu focused_text_input_target --all-features -- --nocapture
cargo test -p arcweft-player-scene text_input --all-features -- --nocapture
cargo test -p arcweft-player-web runtime_text_input --all-features -- --nocapture
cargo test -p arcweft-player-native text_input --all-features -- --nocapture
npm --prefix web run test:ime
cargo check -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-player-web -p arcweft-player-native --all-targets --all-features
cargo clippy -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-player-web --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/source-gates/seq06_4k_text_input_windowed_gates.rs --root .
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Result: passed. Structure audit reported `0 error(s), 118 warning(s)`. Existing `arcweft-player-native` dead-code warnings remain in the legacy native audio/windowed modules and are tracked outside this cut.

Additional validation after installing LLVM/clang on the local Windows host:

```bash
cargo check -p arcweft-player-web --target wasm32-unknown-unknown --all-features
```

Result: passed. The previous `zstd-sys` C shim build blocker is resolved with
`clang` available on `PATH`. This check produced one pre-existing warning in
`arcweft-runtime-accelerator` for an unused `native_jit` import when compiling
the wasm target.

The wasm run also exposed an Arcweft-side type inference issue in
`WebEditContextAdapter::create_edit_context_object`; the `EditContext`
constructor is now explicitly converted to `js_sys::Function`. A stale unused
Rust-side `rect_init` DOMRect fallback was removed because runtime geometry
conversion is owned by `web/player-editcontext.js`.

## Design Deviations

The package patch attempted to convert `RenderTextInputControl::resolved_options` errors into an unrelated `TextEditorError` in `InputController::activate_text_control`. The production implementation keeps that as `FramePlanError`, because invalid semantic roles are renderer/input-schema errors rather than editor mutation errors.

Secure text display uses ASCII `*` mask glyphs in renderer-visible text blocks to match the repository editing policy. Platform snapshots and geometry are still redacted through `TextInputSecurityPolicy`.
