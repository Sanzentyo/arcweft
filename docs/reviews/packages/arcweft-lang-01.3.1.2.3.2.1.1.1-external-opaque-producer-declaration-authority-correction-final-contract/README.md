# Lang-01.3.1.2.3.2.1.1.1 final contract

This archive is the independently usable, design-only correction requested by
`SOURCE_REQUEST.md`. It is pinned to `Sanzentyo/arcweft` `main` at Git commit
`78f50f5b5ac082745bab91b7373a6602918a436d`. The commit is the request commit whose parent is `7636b61a1c4c8e81127cb81a8fd27ef765d5ce2a`.

## Result

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_QUESTIONS=0
OPEN_RESULT_CHANGING_DECISIONS=0
DESIGN_ONLY=YES
PRODUCTION_CODE_CHANGED=NO
PRODUCTION_OVERLAY_INCLUDED=NO
SCHEMA_CUTS=adapter-manifest/2,rust-abi/2
```

The retained-byte parent authority is identified by SHA-256
`93af482a2914ca4a9e6b985aa7a09c040f569bd71141611dcaa4d579ac01640c`. This package narrows only the external declaration-authority
gap found after parent intake. It does not redesign the parent's opaque runtime
owner/value relation, exact versus producer-wide admission, composite recursion,
AWBC ABI 1 / codec 11, canonical runtime-value tag 16, opaque type tag 23,
opaque constant tag 18, session-save schema 3, nominal-record layout,
identity/slot/path, activation, View, or Stream decisions.

## Normative reading order

1. `FINAL_CONTRACT.md`
2. `RUST_OWNERS_AND_APIS.md`
3. `SCHEMA_2_CODEC_AND_DERIVE.md`
4. `ACCEPTED_CATALOG_AND_PUBLICATION.md`
5. `ERROR_AND_PRECEDENCE.md`
6. `DIGEST_AND_GENERATED_SOURCE.md`
7. `IMPLEMENTATION_ORDER.md`
8. `DELETION_SET.md`
9. inventories, matrices, traceability, and evidence files

`OPEN_QUESTIONS.md` is exactly `none`. `MANIFEST.txt` is the sorted SHA-256
ledger and uses the all-zero self-entry convention. This archive contains no
Rust source overlay, patch, diff, Cargo manifest, branch, generated executable,
compatibility layer, or claimed production test log.
