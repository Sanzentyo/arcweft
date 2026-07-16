# Seq-06.11d.4.1.2 native physical box geometry implementation

## Outcome

The implementation-ready, internally consistent portion of
`arcweft-seq-06.11d.4.1.2-native-physical-box-geometry-final-contract.zip` is
implemented as a checked, Sans-I/O physical geometry substrate in
`arcweft-view`, together with canonical Style projections and direct contract
tests. Runtime integration that depends on unresolved occurrence identity,
container packet ownership, and contradictory acceptance rows is deliberately
excluded and has a separate independently throwable reconciliation request.

Package evidence:

- archive SHA-256:
  `fbbc194def3c06b94290301a58856576e15d6ea81c6311a47909209934128d01`;
- package manifest verification: 13 of 13 listed members matched;
- implementation basis Git commit:
  `76d39983ad8770a87d6e81745785b6b362a381b4`;
- implementation Jujutsu change: `uqyzvpuvwtxstwwxskksonrptzukwusp`.

The package's standalone reference crate was also checked as evidence. Its 33
tests passed offline. Its format check failed, and clippy found two unused
bindings plus one collapsible conditional; therefore the reference code was
not copied as an assumed production-quality overlay.

## Implemented substrate

- `arcweft-view::geometry` is a small facade over responsibility modules for
  checked primitives, box measurement/placement, consumer calculations,
  structured errors, and domain-separated revision keys.
- Geometry node identity preserves the existing mount, full repeat/call path,
  and instruction identity. The package's mount-plus-instruction identity was
  not adopted because it aliases distinct runtime occurrences.
- Integer milli-pixel points, sizes, spans, rectangles, translation, uniform
  scale, transform chains, insets/outsets, intersection/union, outward raster
  rounding, and finite pointer ingress use checked arithmetic.
- Border-box measurement validates non-negative sizes/edges/gaps, min/max
  conflicts, padding-plus-border fit, signed margin inversion, positioning,
  auto stretch, forward/reverse flows, and non-collapsing margin/gap advances.
- Consumers share typed visible geometry, signed scroll ranges, nearest reveal,
  per-axis overflow/clip behavior, focus/avoidance/hit/capture bounds, and
  deterministic errors.
- Exact measure/place/final keys preserve path-aware node identity, ordered
  child and sibling dependencies, viewport/scroll/style revisions, and distinct
  hash domains.
- `ComputedViewStyle::physical_box` now includes display, position, border,
  zero-default physical edges, and canonical scale. Its container projection
  includes physical flow and canonical row/column gaps.
- `Gap` expands to the `RowGap` and `ColumnGap` canonical slots before cascade
  resolution. The property owner exposes exhaustive supported,
  represented-only, and not-geometry metadata.
- Player-scene container spacing now consumes only those canonical slots:
  columns read `RowGap`, rows read `ColumnGap`, and no runtime `Gap` fallback
  remains.
- A dedicated `PHYSICAL_GEOMETRY` invalidation bit reaches layout, transform,
  clip, paint-outset, hit, focus, avoidance, and scroll domains as appropriate.
- Player-scene and render-wgpu now read scale from the canonical physical box
  packet rather than duplicating a composite-property read.

No new dependency, compatibility layer, source gate, CSS/Takumi path, unsafe
code, or serialized provisional geometry format was added.

### Canonical player gap follow-up

A narrow follow-up at parent revision
`1aa5ad6d395ea2b8a643567c1b98e3ed765485be` removed the player-scene
compatibility read of noncanonical `Gap`. The existing column fixture now
provides the owner-projected `RowGap` value directly. This does not enter the
unsettled retained-tree, cache, packet, or consumer reconciliation scope below;
it only makes the existing spacing adapter obey the already implemented Style
owner boundary.

The changed Rust files remain responsibility-sized at the current checkout:

- `frame/view_style/consumer.rs`: production consumer policy, 20,633 bytes and
  541 physical LOC;
- `frame/view_style/layout.rs`: production layout offset adaptation, 11,031
  bytes and 330 physical LOC;
- `frame/view_style/tests.rs`: unit tests, 40,570 bytes and 1,159 physical LOC.

No crate dependency, public contract, feature, or serialization boundary
changed in this follow-up.

## Package deviations and excluded production work

The following are not implementation discretion; they prevent safe completion
of the package's full production claim:

1. `ViewGeometryNodeId { mount, instruction }` in the package loses the current
   repeat/call path and can alias two live occurrences. The safe substrate uses
   a lossless path-aware identity, but its exact runtime authority and public
   exposure still require reconciliation.
