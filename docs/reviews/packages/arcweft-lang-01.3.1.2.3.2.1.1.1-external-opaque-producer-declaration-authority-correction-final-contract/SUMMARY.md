# Delivery summary

- Sequence: `Lang-01.3.1.2.3.2.1.1.1`
- Repository basis: `Sanzentyo/arcweft@78f50f5b5ac082745bab91b7373a6602918a436d`
- Source request SHA-256: `a7ab7d47f50804bae5a5b9fff1e5e39b7c97922bdda191b444216724d56ba9a7`
- Parent contract ZIP SHA-256: `93af482a2914ca4a9e6b985aa7a09c040f569bd71141611dcaa4d579ac01640c`
- Status: **READY_FOR_IMPLEMENTATION**
- Open questions: **0**
- Production implementation: **not performed**
- Production overlay: **not included**

## Selected closure

Adapter-native opaque declarations gain one mandatory
`AdapterOpaqueTypeProducerId`. Rust exports gain one mandatory
`ArcweftRustOpaqueTypeProducerId` on `ArcweftRustTypeDecl`. Both are explicit,
validated descriptor data; neither is inferred from a name, path, package,
hash, layout, or semantic identity. `AdapterRustType` merely delegates an
immutable accessor to its stored declaration.

Adapter manifest schema 1 and Rust ABI schema 1 are deleted in favor of strict
schema 2. All decoders perform a header/version preflight before inspecting the
required producer field. Schema 1 therefore fails as unsupported even when its
rows omit `opaque_producer`. There is no dual reader, default, migration map,
optional producer, source-string reconstruction, or post-build overlay.

External rows always publish exact admission. `arcweft-adapter-sema` is the
sole conversion boundary into core `RuntimeOpaqueTypeProducerId`; it attaches a
producer payload range in generated source and places mandatory producer
evidence in `AcceptedNominalInventoryInput`. One producer may own many exact
nominal identities. Only the exact case-sensitive prefix `std.` is reserved to
fixed core/CharacterDialogue constructors.

The adapter environment-manifest digest moves to v2 and the accepted nominal
catalog digest moves to v2. Rust ABI deterministic JSON naturally changes
because schema/version/producer bytes change. The external type-input digest,
semantic accepted-nominal identity digest, Rust structural metadata digest,
AWBC ABI/codec/tag allocation, and session-save schema remain unchanged by this
correction.
