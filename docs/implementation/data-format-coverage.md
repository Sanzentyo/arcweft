# Format coverage

| Format | Crate | Default role | Notes |
|---|---|---|---|
| JSON | `arcweft-codec-json` / `serde_json` | HTTP/config/debug default | Uses Arcweft `Value`; serde only in adapter. |
| YAML | `arcweft-codec-yaml` / `yaml-rust2` | Authoring/config import | Avoids `serde_yaml`; aliases rejected. |
| TOML | `arcweft-codec-toml` / `toml` | Project config | Record/table oriented. |
| MessagePack | `arcweft-codec-msgpack` / `rmp-serde` | Compact network/debug payload | Uses serde bridge only inside adapter. |
| CBOR | `arcweft-codec-cbor` / `ciborium` | Binary interop | Conservative structured binary. |
| CSV | `arcweft-codec-csv` / `csv` | Tabular import/export | Sequence of records, scalar cells. |
| Arrow IPC | `arcweft-codec-arrow` / `arrow` | In-memory/table interchange | Sequence of records -> `RecordBatch`. |
| Parquet | `arcweft-codec-arrow` / `parquet` | Columnar storage/export | Shares Arrow table conversion. |
| Avro | `arcweft-codec-avro` / `apache-avro` | Schemaful stream/storage | Requires Avro schema JSON at adapter construction. |
| Arcweft Binary | `arcweft-codec-binary` | Recommended save/network binary | Stable Arcweft-owned wire, checksum handled in save envelope. |
| bincode interop | `arcweft-codec-binary` optional `bincode-interop` feature | Interop only | Not the primary Arcweft binary format; disabled by default. |
| ORC | future adapter boundary | Not default | Reserved until a suitably maintained Rust crate is selected. |

## Typed JSON save boundary

Fixed-version typed JSON saves use the checksummed `SaveEnvelope` and have one
decode authority: `decode_strict_typed_json_save`. It rejects unknown fields at
any payload depth, duplicate fields, trailing JSON values, trailing envelope
bytes, mismatched schema IDs/codecs, and old or future schema versions unless a
separate explicitly typed migration boundary owns that format. There is no
permissive typed JSON save decoder or predecessor reader.

## DSL runtime surface

Arcweft source now selects serialization formats with the built-in `DataFormat`
enum instead of string labels. The type checker accepts short enum variants
when the expected type makes the enum unambiguous:

```arcw
let bytes: Bytes = data.encode(["hello"], .Json)
let value: AgentValue = data.decode(bytes, .Json)
let shape: DataShape = data.shape(value)
let shaped: AgentValue = data.decode(bytes, .Json, shape)
```

`arcweft-data::DataFormat::ALL` is the authoritative format inventory. Semantic
registration and tooling completion consume it directly, so adding a format no
longer requires a separately maintained list in the language layer.

Runtime execution handles `Json`, `Toml`, `Yaml`, `MessagePack`, `Cbor`, and
`Avro` through the external pure-call adapter boundary. `Avro` uses an Arcweft
dynamic-value envelope so DSL calls can round-trip without a source-level Avro
schema argument. `Csv`, `ArrowIpc`, `Parquet`, and `ArcweftBinary` are also
available through the same DSL runtime boundary; the tabular formats expect a
sequence of records.

Two-argument `data.decode(bytes, format)` is the dynamic decode form. It is
available for JSON and the current dynamic Avro envelope. Shape-required
formats use the explicit form `data.decode(bytes, format, shape)`, where
`shape` is a `DataShape` value such as `data.shape(value)`. The runtime converts
that value into a real `TypeShape` and dispatches through the same core codec
`decode_value` path used by save/config/http adapters. Schema-bound Avro decode
still belongs to an Avro-schema-bearing adapter surface rather than this
`TypeShape`-only call.
