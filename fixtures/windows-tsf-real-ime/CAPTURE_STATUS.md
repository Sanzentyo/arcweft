# Windows TSF real IME fixture capture status

This package includes the trace schema and expected acceptance trace shape.
A real Windows Japanese IME capture was **not** produced in this Linux package
build environment.

The previous standalone capture command is now a diagnostic harness only:

```bash
cargo run -p arcweft-player-native --bin windows-tsf-ime-sample -- \
  --trace-out fixtures/windows-tsf-real-ime/microsoft-japanese-ime-hiragana.real.json
```

This command is backend-diagnostic evidence. Player acceptance runs a DSL-backed
scene through the normal native player, with the platform backend connected
through the cross-platform text-input bridge. The maintained
[text-control contract](../../docs/03-presentation/view-input-text-field.md)
defines that boundary; an old design request is not current acceptance evidence.

The captured file must be checked against `schema.json` and compared to
`expected-microsoft-japanese-ime-hiragana.trace.json` for event coverage.

A package consumer must not replace this status file with a success claim until
that player/DSL-based Windows trace exists and is attached to repository
validation notes.
