# seq06.4h Web EditContext real IME validation

## Status

This implementation note adds a real-browser validation harness and trace fixtures for
Arcweft Web `EditContext` Japanese IME validation. It does not redesign the seq06.3
text-input contract, the shared `TextEditorState`, or the player-owned Web glue.

The current package is a validation overlay, not a production behavior rewrite. The
available source evidence already shows that Arcweft's Web sample installs the
player-owned `EditContext` path and no DOM text-entry fallback. Real Japanese IME
candidate-window validation remains a desktop/browser/OS task because the packaging
sandbox has no interactive Japanese IME surface.

## Added files

- `web/tests/editcontext-real-ime-harness.mjs`: Playwright headed/manual recorder for an
  installed EditContext-capable Chrome, Edge, or Chromium browser. It records
  `EditContext` method calls, Arcweft status/render events, keyboard/pointer events,
  screenshots, and pass/fail analysis.
- `fixtures/web-editcontext-real-ime/trace-schema.json`: JSON schema for redacted traces.
- `fixtures/web-editcontext-real-ime/manual-session-template.json`: operator template for
  repeatable Japanese IME sessions.
- `fixtures/web-editcontext-real-ime/blocked-headless-chromium-2026-06-28.json`: evidence
  that the packaging sandbox could not complete real IME validation.
- `fixtures/web-editcontext-real-ime/unsupported-browser-synthetic.json`: fixture shape for
  the unsupported-no-fallback path.

## Browser requirement

Run the real harness in a headed desktop browser whose runtime satisfies all of these
checks:

```js
typeof window.EditContext === "function"
"editContext" in document.getElementById("arcweft-ime-surface")
```

The preferred channels are installed Google Chrome Stable or Microsoft Edge Stable. If a
Stable build does not expose `EditContext`, use Chrome Dev/Canary or Edge Dev/Canary and
keep the harness's runtime evidence in the trace. The harness passes
`--enable-blink-features=EditContext` so older Chromium-family builds with a gated
implementation can be tested, but the trace is still blocked unless the runtime exposes
the constructor and element association.

Bundled Playwright Chromium or headless execution is not accepted as real IME proof. It
is allowed only for the unsupported-browser and source-gate paths.

## Commands

```bash
cd web
npm install
npm run test:ime
ARCWEFT_REAL_IME_CHANNEL=chrome npm run test:ime:real
ARCWEFT_REAL_IME_MODE=unsupported npm run test:ime:real
```

For local browser binaries:

```bash
ARCWEFT_REAL_IME_BROWSER="/path/to/Google Chrome" npm run test:ime:real
```

Set `ARCWEFT_REAL_IME_ALLOW_TEXT=1` only for intentionally non-sensitive local fixtures.
Secure-field traces must leave raw text, clipboard text, and selection-derived sensitive
geometry redacted.

## Applied checkout validation

On 2026-06-28, this checkout validated the non-interactive parts of the package:

```bash
cd web
npm run test:ime
ARCWEFT_REAL_IME_MODE=unsupported ARCWEFT_REAL_IME_OUTPUT_DIR="$TEMP/arcweft-real-ime-unsupported" npm run test:ime:real
```

Both commands passed. The unsupported-mode run wrote its executable trace to a temporary
directory rather than adding generated evidence files to the repository.

The headed real Japanese IME session was not run in this Codex environment because the
harness intentionally waits for a human-controlled desktop browser and OS Japanese IME.
That session remains required before treating seq06.4h as final acceptance evidence.

## Real IME operator script

1. Focus `#arcweft-ime-surface`.
2. Enable the OS Japanese IME.
3. Type `nihongo`, convert to `日本語`, and commit.
4. During composition, exercise ArrowLeft and Shift+ArrowLeft.
5. Click and drag inside the text surface to create a ranged selection.
6. Exercise Backspace, Delete, and Select All.
7. Click the in-page **Finish trace** button.

The harness then writes a trace JSON and screenshot under
`fixtures/web-editcontext-real-ime/` and fails if geometry, caret, command, pointer, or
fallback assertions do not pass.

## Validation boundary

A passing real trace must include all of the following:

- `compositionstart`, `textupdate`, and `compositionend` events from the real
  `EditContext` object.
- Non-origin `updateSelectionBounds` and `updateCharacterBounds` calls.
- Geometry that tracks the latest Arcweft caret or composition range within the harness
  tolerance.
- Exactly one Arcweft mirror caret and transparent native CSS caret color.
- Keyboard movement, ranged selection, pointer selection, Backspace, Delete, and Select
  All status events.
- No `textarea`, `input`, or `contenteditable` text-entry fallback in the sample DOM.
