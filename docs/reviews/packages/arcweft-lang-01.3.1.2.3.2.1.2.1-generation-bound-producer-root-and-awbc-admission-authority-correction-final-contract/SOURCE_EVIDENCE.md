# Current-source evidence used by this correction

## 1. Repository state

Design authority was inspected at:

- current audit commit:
  `50771a19f57f86570837f616a66252be24e77e0c`;
- independently accepted closed-variant correction:
  `1648894fbfc38ba623d1b01c6001fbd55b67b10b`;
- returned `.1.2` production parent:
  `98ccafa5f0113a50f8a0f5e985df5f695c401588`.

The returned `.1.2` ZIP supplied with the request was inspected locally and its
SHA-256 was recomputed as:

```text
7a7001cba41f312d428a88589877ce48eb3bb6734aff234b72601d7bfa6a9d70
```

## 2. Instructions and architecture

The root and scoped `AGENTS.md`, docs index/review guidance, crate map, and test
execution policy were inspected at the pinned source.

The resulting owner placement follows current layering:

- `arcweft-core`: Sans I/O runtime types, plan, AWBC, verification, operational
  authority;
- `arcweft-interaction-model`: lower shared domain coordinate;
- `arcweft-lang-sema`: accepted semantic source facts;
- `arcweft-runtime-plan`: checked projection and lowering;
- `arcweft-dialogue`: CharacterDialogue domain/schema;
- `arcweft-runtime-driver`: generation images, session, restore, activation.

## 3. Core plan and nominal values

Inspected:

- `crates/arcweft-core/src/plan.rs`;
- `crates/arcweft-core/src/plan/entry_inventory.rs`;
- `crates/arcweft-core/src/value/nominal_record.rs`;
- `crates/arcweft-core/src/pattern.rs`;
- current value ownership/path evidence.

Findings:

- raw `RuntimePlan` is Clone/Serde and has no generation-contract/catalog
  authority;
- `RuntimePlan::verify` is structural;
- `RuntimePlanError` already has an owning enum that must be extended;
- `RuntimeNominalRecordValue::new` is public and unchecked;
- `try_from_accepted_layout` is crate-private;
- `validate_shape` is a second shallow authority;
- `validate_against_layout` retains exact identity/layout/count/field order;
- current `RuntimeCheckedType::Choice` uses boolean-any acceptance;
- commit `1648894...` closes exact nominal Variant owner/ordinal/name/payload
  checks and remains accepted.

## 4. AWBC

Inspected:

- `crates/arcweft-core/src/awbc/schema.rs`;
- `codec.rs`;
- `verify.rs`;
- `type_projection.rs`;
- `vm.rs`;
- `fiber.rs`;
- `product_step.rs`;
- core executor and bytecode conversion surfaces.

Findings:

- `AwbcProgram` is independently Serde and executable and contains no generation
  contract;
- ABI and codec versions are `1`;
- structural verifier does not create operational authority;
- public VM step/host-step accept raw program;
- public fiber construction accepts raw program;
- `MakeRecord` reads a raw program layout and invokes the crate-private
  nominal primitive;
- `AwbcProductStepExecutor` owns raw program and raw replacement calls verify;
- `ArcweftRuntimeExecutor::{from_awbc_product,
  from_awbc_product_function, replace_product_awbc_program}` expose raw
  product paths;
- `BytecodeProgram` converts raw plans/programs without admission.

## 5. Runtime-plan and semantic facts

Inspected:

- `crates/arcweft-runtime-plan/src/semantic_facts.rs`;
- final plan lowering and AWBC lowering/inventory evidence.

Findings:

- `RuntimePlanSemanticFacts` already owns normalized closed vocabulary and the
  interned `Arc<RuntimeNominalRecordLayout>` map needed for one canonical
  catalog;
- unresolved `TypeKind::Named` fails checked runtime projection rather than
  becoming a valid checked type;
- the current map/facts are not emitted as one generation authority shared by
  plan and AWBC.

## 6. Sema CharacterDialogue facts

Inspected:

- `crates/arcweft-lang-sema/src/character_dialogue.rs`;
- `crates/arcweft-lang-sema/src/callable/schema/families.rs`;
- accepted nominal/opaque fact evidence.

Findings:

- standard role rows currently use `TypeKind::Named` values such as
  `DialogueStage`, `DialoguePortrait`, `DialogueFocus`, `DialogueCleanup`,
  `DialogueHook`, and `RichTextStyle`;
- current style intent is source-ordered
  `Choice(EntityRef, RichTextStyle)`;
- the accepted custom registry owns field ID, type, clearability, accepted
  Views, accepted-world/source evidence, and a semantic digest;
- there is no single accepted owner publishing all standard role types as
  executable closed facts.

## 7. CharacterDialogue runtime

Inspected:

- `crates/arcweft-dialogue/src/character_dialogue.rs`;
- `character_dialogue/schema.rs`;
- `typed_value.rs`;
- `patch.rs`.

Findings:

- current runtime schema uses root nominal/layout authority and independent raw
  catalogs;
- current custom runtime catalog accepts caller digest separately from
  descriptors;
- typed wrappers permit raw/direct construction and deserialization paths;
- normalize/empty/patch contain descriptorless behavior;
- patch uses a dialogue-local path;
- current voice encoding is nested `encode_option(... encode_voice ...)`, which
  resolves the split decision in favor of nested Option;
- `.1.2` remains authoritative for replacing root/custom/inline physical
  representations.

## 8. Runtime-driver and cross-crate surfaces

Inspected current driver generation/session/save/View source evidence and the
request-named direct surfaces. Findings include:

- existing host `GenerationId(u64)` is a runtime slot, not a canonical
  generation contract;
- `GenerationRuntimeImage`/session state does not yet enforce the shared
  semantic identity described here;
- save/restore and driver errors can flatten typed program/value failures;
- broad bundle/save/View/player/agent/JIT/AOT consumers require compile-clean
  migration after the core API cut.

## 9. Evidence classification

`PRODUCER_CONSUMER_DELETION_INVENTORY` marks rows based on:

- exact pinned source inspection;
- returned package evidence;
- request-mandated call-site closure;
- workspace compilation/typed-test closure.

Search is discovery evidence only. Final closure is compile/test/structural
evidence at implementation time.
