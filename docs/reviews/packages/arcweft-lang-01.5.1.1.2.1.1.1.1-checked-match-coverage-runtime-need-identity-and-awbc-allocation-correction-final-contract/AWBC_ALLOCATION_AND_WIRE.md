# Global AWBC allocation and canonical wire

## Opcode table

| Byte | Opcode | Class | Landing status at inspected main |
|---:|---|---|---|
| `0x00` | `Nop` | instruction | implemented |
| `0x01` | `LoadConst` | instruction | implemented |
| `0x02` | `Move` | instruction | implemented |
| `0x03` | `Clear` | instruction | implemented |
| `0x04` | `EnterScope` | instruction | implemented |
| `0x05` | `ExitScope` | instruction | implemented |
| `0x06` | `BindPattern` | instruction | implemented |
| `0x07` | `TestPattern` | instruction | implemented |
| `0x08` | `MakeTuple` | instruction | implemented |
| `0x09` | `MakeSequence` | instruction | implemented |
| `0x0a` | `RepeatSequence` | instruction | implemented |
| `0x0b` | `SequenceLen` | instruction | implemented |
| `0x0c` | `SequenceGet` | instruction | implemented |
| `0x0d` | `SequenceSlice` | instruction | implemented |
| `0x0e` | `SequencePush` | instruction | implemented |
| `0x0f` | `MakeRecord` | instruction | implemented |
| `0x10` | `MakeVariant` | instruction | implemented |
| `0x11` | `ProjectTuple` | instruction | implemented |
| `0x12` | `ProjectRecord` | instruction | implemented |
| `0x13` | `ProjectField` | instruction | implemented |
| `0x14` | `Unary` | instruction | implemented |
| `0x15` | `Binary` | instruction | implemented |
| `0x16` | `CallPureHelper` | instruction | implemented |
| `0x17` | `CallIntrinsic` | instruction | implemented |
| `0x18` | `EnsureContent` | instruction | implemented |
| `0x19` | `EmitEffect` | instruction | implemented |
| `0x1a` | `StartTask` | instruction | implemented |
| `0x1b` | `SpawnFiber` | instruction | implemented |
| `0x1c` | `StreamYield` | instruction | implemented |
| `0x1d` | `StreamClose` | instruction | implemented |
| `0x1e` | `NeedTimeout` | instruction | allocated_pending_feature_cut |
| `0x1f` | `Drop` | instruction | implemented |
| `0x20` | `CommitDialogueResult` | instruction | allocated_pending_feature_cut |
| `0x21` | `AssignRecordField` | instruction | implemented |
| `0x22` | `CallTraitMethod` | instruction | implemented |
| `0x23` | `RegisterCleanup` | instruction | implemented |
| `0x24` | `CancelCleanup` | instruction | implemented |
| `0x25` | `MakeFunction` | instruction | implemented |
| `0x26` | `ApplyFunction` | instruction | implemented |
| `0x27` | `MakeAgent` | instruction | implemented |
| `0x28` | `MakeReductionUnchanged` | instruction | implemented |
| `0x29` | `MakeNeedHandle` | instruction | allocated_pending_feature_cut |
| `0x2a` | `CopyValue` | instruction | allocated_pending_feature_cut |
| `0x2b` | `ExecuteLineOperation` | instruction | allocated_pending_feature_cut |
| `0x2c` | `OpenStream` | instruction | allocated_pending_feature_cut |
| `0x2d` | `FinishStream` | instruction | allocated_pending_feature_cut |
| `0x2e` | `ApplyExternalStreamGroup` | instruction | allocated_pending_feature_cut |
| `0x80` | `Jump` | terminator | implemented |
| `0x81` | `Branch` | terminator | implemented |
| `0x82` | `Match` | terminator | implemented |
| `0x83` | `CallFunction` | terminator | implemented |
| `0x84` | `GotoStatic` | terminator | implemented |
| `0x85` | `GotoDynamic` | terminator | implemented |
| `0x86` | `Dialogue` | terminator | implemented |
| `0x87` | `Choice` | terminator | implemented |
| `0x88` | `Await` | terminator | implemented |
| `0x89` | `AwaitMany` | terminator | implemented |
| `0x8a` | `HostCall` | terminator | implemented |
| `0x8b` | `Return` | terminator | implemented |
| `0x8c` | `Trap` | terminator | implemented |
| `0x8d` | `BudgetYield` | terminator | implemented |
| `0x8e` | `Unreachable` | terminator | implemented |
| `0x8f` | `NextStream` | terminator | allocated_pending_feature_cut |
| `0x90` | `YieldStream` | terminator | allocated_pending_feature_cut |

`0x2f..=0x7f` and `0x91..=0xff` are unknown/reserved and reject. A pending row
is an accepted allocation, not permission to land an unsupported enum variant.
It becomes constructible only in the feature cut named by
`COMPILE_CLEAN_SEQUENCE.md`.

