# Decision register

| ID | Final decision | Rejected alternatives |
|---|---|---|
| D01 | Adapter-native declarations own `AdapterOpaqueTypeProducerId` in `manifest::nominal`. | Core dependency, raw `String`, sema-only wrapper. |
| D02 | Rust ABI owns `ArcweftRustOpaqueTypeProducerId` in `producer.rs`. | Reusing adapter type, core type, or package/path metadata. |
| D03 | Both IDs preserve exact UTF-8 and reject empty/control/`std.`; no ID-specific maximum. | Trimming, normalization, path grammar, inferred value, arbitrary extra limit. |
| D04 | Reserved namespace is exact case-sensitive `starts_with("std.")`; lower constructors and adapter-sema enforce it. | Reserving `std`, case-insensitive matching, allowing external standard claims. |
| D05 | `AdapterNominalDeclaration::try_new(path, arity, opaque_producer, visibility, source_label)`. | Optional producer, setter, side map. |
| D06 | Adapter schema 2 key is `nominal_types[].opaque_producer`. | `producer`, `opaqueProducer`, admission field. |
| D07 | Rust type field/key is `ArcweftRustTypeDecl::opaque_producer` / `types[].opaque_producer`. | Field on `AdapterRustType`, package-level default. |
| D08 | Public root/direct row `Deserialize` bypasses are removed; private schema-2 DTOs follow preflight. | Generic serde first, schema-1 reader, default field. |
| D09 | Header preflight precedes every required field. | Letting serde report missing producer on schema 1. |
| D10 | Schema-2 producer validation is global by category then authored row index. | First arbitrary serde traversal error. |
| D11 | Empty type manifest needs no producer; functions need none. | Manifest-wide producer or dummy value. |
| D12 | `AdapterRustType::opaque_producer()` delegates to `decl`; no second field/setter/mutable override. | Copied producer or post-mount patch. |
| D13 | Derive syntax is one required `#[arcweft(opaque_producer = "...")]`. | Inferring from type name, accepting several aliases, optional attribute. |
| D14 | Macro diagnostics and spans are exact and deterministic. | Generic syn/serde text as public contract. |
| D15 | External admission is always exact and not authored. | Producer-wide descriptor switch. |
| D16 | A producer may own many exact identities; duplicate producer is not collision. | One producer per nominal, producer-only index. |
| D17 | Adapter-sema private enum with inherent projection is the sole cross-layer conversion. | Public trait, extension trait, scattered helpers. |
| D18 | `AcceptedNominalInventoryInput` producer is mandatory. | `Option`, fallback from identity, late side table. |
| D19 | Accepted instantiation/substitution copy producer unchanged. | Recompute from semantic identity or arguments. |
| D20 | Generated source contains length-prefixed exact producer payload and source-map range. | Quoted/escaped raw value, path-derived comment. |
| D21 | Environment manifest digest domain moves to v2 and rows include producer. | Silent grammar change under v1. |
| D22 | Accepted nominal catalog domain moves to v2 and includes producer. | Producer-insensitive catalog, side digest. |
| D23 | External type-input v1, semantic nominal identity, Rust structural metadata, AWBC, and save versions remain unchanged. | Replacing type identity with producer or unnecessary wire bumps. |
| D24 | Rust ABI artifact hash remains BLAKE3 of deterministic pretty JSON bytes. | New parallel hash or producer sidecar. |
| D25 | Standard adapter producer constants are explicitly authored, not computed. | `format!` from adapter/type paths. |
| D26 | Fixture producers use reviewed `fixture.<crate>.<case-or-domain>` literals. | Production-ID derivation or one global fixture default. |
| D27 | Schema cuts are hard deletions with no compatibility interval. | Dual reader/writer, alias, migration map. |
| D28 | Adapter-sema/lang-sema switch is one protected atomic group. | Producerless catalog publication between commits. |
| D29 | Parent A1.2 resumes only after all descriptor/publication/digest/fixture gates pass. | Implementing projection against incomplete external evidence. |
| D30 | No production overlay in this delivery. | Patch, branch, source gate, post-build injection. |
