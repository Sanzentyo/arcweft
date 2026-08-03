# Proof transactional final-HIR and Thread-control private cut

Date: 2026-08-03

Status: `PRIVATE_SUBSTRATE_VALIDATED_PUBLIC_SWITCH_PENDING`

Inspected baseline: `8f39b30846ff3c45c278fdb58e9640a65a87a3e1`, which was
both local `main` and `origin/main` before this cut.

The main checkout contained a protected dirty Proof integration working set.
The exact `208` code paths in this cut were copied into a detached validation
worktree at the same baseline. All `208` files matched the main checkout
byte-for-byte by SHA-256 after validation. Existing documentation changes and
the six public-consumer paths listed below were not admitted to this cut.

## Selected contract and precedence

This cut continues the revision-bound attached-syntax substrate and implements
the repository-adjudicated portion of Proof-concurrency
`v6.1.1.2.1.1.1`. The returned ordinary-Flow archive is retained with
SHA-256
`BDC55671E7D4F8CDB3D07D8EC004672C90E14DEA88A47E63D8189E585BB3E4DF`.
The repository intake remains authoritative where the archive conflicts with
accepted shared owners, limits, or current grammar.

## Scope released by this cut

This cut establishes the private final-HIR transaction boundary needed before
the public compiler/project switch:

- qualified arena IDs, paged allocation, staged reservations, exact commit or
  rollback, immutable module snapshots, and module-preserving project freeze;
- typed final-HIR payloads for items, expressions, patterns, statements,
  types, scopes, locals, source sites, recovery, diagnostics, capture, and
  poison state;
- source-index construction from the revision-bound attached syntax graph,
  without source-string reparsing or a detached semantic reader;
- one shared ordered `HirThreadBody` owner for Flow bodies and Thread
  expressions, with source, scope, local, limit, and recovery accounting;
- exact private lowering for Loop, While, WhileLet, For, Select, and AwaitWith
  control families, including branch/body scopes and recovered children;
- source-backed For synthetic iterator and next-value roles frozen against the
  statement owner;
- Select branch-head bindings visible at their accepted source position;
- root Flow/Thread scope and poison rederived from the attached owner rather
  than copied from provisional state; and
- arbitrary-precision numeric payload support through the HIR-owned
  `num-bigint` dependency, with the LSP dependency-direction expectation
  updated in the same dependency cut.

The duplicated Thread-control generation seeding path was deleted. Pattern and
Select binding validation are the sole generation authority; no compatibility
counter, alias, or second accounting route remains.

## Deliberately excluded public switch

This cut does not make final HIR the production compiler, semantic, LSP, or
runtime authority. The following protected public-consumer changes remain
outside this commit:

- `crates/arcweft-compiler/src/tests.rs`;
- `crates/arcweft-lang-sema/src/checker/expr.rs`;
- `crates/arcweft-lang-sema/src/checker/helpers.rs`;
- `crates/arcweft-lang-sema/src/tests/function_stack.rs`;
- `crates/arcweft-lang-sema/src/tests/line_plan.rs`; and
- `crates/arcweft-lang-sema/src/tests/typecheck.rs`.

The cut therefore does not claim that all Flow/Thread variants are reachable
through the public compiler. It introduces no adapter from final HIR back to
old HIR, dual reader, source gate, compatibility spelling, CSS path, Takumi
path, or removed-syntax-only final diagnostic. The frozen old production route
is not repaired. Its readers are removed first in the later public authority
switch, and the resulting compiler errors are the consumer inventory.

## Validation evidence

Passed in the detached exact-cut worktree:

- `cargo fmt --all -- --check`;
- full `arcweft-lang-syntax` tests with all features;
- `cargo test -p arcweft-lang-hir --all-features`, including unit,
  integration, compile-fail UI, and documentation tests;
- the focused final-HIR Thread-control statement suite (`27` passed);
- the focused module-resolution suite (`8` passed);
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features`;
- `cargo test -p arcweft-lsp --test dependency_direction`;
- the non-CLI workspace library and integration-test command used by
  `just test-workspace`;
- `cargo test -p arcweft-cli --lib --bins --quiet` (`196` passed);
- CLI runtime-native options (`3` passed), core check (`4` passed), native
  style parity (`1` passed), release trust JSON (`5` passed), responsive stage
  placement (`4` passed), and persistent-cache build goldens (`2` passed);
  and
- `git diff --check` for the exact cut.

`just test-workspace` is not claimed as fully passing. Its exact Arcweft
fixture command produced `3` passes and the same two previously recorded
failures:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both fail at the known public-switch frontier with
`sema.nominal.unknown_type` for the capability-owned `FsError`. This cut does
not fabricate a global type, restore an obsolete reader, or bypass the
fixtures. All remaining CLI commands in the workspace recipe passed when run
individually.

One Rust 1.96.0 incremental compilation attempt encountered an internal
compiler error in cached type-dependent-definition state. The affected bundle
and LSP targets compiled successfully with `CARGO_INCREMENTAL=0`, and all broad
validation above used that setting afterward. A later workspace run exhausted
the target volume after completing the non-CLI phase; scoped `cargo clean` for
the five involved packages reclaimed 50.6 GiB, after which the remaining CLI
commands passed as recorded above.

Tier 2 was not run for this cut. The new authority remains private and does not
change a production runtime, renderer, Agent, MCP, or capture path, so the
repository Tier 2 trigger is not applicable. The immediately preceding
attached-syntax cut ran and passed Tier 2.

## Structural audit

The canonical command

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

scanned `4,111` files, `2,244` Rust files, `1,101,837` physical Rust lines,
and `95` manifests. It reported `0` errors and `176` warnings. The warnings
remain decomposition and private-substrate follow-ups; no suppression or
compatibility facade was added to hide them.

## Next authority boundary

The next coherent cut is the deletion-driven public final-HIR/project switch:

1. remove the obsolete lowering, linked-HIR, detached Flow/Choice, legacy
   SpeakerLine/ContentCall/HirDialogue, and capability readers;
2. migrate compiler, project, sema, verifier, runtime-plan, formatter, LSP,
   CLI, Agent, cache, and persistence consumers exposed by compilation;
3. publish one accepted `Arc<HirProject>` generation across those consumers;
4. close the two `FsError` fixtures through the typed capability owner; and
5. validate the public switch as its own workspace-compiling, Tier 2-eligible
   commit.

No compatibility layer from the old authority is admitted between this
private cut and that switch.
