# Error model and deterministic precedence

## 1. Global order

Every serialized or programmatic path closes in this order:

1. raw syntax / complete header decode;
2. schema version support;
3. schema-2 required producer presence and string type;
4. producer empty/control spelling validity;
5. exact reserved namespace `std.`;
6. remaining descriptor/model validation;
7. package mount and full Rust ABI validation;
8. nominal duplicate, capacity, and work accounting;
9. atomic catalog publication.

Within a category, authored row index is the tie-breaker. JSON object-key order
never changes row order. Adapter-native rows precede Rust-export rows only at
the later combined publication boundary; each source codec first reports its
own schema-2 errors.

## 2. Adapter JSON examples

| Input condition | Result |
|---|---|
| malformed JSON plus any header/body defects | existing raw JSON syntax error |
| root array | `MalformedSchemaVersion::RootNotObject` |
| no `schema_version` | `MissingSchemaVersion { Json }` |
| duplicate header | `MalformedSchemaVersion::DuplicateSchemaVersion` |
| `schema_version: "2"` | `MalformedSchemaVersion::WrongType(String)` |
| schema 1, missing producer | `UnsupportedSchema { found: 1, expected: 2 }` |
| schema 2, row 1 missing, row 0 empty | row 1 `MissingOpaqueProducer` (presence outranks spelling) |
| schema 2, row 0 control, row 1 `std.x` | row 0 `InvalidOpaqueProducer::ControlCharacter` |
| schema 2, row 0 reserved, malformed arity elsewhere | row 0 `InvalidOpaqueProducer::ReservedStandardNamespace` |
| all producer passes valid, malformed body | existing typed body/model error |

## 3. Adapter TOML exception

TOML duplicate keys are rejected by the raw TOML parser before a header value
can be represented. That raw syntax error is phase 1 rather than
`DuplicateSchemaVersion`; this is the only codec-specific representation
difference. Ordering remains deterministic because no body interpretation has
occurred. All other TOML header and producer cases follow the global order.

## 4. Rust ABI JSON

Rust ABI `from_json` follows the same header/value/presence/spelling/reserved/body
order. After body conversion, `ArcweftRustManifest::validate` repeats schema and
producer invariants for programmatically built manifests before package/type
validation. `serde_json::from_str::<ArcweftRustManifest>` is not a public
success route.

## 5. Macro order

Rust token parsing is first. Helper attributes are visited in lexical order.
Malformed helper syntax/unknown options are reported at the first offending
attribute. Duplicate is reported at the second key. Only after one syntactically
valid value is selected does empty/control/reserved validation run. Missing is
reported only after all helper attributes have been inspected.

## 6. Adapter-sema defensive errors

For one row, `InvalidOpaqueProducer` outranks `ReservedOpaqueProducer` because
core spelling must be valid before namespace policy. All producer rows are
projected before package mount, duplicates, limits, digest construction, or
publication. Source-map duplicate/missing evidence is an invariant failure and
aborts before accepted inventory construction.

## 7. Post-publication guarantee

The successful `SourceBackedEnvironmentRegistrationInput`, accepted nominal
record, accepted nominal type, runtime checked type, generated source, and both
registration/catalog digests all carry producer evidence. No successful public
API can observe a missing producer after publication.
