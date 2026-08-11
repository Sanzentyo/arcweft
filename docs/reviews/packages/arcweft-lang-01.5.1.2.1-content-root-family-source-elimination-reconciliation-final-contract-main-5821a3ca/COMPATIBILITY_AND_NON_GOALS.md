# Compatibility statement and non-goals

## Compatibility statement

This is a clean semantic cut.

There is no compatibility support for:

- author-facing `source` declarations;
- `Source<T, E>`;
- `EntityKind::Source` or `TypeKind::Source`;
- Source content roots;
- source-owned `content` declarations;
- legacy source-content root arrays;
- a Source root alias implemented as a function, resource, or Stream;
- a dual old/new family reader;
- a removed-syntax parser branch;
- a migration-only Source diagnostic;
- a provisional Source topology/bundle/cache/LSP tag;
- last-known-good candidate acceptance.

The old family type/variant spellings are not type aliases and are not
deprecated wrappers. They are absent.

A previously authored reference is allowed to appear verbatim in ordinary
diagnostic source text. Quoting source evidence is not a compatibility reader.

Configured resource public-ID namespace policy remains owned by the resource
extension contract. An exact configured resource is always categorized as
`ConfiguredResource`; it may not acquire Source runtime behavior or be
presented as a Source compatibility category.

## Stream relationship

The Stream callable model is orthogonal to content packaging.

No callable is admitted based on:

- `Stream<T, E>` return type;
- own-scope `yield`;
- external capability origin;
- runtime Stream definition/instance identity;
- queue/replay/lifecycle policy;
- function name or attribute.

A future use case that packages live producers must introduce a new semantic
category with a closed typed identity, explicit manifest policy, lifecycle and
bundle semantics, and a separate reviewed contract. It cannot revive Source or
reuse this package's configured-resource category by convention.

## Preserved compatibility-free substrate

The following remain unchanged:

- strict one-decoder manifest authority;
- `SourceBackedManifest` source provenance;
- exact binary bytes outside `SourceDocument`;
- text/binary overlays;
- Character nominal identity and complete package validation;
- exact generated metadata admission;
- project containment;
- optional Character absence facts;
- canonical topology transcript/tags;
- accepted resource registry digest;
- transaction-before-publication;
- existing resource/retained-reference type separation.

## Non-goals

This package does not:

- implement or patch production code;
- redesign Stream runtime/AWBC/save/wire semantics;
- settle the independent Lang-01.3.1.2.2 curried external argument projection;
- add a public resource-extension manifest decoder;
- make callables into content roots;
- make configured resources file-backed optional roots;
- add CSS, Takumi, directory discovery, or source gates;
- add host I/O to core/data-format crates;
- change Character package wire format;
- change topology transcript version or frozen tags;
- select a new entry/profile schema;
- define a second symbol/resource world;
- treat a prior accepted snapshot as current success after failure.
