# Resource revision and overlay rules

## 1. Effective-resource rule

For each exact contained host path, admission selects either the matching
validated overlay or disk bytes. The selected bytes are the only bytes visible
to decoding, semantic checking, Character package validation, and revision
construction.

Overlay origin is provenance for diagnostics and editor coordination, not
semantic identity.

## 2. Overlay validation

`ProfileTopologyOverlaySet::try_new` SHALL:

1. normalize every host path under the existing project-containment authority;
2. reject duplicate text paths;
3. reject duplicate binary paths;
4. reject a path present in both collections;
5. sort each collection by normalized host path;
6. charge path and payload bytes to existing topology limits; and
7. retain exact `Arc` allocations without converting binary to UTF-8.

An overlay is consumed only when the accepted typed resource plan names its
exact path and kind. Any unconsumed overlay is an error. This is how explicit
unreferenced layer payloads are rejected without scanning disk directories.

## 3. Canonical revision transcript

The current `ProjectTopologyRevision::try_for_inventory` transcript is
retained. Present records are keyed by package ID/version, typed resource kind,
semantic key, and logical path. Resource bytes use existing `BuildDigest`.
Semantic records use existing semantic digests. Absences use exact typed
fields.

No new hash, overlay digest, package digest, root-fact digest, or LSP digest is
introduced.

## 4. Included mutations

| Mutation | Revision result | Reason |
| --- | --- | --- |
| `arcw.toml` effective byte change | changes | project manifest record changes |
| selected `.arcw` byte change | changes | module record changes |
| selected generated metadata byte change | changes | metadata record changes |
| Character manifest byte change | changes | Character manifest record changes |
| exact layer byte change | changes | Character layer record changes |
| resource type registry semantic digest change | changes | semantic record changes |
| optional Character changes absent ↔ present | changes | absence record removed/added and present records added/removed |
| root target/policy/visibility/demand change | changes | accepted manifest or source/metadata bytes change |
| disk replaced by overlay with different bytes | changes | effective bytes change |
| disk replaced by overlay with identical bytes | unchanged | effective bytes are identical |
| overlay LSP version changes but bytes do not | unchanged | LSP version is not semantic identity |
| host absolute path or mtime changes with same logical path/bytes | unchanged | host acquisition metadata is excluded |
| map insertion order changes | unchanged | transcript is canonically sorted |

## 5. Cache and stale keys

Any cache representing an accepted project uses at least:

```text
(package identity, selected profile, ProjectTopologyRevision)
```

Feature-specific revisions may be appended, but `SourceSetRevision` may not
replace `ProjectTopologyRevision`. LSP document versions are request-staleness
inputs and never enter persisted semantic identity.
