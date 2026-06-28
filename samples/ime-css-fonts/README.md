# Arcweft IME CSS Font Sample

This sample records the current practical boundary for seq06 IME work.

## Web

Run:

```bash
just ime-sample-web
```

Then open `http://127.0.0.1:8786/ime-sample.html`.

Validation:

```bash
just ime-sample-check
```

The Web sample uses `EditContext` when the browser exposes it and does not use
hidden DOM text entry. The visual styling is restricted to CSS that maps cleanly
to the seq06 direct-wgpu feature set: rounded rects, borders, opacity,
transforms, linear gradients, text decoration, and explicit font stacks.

Fonts used by the sample:

- `Arcweft Demo` from `web/assets/arcweft-demo.ttf`
- `Yu Gothic`
- `Yu Mincho`
- `Hiragino Sans`
- `Hiragino Mincho ProN`
- `Noto Sans JP`
- `Noto Serif JP`
- platform `system-ui` fallbacks

## Native

Run:

```bash
just ime-sample-native
```

This executes `arcweft-desktop-native`'s runnable adapter contract sample. It
creates the same session-scoped Arcweft text-input events that a native IME
bridge must emit, including Japanese preedit and commit operations.

It is not yet a real OS IME window sample. Real native IME requires the remaining
platform object work:

- Windows: approved unsafe COM boundary for TSF `ITextStoreACP`, sink callbacks,
  document manager/context activation, and candidate rectangle callbacks.
- macOS: AppKit `NSView<NSTextInputClient>` lifecycle wiring from the Swift
  owner into the native window.
- Cross-platform player: focus-to-TextField session activation, geometry update
  pumping, and committed/preedit text painting in the actual windowed renderer.
