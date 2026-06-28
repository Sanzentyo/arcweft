# Seq06.4a.1 Web EditContext Player-Owned Glue

Date: 2026-06-28

## Applied Scope

This slice moves Web `EditContext` browser glue from the sample into the Arcweft Web player boundary.

Implemented by the overlay:

- Added `arcweft-player-web::web_text_input` as the wasm/value bridge for player-owned text-input installation and event dispatch.
- Kept UTF-16 range normalization, secure redaction, and runtime-host dispatch in the existing `arcweft-player-web::edit_context` adapter rather than duplicating platform logic in JavaScript.
- Added `web/player-editcontext.js` as the browser-object owner for `EditContext`, element association, event listeners, geometry updates, status events, and sample mirror rendering.
- Updated `web/player.js` so normal startup installs Arcweft Web text input by default and exposes `setupArcweftWebTextInput` for thin samples/custom host setup.
- Demoted `web/ime-sample.js` to a status consumer that no longer creates `EditContext`, handles `textupdate`, owns composition state, owns model text, or installs keyboard insertion fallback.
- Extended Rust and Node source gates to prevent hidden DOM editing substitutes and sample-side event ownership from returning.
- Added browser fixture/test paths for ready and unsupported cases.

## Behavioral Decisions

- Unsupported browsers report `unsupported_no_fallback`; the Web player remains explicit and does not create a hidden editing substitute.
- Keyboard events are not converted into text insertion. Normal shortcuts route through existing player input paths and are suppressed while IME composition is active.
- `textupdate` payloads are primitive values only and are normalized through `TextIndexSnapshot` by `WebEditContextAdapter`.
- Geometry updates are host commands, not text batches.
- Secure fields narrow capabilities, mark text input as sensitive, and redact surrounding text, character bounds, status text, Agent-observable values, replay values, and capture metadata.

## Validation Commands

Run after applying the overlay:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-player-web --lib edit_context --all-features
cargo test -p arcweft-player-web --test web_text_input_glue --all-features
cargo test -p arcweft-player-web --test web_edit_context_source_gate --all-features
cargo check -p arcweft-player-web --all-targets --all-features
cargo clippy -p arcweft-player-web --all-targets --all-features -- -D warnings
node web/tests/ime-sample-source.mjs
node web/tests/player-editcontext-glue-unit.mjs
node web/tests/ime-sample-smoke.mjs
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

## Validation Status From Package Creation

This package was assembled from connector-inspected source and statically checked in a sandbox. A live Arcweft checkout, Rust toolchain execution, wasm build, and real browser execution were not available inside the packaging sandbox. The included `verification/VALIDATION.md` records which checks were run locally against the package and which must be run after applying to the repository.

## Remaining Non-Goals

- Windows, macOS, Wayland, Android, and iOS implementation.
- Rich document editing beyond current TextField/TextArea/SecureField scope.
- Replacing the seq06.3 common text-input contract.
