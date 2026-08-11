# Lang-01.1.1.2.2 Final Contract

**Status:** normative, single-design, implementation-ready  
**Repository:** `Sanzentyo/arcweft`  
**Baseline:** `main@4fd6331dc342d30a7f4ac7774852b60801866ef7` (`Implement project nominal type resolution`)  
**Prepared:** 2026-07-22 (Asia/Tokyo)  
**Production code changes in this package:** none

This package settles the adapter/Rust callable nominal publication boundary. It is not a patch and contains no repository source replacement. It fixes every implementation choice required by the request so that an implementation agent can make one direct-final production cut.

## Final decision

1. A Rust-exported nominal is always owned by `AcceptedNominalOwnerId::RustPackage(RustPackageId)`. An adapter remains the callable provider and mount authority; it never becomes the semantic owner of that Rust nominal.
2. `ArcweftRustTypeRef::Named` and `AdapterTypeKind::Named` are replaced in place by validated owner + path + argument carriers.
3. `SourceBackedAdapterRegistrationFacts` emits unresolved-but-typed sema publication inputs. `CharacterRegistrar` projects them through the already constructed `AcceptedNominalWorld` before any `EnvironmentCallablePublicationRecord` exists.
4. `AdapterTypeKind::to_sema_type_kind()` is deleted. No public context-free operation can produce a semantic nominal.
5. Rust ADT metadata and enum payload metadata migrate in the same atomic registration cut and are keyed by `AcceptedNominalId` / `AcceptedNominalType`, never by display text.
6. Schema, candidate, tooling, and persistence identities consume exact `TypeKind::AcceptedNominal` identity. Display labels are output only and are never reparsed.
7. The unpublished Rust ABI and adapter file schema constants remain `1`; the carrier shapes are replaced in place, with one reader and one writer only.

## Package map

- `FINAL-CONTRACT.md` — normative decisions and invariants.
- `API-SHAPES.md` — exact Rust/API shapes, visibility, and deleted operations.
- `CONSTRUCTION-ORDER.md` — source-backed fact flow, dependency order, and atomic admission.
- `ERROR-ROLLBACK.md` — structured failures, deterministic reporting, and rollback table.
- `SCHEMA-TOOLING-PERSISTENCE.md` — canonical digests, candidate identity, tooling, and persistent keys.
- `IMPLEMENTATION-MAP.md` — ordered crate/file implementation map.
- `TEST-MATRIX.csv` — exhaustive typed-API test plan.
- `TRACEABILITY.md` — request-to-contract and request-to-test mapping.
- `NON-GOALS.md` — boundaries that remain intentionally unchanged.
- `REPOSITORY-VALIDATION.md` — repository evidence and validation scope.
- `COMMANDS-RUN.md` — repository and artifact commands actually executed.
- `contract/DECISIONS.json` — machine-readable final decisions.
- `evidence/BASELINE.json` and `evidence/INSPECTED-FILES.tsv` — pinned repository evidence.
- `validation/validate_contract.py` and `validation/VALIDATION-RESULT.txt` — artifact validation.
- `MANIFEST.sha256` — SHA-256 for every package file except the manifest itself.

## Validation result

The package is validated against the exact `main` commit above by read-only GitHub connector inspection, dependency/order reconciliation, machine checks of required decisions and test coverage, deterministic archive creation, and ZIP integrity verification. See `REPOSITORY-VALIDATION.md` for the precise boundary between evidence actually checked now and commands prescribed for the production implementation.
