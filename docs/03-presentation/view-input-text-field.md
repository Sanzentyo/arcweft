# Text input and in-progress IME display

## Committed text vs composition overlay

Japanese input must display preedit text without mutating the committed Binding
value. A `TextEditState` owns committed document text and an optional
`TextComposition` overlay. Rendering creates a `TextVisualBuffer`:

```text
document before replacement
+ composition.preedit
+ document after replacement
```

Commit turns the overlay into a single committed edit transaction. Cancel drops
it. Preedit updates only repaint layout/parts.

## Visual parts

- `Content`: committed and composition glyphs;
- `Placeholder`: placeholder text;
- `Selection`: selected range backgrounds;
- `Caret`: caret rect;
- `Composition`: ordinary preedit underline/background;
- `CompositionTarget`: active conversion target underline/background.

## Candidate window geometry

The layout system computes composition cursor geometry in text-local coordinates,
then maps it through view/layer/viewport transforms. The platform adapter
receives that rect through a `TextInputHostCommand::Update` snapshot.

## Focus/session safety

Text input batches carry `TextInputSessionId`. A stale commit from a previous
focus generation is rejected. Forced blur due to modal/window/target removal
uses the configured composition-on-blur policy and then invalidates the session.

## Clipboard and secure fields

Text editing remains Sans I/O. The editor emits a typed `TextClipboardIntent`
for copy, cut, paste, or clear; each intent carries its interaction target,
`TextInputSessionId`, text revision, operation, and origin. The player scene
assigns a `TextClipboardRequestId`, retains the pending request, and returns a
typed `TextClipboardOutcome` to the focused editor. A paste result is applied
only while its request, target, session, and focused frame still match. The
runtime-host adapter owns capability checks and host access; presentation and
text-layout code do not call OS or browser clipboard APIs.

The default `ClipboardCapabilityPolicy` permits reads and writes only for
user-initiated text-control operations and denies clear. Secure fields reject
copy, cut, and paste before selection or clipboard text is exposed; the host
policy independently blocks secure copy/cut and denies secure paste by
default. The editor's secure-redaction policy also disallows clipboard access,
so a host policy override cannot bypass that guard. The in-app
`TextEditorLocalClipboard` is a fallback, not evidence of a successful host
write. Copy, cut, deletion, and paste replacement use `TextIndexSnapshot`;
non-collapsed byte selections expand to grapheme boundaries before text is
copied or removed. `ClipboardText` is plain UTF-8, normalizes CRLF and bare CR
to LF, and redacts its payload in `Debug` output.
