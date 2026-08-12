# Exact deletion set

The final cut contains none of the following:

1. producerless `AdapterNominalDeclaration` constructor or direct literal;
2. producerless `ArcweftRustTypeDecl` literal/builder/macro expansion;
3. unit `AcceptedNominalSemantics::Opaque` or any optional producer field;
4. producerless `AcceptedNominalInventoryInput`, `try_new_opaque`, or
   `AcceptedNominalType::new` overload;
5. schema-1 adapter manifest constant, writer, JSON reader, TOML reader,
   private/public DTO, fixture, snapshot, golden, example, or success test;
6. schema-1 Rust ABI constant, writer, reader, DTO, fixture, snapshot, golden,
   example, build artifact, or success test;
7. public direct serde manifest/declaration decode that bypasses header
   preflight;
8. `#[derive(ArcweftType)]` success without exactly one explicit
   `opaque_producer` helper option;
9. macro aliases, inferred producer, package/type-name fallback, or generated
   metadata reconstruction;
10. producer default, manifest-wide default, migration map, compatibility
    carrier, dual reader, dual writer, serde alias, feature/source gate, or
    legacy-success branch;
11. `AdapterRustType` producer field, setter, mutable override, side map, or
    constructor parameter independent of `decl`;
12. external `admission` descriptor field or external producer-wide success;
13. producer-only unique index, one-producer-per-nominal rule, registry
    callback, generic producer trait, extension trait, schema publication, or
    post-build side table;
14. generated-source header `adapter-manifest-v1` and v1 producerless row
    snapshots;
15. `arcweft.environment-manifest.v1\0` and
    `arcweft.accepted-nominal-catalog.v1\0` writers/readers/goldens;
16. temporary source-string reconstruction for producer diagnostics;
17. any production overlay, patch, diff, generated `.rs`, or post-build
    injection included by this design package.

Old names may appear only in frozen predecessor/request/evidence prose and in
negative assertions that prove they are rejected. They may not remain as a
successful production symbol or wire path.
