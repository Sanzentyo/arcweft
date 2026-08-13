# Final contract

## 1. Precedence and narrow supersession

This is Lang-01.3.1.2.3.2.1.2.1. It supersedes only these defective `.1.2`
decisions:

- producer authorization rows containing only producer ID plus catalog keys;
- public arbitrary construction of seven CharacterDialogue runtime role types;
- caller-supplied custom-catalog digest independent of descriptors;
- lifetime-only generation correlation;
- raw `AwbcProgram` verification as sufficient execution admission;
- an ID lookup described as producer-owned authority.

All other `.1.2` representation, validation, transformation, persistence, and
deletion decisions remain normative.

## 2. D1 — one non-circular producer payload contract

The serialized authority owner is
`arcweft_core::plan::producer_contract::RuntimeProducerPayloadContractDeclaration`.

Each declaration contains:

1. one exact `RuntimeOpaqueTypeProducerId`;
2. one typed `RuntimeProducerPayloadRootSet`;
3. one sorted, duplicate-free claimed
   `RuntimeNominalRecordCatalogKey` authorization set.

`RuntimeProducerPayloadRootSet` is either:

- `CheckedRoots`, containing independently accepted semantic root coordinates
  paired with closed `RuntimeCheckedType` values; or
- `CharacterDialogue`, containing the complete typed standard-role facts,
  custom-field declaration, Character-catalog digest, and View-catalog digest.

The claimed key set is not evidence. Admission traverses the independent roots,
derives the exact key set, and requires equality. A missing key, extra key,
duplicate key, or key absent from the canonical layout catalog is a typed
failure before any handle exists.

## 3. D2 — exact root vocabulary and traversal

A generic root coordinate is `RuntimeProducerRootId([u8; 32])`, emitted by
accepted semantic facts. CharacterDialogue roots use the closed
`CharacterDialogueRuntimeRole` enum and
`CharacterDialogueCustomFieldId`; they are generated from the typed payload and
cannot be supplied as generic string coordinates.

Project roots use `RuntimeProjectRootId([u8; 32])` plus one closed checked type.
Producer roots and project roots are separate. Producer roots cannot make a
project layout reachable and a claimed authorization row cannot make its own
root reachable.

Traversal is deterministic, iterative, bounded, and defined in
`PRODUCER_ROOT_CONTRACT_AND_TRAVERSAL.md`. It follows every `Choice` branch,
variant case, result branch, tuple element, sequence/option child, and nominal
field in canonical order. Opaque values are atomic after exact owner validation.
Every nominal node resolves one catalog key and descends through the
catalog-owned defining-order fields.

## 4. D3 — accepted semantic-fact owner and bridge

`arcweft_interaction_model::dialogue::CharacterDialogueRuntimeRole` is the
shared closed coordinate enum.

The existing semantic `TypeKind` enum gains its own
`CharacterDialogueRole(CharacterDialogueRuntimeRole)` variant. This is an
accepted-semantic coordinate, not a runtime checked type. The original
`TypeKind` implementation performs role substitution; no string helper,
extension trait, or ad-hoc `"DialogueStage"` recognizer is introduced.

`arcweft_lang_sema::character_dialogue::AcceptedCharacterDialogueRuntimeTypes`
is the sole accepted semantic owner. It is tied to one
`AcceptedNominalWorldStamp`, records source/type evidence for six base roles,
requires exactly one role declaration per coordinate, and derives `Style`
rather than accepting a second independent definition.

`arcweft_runtime_plan::semantic_facts::RuntimeCharacterDialogueProducerFacts`
is the sole closed runtime projection. It owns seven exact
`RuntimeCheckedType` values, the projected custom declaration, source evidence,
and accepted-world correlation. `RuntimePlanSemanticFacts` produces it through
one inherent checked projection API.

## 5. D4 — seven exact CharacterDialogue role types

The final role set for one generation is:

| Role | Exact final checked type |
|---|---|
| Stage | checked projection of the accepted typed `Stage` declaration |
| Portrait | checked projection of the accepted typed `Portrait` declaration |
| Focus | checked projection of the accepted typed `Focus` declaration |
| Cleanup | checked projection of the accepted typed `Cleanup` declaration |
| Hook | checked projection of the accepted typed `Hook` declaration |
| RichText | checked projection of the accepted typed `RichText` declaration |
| Style | `RuntimeCheckedType::Choice(vec![RuntimeCheckedType::EntityRef, rich_text.clone()])` in that source order |

