# ProjectIndex, deletion, and consumer contract

## Manifest-to-`ProjectIndex` lowering

`ProjectSemanticIndexInput` receives the final `AcceptedContentInventory`. For every unit it publishes:

```rust
pub struct ProjectContentUnitFact {
    pub id: ContentUnitId,
    pub visibility: ManifestVisibility,
    pub demand: DependencyDemand,
    pub profile: ContentUnitProfileSelection,
    pub source: ContentUnitManifestSource,
    pub topology_revision: ProjectTopologyRevision,
}

pub struct ProjectContentRootFact {
    pub occurrence: ContentRootOccurrenceId,
    pub authored: ContentRootRef,
    pub state: AcceptedContentRootState,
    pub referenced_by: ContentRootReferenceKind,
    pub source: ContentRootOccurrenceSource,
    pub topology_revision: ProjectTopologyRevision,
}
```

Rules:

1. Facts are ordered by `ContentUnitId`, then root ordinal.
2. Root ordinal is the manifest array position and is semantic occurrence identity; reordering roots changes occurrence facts and topology revision through manifest bytes.
3. `ProjectGraphSymbolKind::ContentUnit` uses the manifest content-unit key as its source anchor.
4. `ProjectGraphRelationKind::ContentRoot` connects that unit symbol to the canonical present target. An optional absence remains a typed fact but has no target edge.
5. Alias/reexport spelling never replaces the canonical target in graph identity; the occurrence retains the authored source.
6. `ProjectSemanticIndex` stores `ProjectTopologyRevision` and rejects construction against a mismatched inventory revision.
7. `ProgramHash` and all generation/cache keys incorporate `ProjectTopologyRevision` through an inherent typed constructor. Source-only string formatting is removed.

## Source `content` deletion

After the manifest fact path passes focused tests, delete the old source authority in one atomic cut:

- syntax: `EntityDeclKind::Content`, `EntityDeclBody::Content`, `ContentDeclBody`, parser branches, CST/AST kinds, formatting/completion support, and source-content tests;
- HIR: every content declaration/body/root variant and lowerer;
- sema: `EntityKind::Content` when no longer otherwise used, symbol registration, duplicate checks, source-body root resolution, and the source producer of content-root relations;
- tooling/LSP: completion, symbol, hover, rename, formatter, and fixture expectations that expose source `content`;
- docs/examples/fixtures: migrate to `arcw.toml` content units or remove obsolete-only coverage.

The graph relation itself remains because the manifest owns it. Final parser behavior is ordinary current-grammar rejection/recovery. There is no historical AST node, removed-spelling recognizer, dedicated removed-syntax diagnostic, or source-text gate.

## Bundle

- `AcceptedContentInventory` is the only package input.
- A selected or compiler-reachable present Character target looks up the existing `Arc<CharacterPackage>` and calls `BundleCharacterPackage::from_character_package`. Static typed-reference admission has already rejected every reference to an absent target.
- The bundle builder does not open the package root, list a directory, decode the character manifest again, or reconstruct layer membership.
- Optional absence emits no virtual file/package record.
- Present but unselected/unreachable optional content follows the existing `ContentPartitionPlan` outcome; this correction does not redesign partitioning.
- Bundle content/root digests are derived from accepted typed product bytes, never from host paths.

## Watch inventory

```rust
pub enum ProfileTopologyWatchExpectation {
    MustExist,
    OptionalMayAppear,
}

pub struct ProfileTopologyWatchEntry {
    pub logical_path: ProfileTopologyLogicalPath,
    pub host_path: PathBuf,
    pub kind: ProfileTopologyResourceKind,
    pub expectation: ProfileTopologyWatchExpectation,
}
```

- Manifest, selected source modules, generated metadata, Character manifests, and every named present layer are `MustExist`.
- An accepted absent optional Character root contributes only its expected Character manifest path as `OptionalMayAppear`.
- When that manifest appears, the next complete candidate discovers its exact typed layer list and replaces the watch inventory atomically.
- Watchers consume this list; they never watch a package directory to infer files.

## LSP and atomic publication

The existing candidate/CAS architecture remains. Extend it so `AcceptedProfileCandidate` and `AcceptedProfileEnvironment` retain:

- `Arc<AcceptedContentInventory>`;
- exact `ProjectTopologyRevision`;
- an accepted watch inventory identifier or immutable list.

Candidate checks require that the inventory revision equals the topology/project revision used for semantic and project-index construction. A binary-only layer change therefore:

1. changes `ProjectTopologyRevision`;
2. creates a new candidate and `ProgramHash`;
3. publishes a fresh LSP generation and empty cache namespace only after all validation succeeds.

Any failure leaves the prior accepted environment internally intact, but the operation reports the new failure and never labels or uses the prior generation as a successful result.

Character-definition/LSP diagnostics consume the retained `SourceBackedCharacterManifest`; they do not call a Character manifest decoder again. Binary bytes remain non-text resources and are not inserted into `AcceptedSourceDocuments`.
