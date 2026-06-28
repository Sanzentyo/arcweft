# Windows TSF real IME fixture capture status

This package includes the trace schema and expected acceptance trace shape.
A real Windows Japanese IME capture was **not** produced in this Linux package
build environment.

The previous standalone capture command is now a diagnostic harness only:

```bash
cargo run -p arcweft-player-native --bin windows-tsf-ime-sample -- \
  --trace-out fixtures/windows-tsf-real-ime/microsoft-japanese-ime-hiragana.real.json
```

This command must not be treated as final Arcweft player acceptance. The final
capture path is tracked by
`docs/reviews/requests/2026-06-29-seq-06.4j-native-player-platform-text-input-bridge-package.md`
and must run a DSL-backed scene through the normal native player path, with the
platform backend connected through the cross-platform native text-input bridge.

The captured file must be checked against `schema.json` and compared to
`expected-microsoft-japanese-ime-hiragana.trace.json` for event coverage.

A package consumer must not replace this status file with a success claim until
that player/DSL-based Windows trace exists and is attached to repository
validation notes.
