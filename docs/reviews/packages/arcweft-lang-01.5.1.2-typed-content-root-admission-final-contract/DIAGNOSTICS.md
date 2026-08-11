# Typed diagnostics and source ownership

## Stable codes

| Code | Typed condition |
|---|---|
| `aw.content.root.unknown_family` | family is neither a closed built-in nor an accepted configured-resource family |
| `aw.content.root.wrong_family` | known family is not permitted as a content root |
| `aw.content.root.duplicate` | same canonical root occurs twice in one unit |
| `aw.content.root.required_missing` | required file-backed root is absent |
| `aw.content.root.optional_referenced_missing` | optional absent root is selected and/or runtime reachable |
| `aw.content.root.semantic_target_missing` | source-owned target cannot resolve in the accepted world |
| `aw.content.root.resource_target_missing` | configured-resource family is known but no accepted declaration matches |
| `aw.content.root.resource_target_ambiguous` | accepted resource declaration index violates unique public identity |
| `aw.content.character.manifest_missing` | exact Character package manifest is absent |
| `aw.content.character.identity_mismatch` | manifest Character ID differs from the root identity |
| `aw.content.character.manifest_invalid` | strict Character manifest decode/validation fails |
| `aw.content.character.layer_missing` | manifest-named payload is absent |
| `aw.content.character.layer_duplicate` | package input repeats a layer path |
| `aw.content.character.layer_unreferenced` | explicit package/overlay input is not named by the manifest |
| `aw.content.character.layer_invalid_png` | complete PNG decode fails |
| `aw.content.character.layer_dimensions` | decoded dimensions differ from the variant rectangle |
| `aw.content.overlay.kind_conflict` | same path supplied as both text and binary |
| `aw.content.overlay.duplicate` | duplicate seed in one overlay class |
| `aw.content.overlay.unconsumed` | binary overlay is not consumed by an exact named layer |
| `aw.content.topology.revision_conflict` | candidate/inventory/index/cache revisions disagree |
| `aw.content.limit` | an inclusive resource/byte/diagnostic/work limit is exceeded |
| `aw.content.arithmetic_overflow` | count/length/work arithmetic cannot be represented |

## Primary and related ownership

| Failure | Primary location | Related locations |
|---|---|---|
| unknown/wrong/duplicate root | exact root string content in `arcw.toml` | first duplicate root for duplicate case |
| required manifest missing | exact root string content | expected normalized manifest path as structured path data |
| optional profile-referenced missing | exact root string content | selected `profiles.<id>.content.<unit>` key/table and policy fields |
| optional runtime-referenced missing | exact root string content | first deterministic typed reference occurrence in the selected accepted source/runtime closure, then bounded additional occurrences in source order |
| source/configured target missing | exact root string content | nearest same-family candidates only when derived from typed symbol/resource lookup |
| Character identity mismatch | Character manifest `character` string content | manifest root occurrence in `arcw.toml` |
| Character manifest invalid | exact failing Character manifest token/span | root occurrence; whole manifest only when no narrower span exists |
| layer missing | Character manifest `asset` string content | root occurrence and expected normalized layer path |
| duplicate/unreferenced layer | duplicate/unreferenced typed binary logical path | manifest asset span when one exists; root occurrence |
| invalid PNG/dimensions | Character manifest `asset` string content | typed binary resource logical path; decoder byte range only when available |
| text/binary overlay conflict | structured host path on both seeds | both overlay classes; no fabricated source span |
| revision conflict | candidate boundary object/revision values | topology, index, or environment revisions involved |
| limit/overflow | exact resource/root token when attributable | observed and maximum counts; otherwise transaction-level location |

## Determinism

Diagnostics sort by:

```text
(document identity or binary logical-path identity,
 primary byte/path position,
 diagnostic code,
 content-unit ID,
 root ordinal)
```

Binary inputs use a typed `BinaryResourceLocation { resource_id, logical_path, byte_range }`. A `SourceSpan` is created only for an actual UTF-8 `SourceDocument` revision.
