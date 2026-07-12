# Visual smoke / golden fixture notes

This directory contains small, reviewable metadata fixtures for seq06.6. It does
not contain generated PNGs or raw RGBA blobs because the useful image artifacts
are produced under `target/` by `just test-visual-smoke`,
`just native-visual-artifacts`, and `just fixture-refresh-all` on a real Arcweft
checkout.

The JSON fixtures mirror the seq06.5 `selected_capture` shape consumed by the
new visual smoke tests:

- `selected-layer-smoke-metadata.json`: selected dialogue layer PNG smoke.
- `selected-object-smoke-metadata.json`: selected dialogue View object PNG smoke.
- `exact-native-golden-policy.json`: checked-in native golden policy and imq
  tolerance contract.

No exact selected object/layer PNG is checked in by this package. Exact pixels
remain Tier 2 and environment-gated; selected object/layer coverage is metadata
+ non-empty image-content smoke.
