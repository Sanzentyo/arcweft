# Required decisions

## D1 — Complete `.awchar` carrier

**Selected:** one `Arc<CharacterPackage>` is the complete byte authority. A loader-owned `LoadedCharacterPackage` pairs it with the already decoded `SourceBackedCharacterManifest` solely for source provenance and with topology resource IDs solely for location/watch lookup.

`CharacterPackage` continues to own exact manifest bytes and every manifest-named layer. Its membership invariants remain: no missing, duplicate, or unreferenced payload. It is extended in place to validate each payload by fully decoding the PNG stream and requiring decoded width/height to equal the unique referencing variant rectangle.

**Rejected:**

- manifest plus an independently authoritative layer map — two package authorities;
- package descriptor plus a new content-addressed blob set — a second resource system not required by this migration;
- manifest-only topology plus later filesystem reads — violates atomic admission and consumer reuse.

## D2 — Binary overlay ownership

**Selected:** retain `ProfileTopologyOverlaySeed { source: Arc<str> }` for text and add `ProfileTopologyBinaryOverlaySeed { bytes: Arc<[u8]> }` for binary data. They are separate public inputs and separate maps.

Rules:

1. Both seed types require an absolute normalized contained host path.
2. A logical path may occur in at most one text seed and at most one binary seed; a path present in both classes is `OverlayKindConflict`.
3. A binary overlay is considered only for an exact layer path named by an accepted character manifest. It replaces disk bytes completely; an invalid overlay never falls back to disk.
4. Every supplied binary overlay must be consumed exactly once by the candidate. An unconsumed overlay is rejected.
5. Disk reads are issued only for exact manifest-named paths. No directory enumeration is permitted.
6. Dependency packages receive a separate typed binary dependency seed; binary data is never squeezed through `ProfileDependencyResourceSeed`'s source-document contract.

## D3 — One topology revision

**Selected:** `ProjectTopologyRevision(BuildDigest)` in `arcweft-project::content`. It uses the existing `BuildDigest::of` BLAKE3 owner and a new canonical transcript; no parallel hash primitive or source-text hash system is added.

The revision covers exact accepted bytes, the accepted `ResourceTypeRegistryDigest` as a typed semantic record, and explicit absence records. Disk/overlay origin, absolute host paths, URIs, mtimes, permissions, and read order are excluded. Identical logical resources and bytes yield the same revision regardless of acquisition origin. Any manifest, selected source, generated metadata, character manifest, layer byte, accepted registry digest, logical identity/path, or absence-state change yields a different revision.

`SourceSetRevision` remains the exact revision of source documents for source indexes. It is not a second topology revision and is not accepted as a cache/LSP generation key after this cut.

## D4 — Required/optional admission

For each manifest content unit:

- `required` means every file-backed root must be present and valid whether or not the selected profile lists the unit.
- `optional` and present still means fully validate; optional never masks corruption, identity mismatch, missing layers, malformed PNG, or semantic-resolution failure.
- `optional` and absent creates a candidate absence only when the selected profile does not list the unit.
- The selected profile references a unit exactly when `profiles.<selected>.content` contains that content-unit ID.
- Runtime references are exact typed entity/resource reference occurrences in the selected accepted source/runtime closure after alias/reexport canonicalization. Reachability does not erase a reference: even a dead branch must not retain a typed node naming absent content.
- Final accepted absence requires `optional && !profile_referenced && !runtime_referenced`.
- `required` absence is `RequiredRootMissing`.
- Optional absence with profile and/or runtime reference is `OptionalRootReferencedMissing { referenced_by }`, where `referenced_by` is `Profile`, `Runtime`, or `ProfileAndRuntime`.
- Source-owned and configured-resource roots have no file-absence state: they must resolve in the accepted world even for optional/unselected units. Their demand affects packaging policy, not declaration validity.

The initial candidate may carry an optional absence, but no accepted topology/project index/catalog/cache/LSP generation is published until project-wide typed-reference reconciliation succeeds. File-backed roots are grouped by canonical `CharacterId`: any required occurrence makes absence `RequiredRootMissing`; otherwise any profile-selected occurrence makes absence `OptionalRootReferencedMissing(Profile)`; when all occurrences are optional/unselected, an absent target creates one shared acquisition/watch state and one explicit absence fact per manifest occurrence.

## D5 — Closed family model

**File-backed:** `character` only.

**Source-owned semantic roots:** `flow`, `view`, `action`, `activity`, `source`, `asset`, `signal`, `metric`, `layer`.

**Configured resources:** any actual accepted `res` declaration, resolved to its exact `ResourceDeclarationIdentity`. A public-ID prefix or extension manifest descriptor without a matching accepted declaration is insufficient.

**Invalid:** launch-only `entry`; removed `content`; nested/scoped/runtime products such as choice, option, dialogue line, text, input, button, style, scene, capture, hook, slot, target, presentation target, and scroll region; old Image/Voice/Se/Bgm/AudioBus/MixerSnapshot/Ducking/Motion/Rig source-family ownership; type/proof/function names; and unknown families.

Resolution precedence and the full table are normative in `CONTENT_ROOT_FAMILIES.md`.

## D6 — Manifest facts in `ProjectIndex`

The accepted inventory injects one content-unit fact and one root-occurrence fact per manifest occurrence. Facts include:

- `ContentUnitId` and root ordinal;
- exact authored `ContentRootRef`;
- visibility and demand;
- selected/unselected profile state plus exact `ProfileContentSpec` when selected;
- exact manifest source spans for unit key/table, root value/string content, visibility, demand, and selected profile policy;
- canonical target identity or explicit optional absence;
- profile/runtime reference flags; and
- `ProjectTopologyRevision`.

`ProjectGraphSymbolKind::ContentUnit` is manifest-owned. `ProjectGraphRelationKind::ContentRoot` remains a useful semantic relation but is produced from manifest facts, not from a source `content` node.

## D7 — Bundle/watch/LSP consumers

- **Bundle:** consumes `AcceptedContentInventory`; character packages are passed directly to the existing `BundleCharacterPackage::from_character_package`. Optional absences produce no bundle payload. Existing reachability/content partitioning decides inclusion; it does not rediscover files.
- **Watch:** consumes a topology-owned exact path inventory. Present characters contribute manifest and every named layer path. An absent optional character contributes its expected manifest path only; once it appears, the next candidate yields its layer paths. No directory watch is used to infer members.
- **LSP:** stores the accepted inventory and `ProjectTopologyRevision` in the same candidate/environment generation. A binary-only change produces a fresh generation and cache namespace. Source diagnostics use retained manifest source maps; binary locations are typed logical-path locations, not fake `SourceSpan`s.
