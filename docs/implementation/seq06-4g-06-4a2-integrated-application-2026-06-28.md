# Seq06.4g / Seq06.4a.2 Integrated Text Editing Application

This note records the production application of
`arcweft-seq06.4g-06.4a.2-integrated-text-editing-web-native-2026-06-28.zip`.

## Applied Scope

- Applied the shared text editing behavior owner in
  `arcweft-presentation::text_editor`.
- Updated `arcweft-presentation::text_index` so shared text movement uses the
  same byte/UTF-16/scalar/grapheme/word boundary owner.
- Added Web player-owned text-input wasm entry points in
  `arcweft-player-web::web_text_input`.
- Added `WebEditContextAdapter::dispatch_command` so keyboard and pointer
  commands flow through the typed text-input dispatch path.
- Replaced the Web IME sample's sample-owned state with player-owned
  `web/player-editcontext.js` glue.
- Updated the Web IME sample CSS so the Arcweft caret/selection/composition
  mirror is positioned by player geometry and the browser caret is hidden where
  the browser permits it.

## Native Overlay Decision

The integrated zip also contained inherited Windows TSF COM and macOS AppKit
overlay material. Those hunks were not applied in this cut because they add
platform object lifecycle and unsafe/build boundaries, and the current checkout
already has native adapter work that must be reconciled deliberately rather than
blind-copied over.

Native work remains assigned to the existing requests:

- `docs/reviews/requests/2026-06-28-seq-06.4b.1-windows-tsf-com-bootstrap-window-integration-package.md`
- `docs/reviews/requests/2026-06-28-seq-06.4c.1-macos-appkit-window-integration-package.md`

## Additional Follow-Up Requests

Two remaining gaps were split into independent follow-up packages:

- `docs/reviews/requests/2026-06-28-seq-06.4h-web-editcontext-real-ime-validation-package.md`
- `docs/reviews/requests/2026-06-28-seq-06.4i-renderer-backed-text-editor-geometry-package.md`

Seq06.4h should validate a real EditContext-capable browser with Japanese IME
preedit/commit and candidate placement evidence. Seq06.4i should replace
production monospaced/DOM-estimated geometry with renderer-backed glyph
geometry for TextField/TextArea.

## Validation

Executed in this checkout:

```bash
cargo fmt --all
cargo check -p arcweft-presentation --all-targets
cargo test -p arcweft-presentation --test text_editor_behavior -- --nocapture
cargo clippy -p arcweft-presentation --all-targets -- -D warnings
cargo clippy -p arcweft-player-web --all-targets -- -D warnings
cargo test -p arcweft-player-web --tests -- --nocapture
npm run test:ime
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
just test-workspace
```

Results:

- `arcweft-presentation` check, test, and clippy passed.
- `arcweft-player-web` clippy and tests passed.
- Web IME source, glue, caret/selection, and sample smoke passed.
- Structural audit passed with `0 error(s), 107 warning(s)`.
- `git diff --check` passed.
- `just test-workspace` passed.
- The local Playwright Chromium reported the sample as `unsupported`, so real
  candidate-window placement remains unverified until seq06.4h.

Blocked validation:

```bash
cargo check -p arcweft-player-web --target wasm32-unknown-unknown
```

This failed before reaching Arcweft code because `zstd-sys` required a C
compiler for the wasm target and `clang` was not installed in this environment.

## Design Deviations

- The package's native Windows/macOS overlay was intentionally not applied in
  this cut; it remains a separate platform-object bridge task.
- The shared editor keeps deterministic monospaced `TextEditorLayout` support
  for tests and minimal samples. Production renderer-backed geometry is tracked
  by seq06.4i.
- The Web sample smoke accepts typed unsupported state when the local browser
  lacks real `EditContext`; this is not considered real IME validation success.
