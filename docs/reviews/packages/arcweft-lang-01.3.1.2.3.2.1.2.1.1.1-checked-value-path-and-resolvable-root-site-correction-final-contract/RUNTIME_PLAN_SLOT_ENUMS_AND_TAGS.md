# Exact RuntimePlan nested slot/field enums

Owner: `arcweft_core::plan::typed_sites`. All fields are private; every enum implements `canonical_tag` in its original inherent `impl`. They derive `Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize` and use manual checked `Deserialize` through private wire DTOs. `RUNTIME_PLAN_NESTED_SLOT_TAGS.csv` is the byte/tag authority.

```rust
pub enum RuntimeEntryTypedSlot {
    State,
    Event,
    CommandPayload { command: u32 },
}
```

```rust
pub enum RuntimeFlowExecutableTypedSlot {
    Parameter { parameter: u32 },
}
```

```rust
pub enum RuntimePureHelperTypedSlot {
    Parameter { parameter: u32 },
    Result,
}
```

```rust
pub enum RuntimeTraitMethodTypedSlot {
    Receiver,
    Parameter { parameter: u32 },
    Result,
}
```

```rust
pub enum RuntimeStreamTypedSlot {
    Item,
    Error,
}
```

```rust
pub enum RuntimeSourceTypedSlot {
    Item,
    Error,
}
```

```rust
pub enum RuntimeFlowExpressionField {
    LetValue,
    LetElseValue,
    AwaitManySource,
    IfCondition,
    IfLetScrutinee,
    IfLetGuard,
    MatchScrutinee,
    MatchArmGuard { arm: u32 },
    WhileCondition,
    WhileNextCondition,
    WhileLetScrutinee,
    WhileLetGuard,
    WhileLetNextScrutinee,
    WhileLetNextGuard,
    ForSource,
    LetScopeValue,
    BreakValue,
    GotoExprTarget,
    ReturnExprValue,
    ExitScopeBindValue,
    AwaitArgument { argument: u32 },
    AwaitManyArgument { argument: u32 },
    HostCallArgument { argument: u32 },
    EvaluatedEffectArgument { argument: u32 },
}
```

```rust
pub enum RuntimeFlowPatternField {
    LetPattern,
    LetElsePattern,
    AwaitBinding,
    AwaitManyBinding,
    HostCallBinding,
    IfLetPattern,
    MatchArmPattern { arm: u32 },
    LetLoopPattern,
    WhileLetPattern,
    WhileLetNextPattern,
    ForPattern,
    ForNextPattern,
    LetScopePattern,
    ExitScopeBindPattern,
}
```

```rust
pub enum RuntimeStreamExpressionField {
    LetValue,
    ForNextSource,
    YieldValue,
    IfCondition,
    MatchScrutinee,
    MatchArmGuard { arm: u32 },
    CloseSource,
}
```

```rust
pub enum RuntimeStreamPatternField {
    LetPattern,
    ForNextPattern,
    MatchArmPattern { arm: u32 },
}
```

```rust
pub enum RuntimeSourceExpressionField {
    OpenNode,
    ItemYieldNode { op: u32 },
    ErrorYieldNode { op: u32 },
    ProgressYieldNode { op: u32 },
    DisconnectedYieldNode { op: u32 },
    PermissionRevokedYieldNode { op: u32 },
    EndYieldNode { op: u32 },
    EvaluatedEffectArgument { argument: u32 },
}
```

Coordinate step tags are exact in `RUNTIME_PLAN_COORDINATE_STEP_TAGS.csv`. A flow coordinate is `flow:u32_le || root:u32_le || u32_le(step_count) || steps`; a stream coordinate is `plan:u32_le || root:u32_le || u32_le(step_count) || steps`. Each step is `tag:u8 || payload`. A source coordinate is exactly `plan:u32_le || handler:u32_le || op:u32_le`.

`RegisterCleanup` currently stores a static/text `LineEffectRequest`, not a `RuntimeExpr`; therefore `RegisterCleanupEffectArgument` is deliberately absent from this enum and from `RUNTIME_PLAN_NESTED_SLOT_TAGS.csv`. Its exclusion remains an explicit structural row in `RUNTIME_PLAN_SITE_RESOLUTION.csv` and cannot synthesize a root.

A vector-bearing field stores the field-named `u32` shown above. Bounds are checked against the actual owning vector before type resolution. There is no generic slot ordinal, no tag inferred from display spelling, and no unknown-tag fallback.
