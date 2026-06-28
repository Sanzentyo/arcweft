# Capture instructions

1. Apply the overlay on macOS with Xcode Command Line Tools installed.
2. Run:

```bash
cargo run -p arcweft-player-native \
  --example macos_nstextinputclient_real_ime \
  --features macos-appkit-ime-sample \
  -- --mode text-field 2>&1 | tee fixtures/macos-nstextinputclient-real-ime/captured-japanese-ime-$(date +%F).log
```

3. Switch the active input method to Japanese.
4. Type `nihongo`, confirm conversion to `日本語`, move selection left/right, backspace, then close the window.
5. Repeat for:

```bash
cargo run -p arcweft-player-native --example macos_nstextinputclient_real_ime --features macos-appkit-ime-sample -- --mode text-area
cargo run -p arcweft-player-native --example macos_nstextinputclient_real_ime --features macos-appkit-ime-sample -- --mode secure-field
```

6. Convert the log into JSONL trace records matching `trace-schema.json` and store it as `captured-japanese-ime-YYYY-MM-DD.jsonl`.
7. Record the exact macOS version, keyboard input source, Xcode/Swift version, Cargo version, Arcweft revision, and whether candidate placement appeared at the Arcweft caret rectangle.
