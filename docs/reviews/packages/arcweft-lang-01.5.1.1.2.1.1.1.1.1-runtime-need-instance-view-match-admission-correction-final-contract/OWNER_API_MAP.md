# Owner and API map

| Concern | Final owner | Owning crate/module | Sole construction/API | Consumers | Deleted/rejected owner |
|---|---|---|---|---|---|
| Generation identity | `GenerationId` | `arcweft-core::task` | `GenerationId::new` | runtime-driver, journal, save/replay, adapters | runtime-driver::swap::GenerationId |
| Producer family | `NeedProducerFamily` | `arcweft-core::task` | `inherent semantic_tag/from_semantic_tag` | compiler, runtime-plan, AWBC, host | feature-local family tags |
| Producer contract | `NeedProducerContractDigest` | `arcweft-core::task` | `NeedProducerContractDigest::for_input` | compiler/runtime-plan/task host | string producer names as identity |
| Task-plan meaning | `TaskPlanSemanticDigest` | `arcweft-core::task` | `owning plan semantic_digest` | producer instance, bundle, restore | AwbcTaskPlan.plan_digest |
| Runtime type meaning | `RuntimeTypeSemanticDigest` | `arcweft-core::pattern` | `RuntimeCheckedType inherent semantic_digest` | TaskSpec/outcome/producer | copied runtime type maps |
| Runtime argument identity | `RuntimeValueDigest` | `arcweft-core::entry` | `RuntimeValue::try_digest` | all producer instances/AwaitMany | duplicate digest type/ZERO empty |
| Producer instance | `NeedProducerInstanceKey` | `arcweft-core::task` | `NeedProducerInstance::try_new` | TaskSpec, task host, journal | source-derived/revision-derived identity |
| Terminal cell | `NeedId` | `arcweft-core::task` | `NeedId::try_for` | handle/state/journal/timeout | String/suffix identity |
| Coalescing group | `TaskKey` | `arcweft-core::task` | `TaskKey::try_for` | ensure_task/group journal | ordinal-bearing key |
| Actual launch | `TaskId` | `arcweft-core::task` | `TaskId::try_for` | events/tasks/adapters | caller task id |
| Launch ordinal | `TaskLaunchOrdinal` | `arcweft-core::task` | `journal allocation` | correlation/save/replay | caller/random ordinal |
| Correlation | `TaskCorrelation` | `arcweft-core::task` | `TaskHost::ensure_task` | all runtime/host/persistence envelopes | partial conversion DTOs |
| Need carrier | `RuntimeNeedHandle` | `arcweft-core::task/value` | `try_reusable_join / try_from_accepted_launch` | RuntimeValue/Await/timeout/snapshot | NeedHandle String |
| Task launch | `TaskHost::ensure_task` | `arcweft-core trait; scheduler implementation` | `journal transaction` | all task producers | identity-supplying adapter |
| Task event | `TaskEvent` | `arcweft-core::task` | `host publication envelope` | engine/journal/observer/replay | task-id-only event |
| Need state | `RuntimeNeedState` | `arcweft-core::task` | `journal event application` | Await/View/save/replay | uncorrelated state |
| Generic Match | `CheckedMatch` | `arcweft-lang-sema` | `CheckedMatch::try_from_hir` | ordinary lowering/View ref | caller-provided coverage |
| Coverage | `MatchCoverageAnalyzer` | `arcweft-lang-sema private` | `analyze` | CheckedMatch only | copied View coverage |
| Match digest | `CheckedMatchSemanticDigest` | `arcweft-lang-sema` | `CheckedMatch construction` | View admission/compiler | HIR/View identity inputs |
| Ownership | `RegisteredSemanticWorld::checked_ownership` | `arcweft-lang-sema` | `inherent method` | View/producer admission | ResourceTypeRegistry route/extension trait |
| Opaque evidence | `AcceptedNominalSemantics::Opaque` | `arcweft-lang-sema` | `input->registrar->record` | catalog digest/ownership/runtime plan | side table/default inference |
| Producer admission | `CheckedNeedProducerAdmission` | `arcweft-lang-sema/compiler` | `try_new from exact args/captures` | View admission/task product | contract identity construction |
| View admission | `CheckedViewMatchAdmission` | `arcweft-lang-sema/compiler boundary` | `CheckedViewMatchAdmission::try_new` | View catalog/bundle/runtime | generic Match gating |
| View site | `ViewMatchSiteId` | `arcweft-lang-sema/view projection` | `for_checked_path` | coordinate/task-plan/replacement | HIR/span/revision seed |
| View program | `ViewProgramId` | `arcweft-view` | `current owner` | catalog/bundle/runtime | ViewProgramSemanticDigest |
| View revision | `AcceptedViewProgramRevision` | `arcweft-view` | `current semantic transcript owner` | catalog/bundle/registry/replacement only | u32 revision/Need identity |
| AWBC task producer row | `AwbcNeedProducerRow` | `arcweft-core::awbc` | `verifier/compiler` | VM/bundle/restore | AwbcTaskPlan.need_id |

## Owner rules

- Every fixed identity transcript is implemented as an inherent method on its
  identity owner.
- Every closed Arcweft enum gains its missing row in the original inherent
  implementation. No extension trait, copied match, or feature-local helper
  becomes behavior authority.
- Compiler projection may carry private validated byte/string projections
  across a dependency boundary, but such projections expose no constructor
  that can mint semantic identity.
- Runtime-driver coordinates transactions and adapters; it does not own core
  identity or type conversion DTOs.
- Bundle/save/replay store expected digests and final fields; they never become
  a second generator of those digests.
