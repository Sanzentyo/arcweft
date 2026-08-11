# Lang-01.3.1.2.3.2.1.1 final contract

This archive is the independently usable, design-only correction requested by
`SOURCE_REQUEST.md`. It is pinned to repository `Sanzentyo/arcweft`, branch
`main`, Git commit `a38c736ba577172b1f4c3fe1a0c3e85443e97e6f`. The retained-byte parent ZIP identity is
`4b15a5eaea31663a9323f41f75345b2acb6faa0ea3a61784eeeabd482a13966a`.

## Result

- `STATUS=READY_FOR_IMPLEMENTATION`
- `OPEN_RESULT_CHANGING_DECISIONS=0`
- `OPEN_QUESTIONS=0`
- `PRODUCTION_OVERLAY_INCLUDED=0`
- `IMPLEMENTATION_PERFORMED=0`
- `AWBC_ABI_VERSION=1` remains unchanged.
- The inspected commit owns `AWBC_CODEC_VERSION=10`; this contract allocates
  codec 11 for the new opaque rows instead of repeating the parent's stale
  codec-8 statement.
- Session-save schema 2 becomes 3 because `RuntimeValue::Opaque` is persisted
  through fibers/snapshots.

## Normative reading order

1. `FINAL_CONTRACT.md`
2. `RUST_OWNERS_AND_APIS.md`
3. `OPAQUE_OWNER_AND_VALUE_CARRIER.md`
4. `COMPOSITE_AND_VARIANT_RULES.md`
5. `PRODUCER_PROJECTION_CONTRACT.md`
6. `RUNTIME_RESOLVED_VARIANT_API.md`
7. `AWBC_WIRE_AND_VERIFIER.md`
8. `PERSISTENCE_AND_MIGRATION.md`
9. `ERROR_AND_PRECEDENCE.md`
10. `IMPLEMENTATION_ORDER.md`
11. `SUPERSESSION_DELTA.md`
12. inventories, test matrices, traceability, and verification evidence.

`OPEN_QUESTIONS.md` is exactly `none`. `MANIFEST.txt` is the sorted SHA-256
ledger; its self-entry uses 64 zeroes. The archive contains no `.rs`, patch,
diff, Cargo manifest, generated executable, production overlay, compatibility
shim, or test log that claims repository execution.
