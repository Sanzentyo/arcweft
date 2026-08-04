# Proof final-HIR database-owned lowering entry

Date: 2026-08-03

Inspected Git commit:
`be9ef9588fa885ece3a5bed7460ef64ef5d93374` (`main` and `origin/main`)

Initial worktree state: clean detached Git worktree. Validation state recorded
below is for the three-file dirty cut described here.

Status: `PRIVATE_PREREQUISITE_VALIDATED_WITH_WORKSPACE_TEST_BLOCKED`

## Implemented boundary

`HirDatabase::lower_attached_source` is now the sole database-owned internal
entry that composes the existing final-HIR transaction:

```text
LoweringRequest over exact incremental ParsedSource
  -> private staged module transaction
  -> attached source-file lowering
  -> immutable module validation and publication
  -> HirLowerOutput with the exact accepted Arc<HirModule>
```

The method remains `pub(crate)`. It exposes no second production reader and
does not change compiler, semantic, runtime, verifier, LSP, CLI, or Agent
authority. The lower-level staging API remains crate-private for invariant and
rollback tests; external consumers cannot publish a partially lowered module.

The focused test proves that the owner method lowers all source-file items,
publishes the exact current `Arc<HirModule>`, and advances the same module by
one revision for a same-lineage edit.

## Explicit non-goals

- No final-HIR module or project API was made public.
- No old detached parser, clone HIR, linked project, or consumer was adapted.
- No compatibility alias, wrapper, dual reader, source reparse, source gate,
  removed-syntax diagnostic, CSS path, or Takumi path was added.
- The final public authority switch remains atomic across project symbols,
  sema, verifier/runtime-plan, compiler/cache, LSP, CLI/tooling, and Agent.
- Tier 2 was not run because this private unconsumed HIR entry changes no
  runtime, render, Agent, MCP, or capture path.

## Validation performed

Passed:

- `cargo fmt --all -- --check`.
- `cargo test -p arcweft-lang-hir --lib --all-features database_owned_entry_lowers_the_complete_attached_source_atomically`:
  1 passed.
- `cargo test -p arcweft-lang-hir --all-features`: passed; 749 library tests
  plus the crate integration, trybuild, and doc-test suites completed.
- `cargo check --workspace --all-targets --all-features`: passed after the
  ignored `web/assets/noto-sans-jp-vf.ttf` fixture was copied from the primary
  checkout into the isolated worktree with matching SHA-256. The first attempt
  failed only because that ignored fixture was absent.
- `cargo clippy --workspace --all-targets --all-features`: passed with the
  repository's existing warnings.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: 4,113 files,
  2,244 Rust files, 1,101,765 Rust physical LOC, 95 package manifests, 0 errors,
  and 176 warnings; dry-run wrote no report files.

Blocked:

- `just test-workspace` did not complete. The first run reached unrelated Agent
  test targets and several concurrent `rustc` processes exited with Windows
  `STATUS_STACK_BUFFER_OVERRUN (0xc0000409)`. A second run with
  `CARGO_BUILD_JOBS=2` avoided that crash but exceeded the 20-minute execution
  limit. Neither run produced a failing Arcweft test assertion. This command is
  not recorded as passed and must be rerun at the final public switch gate.

## Remaining boundary

The next public cut must delete the detached whole-source parser, clone/linked
HIR project, and every old consumer while moving all layers to one compiler
session-owned syntax database, HIR database, and accepted `Arc<HirProject>`.
The final ExternCapability associated-type publication (including capability-
owned `FsError`), project Dialogue line inventory, semantic Unit classification,
and checked final-ID facts are prerequisites within that unmerged atomic
switch; the old carriers remain frozen until their direct replacement.
