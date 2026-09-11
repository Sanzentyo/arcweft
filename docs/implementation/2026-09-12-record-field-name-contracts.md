# Record field names in checked contracts

Date: 2026-09-12.
Inspected base: `4fc0b705d1479f947c7bafdd539aad94bc228482` on `main`.
The checkout contains preserved nominal/schema, Rust ADT, and callable WIP.
Validation below ran in that dirty checkout; it does not establish completion
of those migrations.

## Accepted scope

The accepted structural nominal design requires structural record predicates
to retain exact field IDs, names, order, and recursive types. Previously,
`RuntimeCheckedRecordField` equality and checked-type digest omitted names,
and value acceptance checked only field coordinates and child types. Plan
record-field equality also ignored names, allowing a conflicting declaration
to coalesce with an existing semantic type row.

This cut makes names participate in all four places. A field rename or reorder
now changes the checked contract even when every child type is identical.
Plan admission rejects a conflicting name and leaves the whole candidate
batch unpublished. The maintained executable-runtime chapter records this
rule. Contract versions remain `1`.

Sources:

- [Accepted structural nominal design, checked predicates](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier/FINAL_DESIGN.md).
- [Prior nominal record-domain propagation](2026-09-12-plan-record-domain-shapes.md).

Only record-name hunks in `pattern.rs`, record-field equality in
`plan/type_kind.rs`, the updated AWBC record tests, the new
`tests/record_field_names.rs`, and the two documentation files belong to this
commit. Existing Variant layout and AgentValue changes in `pattern.rs` remain
outside it. Graph-aware plan value admission, its shared visitor changes, and
builtin payload-table admission remain working-copy work.

## Validation actually run

- `cargo check -p arcweft-core --all-targets --all-features`: passed.
- `cargo test -p arcweft-core --all-features`: final run passed **506 tests**
  (473 unit, 33 integration), with zero core doctests. This includes eight
  tests for adjacent, uncommitted plan-value work.
- `cargo test -p arcweft-core --all-features --test record_field_names --test
  awbc_record_type`: **5 passed**. The two new record-name integration tests
  were added after the full run; the other three repeat the AWBC record suite.
- `cargo clippy -p arcweft-core --all-targets --all-features`: final run passed,
  with **125 library warnings and 145 test-library warnings (124 duplicates)**.
  This is not a warning-free result.
- `cargo fmt -p arcweft-core` and `git diff --check`: passed.
- `just structure-audit-gate`: passed; 95 packages, 2,320 Rust files,
  1,280,677 physical Rust LOC, 310 review triggers, zero blocking violations.
- `cargo check --workspace --all-targets --all-features` and
  `cargo clippy --workspace --all-targets --all-features`: failed at
  `arcweft-host-adapter/src/lib.rs:503`, which still reads the removed Rust
  `opaque_producer` getter.
- `just test-workspace`: failed during the first Cargo compilation at the
  same host-adapter error. Subsequent CLI commands were not run.
- `just test-doc`: failed during dependency compilation at that same error.
- `just test-tier2`: failed in `test-slow-mcp` dependency compilation. It also
  exposed the preserved missing nominal Variant `layout` fields in
  `arcweft-dialogue/src/character_dialogue/schema.rs:83` and
  `arcweft-runtime-accelerator/src/external.rs:543`. The ignored MCP test and
  later Tier 2 recipes did not execute.

Tight-loop failures were resolved before the final core checks: the initial
new plan-value code had two type-conversion and two test-visibility errors;
an intermediate core test run had 470 passes and two graph-diagnostic failures
after builtin validation changed rejection order. Graph cycle/depth checks
again precede payload admission. An initial core Clippy run also exposed an
oversized new build-error variant in adjacent WIP; boxing its nested error
removed that warning expansion before the final lint run.

Cargo commands were sequential, without explicit job counts. Logs are under
`.arcweft-local/validation/2026-09-11-effect-row-formulas/`, using the
`record-field-names-` and `plan-value-` prefixes. `git fetch --no-tags origin
main` confirmed that the remote still matched the inspected base before
staging. Explicit paths and five selected record-only hunks were reviewed for
the commit; unrelated changes were preserved.

## Ownership review

Current working-copy measurements for the changed Rust owners:

| Owner | Physical LOC | Bytes | Classification |
| --- | ---: | ---: | --- |
| `core/src/pattern.rs` | 2,916 | 106,772 | production with 692 embedded test LOC |
| `core/src/plan/type_kind.rs` | 729 | 28,880 | production type algebra |
| `core/tests/awbc_record_type.rs` | 168 | 6,085 | integration test |
| `core/tests/record_field_names.rs` | 91 | 3,056 | integration test |

`pattern.rs` was 2,944 physical LOC at the inspected commit; the working-copy
measurement also includes previously preserved WIP. Its existing upper size
trigger was reviewed. The affected state is the checked record field itself;
equality, identity encoding, and value admission consume that same field.
These edits introduce no parallel state or new dependency. Keeping these
coupled rules on their existing owner is the cohesion justification for this
cut; file size alone is not treated as a successful decomposition. The new
integration tests exercise the public predicate and atomic plan admission
boundaries. The structured dependency gate reports no blocking edge.

## Remaining work and deviations

There is no design deviation in this cut. Whole nominal/schema admission,
recursive plan/AWBC value acceptance, accepted Rust producers, host result
authority, Dialogue/data producers, program-bound restore, and ownership
success remain unfinished. The existing
[producer/schema closure request](../reviews/requests/2026-09-11-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1.1-nominal-variant-layout-producer-and-schema-closure.md)
retains that design work. This record does not close C1-C6 or the convergence
goal, and a core test pass does not substitute for the failed workspace gates.
