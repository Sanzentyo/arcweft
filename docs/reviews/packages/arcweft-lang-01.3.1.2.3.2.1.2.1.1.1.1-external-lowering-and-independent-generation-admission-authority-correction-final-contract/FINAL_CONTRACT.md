# Final contract

The selected contract is one-way:

```text
accepted HIR + accepted nominal/producer world + immutable catalogs
    -> compiler-owned ProjectRuntimeGenerationAssembly
    -> core-owned non-Serde RuntimeGenerationAdmissionProjection
    -> core-owned AdmittedRuntimeGeneration (Arc parent)
    -> external runtime-plan lowerer uses public checked raw builders
    -> generation.try_admit_plan(raw plan)
    -> admitted_plan.try_admit_awbc(raw AWBC)
    -> direct coordinate equality
    -> runtime-driver RuntimeDriverGeneration + same-parent catalogs
    -> restore/replay payload decode and execution
```

Raw artifacts never appear on the left side of the generation issuance arrow.
Public checked construction proves raw structural validity only. Operational
publication always requires a separately issued generation.

Opaque checked values are atomic. `RuntimeValuePathSegment::OpaquePayload`
continues to describe the physical payload edge for ownership/save/diagnostic
walks, but the generic checked validator neither descends nor charges payload
work/depth.

Audio sites are effect-owned:
`AwbcTypedSite::AudioCommand { effect, slot }`. The effect selects the command
and the signature that gives meaning to `Arg(n)`. Reusing one command from two
effects produces two independent site families. `EffectPlan::AudioValue` does
not exist.

Every admitted expression has exactly one root fact at `[0]` and one fact for
every present child. The declaration kind is either `Checked` or a closed
`Operational` root-shape classification. Operational rows are matched against
independent accepted-world semantic facts but do not create checked-value
roots or AWBC correlation origins.

All constructors publish atomically by consuming staging builders/projections.
All final fields are private. Every invariant-bearing decode uses the same
checked construction path as the legitimate external lowerer.
