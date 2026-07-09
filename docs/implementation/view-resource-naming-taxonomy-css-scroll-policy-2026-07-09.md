# View resource naming taxonomy and CSS scroll policy package application

Date: 2026-07-09

## Source

Applied package:

```text
D:/sanze/Downloads/arcweft-seq06.16.6.4-view-resource-naming-taxonomy-css-scroll-policy-2026-07-07.zip
```

The package records seq 06.16.6.4 as the concrete View-owned naming taxonomy
and CSS Scroll policy cut. It contains review/backport overlays rather than a
blindly applicable patch because the inspected upstream `main` already carried
equivalent implementation slices.

## Current repository shape

The current checkout is newer than the package's conservative taxonomy note:

- retained View program/resource/runtime boundaries use `View*` names;
- catalog-level View resources also use `ViewStyleResource`,
  `ViewThemeResource`, `ViewTextResource`, and `ViewInputResource`;
- no compatibility aliases or serde fallback names were added;
- leaf containment uses `containing_scroll_region`;
- structural `Scroll { ... }` owns interactive overflow;
- non-Scroll interactive overflow emits
  `AWF0617 view::interactive_overflow_requires_scroll`.

This follows the later repository direction to remove legacy UI-prefixed names
from View-owned boundaries instead of keeping product-catalog `Ui*` names.

## Implemented in this cut

- Confirmed the package's Rust implementation targets are already present in
  the current `resource_codec/view`, CLI lowering, overflow diagnostic, runtime,
  player-scene, render-wgpu, and web parity code paths.
- Updated active stable design docs so public View authoring language no longer
  uses `component` for retained View UI handles, capture, or cascade layers.
- Wrote structure audit output under:

```text
docs/implementation/structure-audits/view-resource-naming-taxonomy-css-scroll-policy-2026-07-09/
```

## Non-goals

- No PNG visual baselines were changed.
- No browser DOM/CSSOM fallback was introduced.
- No generalized `clip_chain` was added; nested scroll, clip, mask, and
  transform composition remain a separate contract.
- No attempt was made to redesign older request files that intentionally record
  pre-rename source material.

## Search gates

Executed and reviewed:

```text
rg -n '\bUi(Program|ProgramInstruction|ChildSpan|HandlerRef|SemanticTarget|StyleApplyRef|LayoutBoundsResource|ScrollRegionResource|TextBlockResource|ActionButtonResource|FocusGroupResource|FocusNavigationResource|RuntimeScrollRegion|RuntimeTextBlock|RuntimeActionButton|RuntimeFocusGroup|RuntimeFocusNavigation)\b' crates -g '*.rs'
rg -n 'pub scroll_region\s*:|scroll_region\s*:' crates -g '*.rs' | rg -v 'containing_scroll_region|scroll_regions|ViewScrollRegion|RuntimeScrollRegion'
rg -n '\bcomponent\b|@component|component\(' docs/design -g '*.md'
rg -n '\bcomponent\b|@component|component\(' samples examples -g '*.arcw' -g '*.md'
```

All four gates returned no active hits after the stable design-doc cleanup.

## Validation

Executed:

```text
cargo fmt --all -- --check
cargo test -p arcweft-bundle --all-features --test view_resource_codecs -- --nocapture
cargo test -p arcweft-cli --all-features --lib view_scroll_ -- --nocapture
cargo test -p arcweft-cli --all-features --lib interactive_overflow -- --nocapture
cargo test -p arcweft-runtime-driver --all-features view_handle_lifecycle_filters_text_blocks -- --nocapture
cargo test -p arcweft-player-scene --all-features --test scroll_regions player_frame_offsets_and_clips_scroll_contained_text_blocks -- --nocapture
cargo test -p arcweft-render-wgpu --all-features --test geometry scroll_region -- --nocapture
cargo test -p arcweft-player-web --all-features --test parity scroll -- --nocapture
cargo check -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web -p arcweft-player-native --all-targets --all-features
cargo clippy -p arcweft-bundle -p arcweft-cli -p arcweft-runtime-driver -p arcweft-player-scene -p arcweft-render-wgpu -p arcweft-player-web -p arcweft-player-native --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/view-resource-naming-taxonomy-css-scroll-policy-2026-07-09
```

Results:

- All focused tests and all-features checks passed.
- `cargo clippy` exited successfully with existing warnings in unrelated or
  pre-existing areas.
- Structure audit reported the existing error-level size violations in:
  - `crates/arcweft-cli/src/app/bundle_view.rs`
  - `crates/arcweft-player-scene/src/input.rs`

This package did not expand those files.
