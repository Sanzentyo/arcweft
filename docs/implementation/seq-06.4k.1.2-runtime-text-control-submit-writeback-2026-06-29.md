# Seq-06.4k.1.2 Runtime Text-Control Submit/Change Write-Back - 2026-06-29

## Source package

Prepared as `seq06-4k1-2-runtime-text-control-writeback.zip` for application
after seq06.4k.1.1.

## Applied scope

- Adds typed `TextControlWriteBackKind`, `TextControlValue`, and
  `TextControlWriteBack` to `arcweft-presentation`.
- Makes `InputController` emit change/submit write-backs from committed
  text-input operations.
- Keeps IME preedit/composition frame-local until commit.
- Extends `ViewRuntimeTextControl` with typed change/submit handler metadata
  derived from existing authored `ViewInputOptions::{change_handler,submit_handler}`.
- Adds `arcweft-runtime-driver::text_control_writeback` as the runtime-facing
  event type.
- Makes `BundleSession` the owner of the runtime text-control value overlay and
  pending typed write-back queue.
- Routes native and Web players through
  `BundleSession::queue_text_control_write_backs`.
- Preserves matching values/selection deterministically across hot-swap.
- Adds redaction in write-back debug output and runtime text-control diagnostic
  formatting.

## Deferred runtime handler behavior

This cut exposes typed runtime/AWBC-facing write-back events and maps authored
handler ids into runtime text-control metadata. It intentionally does not invoke
arbitrary AWBC handler functions from those events.

The remaining AWBC call boundary needs a separate reviewed ABI because secure
text values must not pass through generic diagnostics, replay, capture metadata,
or `InteractionPayload::Text`. Until that ABI exists, runtime/AWBC-facing logic
can observe typed write-back events and handler metadata, but handler execution
is not automatic.

## Security

Secure values remain available to the runtime owner through `TextControlValue`,
but `Debug` and observation helpers redact them. Platform IME activation still
uses `PreparedFrame::focused_text_input_target()` and the existing secure
snapshot/geometry redaction.

## Validation

Applied checkout validation:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-presentation text_control_write_back
cargo test -p arcweft-bundle runtime_text_control_carries_authored_change_and_submit_handlers
cargo test -p arcweft-player-scene text_input
cargo test -p arcweft-player-scene input::tests
cargo test -p arcweft-player-scene --test runtime_text_controls
cargo test -p arcweft-player-scene --test text_control_writeback_source_gates
cargo test -p arcweft-runtime-driver text_control_writeback
cargo clippy -p arcweft-presentation -p arcweft-bundle -p arcweft-player-scene -p arcweft-runtime-driver -p arcweft-player-native -p arcweft-player-web --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The structural audit scanned 2001 files, including 1018 Rust files and 480103
Rust physical LOC, with 0 errors and 121 warnings.

Manual platform validation with real browser EditContext, Windows TSF, macOS
AppKit IME, Wayland text-input, Android InputConnection, and iOS text input is
outside this patch package and must be run by a platform host checkout.
