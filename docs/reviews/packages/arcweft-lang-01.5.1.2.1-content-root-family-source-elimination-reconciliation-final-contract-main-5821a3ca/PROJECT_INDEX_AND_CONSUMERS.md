# ProjectIndex, bundle, watch, and LSP projections

## 1. One accepted inventory

Every downstream projection receives the same immutable:

```text
Arc<AcceptedContentInventory>
```

bound to the same package, profile, `ProjectTopologyRevision`, symbol revision,
resource registry/declaration world, and `ProgramHash`.

A consumer may select, partition, or render facts. It may not:

- decode the manifest again;
- parse source again to discover roots;
- scan project/asset/Character directories;
- infer a root from a callable signature;
- rebuild a configured-resource index;
- accept a previous snapshot after current failure.

## 2. Manifest-to-ProjectIndex projection

ProjectIndex receives one `ProjectContentUnitFact` per accepted manifest content
unit and one `ProjectContentRootFact` per authored occurrence.

### Content-unit fact

Carries:

- exact `ContentUnitId`;
- accepted manifest visibility;
- exact `DependencyDemand`;
- selected/unselected profile policy and exact source spans;
- exact unit key/table/visibility/demand spans;
- roots in authored ordinal order.

### Root fact

Carries:

- exact `ContentRootOccurrenceId`;
- exact authored `ContentRootRef`;
- full value and selection spans;
- `Present(AcceptedContentRootTarget)` or exact optional Character absence;
- profile/runtime reference classification.

No `EntityKind::Content` entity is synthesized. No source declaration is
retained solely to own a graph edge.

The graph uses typed endpoints:

```text
ContentUnit(ContentUnitId)
  --ContainsContentRoot-->
ContentRootOccurrence(ContentRootOccurrenceId)

ContentRootOccurrence(ContentRootOccurrenceId)
  --ResolvesContentRoot-->
ContentRootTarget(AcceptedContentRootTarget)
```

The resolution edge exists only for a present target. Optional absence remains
a fact on the occurrence.

The old PublicId-to-PublicId source-content `ContentRoot` relation producer is
deleted in the same cut. It is not redirected through an alias or compatibility
node.

## 3. Entity and configured-resource targets

### Authored entity

The ProjectIndex target is the canonical original `EntityId` and
`AuthoredContentRootFamily`. Alias/reexport bindings remain available as source
evidence but do not create duplicate targets.

The entity declaration's existing ProjectIndex symbol remains the authority for
kind, source anchor, semantic hash, visibility, and Agent/tooling metadata. The
content fact points to it; it does not copy a second mutable symbol record.

### Configured resource

The target is the exact `ResourceDeclarationIdentity`, including:

- `EntityId`;
- `PublicId`;
- exact nominal `ResourceTypeId`.

ProjectIndex exposes the accepted resource declaration/dependencies through the
resource owner. A raw `@image.*`, `@voice.*`, or other family spelling does not
create a resource symbol without an exact accepted declaration.

`ResourceRef<T>`, `AssetRef<P>`, and `RetainedIdentityRef<K>` remain distinct
typed value categories. Content admission consumes their already typed
dependency facts and never scans strings.

### Character

A present Character target points to the accepted `CharacterId` and shared
`AcceptedCharacterPackage`. Its exact manifest/layer bytes remain topology
resources. An optional absence is not added to the Character catalog or entity
symbol map.

## 4. Generated metadata

Accepted generated/external-module metadata is part of the exact topology
resource set and has its own accepted `SourceDocument`/typed model.

Any content-root dependency contributed by generated metadata must be emitted by
that metadata's typed decoder/linker with exact source spans. The reference
collector consumes those facts under the same topology revision.

Metadata text is never reparsed by bundle, watch, or LSP to reconstruct roots.
A metadata hash/identity/family/ABI mismatch fails before content publication.

## 5. Bundle projection

The compiler/partition owner converts the accepted inventory to bundle input.

For each selected profile:

- present required roots are included according to the profile's
  residency/placement/compression policy;
- present optional roots are included only according to the accepted partition
  and reachability policy;
- accepted optional absences contribute no bundle payload/section;
- Character inclusion uses exact accepted manifest and PNG bytes;
- configured resources use the resource declaration's existing bundle-section
  owner and codec, not a new content-root codec;
