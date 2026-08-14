# Exact AWBC non-instruction slot enums

Owner: `arcweft_core::awbc::typed_sites`. All fields are private; all enums implement `canonical_tag` on their original inherent `impl`, derive `Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize`, and use private checked wire DTOs for `Deserialize`. `AWBC_NESTED_SLOT_TAGS.csv` is normative.

```rust
pub enum AwbcPatternTypedSlot {
    BindExpected,
    DiscardExpected,
    LiteralExpected,
    EntityExpected,
    TupleExpected,
    RecordExpected,
    SequenceExpected,
    VariantExpected,
    WholeExpected,
}
```

```rust
pub enum AwbcSignatureTypedSlot {
    Parameter { parameter: u32 },
    Result,
}
```

```rust
pub enum AwbcTaskPlanTypedSlot {
    Parameter { parameter: u32 },
    Result,
    AwaitManyItem,
}
```

```rust
pub enum AwbcEffectPlanTypedSlot {
    Parameter { parameter: u32 },
    Result,
    StaticArgument { argument: u32 },
}
```

```rust
pub enum AwbcLineTaskGroupTypedSlot {
    OptionValue { option: u32 },
    BindingsParameter { parameter: u32 },
    BindingsResult,
    OutParameter { parameter: u32 },
    OutResult,
    CancelHandlerParameter { handler: u32, parameter: u32 },
    CancelHandlerResult { handler: u32 },
}
```

```rust
pub enum AwbcStreamPlanTypedSlot {
    Item,
    Error,
    TransformParameter { parameter: u32 },
    TransformResult,
}
```

```rust
pub enum AwbcSourcePlanTypedSlot {
    Item,
    Error,
    OpenParameter { parameter: u32 },
    OpenResult,
    HandlerPattern { handler: u32 },
    HandlerParameter { handler: u32, parameter: u32 },
    HandlerResult { handler: u32 },
}
```

```rust
pub enum AwbcPureHelperTypedSlot {
    Parameter { parameter: u32 },
    Result,
}
```

```rust
pub enum AwbcTraitMethodTypedSlot {
    Parameter { parameter: u32 },
    Result,
    ReceiverState,
}
```

```rust
pub enum AwbcFlowExecutableTypedSlot {
    Parameter { parameter: u32 },
}
```

```rust
pub enum AwbcEntryTypedSlot {
    Parameter { parameter: u32 },
    Result,
}
```

`AwbcEffectPlan.audio` is an indirect reference to one `AwbcAudioCommandId`; its typed fields are represented only by the effect-owned `AwbcTypedSite::AudioCommand { effect, slot }` coordinate and `AWBC_AUDIO_TYPED_SLOTS.csv`, never by an unshaped `AudioValue(slot)`.

Every index is field-named and bounds-checked against the actual referenced table before type/root correlation. Unknown tags, wrong variant/slot pairs, and indices outside the current owner fail; no numeric fallback or alias exists.
