# Rust owner/API index

| Owner module | New or changed owner/API |
|---|---|
| `arcweft_character::catalog` | `CharacterCatalogRuntimeDigest`, errors/field enums, `CharacterCatalog::runtime_digest_v1` |
| `arcweft_view::view::identity` | `ViewId::projected_runtime_id_v1` returning `ProjectedRuntimeViewId` |
| `arcweft_view::view::registry` | Arcweft implementation revision, `ViewRegistryRuntimeDigest`, errors, `ViewRegistry::runtime_digest_v1` |
| `arcweft_interaction_model::dialogue` | inherent role arrays/tag/authored predicate on existing enum |
| `arcweft_lang_sema::types` | `TypeKind::CharacterDialogueRole` and owning projection behavior |
| `arcweft_lang_sema::character_dialogue::runtime_types` | role declaration, accepted role set, exact errors |
| `arcweft_runtime_plan::semantic_facts::runtime_roots` | project/producer facts, typed coordinates/source evidence |
| `arcweft_core::plan::generation_contract` | lossless root-ID constructors on current newtypes |
| `arcweft_core::plan::project_root` | project-capable errors and project root declaration validation |
| `arcweft_core::plan::typed_roots` | plan site/slot enums and root-use rows |
| `arcweft_core::plan::nominal_admission` | borrowed project/producer admission domain and issuance |
| `arcweft_core::pattern` | typed validator, paths, structured errors, unique Choice behavior |
| `arcweft_core::awbc::schema` | semantic runtime type declarations, AWBC typed sites/uses, nominal domain table, MakeRecord operand |
| `arcweft_core::awbc::{codec,verify,vm,fiber,product_step}` | version-1 LE grammar, admission-only execution, typed errors |
| `arcweft_runtime_driver::generation_catalogs` | lossless lower-digest projection and same-generation catalog wrappers |
