# Exact effect-owned AudioCommand site API

Owner: `crates/arcweft-core/src/awbc/typed_sites.rs`; resolution owner:
`crates/arcweft-core/src/awbc/admission.rs`; command behavior remains inherent
on `AwbcAudioCommand` in `awbc/schema.rs`.

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

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum AwbcTypedSite {
    // retained variants
    AudioCommand {
        effect: AwbcEffectPlanId,
        slot: AwbcAudioCommandTypedSlot,
    },
    EffectPlan {
        effect: AwbcEffectPlanId,
        slot: AwbcEffectPlanTypedSlot,
    },
    // retained variants
}

impl AwbcAudioCommandTypedSlot {
    pub const fn command_tag(self) -> u8;
    pub const fn field_tag(self) -> u8;
}

impl AwbcAudioCommand {
    pub fn typed_values(
        &self,
    ) -> Vec<(AwbcAudioCommandTypedSlot, AwbcAudioValueRef)>;

    pub fn typed_value(
        &self,
        slot: AwbcAudioCommandTypedSlot,
    ) -> Result<Option<AwbcAudioValueRef>, AwbcAudioSlotError>;
}
```

`typed_values` returns actual fields in canonical command/field order. The
optional `SetCaptureMonitor.bus` row is omitted when `None`.
`typed_value(SetCaptureMonitorBus)` returns `Ok(None)` for that absence; a slot
from another command variant returns `WrongCommandVariant`.

## Site resolver

```rust
impl AdmittedAwbcProduct {
    pub(crate) fn resolve_audio_site(
        &self,
        effect: AwbcEffectPlanId,
        slot: AwbcAudioCommandTypedSlot,
    ) -> Result<&RuntimeResolvedType, AwbcTypedSiteResolutionError>;
}
```

Resolution is exact:

1. bounds-check `effect` against `program.effect_plans()`;
2. require `effect.kind == AwbcEffectKind::Audio`;
3. require `effect.audio == Some(command)`;
4. bounds-check `command` against `program.audio_commands()`;
5. call the command's inherent `typed_value(slot)`; explicit lookup of an
   absent optional slot errors, while enumeration emits no row;
6. for `Arg(n)`, bounds-check `effect.signature`, then `params[n]`;
7. for `Const(c)`, bounds-check typed constant `c`, then its type ID;
8. resolve that type declaration against the admitted generation;
9. apply direct plan/AWBC origin equality.

The same command referenced by effects `e0` and `e1` yields sites
`(e0, slot)` and `(e1, slot)`. Each uses its own effect signature. Reuse is not
a duplicate and cannot create a cycle; commands contain no effect reference.
A repeated `(effect, slot)` origin pair is a duplicate. A command-reference
cycle is structurally impossible; existing constant/type reference cycles use
the retained tri-color verifier.

Canonical site bytes are:

```text
0x08 || effect:u32_le || command_tag:u8 || field_tag:u8
```

`AwbcEffectPlanTypedSlot` is exactly:

```rust
pub enum AwbcEffectPlanTypedSlot {
    Parameter { parameter: u32 },
    Result,
    StaticArgument { argument: u32 },
}
```

There is no `AudioValue` variant and no command-only coordinate.
