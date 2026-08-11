# Lang-01.3.1.2.3.2 — generic ownership identity and slot reconciliation correction

Status: **READY_FOR_IMPLEMENTATION**  
Open result-changing decisions: **0**  
Production implementation included: **no**

This archive is the standalone design-only correction requested by
`SOURCE_REQUEST.md`. It closes the identity, slot, transaction, path, activation,
and persistence symbols that were left undefined by Lang-01.3.1.2.3 and
Lang-01.3.1.2.3.1. It does not contain a checkout, patch, implementation
overlay, compatibility layer, source gate, or generated production file.

## Fixed basis

- Repository: `Sanzentyo/arcweft`
- Inspected `main`: `d8fbeaa5757fe5836fba17fca35fa104eeb72a1d`
- Accepted classifier implementation: `b76465c128322be2d5e66398bc6c30794ca0276f`
- Source request SHA-256: `dc9d39578e4706b7b518bc2cfdd37fda33d6be38352007c957e2360704afcf76`
- Parent Lang-01.3.1.2.3 archive SHA-256:
  `d053fae201afa104f7db9914aebbc08f2456875d1229f5325f86235d4bc0ea94`
- Parent Lang-01.5.1.1.2 archive SHA-256:
  `87b7f7bea85bc54254e3a979f0d668026ab75cb1c71955fd7a0f740e4f30c1c6`
- Parent Lang-01.3.1.2.3.1 archive SHA-256:
  `a52453fd07fdacf10205cbf621077f923ded714b83e4c64b9b69c52a7350ff7f`

The inspected `main` is one documentation-only correction commit after the
accepted classifier cut. The classifier remains the sole ownership lattice
authority and is not redesigned here. No Stream handle or affine token becomes
constructible in this correction.

## Read order

1. `FINAL_CONTRACT.md` — complete normative result.
2. `RUST_OWNERS_AND_APIS.md` — exact target owners, visibility, traits, APIs,
   support symbols, and sealed storage protocol.
3. `IDENTITY_AND_CODEC_CONTRACT.md` — allocation, ordering, text/binary codecs,
   and golden bytes.
4. `TRANSACTION_AND_COMMIT_CONTRACT.md` — preflight, preparation, reservation,
   commit permit, owner return, and infallibility boundary.
5. `VALUE_PATH_AND_PRECEDENCE.md` — canonical graph traversal, path ordering,
   and deterministic first-error selection.
6. `SNAPSHOT_ACTIVATION_AND_RESTORE.md` — domain-wide activation, save/restore,
   replay, allocator cursors, tamper checks, and atomic replacement.
7. `PRODUCER_CONSUMER_DELETION_INVENTORY.md` — current owners, target producers
   and consumers, and direct deletions.
8. `SUPERSESSION_DELTA.md` — narrow override against the affine parents.
9. `IMPLEMENTATION_ORDER.md` — compile-clean G1.1/G1.2/G1.3/G1.4 interleave.
10. `TEST_MATRIX.csv` and `NEGATIVE_AND_TAMPER_MATRIX.md` — positive, negative,
    boundary, tamper, API, parity, and full-gate coverage.
11. `REQUIREMENTS_TRACEABILITY.md` — request-to-decision/test closure.
12. `REPOSITORY_EVIDENCE.md`, `VALIDATION.md`, and `FINAL_STATUS.md` — evidence
    and verified limitations.
13. `MANIFEST.txt` and `PACKAGE_VALIDATION.json` — archive integrity.

## Normative conventions

- “must”, “must not”, “only”, and “exactly” are normative.
- Rust declarations marked **exact target declaration** are complete for the
  symbol shown. Existing types named as imported owners are not duplicated.
- Fields are private unless the declaration explicitly writes `pub`.
- No name, source span, display string, process pointer, vector accident, or
  source-text scan is an identity source.
- Any new behavior on an existing Arcweft enum/newtype is added through the
  owning type's inherent `impl`; no extension trait or ad hoc helper is the
  public authority.
- Every serialized integer width, enum tag, field order, duplicate rule, and
  first-error order is fixed below.
- Parent decisions not listed in `SUPERSESSION_DELTA.md` remain authoritative.

## Package integrity model

`MANIFEST.txt` lists every lexical member. Its own row uses 64 ASCII zeroes;
every other row contains the exact SHA-256 of the archived bytes. The ZIP uses
fixed timestamps, lexical member order, fixed Unix permissions, and no directory
entries. `PACKAGE_VALIDATION.json` records actual archive checks and does not
claim production compilation.
