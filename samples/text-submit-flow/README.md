# Text Submit Flow Sample

This sample declares its text input directly in Arcweft DSL and waits for a
player-owned submit event from that control.

```bash
cargo run -p arcweft-cli -- run --runner native samples/text-submit-flow/src/main.arcw
```

Focus the field, enter text, then press Enter or the platform IME send/done
action. The flow branches by submitted character count and uses the submitted
string in dialogue and as the flow return value.
