# Seq06 IME Caret, Candidate Geometry, And Selection Gap Analysis (2026-06-28)

## User-Visible Symptoms

The current Web IME sample can accept simple IME text, but it is not a complete
Arcweft text-field integration:

- Japanese conversion candidates can appear at the upper-left of the browser
  viewport instead of near the edited text.
- A browser/native caret can be visible while Arcweft's teal sample caret is
  separately rendered and not synchronized.
- Arrow-key caret movement, mouse click positioning, drag selection, shortcut
  selection, deletion, and selected-text movement are not implemented in the
  sample path.

## Current Implementation Evidence

- `web/ime-sample.js` owns a local `modelText` string, constructs
  `window.EditContext`, and listens only for `textupdate`, `compositionend`, and
  `textformatupdate`.
- The sample renders committed text plus one composition span. It does not
  render selection ranges, split text runs around the caret, or move the
  `.caret` element by model selection.
- The sample does not handle `keydown`, pointer hit-testing, selection drag,
  clipboard shortcuts, delete commands, or movement commands.
- The sample does not send caret or character-bound geometry back to
  `EditContext`, so candidate placement has no reliable text-local anchor.
- `crates/arcweft-player-web/src/edit_context.rs` has a typed adapter core for
  activation, text updates, UTF-16 conversion, composition, secure redaction, and
  geometry host commands, but the sample does not use player-owned glue.
- `arcweft-presentation::text_input` already has `TextEditCommand`,
  `TextInputClientSnapshot`, and `TextInputGeometrySnapshot`, but the current
  Web sample is not a real editor session that applies those commands to a text
  model and refreshed geometry.

## Assessment

This is primarily missing implementation, not one isolated implementation bug.
The code that exists proves the lower-level adapter vocabulary, but the sample
is still a temporary DOM demo. It does not yet have the player-owned editor
state, caret geometry pump, visual caret policy, hit testing, selection model,
or edit-command application that a product Arcweft TextField needs.

There are two likely implementation issues that should still be checked while
finishing the missing work:

- The static `.caret` in `web/ime-sample.html` should not be treated as an
  editor caret. It is always rendered after the composition span and cannot
  represent arbitrary collapsed or ranged selection.
- If the browser exposes an EditContext-native caret for the associated element,
  the Web player must either suppress visible native caret styling when possible
  or keep native and Arcweft caret state strictly synchronized. Arcweft-rendered
  text fields should present one visible caret, owned by Arcweft.

## Resulting Requests

- `docs/reviews/requests/2026-06-28-seq-06.4a.2-web-editcontext-caret-geometry-selection-package.md`
- `docs/reviews/requests/2026-06-28-seq-06.4g-cross-platform-text-editing-behavior-package.md`

Seq06.4a.2 should be applied before treating the Web sample as representative
of production IME behavior. Seq06.4g should be designed in parallel with native
adapter window-integration packages and then used as the acceptance baseline for
Windows, macOS, Wayland, Android, and iOS.
