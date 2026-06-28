# macOS NSTextInputClient real IME fixtures

This directory contains the trace schema and expected smoke trace for the seq06.4c.2 macOS AppKit bridge.

`expected-japanese-ime-smoke.jsonl` is a contract trace showing the expected shape of a Japanese IME run. It is not a real machine-captured trace. After applying the overlay on macOS, capture a real run and add it as:

```text
fixtures/macos-nstextinputclient-real-ime/captured-japanese-ime-YYYY-MM-DD.jsonl
```

The real trace must include:

- helper `ready` event with screen/view geometry;
- `focus`;
- at least one `set_marked_text` preedit callback;
- candidate `first_rect` response derived from Arcweft geometry;
- one `insert_text` commit;
- deletion and selection command callbacks;
- `blur` / deactivation;
- secure-field run with no text, ranges, character bounds, or diagnostics exposed.
