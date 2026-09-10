# Ordered structural record storage — 2026-09-11

Status: implemented and validated; known workspace/Tier 2 failures are
retained below. This is completion of the record storage correction only.

The inspected base is `main` at
`fc6bba31548b34391fd70595c8de148476bb4f2b`, with 30 unfinished callable/scope
files preserved separately from this cut. This implements the structural
record storage and canonical identity boundary required by the
[accepted nominal design](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier/FINAL_DESIGN.md)
and its
[wire contract](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier/WIRE_AND_RESTORE.md).
It does not complete nominal C1-C6 or the
[convergence goal](2026-09-08-convergence-goal-plan.md).

## Changed behavior and authority

`RuntimeValue::Record` now owns `RuntimeRecordValue`, whose private inventory
contains contiguous one-based field IDs and unique nonempty names in supplied
order. Public consumers can inspect fields or transform their values, but
cannot reorder, remove or replace the inventory of a live record. The public
`try_record` constructor retains its role of defining an anonymous record in
the supplied order. Record serde retains the existing array shape and validates
the inventory before issuing the value.

One `RecordFieldAdmission` context defines count, ID/order and name admission
for both logical rows and record columns. `RecordSeq` additionally owns column
length validation; the established column-length-before-duplicate-name error
precedence is preserved. `TupleSeq` and `RecordSeq` now deserialize through
their constructors. Their private fields live with their sequence owner.
Previously, derived deserialization bypassed these constructors. Columnar row
projection preserves the admitted IDs and names. Homogeneous rows pack into
columns; heterogeneous valid layouts retain their original logical values.
Malformed record storage cannot enter that fallback.

AWBC snapshot reconstruction delegates both row and column admission to these
owners, preserving the supplied IDs. Its duplicate ordinal validators and
the column path that discarded then reissued IDs are removed. This is storage
admission only; the required program-bound restore authority is still pending.

Canonical record bytes now include each field ID, name and value in accepted
order. Alphabetic sorting is removed. For example, `z = true, a = false`
encodes as `0d 02 01 01 7a 02 01 02 01 61 02 00`. Reversing that declaration
order changes the canonical bytes. Logical rows and columnar rows have the
same transcript and BLAKE3 digest. The preceding count-encoding cut supplies
the shared canonical varint primitive. All contract versions remain `1`.

Canonical sequence encoding and schema validation no longer expand an entire
columnar sequence before checking its limits. They check the encoded length
or schema item bound first and process one row at a time. A compact sequence
with `u32::MAX` zero-column tuple rows rejects a five-byte encoding budget
without expanding its rows; Seq, Bytes and Map schema item limits reject the
same input before iteration.

Dialogue normalization uses the record owner's value mapping operation and
accepts declaration order such as `beta, alpha`. The prior alphabetical-order
rejection is removed. Existing numeric normalization and size limits remain
covered. Core suspension conversion, runtime predicates, accelerator decoding
and Agent test helpers consume the immutable field view.

The old raw record vector, independent row/column admission helpers,
post-admission duplicate-field schema error, and malformed-row success path
are deleted. Runtime checked-type field names remain diagnostic metadata under
the later Generic Match contract: this cut changes value storage/transcripts,
not type equality or predicate identity.

## Validation and exact cut

Logs are under `.arcweft-local/validation/2026-09-11-record-storage/`.

- Initial all-target/all-feature workspace checks exposed consumer migration
  errors; those were fixed and `check-final-migration.log` passed before final
  privacy/restore cleanup.
- `core-tests.log`: 375 library tests passed; the compile-fixture runner
  failed because an edited fixture's expected output was stale. The new owner
  privacy checks were separated into a dedicated fixture so their rejection
  is observed independently of the existing carrier diagnostics.
- `privacy-tests.log`: the new fixture produced the expected three private
  field errors. Its exact diagnostic output was then admitted as the golden.
- `core-tests-final.log`: **408 passed** (375 library, 33 integration,
  including the compile-fixture runner); zero doctests. Coverage includes
  rejected serde inventories, rectangular column validation, immutable field
  metadata, row/column identity, canonical bytes/digest/budgets, and AWBC
  snapshot reconstruction with tampered IDs or names.
- The first `consumer-tests.log` build failed with disk exhaustion (`os error
  112` / `no space on device`). The following workspace check was interrupted;
  no later gate from that initial sequence is credited. With all compiler
  processes stopped and the resolved Cargo target verified as
  `D:\git\arcweft\target`, `cargo clean` removed **240,980 files / 269.4 GiB**.
  Source, preserved WIP and validation logs were retained. The free space after
  cleaning was 273,626,705,920 bytes.
- `changed-crate-tests-after-clean.log`: the isolated cut was rebuilt from an
  empty target; **600 tests passed**: Core 408, Dialogue 38 (including four
  doctests), accelerator 89, Agent runner 65. All four packages used
  `--all-features` in one Cargo invocation with normal concurrency.
- `workspace-check-after-clean.log` and
  `workspace-clippy-after-clean.log`: workspace all-target/all-feature check
  and Clippy **passed**, with warnings. No new crate or feature was introduced.
