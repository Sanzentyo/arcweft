# AWBC typed-site API

Owner: `crates/arcweft-core/src/awbc/typed_sites.rs`, re-exported from `arcweft_core::awbc`. Raw DTO fields are private; the external `arcweft-runtime-plan` lowerer uses public checked constructors, and deserialization uses private wire DTOs followed by those same public constructors. Public construction creates raw checked data only; admission authority remains separate.

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum AwbcTypedSite {
    RuntimeType { ty: AwbcTypeId },
    Constant { constant: AwbcConstantId },
    Pattern { pattern: AwbcPatternId, slot: AwbcPatternTypedSlot },
    Signature { signature: AwbcSignatureId, slot: AwbcSignatureTypedSlot },
    FunctionFrame { function: AwbcFunctionId, register: AwbcRegisterId },
    Instruction { function: AwbcFunctionId, instruction: AwbcInstructionId, slot: AwbcInstructionTypedSlot },
    Terminator { function: AwbcFunctionId, block: AwbcBlockId, slot: AwbcTerminatorTypedSlot },
    TaskPlan { task: AwbcTaskPlanId, slot: AwbcTaskPlanTypedSlot },
    AudioCommand { effect: AwbcEffectPlanId, slot: AwbcAudioCommandTypedSlot },
    EffectPlan { effect: AwbcEffectPlanId, slot: AwbcEffectPlanTypedSlot },
    LineTaskGroup { group: AwbcLineTaskGroupId, slot: AwbcLineTaskGroupTypedSlot },
    StreamPlan { stream: AwbcStreamPlanId, slot: AwbcStreamPlanTypedSlot },
    SourcePlan { source: AwbcSourcePlanId, slot: AwbcSourcePlanTypedSlot },
    PureHelper { helper: AwbcPureHelperId, slot: AwbcPureHelperTypedSlot },
    TraitMethod { method: AwbcTraitMethodId, slot: AwbcTraitMethodTypedSlot },
    FlowExecutable { flow: AwbcFlowExecutableId, slot: AwbcFlowExecutableTypedSlot },
    Entry { entry: AwbcEntryId, slot: AwbcEntryTypedSlot },
}
```

## Exact instruction slot enum

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum AwbcInstructionTypedSlot {
    LoadConstDestination,
    LoadConstConstant,
    MoveDestination,
    MoveSource,
    ClearRegister,
    BindPatternPatternExpected,
    BindPatternValue,
    TestPatternDestinationBool,
    TestPatternPatternExpected,
    TestPatternValue,
    MakeTupleDestination,
    MakeTupleItem { item: u32 },
    MakeSequenceDestination,
    MakeSequenceItem { item: u32 },
    RepeatSequenceDestination,
    RepeatSequenceValue,
    RepeatSequenceLength,
    SequenceLenDestination,
    SequenceLenSequence,
    SequenceGetDestination,
    SequenceGetSequence,
    SequenceGetIndex,
    SequenceSliceDestination,
    SequenceSliceSequence,
    SequenceSliceStart,
    SequencePushSequence,
    SequencePushValue,
    MakeRecordDestination,
    MakeRecordDeclaredType,
    MakeRecordField { field: u32 },
    MakeVariantDestination,
    MakeVariantDeclaredType,
    MakeVariantPayload,
    ProjectTupleDestination,
    ProjectTupleTarget,
    ProjectRecordDestination,
    ProjectRecordTarget,
    ProjectFieldDestination,
    ProjectFieldTarget,
    UnaryDestination,
    UnarySource,
    BinaryDestination,
    BinaryLeft,
    BinaryRight,
    CallPureHelperDestination,
    CallPureHelperArgument { argument: u32 },
    CallIntrinsicDestination,
    CallIntrinsicArgument { argument: u32 },
    EmitEffectArgument { argument: u32 },
    StartTaskDestination,
    StartTaskArgument { argument: u32 },
    SpawnFiberDestination,
    SpawnFiberArgument { argument: u32 },
    StreamYieldValue,
    DropRegister,
    SourceYieldValue,
    AssignFieldTarget,
    AssignFieldValue,
    CallTraitMethodDestination,
    CallTraitMethodReceiver,
    CallTraitMethodArgument { argument: u32 },
    CallTraitMethodReceiverOut,
    RegisterCleanupArgument { argument: u32 },
}
```

