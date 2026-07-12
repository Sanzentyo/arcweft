# Arcweft IME Player-Rendered Sample

This sample records the seq06 Web IME boundary after player-owned runtime
text-control convergence.

## Web

Run:

```bash
just ime-sample-web
```

Then open:

```text
http://127.0.0.1:8786/ime-sample.html
```

Equivalent normal player URL:

```text
http://127.0.0.1:8786/index.html?bundle=./ime-player-rendered.awfb
```

Validation:

```bash
just ime-sample-check
```

The Web sample uses the normal Web player path. The active page contains a canvas
host and minimal loading/fatal elements only. It does not contain a visible DOM
dialogue View, mirrored text spans, CSS caret, DOM selection/composition surfaces, or
status/font cards.

The visible controls are Arcweft-rendered from product/runtime text-control data:

- `input.jp_text_field` — `TextField`, Japanese IME target;
- `input.long_latin_area` — `TextArea`, long Latin/Japanese content;
- `input.secret_secure_field` — `SecureField`, masked and redacted.

Fonts and seq06 styling intent are recorded as product/fixture metadata. CSS no
longer renders the field itself; it only hosts the canvas and loading/fatal
surfaces.

## Native

Run:

```bash
just ime-sample-native
```

Equivalent direct command:

```bash
cargo run -p arcweft-cli --features native-player -- run \
  --runner native samples/native-text-input/src/main.arcw \
  --text-input-trace-out target/native-text-input-trace/native-player-ime.real.json
```

This opens a normal Arcweft native player window and renders `TextField`,
`TextArea`, and `SecureField` controls declared in
`samples/native-text-input/src/main.arcw`. Pointer focus, keyboard
traversal, platform IME preedit/commit, routed text-input batches, runtime
write-back, and secure redaction are validated through the same player bridge.

The older adapter-contract sample and backend harnesses remain diagnostics only:

- `just ime-sample-native-contract` runs the desktop-native adapter contract
  diagnostic.
- `windows-tsf-ime-sample` is a Windows TSF diagnostic.
- macOS AppKit helper samples are AppKit diagnostics.
- blank native windows and synthetic contract batches are not final acceptance.
