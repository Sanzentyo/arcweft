# Decision 05 — mechanically resolvable AWBC site authority

## Raw schema replacement

The AWBC program uses mandatory typed owners:

```rust
pub struct AwbcProgram {
    header: AwbcHeader,
    generation_contract: RuntimeGenerationContractDeclaration,
    strings: Vec<String>,
    runtime_types: Vec<AwbcRuntimeTypeDeclaration>,
    constants: Vec<AwbcTypedConstant>,
    // current effect/signature/frame/function/block/instruction tables
    patterns: Vec<AwbcTypedPattern>,
    typed_origins: Vec<AwbcTypedOrigin>,
    nominal_domains: Vec<AwbcNominalRecordDomainDeclaration>,
    // remaining current tables
}
```

No field has `#[serde(default)]`. The current `AwbcProgram::default()` is retained only for test/build staging inside core and cannot create an admitted program; its generation/type/origin/domain candidates fail admission until explicitly filled. Public execution APIs never accept it.

`AwbcRuntimeTypeDeclaration` is resolved against `AdmittedRuntimeGeneration`; `AwbcTypeId` remains an artifact-local dense index. `AwbcTypedConstant` makes every constant's type independently resolvable. `AwbcTypedPattern` makes every pattern's expected type mandatory. Existing optional `AwbcPattern::Bind.expected` and `AwbcPattern::Record.ty` are removed as duplicate claims; the wrapper is the single expected-type owner.

## Sites and slots

`AWBC_TYPED_SITE_ENUMS.md` is the exact Rust API. `AWBC_SITE_RESOLUTION.csv` maps every current table owner and field. `AWBC_INSTRUCTION_TYPED_SLOTS.csv`, `AWBC_TERMINATOR_TYPED_SLOTS.csv`, and `AWBC_AUDIO_TYPED_SLOTS.csv` are normative closed slot tables.

Repeated vector positions are permitted only in a field-named variant such as `LoadConst...` (no index), `MakeTupleItem { item }`, `CallIntrinsicArgument { argument }`, or `SetCaptureMonitorBus`. A bare `slot: u32`, an opcode-relative undocumented ordinal, or a display-name field resolver is rejected.

## Independent resolution

- register type: owning function → frame layout → exact frame slot;
- signature type: signature table parameter/result;
- constant type: `AwbcTypedConstant.ty` and recursive constant validation;
- pattern type: `AwbcTypedPattern.expected` and recursive pattern validation;
- instruction/terminator: actual enum variant plus its named field slot, then frame/signature/constant/pattern owner;
- task/effect/audio: table signature, static argument prefix, named audio value slot, and dynamic argument suffix;
- stream/source: direct item/error type IDs plus referenced function/pattern signatures;
- entry/helper/trait/flow executable: referenced signature and shared typed metadata;
- nominal record construction: declared runtime type plus mandatory version-1 project/producer domain operand retained from the parent.

Bounds are checked before dereference. Table range `checked_end` overflow, duplicate semantic IDs where uniqueness is required, noncanonical order, and malformed vector cardinalities fail before type comparison.

## Aliasing and cycles

Dense references may alias only when they resolve to the same owner row and exact type required by the consuming field. Aliasing never makes two unequal types equivalent. Runtime-type, constant, and pattern graphs use tri-color DFS. A back edge returns a typed cycle error before root/domain correlation. Control-flow block cycles are permitted and are not treated as type cycles. `AWBC_REFERENCE_RESOLUTION.csv` defines each reference class.

## Deliberate exclusions

Choice/content/display/source/resource/identity-only tables do not gain fake typed roots. Their referenced functions/effects/constants own any value types. Current operational function values (`MakeFunction`, `ApplyFunction`, dynamic goto targets) are explicitly non-root because the current closed `RuntimeCheckedType` has no Function case; they may not be used as project/producer construction authority or persisted root evidence. This exclusion is executable and tested rather than represented by a placeholder type.

## Admission and operational wrapper

`ADMISSION_AND_PAIR_API.md` is normative for `AwbcProgram::try_admit`, private same-parent pair admission, `AdmittedAwbcProduct`, `AdmittedRuntimeProduct`, and the product-step execution cut. No `AdmittedAwbcProgram` alias exists.
