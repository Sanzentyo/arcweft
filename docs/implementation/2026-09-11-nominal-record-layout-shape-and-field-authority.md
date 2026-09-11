# Nominal record layout shape and field authority — 2026-09-11

Inspected Git commit `45fe11dc8328647826184035a92cc79835b2b93e` on `main`,
equal to `origin/main`. The index was empty. Existing callable/effect and
nominal-graph work was preserved. Only the complete record-layout change is
selected for this cut; compiler and schema integration use explicit hunks.

## Result and cut boundary

`RuntimeNominalRecordLayout` owns its source shape and explicit ordered field
IDs. Each field retains an optional name and its checked type. Unit, empty
tuple, empty named record, and newtype retain distinct shapes. The one
`RuntimeNominalRecordShape::validate_field_names` rule checks shape, count,
name presence, empty names, and duplicates. Layout admission also requires
contiguous one-based IDs in defining order.

The shape domain lives in `entry/schema/record_shape.rs`, re-exported through
the existing entry boundary. Executable layouts and the preserved, uncommitted
schema-graph admission draft use that owner. This separates the reusable shape
rule from graph
construction, without adding a second shape type or validation rule.

The old `(String, RuntimeCheckedType)` layout input is replaced directly.
Compiler projection preserves the already accepted semantic field IDs. The
current AWBC named-record row derives IDs at its ordinal projection boundary,
then supplies explicit fields to the same layout admission. Project record
facts require the named-record source shape, including for zero fields.
The existing AWBC round-trip test checks the projected shape, IDs, names and
field predicates.

Layout deserialization passes through the same admission function. Raw field
input can be deserialized, but an invalid shape, missing name, duplicate or
reordered ID cannot publish a layout. Value checking uses the admitted IDs;
the repeated value-side ID derivation and its unreachable error branch are
deleted. Field-type errors retain optional names for unnamed fields.

This is the complete existing record-layout responsibility within nominal C1.
Splitting its commit from the graph/Variant work changes the historical cut
grouping, not the selected architecture or acceptance criteria. There is no
temporary constructor, alternate descriptor, compatibility reader, version
bump, or ownership-success branch. The remaining C1 work is still required.

Not claimed: graph `accepts_value`, layout-bearing Variant owners, graph-bound
layout construction, C2-C6 completion, AWBC support for unnamed nominal
record rows, program-bound restore, or deletion of unchecked value creation.
The preserved callable failures also remain part of the convergence goal.

## Validation

Logs are under the ignored directory
`.arcweft-local/validation/2026-09-11-effect-row-formulas/`, prefixed
`nominal-layout-`. Commands below run against the preserved complete working
tree, not an isolated checkout or the staged subset.

- Passed: `cargo fmt -p arcweft-core -p arcweft-compiler -p
  arcweft-runtime-plan` and `git diff --cached --check`.
- Passed: core layout tests, 7; core all-feature library/integration tests,
  436 (403 library and 33 integration); the one exact runtime-plan project
  record-shape test. The core run includes the AWBC projection round trip.
- Failed then corrected: the first focused/check attempts imported the private
  schema module instead of its public re-export. A later core test compile
  exposed an unqualified field-ID name in the added AWBC assertion. Both were
  corrected before the passing tests; their failed logs are retained.
- Passed: `cargo check --workspace --all-targets --all-features` and
  `cargo clippy --workspace --all-targets --all-features`, both with warnings.
  Clippy includes one nonblocking redundant-closure finding in the changed
  `field_id` accessor; no unrelated lint cleanup is included.
- All 71 retained design ZIPs were enumerated and rehashed; all match the
  previously inspected archive inventory, with no added or changed archive.
- Failed: `just test-workspace` stops at its first Cargo command, with 768
  tests passed and 28 failed in the completed binaries. Compiler library
  results are 58 passed / 28 failed. The 28 failure names exactly match the
  writer-cut run: 27 report `semantic effect row is not closed`, and the other
  expects the later analyzed lease. The remainder of that Cargo run and the
  recipe's CLI commands did not run. This is not an accepted regression
  baseline or a passed workspace gate.
- Passed: the canonical structural screening and blocking gate,
  `cargo +nightly -Zscript tools/structure-audit.rs --root .
  --fail-on-blocking`. No retained generated report was needed; the full log
  reports 95 packages, 2,295 Rust files, 1,272,855 physical Rust lines,
  309 review triggers and zero blocking violations. The ownership disposition
  is below.
- Passed: `just test-doc`, 8 doctests passed and none failed across the
  workspace's 95 completed test groups.
- Failed: `just test-tier2`, 5 passed / 1 failed in three completed test
  groups. MCP stdio and slow Agent observe passed. The first auxiliary
  capture test, `agent_observe_read_uri_preserves_animated_image_object_frame_metadata`,
  rejects `samples/image-animation.arcw:1` at `pub image` with
  `syntax.parse: unexpected top-level item`; HIR admission then rejects the
  recovered module. No image-capture result is accepted. The remaining
  auxiliary capture commands, visual goldens, and Select/Flow production-boundary
  recipes did not run. The sample and parser are unchanged by this cut. Their
  reconciliation and complete Tier 2 rerun remain required by the goal.

## Ownership review

Current complete working-tree measurements: nominal record owner 653 lines /
21,835 bytes; shape rule 91 / 3,152; AWBC type projection 525 / 21,130;
schema integration 1,163 / 37,520; compiler lowerer 7,862 / 332,664; runtime
semantic facts 10,464 / 398,367; semantic-fact tests 2,901 / 105,099; AWBC
tests 5,012 / 178,968. These include preserved changes outside this commit.

The touched large compiler/fact owners remain the existing dependency-inversion
and generation-admission boundaries. This cut changes only their typed record
projection and source-shape correlation; it introduces no unrelated state,
graph traversal, transport, I/O, crate, feature, or dependency. Schema receives
only the shape module/export in the staged cut. The shape/name invariant is
decomposed into its own shared owner instead of being copied into those large
modules. The large test modules receive the corresponding AWBC round-trip
assertions and a direct generation-fact admission test. Their existing broader
responsibilities are not claimed as redesigned by this cut.
