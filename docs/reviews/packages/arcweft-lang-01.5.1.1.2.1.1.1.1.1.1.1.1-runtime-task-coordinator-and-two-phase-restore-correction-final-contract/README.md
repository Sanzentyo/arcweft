# Arcweft runtime task coordinator / two-phase restore — design-only final contract

## Verdict

**ACCEPTED DESIGN / IMPLEMENTATION-READY CONTRACT**

- Request: `request.md`
- Repository: `Sanzentyo/arcweft`
- Exact inspected main SHA: `UNAVAILABLE (authenticated clone was not available in the execution container)`
- Production source changes: **none**
- Production patch/overlay: **not included**
- `OPEN_QUESTIONS=0`

The design assigns one authoritative owner, `RuntimeTaskCoordinator`, and fixes restore as two public semantic phases: observer-silent **prepare** and consuming, journal-backed, atomic **commit**. Durable `COMMITTED` precedes visibility; a crash between decision and publication is completed by mandatory idempotent replay.

## Package reading order

1. `01-request-coverage.md` — one-to-one requirement closure and decision IDs.
2. `02-current-source-evidence.md` — exact SHA/AGENTS/source anchors and evidence limits.
3. `03-normative-design.md` — authority model, two phases, invariants, conflicts.
4. `04-rust-api-and-data-model.md` — concrete Rust types, methods, errors, owner placement.
5. `05-state-machine-and-persistence.md` — journal grammar, sequence, CP-00..CP-11, recovery.
6. `06-concurrency-and-lifecycle.md` — lock order, publication visibility, cancellation/shutdown.
7. `07-implementation-plan.md` — ordered file-by-file work and acceptance gates.
8. `08-test-plan.md` — named unit/integration/property/fault/concurrency rows.
9. `09-compatibility-migration-rollout.md` — versioning, rollout, rollback, telemetry.
10. `10-verification.md` — what was actually inspected/run and what was not.
11. `11-self-audit.md` — final completeness/contradiction audit.
12. `99-input-request.md` — original request verbatim for auditability.
13. `MANIFEST.txt`, `SHA256SUMS.txt`, `ZIP-VERIFICATION.txt` — package integrity.

## Core implementation rule

Where an enum/type is already owned by an arcweft crate, add the missing restore behavior to that original definition/`impl`. Do not route around it with an extension trait, duplicate helper enum, or stringly ad-hoc branch.

## Verification boundary

See `10-verification.md`. Source-level claims are tied to the exact SHA only when the authenticated repository was materialized. No compile/test claim is made for a design-only package without a production implementation.
