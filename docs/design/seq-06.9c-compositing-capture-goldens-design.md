# seq06.9c Compositing Capture, Hit Metadata, and Goldens Design

> **Superseded Style-path premise (2026-07-13):** The Arcweft CSS/Takumi authoring, lowering, and evidence path assumed below was removed by the [native-only typed Style path](../implementation/native-only-style-path-2026-07-13.md). The body is retained as historical rationale and is not a current Style contract.

## Goal

seq06.9c closes the evidence layer for the seq06.9a scene contract and seq06.9b
wgpu compositor substrate. The implementation adds deterministic capture
metadata and JSON evidence for CSS compositing features so reviewers can inspect
filter, backdrop-filter, mask, clip-path, and blend behavior without relying on
manual screenshots alone.

The active repository baseline is the main-line cut that includes `Apply
seq06.9b wgpu View compositor effects` (`21476c5843845eed29e0376efb9aa4930c6abf02`).
The existing capture record only stores Arcweft metadata, primitive range,
local bounds, transform, and clip. This design extends that layer rather than
adding renderer, platform, or screenshot concepts to capture data.

## Ownership

The evidence contract lives in `arcweft-takumi-adapter` because it is generated
at the Takumi lowering boundary where Arcweft metadata, layout bounds, direct
primitive ranges, and seq06.9a compositing groups are all visible.

The renderer crate remains responsible for GPU execution and effect passes.
seq06.9c records the evidence needed to review those passes; it does not add new
shader effects and does not CPU-rasterize Takumi output.

## Capture model

Two record categories are emitted:

- object records for direct paint nodes that emit primitives;
- compositing group records for seq06.9a `ViewCompositingGroup` nodes.

Both record kinds expose:

- Arcweft metadata;
- `paint_node_id`;
- `compositing_group_id`;
- primitive range when available;
- layout bounds;
- visual bounds;
- hit bounds;
- clip bounds;
- mask bounds;
- effect outsets.

Object hit bounds remain equal to layout bounds. Group visual bounds are layout
bounds expanded by the maximum filter/backdrop/mask outset. This makes blur and
drop-shadow evidence explicit while avoiding accidental hit-region expansion.

## Evidence JSON

`TakumiCaptureFrame::evidence_json()` emits schema version
`arcweft.compositing-capture.v1`. The matching schema is checked in at
`docs/schemas/compositing-capture-evidence.schema.json`.

The JSON intentionally excludes native/platform identities: window handles,
surface handles, swapchain identifiers, GPU adapter names, filesystem paths, and
screenshot-only identifiers. It is safe for normal review and for non-pinned CI
smoke tests.

## Fixtures

The fixture CSS under
`crates/arcweft-takumi-adapter/tests/fixtures/compositing-capture/scene.css`
contains all five compositing families:

- `filter`;
- `backdrop-filter`;
- `mask-image` and related mask placement fields;
- `clip-path`;
- `mix-blend-mode`.

`expected-evidence.json` is the stable packet reviewers can inspect before exact
PNG promotion. Exact PNG promotion remains manual-only and ignored by default.

## Non-goals

- no renderer shader effects are added here;
- no platform/window/event-loop integration is added;
- no GitHub CI exact PNG enforcement is enabled by default;
- no CPU-rasterized Takumi output is accepted as expected visual evidence.