## Function kinds

| Tag | Kind | Status |
|---:|---|---|
| `0` | `Flow` | implemented |
| `1` | `PureHelper` | implemented |
| `2` | `TraitMethod` | implemented |
| `3` | `StreamTransform` | implemented |
| `6` | `LineTask` | implemented |
| `7` | `Synthetic` | implemented |
| `8` | `Ordinary` | allocated_pending_stream_cut |
| `9` | `GeneratorProducer` | allocated_pending_stream_cut |
| `10` | `LineActivation` | allocated_pending_line_cut |

Tags 4 and 5 are removed tombstones and are never reused. Tags 11..=255 reject.
The authority is one `#[repr(u8)] AwbcFunctionKind` with the same inherent
numeric/Serde/Wire architecture as `AwbcOpcode`.

## Function flags

| Bit | Mask | Flag | Status |
|---:|---:|---|---|
| `0` | `0x01` | `MaySuspend` | implemented |
| `1` | `0x02` | `MayAllocate` | implemented |
| `2` | `0x04` | `Deterministic` | implemented |
| `3` | `0x08` | `HasDynamicTarget` | implemented |
| `4` | `0x10` | `OwnsStreamProducer` | allocated_pending_stream_cut |
| `5` | `0x20` | `NeedProducer` | allocated_pending_need_cut |

`KNOWN_MASK` is exactly `0x3f`. `AwbcFunctionFlags` exposes only `empty`,
`with`, `contains`, `bits`, and `try_from_bits`. The mask is always derived by
`1_u32 << flag as u8`.

- `GeneratorProducer`: requires bit 4 and forbids bit 5.
- Need producer: kind `Synthetic`, requires bits 5,2,1 and forbids bits 4,0.
- `LineActivation`, `LineTask`, `Ordinary`, and `StreamTransform`: forbid both
  producer bits.
- Bits 4 and 5 together always reject.
- A non-producer selector may be `Synthetic` without bit 5.

## Canonical integer grammar

Ordinary `u32` uses the shortest unsigned base-128 varint:

```text
value 0          00
value 1          01
value 127        7f
value 128        80 01
value u32::MAX   ff ff ff ff 0f
```

Decoder failure classes are non-canonical redundancy, overflow, sixth byte,
unterminated input, and truncation. `80 00` and `81 00` reject. Every ID,
register, table index, site, ordinal, revision, source offset, count, length,
tensor dimension, and Char scalar uses this grammar. Collection length first
converts to u32 and is budget-checked before allocation. No `usize` enters the
wire.

Fixed-width fields are limited to u8 tags; envelope version/reserved and
existing Stream group/parameter/audio-channel u16-le coordinates; envelope
length and duration/frame/feature/budget u64-le; priority i32-le; fixed 16/32
byte digests or integer bit patterns; F32 raw bits/TensorF32 elements as 4-byte
LE; and F64 raw bits/TensorF64 elements as 8-byte LE.

Tensor shapes use `Vec<u32>::Wire` for both writing and reading. The previous
fixed-LE `write_u32_slice` path is deleted.

## New/pending instruction grammars

```text
29 dst:varu32 plan:varu32 site:varu32 argc:varu32 args[argc]:varu32
1e dst:varu32 source:varu32 limit:varu32 producer_site:varu32
20 source:varu32
2a dst:varu32 source:varu32
2b dst:varu32 operation:varu32 argc:varu32 args[argc]:varu32
```

The protected Stream operand shapes are retained while their bytes move:

```text
2c dst:varu32 callee:varu32 definition:varu32 signature:varu32
   group:u16-le
   coordinate_count:varu32
   repeat coordinate_count: group:u16-le parameter:u16-le
   operand_count:varu32
   operands[operand_count]:external-stream-operand

2d stream:varu32 outcome:u8

2e dst:varu32 callee:varu32 definition:varu32 signature:varu32
   group:u16-le
   coordinate_count:varu32
   repeat coordinate_count: group:u16-le parameter:u16-le
   operand_count:varu32
   operands[operand_count]:external-stream-operand
```

`NextStream` and `YieldStream` retain the protected Stream package's exact
semantic operands and move to `0x8f` and `0x90`; their implementation cut owns
the final verifier and execution behavior. No dummy decoder is admitted before
that cut.

## One-buffer encoder

`AwbcProgram::encode_canonical` allocates the final Vec once. It records the
starting length, appends the 20-byte envelope with a zero payload-length slot,
writes final payload values directly, computes the payload length, and patches
bytes 12..20. Every error truncates to the recorded starting length. Decoding
passes the original payload slice directly to `Reader<'_>` and constructs final
owned values; opcode/kind/flag/ID/scalar decoding allocates nothing.

Serde remains a structured-data boundary and is not part of the canonical
zero-intermediate-copy claim.