The instruction variant is embedded in every emitted slot variant. `MakeFunction` and `ApplyFunction` are deliberately absent because current `RuntimeCheckedType` has no Function case; their frame/capture/signature invariants are verified structurally and cannot emit a root-correlated site. A repeated field carries a field-named `u32`; there is no unshaped ordinal. `AWBC_INSTRUCTION_TYPED_SLOTS.csv` fixes the canonical tag, current field, and type resolver for every row.

## Exact terminator slot enum

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum AwbcTerminatorTypedSlot {
    BranchCondition,
    MatchScrutinee,
    CallFunctionArgument { argument: u32 },
    CallFunctionDestination,
    GotoStaticArgument { argument: u32 },
    ChoiceDestination,
    AwaitHandle,
    AwaitBinding,
    AwaitManySource,
    AwaitManyBinding,
    HostCallArgument { argument: u32 },
    HostCallDestination,
    ReturnValue,
}
```

`GotoDynamic` is deliberately absent from the terminator slot enum until its target carries an exact retained callable signature. The verifier rejects any attempt to use it as a root-correlated origin; no target/argument slot tag exists.

## Exact audio slot enum

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum AwbcAudioCommandTypedSlot {
    PlayVoice,
    PlayResource,
    PlayBus,
    PlayGainDbMilli,
    PlayPanMilli,
    PlayStartFrame,
    PlayFadeInMillis,
    StopVoice,
    StopFadeOutMillis,
    StopAllFadeOutMillis,
    SetVoiceGainVoice,
    SetVoiceGainValue,
    SetVoiceGainTransition,
    SetVoicePanVoice,
    SetVoicePanValue,
    SetVoicePanTransition,
    SetBusGainBus,
    SetBusGainValue,
    SetBusGainTransition,
    SetBusMuteBus,
    SetBusMuteMuted,
    SetEffectEnabledBus,
    SetEffectEnabledEffect,
    SetEffectEnabledEnabled,
    SetEffectParameterBus,
    SetEffectParameterEffect,
    SetEffectParameterValue,
    SetEffectParameterTransition,
    ApplySnapshotSnapshot,
    ApplySnapshotTransition,
    RequestMicrophoneCapture,
    StopMicrophoneCapture,
    SetCaptureMonitorCapture,
    SetCaptureMonitorBus,
    SetCaptureMonitorGain,
}
```

## Origin evidence

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AwbcTypedOrigin {
    plan_site: RuntimePlanTypedSite,
    awbc_site: AwbcTypedSite,
}

impl AwbcTypedOrigin {
    pub fn new(
        plan_site: RuntimePlanTypedSite,
        awbc_site: AwbcTypedSite,
    ) -> Self;

    pub const fn plan_site(&self) -> &RuntimePlanTypedSite;
    pub const fn awbc_site(&self) -> &AwbcTypedSite;
}
```

`AwbcTypedOrigin::new` is public because the pair has no invariant beyond owning two closed coordinates. Canonical ordering, duplicate rejection, actual-owner bounds, and equality are enforced by `AwbcProgramBuilder::push_typed_origin` and admitted pair correlation. `AwbcTypedOrigin` deliberately has no root ID, semantic ID, checked type, generation ID, or dense type ID. Those values are independently resolved from the actual plan owner and actual AWBC owner after each raw artifact has been admitted.

## Normative remaining slot enums and top-level tags

`AWBC_SLOT_ENUMS_AND_TAGS.md` and `AWBC_NESTED_SLOT_TAGS.csv` define every non-instruction/non-terminator/non-audio slot enum. `AWBC_SITE_CANONICAL_TAGS.csv` exactly follows the `AwbcTypedSite` declaration above: it contains `FunctionFrame` and contains neither a bare `Frame` nor a separate `Function` site. `Frame` and `Function` rows in `AWBC_SITE_RESOLUTION.csv` are independently checked reference invariants, not serialized site alternatives.
