# Owners, dependency direction, and APIs

## Authority chain

```text
syntax/HIR identities
 -> FinalSemanticAnalysis::CheckedViewCatalog
 -> compiler scratch View/AWBC product
 -> strict ValidatedViewProduct
 -> immutable runtime ViewProgramCatalog
 -> generation-bound Need journal
 -> generic Match/AWBC RuntimeValue execution
 -> shared BundleViewFrame
 -> native/Web/headless/Agent/generated consumers
```

No arrow points upward. `arcweft-view` remains Sans I/O and does not depend on
`arcweft-core`. `arcweft-bundle` owns strict cross-section DTOs. Runtime-driver
joins lightweight View coordinates to existing core Need/AWBC owners.

| Fact/behavior | Sole owner | Consumer | Forbidden duplicate |
|---|---|---|---|
| checked Need match | `CheckedViewCatalog` | compiler/tooling lookup | compiler/source reconstruction |
| session subscription key | `CheckedViewNeedSubscriptionKey` | catalog | string/span hash |
| product subscription | containing program's `ViewNeedSubscriptionId` | validated table | global/mount surrogate |
| replacement join | semantic ID + contract digest | reconciler | dense ID across revisions |
| producer/Need binding | verified AWBC function/task + `NeedId` | runtime binder | endpoint copy |
| publication selection | generation-bound journal | observers | backend/per-mount selector |
| Need state projection | inherent `RuntimeNeedState` impl | AWBC selector | extension trait/View helper |
| pattern/guard/binding | AWBC pattern/match tables | generic Match | View matcher |
| occurrence | `ViewMountAllocator`/`ViewMountId` | observer key | program as occurrence |
| start | AWBC product-step + task registry | host dispatch | lower-layer I/O |
| cancellation | existing task/cancel scope | runtime host owner | last-observer cancel |
| static proof | checked View static disposition | compiler/certificate | Need-only proof |
| save/replay | session save + same journal API | restore/replay | replay-only state machine |
| replacement | runtime scratch transaction | atomic swap | in-place mutation |

## Required inherent APIs

Arcweft-owned behavior is added to its original inherent `impl`, not an ad-hoc
extension trait or helper detour.

```rust
impl CheckedViewCatalog {
    pub fn need_match(
        &self,
        key: CheckedViewNodeKey,
    ) -> Option<&CheckedViewNeedMatch>;

    pub fn need_subscription(
        &self,
        key: CheckedViewNeedSubscriptionKey,
    ) -> Option<&CheckedViewNeedSubscription>;
}

impl RuntimeNeedState {
    pub fn cursor(&self) -> TaskPublicationCursor;

    pub fn canonical_state_digest(
        &self,
        contract: &RuntimeNeedMatchContract,
        budget: &mut RuntimeValueBudget,
    ) -> Result<RuntimeNeedStateDigest, RuntimeNeedProjectionError>;

    pub fn project_match_value(
        &self,
        contract: &RuntimeNeedMatchContract,
        budget: &mut RuntimeValueBudget,
    ) -> Result<RuntimeValue, RuntimeNeedProjectionError>;
}

impl ViewNeedRuntime {
    pub fn bind_observer(
        &mut self,
        request: BindViewNeedObserver<'_>,
    ) -> Result<BindViewNeedOutcome, ViewNeedRuntimeError>;

    pub fn apply_publication_batch(
        &mut self,
        generation: GenerationId,
        publications: &[RuntimeNeedState],
        limits: ViewNeedRuntimeLimits,
    ) -> Result<ViewNeedPublicationOutcome, ViewNeedRuntimeError>;

    pub fn snapshot_v1(
        &self,
        limits: ViewNeedSnapshotLimits,
    ) -> Result<ViewNeedRuntimeSnapshotV1, ViewNeedSnapshotError>;

    pub fn restore_v1(
        snapshot: &ViewNeedRuntimeSnapshotV1,
        catalog: &ViewProgramCatalog,
        limits: ViewNeedSnapshotLimits,
    ) -> Result<Self, ViewNeedSnapshotError>;
}
```

Producer journal key is `(GenerationId, NeedId)`. Observer key is
`(ViewMountId, ViewNeedSubscriptionId)`. One producer may have many View and
non-View observers; each View observer retains independent cursor, arm, locals,
and invalidation revision.
