# Lang-01.1.1 typed suspension diagnostics

Date: 2026-07-24

## Outcome

The Lang-01.1.1 semantic negative rows A017, A024, and F007 now use typed
diagnostic owners instead of the former generic `TypeCheckError::new` paths:

- non-`Need<T, E>` direct Await and Flow await-with publish
  `TypeCheckErrorKind::AwaitOperandNotNeed` with stable code
  `sema.await.operand_not_need`;
- a source-backed direct Await labels the exact parser-owned operand range;
- a `ThreadHandle` operand follows that same type rule without a name special
  case or implicit conversion; and
- active borrows at suspension boundaries publish
  `TypeCheckErrorKind::BorrowAcrossSuspension` with stable code
  `sema.suspend.borrow_across`. Direct Await labels only its exact
  parser-owned `await` keyword and retains the active lifetime inventory.

Flow await-with does not currently own an exact Await keyword range in HIR. It
therefore publishes the typed operand and borrow failures without fabricating
a source span. Adding a second parser, reparsing its source, or borrowing an
unrelated expression range would create a false source authority.

## Deletion-driven switch

This cut directly removed both old generic non-Need Await failure branches and
the generic active-borrow failure emitted by `reject_active_borrows`. Every
current await, yield, thread, and defer consumer now reaches the structured
borrow diagnostic; no compatibility diagnostic, alias, dual reader, or source
gate was added.

`AGENTS.md` already requires this direct-deletion workflow and prohibits the
transitional mechanisms above, so no policy edit was necessary.

## Contract mapping

| Row | Evidence |
|---|---|
| A017 | a non-Need direct Await reports `sema.await.operand_not_need`, carries the actual `I32` type, and labels exactly the operand `42` |
| F007 | awaiting a `ThreadHandle<Unit>` reports the same typed diagnostic and labels the complete thread expression operand |
| A024 | an active `'asset` borrow reports `sema.suspend.borrow_across`, retains that lifetime fact, and labels exactly the five-byte `await` keyword |
| await-with parity | a non-Need operand uses the same kind and code with no invented span |

## Remaining boundary

This cut does not implement direct Ready/Err `Need<T, E>` runtime
materialization, effect-trait requirement/implementation facts, project/LSP
callable-execution publication, the codec-8 authored-function kind, or the
Stream runtime switch. Effect-trait rows A023, E014-E017, E022, and E023 need a
typed trait-method effect source and semantic callable owner; the omitted-row
diagnostic contract remains underspecified and is not guessed here.

## Validation

- `cargo test -p arcweft-lang-sema --lib tests::await_ -- --nocapture`: 24
  passed;
- `cargo test -p arcweft-lang-sema --lib`: 1,119 passed;
- `cargo test -p arcweft-lang-sema --all-features --lib`: 1,119 passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- the non-CLI workspace library and integration target completed successfully;
- `just test-workspace`: all preceding targets passed, then the known
  parent-reproducible `arcw_fixtures_check_run` target stopped on
  `010_capability_fs_read.arcw` and `002_file_read_task.arcw`. Both still
  depend on the Proof attached syntax/HIR switch publishing typed external
  capability members, and this cut did not repair the detached raw-body
  carrier;
- the recipe's remaining
  `seq04_8_4_persistent_cache_build_cli_goldens` target was run separately and
  passed 2 tests;
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: scanned 3,657
  files, 1,937 Rust files, 909,092 Rust physical LOC, and 94 manifests with 0
  errors and 146 existing warnings;
- Tier 2 was not applicable because this is an isolated sema diagnostic cut
  with no runtime, render, Agent, MCP, or capture-path change;
- package intake audit: all 26 repository ZIPs have recorded SHA-256 evidence;
  no unrecorded archive was found; and
- `cargo fmt --all -- --check` and `git diff --check`: passed.
