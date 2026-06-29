# Compositing Capture Evidence Schema

`compositing-capture-evidence.schema.json` defines the reviewable JSON emitted
from `TakumiCaptureFrame::evidence_json()` for seq06.9c.

The schema intentionally stores Arcweft object identity, paint-node identity,
compositing-group identity, and bounds evidence. It does not store native window,
view, surface, device, swapchain, filesystem path, or screenshot-only identity.

Bounds fields are split by responsibility:

- `layout_bounds`: the layout box that Takumi produced for the object or group.
- `primitive_bounds`: the direct primitive bounds for object records.
- `visual_bounds`: layout bounds expanded by filter/drop-shadow/mask outsets.
- `hit_bounds`: the interactive bounds; these stay equal to layout bounds unless
  a later input-routing request explicitly changes hit policy.
- `clip_bounds`: the represented clip shape bounds when known.
- `mask_bounds`: one bounds entry per mask layer.
- `effect_outsets`: the filter/backdrop/mask outsets that justify visual bounds.

Exact PNG baselines remain outside this schema. Promotion packets may reference
this JSON, but the JSON is the stable evidence layer that CI can review without
requiring a pinned GPU.
