# Final contract

## 1. Normative language and status

`MUST`, `MUST NOT`, `SHALL`, and `SHALL NOT` are normative. This contract is
`READY_FOR_IMPLEMENTATION`. `OPEN_QUESTIONS=0`.

This is design-only. It does not contain a production patch, migration overlay,
compatibility layer, second decoder, or alternate resolver.

## 2. Authority order

Implementation SHALL apply authority in this order:

1. current accepted `main` and applicable `AGENTS.md`;
2. the strict schema-1 `SourceBackedManifest` decoder and generic
   `ManifestSourceMap`;
3. the landed `arcweft-project::content` binary/revision substrate;
4. the landed `arcweft-character::package::CharacterPackage` validation;
5. this contract's final family, admission, index, and consumer decisions;
6. superseded Lang-01.5.1.2 material only where this contract explicitly
   retains it.

Lang-01.5.1.2.1 supersedes every Lang-01.5.1.2 row that retained `Source` as a
root family or relied on `EntityKind::Source`. It does not reopen the binary,
package, decoder, containment, generated-metadata, or Character nominal
substrates.

## 3. Closed root-family decision

The final `ContentRootFamily` inventory SHALL contain exactly:

- `Character` — file-backed, package-complete, nominal Character identity;
- `Resource` — source-owned typed `res` declaration identity;
- `Activity` — source-owned abstract Activity identity whose concrete selected
  implementation remains manifest/generated-metadata policy.

No other entity family is a content root in schema 1. In particular:

- `Source` is absent;
- packaged `Asset` bytes are not promoted to roots;
- View, Style, Layer, Action, Signal, and other retained identities are not
  roots merely because a Resource or Activity depends on them;
- a function is not a root;
- a `Stream<T, E>` return type, generator classification, or external
  capability classification never changes that conclusion.

Wrong-family references SHALL fail through the ordinary typed project-symbol
resolver. There SHALL be no Source-specific historical diagnostic.

## 4. Accepted topology carrier

A present Character root SHALL retain exactly one complete
`Arc<CharacterPackage>` in `LoadedCharacterPackage`. The topology SHALL also
retain its source-backed manifest for source provenance, exact logical paths,
and exact host acquisition paths. It SHALL NOT split the package into a second
content-addressed blob authority.

The immutable published carrier is:

```rust
pub struct AcceptedProfileProject {
    topology: Arc<LoadedProfileTopology>,
    project_index: Arc<ProjectSemanticIndex>,
}
```

Its checked constructor SHALL accept only products with the same package, package version,
profile, and `ProjectTopologyRevision`, and SHALL verify every present/absent
Character fact against the topology package inventory. No caller may publish
these two `Arc`s independently after this cut.

## 5. Binary overlay decision

Text and binary overlays remain different types. `ProfileTopologyOverlaySet`
shall own ordered, duplicate-checked collections of:

- `ProfileTopologyOverlaySeed { path, source: Arc<str> }`;
- `ProfileTopologyBinaryOverlaySeed { path, bytes: Arc<[u8]> }`.

An exact path appearing in both collections is an overlay conflict. Binary
bytes SHALL NOT enter `SourceDocument` or `Arc<str>`. Text SHALL NOT enter the
binary carrier. Effective overlay bytes replace disk bytes before parsing,
package validation, semantic checking, and revision construction.

## 6. Sole revision decision

`ProjectTopologyRevision` is the only accepted topology/resource revision. Its
existing canonical transcript SHALL cover:

- the exact effective bytes of `arcw.toml`;
- every selected Arcweft module;
- every selected generated metadata document;
- each present Character manifest;
- each exact manifest-named Character layer payload;
- the accepted `ResourceTypeRegistryDigest`; and
- one `ProjectTopologyAbsenceRecord` for each accepted absent optional
  Character root.

The transcript excludes host paths, file timestamps, overlay origin, map
allocation order, and LSP document versions. Replacing disk bytes with an
overlay containing identical bytes preserves the revision. Any effective byte,
semantic digest, or absence-record change changes the revision.

`SourceSetRevision` may remain an internal text-analysis optimization. It SHALL
NOT identify an accepted project, bundle inventory, LSP generation, watch
namespace, or cache namespace.

## 7. Presence semantics

Only Character roots have filesystem presence. For each content-unit root in
`(ContentUnitId, root_ordinal)` order:

- required + exact manifest path `NotFound` => `RequiredContentRootAbsent`;
- optional + exact manifest path `NotFound` + zero selected-profile typed
  references => accepted `AbsentOptional(ProjectTopologyAbsenceRecord)`;
- optional + exact manifest path `NotFound` + one or more selected-profile typed
  references => `ReferencedOptionalContentRootAbsent`;
- manifest present in any form => validate it exactly as required content;
  malformed UTF-8, decode failure, identity mismatch, missing layer, corrupt
  PNG, dimension mismatch, or unconsumed explicit payload fails the candidate.

Only `NotFound` for the exact contained manifest path is absence. Permission,
I/O, symlink-containment, and metadata failures are errors. Directory existence
is irrelevant. The loader SHALL NOT scan directories to infer package members.

Resource and Activity roots must resolve and pass visibility checks regardless
of `DependencyDemand`. Their presence is always `Present`; optional demand is
packaging/profile policy, not permission for an unknown semantic identity.

## 8. Referenced-optional definition

A Character is referenced when the selected-profile reference inventory
contains at least one successfully typed reference whose final target is that
`CharacterId`. The inventory is the union of:

- typed entity references in every Arcweft module admitted by the selected
  profile;
- typed Resource value dependencies in those modules;
- selected entry and Activity binding references;
- selected generated metadata/export references; and
- manifest-owned profile references represented by the existing source map.

The content-root declaration itself does not count as a reference. Dead-code
reachability is not used: admission is conservatively project-wide within the
selected profile's admitted module/metadata set. No source text search,
formatted label parsing, prefix test, or directory scan may contribute.

## 9. Manifest-owned facts

`AcceptedProjectContent` SHALL be the sole final content admission authority.
For each content unit it stores:

- `ContentUnitId`;
- ordered root facts with authored `ContentRootRef`, resolved typed target,
  source span, and presence;
- `ManifestVisibility`;
- `DependencyDemand`;
- the selected `ProfileContentSpec`;
- exact revision-bound spans for the unit table, root entry, visibility,
  demand, profile-content table, residency, placement, and compression; and
- the accepted `ProjectTopologyRevision` at the aggregate level.

The final `ProjectSemanticIndex` SHALL contain one `Arc<AcceptedProjectContent>`
and expose read-only inherent accessors. The old source-HIR
`ProjectGraphRelationKind::ContentRoot` and `index_content_root_relations` SHALL
be deleted in the same authority cut. No copied graph edge or side table
survives.

## 10. Source-map decision

The sole `arcweft-launch::ManifestSourceMap` remains authoritative. Its owning
`ManifestTokenPath` enum and existing inherent path conversion SHALL be
extended for content-unit and profile-content table/field/index paths. No
content-specific side map, extension trait, free string converter, or manifest
reparse is permitted.

Missing required source evidence is a candidate-fatal
`ManifestSourceEvidenceMissing`, not a fabricated zero range.

## 11. Resolver and family validation

The content-root resolver SHALL consume the same typed project symbol,
Resource registry, Character registration, Activity registration, import,
alias, re-export, visibility, ambiguity, and collision authorities used by
ordinary semantic resolution. It SHALL return a typed
`AcceptedContentRootTarget` or one structured error:

- unknown;
- ambiguous/collision;
- not visible;
- wrong family;
- bounded-work exhaustion.

The loader's current `strip_prefix("@character.")` preloading branch SHALL be
deleted. Character path construction begins only after typed resolution to
`AcceptedContentRootTarget::Character`.

## 12. Project-index construction order

Root resolution does not depend on a partially built `ProjectSemanticIndex`.
The transaction SHALL:

1. produce the accepted typed semantic world/project symbol authority;
2. resolve content roots and collect typed references;
3. acquire/validate Character packages and classify optional absence;
4. compute `ProjectTopologyRevision`;
5. construct `AcceptedProjectContent`;
6. construct the final `ProjectSemanticIndex` with that content authority; and
7. construct `AcceptedProfileProject` and publish once.

`ProjectSemanticIndex::new` SHALL become crate-private and require
`Arc<AcceptedProjectContent>`. There is no publicly observable index lacking
content facts.

## 13. Consumer contract

- **Bundle:** iterate unique present Character targets in
  `AcceptedProjectContent`, obtain the exact `LoadedCharacterPackage`, and call
  the existing inherent `BundleCharacterPackage::from_character_package`.
  Absent optional roots create no fake bundle entry.
- **Watch:** watch each present exact resource as `MustExist`; for each accepted
  absence watch only the exact expected Character manifest path as
  `OptionalMayAppear`. Do not recursively watch a package directory to infer
  layers.
- **LSP:** publish one `Arc<AcceptedProfileProject>` per accepted generation.
  Binary open-document/host overlays flow through `ProfileTopologyOverlaySet`.
  All caches and stale checks include `ProjectTopologyRevision`.
- **Compiler/Agent/CLI:** obtain manifest-owned content facts only from the
  final `ProjectSemanticIndex`; they shall not decode `arcw.toml` or inspect the
  filesystem.

## 14. Atomicity

Every stage before `AcceptedProfileProject::try_new` is candidate-local. On any
failure:

- no new topology is published;
- no new project index is published;
- no catalog is published;
- no bundle/cache namespace is created;
- no LSP generation advances;
- no watch inventory replaces the accepted one; and
- no candidate overlay is served through the prior generation.

The previous accepted object may remain pointer-identical and queryable. That
is rollback of publication, not last-known-good acceptance of the failed
candidate.

## 15. Direct deletion and compatibility

The migration SHALL directly delete:

- author-facing `source` declaration syntax and HIR;
- runtime `Source<T, E>`;
- `EntityKind::Source` and `TypeKind::Source`;
- all Source content-root variants and tags;
- source `content` declaration syntax/HIR/sema/tooling ownership;
- `ProjectGraphRelationKind::ContentRoot` and its source-HIR producer;
- loader string-prefix Character root discovery;
- any content-root directory inference, manifest reparse, dual resolver, or
  compatibility reader.

No released schema or persisted artifact requiring compatibility was evidenced.
The replacement is a direct unreleased internal authority switch.

## 16. Completion predicates

Implementation is complete only when all of the following hold:

- the family enum is closed and contains no Source path;
- every accepted content unit is represented by `AcceptedProjectContent`;
- every Character package is complete and revision-covered;
- optional absence is explicit, typed, and watchable;
- bundle, watch, LSP, compiler, Agent, and CLI consume the same accepted
  carrier;
- failed candidates publish nothing;
- source `content` and Source inventories are absent through typed/behavioral
  evidence; and
- every `TEST_MATRIX.md` row is green under the validation plan.
