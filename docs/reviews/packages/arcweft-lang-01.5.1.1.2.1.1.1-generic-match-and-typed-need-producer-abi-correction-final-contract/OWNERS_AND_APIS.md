# Owners and API map

| Responsibility | Sole owner | Normative API / row |
|---|---|---|
| HIR Match structure/source spans | `arcweft-lang-hir` | `HirMatchExpr`, `HirMatchArm`, source index |
| Match checked meaning | `arcweft-lang-sema` | `CheckedExpressionResolution::Match(Box<CheckedMatch>)` |
| Match type/effect facts | existing final-analysis maps | `CheckedExpression`, `CheckedPattern`, `CheckedBinding` |
| Checked-Match digest/reference | `arcweft-lang-sema` | `checked_match_digest`, `CheckedMatchRef` |
| Ownership classification | `RegisteredSemanticWorld` / sema | inherent `checked_ownership(&TypeKind)` |
| Checked View reference | sema checked View catalog | `CheckedViewNeedMatch.match_fact` only |
| Sema-to-runtime codegen projection | `arcweft-compiler` | constructs `RuntimeViewMatchSelectorSeed` |
| Runtime codegen seed/final row | `arcweft-runtime-plan` | `RuntimeViewMatchSelectorSeed`, `RuntimeViewMatchSelector` |
| One runtime type graph | `RuntimePlan` / `AwbcInventory` | type seeds -> `RuntimePlanTypeId` -> `AwbcTypeId` |
| Selector construction | runtime-plan AWBC lowerer | `ViewMatchSelectorBuilder::lower` |
| Selector result verification | `arcweft-core::awbc` | `AwbcProgram::verify_view_match_selector` |
| View Match coordinates | `arcweft-view` | `ViewMatchSite` coordinate rows |
| Static View/AWBC join/source roles | `arcweft-bundle` | `ViewReactiveBindingSectionV1` |
| Active join validation | `arcweft-runtime-driver` | `VerifiedViewReactiveBindings` |
| Selector decode/local transaction | runtime-driver | private decode + `LocalInstallTransaction` |
| Checked Need runtime type | `arcweft-core::pattern` | `RuntimeCheckedType::Need` |
| Need identity | `arcweft-core::task` | fixed-byte `NeedId::derive` |
| Need live carrier | `arcweft-core::value` | `RuntimeValue::NeedHandle(RuntimeNeedHandle)` |
| Need AWBC type/instruction | `arcweft-core::awbc` | typed NeedHandle + MakeNeedHandle 0x1e |
| Producer verification/execution | core AWBC verifier/VM | verify/invoke Need producer |
| Producer plan construction | runtime-plan `AwbcInventory` | one plan/signature/type interner |
| Active generation binding | runtime-driver | private extract/VerifiedNeedHandle |
| Producer state | existing driver journal | key `(GenerationId, NeedId)` |
| Resource registry/digest | `arcweft-resource-model` | existing registry/integrity/digest |
| Semantic catalog input | `arcweft-lang-sema` | fallible `FinalSemanticCatalogs::production` |
| Snapshot DTO | core `value::awbc_save` | dedicated NeedHandle snapshot variant |
| Bundle digest/generation | bundle/runtime-driver | content root + ProgramGeneration |

## Visibility

Generic semantic facts and lightweight View coordinates are public for compiler/tooling. Runtime-plan seed/final rows are public only across compiler/runtime-plan crate boundary. AWBC schemas are core product rows. Bundle DTOs are public bundle APIs with unknown fields denied. Decoded selection, install transaction, verified binding index, active-generation Need handle, and start construction remain runtime-driver-private.

RuntimeNeedHandle construction is restricted to verified VM code; read-only accessors are public core APIs. No public API exposes selector registers/frames, mutable journal rows, or unverified NeedId extraction.

## Inherent implementation rule

Behavior is added directly to Arcweft-owned `CheckedExpressionResolution`, `RegisteredSemanticWorld`, `RuntimeCheckedType`, `AwbcRuntimeType`, `RuntimeValue`, `AwbcProgram`, `AwbcInventory`, and `NeedId`. No extension trait, wrapper-only workaround, String helper, fallback resolver, or parallel enum authority is part of the contract.
