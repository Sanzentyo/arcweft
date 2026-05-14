# Hook Manifest Schema

Hook Manifest は、DSL / Rust export / macro-generated hook を共通に表す schema である。

```rust
pub struct HookManifest {
    pub schema_version: u32,
    pub hook_id: EntityId,
    pub public_id: PublicId,
    pub target: HookTarget,
    pub phase: HookPhase,
    pub check: CheckPolicy,
    pub condition: Option<ExprSummary>,
    pub priority: i32,
    pub once: bool,
    pub purity: HookPurity,
    pub effects: Vec<EffectCapability>,
    pub memo: Option<HookMemoPolicy>,
    pub contracts: Vec<ContractSummary>,
    pub source: Option<SourceAnchor>,
}
```

```rust
pub enum HookTarget {
    Entity(EntityId),
    Layer(LayerId),
    UiNode(EntityId),
    Signal(EntityId),
    StatePath(StatePathId),
    Pattern(TargetPattern),
}

pub enum CheckPolicy {
    EveryFrame,
    EveryFrames(u32),
    EveryLogical(Duration),
    OnChange(Vec<DependencyRef>),
    OnSignal(EntityId),
    OnEvent(EventPattern),
    OnTaskReady(TaskId),
    OnLayerVisible(LayerId),
    Manual,
    Any(Vec<CheckPolicy>),
    All(Vec<CheckPolicy>),
}

pub struct HookMemoPolicy {
    pub kind: HookMemoKind,
    pub scope: MemoScope,
    pub key: Vec<MemoKeyPart>,
}

pub enum HookMemoKind {
    ConditionOnly,
    ComputedLocals,
    Disabled,
}
```

## JSON example

```json
{
  "schema_version": 1,
  "public_id": "hook.opening.choice_enable",
  "target": { "Entity": "choice.opening.listen" },
  "phase": "input.hit_test",
  "check": { "OnChange": ["state.affection[@character.alice]"] },
  "condition": "state.affection[@character.alice] >= 3",
  "priority": 10,
  "purity": "Command",
  "effects": ["ui.enable", "log.debug"],
  "memo": {
    "kind": "ConditionOnly",
    "scope": "StateHash",
    "key": ["state.affection[@character.alice]"]
  }
}
```