2. TEST_MATRIX BX-016 permits explicit zero size with nonzero edges, while
   DESIGN section 7, D-015, BX-006, and BX-007 require
   `EdgesExceedUsedBorderBox`. The implemented kernel follows the general
   checked-fit rule; the final contract must correct the matrix.
3. The phrase “parent first” conflicts with the package's inner-to-outer
   transform algorithm. The substrate and tests execute inner to outer.
4. The package does not freeze one production clip packet for empty versus
   per-axis unbounded state, or one exact element-kind/container projection
   boundary through bundle/runtime.
5. It does not specify how a path-aware geometry occurrence maps to runtime
   resource bindings, retained tree traversal, cache transaction, prepared-
   frame rollback, or every current consumer.

Consequently, authoritative player-scene bounds mutation, retained tree
measurement/placement, runtime caches, bundle container projection, hit/focus/
avoidance/scroll/capture publication, and removal of all legacy saturating or
clamping consumer arithmetic remain outside this cut. They are the acceptance
scope of:

- [d.4.1.2.1 production reconciliation](../reviews/requests/2026-07-16-seq-06.11d.4.1.2.1-native-physical-box-geometry-production-reconciliation.md)

## Direct test evidence

`crates/arcweft-view/tests/geometry_contract.rs` contains 22 direct tests
covering the package's safe BX, NEG, FLOW, POS, XFM, CLIP, CON, SCR, CAP, NUM,
AXIS, and CACHE families. Existing logical-axis and style-metadata tests also
cover physical projection, Gap expansion/override, invalidation, and
represented-only property classification.

Commands completed successfully:

```bash
CARGO_INCREMENTAL=0 cargo check -p arcweft-view --all-targets
CARGO_INCREMENTAL=0 cargo test -p arcweft-view --all-targets
CARGO_INCREMENTAL=0 cargo test -p arcweft-view --test style_metadata --test logical_axis_cascade --test geometry_contract
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-view --all-targets -- -D warnings
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-bundle --all-targets -- -D warnings
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-render-wgpu --lib -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/native-physical-box-geometry-2026-07-16
```

A continuation audit on the same Jujutsu change also completed successfully:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-view --all-targets
cargo fmt --all -- --check
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The canonical player-gap follow-up completed successfully with:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-player-scene --lib frame::view_style::tests::column_gap_repositions_each_direct_child_subtree_from_actual_bounds -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-player-scene --lib frame::view_style::tests
CARGO_INCREMENTAL=0 cargo test -p arcweft-view --test style_metadata --test logical_axis_cascade
CARGO_INCREMENTAL=0 cargo check -p arcweft-player-scene --all-targets
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-player-scene --all-targets -- -D warnings
cargo fmt --all -- --check
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

That dry-run structural audit scanned 2,959 files, including 1,459 Rust files
and 681,657 physical Rust LOC. It reported 0 errors and 129 repository-wide
warnings. The largest Rust source remains the explicitly generated Unicode
vertical-orientation table; none of the changed files crosses an audit warning
threshold.

The final structural audit scanned 1,416 Rust files and 665,304 physical Rust
LOC. It reported 0 errors and 128 pre-existing or repository-wide warnings. An
earlier run showed that this change had pushed `style/property.rs` above its
warning threshold; geometry support metadata was then moved to
`style/property/geometry.rs`, and the rerun removed that warning. The geometry
modules are responsibility-sized. Exact file/dependency evidence is retained
in:

- [file metrics](structure-audits/native-physical-box-geometry-2026-07-16/file_metrics.csv)
- [dependency edges](structure-audits/native-physical-box-geometry-2026-07-16/dependency_edges.csv)
- [public type duplicates](structure-audits/native-physical-box-geometry-2026-07-16/public_type_duplicates.csv)
- [violations](structure-audits/native-physical-box-geometry-2026-07-16/violations.md)

## Validation prerequisite

`web/assets/noto-sans-jp-vf.ttf` is intentionally ignored but is required by
existing `include_bytes!` owners. The dedicated workspace initially lacked it,
so an early exact-test compile stopped before running the test. A hard link to
the main checkout's existing 9,590,844-byte font was installed as an ignored
local validation prerequisite; no generated or placeholder font and no
repository diff was introduced. The player-scene validations listed above then
passed.

## Remaining TODO

Obtain and apply the d.4.1.2.1 corrected contract, then integrate the existing
checked substrate into the retained runtime and replace every remaining ad hoc
consumer in dependency order. Do not mark the original package's full runtime
goal complete until the reconciliation acceptance matrix and those integration
tests pass.
