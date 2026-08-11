# Repository evidence

## Audited remote state

- repository: `Sanzentyo/arcweft`
- default branch: `main`
- latest inspected main: `66f9bffa0ec3422c14627fcacd0457b28c28e146`
- commit message: `Add typed synthetic HIR owner projection`
- request Git blob SHA-1: `534e3b2c40c1b0dcc1688ca8c1413b1dbe9bdad6`
- uploaded/request-copy Git blob SHA-1: the same value
- uploaded request SHA-256: `c4f7d650f2e0674b81ff19d85216868363be47982fa9cf72fa43996d8f16cf53`

## Current implementation owners inspected

| repository path | Git blob SHA | observed responsibility |
|---|---|---|
| `AGENTS.md` | `e91f99213dde67953beda6aa078c370a8dc4541d` | typed owner/inherent-method/deletion/no-source-gate policy |
| `crates/arcweft-lang-hir/src/identity.rs` | `2c5abea32ca7df642522b449af832064bd1dd1ce` | database-qualified typed IDs, eight-owner enum, 21 roles, `HirLimit` maximum 1,024; no final key yet |
| `docs/implementation/2026-07-28-proof-01-1-1-4-1-1-source-owner-consistency-intake.md` | `85e91f5e0faf379914893d904857dfafad4a8d32` | predecessor integrity and focused admission/fingerprint blocker |
| `docs/implementation/2026-07-28-proof-database-qualified-hir-identity.md` | `edc6d2cd2c9e8fd5f77d324e0c0d3fcd77bf9bb4` | process-local `HirDatabaseId`, qualified module/slot identity, no numeric public accessor |
| `docs/implementation/2026-07-28-proof-typed-synthetic-owner-projection.md` | `bab2a4b75c4f708ce4d49c476889d08760b7fbb4` | already-landed eight typed owner variants and projections; key deliberately deferred |
| `docs/reviews/requests/2026-07-28-seq-proof-01.1.1.4.1.1.1-synthetic-role-owner-admission-correction.md` | `534e3b2c40c1b0dcc1688ca8c1413b1dbe9bdad6` | exact focused assignment |

## Retained predecessor package verification

- path supplied locally and retained in repository: `arcweft-proof-concurrency-v6.1.1.4.1.1-source-owner-and-semantic-consistency-correction-final-contract.zip`
- bytes: `91023`
- SHA-256: `2bcd3f78efb76442c2698a24251c4d874f7a941c5a8985649ea157100908a72e`
- members: 24
- non-self manifest rows: 23
- ZIP CRC/decompression: clean
- manifest byte lengths and SHA-256 values: exact
- `OPEN_QUESTIONS.md`: four bytes, `none`
- `FINAL_STATUS.md`: `READY_FOR_IMPLEMENTATION`

The complete predecessor archive was extracted. Its schemas, predecessor precedence, source/transaction contract, 82-row lowering matrix, 106-row test matrix, 164 subtest registry, evidence, and validation material were reconciled. This correction does not silently replace any unaffected row.

## Base and AW-AH evidence used

The repository-retained base Proof package fixes the synthetic role vocabulary, source anchors, deterministic source-order allocation, typed arenas, and 1,024 descendants-per-owner limit. Its accepted `HIR_DATABASE_AND_ARENAS.md` and `SCOPES_LOCALS_CAPTURES.md` role tables were applied directly.

The repository-retained AW-AH-009.4.2 package fixes candidate ownership by the source-backed postfix expression, root ordinal zero, shared-target reuse, two bounded interpretation families, and the prohibition on reusing candidate-only identity as a selected committed expression.

Hashes are recorded in `PREDECESSOR_PRECEDENCE.md`. No external attachment or unverified alternate package was substituted.

## Current-main reconciliation

The commits after the v6.1.1.4.1.1 package intake qualify HIR IDs by database and land only the typed `SyntheticOwner` projection. They do not select any role admission or fingerprint bytes. The schema in this archive therefore adds behavior to the existing owner/role enums and does not preserve an obsolete implementation island.
