# Proof-concurrency v6.1.1.4.1.1.1.1 final correction

`FINAL_STATUS: READY_FOR_IMPLEMENTATION`

This English, design-only archive is the standalone corrected authority requested by
`2026-07-28-seq-proof-01.1.1.4.1.1.1.1-tail-owner-and-generator-evidence-correction.md`.
It replaces the rejected v6.1.1.4.1.1.1 role-admission return for every affected row;
an implementer does not need to compare the two archives.

## Repository basis

- repository: `Sanzentyo/arcweft`
- inspected GitHub `main`: `5214a4836d5aa13a934ea8cb7037cc3a2a3c8e31`
- current-main state: the eight-variant typed `SyntheticOwner` is implemented; the
  final `SyntheticKey` is intentionally not implemented while this correction was
  pending
- current `identity.rs` blob: `2c5abea32ca7df642522b449af832064bd1dd1ce`
- current request blob: `abcab3da13ddf2241d4a97ea47437de9a1bb7311`
- rejected-return intake blob: `b95e44abf4fb7f0f7bafd5c0d91d785ecc932a79`

## Closed correction

1. `ImplicitUnitTail` and `MissingRequiredTail` accept exactly `Expr | Scope`,
   with ordinal exactly zero.
2. Source-backed ordinary expression containers use their reserved root `ExprId`.
   Predicate/proof block bodies and individual match arms use their already-required,
   already-reserved `ScopeId`.
3. The owner is reserved before the synthetic tail, so no tail owns itself. Each
   match arm has a distinct scope, so exact-zero keys cannot collide across arms.
4. `RecoveryOperand`, `DesugaredTemporary`, `DestructuredBinding`,
   `ClosureCapture`, and both postfix candidate roles now have named direct
   production lowering/transaction tests in addition to identity admission tests.
5. Liveness tests use the exact retained `NotYetLive { id, snapshot, born }` and
   `Retired { id, snapshot, retired_at }` payloads.
6. The rejected return's 51-byte fingerprint transcript, tag allocation, fixed
   vectors, constructor precedence, source-ordered ordinal bound, candidate-only
   identity, and compatibility prohibitions are retained unchanged.

## Normative member guide

- `FINAL_CORRECTION.md` — decision summary and readiness conclusion.
- `RUST_SCHEMAS.md` — complete affected Rust-facing schemas and invariants.
- `ROLE_OWNER_ORDINAL_MATRIX.tsv` — complete 21-role truth table.
- `TAIL_PRODUCER_OWNER_MATRIX.tsv` — exact producer/owner/allocation/anchor mapping.
- `AFFECTED_LOWERING_ROWS.tsv` — complete corrected lowering rows.
- `GENERATOR_EVIDENCE_CONTRACT.md` — production ordinal algorithms and direct evidence.
- `CONSTRUCTOR_AND_TRANSACTION_CONTRACT.md` — structural admission, liveness,
  reuse, limits, and rollback.
- `FINGERPRINT_TRANSCRIPT.md` — retained byte-for-byte from the rejected return.
- `TEST_MATRIX.tsv` — complete focused behavioral, transaction, compile-fail, and
  fingerprint matrix.
- `REQUIREMENTS_TRACEABILITY.tsv` — every request clause mapped to normative rows.
- `PREDECESSOR_PRECEDENCE.md`, `REPOSITORY_EVIDENCE.md`, and
  `VALIDATION_REPORT.md` — authority and validation evidence.
- `REQUEST_COPY.md` and `REJECTED_RETURN_INTAKE_COPY.md` — byte-identical repository
  inputs.
- `MANIFEST.txt` — exact length and SHA-256 of every other member; the manifest
  intentionally omits itself.

## Status boundary

`READY_FOR_IMPLEMENTATION` means the focused design decisions and required tests are
closed. This archive contains no production Rust, patch, branch, PR, overlay,
compatibility reader, source gate, or implementation result. `OPEN_QUESTIONS.md` is
exactly the four bytes `none`.
