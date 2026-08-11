# Lang-01.1.1.3.1 final contract

This archive is the decision-complete correction for **Lang-01.1.1.3 — effect-trait contract and dynamic-dispatch production reconciliation**. It resolves the only rejected boundary in the returned parent package: the ownership relation between the proposed checked callable facts and Arcweft's already accepted `RegisteredCallableCatalog` / `CallableRecord` authority.

## Status

`READY_FOR_IMPLEMENTATION`

There are no open design questions. `OPEN_QUESTIONS.md` contains exactly `none`.

## Normative result

The final model is **retention, not replacement**:

- `RegisteredCallableCatalog` and its exact `Arc<CallableRecord>` values remain the sole accepted owners of callable signature, source, documentation, declaration access, provider provenance, publication metadata, and fixed environment/standard effect rows.
- `CheckedCallableCatalog` is the sole owner of checked-context facts. Each accepted checked record retains the exact `Arc<CallableRecord>` from the accepted catalog and adds only revision-bound checked identity, execution role, body/contract effect facts, conformance facts, closure rows, and derived non-authoritative digests.
- `CheckedCallableFacts` does **not** own copied signature, source, documentation, access, provider, publication, or fixed-row fields.
- Project symbols, graph relations, LSP, Agent projections, compiler persistence, and runtime lowering consume the same immutable checked catalog generation. They store typed identity or derived output only; they do not reconstruct a callable from HIR, source spelling, declaration name, local indices, or copied effect rows.

The existing accepted catalog is extended in place to register trait requirements, trait implementation methods, inherent methods, standard methods, and their exact structural identities. There is no trait-only catalog, effect registry, adapter DTO, synchronization pass, compatibility view, shim, dual reader, source gate, or removed-syntax-only diagnostic.

## Parent package and precedence

Parent archive:

`arcweft-lang-01.1.1.3-effect-trait-contract-and-dynamic-dispatch-production-reconciliation-final-contract.zip`

SHA-256:

`4FD834564C458639CD4EBE46615E4EC79C54F91D686439AAAACCC7F2B3714B5E`

The parent package's E017 supersession, E017S static-witness disposition, typed effect-row semantics, effect-contract source model, typed diagnostics, trait conformance model, runtime identity migration, and deletion inventory remain normative except where this correction explicitly replaces ownership, identity-context, consumer-storage, and persistent-projection clauses. `FINAL_CONTRACT.md` contains the exact precedence table.

## Repository identity inspected

- Repository: `Sanzentyo/arcweft`
- Pushed `main` Git commit: `b305c698b22a01b30f1d7e68be6d925e6e3a2875`
- Commit subject: `Adjudicate returned Lang and Stream contracts`
- Latest `AGENTS.md` inspected at that commit; blob SHA: `e91f99213dde67953beda6aa078c370a8dc4541d`
- Applicable Rust skill read in full; supplied file SHA-256: `1A28F552ADF5EFDE95205BEE8D56590AEB82346C48EBDF3FDBBAFF5DECA33665`
- Correction request SHA-256: `77CCD51D4D382A5CF82804E6E9476F9E3D506A81ACF3F0920B94A13EE4B8DF08`

Jujutsu change identity is not encoded by the pushed Git commit and is not exported by the GitHub connector. This archive therefore does not invent one. The exact local resolution command is recorded in `REPOSITORY_EVIDENCE.md`; this evidence limitation does not leave an implementation design choice.

## Archive contents

- `README.md` — package orientation and precedence.
- `SUMMARY.md` — answer-first design summary.
- `FINAL_STATUS.md` — readiness declaration and validation boundary.
- `FINAL_CONTRACT.md` — normative corrected contract.
- `CATALOG_AUTHORITY.md` — exact Rust-shaped owner, construction, sharing, and transaction model.
- `IDENTITY_AND_CONSUMER_SCOPE.md` — structural/checked/durable identity and complete consumer table.
- `IMPLEMENTATION_ORDER.md` — deletion-driven, compile-clean public switch.
- `TEST_MATRIX.md` — retained parent matrix plus correction evidence.
- `REQUIREMENTS_TRACEABILITY.md` — request-to-contract-to-test mapping.
- `REPOSITORY_EVIDENCE.md` — exact repository and package evidence, with validation limits.
- `OPEN_QUESTIONS.md` — exactly `none`.
- `MANIFEST.sha256` — filename-sorted SHA-256 and byte length for every other member.

## Interpretation rules

Normative keywords `MUST`, `MUST NOT`, `REQUIRED`, `SHALL`, `SHALL NOT`, and `SHOULD` use their ordinary requirements meaning. Rust-shaped declarations are normative as to ownership, fields, visibility, relationships, and owning crate/module named by this package; no field, owner, identity, fallback, publication, or module-placement choice is left to the implementer.
