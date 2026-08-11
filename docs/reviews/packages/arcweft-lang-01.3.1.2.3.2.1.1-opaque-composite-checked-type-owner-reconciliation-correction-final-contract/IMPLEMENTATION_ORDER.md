# Corrected A1 implementation and continuation order

Every subgate is a coherent compile-clean cut. No compatibility-only state may
be committed between subgates.

## A1.0 — repin and stale-allocation reconciliation

- checkout exact Git commit `a38c736ba577172b1f4c3fe1a0c3e85443e97e6f`;
- confirm root/scoped `AGENTS.md`;
- record that current AWBC is ABI 1 / codec 10 and session save is schema 2;
- apply the retained parent nominal layout contract as baseline;
- do not edit production merely to preserve the parent's stale codec-8 text.

Exit: exact source inventory and version constants pinned.

## A1.1 — core owner, value, and native acceptance

Owners: `arcweft-core` pattern/value/canonical encoding.

1. Add producer ID, admission, owner, opaque value, and errors.
2. Extend `RuntimeCheckedType` and `RuntimeValue` in place.
3. Complete parent nominal layout field and inherent `accepts_value` migration.
4. Add `variant_case`; delete `accepts_variant_case` and free type matcher.
5. Traverse opaque payload in nesting/canonical validation.
6. Allocate canonical runtime value tag 16.
7. Update every exhaustive core match and core Serde test.

Exit tests: owner relation, exact/wide construction, native acceptance,
composites, depth, canonical bytes, nominal parent tests.

## A1.2 — producers, semantic projection, and variant API

Owners: `arcweft-lang-sema`, `arcweft-dialogue`, `arcweft-runtime-plan`, and
compiler runtime semantic projection.

1. Make opaque accepted semantics producer-bearing and update constructors.
2. Republish runtime-facing standard Named atoms as accepted opaque rows.
3. Retain producer in `AcceptedNominalType` and accepted Rust descriptor path.
4. Add CharacterDialogue inherent producer/owner/encode/decode APIs.
5. Replace `RuntimeTypeShape::Named`/bare Opaque and return typed projection
   errors/path.
6. Add `RuntimeCheckedVariantSelection` and migrate expression/pattern lowerers.
7. Fix compiler project nominal schema digest/layout projection per parent.
8. Close entry roles for the two required Result signatures.
9. Delete producerless/name/digest/selected-case fallback paths.

Exit tests: producer catalog, Named failure, generic Reduction atomic leaf,
CharacterDialogue exact/any, entry roles, variant selection, compiler entry
suite.

## A1.3 — AWBC codec 11 and native parity

Owners: core AWBC schema/codec/verifier/fiber/VM and runtime-plan AWBC lowerers.

1. Add runtime type tag 23 and constant tag 18.
2. Bump codec 10 to 11; retain ABI 1.
3. Intern complete opaque/composite types.
4. Migrate `MakeVariant` and pattern lowering to checked selections.
5. Add exact/wide compatibility at all register/call/return/merge boundaries.
6. Materialize opaque constants only through exact rows.
7. Add VM acceptance through core owner methods.
8. Replace all codec goldens and reject codec 10.

Exit tests: canonical bytes/tamper, structural verifier, VM, Result both cases,
patterns, branch merge, calls/returns, native/AWBC parity.

## A1.4 — persistence, deletion, and full closure

Owners: bundle/save/runtime driver/host consumers plus all affected crates.

1. Bump session save 2 to 3 and migrate snapshot/fiber/capture Serde.
2. Keep outer bundle ABI-1 product key; update exact inner AWBC bytes/digests.
3. Add producer decode validation at domain reification/restore boundaries.
4. Delete save-2/codec-10 readers, fixtures, aliases, and old enum branches.
5. Audit all producer/consumer/deletion rows.
6. Run required focused suites and full workspace commands.

Exit: no old reader/writer, no producerless Opaque/Named runtime success, no free
matcher, no selected-case owner, no compatibility layer.

## Commands at every subgate

```text
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Run matrix-selected focused tests before each workspace check. At A1.4 also run
the repository's current checked-in fast verification and structure audit as
required by the exact commit's instructions.

## Continuation

Parent A2 through its final gate remain in their accepted order and may begin
only after A1.4 is green. A1 is therefore one acceptance group composed of four
named compile-clean implementation subgates, not one unreviewable source cut.
Rollback is by whole subgate; no partial format or dual reader remains live.