The six base declarations must already be closed after alias resolution and
must not contain unresolved `TypeKind::Named` or
`TypeKind::CharacterDialogueRole`. Runtime projection rejects either leak.
Current `"DialogueStage"`, `"DialoguePortrait"`, `"DialogueFocus"`,
`"DialogueCleanup"`, `"DialogueHook"`, and `"RichTextStyle"` rows are directly
replaced by typed role coordinates and accepted declarations; they are never
recognized by spelling.

The tuple18 root wraps Stage, Portrait, Focus, and Cleanup in the existing outer
`Option`. Hook is the element type of the hooks sequence. Style and RichText
are direct fields.

## 6. D5 — role-type construction boundary

`CharacterDialogueRuntimeRoleTypes` has private fields, no public `new`,
`Default`, Serde, or arbitrary builder. It is a borrowed operational view
issued only by
`AdmittedRuntimeGeneration::character_dialogue()` after whole-generation
admission. It carries the aggregate generation identity and an exact reference
to the admitted role declaration.

Raw role declarations remain serializable data inside the generation contract.
They are not operational and cannot construct a schema or value.

## 7. D6 — custom-field declaration and digest

The raw owner is
`CharacterDialogueRuntimeCustomFieldCatalogDeclaration`. Its public
`try_from_checked_projection` takes descriptors only and computes the digest.
It does not accept a digest argument.

Raw Serde still carries the claimed digest so tampering is diagnosable.
Admission recomputes it from:

- strict ascending field ID;
- canonical bytes of the exact closed `RuntimeCheckedType`;
- `clearable`;
- strict ascending accepted View IDs.

The BLAKE3 domain is
`arcweft.character-dialogue-runtime-custom-fields.v1\0`. Count and length
fields are little-endian `u32`. Source spans and source binding locations are
not runtime semantics and are not included; sema retains its own source
evidence separately. Any semantic descriptor change changes the runtime digest.

## 8. D7 — exact voice and branch grammar

Tuple index 5 remains nested, never flattened:

```text
Option::None
Option::Some(CharacterDialogueVoice::Auto)
Option::Some(CharacterDialogueVoice::Id(EntityRef))
```

Outer Option owner/cases are exactly `Option`, `Some` ordinal 0 with payload,
and `None` ordinal 1 without payload.

The inner owner is exact
`arcweft.dialogue.CharacterDialogueVoice`; `Auto` is ordinal 0 without payload
and `Id` is ordinal 1 with one `EntityRef` payload. Canonical bytes use
the existing nested RuntimeValue variant encoding.

Variant checks are owner, ordinal, exact name, payload presence, then payload
type. Runtime `Choice` selection evaluates all branches under budget: exactly
one success is required; zero gives `ChoiceNoMatch`, more than one gives
`ChoiceAmbiguous`. Admission traversal always walks all branches.

## 9. D8 — generation identity and correlation

`RuntimeGenerationIdentity([u8; 32])` is owned by
`arcweft_core::plan::generation_contract`. It is the BLAKE3 digest of the
canonical generation-contract body under domain
`arcweft.runtime-generation-contract.v1\0`.

The raw declaration stores a claimed identity; admission recomputes and
compares it. Equality is all 32 bytes. When joining separately carried
artifacts, canonical body bytes must also be byte-identical; equal identity
with unequal bytes is `GenerationContractCollision`.

Existing runtime-driver `GenerationId(u64)` remains a host slot and is not
semantic identity. `ProgramGeneration`, generation image, save/restore context,
Character/View catalog admissions, plan/AWBC wrappers, and CharacterDialogue
schema all retain and compare `RuntimeGenerationIdentity`.

## 10. D9 — exact RuntimePlan fields and admission

`RuntimePlan` gains one private Serde field:

```text
generation_contract: RuntimeGenerationContractDeclaration
```

The `.1.2` standalone catalog field and producer-only rows are replaced by the
single declaration. Other current fields remain unchanged.

`RuntimePlan::try_admit(self)` follows the complete order in
`RUNTIME_PLAN_ADMISSION.md`: current plan verification; producer/role/custom
declaration validation; custom digest; catalog consistency; independent project
and producer traversal; exact authorization equality; missing/extra/conflict/
unreachable checks; generation identity/correlation; atomic creation of
`AdmittedRuntimeGeneration` and `AdmittedRuntimePlan`.

No catalog, handle, runtime, callback, ownership traversal, or decoded value is
published before the last step.

## 11. D10 — AWBC product grammar

`AwbcProgram` gains one non-public field with a public read-only accessor:

```text
generation_contract: RuntimeGenerationContractDeclaration
```

