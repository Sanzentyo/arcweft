# Seq-06.4k.1.1 Product Runtime Text Control Emission - 2026-06-29

## Source Package

Applied from:

```text
D:/sanze/Downloads/seq06-4k1-1-product-runtime-text-control-emission.zip
```

The package patch was mostly applicable to current `main`. One hunk in
`crates/arcweft-player-native/src/scene_windowed.rs` conflicted because the
native player windowed clippy cleanup had already moved audio ownership into the
current scene-windowed path. The rejected hunk only added the runtime
text-control lowering error variant, and it was ported manually.

## Applied Scope

- Added runtime text-control emission to the typed UI resource model.
- Added `UiRuntimeTextControl`, bounds, selection, and runtime-facing options.
- Added deterministic text-input session identity from authored public ids.
- Extended `BundlePresentationSnapshot` with `text_inputs`.
- Populated runtime text controls when a `BundleSession` is built or
  hot-swapped.
- Added `arcweft-player-scene::text_controls::RuntimeTextControlLowerer`.
- Routed native and Web scene builders through the same lowerer before
  `SharedFramePlanner::prepare`.
- Added unit tests for product/runtime emission and player-scene lowering.
- Documented the native/Web scene-builder boundary in code comments instead of
  adding a brittle string-scanning source gate for `text_inputs: Vec::new()` or
  DOM fallback tokens.

## Intentional Boundaries

This cut is player-owned and frame-local. Text commits and selection changes
update `InputController` live editor state and are reflected in the next planned
frame. Persistent write-back into AWBC/product state is intentionally not
implemented here; that requires a typed submit/change-handler boundary and is
split to seq06.4k.1.2.

No platform-specific TSF/AppKit/Wayland/Android/iOS/EditContext adapter behavior
was changed. Native and Web still activate IME only from
`PreparedFrame::focused_text_input_target()`.

This cut intentionally avoids a seq06.4k.1.1-specific source gate for hidden DOM
fallback or empty text-input vector strings. Those checks are too tightly coupled
to source spelling and would make normal refactors noisy. The contract is kept
in this note, the request documentation, shared lowerer comments, and behavioral
tests around runtime text-control lowering and focused prepared targets.

## Validation

Passed:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-bundle --test ui_runtime_text_controls
cargo test -p arcweft-player-scene --test runtime_text_controls
cargo clippy -p arcweft-bundle -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-player-native -p arcweft-player-web --all-targets --all-features -- -D warnings
```

Platform manual validation with real browser EditContext, Windows TSF, and
macOS AppKit IME remains outside this package.
