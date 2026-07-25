# Proof-concurrency v6.1.1 exact syntax transaction lease

## Status

`IMPLEMENTED_VALIDATED_WITH_INHERITED_PROOF_GATE`

This private-preparatory cut moves the incremental syntax transaction to the
exact ownership shape required by Proof-concurrency v6.1.1 without publishing
attached typed handles beside the detached production parser.

## Package decision

The retained Proof-concurrency v6.1.1 package remains
`READY_FOR_IMPLEMENTATION`. Its syntax transaction contract requires a
fallible database constructor, one caller-owned `Arc<SourceDocument>`, and a
cheap-clone `ParsedSource` value. The current `AGENTS.md` already requires
root-cause deletion, no compatibility aliases or dual readers, and no source
gates, so no repository policy edit is needed.

The returned AW-AH-009.3.3.3.1 capacity package remains usable under the
previously recorded precedence decision: bare `Vec.with_capacity` remains a
type-argument arity failure under the package's `.3.3.4` authority. TTS
production remains skipped.

## Deleted provisional ownership

The previous incremental implementation had four provisional boundaries:

- `SyntaxDatabase::default()` silently converted database-ID exhaustion into
  an optional internal field;
- the transaction stored `Option<SyntaxDatabaseId>` and reported absence later
  as a generic invariant failure;
- parse and fragment entrypoints accepted owned or borrowed `SourceDocument`
  values and attachment cloned them into a new `Arc`; and
- `Arc<ParsedSource>` added a second lease around a value that already owns one
  immutable bound snapshot.

This cut deletes those shapes. `SyntaxDatabase::try_new` now returns
`SyntaxDatabaseCreateError::IdentityExhausted`, the transaction owns a checked
database identity, and the caller's exact `Arc<SourceDocument>` is shared by
the grammar attachment and returned `ParsedSource`. `ParsedSource` itself is a
cheap-clone value and exposes `is_same_snapshot` for exact immutable snapshot
comparison.

The old `ShadowDatabaseState`, `ShadowLineageState`, `ShadowFault`, and
corresponding field/function names were also deleted. After the incremental
dual parse was removed in the preceding cut, these objects are the canonical
syntax transaction and lineage state rather than a shadow authority. The
private grammar parser names remain temporary until the Stage 3 atomic public
switch deletes the standalone parser.

## Direct evidence

The database test verifies pointer equality between the caller's
`Arc<SourceDocument>` and the exact document retained by `BoundParsedSource`.
No-op edit transactions verify `ParsedSource::is_same_snapshot`; attachment
and reconciliation tests continue to verify rollback, independent database
identity, exact spans, and never-reused node IDs.

Completed focused validation:

```text
cargo check -p arcweft-lang-syntax --all-targets
  PASS
cargo test -p arcweft-lang-syntax --lib incremental::database::tests -- --nocapture
  PASS: 36 passed, 0 failed
cargo test -p arcweft-lang-syntax expr::dialogue_application::surface::tests --lib -- --nocapture
  PASS: 2 passed, 0 failed
cargo test -p arcweft-lang-syntax --all-targets
  PASS: 473 unit tests and all integration/compile-fail tests
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
  PASS
cargo check --workspace --all-targets --all-features
  PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings
  PASS
cargo fmt --all -- --check
  PASS
git diff --check
  PASS
```

The first `just test-workspace` attempt stopped after 434.1 seconds while
Windows failed to mmap an `arcweft_bundle` rlib with `os error 1455` (paging
file exhaustion). No cargo or rustc process remained, and the machine reported
approximately 48 GiB free physical and 51 GiB free virtual memory after the
failure. The same recipe was rerun with `CARGO_BUILD_JOBS=4`; this changes only
build parallelism, not the feature set or test scope.

The low-parallelism rerun reached the inherited Proof migration gate after
807.5 seconds. All preceding workspace suites and compile-fail matrices passed.
The only test failures were:

```text
spec_should_pass_check_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw
spec_should_pass_run_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw
```

An exact focused rerun of
`cargo test -p arcweft-cli --test arcw_fixtures_check_run -- --nocapture`
confirmed 3 passed and only those 2 failed. They are unchanged from the
pre-existing Stage 3 public-source gate. This cut does not repair the detached
reader or add a compatibility path to make them pass.

Tier 2 is not applicable. This cut changes no runtime, render, Agent, MCP,
capture, or corresponding transport behavior.

## Structural audit

The audit used parent Git revision `5cf8f193b603cdb396e2a22aaefdf3e1e5f4ee5d`
and working change `wptxkwym`.

```text
cargo +nightly -Zscript tools/structure-audit.rs --root .
files scanned: 3669
Rust files: 1936
Rust physical LOC: 906962
package manifests: 94
violations: 0 error(s), 146 warning(s)
```

Reports are retained under
`docs/implementation/structure-audits/proof-stage3-exact-syntax-transaction-lease-2026-07-25/`.

| Path | Bytes | Physical LOC | Classification | Responsibility |
|---|---:|---:|---|---|
| `crates/arcweft-lang-syntax/src/expr/dialogue_application/surface.rs` | 27,429 | 777 | production with embedded tests | exact document lease in syntax-ID test fixture |
| `crates/arcweft-lang-syntax/src/incremental/database.rs` | 16,346 | 507 | production | fallible database creation and accepted snapshot ownership |
| `crates/arcweft-lang-syntax/src/incremental/database_tests.rs` | 61,552 | 1,702 | unit test | exact lease, rollback, reconciliation, fragment, and limit evidence |
| `crates/arcweft-lang-syntax/src/incremental/transaction.rs` | 9,123 | 284 | production | canonical identity/attachment transaction over caller-owned documents |
| `crates/arcweft-lang-syntax/src/incremental.rs` | 326 | 14 | production facade | deliberate transaction API exports |

No changed production file exceeds the 1,200-LOC warning threshold. The test
module remains below the 2,500-LOC warning threshold. No dependency, feature,
crate boundary, compatibility alias, or parallel reader was added.

## Remaining Stage 3 boundary

This cut does not publish `AstNode`, `SyntaxNodeHandle`, attached diagnostics,
or final fragment types, and it does not claim the Stage 3 public switch. The
standalone `source::ParsedSource`, detached `TypedSyntaxTree`, and their
compiler/LSP/tooling consumers remain one production authority until the
workspace-wide attached switch can delete them atomically.

The next independently coherent deletion is the redundant public
`parser::parse_document` alias, which has no external production caller. The
larger Stage 3 switch must then delete old source/fragment readers first in its
working change and migrate compiler, project-loader, CLI, LSP, tooling, Agent,
and source-backed HIR keys to the final attached owner without a compatibility
facade.
