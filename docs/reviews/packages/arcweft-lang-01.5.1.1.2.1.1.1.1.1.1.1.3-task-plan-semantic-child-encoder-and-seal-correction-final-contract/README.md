# Lang-01.5.1.1.2.1.1.1.1.1.1.1.3 final contract

This archive is the design-only, implementation-ready return for the task-plan
semantic child encoder and seal correction. It contains no production patch,
Cargo overlay, generated production fixture, branch, or compatibility reader.

## Final decision

The final model has one structured-plan authority and one cross-layer View
protocol:

```text
private RuntimePlan candidate
  -> RuntimePlanSemanticEncoder
       -> RuntimeExecutableSemanticDigest
       -> ProducerFunctionSemanticDigest
       -> TaskRequestTemplateDigest
       -> ControlEffectContractDigest
       -> opaque RuntimeTaskPlanDigestBase<'a>
  -> core seals every non-View row itself
  -> ViewTaskPlanAuthority seals only View-marker rows from
       ValidatedViewTaskPlanBinding held by ValidatedViewProgramResource
  -> expected-key comparison for decoded images
  -> global duplicate check
  -> RuntimeTaskPlanTable + public RuntimePlan are constructed together
```

`RuntimeTaskPlan` stores only static fields. It never stores its own digest, an
expected digest, producer-instance fields, or a copied View identity. The final
lookup table is built only after every row has been recomputed and sealed.

The executable digest is acyclic because it commits source-order task-plan base
rows and construction coordinates, never task-plan map keys, completed
`TaskPlanSemanticDigest` values, decoded expected keys, or a self-digest field.
The upper View binding adds the actual `ViewProgramId`, `ViewMatchSiteId`, and
`CheckedViewMatchAdmissionDigest`; `AcceptedViewProgramRevision` validates the
binding owner but is never hashed into task-plan identity.

## Repository basis

- Repository: `Sanzentyo/arcweft`
- Inspected branch: `main`
- Full inspected Git commit: `515bb071437c3af053f1560c3119906dc8002efc`
- Access method: GitHub connector against the private repository
- Working-tree state: not observable because no local checkout was used
- Production changes made by this return: none

Current source, maintained documentation, current `AGENTS.md` files, the
accepted predecessor contracts, and the attached request were used as evidence.
See `SOURCE_INVENTORY.md` for the exact paths and observations.

## Normative files

- `FINAL_CONTRACT.md` — decisions 1–9, ownership, publication, and errors.
- `RUST_SCHEMAS.md` and `schemas/final_contract.rs` — Rust-shaped final API.
- `TRANSCRIPTS.md` and `EXECUTABLE_TRANSCRIPT.md` — exact version-one bytes.
- `CYCLE_PROOF.md` — dependency DAG and termination proof.
- `PRIVATE_CODEC_AND_EXPECTED_KEYS.md` — strict private wire image and recomputation.
- `SEAL_STATE_MACHINES.md` — builder, compiler/bundle, and decode machines.
- `ERROR_PRECEDENCE_AND_LIMITS.md` — limits and deterministic first errors.
- `OWNER_CONSUMER_MATRIX.md`, `DEPENDENCY_MATRIX.md` — migration inventory.
- `COMPILE_CLEAN_SEQUENCE.md` — deletion-driven Cut 5 atomic switch.
- `TEST_MATRIX.md` — exhaustive implementation acceptance tests.
- `machine/`, `tables/` — machine-readable mirrors.
- `tools/validate_contract.py` — read-only package/repository validator.
- `tools/negative_self_tests.py` — mandatory validator mutation corpus.
- `VALIDATION_REPORT.md` — validation actually performed for this archive.
- `FINAL_STATUS` — exactly `READY_FOR_IMPLEMENTATION`.
- `OPEN_QUESTIONS` — exactly `none`.

## Validation commands

Extracted directory:

```bash
python3 tools/validate_contract.py .
python3 tools/negative_self_tests.py .
```

Returned ZIP:

```bash
python3 tools/validate_contract.py \
  ../arcweft-lang-01.5.1.1.2.1.1.1.1.1.1.1.3-task-plan-semantic-child-encoder-and-seal-correction-final-contract.zip
```

Repository-aware validation from an Arcweft checkout at the inspected commit:

```bash
python3 tools/validate_contract.py . --repo /path/to/arcweft
```

The repository mode is read-only. It uses Git and structured Cargo metadata;
it does not grep production source spelling as an acceptance gate.
