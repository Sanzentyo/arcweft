# arcweft-lang-01.1.1.3.2-suspended-function-runtime-emission-and-opaque-nominal-layout-reconciliation-final-contract

`FINAL_STATUS=READY_FOR_IMPLEMENTATION`  
`OPEN_QUESTIONS=0`

- Request date: 2026-08-21
- Request-observed commit: `6e17c9fafe7c254b27e99f51af52ccc109a3a41d`
- Actual design baseline: `9138efeeabdfca56809e8ad9c16fc85380ae18c5` (`origin/main` at inspection time)
- Package kind: design-only; no production source, patch, overlay, branch, compatibility reader, or generated Rust file is included.

## Final result

The runtime product is selected by **generation-bound reachability**, not by the presence of a semantically accepted declaration. An ordinary function remains fully checked and visible to tooling, but it publishes runtime semantic facts only when it is a Flow/Entry root or is reached through an exact checked edge.

Accordingly, fixture 013 remains unchanged and succeeds: its `load_opening_assets()` function is accepted by final semantic analysis but is not reachable from `flow main`, so neither the authored suspension nor `OpeningAssets { bg: ImageHandle }` enters runtime projection.

No new opaque nominal layout is introduced. Transient and persisted project nominals retain the same closed-schema contract: `RuntimeTypeSchema` is projected and `RuntimeTypeSchema::try_layout_hash` is the sole `TypeLayoutHash` authority. A reachable project struct or enum containing an opaque leaf is rejected with a typed, path-bearing diagnostic. A reachable suspending ordinary function is rejected earlier, before any nominal schema/layout projection.

## Reading order

1. `FINAL_CONTRACT.md`
2. `DECISION_TRACEABILITY.md`
3. `OWNER_AND_API_CONTRACT.md`
4. `REACHABILITY_ALGORITHM.md`
5. `LAYOUT_AND_PERSISTENCE_CONTRACT.md`
6. `PIPELINE_CONSUMER_CONTRACT.md`
7. `DIAGNOSTIC_PRECEDENCE.md`
8. `RUST_SHAPES.md`
9. `IMPLEMENTATION_ORDER.md`
10. `TEST_MATRIX.tsv`
11. `CONSUMER_DELETION_INVENTORY.tsv`
12. `REPOSITORY_EVIDENCE.md`
13. `VALIDATION.md`
14. `ACCEPTANCE_COMMANDS.md`
15. `SOURCE_REQUEST.md`
16. `MANIFEST.sha256`

## Non-negotiable invariants

- One reachability owner; no fallback reader and no second owner inventory.
- No source text, fixture name, display name, or `TypeKind::Named` success gate.
- No fabricated opaque `RuntimeTypeSchema`, dummy record, bytes fallback, producer schema copy, or name-derived layout.
- A checked call/Entry/Flow edge that reaches a function may never be silently dropped.
- `ImageHandle` and `ArcError` retain accepted producer and semantic identities as `RuntimeCheckedType::Opaque` leaves.
- All Arcweft-owned ABI, codec, save, and schema version markers remain exactly `1`.
