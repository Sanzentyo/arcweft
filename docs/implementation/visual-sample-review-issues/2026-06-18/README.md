# Visual Sample Review - 2026-06-18

This directory records review evidence for treating rich-text proxy spans as
first-class presentation objects. The source sample is
`samples/rich-text-full-grammar.arcw`.

| ID | Review | Evidence | Reproduction |
|---|---|---|---|
| `SVR-2026-06-18-001` | A `#[text_proxy]`-backed `KeywordHit` span can be found from the standalone presentation-tree resource using `proxy_param.channel=choice`, without reading the whole observation object table. The filtered tree preserves ancestors and resolves the concrete proxy object as `object.dialogue.0.7.proxy.11.0`; the proxy index keeps `id=hotspot`, `type_name=KeywordHit`, `role=keyword`, declaration provenance, `depth=4000`, and typed `params.channel=choice`. | [`proxy-param-choice-presentation-tree.json`](proxy-param-choice-presentation-tree.json) | `cargo run -p arcweft-cli --quiet -- agent observe samples\rich-text-full-grammar.arcw --read-uri "arcweft://session/cli/frame/0/presentation-tree.json?proxy_param.channel=choice" --mode drain --steps 16 --max-ops 256` |
| `SVR-2026-06-18-002` | The resolved proxy object can be captured like an image/model object. Color, object-id, and mask captures all target `object.dialogue.0.7.proxy.11.0`, report `content_pixels=521`, and preserve the same proxy metadata in compact capture metadata. | Color: [`proxy-hotspot-color.png`](proxy-hotspot-color.png); object-id: [`proxy-hotspot-object-id.png`](proxy-hotspot-object-id.png); mask: [`proxy-hotspot-mask.png`](proxy-hotspot-mask.png); metadata: [`proxy-hotspot-capture-metadata.json`](proxy-hotspot-capture-metadata.json) | `cargo run -p arcweft-cli --quiet -- agent observe samples\rich-text-full-grammar.arcw --json --image png --out docs\implementation\visual-sample-review-issues\2026-06-18\proxy-hotspot-color.png --mode drain --steps 16 --max-ops 256 --object object.dialogue.0.7.proxy.11.0`; repeat with `--capture object-id` and `--capture mask` for the object-id and mask artifacts. |

The reviewed object is intentionally small: the color crop reads as the visible
word `proxy`, the object-id crop uses the proxy object's stable id color, and
the mask crop covers the same glyph region. This proves the same typed proxy
identity is usable for semantic discovery, partial image capture, object-id
capture, and mask capture.
