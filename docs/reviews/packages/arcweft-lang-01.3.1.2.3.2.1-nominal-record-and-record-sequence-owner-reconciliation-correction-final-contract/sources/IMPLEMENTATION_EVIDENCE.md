# Lang-01.3 G1.2-A representation foundation and carrier blocker

Date: 2026-08-11

Inspected clean production baseline and pushed result:
`08bc30c0c8eac77152a42e92a5ca2f83280b94bc` on `main`, equal to
`origin/main` after the production cut.

This note supersedes only the production-readiness conclusion in
`2026-08-11-lang-01-3-1-2-3-2-return-intake.md`. The archive integrity,
package-validator results, accepted identity/slot/path decisions, and retained
ZIP hash remain valid evidence. G1.2-A as a whole is not complete.

## Performed and passed

The production cut adds the independently closed G1.2-A representation
foundation:

- seventeen private-field nonzero runtime identity wrappers;
- `RuntimeIdCursor`, namespace/exhaustion behavior, strict human-readable
  Serde, and fixed-LE scalar/cursor codec;
- one-based `RuntimeRecordFieldId`;
- complete `RuntimeOwnedSlotId` ordering, diagnostic rendering, strict Serde,
  fixed-LE codec, and all eight golden variants;
- the exact ten-segment `RuntimeValuePath`, manual lexical ordering, 64-segment
  limit, strict Serde, fixed-LE codec, and invalid-vector rejection; and
- compile-fail evidence that raw execution and record-field constructors are
  inaccessible.

Validation on the final production tree passed:

- `cargo test -p arcweft-core --all-features --jobs 4`: 265 unit tests,
  1 trybuild harness with 3 compile-fail cases, 21 integration tests, and 0 doc
  tests passed;
- `cargo check --workspace --all-targets --all-features --jobs 4`: passed;
- `cargo clippy --workspace --all-targets --all-features --jobs 4 --
  -D warnings`: passed;
- focused identity, record-ID, ownership/path/slot/codec tests: passed; and
- `cargo fmt --all` plus `git diff --check`: passed.

The trybuild dependency patch notices are pre-existing informational Cargo
output; they did not fail the harness.

## Structural review

The touched lower owners are `arcweft_core::runtime_id`,
`arcweft_core::value`, and `arcweft_core::value::ownership`.

- fixed-width identity codec mechanics are isolated under `runtime_id/binary`;
- record-field identity remains under the existing value owner;
- path, diagnostic slot union, and their independent binary codec are separate
  cohesive modules under the existing ownership owner; and
- workspace compilation confirms no reverse dependency from core to HIR,
  sema, runtime-plan, or driver.

No second value model, side identity table, source-string recovery, fallback
reader, fake affine token, View path, or Stream handle was introduced.

## Blocked

The returned contract calls `RuntimeNominalRecordSchema` and `RecordSeqError`
existing owners, but neither symbol exists in current source or any Git
history. The archive references both without defining them. Nearby
`RuntimeNominalRole`, `RuntimeTypeSchema`, sema accepted records, and
`RuntimeSeqError` differ in layer or observable behavior; selecting one locally
would decide schema sharing/validation, public error variants, and deterministic
failure precedence.

The independently throwable blocker request is
[Lang-01.3.1.2.3.2.1 nominal-record and record-sequence owner reconciliation](../reviews/requests/2026-08-11-lang-01.3.1.2.3.2.1-nominal-record-and-record-sequence-owner-reconciliation-correction.md).

Until it returns, the following are explicitly blocked and receive no G1.2-A
completion credit:

- changing `RuntimeFieldValue` or `RecordSeqField` carriers;
- anonymous/column/nominal record admission and deletion of unchecked nominal
  construction;
- wiring record identities into the shared value-path visitor; and
- proceeding to G1.2-B through G1.2-F, G1.3/G1.4, View expansion, AWBC wire
  publication, activation, or Stream publication.

## Failed, not run, and non-goals

One intermediate focused cursor test exposed acceptance of an exhausted JSON
cursor carrying `value: null`; the decoder was corrected to a strict struct
variant and the focused test plus final full core suite passed. This was not a
remaining failure.

Workspace-wide tests, Tier 2, runtime-driver activation/save/restore tests, and
a generated structure-audit report were not run for this additive lower-layer
cut. No carrier, transaction, snapshot, activation, View, ABI, or Stream
behavior was changed.
