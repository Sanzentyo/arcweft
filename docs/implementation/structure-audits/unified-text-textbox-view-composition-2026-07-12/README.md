# Unified text: standard TextBox View composition structural audit

Audit target: Jujutsu change `uormqyrm` over parent revision `94821c0b`.
Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-textbox-view-composition-2026-07-12
```

The audit scanned 1,258 Rust files / 623,665 physical Rust LOC and found 0
errors / 133 warning-level findings. No Cargo manifest changed in this slice,
so workspace dependency fan-in and fan-out are unchanged. The generated
`dependency_edges.csv`, `file_metrics.csv`, `public_type_duplicates.csv`, and
`violations.md` are the exact checkout evidence.

## Changed production responsibilities

| Path | Bytes | Physical LOC | Role |
| --- | ---: | ---: | --- |
| `crates/arcweft-player-scene/src/frame/textboxes.rs` | 17,818 | 531 | standard TextBox geometry, canonical speaker/body preparation, typed ownership, and View-scene composition; includes 35 LOC of focused geometry tests |
| `crates/arcweft-player-scene/src/frame.rs` | 17,341 | 496 | player-frame orchestration and the public standard-bounds query |
| `crates/arcweft-player-scene/src/frame/view_text.rs` | 13,964 | 373 | authored View text preparation; now records the mount in typed ownership |
| `crates/arcweft-render-wgpu/src/geometry/dialogue_prepared.rs` | 29,030 | 788 | canonical RichText stage preparation from a closed request object |
| `crates/arcweft-render-wgpu/src/geometry.rs` | 78,452 | 2,367 | shared frame contract, prepared TextBox state/owner types, and generic content-avoidance input |
| `crates/arcweft-render-text/src/resolved_document.rs` | 36,223 | 1,184 | owned style-cascade API used by TextBox sibling content |
| `crates/arcweft-cli/src/app/agent/native/player_observation.rs` | 35,575 | 1,030 | observation orchestration; enumerates every active TextBox target |
| `crates/arcweft-cli/src/app/agent/native/prepared_text_observation.rs` | 38,346 | 1,129 | canonical TextBox geometry projection with exact typed owner lookup |

The new 531-LOC TextBox module is inside the preferred 300–800 LOC range and
keeps layout/composition out of the 496-LOC frame orchestrator. Its production
logic is 499 LOC; its embedded tests are small and test the private tiling rule.
The 144-LOC integration test separately exercises the public product path.

`geometry.rs` remains a warning-level hotspot below the 2,500-LOC error
threshold. The change adds boundary types and delegates the actual RichText and
TextBox algorithms to `dialogue_prepared.rs` and player-scene's new
`textboxes.rs`; it does not add another renderer algorithm to the orchestrator.
The touched 1,749-LOC `geometry/text_controls.rs` production file changes only
an internal test fixture for the new `RenderScene` field, so no new control
responsibility was introduced there.

## Boundary review

- `TextBoxPresentationStore` remains owned by `arcweft-runtime-driver`; the
  player consumes typed IDs and does not duplicate persistence or allocation.
- `ResolvedTextDocument` and style cascade remain owned by
  `arcweft-render-text`; the player uses the new owned cascade method instead
  of matching `RichTextStyle` itself.
- the renderer receives only prepared text IDs, scalar TextBox observation
  state, View primitives, and generic avoidance rectangles. It does not depend
  on the runtime driver.
- native, Web, and Agent paths continue to consume the same player-prepared
  frame. No platform-specific TextBox evaluator, layout, or Fx arithmetic was
  added.
- `content_avoidance_regions` is a generic render-layout boundary. It replaces
  dialogue-name inspection for choice placement and can serve other occluding
  View content without introducing a second TextBox geometry implementation.

No source gates, compatibility aliases, migration readers, wildcard public
re-exports, unsafe code, generated source, or new dependency edges were added.
