# Retained text submission

`FeedbackForm` owns one `local state mut feedback_text: String`. The text field
binds that value to `@input:.feedback`, with a distinct label, placeholder and
typed IME purpose / Enter-key hint. The flow mounts the view with
`mount(FeedbackForm())` and awaits `receive(feedback.submit)`.

Both `.on_submit(|| feedback.submit(value = feedback_text))` and the Send
button's `.on_click` return the same declared Action and its current String
payload. The flow returns the submitted String. Its empty, short and long
branches use `scalar_count()`, which counts Unicode scalar values, including
combining marks and individual scalars in an emoji sequence.

```bash
cargo run -p arcweft-cli -- check \
  --manifest-path samples/text-submit-flow/arcw.toml --json
cargo run -p arcweft-cli -- bundle \
  --manifest-path samples/text-submit-flow/arcw.toml --profile main \
  --output target/arcweft/text-submit-flow.awfb --json
```

Interactive verification covers Enter / IME send and the Send button, the
unchanged input and Action IDs, and all response branches. Native and Web use
player-rendered controls through the shared scene path. Validation results and
remaining acceptance belong to the single
[current status](../../docs/implementation/current-status.md).
