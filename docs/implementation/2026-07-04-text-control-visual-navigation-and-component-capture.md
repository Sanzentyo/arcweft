# Text-control visual navigation and component capture audit - 2026-07-04

## Scope

This note records the implementation slice for optional soft-wrap-aware
text-control Up/Down navigation and the audit result for component-scoped
render/capture.

## Implemented

- `TextInputOptions` now has a `TextVerticalNavigationPolicy`.
- The default remains `logical_line`, preserving the previous newline-delimited
  Up/Down behavior.
- `visual_line` is opt-in. When renderer-backed glyph geometry is available for
  the focused control, Up/Down moves across soft-wrapped visual rows and keeps a
  preferred text-local x column across repeated vertical moves.
- If geometry is unavailable, invalid, non-renderer-backed, or non-horizontal
  writing mode, the editor falls back to the existing logical-line behavior.
- Runtime View resource options and component-view lowering can carry the policy
  through `vertical_navigation`, `vertical_navigation_policy`,
  `verticalNavigation`, or `verticalNavigationPolicy`.

## Component render/capture audit

Current Agent/native capture contracts support viewport, layer, object, and
rich-text-child scoped captures. Documentation and CLI shape expose
`--layer` and `--object`; observed object results carry `capture_refs`.

No component-scoped render/capture contract was found in the current runtime:

- component identity is not preserved as a first-class render scope in the
  prepared frame capture selector;
- component bounds are not exposed as a stable capture resource independent of
  individual semantic objects or runtime controls;
- the CLI/MCP capture selector surface has no `component` selector;
- rendering only one component would require a contract for dependencies such as
  inherited style, text/image resources, focus state, runtime text-control
  writebacks, and child object ordering.

This slice therefore does not claim component-only rendering or
component-region capture support. The follow-up design request is
`docs/reviews/requests/2026-07-04-seq-06.16.5-component-scoped-render-capture.md`.

## Non-goals

- No checked-in PNG baseline promotion.
- No web exact capture work.
- No vertical-writing visual-column navigation contract; vertical modes continue
  to use logical-line fallback until they receive their own direction semantics.

## Validation

Completed during implementation:

```bash
cargo test -p arcweft-presentation --test text_editor_behavior
cargo test -p arcweft-bundle --test view_runtime_text_controls
cargo test -p arcweft-bundle --test view_resource_codecs
cargo test -p arcweft-player-scene --test runtime_text_controls
cargo test -p arcweft-player-scene --test runtime_control_style_lowering
cargo check -p arcweft-cli --all-targets
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The structure audit reported 0 errors and 129 warnings, with no report files
written.
