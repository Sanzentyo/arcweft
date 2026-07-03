# text-submit-flow sample with component/View submit Button

This sample demonstrates the seq-06.16.1 authoring surface:

- transitional `ui text_input @input.feedback` declares the text-control resource;
- component/View `TextField(@input:.feedback)` places that control;
- component/View `Button("Send")` lowers to `UiProgramResource.action_buttons`;
- Enter/IME send and button activation all produce the same typed
  `TextControlWriteBack::submit` path.

## Native smoke

```bash
cargo run -p arcweft-cli -- bundle samples/text-submit-flow/src/main.arcw \
  --output target/arcweft/text-submit-flow-button.awfb
cargo run -p arcweft-player-native --bin arcweft-player-native -- \
  target/arcweft/text-submit-flow-button.awfb
```

To capture player text-input write-back traces, run through the native runner:

```bash
cargo run -p arcweft-cli -- run --runner native samples/text-submit-flow/src/main.arcw \
  --text-input-trace-out target/arcweft/traces/text-submit-flow-native.jsonl
```

Trace file:

```text
target/arcweft/traces/text-submit-flow-native.jsonl
```

Expected entries:

- presentation snapshot contains one `action_buttons` record with label `Send`;
- button pointer activation writes `RuntimeTextControlWriteBackKind::Submit`;
- focused-button keyboard activation writes `RuntimeTextControlWriteBackKind::Submit`;
- text-field Enter/IME send reaches the same flow result.

## Web smoke

```bash
cargo run -p arcweft-cli -- bundle samples/text-submit-flow/src/main.arcw \
  --output target/arcweft/text-submit-flow-button.awfb
wasm-pack test --headless --chrome crates/arcweft-player-web
```

Expected trace file:

```text
target/wasm32-unknown-unknown/debug/arcweft-player-web/text-submit-flow-web.jsonl
```

The web sample must not create a DOM `<button>`, hidden form submit, or platform
widget. The button is player-rendered through the shared scene path.
