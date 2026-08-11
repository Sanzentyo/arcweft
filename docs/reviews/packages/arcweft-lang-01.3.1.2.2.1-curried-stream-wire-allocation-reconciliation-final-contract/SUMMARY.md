# Summary — Lang-01.3.1.2.2.1

## Final allocation

| Opcode | Final instruction |
| ---: | --- |
| `0x27` | `OpenStream` |
| `0x28` | `FinishStream` |
| `0x29` | `ApplyExternalStreamGroup` |

Parent lifecycle meanings at `0x27` and `0x28` win because Lang-01.3.1.2.2
expressly preserved unrelated Lang-01.3.1.2.1 lifecycle choices. The new
group-application instruction takes the next unused current-main instruction
byte, `0x29`. Current pushed `main` at `0b7e095f4193b9f7fbbc95cc350a626a8a63640a` uses `0x26` as its highest
non-terminator opcode, so the allocation is collision-free.

The codec-8 removed instruction bytes that remain invalid are exactly
`0x1c`, `0x1d`, `0x1e`, and `0x20`. Current-main `0x22=CallTraitMethod` and
`0x23=RegisterCleanup` remain valid and are not Source removals.

## Single owner decisions

- `arcweft-core::entry::RuntimeCallableBoundarySignature` remains the sole
  runtime callable boundary schema and is changed in place from a flat
  `parameters` vector to ordered parameter groups with `(group, parameter)`
  coordinates.
- No `RuntimeExternalStreamCallableSignature` family is introduced. AWBC
  callable metadata is a codec projection of the sole core owner, not a
  second semantic schema.
- `RuntimeFunctionValue` is changed in place to the sole closed
  `Closure | ExternalStreamPartial` owner.
- The sole external argument carrier is
  `RuntimeExternalStreamArgumentProduct`; no flat external argument adapter
  or endpoint DTO survives.

## Identity decisions

`RuntimeStreamDefinitionId(u32)` is only the RuntimePlan/AWBC table index.
`RuntimeStreamDefinitionKey([u8;32])` is the stable semantic definition key.
`StreamGeneration(u64)` and `StreamInstanceOrdinal(u64)` are components of the
complete live identity:

```text
StreamInstanceKey { definition_key, generation, ordinal }
```

Child spellings `StreamDefinitionId`, `GenerationId`, `StreamInstanceId`, and
`RuntimeTypeLayoutHash` are not aliases and are not published.

## Status

`READY_FOR_IMPLEMENTATION`, `OPEN_QUESTIONS=0`.
