# Dependency selection notes

The implementation uses current maintained ecosystem crates where the Rust ecosystem has a clear owner and release line.

| Area | Selected crate | Version in this package | Rationale |
|---|---:|---:|---|
| Core reflection | none | n/a | Arcweft-owned traits and derive macros avoid serde as the core contract. |
| Derive macros | `syn`, `quote`, `proc-macro2` | workspace | Standard procedural macro stack already present in Arcweft workspace. |
| JSON | `serde_json` | 1.0.150 | Mature, widely used; adapter-only use. |
| TOML | `toml` | 1.1.2 | Matches Rust/Cargo ecosystem and existing Arcweft dependency style. |
| YAML | `yaml-rust2` | 0.11.0 | Pure parser/emitter path; avoids depending on `serde_yaml`, whose maintenance status does not fit Arcweft's default adapter policy. |
| MessagePack | `rmp-serde` | 1.3.1 | Established MessagePack crate with serde bridge; adapter only. |
| CBOR | `ciborium` | 0.2.2 | Conservative CBOR serde implementation, preserves dynamic values. |
| CSV | `csv` | 1.4.0 | Standard Rust CSV implementation with fast reader/writer builders. |
| Arrow / IPC | `arrow` | 59.0.0 | Apache Arrow Rust implementation, includes IPC/CSV/JSON subcrates. |
| Parquet | `parquet` | 59.0.0 | Same Apache Arrow release line as `arrow`. |
| Avro | `apache-avro` | 0.21.0 | Apache-owned Avro implementation, includes schema and codecs. |
| bincode legacy compat | `bincode` | =2.0.1 optional | 3.0.0 is an unmaintained compile-error notice release; legacy support is feature-gated and not default. |
| serde bytes | `serde_bytes` | 0.11.19 | Used in serde bridge and Avro/MessagePack interop behavior. |
| serde repr | `serde_repr` | 0.1.20 | Used as an interop reference; Arcweft core has its own `EnumRepr`. |

## ORC

ORC is kept as a design boundary in this package. The package does not enable a default Rust ORC crate because the Rust ecosystem does not currently have a first-party, actively maintained Apache ORC Rust crate comparable to `arrow`, `parquet`, or `apache-avro`. The adapter boundary is ready for a future ORC implementation.