It is encoded immediately after the current AWBC header and before the string
table. It includes the same catalog layouts, project roots, producer payload
contracts, CharacterDialogue role/custom facts, Character/View correlations,
claimed authorization sets, and generation identity as the source RuntimePlan.

The AWBC canonical body and product digest include those bytes. ABI and codec
versions remain `1`; there is no old reader.

Runtime-plan lowering constructs the declaration once and clones the same value
into RuntimePlan and AWBC products. Canonical body bytes must be identical.

## 12. D11 — admitted AWBC and one aggregate

`AwbcProgram::try_admit(self)` returns non-Serde `AdmittedAwbcProduct`.
Standalone admission builds one `Arc<AdmittedRuntimeGenerationInner>` from the
embedded declaration.

`AdmittedRuntimePlan::try_admit_awbc(&self, raw_program)` first performs AWBC
header/structural checks, then requires byte-identical generation contract, and
reuses the plan's existing `Arc`. It never builds a second catalog.

There is no API that combines an already independently admitted product with a
plan. A runtime generation image owns exactly one admitted aggregate.

## 13. D12 — raw execution API closure

Public VM step, host step, fiber construction/resume, product-step construction,
session activation, hot swap, restore, player startup, and codegen entry points
no longer accept raw `AwbcProgram` or raw `RuntimePlan`.

Low-level VM/fiber functions become crate-private and take
`&AdmittedAwbcProduct`. Public convenience constructors may consume a raw
artifact only when they perform complete atomic admission before publishing a
runtime object. They may not store raw input, call only `verify()`, expose
`Deref`, `into_inner`, raw replacement, or restore bypass.

The exact decisions for every named API are in
`EXECUTION_API_MIGRATION.md`.

## 14. D13 — CharacterDialogue schema from one generation

The only schema constructor is
`CharacterDialogueRuntimeSchema::try_from_generation`.

It accepts:

1. one borrowed `RuntimeCharacterDialogueProducerShape` issued by the admitted
   aggregate;
2. one admitted Character catalog view with the same generation identity and
   exact declared digest;
3. one admitted View registry with the same generation identity and exact
   declared digest.

It checks generation, exact producer, role/custom declaration identity and
digest, Character/View correlations, nominal lookup/authorization, and nested
field validity before schema publication. Encode, decode, digest, equality,
hashing, patch, restore, and replay all use that same schema/aggregate.

## 15. D14 — typed errors and evidence

Core owns generation-contract, root-traversal, catalog, lookup, tree, generation
mismatch, RuntimePlan, and AWBC admission errors. Sema/runtime-plan errors retain
role coordinate, accepted type ID, and source span. Dialogue errors retain
generation, role/custom coordinate, `RuntimeValuePath`, and typed sources.
Driver/bundle/save/replay/View errors wrap sources instead of stringifying them.

The two required precedence chains are exact in
`ERROR_AND_PRECEDENCE.md`.

## 16. D15 — ID-only producer lookup

The selected model is a **non-exclusive admitted-shape view**, not a caller
identity credential.

`AdmittedRuntimeGeneration::producer_shape(producer_id)` may return a borrowed
`RuntimeNominalRecordProducerShape`. It can validate and construct values only
for the exact authorization set independently derived and admitted for that
producer. Any code already holding the admitted aggregate may request the view;
the security property is canonical shape authority, not Rust caller identity.

The type has private fields, no public constructor, Serde, `Default`, or
generation-erasing conversion. Raw IDs, keys, descriptors, layouts, and claimed
rows cannot create it. The specialized
`RuntimeCharacterDialogueProducerShape` is issued without an ID lookup and
carries the same shape view plus admitted role/custom facts.

## 17. D16 — deletion and implementation cut

The final cut deletes:

- self-authorizing producer-only declarations and any admission path that
  trusts their keys;
- `.1.2` public arbitrary `CharacterDialogueRuntimeRoleTypes::new`;
- caller-supplied custom runtime digest;
- raw executable plan/AWBC VM, fiber, product-step, driver, player, and restore
  paths;
- duplicate operational catalogs for one plan/AWBC generation;
- generation-blind role/custom/Character/View catalog combinations;
- ID-only producer lookup described as an exclusive credential;
- public unchecked nominal `new`, `validate_shape`, and every descriptorless
  nominal reconstruction required by parent A4;
- stale compatibility aliases, old readers, old tuple/nominal dialogue
  representations, and source-name fallback.

`IMPLEMENTATION_ORDER.md` defines compile-clean gates that avoid any interval in
which unchecked publication remains reachable.
