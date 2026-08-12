# Schema 2, header preflight, and derive contract

## 1. Adapter wire spellings

The only accepted root version is:

```json
"schema_version": 2
```

Every adapter-native nominal row has:

```json
"opaque_producer": "example.adapter.native"
```

The TOML spelling is the same key under each `[[nominal_types]]` table:

```toml
opaque_producer = "example.adapter.native"
```

There is no `admission`, `producer_wide`, package-level producer, default, or
alias. Private schema-2 DTO fields are raw `String` until the presence/type and
spelling passes have completed.

## 2. Rust ABI wire spelling

The only accepted root version is `"schema_version": 2`. Every row in `types`
has a required `"opaque_producer"` string. `types: []` is valid without any
producer elsewhere. Functions remain unchanged.

## 3. Header preflight public representation

Adapter codec:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterManifestSourceFormat { Json, Toml }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterManifestValueKind {
    Null,
    Boolean,
    Integer,
    IntegerOutOfRange,
    Float,
    String,
    Array,
    Object,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AdapterManifestSchemaHeaderProblem {
    #[error("manifest root must be an object/table")]
    RootNotObject,
    #[error("schema_version appears more than once")]
    DuplicateSchemaVersion,
    #[error("schema_version has wrong value kind {found:?}")]
    WrongType { found: AdapterManifestValueKind },
    #[error("schema_version integer is outside u32")]
    IntegerOutOfRange,
}
```

Adapter codec error additions:

```rust
MissingSchemaVersion { format: AdapterManifestSourceFormat },
MalformedSchemaVersion {
    format: AdapterManifestSourceFormat,
    problem: AdapterManifestSchemaHeaderProblem,
},
UnsupportedSchema {
    format: AdapterManifestSourceFormat,
    found: u32,
    expected: u32,
},
MissingOpaqueProducer { site: AdapterManifestFieldSite },
MalformedOpaqueProducer {
    site: AdapterManifestFieldSite,
    found: AdapterManifestValueKind,
},
InvalidOpaqueProducer {
    site: AdapterManifestFieldSite,
    #[source]
    error: AdapterOpaqueTypeProducerIdError,
},
```

`AdapterManifestFieldSite` contains source format and zero-based nominal row
index. Existing source-offset/range evidence remains attached by the codec's
source owner; the public index is stable even when parser range internals vary.

Rust ABI JSON exposes the analogous `ArcweftRustJsonValueKind`,
`ArcweftRustSchemaHeaderProblem`, `ArcweftRustTypeFieldSite`, and errors:
`MissingSchemaVersion`, `MalformedSchemaVersion`, `UnsupportedSchema`,
`MissingOpaqueProducer`, `MalformedOpaqueProducer`, `InvalidOpaqueProducer`,
`Json`, and `Manifest`.

## 4. JSON preflight algorithm

A custom top-level Serde map visitor parses the complete JSON token stream,
records the first/duplicate `schema_version`, and skips all body values without
deserializing schema-2 DTOs. Complete raw syntax errors outrank the header.
The visitor distinguishes root-not-object, missing field, duplicate field,
non-integer kinds, negative/out-of-u32 integers, and valid `u32`.

After a supported version is proved, the decoder parses a generic JSON value
under existing input limits and performs these global passes:

1. locate every structurally recognizable nominal/type row and require a string
   `opaque_producer` in authored row order;
2. validate empty/control spelling in authored row order;
3. reject `std.` in authored row order;
4. deserialize the complete private schema-2 DTO and report remaining body
   shape/model errors;
5. perform package mount/Rust ABI validation, duplicate/capacity/work checks,
   then atomic publication.

If `nominal_types`/`types` or an individual row has the wrong shape, that issue
is retained as the first remaining-body error while producer checks continue
for every row that is structurally recognizable. Thus category precedence is
global rather than an accident of serde traversal.

JSON `2.0` is a float and malformed. `-1` and integers above `u32::MAX` are
`IntegerOutOfRange`. Syntax errors anywhere in the document remain first.

## 5. TOML preflight algorithm

The TOML parser first parses the complete source into `toml::Value`. TOML
syntax, including duplicate-key rejection, is raw-syntax phase 1. Root must be
a table. `schema_version` must be an integer in `0..=u32::MAX`; TOML floats and
strings are malformed. A supported version is required before any producer
field is inspected. The same three global producer passes and private DTO body
pass then run.

A TOML header preflight is not a schema-1 body reader: schema 1 is rejected
without interpreting its nominal rows.

## 6. Exact derive syntax and diagnostics

The declaration is:

```rust
#[proc_macro_derive(ArcweftType, attributes(arcweft))]
```

Exactly one option is mandatory across all helper attributes:

```rust
#[arcweft(opaque_producer = "example.gameplay")]
```

A trailing comma is accepted. No bare form, list value, path value, alias, or
additional key is accepted. Options are visited in source order.

| Condition | Exact diagnostic | Primary span |
|---|---|---|
| missing | `ArcweftType requires #[arcweft(opaque_producer = "...")]` | ADT identifier |
| duplicate | `duplicate ArcweftType opaque_producer option` | second `opaque_producer` key |
| malformed list/form | `malformed ArcweftType attribute; expected #[arcweft(opaque_producer = "...")]` | whole helper attribute |
| non-string | `ArcweftType opaque_producer must be a string literal` | supplied value |
| unknown key | `unsupported ArcweftType option; expected opaque_producer` | unknown key |
| empty | `ArcweftType opaque_producer must not be empty` | string literal |
| control | `ArcweftType opaque_producer contains a control character at byte {byte}` | string literal |
| reserved | `ArcweftType opaque_producer must not use the reserved std. namespace` | string literal |

Validation uses `LitStr::value()`; byte offsets are UTF-8 offsets in the decoded
string. Expansion embeds the literal through the validated constructor:

```rust
opaque_producer:
    arcweft_rust_abi::ArcweftRustOpaqueTypeProducerId::try_new(#literal)
        .expect("ArcweftType macro validated the opaque producer literal"),
```

The `expect` is an unreachable invariant assertion for macro-generated code,
not a runtime fallback. The macro must not construct from an identifier, path,
package, or metadata hash.
