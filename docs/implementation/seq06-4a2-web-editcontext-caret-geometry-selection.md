# Seq06.4a.2 Web EditContext Caret Geometry And Selection

This overlay layers on top of seq06.4a.1 player-owned Web EditContext glue.

## Ownership

- Browser `EditContext` object identity, event listeners, geometry calls, and DOM mirror rendering remain inside `web/player-editcontext.js`.
- The wasm boundary forwards text updates and keyboard commands as typed values to `arcweft-player-web`; user-authored pages still only opt in through `setupArcweftWebTextInput`.
- The sample is a thin status/mirror consumer and does not instantiate `EditContext` or own IME conversion.

## Behavior added

- Candidate geometry is updated through `updateControlBounds`, `updateSelectionBounds`, and `updateCharacterBounds` using host/client coordinates.
- The browser/native caret is suppressed where CSS permits via `caret-color: transparent`; the Arcweft mirror renders one `.caret` positioned from the editor selection/caret snapshot.
- Pointer down/drag/up maps client X to UTF-16 grapheme slots and dispatches selection-only text updates.
- Keyboard movement, word movement, home/end, delete/backspace, select-all, copy/cut/paste, submit, and cancel route through `TextEditCommand` labels and local mirror updates.
- IME `textformatupdate` and composition ranges are rendered as Arcweft mirror spans without making the sample own the state machine.
- Secure mode redacts text, selection-derived caret geometry, clipboard contents, and character bounds.

## Browser evidence

The W3C EditContext specification states that EditContext exposes text, selection, composition, control bounds, selection bounds, and codepoint rectangles; that bounds are client-coordinate CSS pixels; that scroll can require geometry refresh; and that canvas-associated EditContext hosts require authors to implement caret navigation and selection. The implementation uses those facts to make the Web player the owner of geometry pumping and selection behavior.