- authored entity roots contribute reachability/partition seeds, not invented
  payload bytes;
- no Source table, Source root section, or callable-as-content section exists.

The existing `LinkGraph::reachability` and `ContentPartitionPlan` decide
startup/on-demand/bundle placement only after content admission. They do not
decide whether missing content is acceptable.

The bundle model remains Sans I/O. A compiler/loader adapter hands it exact
typed entries; the bundle crate does not open paths or depend on watcher/LSP
state.

## 6. Watch projection

The watch adapter derives its set from:

1. exact present topology resource records; and
2. exact optional Character absence paths from the accepted content inventory.

Watch entries are:

```text
MustExist          for accepted present manifest/module/metadata/Character files
OptionalMayAppear  for accepted optional-absent Character manifest paths
```

Rules:

- no authored entity/configured-resource root causes new filesystem discovery;
- no callable causes a watch entry;
- no removed Source reference causes a watch entry;
- Character layer watch paths come only from the accepted Character manifest;
- optional absence watches the exact expected package/manifest path, not a
  directory glob;
- disk and overlay use the same logical watch identity; host adapters may
  suppress a disk watch while an open overlay is authoritative, but cannot
  change semantic inventory;
- any changed path triggers a new complete candidate transaction; watchers do
  not mutate accepted inventory in place.

## 7. LSP projection

The LSP accepted-project snapshot consumes the same accepted tuple.

It publishes:

- manifest content-unit symbols/facts with exact manifest spans;
- present root links to existing authored entity/resource/Character targets;
- exact diagnostics produced by the failed/current candidate;
- optional-absence information as a diagnostic/status fact, not a Character
  declaration;
- revision keys from the accepted topology/symbol world.

It does not publish:

- a Source symbol, Source root, Source type, or compatibility node;
- a source `content` symbol;
- a callable as a root because it returns `Stream<T, E>`;
- a symbol for an optional-absent Character;
- symbols or links from a failed partial candidate.

Go-to-definition for an accepted root selects:

- Character manifest span/path for Character;
- canonical original declaration source for authored entity;
- canonical configured-resource declaration source for configured resource.

Aliases/reexports may be shown as secondary binding evidence but canonical
identity remains the original declaration.

## 8. Cache and revision projection

ProjectIndex, compiler cache, bundle cache, watch state, and LSP cache keys use
the inherent accepted `ProgramHash`/`ProjectTopologyRevision` projections.

No consumer formats an ad hoc key from a root string. A cache hit is valid only
when package/profile, topology revision, symbol revision, and consumer schema
match.

A stale overlay, stale symbol table, wrong topology revision, or stale generated
metadata candidate fails before any cache/LSP/watch state is replaced.

## 9. Exact layered atomic publication

The lower accepted boundary is owned by
`arcweft-project-loader::topology::AcceptedProfileTopology`:

```rust
pub struct AcceptedProfileTopology {
    loaded: Arc<LoadedProfileTopology>,
    symbols: Arc<ProjectSymbolTable>,
    resources: Arc<AcceptedResourceDeclarationIndex>,
    content: Arc<AcceptedContentInventory>,
    project_index: Arc<ProjectSemanticIndex>,
    program_hash: ProgramHash,
    bundle_plan: Arc<ContentPartitionPlan>,
    watch: Arc<[ProfileTopologyWatchEntry]>,
}
```

Construction is all-or-nothing and returns this value only after all fields
agree on package/profile/topology/symbol/resource revisions. Returning the value
does not mutate a process-global state.

Each host owns its final swap without reversing dependencies:

- compiler/CLI consumes the returned value locally;
- bundle construction returns a complete bundle product before any output path
  replacement;
- the watch host replaces its active exact watch set only after it has the
  accepted value;
- `arcweft-lsp::profiles::state` builds an
  `AcceptedProjectSnapshot` from the accepted value and atomically swaps the
  snapshot/caches/diagnostics together.

The loader does not depend on LSP. An LSP snapshot-build failure leaves both the
previous accepted loader value and previous LSP snapshot labelled as prior
state; it does not publish any field from the failed candidate or report the
prior state as success for the attempted revision.

Any constructor/host-candidate failure returns diagnostics and commits none of
that host's new state. There is no successful fallback publication.
