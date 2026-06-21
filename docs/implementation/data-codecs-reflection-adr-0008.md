# ADR-0008: Data codecs, reflection, save/config/http integration

## Status

Proposed final implementation package.

## Context

Arcweft core is Sans I/O. Data formats such as source, manifests, bundle, save snapshot, and schema are data-only layers. Filesystem, network, wall-clock, and runtime resources belong to host/adapters.

The current repository already has JSON/TOML parsing in adapter-local manifest code. This is useful but not a reusable Arcweft data boundary: serde_json/toml errors leak into local code, and there is no common reflection/type-shape model for save, config, HTTP, and analytics formats.

## Decision

Introduce `arcweft-data` as a builtin data contract crate and keep all concrete file/wire formats in adapter crates.

```text
builtin:
  TypeShape
  Reflect
  Encode / Decode
  DecodeLimits
  DataError with field path
  Codec registry trait
  save envelope schema
  config merge policy

adapter:
  JSON
  YAML
  TOML
  MessagePack
  CBOR
  CSV
  Arrow IPC
  Parquet
  Avro
  Arcweft Binary
  bincode interop
  file/env/http sources
```

`arcweft-data` does not depend on serde. serde is allowed only in adapters and bridge crates.

## Reflection design

Reflection is syntactic and compile-time generated, not runtime scanning.

```arcw
#[derive(Encode, Decode, Reflect)]
#[arcweft(rename_all = "snake_case", deny_unknown_fields)]
record SaveData {
    schema_version: U32,
    #[arcweft(bytes)]
    screenshot_hash: Bytes,
}
```

Rust equivalent:

```rust
#[derive(ArcweftEncode, ArcweftDecode, ArcweftReflect)]
#[arcweft(rename_all = "snake_case", deny_unknown_fields)]
pub struct SaveData {
    pub schema_version: u32,
    #[arcweft(bytes)]
    pub screenshot_hash: arcweft_data::Bytes,
}
```

Generated implementations provide:

- `Encode::encode_value`
- `Decode::decode_value`
- `Reflect::shape`

The generated shape carries record fields, enum variants, rename policy, byte policy, repr enum policy, and unknown-field policy.

## Byte and repr enum behavior

serde_bytes-equivalent behavior is builtin as `Bytes` and `BytesFormat`.

serde_repr-equivalent behavior is builtin as `EnumRepr`. C-like enums can be represented by integer discriminant when annotated.

## Format coverage

| Format | Crate | Role |
|---|---|---|
| JSON | `arcweft-codec-json` + `serde_json` | HTTP/config/debug default |
| YAML | `arcweft-codec-yaml` + `yaml-rust2` | human-authored config/docs |
| TOML | `arcweft-codec-toml` + `toml` | project/config format |
| MessagePack | `arcweft-codec-msgpack` + `rmp-serde` | compact binary API/interchange |
| CBOR | `arcweft-codec-cbor` + `ciborium` | compact standard binary interchange |
| CSV | `arcweft-codec-csv` + `csv` | tabular import/export |
| Arrow IPC | `arcweft-codec-arrow` + `arrow` | analytics/inter-process columnar data |
| Parquet | `arcweft-codec-arrow` + `parquet` | columnar persisted data |
| Avro | `arcweft-codec-avro` + `apache-avro` | schematized Hadoop/streaming interchange |
| ORC | design-only adapter boundary | no default Rust crate selected in this package |
| bincode | `arcweft-codec-binary` compat | optional interop only |
| Arcweft Binary | `arcweft-codec-binary` | recommended fast internal binary |

## Consequences

- save/config/HTTP can share one data error and limit model.
- concrete external formats remain replaceable.
- serde can be used where it is format ecosystem glue, but not as Arcweft's core type model.
- bincode interop is isolated from primary save/network formats.
