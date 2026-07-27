# Proof convergence: database-qualified HIR identity

Date: 2026-07-28

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

Jujutsu change audited: `suuswyywvpkkokqstxqlmpptkmzyqnyt`

Parent main commit: `afc68284ea1f`

## Boundary

This cut replaces the provisional module-local HIR identity representation
with the accepted database-qualified identity vocabulary:

- `HirDatabaseId` is a private-field nonzero `u64` session identity;
- `HirModuleId` contains the database identity and a private nonzero module
  slot;
- private `RawHirId` records module, globally unique module slot, and
  `HirIdKind`;
- every typed HIR ID exposes only its module and wrapper-owned fixed kind;
- `RawHirIdView` exposes module and kind for structured diagnostics but has no
  numeric-slot accessor; and
- `IdResolveError` now carries the exact `RawHirIdView`, full
  `HirSnapshotId`, birth revision, retirement revision, and expected/actual
  kind required by the accepted Proof contract.

Equal internal module and node slot ordinals in independent HIR databases are
therefore unequal identities. No raw constructor, numeric conversion, parser,
text codec, or Serde implementation was added.

## Deletion-driven migration

The old `SyntheticKey { owner: RawHirId, role, ordinal }` was deleted. It had
no production consumer, exposed an untyped raw owner, and conflicted with the
then-pending exact Type-owned `SyntheticOwner` decision in Proof
01.1.1.4.1.1. No alias, wrapper, provisional `SyntheticOwner`, or raw-owner
replacement was introduced. The subsequently accepted
[01.1.1.4.1.1 correction](2026-07-28-proof-01-1-1-4-1-1-source-owner-consistency-intake.md)
confirms that deletion and now authorizes the final typed owner in the next
coherent cut.

There is intentionally no compile-fail test freezing the public spelling
`SyntheticKey` as absent: the accepted final contract retains that name for a
future correctly typed owner. This cut removes the obsolete representation
without preventing the final type from returning.

`SyntheticRole`, `LocalGeneration`, all eight typed HIR ID families, and
`HirLimit` remain because the accepted correction retains and extends their
final vocabulary.

## Direct evidence

Unit tests prove that:

- database identity participates in typed ID equality and ordering;
- the full database-qualified module survives through `HirSnapshotId`;
- typed wrappers report their fixed kind independently of a corruption-hook
  raw kind; and
- direct payload-shape rows prove that every final `IdResolveError` variant
  retains the exact typed identity, snapshot, revision, and kind fields. The
  first real resolver consumer will separately prove construction order and
  behavior.

Trybuild rows prove that downstream code cannot:

- construct `HirDatabaseId` from a raw number;
- initialize the private database/module-slot fields of `HirModuleId`;
- read the numeric slot from `RawHirIdView`; or
- serialize `ExprId`, `HirDatabaseId`, `HirModuleId`, `RawHirIdView`,
  `LocalGeneration`, or `SourceGeneration`; or
- deserialize `HirSnapshotId` or `SourceSnapshotId`.

These are Rust type/API tests, not source gates.

## Explicit non-goals

This cut does not add `HirDatabase`, its process-local atomic allocator,
database create/snapshot errors, an arena, a slot ledger, liveness resolution,
or a public lowering reader. Current main has no final production consumer for
those owners. Placing them privately now would require dead-code suppression;
publishing staging APIs would create a provisional public boundary.

The slot/liveness kernel will be connected in the same compiling cut as its
first final arena/lowering consumer. That cut must own the shared lifetime
ledger, immutable snapshot high-water mark, staged atomic commit, rollback,
stale proposal rejection, exact/one-over limits, and
`WrongModule -> NotYetLive -> Retired -> KindMismatch` resolution order.

The accepted Proof 01.1.1.4.1.1 correction now specifies Pattern/Type source
ownership, pathless variant payloads, Duration comparison, checker
overflow/budget ownership, and the Type-owned synthetic owner. This identity
cut deliberately does not pull those later responsibilities backward. Its
immediate successor adds only final typed `SyntheticOwner` / `SyntheticKey`;
the source index, elided-region payload, arenas, and consumers remain separate
coherent cuts.

## Validation

Completed:

- `cargo fmt --all -- --check`: passed;
- `cargo check -p arcweft-lang-hir --all-targets --all-features`: passed;
- `cargo test -p arcweft-lang-hir --all-features --lib identity::tests --
  --nocapture`: passed, including all 3 identity rows;
- `cargo test -p arcweft-lang-hir --all-features`: passed, including 86 unit
  tests, every integration target, 13 public API compile-fail rows, and
  doc-tests; and
- `cargo clippy -p arcweft-lang-hir --all-targets --all-features -- -D
  warnings`: passed.

Reviewable-cut workspace validation:

- `cargo check --workspace --all-targets --all-features`: passed in `55.307`
  seconds;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed in `57.103` seconds;
- `just test-workspace`: components 1 through 7 passed; component 8 reached
  the two pre-existing capability-owned `FsError` fixture mismatches and
  stopped after `990.4` seconds:
  - `tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw`;
  - `tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw`;
- the component 9 target skipped by that stop,
  `cargo test -p arcweft-cli --test
  seq04_8_4_persistent_cache_build_cli_goldens --quiet`, passed separately
  with 2 tests; and
- `git diff --check`: passed.

The fixture failures predate and do not exercise this process-local identity
shape. This cut does not touch runtime, render, Agent, MCP, capture,
persistence, or a codec, so Tier 2 is not applicable.

## Structural measurement

The changed production file
`crates/arcweft-lang-hir/src/identity.rs` is 15,282 bytes and 513 physical
lines. It remains one cohesive identity/error vocabulary module, below the
repository's production warning threshold. No dependency edge, manifest,
feature, runtime payload, opcode, or persisted format changed.

The canonical structural audit scanned `3,806` files, `1,965` Rust files,
`906,111` physical Rust LOC, and `95` manifests with zero errors and `146`
pre-existing warnings.
