# AWBC ABI-1 ownership wire

## Fixed version

```text
AWBC_ABI_VERSION = 1
AWBC_CODEC_VERSION = 8
```

The version field is unchanged. The internal unreleased meaning is replaced directly. All generated artifacts and golden fixtures are rebuilt.

## Opcode allocation

| Opcode | Instruction | Final ownership use |
|---:|---|---|
| `0x1f` | existing `Drop` | consume live slot through prepared table-aware drop |
| existing | existing `Move` | consume source and install exact value in destination |
| `0x27` | `OpenStream` | consume callee/arguments and mint one affine owner |
| `0x28` | `FinishStream` | parent lifecycle semantics; consumes terminal producer inputs as specified |
| `0x29` | `ApplyExternalStreamGroup` | consume callee/argument product into partial |
| `0x2a` | `CopyValue { dst, src }` | checked duplicate of statically and dynamically unrestricted value |

Wire for `CopyValue`:

```text
2a <dst: canonical unsigned base-128 varu32>
   <src: canonical unsigned base-128 varu32>
```

`0x2b..=0x7f` are unknown and rejected.

## Verifier facts

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AwbcRegisterState {
    Uninitialized,
    Live {
        ty: AwbcTypeId,
        ownership: RuntimeValueOwnership,
    },
    Moved { ty: AwbcTypeId },
    Dropped { ty: AwbcTypeId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AwbcOperandUse {
    Borrow,
    Copy,
    Consume,
    Destination,
}
```

`AwbcInstruction` owns operand-use behavior through inherent methods. There is no external opcode match table. Open/generic runtime types are treated as affine unless exact closed layout evidence proves unrestricted.

## Direct replacement and stale artifacts

No runtime chooses between two meanings of ABI 1. Pre-cut artifacts are unsupported and are invalidated through build/cache compiler identity, enclosing product content root, accepted program revision, and regenerated golden bytes. No compatibility reader is introduced merely to produce a more specific error for unreleased bytes.

## View joins

`ViewValueFunctionRef.awbc_abi` is exactly 1. Product validation recomputes function input ownership and requires exact equality with each `ViewValueInputBinding.transfer` and `value_type`. A function whose declared result may be affine cannot back a retained View binding in this cut.
