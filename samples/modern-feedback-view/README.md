# Modern Feedback View

This sample exercises the current View and style authoring path with a flow that
waits for typed semantic actions emitted by player-rendered text controls and
buttons.

It demonstrates:

- a bundled, project-authored background image rendered behind the view
  controls;
- `pub style modern_feedback_panel` with tokens, element selectors, hover,
  active, disabled, focus-visible states, translucent fills, `box-shadow`, and
  style-authored control depth;
- `pub view ModernFeedbackPanel()` with `Panel`, `Column`, `Row`, `Text`,
  `TextField`, `TextArea`, and player-rendered `Button` actions;
- view-owned `TextField` and `TextArea` resources referenced by typed
  `pub action feedback.*` submit routes;
- explicit `let panel = view(@view:.ModernFeedbackPanel)` scope-owned mounting
  from the flow, so the view declaration is reusable, scoped, and does not
  display by declaration alone;
- a flow that waits on `receive action(...)`, branches on submitted text
  length, and returns the submitted brief.

## Check

```bash
cargo run -p arcweft-cli -- check --manifest-path samples/modern-feedback-view/arcw.toml
```

## Bundle

```bash
cargo run -p arcweft-cli -- bundle samples/modern-feedback-view/src/main.arcw \
  --output target/arcweft/modern-feedback-view.awfb
```

## Native

```bash
cargo run -p arcweft-cli --all-features -- run --runner native --manifest-path samples/modern-feedback-view/arcw.toml \
  --text-input-trace-out target/modern-feedback-view/text-input-trace.json
```

The visible buttons are Arcweft player-rendered action buttons. They should not
be replaced by DOM or native platform widgets.

## Assets

`src/.arcweft/asset/bg/glass_lights.png` is a deterministic Arcweft-authored
sample image included for this repository. It is distributed under the same
license terms as the sample code and does not require external attribution.