- `test-workspace-after-clean.log`: **failed** at compiler
  `callable_execution`, with **57 passed / 24 failed**. Exact failing test
  names match the immediately preceding accepted cut: `failure-comparison.json`
  records 24/24 with no additions or removals. Subsequent workspace binaries
  and the recipe's later CLI steps were **not run** after that failure.
- `test-doc-after-clean.log`: **8 workspace doctests passed**.
- `test-tier2-after-clean.log`: MCP **4 passed** and Agent observe **1 passed**;
  native auxiliary capture **failed** at
  `agent_observe_native::agent_observe_read_uri_preserves_animated_image_object_frame_metadata`.
  Its failure name matches the preceding accepted cut (1/1, no additions or
  removals). The unchanged `samples/image-animation.arcw` starts with `pub image`
  and fails with `syntax.parse: unexpected top-level item` before execution.
  Subsequent Tier 2 targets were **not run** after this failure.
- Structure audit and structure gate **passed**: **95 packages, 2,272 Rust
  files, 310 review triggers, zero blocking violations**. Formatting and
  cached diff checks passed.

Cargo commands ran sequentially with normal concurrency and no explicit job
count. The core-only tight loop and the final four-package validation both
used all features; workspace and Tier 2 recipes used their checked-in feature
selection. `just verify`, generated JLREQ checks and the full CLI matrix were
not selected for this storage correction.

Fifteen Rust/fixture files were explicitly staged and their cached diff
reviewed. The Rust index tree is
`799080c17c1dde9f7f18d86a159fd9f3f96e839a` on the base above. All 45 changed
files were preserved as raw byte copies, Git blobs and a SHA-256 manifest.
The 30 unfinished callable/scope files were then isolated with a reversible
patch in the existing main checkout. Core's final run preceded isolation on
identical core/dependency sources; subsequent gates validate the isolated cut.
After validation, all 30 unfinished files were restored. Raw-byte SHA-256
verification passed for the complete 45-file manifest, including this cut's
15 staged Rust/fixture files. No additional checkout, branch or worktree was
created. The separate unfinished component receives no completion credit.

## Structural disposition

[Generated measurements](structure-audits/2026-09-11-record-storage/changed-files.csv)
cover all 15 changed Rust/fixture files, using the canonical audit's full-file
metrics and dependency graph. Touched size/growth review triggers are:

| Owner | Bytes | Physical LOC / growth | Embedded test LOC |
| --- | ---: | ---: | ---: |
| Core record storage | 10,978 | 340 / +340 | 81 |
| Core value algebra | 132,093 | 3,687 / -111 | 0 |
| Core sequence storage | 58,965 | 1,760 / -12 | 142 |
| Core schema/transcript | 61,115 | 1,755 / +76 | 237 |
| Core runtime predicates | 103,140 | 2,835 / 0 | 633 |
| Core suspension conversion | 50,383 | 1,239 / 0 | 0 |
| Accelerator external conversion | 65,691 | 1,623 / 0 | 0 |
| Agent execution tests | 128,534 | 3,554 / +3 | 0 |

Normal dependency fan-in/out is Core 29/6, Dialogue 8/9, accelerator 3/11 and
Agent runner 4/4; development fan-in/out is respectively 3/6, 8/0, 0/0 and 1/4.

The new record module owns one immutable field inventory and its admission
state. It does not duplicate a nominal declaration, type layout or program
catalog. The sequence module owns rectangular column storage and projects
rows through the admitted record owner. Narrow value-module exports retain
existing consumer paths. No dependency, feature or I/O boundary is added.

The 340-line record module is a deliberate decomposition of storage ownership
from the larger value algebra. Its owner-local tests exercise admission and
metadata preservation without widening constructors for tests. The larger
`value.rs` retains the runtime value algebra and storage dispatch; invalid
record inventories are no longer representable through that public algebra.
The sequence owner retains dense/row/column operations on the same storage
state. Its column shape and record admission tests follow that ownership
boundary rather than introducing a parallel field model.

The schema owner retains one canonical visitor shared by bytes and digest
sinks, plus its persistence-schema validation. Both operations consume the
same admitted value model. Pattern validation and suspension conversion only
adapt their field borrowing; this cut adds no predicate state, suspension
policy or transport responsibility. AWBC save projection still owns the
recursive DTO-to-value traversal while delegating storage validation to its
typed owners. Accelerator external conversion likewise retains its existing
adapter boundary. Splitting these touched consumers solely for the borrow
adaptation would not remove a state or dependency boundary.

The 3,554-line Agent runner test module remains a dedicated execution test
owner: only the two existing record-access helper signatures change. Dialogue
tests retain numeric normalization and size-limit coverage while asserting
accepted declaration order. The public compile fixture verifies that the
three structural storage owners cannot be forged with field literals.

## Required continuation and design deviations

No accepted design rule is changed by this cut. Record storage validity does
not establish membership in a nominal declaration or executable program.
The reachable nominal schema graph, four nominal record shapes, exact Rust
ADT join, layout-bearing variants and AWBC rows, program-bound restore and
structural ownership admission remain required C1-C6 work. The unfinished
callable component also remains required by the active goal. This correction
neither freezes those old execution/restore paths nor claims their acceptance.
