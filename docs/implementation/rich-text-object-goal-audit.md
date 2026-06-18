# Rich Text Object Goal Audit

This implementation-state note tracks the current evidence for treating
dialogue rich text as typed presentation objects. It is not a language
specification; stable surface rules live under `docs/01-language/` and
debug-tooling contracts live under `docs/04-tooling/`.

## Objective

Rich text and dialogue text should be debuggable like image or 3D model
objects: every meaningful text unit must have typed object identity, metadata,
hit-test behavior, depth/layer routing, object-id/mask/color capture, animation
sampling, custom proxy metadata, and registry-backed effect/shader/motion
execution without compatibility shims.

## Evidence Matrix

| Requirement | Current evidence | Status |
|---|---|---|
| `character.say` / `narrator.say` remain high-level facades over typed text objects | `docs/implementation/native-rich-text-player.md`; `samples/rich-text-full-grammar.arcw`; CLI regressions around `agent_observe_json_reports_rich_text_display_objects` and full grammar native observe | Covered |
| Page, line, run, glyph, glyph-cluster, ruby, and proxy objects are observable with stable ids | `AgentRichTextElementKind`; `agent_observe_json_reports_rich_text_display_objects`; vertical glyph/ruby raw crop regressions; `SVR-2026-06-17-018` through `023` and `SVR-2026-06-18-005` | Covered |
| Object-scoped images preserve typed object metadata | `AgentImageObjectRef`; `image_resource_metadata_preserves_observed_object_ref`; `agent_observe_reports_text_presentation_z_index_depth`; `image_tool_content_preserves_object_rich_text_ref_metadata`; `SVR-2026-06-17-035` through `050` and `SVR-2026-06-18-005` / `006` | Covered |
| `#[text_proxy]` / `#[rich_text_proxy]` metadata survives lowering, observe, hit-test, capture, and MCP readback | `agent_observe_infers_text_proxy_struct_shorthand`; `agent_observe_infers_rich_text_proxy_struct_attribute_family`; full grammar proxy regressions; `SVR-2026-06-17-021`, `025`, `034`, `038`, and `SVR-2026-06-18-001` / `002` / `006` | Covered |
| Layer/depth/custom params work for ordinary text as well as proxies | `presentation_scalar` runtime-plan tests; `agent_observe_reports_text_presentation_z_index_depth`; `SVR-2026-06-17-039` through `044` | Covered |
| Hit-test results carry enough object metadata for follow-up captures | `hit_test_hit_serializes_capture_refs`; `agent_hit_test_reports_depth_sorted_rich_text_proxy`; `agent_hit_test_capture_time_follows_animated_text_proxy_bounds`; `SVR-2026-06-17-045`, `049`, `050` | Covered |
| Color, object-id, and mask captures work for viewport/layer/object and rich-text children | `docs/04-tooling/agent-observe-capture-contract.md`; raw crop regressions for ruby/glyph/cluster/proxy/page/line; `SVR-2026-06-17-017` through `023`; `SVR-2026-06-18-002` | Covered |
| `capture_step` / `capture_time` sample animated text objects, not only typewriter alpha | `agent_hit_test_capture_time_follows_animated_text_proxy_bounds`; typewriter/text-combine/ruby capture-time tests; effects animation combo regression; `SVR-2026-06-17-036`; `SVR-2026-06-18-007` | Covered |
| Registry-backed effects, shaders, and motion functions render through native/debug capture | `RichTextEffectRegistry`, `RichTextShaderRegistry`, `RichTextMotionRegistry`; `rich-text-effects-animation.arcw`; native and CLI regressions for glyph color, post-process, source-local pure helpers, and function motion; `SVR-2026-06-17-014` through `016`, `029` through `031`, `037`, and `SVR-2026-06-18-003` / `004` | Covered |
| Missing/unsupported effect, shader, and motion paths are structured diagnostics in report and image resources | `agent_observe_native_rich_text_reports_structured_visual_diagnostics`; `agent_observe_native_rich_text_reports_missing_motion_diagnostics_in_image_resources`; `image_resource_metadata_preserves_capture_diagnostics`; `docs/04-tooling/agent-observe-capture-contract.md`; `SVR-2026-06-18-008` | Covered |
| Visual sample evidence exists for inspectable rendering outcomes, not only metadata | `docs/implementation/rich-text-object-visual-evidence.md`; 2026-06-15 ruby/HTML comparison images; 2026-06-16 horizontal ruby comparison; 2026-06-17 full grammar/effect captures; 2026-06-18 proxy/source-local post-process captures | Covered |
| A single explicit milestone command exists for the current rich-text object goal gate | `just test-rich-text-object-goal` runs the protocol/MCP/native/player/CLI/sample checks listed below | Covered |

## Remaining Audit Risks

- The matrix is assembled from focused tests and review evidence. Use targeted
  regressions during development, then run `just test-rich-text-object-goal`
  before claiming completion or handing off the milestone.
- `docs/implementation/rich-text-object-visual-evidence.md` classifies fixed
  PNG/HTML review artifacts separately from temporary raw/JSON and
  metadata-only regressions. Keep that index current when new visual sample
  requirements or evidence classes are added.
- Golden-image screenshot checks remain deferred until a stable local UI
  harness exists; the current goal is covered by native offscreen PNG/raw
  readback rather than a window screenshot harness.

## Current Push Gate

Before marking this goal complete, run or justify the equivalent of:

```bash
just test-rich-text-object-goal
```

The recipe currently expands to:

```bash
cargo test -p arcweft-agent-protocol -- --nocapture
cargo test -p arcweft-agent-mcp -- --nocapture
cargo test -p arcweft-player-native motion -- --nocapture
cargo test -p arcweft-player-native shader -- --nocapture
cargo test -p arcweft-player-native post_process -- --nocapture
cargo test -p arcweft-player-native typewriter -- --nocapture
cargo test -p arcweft-cli --test check agent_observe_reports_text_presentation_z_index_depth -- --exact --nocapture
cargo test -p arcweft-cli --test check agent_hit_test_capture_time_follows_animated_text_proxy_bounds -- --exact --nocapture
cargo test -p arcweft-cli --test check agent_observe_native_renderer_captures_combined_typewriter_animation_sample -- --ignored --exact --nocapture
cargo test -p arcweft-cli --test check agent_observe_native_rich_text_reports_missing_motion_diagnostics_in_image_resources -- --exact --nocapture
cargo run -p arcweft-cli --quiet -- check samples/rich-text-full-grammar.arcw
cargo run -p arcweft-cli --quiet -- check samples/rich-text-effects-animation.arcw
```

Use `docs/implementation/test-execution-policy.md` for broader workspace gates
at push cut points.

Last local run on 2026-06-19: `just test-rich-text-object-goal` passed in
250.9s wall time. The slowest selected check was
`agent_observe_native_renderer_captures_combined_typewriter_animation_sample`
at 182.92s test-body time; keep it as milestone evidence, not a tight-loop
command. That exact test is ignored in the default `check.rs` suite and is
selected explicitly by this gate.
