# Rich Text Object Visual Evidence

This implementation-state note indexes the visual and non-visual evidence used
by the rich-text typed-presentation-object goal. It separates fixed PNG review
artifacts from metadata-only and temporary raw/JSON regression evidence, so goal
completion does not rely on ambiguous "reviewed somewhere" claims.

Stable language and tooling contracts remain in `docs/01-language/`,
`docs/03-presentation/`, and `docs/04-tooling/`. This file only records the
current verification evidence.

## Evidence Classes

| Evidence class | Meaning | Storage policy |
|---|---|---|
| Fixed PNG review artifact | Human-inspectable capture of a visible rendering issue or fix. | Keep under `docs/implementation/visual-sample-review-issues/` while it is cited by an open or closed review entry. |
| HTML comparison artifact | Browser reference HTML/PNG/metrics used to decide whether Arcweft ruby/text placement is visually plausible. | Keep source HTML plus PNG/JSON metrics with the corresponding review date. |
| Temporary raw/JSON regression evidence | Tests generate raw RGBA or JSON metadata during execution and assert exact metadata, object identity, or pixel differences. | Do not check in generated temp files unless a visual inspection decision depends on them; cite the focused regression instead. |
| Metadata-only evidence | Behavior that is not a new visible rendering outcome, such as object metadata, hit-test payloads, capture refs, or MCP image-resource JSON. | Cite protocol/CLI/MCP tests and contract docs; no PNG is required. |
| Deferred window golden | Window screenshot/golden-image checks that depend on a stable UI harness. | Deferred; current goal uses native offscreen PNG/raw readback instead. |

## Current Coverage

| Goal area | Evidence | Class |
|---|---|---|
| Horizontal ruby distance, long ruby, short ruby, HTML-like baseline behavior | `SVR-2026-06-15-009` through `011`, `SVR-2026-06-16` horizontal ruby comparison artifacts | Fixed PNG review artifact; HTML comparison artifact |
| Vertical ruby side-track distance, vertical body progression, punctuation placement | `SVR-2026-06-15-012` through `015` | Fixed PNG review artifact; HTML comparison artifact |
| Textbox/object crop bounds for ruby, vertical text, wrapping, and sample object review | `SVR-2026-06-15-002` through `008`; `SVR-2026-06-17-005` | Fixed PNG review artifact |
| Horizontal shaping and Latin text readability in rich-text/effect samples | `SVR-2026-06-17-001`, `002`, `004`, `011` | Fixed PNG review artifact |
| Transform/effect visibility in the full grammar sample | `SVR-2026-06-17-003`, `008` through `010`, `012` | Fixed PNG review artifact plus temporary raw RGBA regression evidence |
| Registry-backed shader/effect/motion execution in `rich-text-effects-animation.arcw` | `SVR-2026-06-17-014` through `016`, `029`, `031`, `037`; `SVR-2026-06-18-003`, `004` | Fixed PNG review artifact where visual tint was inspected; temporary raw RGBA regression evidence for per-object color/time assertions |
| Page, line, glyph, cluster, ruby, and proxy object captures | `SVR-2026-06-17-017` through `023`, `032`, `035`; `SVR-2026-06-18-002`, `005`, `006` | Fixed PNG review artifact for proxy color/object-id/mask; temporary raw/JSON regression evidence for child metadata identity |
| Hit-test, layer/depth, custom params, image metadata, and MCP readback | `SVR-2026-06-17-033` through `050`; `SVR-2026-06-18-001`, `006`, `007`, `008` | Metadata-only evidence; temporary JSON/raw regression evidence |
| Animation sampling through `capture_step` / `capture_time` | `SVR-2026-06-17-036`; `SVR-2026-06-18-007`; `agent_observe_native_renderer_captures_combined_typewriter_animation_sample` | Temporary raw/JSON regression evidence; milestone-only animation sample sweep |

## Completion Rules

Use fixed PNG or HTML comparison artifacts when the requirement is about
visible typography, visible decoration, visual crop boundaries, or the
appearance of a sample.

Use raw/JSON regression evidence when the requirement is about deterministic
object identity, metadata transport, capture refs, object-id colors, hit-test
payloads, animation sample timing, or scoped image resources. Checking in every
generated raw crop would add noise without improving reviewability.

Use metadata-only evidence only when the requirement has no distinct visual
appearance. Examples include `image.object.rich_text_ref`, MCP tool schemas,
proxy params on hit-test results, and presentation tree filtering.

Before marking the rich-text object goal complete, cite this file from
`docs/implementation/rich-text-object-goal-audit.md` and run
`just test-rich-text-object-goal` or justify an equivalent gate.
