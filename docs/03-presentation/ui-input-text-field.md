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
then maps it through component/layer/viewport transforms. The platform adapter
receives that rect through a `TextInputHostCommand::Update` snapshot.

## Focus/session safety

Text input batches carry `TextInputSessionId`. A stale commit from a previous
focus generation is rejected. Forced blur due to modal/window/target removal
uses the configured composition-on-blur policy and then invalidates the session.
