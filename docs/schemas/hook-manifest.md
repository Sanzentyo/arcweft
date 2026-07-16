# Internal Dispatch Manifest Schema

This schema records typed owner-local dispatch after lowering. It is not an
author-facing declaration format. View, line-plan, source, Activity, and host
adapter owners produce records whose event kinds and effects have already been
checked.

```rust
pub struct DispatchManifest {
    pub schema_version: u32,
    pub owner_id: EntityId,
    pub event: DispatchEventKind,
    pub phase: DispatchPhase,
    pub stable_order: StableDispatchOrder,
    pub effects: Vec<EffectCapability>,
    pub contracts: Vec<ContractSummary>,
    pub source: Option<SourceAnchor>,
}
```

The manifest does not carry raw target, condition, phase, or cache-policy
strings. Owner-specific typed lowering supplies those facts, while subsystem
cache metadata is reported separately.

## JSON example

```json
{
  "schema_version": 1,
  "owner_id": "view.choice_button",
  "event": "pointer_click",
  "phase": "input_target",
  "stable_order": {
    "tree_order": 42,
    "entity": "choice.opening.listen"
  },
  "effects": ["action.invoke"]
}
```
