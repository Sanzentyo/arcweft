# Windows TSF real IME fixture capture status

This package includes the trace schema and expected acceptance trace shape.
A real Windows Japanese IME capture was **not** produced in this Linux package
build environment. The real capture must be generated on Windows with Microsoft
Japanese IME enabled by running:

```bash
cargo run -p arcweft-player-native --bin windows-tsf-ime-sample -- \
  --trace-out fixtures/windows-tsf-real-ime/microsoft-japanese-ime-hiragana.real.json
```

The captured file must be checked against `schema.json` and compared to
`expected-microsoft-japanese-ime-hiragana.trace.json` for event coverage.

A package consumer must not replace this status file with a success claim until
that Windows trace exists and is attached to repository validation notes.
