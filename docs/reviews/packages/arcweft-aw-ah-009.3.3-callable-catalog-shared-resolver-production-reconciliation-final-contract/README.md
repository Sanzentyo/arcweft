# AW-AH-009.3.3 callable catalog and shared resolver production reconciliation

Status: `READY_FOR_IMPLEMENTATION`

This archive freezes the production design required to replace Arcweft's
checker-local call and selected-call dispatch with one typed callable catalog,
one typed resolver, and one checker-owned target-fact protocol. It is a design
and implementation contract only. In accordance with the governing request,
it contains no production patch and changes no Rust, Cargo, schema, fixture, or
repository file.

## Repository basis

- Current `main` inspected: `9fd6ee8fb2814ff04dc7a3e4ef413b86b7f4ac4d`
- Current Jujutsu change: unavailable through the connector
- Reconciliation basis: Git `328e362f811896ebf866002c458fe0b970976654`, Jujutsu `wopypppm`
- Original AW-AH-009.3 archive identity:
  `cdd1d7b764da238a6e4e8f3e774a3384017c8da5ffaea1969f2af279102a7cd5`

The current `main` is an accepted descendant of the reconciliation basis. The
root `AGENTS.md`, the complete Rust skill, the governing request, the original
AW-AH-009.3 summary/status/hash sidecars, the current production-reconciliation
audit, and current callable/checker/registration/HIR/adapter code were reviewed.
The original AW-AH-009.3 ZIP binary was not mounted in this artifact runtime, so
its archive members were not independently re-extracted here; its repository
audit, delivery summary, status, and exact SHA-256 were available and were used
as the fixed upstream contract identity.

## Read order

1. `FINAL_CONTRACT.md` — normative Rust model, catalog, resolver, checker facts,
   results, limits, and errors.
2. `SURFACE_INVENTORY.md` — exhaustive current free-call and selected-call
   family reconciliation.
3. `PRODUCTION_RECONCILIATION.md` — current-to-target ownership and deletion
   map.
4. `IMPLEMENTATION_HANDOFF.md` — compiling cuts, exact migration order, and
   validation gates.
5. `TEST_MATRIX.md` — direct typed tests and invariant evidence.
6. `REQUIREMENTS_TRACEABILITY.md` — requirement-to-decision mapping.
7. `REPOSITORY_EVIDENCE.md` — inspected production evidence and validation
   honesty.
8. `FINAL_STATUS.md` — readiness declaration and verified scope.

## Normative conventions

- `must`, `must not`, `only`, and `exactly` are normative.
- Rust declarations are exact target declarations unless a declaration is
  explicitly marked as an existing imported type.
- Private fields are intentional. Public construction is only through the
  listed validating constructors.
- No label, alias, display path, Rust path string, comment, or source-text scan
  is an identity source.
- Once a resolver family is migrated, its old successful checker branch is
  deleted in the same compiling cut. There is no signature-only resolver and
  no compatibility fallback.
- AW-AH-009.3.1 owns exact call/argument ranges. AW-AH-009.3.2 owns accepted HIR
  request leasing. This contract consumes, but does not redesign, those
  carriers.

## Package integrity

`MANIFEST.txt` lists every archive member in lexical path order. The
`MANIFEST.txt` self-entry is 64 ASCII zeroes; every other entry is the SHA-256
of the exact archived bytes. The outside `.sha256` sidecar hashes the exact ZIP.
