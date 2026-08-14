# Decision 09 — effect-owned AudioCommand sites

The final coordinate is:

```rust
AwbcTypedSite::AudioCommand {
    effect: AwbcEffectPlanId,
    slot: AwbcAudioCommandTypedSlot,
}
```

The command ID is deliberately absent. The effect row uniquely selects
`effect.signature` and `effect.audio`; that command then selects the named
value field. `Arg(n)` resolves to parameter `n` of this effect's signature;
`Const(c)` resolves through typed constant `c`. The optional
`SetCaptureMonitor.bus` emits a site only for `Some`.

One command may be referenced by multiple effects. Each effect creates a
separate coordinate family and is checked independently. Reuse is accepted
when each use resolves; it is not deduplicated by command ID. Duplicate origin
rows for the same `(effect, slot)` are rejected. Effect->command aliasing is
acyclic because commands contain only Arg/Const leaves and cannot reference an
effect. Constant/type graph cycles retain their existing tri-color error.

`AwbcEffectPlanTypedSlot` contains only `Parameter`, `Result`, and
`StaticArgument`. `AudioValue` is deleted. Exact bounds/variant/slot/reference
precedence, all 35 slots, lowerer/verifier/VM behavior, and tags are in
`AUDIO_SITE_API.md`, `AWBC_AUDIO_TYPED_SLOTS.csv`, and the corrected AWBC tables.
