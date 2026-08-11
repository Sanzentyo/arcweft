# Admission transaction

## 1. Transaction input

One candidate receives:

- the requested project/package and selected profile;
- an immutable disk snapshot abstraction supplied by the loader;
- one `ProfileTopologyOverlaySet`;
- the existing strict decoder limits, source limits, Character limits, and
  semantic work limits;
- the prior accepted `Arc<AcceptedProfileProject>` only as the publication
  rollback target.

The candidate owns all allocations until final commit.

## 2. Deterministic phase order

| Phase | Operation | Candidate product | Failure rule |
| --- | --- | --- | --- |
| 1 | Normalize and duplicate-check text/binary overlays | effective overlay map | fail before any read |
| 2 | Read effective `arcw.toml`, strict-decode once, select profile, resolve contained layout | `SourceBackedManifest`, resolved profile | decoder/containment error |
| 3 | Read selected source modules and generated metadata through effective overlays | exact `SourceDocument`/binary records | I/O/overlay/limit error |
| 4 | Parse, lower, link, register, and type-check the selected semantic world | immutable registered semantic candidate | parse/HIR/sema error |
| 5 | Obtain every required manifest source span through `ManifestTokenPath` | source evidence records | `ManifestSourceEvidenceMissing` |
| 6 | Resolve roots in `(ContentUnitId, root_ordinal)` order using typed catalogs | resolved root candidates | unknown → ambiguity → visibility → wrong-family |
| 7 | Collect and resolve selected-profile typed reference candidates | accepted reference inventory | typed resolution/limit error |
| 8 | Freeze the ordered root/reference view and initialize an empty typed Character acquisition cache | deterministic admission schedule | invariant error only |
| 9 | Walk root facts in `(ContentUnitId, root_ordinal)` order: map each Character ID to its exact contained paths, probe the exact manifest, reject required absence, reject referenced optional absence immediately, and otherwise accept explicit absence or fully read/validate the present package (deduplicated by `CharacterId`) | finalized presence facts + complete loaded packages | first fatal root error in authored order |
| 10 | Finalize the sorted typed reference inventory and root/package coherence | immutable admission inventory | reference/presence invariant error |
| 11 | Build canonical present/semantic/absence transcript | `ProjectTopologyRevision` | transcript error |
| 12 | Construct `AcceptedProjectContent` | immutable content authority | invariant error |
| 13 | Build `ProjectSemanticIndex` with that exact content authority | immutable semantic index | index error |
| 14 | Freeze `LoadedProfileTopology` with the same revision and exact watch inventory | immutable topology | topology invariant error |
| 15 | `AcceptedProfileProject::try_new` cross-checks both products | one accepted carrier | revision/package/fact mismatch |
| 16 | Compare-and-publish one generation | accepted environment | stale completion loses without publication |

## 3. Root and package deduplication

Root facts remain distinct by `(ContentUnitId, root_ordinal)` even when several
facts resolve to the same target. This preserves independent visibility,
demand, profile policy, and source evidence.

Acquisition is deduplicated by `CharacterId`: one present Character target
produces one `LoadedCharacterPackage`. Every present fact references that
package by typed target. Duplicate root spelling is not itself an error;
project-symbol ambiguity and conflicting identity/path evidence are errors.

## 4. No pre-resolution Character loading

The loader must not inspect `ContentRootRef::as_str`, strip `@character.`, or
construct an asset path before semantic resolution. Only
`AcceptedContentRootTarget::Character(id)` enters the path mapper. The retained
mapping is deterministic:

```text
@character.npc.alice
  -> CharacterId("character.npc.alice")
  -> assets/npc/alice.awchar
  -> assets/npc/alice.awchar/character.awchar.json
```

The exact filename remains the current Character package manifest filename;
this contract does not add a second `.awchar` layout.

## 5. Publication CAS

The publisher compares the candidate's request generation and predecessor
accepted pointer/revision. A stale completion is discarded. The accepted
generation advances exactly once for a successful new carrier. A byte-identical
candidate may reuse the prior carrier and does not need to advance generation.
