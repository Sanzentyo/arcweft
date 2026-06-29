# seq06.4a.3 Web Player Runtime Text Input Bridge

Date: 2026-06-28

## Status

This overlay adds the Web counterpart to the seq06.4j native player bridge.  The
normal Web player owns the active text-input bridge from `app.rs`; JavaScript
owns only browser `EditContext` object identity and DOM event listeners.

## Implemented

- Added `arcweft-player-web::runtime_text_input::WebPlayerTextInputBridge`.
- Replaced sample-style `web_text_input` activation with runtime callback exports.
- Added `WebEditContextAdapter::update_snapshot` and composition state access.
- Integrated the bridge into normal Web player startup, redraw, focus loss,
  resize/client geometry refresh, keyboard routing, and platform edit draining.
- Added JS runtime command consumption in `web/player.js` and
  `web/player-editcontext.js`.
- Added source gates for no hidden DOM fallback and no sample-owned normal path.
- Added fixture and validation instructions for browser-real EditContext testing.

## Explicit gap inherited from current main

`PreparedFrame::focused_text_input_target()` still returns `None` in the inspected
main.  Therefore this overlay cannot claim real browser IME acceptance until the
renderer/UI layer publishes real focused `PreparedTextInputTarget` snapshots for
Arcweft `TextField`, `TextArea`, and `SecureField` controls.

The bridge deliberately does not fabricate a focused target, does not revive
`sample_snapshot()`, and does not use DOM mirror text as production model data.

## Validation commands

```bash
cargo fmt --all -- --check
cargo test -p arcweft-player-web --all-features -- --nocapture
cargo check -p arcweft-player-web --all-targets --all-features
cargo clippy -p arcweft-player-web --all-targets --all-features -- -D warnings
npm --prefix web run test:ime
cargo check -p arcweft-player-web --target wasm32-unknown-unknown
```

Browser-real acceptance must additionally use an EditContext-capable browser and
capture candidate-window geometry near the Arcweft-rendered caret.

## Repository application notes

Applied to the local repository on 2026-06-29 after later seq06 cuts were
already present. The package patch file itself was not valid `git apply` input,
so the overlay source files were copied and the integration hunks were
reconciled manually against current `app.rs`, `player.js`, and
`player-editcontext.js`.

Current `player-editcontext.js` already contained the seq06.4 caret/selection
geometry improvements, so this application only added runtime command ownership
and removed the old JS-initiated `activate(initialText)` delegate path. Runtime
status emission is driven by command handling; `updateFromRuntimeSnapshot()` no
longer emits an extra `runtime_update` status before `runtime_activated`.

## Repository validation status

Executed from the local repository after applying:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-player-web --all-targets --all-features
cargo test -p arcweft-player-web --all-features -- --nocapture
cargo clippy -p arcweft-player-web --all-targets --all-features -- -D warnings
npm --prefix web run test:ime
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Results:

- all `arcweft-player-web` unit, integration, source-gate, and doc tests passed;
- `npm --prefix web run test:ime` passed, including
  `player-editcontext-runtime-bridge-unit.mjs`;
- clippy passed with `-D warnings`;
- structural audit completed with `0 error(s), 117 warning(s)` across `1942`
  scanned files and `997` Rust files;
- `git diff --check` passed.

The wasm target validation was re-run after LLVM/clang became available on the
local Windows host:

```bash
cargo check -p arcweft-player-web --target wasm32-unknown-unknown
```

The later full-feature command also passed:

```bash
cargo check -p arcweft-player-web --target wasm32-unknown-unknown --all-features
```

`zstd-sys` now builds its C shim successfully. The wasm check produced one
pre-existing warning in `arcweft-runtime-accelerator` for an unused
`native_jit` import when compiling the wasm target.
