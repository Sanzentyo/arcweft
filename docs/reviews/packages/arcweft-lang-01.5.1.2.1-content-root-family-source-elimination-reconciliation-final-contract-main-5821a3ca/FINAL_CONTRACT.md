# Final contract

## 1. Authority

This document closes Lang-01.5.1.2.1 with `OPEN_QUESTIONS=0`.

It is based on `Sanzentyo/arcweft` `main` at
`5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`. It is a design and validation artifact only. The explicit delivery
constraint is that production code remains unchanged.

The returned Lang-01.5.1.2 package remains authoritative for the following
already selected substrate:

- the strict schema-1 single manifest decoder and `SourceBackedManifest`;
- exact manifest token/source provenance;
- text and binary overlay separation;
- exact immutable binary bytes outside `SourceDocument`;
- deterministic Character package path derivation;
- complete `CharacterPackage` membership/PNG/dimension validation;
- generated metadata admission;
- project containment;
- required/optional presence semantics;
- canonical typed `ProjectTopologyRevision`;
- candidate-before-publication transaction ownership;
- no directory inference and no last-known-good candidate acceptance.

This correction supersedes every prior row that admits, names, reserves, or
projects a Source content root or derives content ownership from the source
`content` declaration.

## 2. Final closed family

The only accepted `ContentRootFamily` values are:

```text
Character
AuthoredEntity(Flow)
AuthoredEntity(View)
AuthoredEntity(Action)
AuthoredEntity(Activity)
AuthoredEntity(Asset)
AuthoredEntity(Signal)
AuthoredEntity(Metric)
AuthoredEntity(Layer)
ConfiguredResource(exact ResourceTypeId)
```

The accepted target inventory is correspondingly closed:

```text
Character(exact CharacterId)
AuthoredEntity(exact EntityId, exact AuthoredContentRootFamily)
ConfiguredResource(exact ResourceDeclarationIdentity)
```

No Source variant, callable variant, unknown/other variant, provisional tag, or
extension escape hatch exists in either enum.

## 3. Source elimination

After the Lang-01.3.1 migration:

- the parser has no author-facing `source` declaration;
- HIR has no Source top-level node;
- sema has no `TypeKind::Source` and no `EntityKind::Source`;
- public/runtime/AWBC surfaces have no `Source<T, E>` or separate Source plan;
- source-owned `content` syntax and its HIR/sema relation producer are removed;
- no alias, compatibility reader, role attribute, source gate, or migration
  parser is introduced.

A manifest reference whose spelling previously denoted a Source declaration is
processed by the ordinary final reference parser and typed resolver. With no
matching final target it fails as the ordinary unresolved family/target case
selected by that resolver. If it resolves to a callable or another surviving
wrong symbol category, it fails as ordinary wrong symbol kind/family. There is
no spelling-specific Source diagnostic.

## 4. Stream callables

A live producer is not packaged content.

The following are always non-roots unless a future contract introduces a new
semantic category with a distinct manifest policy and typed identity:

- an ordinary function returning `Stream<T, E>` without own-scope `yield`;
- an ordinary authored generator returning `Stream<T, E>`;
- an external capability operation returning `Stream<T, E>`;
- a derived Stream transformation;
- a callable alias or reexport of any of the above.

The content-root resolver does not inspect return type, generator mode,
external-operation mode, function name, attributes, or runtime origin when
classifying roots. There is no `AcceptedContentRootTarget::Callable`.

## 5. Manifest ownership

`ContentRootRef` remains authored only at:

```text
content-units.<ContentUnitId>.roots[ordinal]
```

A profile selects a content unit and supplies residency/placement/compression
policy; it does not author a second root list. A profile entry remains a
separate entry-point reference and is not a content root.

All root occurrences are collected from the accepted
`SourceBackedManifest`/source map. No source reparse, directory walk, or old
source `content` node may reconstruct a root.

## 6. Resolution authority

One exact accepted project world supplies:

- final source/entity symbols and visibility;
- exact configured resource declarations;
- exact generated metadata;
- exact Character package candidates and optional-absence reservations;
- exact source, symbol, resource, and topology revisions.

Family classification is implemented on the owning Arcweft enums/indexes.
The loader may reserve the typed Character category for acquisition, but it
must not classify arbitrary roots with `strip_prefix`, a local match table, a
function-name test, or an extension trait.

Aliases and reexports preserve occurrence spans while canonicalizing to the
original declaration identity. Ambiguity, collision, inaccessible visibility,
wrong family, wrong symbol kind, and stale revision are failures. No
first-wins rule exists.

## 7. Presence and demand

Only Character is file-backed in this contract. Therefore only Character can
produce a present-package candidate or explicit optional-absence fact.

Authored entities and configured resources are semantic-pending candidates.
Their existence is decided by the accepted typed world; `optional` does not
turn an unresolved semantic root into an absence fact.

Required/optional, profile selection, runtime reference, and present-invalid
rules are exact in `REVISION_AND_ADMISSION.md`.

## 8. Project publication

The final ProjectIndex is built from the accepted manifest inventory. It does
not synthesize an `EntityKind::Content` node and does not reuse the old
source-content `ContentRoot` relation producer.

Bundle, watch, and LSP projections consume the same
`Arc<AcceptedContentInventory>` and exact topology resources. They may filter or
format that inventory; they may not rescan source, manifests, resource
directories, or Character directories.

## 9. Atomicity and fallback prohibition

All candidate products are constructed off to the side. Publication is a
single commit of a coherent tuple containing the accepted manifest, topology,
symbol/resource worlds, accepted content inventory, ProjectIndex, compiler
identity, bundle plan, watch inventory, and LSP snapshot.

Any failure publishes none of that tuple. A previously accepted snapshot may
remain internally stored for the host's own continuity, but it is not returned,
reported, labelled, or consumed as success for the failed request. This
contract has `fallback=false`.

## 10. Completion criteria

Production implementation is complete only when the positive, negative,
revision, deletion, consumer, and transaction tests in `TEST_MATRIX.md` pass,
Tier 2 is run, and the repository structural audit is recorded.

Removal is proven by parser/compiler rejection and the absence of executable
typed nodes in accepted products. A repository substring scan is not a
completion gate.
