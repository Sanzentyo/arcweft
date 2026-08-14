# Canonical byte grammar index

All integer encodings in this package are little-endian. Every domain byte string includes the shown final NUL. No Serde/Debug/Display bytes are implicit authority.

| Boundary | Domain / placement | Exact payload |
|---|---|---|
| Character manifest fingerprint | `arcweft-character-manifest-fingerprint-v1\0` | existing source transcript in Decision 01 |
| Character catalog digest | `arcweft.character-catalog.runtime.v1\0` | `u32(1), u32(rows), sorted [str32(CharacterId), fingerprint32]` |
| Runtime View ID | `arcweft.runtime-view-id.v1\0` | `u32(1), str32(ViewId)` |
| View registry digest | `arcweft.view-registry.runtime.v1\0` | `u32(1), u32(rows), sorted public row grammar from Decision 02` |
| Project root ID | no hash/domain | exact `RuntimeSemanticTypeId[32]` byte copy |
| Producer root ID | no hash/domain | exact `RuntimeSemanticTypeId[32]` byte copy |
| Generation identity | retained `arcweft.runtime-generation-contract.v1\0` | retained parent canonical body |
| Custom-field digest | retained `arcweft.character-dialogue-runtime-custom-fields.v1\0` | retained parent exact descriptor rows |
| AWBC nominal domain table | after runtime types, before constants | `u32 count`, then tag 00 + root32 or tag 01 + producer str32 |
| AWBC MakeRecord | existing opcode `0x0f` | LE dst/domain/type/counts/string IDs/register IDs |

`str32(s) = u32_le(s.len_bytes) || UTF8(s)`. Sequence counts use `u32_le`. `ViewSchemaId` is `u64_le`; `RustViewId` and stable blend codes are `u32_le`; coordinates are exact 32-byte newtype payloads.
