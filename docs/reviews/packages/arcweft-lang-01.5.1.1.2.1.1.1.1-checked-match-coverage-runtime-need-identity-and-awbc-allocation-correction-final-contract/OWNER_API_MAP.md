# Owner and API map

| Concern | Sole final owner | Constructible API/boundary | Deleted parallel route |
|---|---|---|---|
| Opcode number | `arcweft_core::awbc::schema::AwbcOpcode` | discriminants, `ALL`, `encoded`, `from_encoded` | codec numeric matches and feature-local maps |
| Function kind tag | `AwbcFunctionKind` | discriminants, `ALL`, inherent numeric Serde/Wire | `wire_enum!` copied tag table |
| Function flag bit | `AwbcFunctionFlag` | enum bit position | mask literals in callers |
| Function flag set | `AwbcFunctionFlags` | `empty/with/contains/bits/try_from_bits` | public tuple field/raw constructor |
| Canonical binary | `arcweft_core::awbc::codec` | `AwbcProgram::encode_canonical/decode_canonical` | payload temporary Vec and raw DTO |
| VM execution | `arcweft_core::awbc::vm` | existing `step` / `step_with_host` | invented nominal VM owner |
| Logical Need identity | `arcweft_core::task::NeedId` | family-specific crate-owned derivation | String constructor/parser/suffix |
| Task coalescing | `TaskKey` | generation+NeedId+policy transcript | String public ID key |
| Task launch | `TaskId` | TaskKey+launch ordinal transcript | formatted launch label |
| Task plan producer | `AwbcTaskPlan::producer` | verifier-recomputed `AwbcTaskProducer` | `AwbcTaskPlan.need_id` |
| Typed Need value | `RuntimeValue::NeedHandle` | `RuntimeNeedHandle::from_verified_producer` | String/Dynamic/payloadless carrier |
| Coverage | `MatchCoverageAnalyzer` | private `analyze` called inside `CheckedMatch::try_from_hir` | caller-supplied coverage |
| Checked generic Match | `CheckedExpressionResolution::Match` | final-analysis construction/completeness | View-only copied Match fact |
| Ownership | `CheckedOwnershipContext` | symbols+world+resources classification | `RegisteredSemanticWorld`-only guess |
| Project nominal layout | `ProjectSymbolTable` | exact accepted declaration and substitutions | source path lookup |
| Accepted opaque evidence | `AcceptedNominalSemantics::Opaque` | original enum variant with producer/value/persistence | side table/extension trait |
| Resource facts | `ResourceTypeRegistry` | existing integrity + digest + exact records | copied endpoint/type registry |
| Stable Match coordinate | View program/revision/site/arm/output owners | compiler one-way projection | persisted HIR IDs |
| Checked Match digest | final-analysis semantic encoder | exact BLAKE3 transcript | debug/canonical-bytes placeholder |
| Runtime-plan staging | existing `RuntimePlanSemanticFactInput` | `try_insert_view_match_selector` | nonexistent `RuntimeSemanticFactInput` |
| Bundle join | bundle static View/AWBC join owner | digest/type/function cross-check | View VM/register export |
| Runtime generation binding | runtime-driver | install verified producer and active key | generation in core handle |

## Visibility

Numeric and semantic constructors that could fabricate authority are private or
`pub(crate)`. Read-only accessors for fixed IDs/digests are public where shared
crates require them. Presentation methods are one-way. Codec construction goes
through verified fixed-byte constructors; semantic construction goes through
final analysis and runtime-plan admission.

## Crate-graph compliance

`arcweft-lang-sema` depends on semantic symbols/world/resource model but not on
runtime-plan or View. `arcweft-compiler` sees final semantic analysis, HIR,
View, and runtime-plan and performs the projection. `arcweft-runtime-plan`
accepts the closed seed vocabulary and has no sema dependency. `arcweft-view`
remains core-independent. Bundle/runtime-driver are the only static/dynamic join
layers.
