# text-submit-flow sample with View semantic submit action

This sample demonstrates the current View/action authoring surface:

- `flow text_submit_flow` explicitly mounts `view(@view:.FeedbackForm)`;
- `let feedback = input.text(@input:.feedback, initial = "")` owns the text
  input handle used by `TextField(feedback)`;
- `pub action feedback.submit(value: String)` defines the semantic submit
  route;
- `TextField(feedback).on_submit { action.invoke(...) }` and
  `Button(...).on_click { action.invoke(...) }` emit the same typed action;
- the flow waits with `let event = receive action(@action:.feedback.submit)`.

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
- button pointer activation emits `action.feedback.submit`;
- text-field Enter/IME send writes `RuntimeTextControlWriteBackKind::Submit`
  and resumes the same `receive action` wait through the submit handler;
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
