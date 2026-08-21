# arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract

## Final status

**READY_FOR_IMPLEMENTATION — DESIGN ONLY — OPEN_QUESTIONS=0**

This archive is the complete corrected design contract for checked Match coverage,
runtime Need/task identity, and the collision-free AWBC version-1 allocation. It
is not a patch, implementation overlay, compatibility package, or delta-only
response. Production source, tests, fixtures, manifests, branches, and pull
requests were not modified.

- Inspected repository: `Sanzentyo/arcweft`
- Frozen inspected `origin/main`: `c49099fb154d9e3dbb587e1bcd7ee243214da0c4`
- Required archive name: `arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract.zip`
- Current request copy: `inputs/CURRENT_REQUEST.md`
- Current request SHA-256: `8bf22dbee57a94ee178e25d0004be7a18694a8b801ef79189da3f9e1a3741299`
- Current request Git blob: `a1411adcf7f2c9651f250d9db3302d3ab61ddfa7`
- Every Arcweft-owned version marker selected here: `1`
- Compatibility readers, aliases, dual carriers, and source reconstruction: none

## Reading order

1. `FINAL_CONTRACT.md`
2. `DECISION_REGISTER.md`
3. `RUST_SCHEMAS.md`
4. `OWNER_API_MAP.md`
5. `DEPENDENCY_GRAPH.md`
6. `AWBC_ALLOCATION_AND_WIRE.md`
7. `NEED_TASK_IDENTITY.md`
8. `CHECKED_MATCH_COVERAGE.md`
9. `OWNERSHIP_AND_PERSISTENCE.md`
10. `CHECKED_MATCH_DIGEST.md`
11. `PERSISTENCE_REPLAY_REPLACEMENT.md`
12. `FAILURE_PRECEDENCE_AND_ATOMICITY.md`
13. `COMPILE_CLEAN_SEQUENCE.md`
14. `SOURCE_EVIDENCE.md`
15. `REQUIREMENT_TRACEABILITY.md`
16. `TEST_MATRIX.md`
17. `STRUCTURAL_ABSENCE.md`
18. `VERIFICATION_SCOPE.md`
19. `VALIDATION.md`
20. `OPEN_QUESTIONS.md`

CSV files are the complete tabular projections. `machine/*.json` files are the
validator-readable normative projections. Human prose and machine rows are
required to agree; the validator rejects drift.

## Closed architecture

The package selects one authority per concern:

- `AwbcOpcode`, `AwbcFunctionKind`, `AwbcFunctionFlag`, and
  `AwbcFunctionFlags` in `arcweft-core` own all numeric AWBC decisions.
- `NeedId`, `TaskKey`, and `TaskId` become fixed 32-byte BLAKE3 identities.
  `AwbcTaskPlan.need_id` is replaced by a mandatory typed producer row rather
  than deleted before its consumers have a replacement.
- `MatchCoverageAnalyzer` is the only constructor of
  `CheckedMatchCoverage`; callers cannot provide an exhaustiveness bit or an
  unreachable-arm set.
- `CheckedOwnershipContext` receives the exact `ProjectSymbolTable`,
  `RegisteredSemanticWorld`, and `ResourceTypeRegistry`; no copied type or
  persistence side table is introduced.
- HIR arena IDs remain session-only lookup coordinates. Product identity uses
  the accepted View program/revision/site/arm/output coordinate family and an
  exact semantic digest transcript.
- Runtime-plan projection uses the existing `RuntimePlanSemanticFactInput`.
  AWBC execution extends the existing functional `awbc::vm::step` and
  `step_with_host` entry points; this design does not invent an `AwbcVm` owner.

## Retained accepted behavior

The selector still returns one owning synthetic nominal Variant whose selected
case is the source arm and whose payload is a source-ordered binding Tuple;
zero bindings use an empty Tuple. Guards lower through explicit pattern, bind,
ordinary guard, and Branch control flow. `arcweft-view` remains independent of
`arcweft-core`, no callee register escapes, typed Need uses a dedicated runtime
value, generation binding belongs to runtime-driver, and
`ResourceTypeRegistry` remains the sole resource authority.

## Validation entry point

```text
python tools/validate_package.py .
python tools/validate_package.py ../arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract.zip
```

The validator is Python standard-library only. It validates the exact request
copy, all structured decisions, evidence ranges, tests, structural absence,
manifest hashes, and safe ZIP shape. `VALIDATION_OUTPUT.txt` records the final
commands actually executed for this return.
