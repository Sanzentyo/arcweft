# Adapter Manifest Schema

Adapter manifests describe host-provided facts for one launch-profile adapter.
They are data formats, not adapter implementations. CLI, LSP, semantic checking,
verification, and runtime adapters consume the decoded manifest, while filesystem
reads and host execution stay in CLI/build/player adapter layers.

## Rust-Like Schema

```rust
pub struct AdapterManifestFile {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub symbols: Vec<AdapterSymbolFile>,
    pub methods: Vec<AdapterMethodFile>,
    pub functions: Vec<AdapterFunctionFile>,
    pub effects: Vec<String>,
    pub host_calls: Vec<AdapterHostCallFile>,
    pub tooling_docs: Vec<AdapterToolingDocFile>,
}

pub struct AdapterSymbolFile {
    pub name: String,
    pub ty: TypeLabel,
}

pub struct AdapterMethodFile {
    pub receiver: TypeLabel,
    pub name: String,
    pub return_type: TypeLabel,
    pub params: Vec<AdapterParamFile>,
}

pub struct AdapterFunctionFile {
    pub name: String,
    pub return_type: TypeLabel,
    pub params: Vec<AdapterParamFile>,
    pub effects: Vec<String>,
}

pub struct AdapterParamFile {
    pub name: String,
    pub ty: TypeLabel,
}

pub struct AdapterHostCallFile {
    pub id: String,
    pub effects: Vec<String>,
}

pub struct AdapterToolingDocFile {
    pub subject: String,
    pub docs: String,
}
```

`schema_version` is required and currently must be `1`.

## Type Labels

Type labels use Arcweft diagnostic spelling for primitive and common generic
types:

- `()`, `Bool`, `String`, `Char`
- `i8`, `i16`, `i32`, `i64`, `i128`, `isize`
- `u8`, `u16`, `u32`, `u64`, `u128`, `usize`
- `f32`, `f64`
- `Vec<T>`, `Seq<T>`, `Option<T>`
- any other label is treated as a named adapter/Rust type

Project-local adapter manifests do not infer Rust APIs from source files. Rust
exports are provided by separate `arcweft-rust-abi` metadata listed in the launch
profile `rust_metadata` field and then merged into the selected adapter manifest.

## TOML Example

```toml
schema_version = 1
id = "custom-file"
display_name = "Custom File"
effects = ["custom.read"]

[[symbols]]
name = "custom"
ty = "CustomApi"

[[methods]]
receiver = "CustomApi"
name = "read"
return_type = "String"
params = [{ name = "path", ty = "String" }]

[[functions]]
name = "custom.read"
return_type = "String"
effects = ["custom.read"]
params = [{ name = "path", ty = "String" }]

[[host_calls]]
id = "custom.read"
effects = ["custom.read"]

[[tooling_docs]]
subject = "custom.read"
docs = "Read custom content."
```

## JSON Example

```json
{
  "schema_version": 1,
  "id": "custom-http",
  "display_name": "Custom HTTP",
  "effects": ["http.respond"],
  "host_calls": [
    {
      "id": "http.respond",
      "effects": ["http.respond"]
    }
  ],
  "tooling_docs": [
    {
      "subject": "http.respond",
      "docs": "Send a server response."
    }
  ]
}
```

## Validation Rules

- `schema_version` must match the supported schema version exactly.
- `id` is the launch-profile adapter id and must match the profile's selected
  `adapter` when the manifest is used to satisfy that profile.
- `functions.effects` and `host_calls.effects` name effect capabilities required
  by the corresponding function or runtime host call.
- `effects` grants capabilities to the selected adapter environment.
- `host_calls` are runtime permissions. A native adapter must reject requests
  whose stable host-call id is not listed in the active manifest-derived set.
- Paths, environment variables, sockets, GPU devices, and host handles are not
  stored in this manifest. They belong in launch profiles or native adapter
  configuration.
