# Relative Runtime ID Boundaries — 2026-07-07

## Status

This cut applies the seq-07.6 final-shape intent without keeping
`FlowRuntimeId(String)`, `EntryRuntimeId(String)`, or `RuntimeLineId(String)` as
tuple string newtypes.

Implemented:

- `FlowRuntimeId`, `EntryRuntimeId`, and `RuntimeLineId` now store a typed
  `RuntimeIdPath`.
- `RuntimeIdPath` stores validated `RuntimeIdSegment` values. It does not store
  source-family prefixes such as `flow`, `entry`, or `say`.
- Source-side absolute/current/parent-relative references are represented by
  `RuntimeIdReference` and `RuntimeIdReferenceAnchor`; execution-facing runtime
  IDs are already resolved paths.
- `RuntimePublicLabel` is the explicit public/debug string domain for AWBC,
  manifests, logs, diagnostics, and reports.
- Tuple constructors, `.0` field access, `From<&str>` conveniences, and hidden
  `flow.*` runtime aliases were removed from the migrated call sites.
- Source-boundary conversions are explicit through:
  - `FlowRuntimeId::from_source_entity_body(...)`
  - `FlowRuntimeId::from_runtime_target_value(...)`
  - `EntryRuntimeId::from_source_entity_body(...)`
  - `RuntimeLineId::from_source_entity_body(...)`
  - `RuntimeLineId::from_runtime_line_value(...)`

## Deviation From The Zip

The supplied seq-07.6 package proposed `RuntimeIdTable` and numeric
`RuntimeIdAtom` storage as the immediate final representation. I intentionally
did not apply that atom-table shape in this cut.

Reason:

- The current engine has not shown runtime-ID equality or hashing as a measured
  bottleneck.
- Adding `RuntimePlan::runtime_ids` now would force every lowering, bundle,
  save/load, AWBC, and report boundary to carry table context before the
  performance need is demonstrated.
- The public API is still opaque: callers interact with typed ID wrappers,
  `RuntimeIdPath`, and label methods. If profiling later shows ID comparison or
  storage cost matters, `RuntimeIdPath` can be changed internally to interned
  atoms without reintroducing source-family strings or public-label splitting.

This is a design deviation, not a compatibility shim. The important final-shape
constraint from the package is preserved: runtime lookup IDs are typed paths,
not raw `flow.*`/`say.*`/`entry.*` strings.

## Boundary Design

Arcweft now treats three ID domains as separate values:

1. **Source references** live in parser/HIR/lowering while relative syntax is
   still meaningful. They may include family syntax such as `@flow.main` or
   parent/current-relative addressing.
2. **Canonical runtime IDs** are lookup keys. The owning Rust type carries the
   family, so source `@flow.main` lowers to a `FlowRuntimeId` whose canonical
   label is `main`.
3. **Public/debug labels** are deliberate strings. `flow.chapter.one.main` is a
   label, not a namespace selector and not a lookup key.

Runtime code must not recover a lookup ID by splitting a public/debug label.

## Diagnostics

`RuntimeIdError` is the structured boundary diagnostic type:

- `Empty { family }`
- `EmptySegment { family }`
- `ReservedFamilySegment { family, segment }`
- `WrongSourceFamily { expected, found, value }`
- `MissingSourceFamily { expected, value }`

Passing `flow.main` to `FlowRuntimeId::canonical(...)` fails because `flow` is a
reserved source-family segment. The correct boundary is
`FlowRuntimeId::from_source_entity_body("flow.main")` or
`FlowRuntimeId::from_runtime_target_value("flow.main")`, depending on whether
the call site owns source syntax or a runtime/debug string boundary.

## Remaining Follow-Up

- AWBC lowering still emits and sometimes indexes public strings at some
  boundaries for product-function names and external debug/report surfaces. This
  is no longer because `FlowRuntimeId` is a string newtype, but further cleanup
  should continue moving internal maps toward typed keys where the AWBC schema
  allows it.
- A true atom table should only be introduced after profiling shows ID
  comparison, hashing, serialization size, or allocation cost matters enough to
  justify carrying table context through runtime-plan/data-format boundaries.
- Source/HIR relative selector resolution should continue to use
  `RuntimeIdReference`-style typed anchors instead of smuggling relative
  selectors into execution-facing runtime IDs.

## Validation

Passed in this cut:

```bash
cargo test -p arcweft-core --test runtime_id_boundaries --all-features
cargo check --workspace --all-targets --all-features
cargo clippy -p arcweft-core --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit-runtime-id-owned-path
```

`cargo check --workspace --all-targets --all-features` reports one unrelated
existing warning for `ViewLayoutFrame::text_line` in
`crates/arcweft-cli/src/app/bundle_view.rs`.

The structure audit reports the existing `crates/arcweft-cli/src/app/bundle_view.rs`
size error and 148 warnings. The temporary `product_step.rs` size error created
while migrating AWBC runtime-ID boundaries was removed by splitting
`product_step/runtime_id.rs`.
