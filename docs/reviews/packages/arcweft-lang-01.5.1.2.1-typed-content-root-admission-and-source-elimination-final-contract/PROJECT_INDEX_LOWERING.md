# Manifest-to-ProjectSemanticIndex lowering

## 1. Final semantic product

The final `ProjectSemanticIndex` embeds one
`Arc<arcweft_project::content::AcceptedProjectContent>`. That product is the
sole query authority for manifest-owned content. Existing entity, callable,
nominal, type, debug, and non-content graph inventories remain in the index.

## 2. Lowering inputs

Lowering receives only accepted typed products:

- strict `SourceBackedManifest` and its source map;
- selected `ResolvedLaunchProfile`;
- registered project symbol/resource/Character/Activity authorities;
- resolved root targets;
- typed selected-profile references;
- finalized presence facts;
- `ProjectTopologyRevision`.

It receives no path scanner, raw manifest text, source body string, or
partially accepted index.

## 3. Content unit construction

Units are visited by `ContentUnitId` order. Roots retain authored order and
zero-based ordinal. A unit constructor rejects:

- no roots;
- non-contiguous or duplicate ordinals;
- mismatched unit IDs;
- absent Resource/Activity targets;
- absent Character under required demand;
- absent optional Character without exact absence record;
- a profile content policy for a different unit/profile;
- a span from another manifest revision.

Two roots may resolve to the same target. They remain two facts. Package
acquisition and bundle output deduplicate by final typed target.

## 4. Index construction

`ProjectSemanticIndex::new(program_hash, accepted_content)` is crate-private.
All source/HIR indexing then populates the remaining inventories. Since content
facts already exist, no intermediate public index can omit them.

The index exposes inherent read-only queries:

- `accepted_content()`;
- `topology_revision()`;
- `content_unit(id)`;
- `content_references(target)`;
- `content_roots_for_target(target)`.

## 5. Deleted old authority

Delete in the same compiling cut:

- `ProjectGraphRelationKind::ContentRoot`;
- its display/string mapping;
- `index_content_root_relations`;
- HIR `content` item dispatch into project indexing;
- Agent/CLI/LSP code that infers manifest content from generic graph relations.

The general graph may still contain ordinary `ReferencesEntity` dependencies.
Those edges do not become a second content-root inventory.

## 6. Collision and visibility

Root resolution uses the ordinary final project symbol authority before facts
are built. Duplicate public IDs, ambiguous imports, alias collisions,
visibility violations, and wrong-family targets fail candidate construction.
No “first manifest wins” or “first symbol wins” rule exists.
