# Rust owner/API index

| Owner | Final declaration/API |
|---|---|
| `arcweft_core::value::ownership::path` | sole `RuntimeValuePath`, `RuntimeValuePathSegment`, `RuntimeValuePathError`; existing inherent API plus `OpaquePayload`; custom Serde tag `10` |
| `arcweft_core::pattern` | non-Serde `RuntimeCheckedTypePath`, `RuntimeCheckedTypePathSegment`, `RuntimeCheckedTypeError`, `RuntimeValueShape`; original `RuntimeCheckedType` inherent `validate_value`, `encode_canonical_v1`, and `decode_canonical_v1` |
| `arcweft_core::plan::admission` | final `AdmittedRuntimeGeneration`, `AdmittedRuntimePlan`, domain-bound `RuntimeCheckedValueContext`, generation/type/root resolution APIs |
| `arcweft_core::plan::typed_sites` | checked `RuntimeIndexPath`, `RuntimeTypedExpr`, `RuntimeTypedPattern`, `RuntimePlanTypeDeclaration`, `RuntimePlanTypedSite`, and exact slot enums |
| `arcweft_core::plan` and current entry/stream/source/task owners | mandatory typed fields listed in `RUNTIME_PLAN_SITE_RESOLUTION.csv` |
| `arcweft_core::awbc::typed_sites` | `AwbcTypedSite`, all closed slot enums, and coordinate-only `AwbcTypedOrigin` |
| `arcweft_core::awbc::schema` | `AwbcRuntimeTypeDeclaration`, `AwbcTypedConstant`, `AwbcTypedPattern`, typed frame slots, and mandatory table fields |
| `arcweft_core::awbc::verify` / `arcweft_core::awbc::admission` | exact bounds/duplicate/cycle/site resolution; `AdmittedAwbcProduct`; standalone and same-parent admission |
| `arcweft_core::runtime_product` | `AdmittedRuntimeProduct`, direct plan/AWBC equality transcript, and admitted pair publication |
| `arcweft_core::awbc::product_step` | `AwbcProductStepExecutor` owns `AdmittedAwbcProduct`; admitted replacement and accessor; raw program replacement/accessor deleted |
| `arcweft_dialogue::character_dialogue::catalogs` | `CharacterDialogueGenerationCatalogs<'generation>` layer-correct bridge |
| `arcweft_lang_sema::character_dialogue` | `CharacterDialogueRuntimeRoleRegistry` issued atomically from `AcceptedNominalWorld` |
| current sema registrar | one `TypeCheckEnv` → `AcceptedNominalWorld` → role registry → `RegisteredTypeCheckEnv` publication transaction |

Exact checked-type tags and nested slot tags are normative in `RUNTIME_CHECKED_TYPE_V1_BYTE_GRAMMAR.md`, `RUNTIME_CHECKED_TYPE_TAGS.csv`, `RUNTIME_PLAN_SLOT_ENUMS_AND_TAGS.md`, and `AWBC_SLOT_ENUMS_AND_TAGS.md`. Exact admitted wrapper and pair APIs are normative in `ADMISSION_AND_PAIR_API.md`.

Every behavior added to an Arcweft-owned enum is implemented on that enum's original inherent `impl`; no helper trait, extension trait, string resolver, or parallel enum is introduced.
