# Lang-01.3 runtime-value ownership classification

Date: 2026-08-11

Inspected production baseline:
`c21dd430d6f4a3c355bc951f35a0adbd88d36d50` on `main`, equal to
`origin/main` before this implementation cut. The checkout contained no
unrelated tracked or untracked changes.

## Outcome

The first compile-clean Lang-01.3.1.2.3 production subcut adds the final
two-point ownership lattice at
`arcweft_core::value::ownership::RuntimeValueOwnership` and inherent,
exhaustive structural classification on the existing `RuntimeValue`,
`RuntimeFunctionValue`, and `RuntimeSeq` owners.

Every currently constructible runtime leaf is `Unrestricted`. Aggregate,
variant, function-capture, sequence-column, iterator, and witness-state
classification traverses the live transitive value graph. In particular, the
current index-backed value iterator visits only its unconsumed suffix, matching
the final contract's remaining-element save/drop meaning until that owner is
replaced in place by `IntoIter<RuntimeValue>`.

The enum derives only data-safe traits and has stable `unrestricted` / `affine`
Serde names. `join` is the affine-dominant lattice operation, and
`permits_copy` is true only for `Unrestricted`.

## Deliberate boundary

This is an additive classifier cut inside G1, not completion of G1.1 or the
affine migration. It does not add `RuntimeValuePath`, duplication errors,
owner tokens, allocators, slots, drop transactions, checked duplication,
closed payloads, checked-type ownership, View facts, AWBC changes, or Stream
handles.

`ExecutionInstanceId`, `RuntimeRecordFieldId`, and `RuntimeLocalSlotId` do not
yet have production owners or representations. Their shapes were not invented
inside `arcweft-core` merely to publish the later path/error signatures. The
context-free checked type also cannot prove transitive nominal layouts from
its two nominal IDs, so no misleading `RuntimeCheckedType::ownership()`
authority was added. Exact static ownership remains for the canonical
type/layout-context cut.

No compatibility alias, parallel value model, side table, fake affine token,
source-string gate, View change, or wire-version change was introduced.

## Validation evidence

Passed:

- `cargo fmt --all -- --check`;
- `cargo metadata --no-deps --format-version 1`;
- `cargo check --workspace --all-targets --all-features --jobs 4`;
- `cargo clippy --workspace --all-targets --all-features --jobs 4 -- -D warnings`;
- `cargo test -p arcweft-core --lib --jobs 4`: 248 passed, 0 failed,
  including five ownership tests;
- `just structure-audit`: 2,149 files, 2,021 Rust files, 95 packages,
  182 review triggers, 0 blocking violations; and
- `just structure-audit-gate`: passed with 0 blocking violations.

`just test-workspace` was attempted twice with `CARGO_BUILD_JOBS=4` and did not
pass as a whole:

1. the first run exhausted D: while linking accumulated historical Cargo
   artifacts (`LNK1318`, LLVM no-space, and OS error 112); and
2. after verifying the exact workspace target and running `cargo clean`
   (285.9 GiB of reproducible `target/` artifacts removed), the clean run
   reached the tests but failed 6 of 7 existing
   `arcweft-compiler/tests/view_product.rs` cases.

The six failures are
`compiler_lowers_every_typed_view_into_one_validated_product`,
`compiler_rejects_nested_view_recovery_before_product_acceptance`,
`compile_project_retains_view_diagnostic_source_and_both_collision_spans`,
`compiler_rejects_every_unknown_text_control_and_scroll_policy_symbol`,
`compiler_retains_authored_owner_for_signature_and_default_failures`, and
`compiler_rejects_well_formed_view_values_without_a_typed_runtime_contract`.
They exercise the legacy/final-HIR View reconciliation that the accepted
Lang-01.5.1.1.2 contract replaces. The new ownership API has no production
consumer yet; repository search found its calls only in its five local tests.
The failures are therefore retained migration evidence, not hidden or claimed
as passed by this isolated core cut.

Tier 2 was not run. This classifier does not yet connect a runtime, adapter,
bundle, save, or host path, so the test-execution policy does not select a Tier
2 scenario for this subcut.

## Structural review

`value.rs` was already above the repository size review trigger. The new
classification was placed in the existing `value` module at
`value/ownership.rs` (239 physical lines including tests), leaving the large
owner as a one-line module declaration rather than adding more behavior to its
facade. The inherent methods remain on the Arcweft-owned runtime types, and no
dependency edge or crate boundary changed.
