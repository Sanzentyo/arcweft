# Worked codec-8 bytes

All IDs and vector lengths below use canonical u32 varints. Group and parameter indices use fixed two-byte little-endian u16.

## WB-APPLY-ONE-EXPLICIT — ApplyExternalStreamGroup

- byte count: `15`
- SHA-256 of the raw record: `4ad85dd86da44a3a003cc30d471c4caabc630189ec9a51602d828803b64ba868`

```text
29 01 02 03 04 00 00 01 00 00 00 00 01 00 05
```

Breakdown: `29` opcode; `01 02 03 04` dst/callee/definition/signature; `00 00` group; `01` coordinate count; `00 00 00 00` coordinate (0,0); `01` operand count; `00 05` Explicit(reg5).

## WB-OPEN-EMPTY-FINAL-GROUP — OpenStream

- byte count: `9`
- SHA-256 of the raw record: `54901812e21086a34e954d54e2a38f61521f522e641f8edd547ce2e24dc99e3c`

```text
27 01 02 03 04 00 00 00 00
```

Breakdown: `27`; four one-byte IDs; group `00 00`; zero coordinates; zero operands. This is valid only when group 0 is also the final declared empty group.

## WB-OPEN-DEFAULTED-AND-OMITTED — OpenStream

- byte count: `52`
- SHA-256 of the raw record: `7186b528596faabb756c04e3985fb701ee70112bb088e2165e8b401b930e50c4`

```text
27 07 08 09 0a 01 00 02 01 00 00 00 01 00 01 00 02 01 aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa aa 0b 02
```

Breakdown before the digest: `27`; IDs `07 08 09 0a`; group `01 00`; two coordinates `(1,0),(1,1)`; two operands; tag `01`; then 32 bytes `aa`; value register `0b`; tag `02` with no payload.

## WB-FINISH-COMPLETE — FinishStream

- byte count: `3`
- SHA-256 of the raw record: `f469a54e07ffff5a7a711d796d5a7dee73ae2ac20929a31886b823de5fb2a561`

```text
28 0c 00
```

The first two bytes are opcode `28` and stream register `0c`; the remaining bytes are the exact outcome tag/payload.

## WB-FINISH-FAIL — FinishStream

- byte count: `4`
- SHA-256 of the raw record: `5e006262fac79d998351ed06574a4607b827e9f90f0eb1e1dffd945ac53236aa`

```text
28 0c 01 0d
```

The first two bytes are opcode `28` and stream register `0c`; the remaining bytes are the exact outcome tag/payload.

## WB-FINISH-CANCELLED — FinishStream

- byte count: `3`
- SHA-256 of the raw record: `5800ba7574840a44cd9dce0bcb6a36884e36041028896a91d672fe962d206f3d`

```text
28 0c 02
```

The first two bytes are opcode `28` and stream register `0c`; the remaining bytes are the exact outcome tag/payload.

## Hard rejection vectors

- Parent flat Open `27 02 03 02 00 04 01`: invalid/incomplete under the final group-aware Open payload.
- Child-old non-final Apply bytes beginning `27`: decode as Open only and fail the final-group verifier rule.
- Child-old Open beginning `28 07 08 ...`: decode as Finish only; outcome tag `08` is invalid.
- `1c`, `1d`, `1e`, `20`: unknown instruction opcodes.
- `2a`: unknown/unassigned instruction opcode.
