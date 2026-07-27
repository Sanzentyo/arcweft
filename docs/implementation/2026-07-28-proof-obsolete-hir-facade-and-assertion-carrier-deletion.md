# Proof convergence: obsolete HIR facade and assertion carrier deletion

Date: 2026-07-28

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

Jujutsu change audited: `pnplworzmysvpvznnrzzumqyksvwowxx`

## Boundary

This deletion-driven cut removes two provisional public HIR boundaries that
have no final ownership role:

- `arcweft_compiler::hir::lower_source_document`, a one-to-one forwarding
  wrapper over `arcweft_lang_hir::lower::lower_document_to_hir`; and
- the standalone `arcweft_lang_hir::assertion::HirAssertion` carrier and its
  public module.

The compiler project pipeline, persistent-fact tests, and CLI Agent RAG source
index now call the existing HIR lowering owner directly. No replacement helper,
alias, wrapper, extension trait, or compatibility module was introduced.

`HirAssertion` had no constructor and no consumer anywhere in the workspace.
It stored detached `TextRange` values beside `ExprId` conditions, while the
accepted Proof contract assigns assertions to the final statement arena as
`HirStmtKind::Assertion { mode, conditions }`; revision-bound component spans
belong to the HIR source index. Keeping the standalone carrier would create a
second assertion authority during the arena switch, so it was deleted rather
than completed.

## Direct evidence

Two dedicated trybuild rows prove that downstream code can no longer import:

- `arcweft_compiler::hir::lower_source_document`; or
- `arcweft_lang_hir::assertion::HirAssertion`.

These are Rust API/type checks. They do not inspect repository source text and
are not source gates.

The complete HIR and compiler suites exercise every migrated direct caller,
including persistent HIR/interface facts and project compilation. The focused
CLI Agent RAG tests exercise the affected Agent module after its compiler HIR
import was removed.

## Contract boundary

This cut does not claim that `lower_document_to_hir` or the borrowed
`ParsedSource::typed_tree()` reader is final. Both remain frozen production
authorities until the attached syntax and arena HIR public switch can replace
all consumers in one compiling cut. This change does not repair, extend, or
wrap either old reader.

The Proof 01.1.1.4.1 READY-claim package remains only partially
implementation-ready pending
[`01.1.1.4.1.1`](../reviews/requests/2026-07-27-seq-proof-01.1.1.4.1.1-source-owner-and-semantic-consistency-correction.md).
No PatternId/TypeId source-query, pathless variant, Duration comparison,
checker-overflow, elided-region owner, or unresolved byte/segment budget is
inferred here.

Runtime assertion identity cannot be connected honestly in this cut. Current
runtime lowering still reads syntax `AssertionStmt` and emits a guardless core
assertion; exact `StmtId`, condition `ExprId`, source-span inventory, and
runtime-plan artifact fingerprint depend on the final arena/project switch.
The checked guard/fingerprint primitives remain private preparation, and no
fake guard or dual AWBC reader is added.

## Validation

Completed:

- `cargo fmt --all -- --check`: passed;
- `cargo check -p arcweft-compiler -p arcweft-lang-hir -p arcweft-cli
  --all-targets --all-features`: passed after direct caller migration;
- `cargo test -p arcweft-lang-hir --all-features`: passed, including 85 unit
  tests and every integration, compile-fail, and doc-test target;
- `cargo test -p arcweft-compiler --all-features`: passed, including 92 unit
  tests and every integration, compile-fail, and doc-test target;
- `cargo test -p arcweft-cli --all-features --lib agent_rag -- --nocapture`:
  passed, 2 focused Agent RAG tests;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed after deleting one remaining unused parent-module `hir` import found
  by the first strict-Clippy run;
- `just test-tier2`: passed in 362.2 seconds, including the 22-test MCP/native
  suite, capture, animated-image, text-combine, ruby, visual-smoke, and all
  four IMQ golden rows; and
- `git diff --check`: passed.

`just test-workspace` ran for 965 seconds. Recipe components 1 through 7
passed. Component 8 stopped at the existing fixture baseline; an exact focused
rerun reported 3 passed and 2 failed:

- `spec_should_pass_check_fixtures_pass_after_refactor` still rejects
  `tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw`;
- `spec_should_pass_run_fixtures_pass_after_refactor` still rejects
  `tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw`; and
- `current_check_fixtures_pass`, `current_run_fixtures_pass`, and
  `spec_should_fail_fixtures_fail` passed.

Both failures are the recorded capability-owned `FsError` fixture baseline for
the later attached-HIR publication; this deletion cut does not change that
surface or add a workaround. Because `just` stopped at component 8, only the
final `seq04_8_4_persistent_cache_build_cli_goldens` component was not run by
that invocation. It was then run directly and passed, 2 tests.

The final review-package ledger contains 30 retained ZIPs, zero unrecorded
SHA-256 values, and zero ZIPs in the `docs/reviews/` root inbox. No returned
Proof 01.1.1.4.1.1 correction archive exists.

## Structural audit

The canonical dry-run scanned 3,802 files, including 1,963 Rust files and
905,919 physical Rust LOC across 95 manifests. It reported zero errors and 146
existing warnings. The review-package ledger was checked in the same audit
pass.

Current changed production-owner measurements are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-cli/src/app/agent.rs` | 22,739 | 703 | Agent command facade/import owner |
| `arcweft-cli/src/app/agent/rag.rs` | 48,907 | 1,353 | Agent RAG orchestration |
| `arcweft-cli/src/app/agent/rag/source_index.rs` | 67,838 | 1,841 | source-backed Agent RAG index |
| `arcweft-compiler/src/hir.rs` | 1,304 | 30 | semantic HIR validation/typecheck facade |
| `arcweft-compiler/src/persistent.rs` | 57,569 | 1,501 | deterministic compiler fact codecs and tests |
| `arcweft-compiler/src/project.rs` | 36,655 | 1,092 | project compiler orchestration |
| `arcweft-lang-hir/src/lib.rs` | 602 | 26 | HIR crate facade |

The deleted standalone assertion owner was 1,146 bytes and 42 physical LOC.
The three warning-level files above 1,200 LOC predate this cut; the change only
replaces imports/call targets and adds no responsibility. No dependency edge,
manifest, feature, opcode, serialized format, or runtime payload changed.

## Next boundary

After this independently validated deletion is pushed, implement the private
database-qualified HIR identity and slot/liveness kernel. Do not publish
`HirDatabase::lower`, source-component queries, Pattern/Type arenas, or the
final public reader until the narrow 01.1.1.4.1.1 correction is accepted.
