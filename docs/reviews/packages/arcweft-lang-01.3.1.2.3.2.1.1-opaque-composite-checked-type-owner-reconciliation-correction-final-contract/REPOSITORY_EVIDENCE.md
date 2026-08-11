# Repository evidence

## 1. Revision and transport

- Repository: `Sanzentyo/arcweft`
- Branch requested: `main`
- Inspected Git commit: `a38c736ba577172b1f4c3fe1a0c3e85443e97e6f`
- Jujutsu change ID: unavailable from the Git object/static web view
- Root `AGENTS.md`: byte-inspected from the pinned commit snapshot
- Rust Skill: read completely before design
- Full local checkout/cargo execution: not performed

GitHub's commit/tree and exact file pages were used to establish the revision
and retrieve targeted source. The execution container could not resolve
`github.com`, `raw.githubusercontent.com`, `codeload.github.com`, or
`api.github.com` through its own network namespace, so no claim is made that a
full repository clone, Cargo metadata graph, compilation, Clippy, or workspace
test was executed. The exact targeted source set and hashes are in
`SOURCE_MANIFEST.csv`; `validation/transport_failure.txt` records the observed
DNS failure.

## 2. Direct implementation facts

| Path | Line | Observed symbol/text | File SHA-256 |
| --- | --- | --- | --- |
| crates/arcweft-core/src/pattern.rs | 93 | pub enum RuntimeCheckedType | e8d7feae80e062cf55eef50ad1125c1d0c8aab70a41d74d6e1993b8d4cf532ae |
| crates/arcweft-core/src/pattern.rs | 144 | pub fn accepts_variant_case | e8d7feae80e062cf55eef50ad1125c1d0c8aab70a41d74d6e1993b8d4cf532ae |
| crates/arcweft-core/src/pattern.rs | 410 | fn runtime_value_matches_pattern_type | e8d7feae80e062cf55eef50ad1125c1d0c8aab70a41d74d6e1993b8d4cf532ae |
| crates/arcweft-core/src/value.rs | 136 | pub enum RuntimeValue | cfb6f24e50f17932a63f5cf9c818e1b92c9fdbc221477f5fbcb8b633d94b43e1 |
| crates/arcweft-core/src/awbc/schema.rs | 11 | pub const AWBC_ABI_VERSION | 3837106f5b571256ba56a040dbfd410a5cbcbff857b3d7c46df526c4620f1a09 |
| crates/arcweft-core/src/awbc/schema.rs | 16 | pub const AWBC_CODEC_VERSION | 3837106f5b571256ba56a040dbfd410a5cbcbff857b3d7c46df526c4620f1a09 |
| crates/arcweft-core/src/awbc/codec/types.rs | 224 | Self::Dynamic => writer.write_u8(20) | 0a56d879d1bd7ae35ee758dc86a30f2a74eba1c5647d10bdd078ca87b47d7e26 |
| crates/arcweft-core/src/awbc/codec/types.rs | 233 | writer.write_u8(22) | 0a56d879d1bd7ae35ee758dc86a30f2a74eba1c5647d10bdd078ca87b47d7e26 |
| crates/arcweft-runtime-plan/src/semantic_facts.rs | 73 | pub enum RuntimeTypeShape | 0586061ab665750d71b5b3495f76138bfd4ea3e3f07bbff9f23c50b1ad67bace |
| crates/arcweft-runtime-plan/src/semantic_facts.rs | 164 | pub fn checked_type | 0586061ab665750d71b5b3495f76138bfd4ea3e3f07bbff9f23c50b1ad67bace |
| crates/arcweft-runtime-plan/src/semantic_facts.rs | 489 | pub struct RuntimeResolvedVariant | 0586061ab665750d71b5b3495f76138bfd4ea3e3f07bbff9f23c50b1ad67bace |
| crates/arcweft-lang-sema/src/env/nominal.rs | 70 | pub enum AcceptedNominalSemantics | e924bece98e6317728e0b3651e4ed74171e648feff695760a09de66ddc23521e |
| crates/arcweft-lang-sema/src/env/nominal.rs | 673 | ("ArcError", TypeKind::Named | e924bece98e6317728e0b3651e4ed74171e648feff695760a09de66ddc23521e |
| crates/arcweft-lang-sema/src/env/nominal.rs | 700 | standard_opaque_record("Reduction" | e924bece98e6317728e0b3651e4ed74171e648feff695760a09de66ddc23521e |
| crates/arcweft-lang-sema/src/types.rs | 468 | CharacterDialogue(CharacterDialogueType) | 984df396eca07e1067dfeff40f4bc781cdd99d60d768f85e0be44bc07a6e1c96 |
| crates/arcweft-dialogue/src/character_dialogue/schema.rs | 105 | pub struct CharacterDialogueRuntimeSchema | 78c1a287cb442d2fdb0a9be45d42b497a164c19c71d6a03944cbfa956e5d000d |
| crates/arcweft-dialogue/src/character_dialogue/schema.rs | 340 | pub fn into_runtime_value | 78c1a287cb442d2fdb0a9be45d42b497a164c19c71d6a03944cbfa956e5d000d |


## 3. Result-changing defects confirmed

1. `RuntimeCheckedType` has no opaque leaf and the native matcher is still a
   private free function.
2. Runtime-plan has bare `Opaque`/`Named` shapes and returns string projection
   failures, so recursive composites cannot truthfully close opaque leaves.
3. Standard `ArcError`, `ReducerError`, `AgentError`, and other domain atoms are
   accepted through `TypeKind::Named`; `Reduction` is producerless Opaque.
4. `CharacterDialogueRuntimeSchema` receives an expected layout from its caller
   and its value currently becomes a raw nominal record.
5. AWBC is ABI 1 / codec 10 at this commit, has runtime type tags through 22,
   and has no opaque row/value constant.
6. Result/Option are complete variant rows and the verifier has no variant
   covariance, confirming selected-case `Never` refinement is invalid.
7. Existing Serde/fiber/snapshot carriers transit `RuntimeValue`, so adding a
   runtime value variant is persisted and requires a version decision.

## 4. Parent authority verification scope

The three supplied searchable parent mirror documents were byte-inspected and
hashed. The parent ZIP bytes themselves were not available in the execution
filesystem, so the declared retained-byte SHA `4b15a5eaea31663a9323f41f75345b2acb6faa0ea3a61784eeeabd482a13966a` is recorded but
not independently recomputed. The correction preserves the parent's nominal
layout/API decisions and explicitly rebases only stale/current contradictions.

## 5. Design-only boundary

No production source was edited. The ZIP allow-list contains only Markdown,
CSV, JSON, TXT, and Python validation/reference-model material. The Python
files are package validators/reference models, not repository source or an
implementation overlay.
