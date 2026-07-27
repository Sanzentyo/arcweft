# Proof convergence: obsolete reference HIR deletion

Date: 2026-07-27

Status: `LANDED_VALIDATED_WITH_EXISTING_WORKSPACE_BASELINE`

## Boundary

This deletion-driven cut removes the zero-consumer public
`arcweft_lang_hir::reference` island. Its provisional `HirRegion`,
`HirBorrowExpr`, `HirDerefExpr`, and `HirReferenceType` types occurred nowhere
outside their defining file and were not part of the active lowering,
semantic, verifier, compiler, tooling, or runtime path.

The deleted payloads mixed syntax-owned `LifetimeName`, `BorrowKind`, and
`TextRange` values directly into a nominal HIR surface. That shape conflicts
with the accepted final direction in which semantic payloads and source roles
have separate owners. Keeping the unused module would therefore create a
plausible but false public authority for later consumers.

The root `pub mod reference` declaration and the complete implementation file
were deleted together. A compile-fail row proves that the module and all four
types are no longer public. No alias, re-export, wrapper, compatibility reader,
source-string reparse, source gate, or removed-syntax diagnostic replaces
them.

This cut does not remove or repair the active syntax-owned reference and
lifetime grammar. Its sema and runtime consumers continue to compile through
the existing production path.

## Deferred final owner

The final `HirTypeRegion` family is intentionally not guessed here. Proof
01.1.1.4.1.1 must close the exact TypeId-owned `SyntheticOwner` representation
for elided regions before that family can be constructed without a sentinel or
untyped owner flag. The final type is added only with its first attached-HIR
consumer; it is not kept alive through dummy use or lint suppression.

## Validation

Completed:

- workspace consumer inventory: every `HirRegion`, `HirBorrowExpr`,
  `HirDerefExpr`, and `HirReferenceType` occurrence was confined to the deleted
  file; the similarly named `arcweft::reference` facade belongs to the separate
  `arcweft-ref` crate and was not changed;
- `cargo test -p arcweft-lang-hir --test public_api --all-features --
  --nocapture`: passed with the new module-removal compile-fail row;
- `cargo test -p arcweft-lang-hir --all-targets --all-features`: passed (85
  unit tests and every HIR integration/compile-fail suite);
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `cargo test -p arcweft-lang-syntax --lib --all-features`: all 494 tests
  passed; and
- `cargo fmt --all -- --check` and `git diff --check`: passed.

`just test-workspace` exceeded the 602-second outer runner limit while its
trybuild stages were still active. Closing the output pipe caused the current
`arcweft-lang-syntax --lib` process to report `BrokenPipe`; this was an
infrastructure interruption rather than an assertion failure. The interrupted
494-test target was rerun directly and passed in full as recorded above.

The fixture harness was then run directly with
`cargo test -p arcweft-cli --test arcw_fixtures_check_run --all-features --
--nocapture`. It reported the exact parent baseline: three passes and two
failures:

- `spec_should_pass_check_fixtures_pass_after_refactor` for
  `010_capability_fs_read.arcw`; and
- `spec_should_pass_run_fixtures_pass_after_refactor` for
  `002_file_read_task.arcw`.

Both fixtures require final attached-HIR publication of capability-owned
`FsError`. This zero-consumer deletion neither changes that owner nor adds a
fallback nominal, fixture bypass, compatibility reader, or source gate.

The final design-package ledger compared all 30 retained
`docs/reviews/**/*.zip` archives against package-specific implementation
records: zero unrecorded or changed archives and zero root-inbox ZIPs.

Tier 2 is not applicable. This public surface deletion changes no runtime,
render, Agent, MCP, capture, persistence, or serialized behavior.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-obsolete-reference-hir-deletion-2026-07-27/`](structure-audits/proof-obsolete-reference-hir-deletion-2026-07-27/).
The final pass scanned 3,769 files, including 1,955 Rust files and 906,160
physical Rust LOC, and reported zero errors plus 146 existing warnings. Its
warning headings are identical to the immediately preceding audit.

Representative retained changed metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-lang-hir/src/lib.rs` | 621 | 27 | production |
| `arcweft-lang-hir/tests/public_api.rs` | 756 | 14 | test |
| `arcweft-lang-hir/tests/ui/removed_reference_hir.rs` | 201 | 7 | test |

The deleted `arcweft-lang-hir/src/reference.rs` contained 123 physical lines.
No new structural error or warning category was introduced.
