# Human-readable validation

## Result

**PASS — design package validation**

- Package: `arcweft-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction-final-contract`
- Current source SHA: `4bda1cdcea63fdf7aac32691d756c1c0e1fc693e`
- Exact decisions: 17 closed
- Open questions: exactly `none`
- Source evidence rows: 48
- Requirement traceability rows: 17
- Producer/consumer rows: 40
- Deletion rows: 32
- Test rows: 148
- Arcweft-owned version markers selected by this contract: 1 only

## Rejection conditions exercised by the package validator

The validator rejects a missing payload, unexpected payload, hash mismatch, unresolved decision count, wrong current SHA, non-`none` open questions, undersized evidence/traceability/test matrices, missing test class, unresolved normative placeholder in the exact Rust schemas, absent single-result selector decision, absent dedicated Need carrier/String rejection, missing guard branch-chain closure, missing resource-registry equality, missing strict deletion, or manifest set mismatch.

## Design-specific semantic checks

Human review confirms the package makes one selection for each previously open ABI choice:

- synthetic nominal Variant rather than Choice/multi-result/register export;
- private driver selection rather than a public View/core value type;
- explicit guard Branch chain rather than the currently split AWBC Match guard field;
- typed NeedHandle value rather than String;
- opcode `0x1e`, flag bit 4, and tag 19 payload grammar;
- fixed-byte NeedId plus existing journal rather than a second endpoint table;
- sema TypeKind and existing runtime projection rather than inferred TypeId/copied map;
- exact current HIR arm fields rather than an invented arm expression;
- existing ResourceTypeRegistry digest rather than a copied digest authority; and
- five compile-clean cuts ending in one atomic strict-v1 deletion.

## Scope honesty

This PASS is for the archive's internal consistency, source grounding, requirement coverage, and hashes. It is not a claim that production Rust code has been implemented or compiled. The implementation acceptance gates are explicitly listed in `VERIFICATION_SCOPE.md` and represented as executable rows in `TEST_MATRIX.csv`.
