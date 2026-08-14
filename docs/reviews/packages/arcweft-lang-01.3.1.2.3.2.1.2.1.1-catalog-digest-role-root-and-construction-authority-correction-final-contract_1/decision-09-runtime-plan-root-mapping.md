# Decision 09 — exhaustive `RuntimePlan` typed-root mapping

## Serialized site and use types

Owner: `arcweft_core::plan::typed_roots`.

```rust
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeIndexPath(Box<[u32]>);

impl RuntimeIndexPath {
    pub fn try_new(segments: impl Into<Box<[u32]>>) -> Result<Self, RuntimePlanTypedSiteError>;
    pub const fn segments(&self) -> &[u32];
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "table", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimePlanTypedSite {
    Entry { entry: u32, slot: RuntimeEntryTypedSlot },
    CallableExecutable { executable: u32, slot: RuntimeCallableTypedSlot },
    FlowExecutable { executable: u32, slot: RuntimeFlowExecutableTypedSlot },
    FlowOp { flow: u32, path: RuntimeIndexPath, slot: RuntimeFlowOpTypedSlot },
    PureHelper { helper: u32, slot: RuntimePureHelperTypedSlot },
    TraitMethod { method: u32, slot: RuntimeTraitMethodTypedSlot },
    LineTaskNode { group: u32, path: RuntimeIndexPath, slot: RuntimeLineTaskTypedSlot },
    StreamPlan { plan: u32, slot: RuntimeStreamTypedSlot },
    SourcePlan { plan: u32, slot: RuntimeSourceTypedSlot },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePlanTypedRootUse {
    site: RuntimePlanTypedSite,
    root: RuntimeProjectRootId,
}
```

All fields are private. `RuntimeIndexPath` rejects an empty path and any path that does not resolve through the exact current nested owner. `RuntimePlanTypedRootUse::try_new` is public only within core/runtime-plan lowering; accessors return borrowed site and copied root. `RuntimePlan` gains mandatory `generation_contract: RuntimeGenerationContractDeclaration` and `typed_root_uses: Vec<RuntimePlanTypedRootUse>` fields; no defaults are supplied at deserialization.

The slot enums are exact and are included in `RUNTIME_PLAN_TYPED_SITE_ENUMS.md`. `RUNTIME_PLAN_ROOT_MAPPING.csv` contains one row for every slot and every deliberate exclusion.

## Admission equality

For each typed plan publication boundary, lowering obtains its accepted `RuntimeNormalizedType`, derives `RuntimeSemanticTypeId`, emits or reuses the matching `RuntimeProjectRootDeclaration`, and emits exactly one site-use row. Admission sorts a candidate copy of site uses by `RuntimePlanTypedSite`, rejects duplicate/noncanonical sites, resolves every site against the corresponding current table/nested path, requires exact root-byte equality and exact canonical checked-type equality, and rejects any declared project root that is not reached from at least one valid site. Producer authorization rows do not count as project reachability.

Replay/save-visible state uses the same underlying entry/frame/local/capture/root site as live execution; there is no second persistence root vocabulary. View input values use entry/flow/custom-field typed sites, not `ViewId` as a type root. Resource IDs without runtime payload, display/source maps, source spans, task IDs, flow IDs, public labels, and other identity-only fields are deliberately untyped and emit no root use.
