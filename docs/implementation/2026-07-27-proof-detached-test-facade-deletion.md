# Proof detached parser test-facade deletion

Date: 2026-07-27
Status: `IMPLEMENTED_VALIDATED_WITH_INHERITED_PROOF_GATE`
Parent Git revision: `479ac72d93c1d173a942dcf175544cb144fac172`
Jujutsu change: `suprsnvkqxom`

## Boundary

This is a deletion-driven Proof-concurrency v6.1.1 preparatory cut. The
corrected final HIR leaf package requested by Proof `01.1.1.4.1` has not
returned, so this cut does not infer expression, literal, call, Thread,
Dialogue, RichText, HIR arena, or runtime-assertion payloads.

The `docs/reviews/` inbox was rehashed at the push boundary. All 29 retained
ZIP archives have case-insensitive SHA-256 matches in package-specific
implementation intake or completion notes, and there is no unclassified
archive. TTS remains skipped.

## Deleted obsolete surfaces

The standalone detached `source::ParsedSource` remains the current production
reader until the atomic attached syntax/HIR switch. Several public or test-only
surfaces around it had no production consumer and encouraged source-free
coordinate reconstruction. This cut deletes them rather than repairing them:

- `ParsedSource::span(TextRange)`, which duplicated the owning
  `SourceDocument::span(SourceRange)` API;
- `ParsedSource::is_ok()`, used only by two parser tests instead of directly
  observing the diagnostic collection;
- public source-free `cst_lines(&SyntaxNode)`;
- `From<&SyntaxNode> for CstLineEvents<'static>` and the owned
  `CstLine::from_node` path that reconstructed every line from a detached CST
  root; and
- CLI `native/repl_snapshot.rs`, its test-only
  `AgentReplSerializedBinding`, and two tests for a parallel AST-to-snapshot
  classifier that was compiled only under `cfg(test)` and never fed the
  production REPL command bridge.

All CST line-event tests now supply the original bytes explicitly through
`cst_lines_for_source`. The first workspace check exposed two sema test callers
of the removed `ParsedSource::span`; both now request a checked `SourceSpan`
from `parsed.document()` with an explicit `SourceRange`. The deleted method was
not restored or renamed.

The production REPL binding path remains the typed result from
`arcweft-agent-repl` projected by `repl_command_bridge`. No detached AST
classifier, stringly fallback, compatibility wrapper, source reparse, source
gate, or removed-syntax diagnostic replaces the deleted test module.

The implementation status note now records that every CST line projection
requires exact source backing. `line_owned_bytes` remains in persistent parser
statistics as an established codec field and is zero on the surviving normal
path; this cut does not create a new serialized schema merely to remove a dead
producer.

## Validation

Passed on the final checkout:

- source-backed CST focused tests: 30 passed;
- View interaction syntax sample: 1 passed;
- `cargo test -p arcweft-lang-syntax --all-targets`: 492 library tests plus all
  integration and compile-fail tests passed;
- `cargo test -p arcweft-cli --lib --bins --quiet`: 196 passed;
- `cargo clippy -p arcweft-lang-syntax -p arcweft-cli --all-targets
  --all-features -- -D warnings`;
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

The first `just test-workspace` attempt stopped before tests because the local
Rust artifact cache temporarily exposed only a metadata stub for `core` and
could not resolve `std` while compiling the tooling capability-policy test.
The exact target was immediately rebuilt and passed 2/2. A complete rerun then
passed the workspace, syntax, HIR, sema, LSP, tooling, and compile-fail routes
and stopped only at the inherited CLI fixture gate. A direct rerun confirmed
exactly 3 passed and the same 2 failures:

```text
spec_should_pass_check_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw
spec_should_pass_run_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw
```

Those fixtures require attached `ExternCapabilityItem` member publication of
`FsError` and remain owned by the Proof public HIR switch. No detached reader,
global nominal fallback, or compatibility alias is introduced to hide them.

Tier 2 was not run. The cut removes unused parser conveniences and `cfg(test)`
CLI code; it does not change production runtime, render, Agent, MCP, or capture
behavior.

## Structural audit

The canonical command was:

```text
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/proof-detached-test-facade-deletion-2026-07-27
```

It scanned 3,724 files, 1,943 Rust files, 904,065 physical Rust LOC, and 95
package manifests, with 0 errors and 146 repository-wide warnings. Reports are
retained under
[`structure-audits/proof-detached-test-facade-deletion-2026-07-27/`](structure-audits/proof-detached-test-facade-deletion-2026-07-27/).

| Path | Classification | Bytes | Physical LOC | Responsibility |
|---|---|---:|---:|---|
| `crates/arcweft-lang-syntax/src/source.rs` | production | 7,924 | 247 | standalone parsed-source owner after dead convenience deletion |
| `crates/arcweft-lang-syntax/src/cst.rs` | production facade | 12,526 | 430 | CST model and exact-source line projection export |
| `crates/arcweft-lang-syntax/src/cst/line.rs` | production | 25,567 | 780 | borrowed line events, block collection, punctuation summaries |
| `crates/arcweft-lang-syntax/src/tests/cst.rs` | unit tests | 20,783 | 598 | source-backed CST behavior and accounting evidence |
| `crates/arcweft-lang-syntax/tests/view_interaction_samples.rs` | integration test | 1,592 | 54 | parser sample diagnostics |
| `crates/arcweft-lang-sema/src/tests/associated_capacity.rs` | unit tests | 60,257 | 1,674 | associated-capacity resolver/accounting matrix with exact source spans |
| `crates/arcweft-cli/src/app/agent/native.rs` | production orchestration | 15,766 | 331 | native Agent module ownership after test-only module deletion |
| `crates/arcweft-cli/src/app/agent/native/repl.rs` | production | 48,883 | 1,477 | REPL session, reporting, and typed bridge consumers |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | unit tests | 116,635 | 3,332 | native Agent/REPL behavior tests after parallel snapshot tests deletion |

The deleted `repl_snapshot.rs` was 6,899 bytes and 203 physical lines in the
parent revision. `repl.rs` and `native/tests.rs` already exceed their warning
thresholds; this cut reduces them by 7 and 29 lines respectively and adds no
responsibility. Splitting unrelated established REPL/Agent test domains is not
mixed into this deletion cut. No changed syntax production module exceeds the
1,200-line warning threshold, and the 1,674-line sema test module remains below
the 2,500-line test warning threshold.

## Remaining boundary

The surviving `parse_source`, `TypedSyntaxTree`, `typed_tree`, and
`into_typed_tree` consumers are production authority and cannot be deleted
without the corrected final HIR leaf contract and one compiling attached
syntax/HIR/project migration. They remain frozen: no new detached helper,
string reparse, linked-HIR adapter, or test-only semantic facsimile is added
while Proof `.1.1.4.1` is pending.
