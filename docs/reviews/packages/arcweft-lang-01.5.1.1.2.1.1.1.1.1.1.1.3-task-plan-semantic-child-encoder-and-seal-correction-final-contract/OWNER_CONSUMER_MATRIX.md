# Owner and consumer matrix

## 1. Sole owners

| Concern | Sole final owner | Create authority | Read authority | Forbidden duplicate |
|---|---|---|---|---|
| structured static task row | `arcweft_core::plan::RuntimeTaskPlan` | `RuntimePlanBuilder::push_runtime_task_plan` | immutable RuntimePlan/table getters | compiler/bundle task row copy |
| binding tag/payload for non-View | `RuntimeTaskSemanticBinding` | core lowering/builder with inherent family validation | core encoder | side tag table |
| completed structured task digest | `RuntimeTaskPlanTable` association | common private sealer | table lookup and typed key copy | self field on row |
| executable digest | `RuntimePlanSemanticEncoder` | private owner pass | opaque child getter/base | caller executable hash |
| producer-function digest | actual resolved `RuntimeFunctionSite` through encoder | private owner pass | opaque base getter | compiler-supplied digest |
| request-template digest | inherent `RuntimeHostTaskRequestTemplate` semantic method | private semantic context | opaque base getter | extension trait/free helper |
| control/effect digest | inherent `RuntimeControlEffectContract` semantic method | private semantic context | opaque base getter | effect side catalog hash |
| View program/revision | current `arcweft_view` identity owners retained by validated resource | View/bundle validators | validated resource | core projection |
| stable View site/admission | compiler-local Cut 3 + validated binding | sema/compiler and bundle join | validated View authority | copied raw bytes in core |
| decoded expected key | private purpose-built codec image | strict decoder | common sealer comparison only | public expected field |
| task-plan build coordinate | core builder/decoded image | returned owner-bound token | lowering/bundle/sealer | caller numeric constructor |
| duplicate index | final `RuntimeTaskPlanTable` | after all rows sealed/expected verified | RuntimePlan lookup | parallel family tables |

## 2. Consumer migration inventory

| Consumer | Final input | Required change | Deletion after cutover |
|---|---|---|---|
| core `RuntimePlan` | one sealed `RuntimeTaskPlanTable` | add table to final immutable object only | provisional/parallel catalog |
| core `RuntimePlanBuilder` | static task rows and optional authority | push rows returns build coordinate; finish invokes common sealer | caller digest parameter, early table publication |
| core plan verification | static rows + sealed table | validate coordinate/family/binding before hash and final references after hash | checks against self digest field |
| core task identity | completed table key in `NeedProducerSpec` | consume sealed key only | plan digest derivation in identity code |
| core Need producer templates | table key/reference | resolve coordinate/index during final projection | caller-filled plan digest |
| core snapshot authority | stored expected bytes + sealed table | resolve bytes against existing key | raw digest constructor |
| core AWBC plan owner | its accepted owner-tag-1 encoder/table | preserve separate legitimate AWBC owner; join structured references by typed APIs | accidental structured-table duplication |
| core line plan owner | accepted `LinePlanSemanticDigest` | provide Line binding payload; owner-tag-2 plan remains separate | line task self digest |
| sema checked producer functions | accepted semantic function/effect facts | expose typed products consumed by runtime-plan lowering | prehashed task-plan fields |
| sema endpoint/child-role facts | source-independent checked roles | feed request-template owner | source spelling/HIR coordinate transcript |
| compiler Cut 3 View catalog | actual program/revision/site/admission | retain compiler-local row; pair with build coordinate | serialization of local lookup evidence |
| runtime-plan lowering | final checked products | construct static row, receive coordinate, never compute final digest | hash helper/extension trait |
| runtime-plan executable encoder | private complete candidate | implement fixed 15-table transcript and memoized row visitors | map-key/self-digest input |
| bundle View validation | actual View types + Cut 3 rows + coordinates | create exact `ValidatedViewTaskPlanBinding` map | raw projection newtypes |
| `ValidatedViewProgramResource` | actual current program and bindings | implement sole `ViewTaskPlanAuthority` | secondary View task catalog |
| runtime resource codec | private row images and expected keys | strict decode then common seal | generic Serde RuntimePlan/task row |
| bundle outer loader | private plan + View images | validate View first, seal core, publish atomically | independently published section handles |
| Need producer instance construction | typed completed plan digest | no transcript change; obtain key from table | recomputation/caller bytes |
| snapshot verification | typed active RuntimePlan and optional current View resource | resolve/check sealed key and authority freshness | expected-key trust |
| generated binary schemas | version-one static row/table codec | regenerate in Cut 5 | old field/reader/schema alias |
| generated fixtures/goldens | deterministic sealed keys | regenerate from same current source | stale provisional digest bytes |
| maintained runtime docs | final owner/dependency/publication model | update in same Cut 5 | provisional row/table prose |
| structural gates | Cargo graph + rustdoc/trybuild + artifact diff | assert forbidden APIs/types absent | source-spelling-only acceptance grep |

## 3. Read/write authority by lifecycle

| Lifecycle | Mutable owner | May construct typed digest? | May observe table? |
|---|---|---:|---:|
| sema/checked analysis | compiler-local facts | no | no |
| runtime-plan lowering | `RuntimePlanBuilder` | no | no |
| bundle View join | private validated candidate | no final digest; owns actual View inputs | no |
| common seal | private `RuntimePlanSemanticEncoder` + optional validated authority | yes | private partial rows only |
| expected verification | private sealer | no new digest from expected bytes | private sealed vector |
| final publication | final constructors | no fallible creation afterward | yes |
| runtime Need/task use | immutable RuntimePlan + task authorities | no recomputation | yes |
| snapshot decode | private decoded image + active authority | same common seal only | only after publication |

## 4. Fields deliberately absent from final static row

| Field | Why absent | Where retained instead |
|---|---|---|
| task-plan semantic digest | self authority/cycle | table key association after sealing |
| expected digest | codec assertion | private decoded image |
| producer contract | explicitly excluded | `NeedProducerSpec`/contract owner |
| producer site | explicitly excluded | producer instance input |
| payload type | explicitly excluded | runtime type/producer instance |
| evaluated arguments/digest | explicitly excluded | canonical RuntimeValue owner/producer instance |
| generation/policy/ordinal | runtime correlation | journal/task identity owner |
| priority/cancel scope | scheduling | final TaskSpec |
| debug label/source text | diagnostics | source/debug owners |
| View program/site/admission | dependency inversion | validated upper binding |
| View accepted revision | validation, not identity | validated View resource/replacement owner |
