# Rust API index

| Owner module | Final public/non-public surface |
|---|---|
| `arcweft_core::plan::typed_sites` | `RuntimePlanTypeId`, `RuntimePlanTypeDeclaration`, `RuntimePlanTypeKind`, `RuntimeOperationalType`, `RuntimeTypedExpr`, `RuntimeTypedPattern`, plan coordinates/sites; private fields; public checked constructors |
| `arcweft_core::plan::construction` | `RuntimePlanBuilder`, `RuntimePlanBuildError`; `RuntimePlan` custom v1 decode and read-only accessors |
| `arcweft_core::awbc::typed_sites` | typed declarations/constants/patterns/origins, `AwbcTypedSite`, exact nested slot enums; private fields and checked constructors |
| `arcweft_core::awbc::construction` | `AwbcProgramBuilder`, `AwbcProgramBuildError`; `AwbcProgram` custom v1 decode and accessors |
| `arcweft_core::plan::generation_admission` | projection rows/builder, `AdmittedRuntimeGeneration`, public non-forgeable `require_same_parent`, plan admission, catalog digest provenance |
| `arcweft_core::plan::admission` | `AdmittedRuntimePlan`, resolved sites, checked contexts/domains, pair admission |
| `arcweft_core::awbc::admission` | `AdmittedAwbcProduct`, AWBC site resolution/verification |
| `arcweft_core::awbc::product` | `AdmittedRuntimeProduct`, direct origin correlation |
| `arcweft_runtime_plan` | legitimate external lowerer calling public checked builders; no core-private access |
| `arcweft_compiler::project::runtime_generation` | compiler-internal accepted-world assembly and full atomic compile convenience |
| `arcweft_dialogue::character_dialogue::catalog_admission` | borrowed `CharacterDialogueGenerationCatalogs` plus owned `AdmittedCharacterDialogueCatalogs` |
| `arcweft_runtime_driver::generation_runtime` | `RuntimeDriverGeneration`, generation-first load, admitted executor construction |
| `arcweft_runtime_driver::swap` | prepared/atomic cross-generation swap |
| `arcweft_runtime_driver::session_save` | generation-header-first restore/replay, product-issued checked contexts |
| `arcweft_bundle::product_awbc::runtime_generation` | `VerifiedRuntimeGenerationSections`; AWFB section-set verification before any generation/plan/AWBC content decode |

Every behavior added to an Arcweft-owned enum is an inherent method on that
enum or its legitimate owner context. No extension trait, string resolver, or
parallel command/type enum is introduced.
