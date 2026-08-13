# arcweft-lang-01.3.1.2.3.2.1.2.1-generation-bound-producer-root-and-awbc-admission-authority-correction-final-contract

## Status

`READY_FOR_IMPLEMENTATION`

This is the decision-complete, design-only Lang-01.3.1.2.3.2.1.2.1
correction. It narrows and corrects the returned Lang-01.3.1.2.3.2.1.2
package before that package's G2 catalog authority or the parent A4 unchecked
nominal-value constructor deletion may be accepted.

This package contains no production source, patch, implementation overlay,
compatibility reader, migration shim, branch, or pull-request material.
`OPEN_QUESTIONS.txt` is exactly `OPEN_QUESTIONS=0`.

## Normative precedence

When files appear to overlap, apply them in this order:

1. `FINAL_CONTRACT.md`
2. `RUST_OWNERS_AND_APIS.md`
3. `CANONICAL_GRAMMARS.md`
4. `GENERATION_IDENTITY_AND_CORRELATION.md`
5. `PRODUCER_ROOT_CONTRACT_AND_TRAVERSAL.md`
6. `CHARACTER_DIALOGUE_ROLE_AND_CUSTOM_CONTRACT.md`
7. `CHARACTER_DIALOGUE_VOICE_AND_BRANCH_GRAMMAR.md`
8. `RUNTIME_PLAN_ADMISSION.md`
9. `AWBC_PRODUCT_ADMISSION_AND_CODEC.md`
10. `EXECUTION_API_MIGRATION.md`
11. `ERROR_AND_PRECEDENCE.md`
12. `IMPLEMENTATION_ORDER.md`
13. inventory, traceability, and test-matrix files.

The supplied request is copied byte-for-byte as `SOURCE_REQUEST.md`.

## Retained substrate

The following decisions remain fixed and are not reopened:

- A1-A3 nominal layout, expression evaluation, field-ID, anonymous-record,
  record-column, defining-order, and canonical-byte contracts;
- crate-private
  `RuntimeNominalRecordValue::try_from_accepted_layout`;
- final A4 deletion of public unchecked `RuntimeNominalRecordValue::new` and
  `validate_shape`;
- a non-Serde operational admission handle rather than a public raw
  nominal/layout/fields constructor;
- CharacterDialogue as one exact `std.character_dialogue` opaque value with
  tuple18 payload, tuple2 custom entries, and direct inline-failure variant;
- descriptor-aware normalize/clear/patch with atomic publication;
- the accepted G1 closed-variant checks for owner, ordinal, name, payload
  presence, and payload checked type;
- no `RuntimeCheckedType::Dynamic`, producerless opaque fallback, source/name/
  hash reconstruction, copied descriptor table, dual reader, or version bump.

## Corrections made here

The `.1.2` producer row is no longer self-authorizing. Each serialized producer
contract contains independent canonical roots and a claimed authorization set;
admission derives the set from the roots and requires exact equality. The
claimed set can diagnose missing or extra keys but cannot legitimize either.

One serialized `RuntimeGenerationContractDeclaration` is embedded identically
in raw `RuntimePlan` and raw `AwbcProgram`. One non-Serde
`AdmittedRuntimeGeneration` owns the operational nominal catalog, producer
shape views, CharacterDialogue role/custom facts, catalog correlations, and
generation identity. Runtime plan and AWBC wrappers borrow the same aggregate
when paired.

CharacterDialogue role types come from one typed accepted semantic-fact owner.
The six base roles are accepted by typed role coordinates; `Style` is
normatively the source-ordered choice
`Choice([EntityRef, RichText])`. No role name is parsed.

The custom-field runtime digest is computed from the exact closed descriptor
map. A caller cannot supply an unrelated digest.

Raw `RuntimePlan` and raw `AwbcProgram` remain serializable quarantine carriers,
but no raw object reaches VM, fiber, product-step, runtime-session, restore, or
publication APIs.

## Version policy

Every Arcweft-owned schema, ABI, codec, digest-domain, protocol, product,
persistence, bundle, save, replay, and runtime version remains exactly `1`.
The unreleased payload is directly replaced; no compatibility path is retained.

## Verification boundary

The package archive, parent ZIP hash, source-request byte identity, JSON/CSV
parsing, manifest, fresh extraction, and deterministic ZIP reproduction are
verified by the package builder.

The exact pinned source and scoped repository instructions were inspected
statically. No production checkout was modified. Cargo, Clippy, rustfmt,
structural audit, and Tier 2 execution are not claimed as run in this
design-only environment; the exact required commands and implementation
acceptance gates are normative in `IMPLEMENTATION_ORDER.md`.
