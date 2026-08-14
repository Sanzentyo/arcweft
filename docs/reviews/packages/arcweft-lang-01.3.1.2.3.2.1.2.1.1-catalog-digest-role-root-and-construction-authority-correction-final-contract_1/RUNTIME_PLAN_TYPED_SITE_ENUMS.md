# Exact `RuntimePlan` typed slot enums

```rust
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeEntryTypedSlot {
    StateInput,
    StateOutput,
    EventInput,
    EventOutput,
    CommandInput,
    CommandOutput,
    CommandConstructorPayload { constructor: u32, field: u32 },
    RouterPayload { route: u32 },
    ViewInput { input: u32 },
    ReplayVisible { slot: u32 },
    SaveVisible { slot: u32 },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeCallableTypedSlot {
    Parameter { parameter: u32 },
    Result,
    Capture { capture: u32 },
    FrameLocal { local: u32 },
    Constant { constant: u32 },
    Pattern { pattern: u32 },
    ReducerInput,
    ReducerOutput,
    ResourcePayload { resource: u32 },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeFlowExecutableTypedSlot {
    Parameter { parameter: u32 },
    Result,
    Capture { capture: u32 },
    FrameLocal { local: u32 },
    ReplayVisible { slot: u32 },
    SaveVisible { slot: u32 },
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeFlowOpTypedSlot {
    ExpressionResult,
    ExpressionInput { input: u32 },
    Condition,
    PatternExpected,
    PatternBinding { binding: u32 },
    CallArgument { argument: u32 },
    CallResult,
    MatchScrutinee,
    MatchArmPattern { arm: u32 },
    MatchArmResult { arm: u32 },
    ChoiceOptionPayload { option: u32 },
    EffectInput { input: u32 },
    EffectOutput,
    ReturnValue,
    Constant,
    NominalConstructor,
    NominalField { field: u32 },
    RecordPattern,
    VariantPattern,
    ViewInput { input: u32 },
    ReducerInput,
    ReducerOutput,
    ResourcePayload,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimePureHelperTypedSlot {
    Parameter { parameter: u32 },
    Capture { capture: u32 },
    FrameLocal { local: u32 },
    Constant { constant: u32 },
    Pattern { pattern: u32 },
    Result,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeTraitMethodTypedSlot {
    Receiver,
    Parameter { parameter: u32 },
    Result,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeLineTaskTypedSlot {
    Content,
    ChoicePayload { option: u32 },
    TaskResult,
    Progress,
    Error,
    Cleanup,
    EffectInput { input: u32 },
    EffectOutput,
    ReplayVisible,
    SaveVisible,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeStreamTypedSlot {
    Item,
    Error,
    TransformParameter { parameter: u32 },
    TransformResult,
    HandlerParameter { parameter: u32 },
    HandlerResult,
    ReplayVisible,
    SaveVisible,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "slot", rename_all = "snake_case", deny_unknown_fields)]
pub enum RuntimeSourceTypedSlot {
    Item,
    Error,
    OpenParameter { parameter: u32 },
    OpenResult,
    HandlerParameter { parameter: u32 },
    HandlerResult,
    ReplayVisible,
    SaveVisible,
}
```
