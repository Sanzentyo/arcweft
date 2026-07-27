# Proof convergence: unused integer-suffix display deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

## Boundary

This deletion-driven cut removes the unreleased `Display` implementation from
`arcweft_lang_syntax::expr::IntSuffix`. Workspace-wide consumer discovery found
no production formatting, `to_string`, or trait-bound use of that
implementation. The active explicit spelling authority remains
`IntSuffix::as_str`, which every current consumer can call without an implicit
string conversion.

The existing syntax public-API compile-fail fixture now proves that downstream
code cannot recover the discarded trait surface. This is Rust type-check
evidence, not a source-text gate. No renamed formatter, wrapper, alias,
extension trait, compatibility shim, dual reader, source reparse, or removed
syntax diagnostic replaces the deleted implementation.

## Retained active sequence API

The same audit initially identified `NumericBracketSeq::is_empty` as having no
direct call site. Removing it was rejected by strict Clippy because the active
public `NumericBracketSeq::len` method has multiple compiler, semantic, and
runtime-plan consumers and therefore requires the conventional `is_empty`
companion. The method was restored before this cut was finalized; no lint
allowance or replacement helper was added.

`NumericBracketSeq` remains a current raw syntax owner. Its lossy downstream
readers are frozen rather than repaired. The sequence type and those readers
must be deleted together only when the corrected attached-HIR leaf payload is
published to every consumer in one authority switch.

## Validation

Completed:

- `cargo fmt --all -- --check`: passed;
- `cargo test -p arcweft-lang-syntax --test public_api --all-features`:
  passed, including the new missing-`Display` row;
- `cargo test -p arcweft-lang-syntax --all-targets --all-features`: passed
  before the final restoration of the unchanged `is_empty` method, with 494
  unit tests and every integration/compile-fail suite;
- `cargo check --workspace --all-targets --all-features`: passed before that
  restoration; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed in the final state.

An exploratory command added `clippy::pedantic`, `clippy::nursery`, and
`clippy::cargo` to the canonical strict gate. It failed on thousands of
pre-existing workspace metadata and documentation-policy findings. Those
unadopted lint groups were not used as the completion gate, no unrelated lint
cleanup was mixed into this cut, and no allowance was added.

`just test-workspace` ran for 548.8 seconds. It passed the changed syntax crate,
the new compile-fail row, and every preceding workspace, integration, and
compile-fail stage, then stopped at the established
`arcw_fixtures_check_run` baseline. The exact suite was rerun and reported
three passes plus the same two failures present at the parent revision:

- `spec_should_pass_check_fixtures_pass_after_refactor` for
  `010_capability_fs_read.arcw`; and
- `spec_should_pass_run_fixtures_pass_after_refactor` for
  `002_file_read_task.arcw`.

Both require final attached-HIR publication of the capability-owned `FsError`.
This syntax trait deletion neither changes that owner nor adds a fallback
nominal, compatibility reader, fixture bypass, or source gate.

The final design-package ledger compared all 30 retained
`docs/reviews/**/*.zip` archives against implementation records: zero
unrecorded or changed archives and zero root-inbox ZIPs.

Tier 2 is not applicable. This cut removes one zero-consumer syntax trait
implementation and does not change runtime behavior, rendering, Agent, MCP,
capture, persistence, or serialization.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-unused-int-suffix-display-deletion-2026-07-27/`](structure-audits/proof-unused-int-suffix-display-deletion-2026-07-27/).
It scanned 3,775 files, including 1,956 Rust files and 906,122 physical Rust
LOC across 95 manifests. It reported zero errors plus 146 existing warnings;
the warning-heading inventory is identical to the immediately preceding
audit.

Changed-file measurements are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-lang-syntax/src/expr/numeric.rs` | 11,122 | 371 | production syntax |
| `arcweft-lang-syntax/tests/ui/removed_zero_consumer_syntax_accessors.rs` | 668 | 26 | compile-fail test |
| `arcweft-lang-syntax/tests/ui/removed_zero_consumer_syntax_accessors.stderr` | 2,114 | 43 | expected compiler output |

The production owner remains far below the 1,200-line warning threshold and
this cut changes no crate dependency, Cargo feature, root re-export, or module
responsibility.
