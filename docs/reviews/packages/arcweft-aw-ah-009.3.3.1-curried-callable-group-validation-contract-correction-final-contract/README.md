# AW-AH-009.3.3.1 final contract correction

## Status

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_QUESTIONS=0
IMPLEMENTATION_PERFORMED=NO
```

This archive is the self-contained implementation contract for **AW-AH-009.3.3.1 — Curried callable group-validation contract correction**. The attached request Markdown is the sole task specification. Repository material is used only as current-state evidence.

- Request SHA-256: `fada8baca5a145aea1597385b609aee199be0b7122c485706e949380ff23d621`
- Inspected repository: `Sanzentyo/arcweft`
- Inspected `main`: `a8403dcb26d78e6cafee3576d5933e9952d8305b`
- Relevant substrate commit: `f420ee8fbf244351e11fd5f793b07e7cdd3f1b6a`

## Final decision

Select **context-free `CurriedCallableId` plus schema-aware validation at the existing `ResolvedCallable::try_new` success boundary**.

The two-argument constructor, ID fields, accessors, schema type, resolved-product type, and accepted-world borrowing boundary are preserved. The correction is deliberately narrow:

1. `CurriedCallableId::try_new` validates only facts available from its two inputs: `next_group != 0` and no recursive `Curried`/`DataLast` wrapper growth.
2. `CallableIdentityError::MissingGroup` is removed. Group zero becomes the exact identity error `CallableIdentityError::InvalidCurriedGroup`.
3. Missing nonzero schema groups, including one-over, are owned by `ResolveCallError::InvalidCallGroup` and retain `CallableDiagnosticCode::InvalidCallGroup`.
4. A successful curried result has exactly one representation: `CallableCandidateId::Curried` paired with matching `CallableInstantiation::Curried` and the full base schema.
5. The current alternative success representation—base ID paired with `CallableInstantiation::Curried`—is rejected as `ResolveCallError::InvalidResolvedCallable`.

No compatibility constructor, alias, deprecated variant, dual reader, source gate, second resolver, global lookup, thread-local world, CSS path, or Takumi path is introduced.

## Archive map

- `FINAL_CONTRACT.md` — normative ownership, APIs, invariants, errors, and rejection rules.
- `SURFACE_INVENTORY.md` — current and final Rust surface, including what remains unchanged.
- `PRODUCTION_RECONCILIATION.md` — evidence-based defect analysis and minimal production correction.
- `IMPLEMENTATION_HANDOFF.md` — exact implementation sequence, file ownership, and deletion gate.
- `TEST_MATRIX.md` — direct typed constructor, resolved-boundary, shared-resolver, and corrupt-world tests.
- `REQUIREMENTS_TRACEABILITY.md` — every request requirement mapped to a decision and test.
- `REPOSITORY_EVIDENCE.md` — inspected revision, files, call-site searches, and verification limits.
- `OPEN_QUESTIONS.md` — exactly `none`.
- `FINAL_STATUS.md` — machine-readable completion and prohibition summary.
- `MANIFEST.txt` — sorted member digests and byte sizes, excluding itself from its own digest.

## Verification scope

This task performed **read-only design and source inspection**. It did not change Arcweft and did not run Cargo commands because implementation was explicitly prohibited. Historical validation recorded in the repository is reported as historical evidence, not as a newly executed result. The archive itself is regenerated deterministically, reopened, member-checked, manifest-checked, and SHA-256 checked before delivery.
