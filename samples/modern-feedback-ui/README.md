# Modern Feedback UI

This sample exercises the current component/View and style authoring path with a
flow that waits for player-rendered text-control submissions.

It demonstrates:

- a bundled, project-authored background image rendered behind the component
  controls;
- `pub style modern_feedback_panel` with tokens, element selectors, hover,
  active, disabled, focus-visible states, translucent fills, `box-shadow`, and
  style-authored control depth;
- `pub component ModernFeedbackPanel() -> View` with `Surface`, `VStack`,
  `HStack`, `Text`, `TextField`, `TextArea`, and player-rendered `Button`
  actions;
- component-owned `TextField` and `TextArea` resources referenced by the same
  submit targets used by the buttons and flow;
- explicit `component(@component:.ModernFeedbackPanel)` mounting from the flow,
  so the component declaration is reusable and does not display by declaration
  alone;
- a flow that waits on `text_submit`, branches on submitted text length, and
  returns the submitted brief.

## Check

```bash
cargo run -p arcweft-cli -- check --manifest-path samples/modern-feedback-ui/arcw.toml
```

## Bundle

```bash
cargo run -p arcweft-cli -- bundle samples/modern-feedback-ui/src/main.arcw \
  --output target/arcweft/modern-feedback-ui.awfb
```

## Native

```bash
cargo run -p arcweft-cli --all-features -- run --runner native --manifest-path samples/modern-feedback-ui/arcw.toml \
  --text-input-trace-out target/modern-feedback-ui/text-input-trace.json
```

The visible buttons are Arcweft player-rendered action buttons. They should not
be replaced by DOM or native platform widgets.

## Assets

`src/.arcweft/asset/bg/glass_lights.png` is a deterministic Arcweft-authored
sample image included for this repository. It is distributed under the same
license terms as the sample code and does not require external attribution.
