# Exact codec-8 wire table

## Instruction allocation

| Opcode | Variant | Rust field order | Binary order |
| ---: | --- | --- | --- |
| `0x27` | `OpenStream` | `dst, callee, definition, signature, group, arguments` | opcode; four varu32 IDs; group u16-le; coordinate vector; operand vector |
| `0x28` | `FinishStream` | `stream, outcome` | opcode; stream varu32; outcome tag and optional error varu32 |
| `0x29` | `ApplyExternalStreamGroup` | `dst, callee, definition, signature, group, arguments` | opcode; four varu32 IDs; group u16-le; coordinate vector; operand vector |

No other instruction is implied. `0x2a..=0x7f` are unknown.

## Group argument record

```text
coordinates_len: varu32
coordinates[coordinates_len]:
    group: u16 little-endian
    parameter: u16 little-endian
values_len: varu32
values[values_len]:
    operand tag: u8
    variant payload
```

`coordinates_len == values_len` is required. Both lengths are checked against
decode/allocation budgets before pairing.

## Operand tags

| Tag | Variant | Payload order |
| ---: | --- | --- |
| 0 | `Explicit` | value register ID |
| 1 | `Defaulted` | 32-byte default digest; value register ID |
| 2 | `OmittedOptional` | none |
| 3 | `RestPositional` | aggregate register ID |
| 4 | `RestNamed` | aggregate register ID |

## Producer outcome tags

| Tag | Variant | Payload order |
| ---: | --- | --- |
| 0 | `Complete` | none |
| 1 | `Fail` | error register ID |
| 2 | `Cancelled` | none |

## Removed instruction bytes

| Opcode | Removed meaning | Codec-8 result |
| ---: | --- | --- |
| `0x1c` | `StreamYield` | unknown instruction opcode |
| `0x1d` | `StreamClose` | unknown instruction opcode |
| `0x1e` | `SourceClose` | unknown instruction opcode |
| `0x20` | `SourceYield` | unknown instruction opcode |

`0x22=CallTraitMethod` and `0x23=RegisterCleanup` remain valid.

## Primitive rules

- `u8`: one byte.
- `u16`: exactly two little-endian bytes.
- `u32` IDs and lengths: canonical unsigned base-128 varint.
- `u64`: exactly eight little-endian bytes.
- digest: exactly 32 raw bytes.
- unknown tags, noncanonical varints, truncation, trailing bytes, and budget
  excess are hard errors; no reader repairs input.
