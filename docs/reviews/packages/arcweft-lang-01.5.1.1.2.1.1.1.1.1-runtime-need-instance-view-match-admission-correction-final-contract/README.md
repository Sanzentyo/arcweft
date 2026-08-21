# arcweft-lang-01.5.1.1.2.1.1.1.1.1-runtime-need-instance-view-match-admission-correction-final-contract

## Final status

**READY_FOR_IMPLEMENTATION — DESIGN ONLY — OPEN_QUESTIONS=0**

This archive is the complete corrected design contract for
Lang-01.5.1.1.2.1.1.1.1.1. It closes the nonnumeric runtime Need-instance,
task-correlation, current-View identity, generic-Match/View-admission, and
ownership-evidence contradictions identified by the repository intake.

- Repository: `Sanzentyo/arcweft`
- Inspected `origin/main`: `17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc`
- Request's older production observation: `cbf0acedb98de260d8ecaab70a39933c39f30708`
- Arcweft-owned version markers selected here: exactly `1`
- Production source, tests, fixtures, manifests, branches, patches, and pull
  requests modified by this return: none
- AWBC opcode/function-kind/function-flag/varint/encoder allocation emitted by
  this return: none; the maintained version-1 numeric authority is an external
  prerequisite
- Compatibility readers, String identity fallbacks, suffix identities, dual
  carriers, source reconstruction, and identity translation tables: none

The inspected main is newer than the SHA written in the request because the
repository has since accepted the predecessor package and its reconciliation
intake. This answer uses `17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc` as the evidence baseline and treats the
intake's `DESIGN_NOT_READY` result as authoritative over the predecessor
archive's internal `READY_FOR_IMPLEMENTATION` label.

## Exact input identity

| Input | SHA-256 | Additional identity |
|---|---|---|
| `inputs/CURRENT_REQUEST.md` | `0152f1dd5f6fd315722f729700d3b94d1b0daa596a59445313e7796bddde8322` | Git blob `7ed008dec6eddb820e228ea0803bf97a1ead2c36` |
| `inputs/RUST_SKILL.txt` | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` | read in full before design |
| `inputs/PROJECT_PREMISE.txt` | `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` | read in full before design |
| retained predecessor ZIP | `DDD097E8057A8D45018528431790C20A2DE665CDE40F0329B82CB0366CF95D32` | repository intake/frozen mirror evidence |

`OPEN_QUESTIONS.md` is exactly four bytes: `none`.

## Reading order

1. `FINAL_CONTRACT.md`
2. `DECISION_REGISTER.md`
3. `RUST_SCHEMAS.md`
4. `OWNER_API_MAP.md`
5. `DEPENDENCY_GRAPH.md`
6. `IDENTITY_AND_DIGESTS.md`
7. `TASK_LIFECYCLE_AND_PERSISTENCE.md`
8. `CHECKED_MATCH_AND_VIEW_ADMISSION.md`
9. `OWNERSHIP_EVIDENCE.md`
10. `FAILURE_PRECEDENCE_AND_ATOMICITY.md`
11. `COMPILE_CLEAN_SEQUENCE.md`
12. `DELETION_MATRIX.md`
13. `SOURCE_EVIDENCE.md`
14. `REQUIREMENT_TRACEABILITY.md`
15. `TEST_MATRIX.md`
16. `STRUCTURAL_ABSENCE.md`
17. `VALIDATION_SCOPE.md`
18. `VALIDATION.md`
19. `FINAL_STATUS.md`
20. `OPEN_QUESTIONS.md`

Machine-readable projections are under `machine/`; CSV projections are under
`tables/`; the read-only, Python-standard-library package validator is
`tools/validate_package.py`.

## Closed architecture in one page

1. `NeedProducerInstanceKey` commits the producer family, exact producer
   contract, complete task-plan semantics, stable producer site, payload type,
   and the sole canonical runtime-value argument digest.
2. `NeedId` identifies one terminal cell within a generation, `TaskKey`
   identifies the generation-bound coalescing group, and `TaskId` identifies
   one actual launch. `TaskLaunchOrdinal` appears in `NeedId` once and
   `TaskId` once, and never in `TaskKey`.
3. `JoinSameKey` uses ordinal `0`. `AlwaysStart` allocates a journal-owned
   ordinal from `1`; allocation, journal insertion, and adapter launch
   acceptance are one transaction.
4. Reusable `RuntimeNeedHandle` construction is `JoinSameKey` only.
   `AlwaysStart` produces a concrete handle only after an accepted launch.
5. `RuntimeValueDigest` remains the existing
   `arcweft_core::entry::RuntimeValueDigest`. The existing canonical value
   grammar becomes sink-parametric; `Tuple([])` is the empty argument value.
6. `GenerationId` moves to `arcweft_core::task`. `TaskSpec` contains no
   caller-supplied Need/task identities; `TaskHost::ensure_task` derives the
   complete `TaskCorrelation`.
7. Generic `CheckedMatch` owns language meaning and bounded coverage only.
   `CheckedViewMatchAdmission` separately owns retained output/capture
   persistence, exact consulted ownership evidence, and Need-producer
   admission.
8. `ViewProgramId` is the stable program owner. The current
   `AcceptedViewProgramRevision([u8; 32])` is catalog/bundle/replacement
   evidence only and never enters Match, View-admission, task-plan, producer,
   or Need identity.
9. Opaque value-class and persistence evidence is mandatory from
   `AcceptedNominalInventoryInput` through registrar and accepted catalog.
   No default, name inference, copied registry, or unkeyed resource lookup is
   permitted.
10. The public task/Need schema, events, journal, Await/AwaitMany, timeout,
    persistence, replay, replacement, adapters, and deletion of the String
    route are switched in one indivisible protected cut.

## Validation interpretation

The package validator passed against both the extracted directory and the final
ZIP. It verifies request hashes, required files, closed decisions, version
markers, policy/identity separation, evidence shape, test/traceability
coverage, manifest hashes, and ZIP safety.

No production Rust build, test, Clippy, rustdoc, native/Web/headless parity, or
generated-artifact gate is claimed as executed. Those commands are specified
as implementation admission gates in `TEST_MATRIX.md` and `VALIDATION_SCOPE.md`.
The predecessor binary ZIP was not independently downloaded through the
connector in this return; its byte identity and safety are inherited only from
the current repository intake, while its frozen textual mirror was inspected.
