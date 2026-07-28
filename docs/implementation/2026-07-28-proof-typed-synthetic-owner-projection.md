# Proof convergence: typed synthetic owner projection

Date: 2026-07-28

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

Jujutsu change audited: `lkmomyoywlynkmyuksoqupoztttznwsu`

Parent main commit: `0c8b05695e15`

## Boundary

This cut adds only the final typed owner projection released by the accepted
portion of Proof 01.1.1.4.1.1:

```text
SyntheticOwner
  = Item | Scope | Local | Expr | Stmt | Type | Pattern | Capture
```

Each variant contains the corresponding non-forgeable typed HIR ID.
`SyntheticOwner::kind()` projects the variant-owned `HirIdKind`, and
`SyntheticOwner::module()` projects the contained database-qualified module.
Neither method probes an arena, reads a raw-kind field, parses a string, or
exposes the numeric HIR slot.

## Deletion-driven boundary

The obsolete `SyntheticKey { owner: RawHirId, ... }` was deleted in the
preceding identity cut and is not restored. This cut does not add a raw-owner
conversion, Syntax owner, compatibility wrapper, or provisional key.

The deeper predecessor audit found that Proof 01.1.1.4.1.1 does not completely
define inherited role-owner/ordinal admission or stable fingerprint bytes.
Therefore this cut deliberately does not add:

- `SyntheticKey`;
- `SyntheticKey::try_new` or `SyntheticRole::accepts_owner`;
- a fingerprint/digest/transcript API or a new hashing dependency;
- `HirElidedRegion`;
- a source index or `HirModule::source_site`;
- liveness, rollback, or synthetic-descendant allocation; or
- any consumer migration.

The first return to the independently throwable
[Proof 01.1.1.4.1.1.1 request](../reviews/requests/2026-07-28-seq-proof-01.1.1.4.1.1.1-synthetic-role-owner-admission-correction.md)
remains rejected by its
[return intake](2026-07-28-proof-01-1-1-4-1-1-1-synthetic-role-admission-intake.md).
The later
[Proof 01.1.1.4.1.1.1.1 correction](2026-07-28-proof-01-1-1-4-1-1-1-1-tail-owner-generator-intake.md)
is accepted and releases the final typed key identity slice. Tail/generator
transaction consumers remain ordered after the final HIR arena owner rather
than being backported into provisional HIR.
The private `RawHirId` remains only as the backing storage of typed HIR IDs; it
is not a synthetic owner and remains inaccessible outside `identity.rs`.

## Direct evidence

Unit tests construct all eight variants inside the identity owner and prove:

- exact `kind()` and database-qualified `module()` projection;
- the complete structural `Clone`, `Copy`, `Debug`, `Eq`, `Hash`, `Ord`,
  `PartialEq`, and `PartialOrd` trait contract; and
- owner-variant identity remains distinct even when test-only raw slot numbers
  are equal.

Public compile-fail evidence proves both serialization directions remain
unavailable for `SyntheticOwner`. It does not freeze `SyntheticKey` as absent,
because that final name must be added after the correction return.

## Validation

Completed:

- `cargo fmt --all -- --check`: passed;
- `cargo test -p arcweft-lang-hir --all-features --lib identity::tests --
  --nocapture`: passed, including the typed-owner row;
- `cargo test -p arcweft-lang-hir --test public_api --all-features --
  --nocapture`: passed, including all 13 compile-fail rows;
- `cargo test -p arcweft-lang-hir --all-features`: passed, including 87 unit
  tests and every HIR integration/compile-fail/doc-test target;
- `cargo clippy -p arcweft-lang-hir --all-targets --all-features -- -D
  warnings`: passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `just test-workspace`: components 1 through 7 passed, including the new HIR
  unit/trybuild evidence; component 8 stopped after `1,053.3` seconds at the
  two pre-existing capability-owned fixture mismatches:
  - `tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw`;
  - `tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw`;
- exact `arcw_fixtures_check_run` reproduction: 3 passed and those same 2
  fixtures failed;
- the component 9 target skipped by that stop,
  `seq04_8_4_persistent_cache_build_cli_goldens`, passed separately with 2
  tests; and
- `git diff --check`: passed.

The fixture failures predate and do not exercise the typed owner projection.
This cut changes no runtime, render, Agent, MCP, capture, persistence, codec,
or artifact path, so Tier 2 is not applicable.

## Structural measurement

The changed production file
`crates/arcweft-lang-hir/src/identity.rs` is `19,295` bytes and `634` physical
lines. It remains one cohesive identity vocabulary module below the production
warning threshold and grew by fewer than 300 lines in this cut. No manifest,
dependency edge, feature, opcode, runtime payload, or persisted format changed.

The canonical structural audit scanned `3,808` files, `1,965` Rust files,
`906,235` physical Rust LOC, and `95` manifests with zero errors and `146`
pre-existing warnings.
