# Proof convergence: attached if-let and unsafe-audit syntax

Date: 2026-07-27

Status: implementation and cut-specific validation complete

## Context

The local Proof HIR decisions already fix statement-form `if let` as a
dedicated `HirIfLetStmt` and unsafe-audit edit ownership as a revision-bound
`UnsafeAuditInsertion` source component. They are recorded in
[`2026-07-26-proof-hir-local-schema-decisions.md`](2026-07-26-proof-hir-local-schema-decisions.md)
and require no additional design request.

The returned Proof-concurrency `v6.1.1.4` archive remains the rejected
SHA-256
`414f95f8ef4c5f3abcce163f0c9b01f124098f0bac856f174af09b5c1e7d564b`.
The final expression leaf and full HIR database switch therefore remain gated
on the corrected
[`01.1.1.4.1` redelivery](../reviews/requests/2026-07-26-seq-proof-01.1.1.4.1-final-hir-leaf-expression-redelivery-correction.md).

The attached syntax owner for the two locally fixed statement rows does not
depend on that leaf schema and was independently implementable without
repairing the detached AST/HIR path.

## Implemented final syntax owner

- added an exact attached `IfStatementHeadNode` that distinguishes ordinary
  conditions from `let { pattern, scrutinee, guard }` heads by typed child
  roles;
- added typed required then-block and optional else-block/nested-if accessors,
  preserving nested `else if let` statement identity in the same snapshot;
- added ordinary attached-block delimiter, ordered statement, tail, and close
  access used by those branches;
- added typed unsafe-lifetime body and audit insertion-anchor accessors; the
  anchor is the exact opening delimiter owned by the same immutable syntax
  snapshot;
- changed the attached shadow grammar's missing-`=` recovery to emit one
  zero-width `MissingExpression` with the `Scrutinee` role instead of omitting
  the required child;
- retained an authored opening brace separately from a zero-width missing close
  in an unclosed unsafe body, while a statement with no body exposes no anchor;
- added no text reparse, range search, wrapper, compatibility alias, dual
  reader, source gate, or removed-syntax diagnostic.

The detached `parser/statements.rs`, detached statement AST, old
`HirFlowItem::IfLet(HirIfLet)`, and all current consumers are unchanged. They
are frozen rather than repaired and remain deletion inventory for the Stage 3
public-authority switch.

## Explicit remaining boundary

This cut is not completion of Proof `01.1.1.3` at the HIR/project boundary.

- `HirIfLetStmt` allocation, pattern-local scope visibility, child liveness,
  and transactional rollback require the final statement/expression arenas;
- `UnsafeAuditInsertion` publication requires the accepted HIR module,
  statement arena, source-component table, source snapshot, and an authored
  close delimiter as well as the opening anchor;
- verifier/LSP intentionally continue to publish the host command without a
  source edit until that accepted source-component query exists;
- the same compiling authority switch must connect these attached nodes to the
  final HIR database and delete the old compressed statement/flow carriers.

The protected WIP already demonstrates these database transactions, but it is
not cherry-picked because it is coupled to provisional leaf records rejected
by the `01.1.1.4` intake.

## Validation

- working change: `txurzumnxqpsyxnokmttypkvnypxnkym` over parent
  `c8f320aa1b6b`;
- `cargo fmt --all -- --check`: passed;
- `cargo test -p arcweft-lang-syntax attachment::tests::`: passed all 18
  attached snapshot tests, including the three new canonical/recovery rows;
- `cargo test -p arcweft-lang-syntax`: passed 476 library tests and every
  syntax integration, compile-fail, and documentation test;
- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D
  warnings`: passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- independent read-only review reported no P0/P1/P2 findings for exact roles,
  recovery, snapshot identity, unsafe-brace behavior, or legacy-path changes;
- `just test-workspace`: every crate, integration, and compile-fail suite
  passed before the final `arcw_fixtures_check_run` suite retained the two
  pre-existing `FsError` fixture failures
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`;
- `cargo test -p arcweft-cli --test arcw_fixtures_check_run -- --nocapture`
  independently confirmed exactly those two failures and no others;
- `cargo +nightly -Zscript tools/structure-audit.rs --root . --write
  docs/implementation/structure-audits/proof-attached-if-let-unsafe-audit-syntax-2026-07-27`:
  scanned 3,712 files, 1,939 Rust files, and 902,619 Rust physical LOC; reported
  0 errors and 144 existing warnings. Exact measurements and dependency
  evidence are retained in the
  [structure audit](structure-audits/proof-attached-if-let-unsafe-audit-syntax-2026-07-27/violations.md).

The changed Rust files are `attachment.rs` at 52,570 bytes / 1,495 physical
LOC, `attachment/access.rs` at 28,348 bytes / 818 LOC, and
`parser/statement.rs` at 26,496 bytes / 786 LOC. `attachment.rs` crosses the
ordinary production warning threshold only when its embedded private-snapshot
test module is counted: production attachment ownership ends at line 330 and
the embedded test module occupies 1,165 LOC. The production responsibility is
cohesive and within the preferred range; the test inventory is below the
integration-test warning threshold and remains colocated while the entire
attachment module is crate-private. No dependency edge or public contract
changed. This syntax-only cut does not affect runtime, render, Agent, MCP, or
capture behavior, so Tier 2 is not required by the test-execution policy.
