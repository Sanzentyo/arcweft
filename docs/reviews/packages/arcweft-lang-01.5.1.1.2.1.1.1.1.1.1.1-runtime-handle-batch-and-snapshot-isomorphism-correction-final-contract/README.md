# Lang-01.5.1.1.2.1.1.1.1.1.1.1 — runtime handle, batch, and snapshot isomorphism correction

**Status:** `READY_FOR_IMPLEMENTATION`  
**Package kind:** design-only, independently throwable final contract  
**Repository basis:** `Sanzentyo/arcweft`  
**Observed latest `origin/main`:** `f42157fc4c8ca890eeaacec6dde3bb3e3af5d009`  
**Production source reconciled by the request/intake:** `3670625a02b9e7e8578b57fc7b148a1758a17dba`  
**Superseded parent archive SHA-256:** `034A2EEAB2D083B5BB4496F4EE63040B2F93B30ABDDA1B18E93138E28B65391B`  
**Open questions:** `0`

This package closes the twelve blockers in the mandatory correction without
reopening the retained substrate. It does not contain a production patch. The
normative implementation target is split across Rust-shaped schemas, exact
state/transaction tables, machine-readable inventories, and a read-only
validator with negative self-tests.

## Start here

1. `FINAL_CONTRACT.md` — the normative end-to-end decision.
2. `RUST_LIVE_SCHEMAS.md` and `schemas/final_contract.rs` — concrete owners,
   constructors, envelopes and projections.
3. `NEED_HANDLE_AND_AWAIT_MANY.md` — reusable/accepted handles and rederivable
   child construction.
4. `BATCH_OBSERVER_AND_CANCEL.md` — one batch transaction, persistent observer
   allocator, and cancellation transaction.
5. `SNAPSHOT_ISOMORPHISM.md` — complete live/snapshot inventory and strict
   rejection boundaries.
6. `MATCH_ROLE_TAG_CALLABLE.md` — exact 38-family current HIR expression table,
   13-family pattern table, stable tags, child roles and callable joins.
7. `OWNERSHIP_PROJECTION_MATRIX.md` — all 85 current `TypeKind` rows.
8. `SOURCE_DELETION_AND_CUTS.md` — current source evidence, deletions and the
   compile-clean five-cut order.
9. `TEST_MATRIX.md` — focused/property/differential/tamper/rollback/restore tests.
10. `VERIFICATION_SCOPE.md` and `VALIDATION_OUTPUT.txt` — what was actually
    verified in this environment.

## Machine contract and validation

Machine-readable truth is under `machine/`. The validator is standard-library
Python and does not modify the package:

```sh
python3 tools/validate_package.py . --self-test
```

It also accepts the ZIP directly:

```sh
python3 tools/validate_package.py   arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1-runtime-handle-batch-and-snapshot-isomorphism-correction-final-contract.zip --self-test
```

The self-test mutates the in-memory contract twelve ways and requires each
mandatory blocker to fail with its specific error code.

## Non-negotiable compatibility boundary

Every Arcweft-owned version marker remains exactly `1`. There is no
compatibility reader, string fallback, identity alias, dual carrier, second
`RuntimeValue` digest grammar, source-spelling identity, or source-string
reconstruction. Existing AWBC opcode allocations, including `MakeNeedHandle`
and `NeedTimeout`, are not renumbered.

The existing `AwbcRuntimeValueSnapshot` owner is evolved in place at Cut 5.
No parallel snapshot DTO or reader remains.
