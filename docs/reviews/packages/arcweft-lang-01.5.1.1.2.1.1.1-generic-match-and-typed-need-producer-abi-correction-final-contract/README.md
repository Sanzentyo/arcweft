# Lang-01.5.1.1.2.1.1.1 — generic Match and typed Need producer ABI correction

## Archive identity

- Required archive: `arcweft-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction-final-contract.zip`
- Repository: `Sanzentyo/arcweft` (private; inspected through the GitHub connector)
- Current `origin/main` used by this return: `4bda1cdcea63fdf7aac32691d756c1c0e1fc693e`
- Request's inspected production baseline: `dec4f6c2de3be87d28a2f976b1ae51e3b40dd3fd`
- The delta between those SHAs adds only the request and its implementation blocker; it changes no production source, manifest, test, fixture, or generated artifact.
- Retained parent archive SHA-256: `A7E146CD8F263127FE36EE29D10B24B118F8717BFC900BB88E957D3D863E30F4`
- Every Arcweft-owned schema, ABI, codec, digest transcript, snapshot, and bundle version in this contract remains exactly `1`.

## Final readiness

| Dimension | Result |
|---|---|
| Design closure | **DESIGN READY** |
| Open questions | **none** |
| Required exact decisions | **17/17 closed** (request decisions 1–16 plus the current-source guard execution defect) |
| Production implementation | **not claimed and not included** |
| Generic `CheckedMatch` sema-only cut | independently implementable |
| Product/runtime publication | unblocked by this design after acceptance; implementation must follow the compile-clean cuts |
| Package validation | machine and human validation included; see `VALIDATION.md` and `VALIDATION.json` |

This is a complete corrected contract. It is not a delta, pointer, patch, implementation overlay, compatibility layer, or validation-only return.

## Closed result in one page

1. A View Match selector returns one ordinary AWBC value: a synthetic nominal closed `Variant`; case ordinal equals source arm ordinal, and every case payload is a source-ordered tuple, including the empty tuple for zero bindings.
2. Its function signature is exactly `(NeedState<T>) -> SelectorResult`; the four-state input is the parent contract's ephemeral ordinary runtime variant. No producer evaluation or View body runs inside the selector.
3. No function-local `AwbcRegisterId` crosses the call boundary. The VM clones the single selector value, pops the callee frame, and the runtime driver decodes that retained value.
4. `arcweft-view` owns only lightweight coordinates: match site, arm ordinal, binding output ordinal, local reference, and body slice. It owns no AWBC register, core runtime value, or copied type table.
5. The bundle owns the sole static View/core join; the runtime driver owns private selector scratch and an all-or-nothing local installation transaction. The previously public `ViewMatchSelection` is deleted.
6. Every final-HIR Match receives one generic sema fact through `CheckedExpressionResolution::Match(Box<CheckedMatch>)`. `CheckedMatch` stores exact HIR IDs, coverage once, and ownership dispositions; expression, pattern, and binding types/effects remain in their existing final-analysis owners. The checked View catalog stores only `CheckedMatchRef`.
7. Compiler projection creates a generation-bound `RuntimeViewMatchSelectorSeed`; runtime-plan finalization rewrites its semantic type identities into the existing single `RuntimePlan` type table. `arcweft-runtime-plan` never depends on sema, View, or bundle and never receives a `CheckedMatch` directly.
8. View selector guards are lowered as explicit `TestPattern`/`BindPattern` plus ordinary guard evaluation and `Branch`. `AwbcTerminator::Match.guard` is forbidden for this selector because current verification accepts it while the current VM does not execute it.
9. `AwbcRuntimeType::NeedHandle` becomes `NeedHandle { payload: AwbcTypeId }`; `RuntimeCheckedType` gains `Need(Box<RuntimeCheckedType>)`; and `RuntimeValue` gains a dedicated `NeedHandle(RuntimeNeedHandle)` carrier. A `String` can never satisfy it.
10. `MakeNeedHandle` occupies unused ordinary-opcode byte `0x1e`; `AwbcFunctionFlags::NEED_PRODUCER` occupies bit 4. A producer is deterministic, non-suspending, returns exactly `NeedHandle<T>`, and is statically tied to one `AwbcTaskPlan` whose payload is `T`.
11. `AwbcTaskPlan.need_id: AwbcStringId` is deleted. A fixed 32-byte `NeedId` is derived from the verified producer contract and canonical source-ordered arguments. Equal contract/arguments join; a different contract or argument digest does not. The handle carries the arguments required to construct the later start intent; no second endpoint table exists.
12. Generation is not embedded into the core value. Runtime-driver extraction binds the handle to the active `GenerationId`; the journal key remains `(GenerationId, NeedId)`.
13. The existing `ResourceTypeRegistry` and canonical `ResourceTypeRegistryDigest` remain the sole resource authority. `FinalSemanticCatalogs` borrows the registry, verifies integrity, and freezes the digest; compiler View publication requires exact digest equality.
14. The final switch deletes the old View Await model, payloadless NeedHandle type row, NeedHandle-as-String admission, `await_target` string conversion, static task-plan need strings, old bundle rows/readers, and every duplicate checked-Match authority in one strict version-1 cut.

## Reading order

1. `FINAL_CONTRACT.md`
2. `DECISION_REGISTER.md`
3. `RUST_SCHEMAS.md`
4. `GENERIC_CHECKED_MATCH.md`
5. `SELECTOR_RESULT_ABI.md`
6. `GUARD_EXECUTION.md`
7. `TYPED_NEED_PRODUCER_ABI.md`
8. `BUNDLE_CROSS_SECTION.md`
9. `WIRE_TYPE_DIGEST_ALLOCATION.md`
10. `PERSISTENCE_REPLAY_REPLACEMENT.md`
11. `RESOURCE_REGISTRY_INPUT.md`
12. `OWNERS_AND_APIS.md`
13. `DEPENDENCY_GRAPH.md`
14. `FAILURE_PRECEDENCE_AND_ATOMICITY.md`
15. `COMPILE_CLEAN_SEQUENCE.md`
16. `CURRENT_SOURCE_EVIDENCE.md` and `SOURCE_EVIDENCE.csv`
17. `REQUIREMENT_TRACEABILITY.md` and `.csv`
18. `PRODUCER_CONSUMER_MATRIX.md`, `DELETION_MATRIX.md`, and their CSV forms
19. `TEST_MATRIX.md` and `.csv`
20. `STRUCTURAL_ABSENCE.md`
21. `VERIFICATION_SCOPE.md`
22. `VALIDATION.md`, `VALIDATION.json`, `tools/validate_package.py`
23. `MANIFEST.json`, `MANIFEST.sha256`, and `SHA256SUMS`

`INPUT_REQUEST.md` is the complete request retained inside the archive for independent review.

## Local package validation

From the extracted package root:

```text
python3 tools/validate_package.py .
```

The validator checks the exact file set and hashes, current SHA, version-1 markers, decision closure, required owners/APIs, forbidden unresolved alternatives and placeholders, matrix sizes/classes, structural-absence assertions, and manifest integrity.

## Verification boundary

Repository source and maintained documentation were inspected at the current SHA above. The archive itself is generated, hashed, extracted, and machine-validated. Because this is design-only and contains no repository checkout or production overlay, Rust compilation, Clippy, nextest, docs, AOT, parity, and Tier-2 suites are implementation acceptance gates, not claims made by this return. Exact commands and required rows are in `VERIFICATION_SCOPE.md` and `TEST_MATRIX.csv`.
