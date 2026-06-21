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
| bincode legacy compat | `arcweft-codec-binary` optional `bincode-legacy` feature | Legacy interop only | Not the primary Arcweft binary format; disabled by default. |
| ORC | future adapter boundary | Not default | Reserved until a suitably maintained Rust crate is selected. |

## DSL runtime surface

Arcweft source now selects serialization formats with the built-in `DataFormat`
enum instead of string labels. The type checker accepts short enum variants
when the expected type makes the enum unambiguous:

```arcw
let bytes: Bytes = data.encode(["hello"], .Json)
let value: AgentValue = data.decode(bytes, .Json)
let shape: DataShape = data.shape(value)
```

Runtime execution handles `Json`, `Toml`, `Yaml`, `MessagePack`, `Cbor`, and
`Avro` through the external pure-call adapter boundary. `Avro` uses an Arcweft
dynamic-value envelope so DSL calls can round-trip without a source-level Avro
schema argument. `Csv`, `ArrowIpc`, `Parquet`, and `ArcweftBinary` are also
available through the same DSL runtime boundary; the tabular formats expect a
sequence of records.
