# Seq 06.16.6.2 scroll axis, policies, and retained content

Date: 2026-07-09

## Source

Applied package:

```text
D:/sanze/Downloads/arcweft-seq-06.16.6.2-scroll-axis-virtualization-retained-content.zip
```

The package was authored against an older resource-codec path. This application
ports the design to the current `view` terminology and current crate layout
without reintroducing `ui` or `component` compatibility names.

## Implemented behavior

- `ViewScrollRegionResource` and `ViewRuntimeScrollRegion` now carry defaulted
  policies for `indicators`, `overscroll`, and `auto_scroll_focus`.
- Scroll policy enums use typed resource values and author-symbol parsers.
  Defaults are `indicators = .auto`, `overscroll = .clamp`, and
  `auto_scroll_focus = .nearest`.
- `Scroll` lowering accepts policy named arguments and style/property spellings:
  `indicators`, `scroll-indicators`, `overscroll`, `overscroll-behavior`,
  `auto_scroll_focus`, `auto-scroll-focus`, and `auto-focus-scroll`.
- `ViewScrollAxis` remains `Vertical | Horizontal`. Authoring `.both`, `.xy`,
  `.yx`, `.all`, `.2d`, or `.both-axes` is rejected with
  `AWF0618 view::scroll_axis_both_unsupported`.
- `LazyRow` and `LazyColumn` remain rejected at the source boundary until the
  runtime can publish one deterministic materialized window and stable range
  table. The compact bundle enum still contains provisional variants while
  that implementation is replaced; they are not described as an eager source
  feature.
- Player-scene input now exposes precision x/y scrolling and explicit
  `scroll_region_by_id` routing over the existing compact x/y scroll-offset
  snapshot.
- Keyboard `PageUp`, `PageDown`, `Home`, and `End` first route to a focused or
  pointer-contained scroll region before falling back to focus-list movement.
- Focus auto-scroll policy is propagated from runtime scroll resources into the
  render scene. `nearest`, `start`, `end`, and `disabled` are applied by the
  input controller for targets represented in the prepared frame.
- `PreparedFrame` exposes target-bound and containing-scroll-region helpers so
  native, web, and Agent-facing input routing use the same prepared geometry.
- Scroll policies now remain typed through player-scene and render geometry.
  `PreparedFrame::scroll_indicators` publishes the same track/thumb geometry
  drawn by the wgpu path, including region id, axis, and opacity.
- `.visible` indicators remain visible, `.hidden` indicators are omitted, and
  `.auto` indicators remain fully visible for 700 ms after scroll/focus
  activity before a 300 ms fade. Reduced-motion mode omits the fade.
- Player input owns one scroll state per region. Its clamped content offset is
  the only persisted component; indicator activity, elastic displacement, and
  spring velocity are transient presentation state.
- Nested scroll chaining is deterministic: `.clamp` forwards unconsumed delta
  to a containing parent, `.contain` consumes it at the current boundary, and
  `.elastic` converts it to a resisted visual displacement and consumes it.
  Elastic displacement settles with an analytic critically damped spring.
  Reduced-motion mode suppresses elastic displacement.
- Right-stick gamepad axes use dead-zone-adjusted, time-integrated precision
  deltas and enter the same focused/pointer-contained scroll chain as wheel and
  touchpad input. Samples are capped after a stalled poll so reconnecting a
  held stick cannot jump an unbounded distance; no controller-specific offset
  store exists.
- Agent protocol now owns a flat region-addressed `scroll` action with signed
  i32 milli-logical-pixel x/y deltas. MCP direct and observed-action dispatch
  validate the payload and current action availability before using the live
  prepared-frame `InputController::scroll_region_by_id` route.
- Native observation publishes one typed record per authored Scroll with
  internal viewport/content parts, current/max offsets, and effective policies.
  Internal parts do not create duplicate objects or action targets.

## Deferred behavior

- Full range virtualization for `LazyRow` and `LazyColumn`.
- Non-materialized child range records for Agent observe/capture and save/load.

These remain within the existing request:

```text
docs/reviews/requests/2026-07-07-seq-06.16.6.2-scroll-axis-virtualization-retained-content.md
```

## Validation

```bash
cargo fmt --all
cargo check -p arcweft-bundle -p arcweft-cli -p arcweft-render-wgpu -p arcweft-player-scene --all-targets --all-features
cargo test -p arcweft-bundle --all-features --test view_resource_codecs scroll_region_policy -- --nocapture
cargo test -p arcweft-cli --all-features --lib view_scroll_ -- --nocapture
cargo test -p arcweft-player-scene --all-features --test scroll_regions scroll_region -- --nocapture
cargo test -p arcweft-player-scene --all-features scroll_ -- --nocapture
cargo test -p arcweft-render-wgpu --all-features --test geometry scroll_region -- --nocapture
cargo test -p arcweft-player-web --all-features --test input scroll -- --nocapture
cargo test -p arcweft-player-web --all-features --test parity scroll -- --nocapture
cargo check -p arcweft-bundle -p arcweft-cli -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-player-web --all-targets --all-features
git diff --check
cargo clippy -p arcweft-view -p arcweft-bundle -p arcweft-agent-protocol -p arcweft-agent-mcp -p arcweft-agent-repl -p arcweft-agent-runner -p arcweft-cli -p arcweft-player-scene -p arcweft-player-native -p arcweft-render-wgpu -p arcweft-player-web --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/seq-06.16.6.2-scroll-axis-virtualization-retained-content-2026-07-09
```

The indicator/overscroll completion additionally passed focused render geometry,
elastic visual-offset, nested chaining, player-scene integration, and Web input
tests. The final all-target/all-feature check and strict clippy route also pass
for the complete View, protocol, Agent, CLI, player, and renderer consumer set.

The Agent scroll contract cut additionally passed the complete
`arcweft-agent-protocol` test suite, all-target/all-feature checks for
`arcweft-agent-mcp`, `arcweft-agent-runner`, and `arcweft-cli`, plus formatting
and diff whitespace validation. The focused CLI scroll observation test passed
after the final protocol and prepared-frame integration.

The structure audit writes:

```text
docs/implementation/structure-audits/seq-06.16.6.2-scroll-axis-virtualization-retained-content-2026-07-09
```

The original audit reported error-level size violations in `bundle_view.rs`
and `input.rs`. Subsequent responsibility cuts split player input and View
bundle lowering into ordinary modules; the final sequence audit records the
post-split measurements rather than retaining those obsolete exceptions. The
final audit scanned 1,164 Rust files / 591,616 physical LOC and reports zero
error-level findings.

## Design deviations

No `.both` axis was added. This follows the package decision to keep the
existing single-primary-axis runtime/render/save contract and reject dual-axis
authoring until a deterministic two-axis contract exists.

The package's proposed eager `LazyRow` / `LazyColumn` phase was not a valid
implemented source feature: the parser continued to reject both spellings and
the focused validation filter did not select the differently named Lazy test.
The final implementation does not preserve that accidental half-state. Source
acceptance will be enabled only together with live range virtualization,
non-materialized observation, and range-aware restore.
