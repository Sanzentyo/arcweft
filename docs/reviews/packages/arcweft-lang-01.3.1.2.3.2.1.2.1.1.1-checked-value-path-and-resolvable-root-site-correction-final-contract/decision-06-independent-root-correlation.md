# Decision 06 — independent root correlation and tamper checks

## Independently verified sources

Project facts originate only from accepted semantic HIR facts and the final runtime-plan lowerer. Producer facts originate only from accepted opaque producer declarations and the accepted CharacterDialogue role/custom registry. These facts are assembled into `AdmittedRuntimeGeneration` before any raw plan or AWBC admission.

Raw `RuntimePlanTypeDeclaration` and `AwbcRuntimeTypeDeclaration` rows may name an already admitted fact but cannot add one. The generation owner checks exact semantic ID, losslessly projected root ID, checked type, producer identity where applicable, and source generation. Project and producer error owners remain distinct.

## Deleted self-authorization rows

The following retry types and fields are deleted:

- `RuntimePlanTypedRootUse` and `RuntimePlan.typed_root_uses`;
- `AwbcTypedRootUse` and `AwbcProgram.typed_root_uses`;
- any row containing `site + root + checked type` as its own proof;
- any standalone root map derived only from a raw artifact.

The replacement `AwbcTypedOrigin` stores only `{ plan_site, awbc_site }`. It has no root, semantic ID, checked type, dense type ID, generation scalar, or domain. It is useful for pair correlation and diagnostics but has zero construction authority.

## Plan admission tamper checks

For every actual owner field, the plan verifier derives the required type from literal/value identity, lexical binding, accepted signature, constructor descriptor, pattern expectation, or enclosing result contract. It compares the derived type ID to the mandatory field/wrapper row and then resolves the declaration against the admitted generation.

Therefore:

- changing only the field's type ID fails owner/type equality;
- changing only its declaration fails generation resolution or another site using the declaration;
- changing both fails structural derivation unless the actual owner is also changed into a different valid artifact;
- adding/removing a wrapper node fails the exact path-set check;
- inventing a root use is impossible because no such table exists.

## AWBC admission tamper checks

The verifier derives type from frame slots, signatures, typed constants, typed patterns, exact instruction fields, and indirect table references. It resolves every declaration against the admitted generation. Changing only a runtime declaration, only a frame/signature/constant/pattern field, or both together cannot pass unless the entire actual AWBC program remains structurally type-correct and names an already admitted semantic type.

## Direct plan-to-AWBC equality transcript

Pair admission obtains one resolved row for every plan site and one resolved row for every `AwbcTypedOrigin`. It never hashes them. Each normalized row is:

```text
u32_le(plan_site_bytes.len)
|| plan_site_bytes
|| RuntimeSemanticTypeId[32]
|| u32_le(checked_type_bytes.len)
|| checked_type_bytes defined by RUNTIME_CHECKED_TYPE_V1_BYTE_GRAMMAR.md
|| authority_tag:u8
|| authority_payload
```

`plan_site_bytes` is encoded exactly by `PLAN_SITE_CANONICAL_TAGS.csv`, `RUNTIME_PLAN_NESTED_SLOT_TAGS.csv`, and `RUNTIME_PLAN_COORDINATE_STEP_TAGS.csv`. `authority_tag=0x00` encodes `RuntimeProjectRootId[32]`. `authority_tag=0x01` encodes `u32_le(producer_id_utf8_len) || producer_id_utf8 || RuntimeProducerRootId[32]`. The checked-type bytes are exactly `RUNTIME_CHECKED_TYPE_V1_BYTE_GRAMMAR.md` and `RUNTIME_CHECKED_TYPE_TAGS.csv`; this correction adds no type digest.

Plan rows are sorted by canonical site bytes and unique. AWBC origins are sorted by `(plan_site, awbc_site)` and unique. Each AWBC site is independently resolved; multiple AWBC sites for one plan site are allowed only when every normalized row after the site coordinate is byte-identical. They collapse to one plan row. Missing plan site, extra origin plan site, disagreement, duplicate pair, unresolvable site, or conflicting AWBC rows fails pair admission.

Standalone AWBC admission validates its generation, type declarations, all actual sites, nominal domains, and origin syntax. It does not treat `plan_site` as authority and does not claim equality to an absent plan. Only pair admission publishes `AdmittedRuntimeProduct<'generation>` with plan-equivalence evidence.

`PLAN_AWBC_EQUALITY_GRAMMAR.md` pins site tags and scalar encodings. No additional digest, root map, optional fallback, or compatibility reader is introduced.

## Exact publication API

`ADMISSION_AND_PAIR_API.md` defines the non-Serde pair wrapper and consuming `AdmittedRuntimePlan::try_admit_awbc`. The pair wrapper stores only admitted plan/product owners and coordinate correlation; equality has no digest, root map, optional field, or independently constructible row.
