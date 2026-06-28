# Seq06 IME CSS Font Sample

## Outcome

This cut adds a runnable sample for the parts of seq06 that can be exercised
today:

- Web: `web/ime-sample.html` uses browser `EditContext` when available, with no
  hidden DOM text-entry fallback.
- Native: `arcweft-desktop-native --example ime_text_input_contract` emits the
  same Arcweft `PlatformTextInputEvent` batch that a Windows TSF bridge must
  send after Japanese preedit and commit callbacks.
- Styling: the Web sample uses CSS-compatible decoration that stays inside the
  direct-wgpu-safe subset: rounded rectangles, borders, opacity, transforms,
  linear gradients, and text decoration. It also demonstrates a multi-font stack
  including the checked-in `Arcweft Demo` font and Japanese system fallbacks.

## Current Native Boundary

A true OS-native IME window sample is not possible yet from the current
production code. The seq06.4 native work has safe adapter cores, but the window
runtime does not yet own platform object wiring:

- Windows TSF still needs an approved unsafe COM implementation boundary for
  `ITextStoreACP`, sink callbacks, document manager/context activation, and
  candidate rectangle callbacks.
- macOS still needs Swift/AppKit `NSView<NSTextInputClient>` lifecycle wiring
  into the native window owner.
- The windowed player still needs focus-to-TextField activation, geometry update
  pumping, and preedit/commit painting in the actual rendered scene.

The native sample is therefore intentionally a runnable adapter-contract sample,
not a user-facing OS IME window.

## Commands

```bash
just ime-sample-web
just ime-sample-native
just ime-sample-check
```

## Validation

Executed from `D:/git/arcweft` on 2026-06-28:

```bash
just ime-sample-check
cargo fmt --all -- --check
cargo check -p arcweft-desktop-native --examples --all-features
cargo clippy -p arcweft-desktop-native --examples --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

`just ime-sample-check` installed and used Playwright Chromium in this local
environment. It reported the Web sample in `ready` state and executed the native
TSF contract example with zero capability diagnostics.

The structural audit scanned 1,686 files, 912 Rust files, and 436,894 Rust
physical LOC with 0 errors and 107 existing warnings.
