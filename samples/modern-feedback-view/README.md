# Modern Feedback View

This sample authors two scoped retained text views and waits for typed Actions
returned by their player-rendered controls. Source checking passes. Bundling
currently encounters the parameterized Flow transfer gap. Execution and capture
acceptance are tracked in the [current status](../../docs/implementation/current-status.md).

It demonstrates:

- an ordinary `glass_bg()` factory returning an owned `Handle<ImageSurface>`
  for the project-authored background image;
- `pub style modern_feedback_panel` with tokens, element selectors, hover,
  active, disabled, focus-visible states, translucent fills, `box-shadow`, and
  style-authored control depth;
- one named-sheet application on each panel, with heading and caption sheets
  applied to the original Text nodes;
- `ModernFeedbackNamePanel()` and `ModernFeedbackBriefPanel()` with `Panel`,
  `Column`, `Row`, `Text`, `TextField`, `TextArea`, and player-rendered `Button` actions;
- retained local `String` state for `TextField` and `TextArea`, with original
  targets, labels, placeholders, IME hints, and `feedback.*` Action payloads;
- explicit `mount(ModernFeedbackNamePanel())` and `mount(ModernFeedbackBriefPanel())`
  scope-owned mounting from the flow, so the view declaration is reusable and does not
  display by declaration alone;
- a flow that uses `await receive(feedback.submit_name)` and
  `await receive(feedback.submit_brief)`, preserves both empty/nonempty branches,
  and transfers back to its beginning after the final dialogue.

## Check

```bash
cargo run -p arcweft-cli -- check --manifest-path samples/modern-feedback-view/arcw.toml
```

## Bundle

```bash
cargo run -p arcweft-cli -- bundle \
  --manifest-path samples/modern-feedback-view/arcw.toml --profile main \
  --output target/arcweft/modern-feedback-view.awfb
```

## Native

```bash
cargo run -p arcweft-cli --all-features -- run --runner native --manifest-path samples/modern-feedback-view/arcw.toml \
  --text-input-trace-out target/modern-feedback-view/text-input-trace.json
```

The visible buttons are Arcweft player-rendered action buttons. They should not
be replaced by DOM or native platform widgets.

The profile manifest owns the 1280 x 720 viewport. The flow does not repeat
that profile setting as an untyped runtime effect.

## Assets

`assets/bg/glass_lights.png` is a deterministic Arcweft-authored
sample image included for this repository. It is distributed under the same
license terms as the sample code and does not require external attribution.
