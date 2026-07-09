# Seq 06.16.6.2 scroll axis, policies, retained content, and eager lazy views

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
- `LazyRow` and `LazyColumn` are typed `ViewElementKind` values and lower
  eagerly through the existing row/column layout paths.
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

## Deferred behavior

- Full range virtualization for `LazyRow` and `LazyColumn`.
- Non-materialized child range records for Agent observe/capture and save/load.
- User-visible visual scroll indicators.
- Elastic overscroll rendering or physics; `.elastic` is stored as policy, but
  offsets remain clamped.
- Raw gamepad analog scroll axes. Future gamepad scroll input should call the
  same explicit region scroll route rather than creating a second offset store.

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
cargo clippy -p arcweft-bundle -p arcweft-cli -p arcweft-render-wgpu -p arcweft-player-scene -p arcweft-player-web --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/seq-06.16.6.2-scroll-axis-virtualization-retained-content-2026-07-09
```

`cargo clippy` exits successfully with pre-existing warnings outside this slice
still present in the workspace. The structure audit writes:

```text
docs/implementation/structure-audits/seq-06.16.6.2-scroll-axis-virtualization-retained-content-2026-07-09
```

The audit reports two current error-level file-size violations:

- `crates/arcweft-cli/src/app/bundle_view.rs` at 2590 physical LOC.
- `crates/arcweft-player-scene/src/input.rs` at 2681 physical LOC.

Those are real structural debt and should be split by responsibility in a
separate refactor cut. This package keeps the implementation in the existing
ownership files to avoid combining a broad decomposition with the scroll
contract change.

## Design deviations

No `.both` axis was added. This follows the package decision to keep the
existing single-primary-axis runtime/render/save contract and reject dual-axis
authoring until a deterministic two-axis contract exists.

`LazyRow` and `LazyColumn` are accepted but not virtualized. They are explicit
typed view elements with eager deterministic lowering in this cut.
