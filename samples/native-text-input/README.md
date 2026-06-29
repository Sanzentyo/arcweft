# Native Text Input Sample

This is the canonical native IME acceptance sample for seq06.4j.1.

Run:

```bash
cargo run -p arcweft-cli --features native-player -- run \
  --runner native samples/native-text-input/src/main.arcw \
  --text-input-trace-out target/native-text-input-trace/native-player-ime.real.json
```

The window should show Arcweft-rendered controls from the product/runtime UI
resource sidecars in `.arcweft/content/`, including `ui.style.json` font/style
cases for Japanese sans, Japanese serif, focus ring, and secure masking:

- `jp_text_field` — single-line `TextField`;
- `jp_text_area` — multiline `TextArea`;
- `secret_secure_field` — secure `SecureField` with redacted trace data.

Use pointer focus or keyboard traversal. Local machine traces belong in
`target/native-text-input-trace/`. Do not check local real-machine traces into
fixtures unless they are reviewed and promoted.

Diagnostic binaries such as `windows-tsf-ime-sample` are useful for backend
plumbing only. They are not final acceptance for this sample.
