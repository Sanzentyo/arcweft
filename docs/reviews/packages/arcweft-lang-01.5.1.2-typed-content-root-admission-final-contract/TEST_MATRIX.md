# Row-by-row test matrix

All rows are normative. They are implementation-time tests; this no-implementation package did not run them.

| ID | Area | Scenario | Required result |
|---|---|---|---|
| `TCRA-001` | Character package | Required selected Character package on disk; manifest and every exact layer valid | One complete package and all resource records are accepted. |
| `TCRA-002` | Character package | Character manifest supplied by text overlay and every layer by binary overlay | Overlay-first package is accepted; no disk read occurs for overlaid files. |
| `TCRA-003` | Character package | Manifest overlay with remaining layers on disk | Mixed acquisition is accepted and yields one package authority. |
| `TCRA-004` | Character package | Disk manifest with one layer overlay and other layers on disk | Exact overlay replaces only the named layer; package validates atomically. |
| `TCRA-005` | Character package | Required Character manifest missing | Reject `aw.content.character.manifest_missing`; publish nothing. |
| `TCRA-006` | Character package | Optional but selected Character manifest missing | Reject `aw.content.root.optional_referenced_missing` with `Profile`. |
| `TCRA-007` | Character package | Manifest names one missing layer | Reject `aw.content.character.layer_missing` at the asset token. |
| `TCRA-008` | Character package | Direct package construction supplies duplicate layer path | Reject existing/typed duplicate-layer error independent of insertion order. |
| `TCRA-009` | Character package | Two binary overlay seeds use the same path | Reject `aw.content.overlay.duplicate` before acquisition. |
| `TCRA-010` | Character package | Direct package construction supplies an unreferenced layer | Reject `aw.content.character.layer_unreferenced`. |
| `TCRA-011` | Character package | Binary overlay path is not named by any accepted Character manifest | Reject `aw.content.overlay.unconsumed`; do not silently ignore. |
| `TCRA-012` | Character package | Layer has a non-PNG signature | Reject `aw.content.character.layer_invalid_png`. |
| `TCRA-013` | Character package | PNG header is readable but CRC/deflate/frame decode is corrupt | Full-frame validation rejects `layer_invalid_png`; header-only acceptance is impossible. |
| `TCRA-014` | Character package | Decoded PNG dimensions differ from manifest variant rectangle | Reject `aw.content.character.layer_dimensions` with exact expected/actual sizes. |
| `TCRA-015` | Character package | Character manifest ID differs from `@character.*` root | Reject `aw.content.character.identity_mismatch` with both source locations. |
| `TCRA-016` | Character package | Root is `@character.npc.alice` | Map exactly to `assets/npc/alice.awchar/character.awchar.json` and nested layers. |
| `TCRA-017` | Character package | Source-backed manifest document identity differs from its source map | Owner constructor rejects `ManifestSourceIdentityMismatch`. |
| `TCRA-018` | Character package | Manifest contains duplicate asset paths | Existing Character manifest validation rejects before payload acquisition. |
| `TCRA-019` | Character package | Extra unrelated PNG exists in package directory but was not supplied as overlay | No directory scan; unrelated disk file is outside accepted topology and ignored. |
| `TCRA-020` | Character package | Every manifest asset appears exactly once in arbitrary input order | Package map and accepted result are deterministic. |
| `TCRA-021` | Overlay | Same host path supplied once as text and once as binary | Reject `aw.content.overlay.kind_conflict`. |
| `TCRA-022` | Overlay | Binary overlay bytes are invalid while disk bytes are valid | Reject overlay candidate; no disk fallback. |
| `TCRA-023` | Overlay | Binary overlay changes one byte | Topology revision changes and full package validation reruns. |
| `TCRA-024` | Overlay | Text source overlay changes one byte | Topology revision changes and source closure is rebuilt. |
| `TCRA-025` | Overlay | Manifest text overlay changes content policy only | Topology revision and accepted content facts change. |
| `TCRA-026` | Overlay | Generated metadata text overlay changes one byte and hash remains old | Existing exact metadata hash validation rejects; no partial topology. |
| `TCRA-027` | Overlay | Binary overlay path is non-normalized or outside contained root | Seed construction/containment rejects before read. |
| `TCRA-028` | Overlay | Character asset path attempts absolute path or `..` traversal | Existing `CharacterAssetPath` validation rejects. |
| `TCRA-029` | Dependency | Dependency-owned Character package supplies typed text and binary seeds | Accepted under exact dependency package coordinate; no workspace path inference. |
| `TCRA-030` | Budget | Text or binary resource is exactly 8,388,608 bytes | Accepted when all other limits permit. |
| `TCRA-031` | Budget | Text or binary resource is one byte over 8,388,608 | Reject `aw.content.limit` with resource kind/path. |
| `TCRA-032` | Budget | Combined text+binary overlay bytes equal/then exceed 8,388,608 | Exact maximum accepted; one over rejected. |
| `TCRA-033` | Budget | Layer resource makes resource count 4,095/4,096 | 4,095 accepted; 4,096 rejected. |
| `TCRA-034` | Budget | Work counter reaches exact maximum/overflows | Maximum accepted; over-limit and arithmetic overflow remain distinct. |
| `TCRA-035` | Revision | Only project manifest bytes change | `ProjectTopologyRevision` changes. |
| `TCRA-036` | Revision | Only one selected Arcweft module changes | Revision changes. |
| `TCRA-037` | Revision | Only one accepted generated metadata document changes | Revision changes. |
| `TCRA-038` | Revision | Only Character manifest bytes change | Revision changes. |
| `TCRA-039` | Revision | Only one Character layer byte changes | Revision changes. |
| `TCRA-040` | Revision | Logical path changes while bytes remain equal | Revision changes because identity/path is semantic. |
| `TCRA-041` | Revision | Semantic module/import/Character key changes with equal bytes | Revision changes. |
| `TCRA-042` | Revision | Disk and overlay supply identical bytes for every logical resource | Revision remains equal. |
| `TCRA-043` | Revision | Absolute project root moves with identical package coordinates, logical paths, and bytes | Revision remains equal. |
| `TCRA-044` | Revision | mtime, permissions, or watcher generation changes only | Revision remains equal. |
| `TCRA-045` | Revision | Resource insertion/acquisition order is reversed | Canonical revision remains byte-for-byte equal. |
| `TCRA-046` | Revision | Two present records share a canonical key | Typed duplicate-key failure; no last-write-wins. |
| `TCRA-047` | Revision | Optional absent root is added/removed | Absence record changes revision. |
| `TCRA-048` | Revision | Optional root changes from absent to present with valid package | Revision changes and absence record is replaced by present records. |
| `TCRA-049` | Revision | Only disk/overlay origin changes with identical bytes | Revision remains equal; origin is not hashed. |
| `TCRA-050` | Revision | Candidate revision differs from project index or LSP environment revision | Reject `aw.content.topology.revision_conflict` before publication. |
| `TCRA-051` | Presence | Required selected file-backed root absent | Reject `RequiredRootMissing`. |
| `TCRA-052` | Presence | Required unselected file-backed root absent | Reject `RequiredRootMissing`; required is independent of profile selection. |
| `TCRA-053` | Presence | Optional unselected Character root absent and never runtime reachable | Accept explicit `OptionalAbsent` fact with expected manifest path. |
| `TCRA-054` | Presence | Optional selected Character root absent | Reject `OptionalRootReferencedMissing(Profile)`. |
| `TCRA-055` | Presence | Optional unselected Character root absent but reachable from selected entry | Reject `OptionalRootReferencedMissing(Runtime)` with reference source. |
| `TCRA-056` | Presence | Optional selected and runtime-reachable Character root absent | Reject `OptionalRootReferencedMissing(ProfileAndRuntime)`. |
| `TCRA-057` | Presence | Optional unselected Character root is present and valid | Accept present package; no absence fact. |
| `TCRA-058` | Presence | Optional present Character manifest is corrupt | Reject exact manifest error; optional does not mask failure. |
| `TCRA-059` | Presence | Optional present package has missing layer | Reject layer-missing error. |
| `TCRA-060` | Presence | Optional present package has corrupt PNG | Reject invalid-PNG error. |
| `TCRA-061` | Presence | Optional source-owned root is missing | Reject semantic-target-missing; no absence fact. |
| `TCRA-062` | Presence | Optional configured-resource root is missing | Reject resource-target-missing; no absence fact. |
| `TCRA-063` | Presence | Same canonical root is listed twice in one unit | Reject `aw.content.root.duplicate` with both root spans. |
| `TCRA-064` | Presence | Same canonical target appears once in two different units | Both occurrence facts are accepted; physical target/package is deduplicated. |
| `TCRA-065` | Presence | Selected profile policy has non-default residency/placement/compression | Exact typed policy and source spans reach ProjectIndex and partition input. |
| `TCRA-066` | Presence | Unselected unit has no profile policy table | Fact is `Unselected`; no invented default policy is attached. |
| `TCRA-067` | Presence | Exact typed reference occurrence targets the root in the selected accepted source/runtime closure | Root counts runtime referenced and the occurrence span is retained. |
| `TCRA-068` | Presence | Exact reference occurs in a branch later proven unreachable | It still counts runtime referenced; static admission never accepts a typed node naming absent content. |
| `TCRA-069` | Family | `@character.alice` | Classifies file-backed and never resolves as source Character declaration. |
| `TCRA-070` | Family | Valid `@flow.*` root | Resolves exact source Flow identity. |
| `TCRA-071` | Family | Valid `@view.*` root | Resolves exact retained View identity. |
| `TCRA-072` | Family | Valid `@action.*` root | Resolves exact retained Action identity. |
| `TCRA-073` | Family | Valid `@activity.*` root | Resolves exact Activity identity. |
| `TCRA-074` | Family | Valid `@source.*` root | Resolves exact Source identity. |
| `TCRA-075` | Family | Valid `@asset.*` root | Resolves exact Asset identity without scanning asset directories. |
| `TCRA-076` | Family | Valid `@signal.*` root | Resolves exact retained Signal identity. |
| `TCRA-077` | Family | Valid `@metric.*` root | Resolves exact retained Metric identity. |
| `TCRA-078` | Family | Valid `@layer.*` root | Resolves exact retained global Layer identity. |
| `TCRA-079` | Family | Root names an actual accepted `res` declaration | Accept exact `ResourceDeclarationIdentity`. |
| `TCRA-080` | Family | Root uses a resource family prefix but no declaration exists | Reject `resource_target_missing`; prefix alone is not a target. |
| `TCRA-081` | Family | Two accepted resource declarations claim one public identity | Reject accepted-world integrity/ambiguous-target error; no arbitrary winner. |
| `TCRA-082` | Family | Root resolves through alias or reexport | Canonical target identity is original; root occurrence and alias source are retained. |
| `TCRA-083` | Family | Unknown first family segment | Reject `aw.content.root.unknown_family` at exact root string content. |
| `TCRA-084` | Family | Known nested/runtime family such as choice, text, input, or style | Reject `aw.content.root.wrong_family`. |
| `TCRA-085` | Family | `@entry.*` root | Reject as launch-only wrong family. |
| `TCRA-086` | Family | `@content.*` root | Reject removed family; no source-content fallback. |
| `TCRA-087` | Family | Old Image/Voice/Rig source-family root without accepted `res` declaration | Reject wrong family; do not preserve old ownership. |
| `TCRA-088` | Family | Presentation target or scroll-region identity used as root | Reject wrong family; these remain scoped retained dependencies. |
| `TCRA-089` | ProjectIndex | Accepted unit/root facts are lowered | Every required field, source span, target/absence, reference flags, and revision match inventory. |
| `TCRA-090` | ProjectIndex | Content roots are declared only in `arcw.toml` | Project graph contains manifest-owned ContentUnit symbol and ContentRoot edges without source syntax. |
| `TCRA-091` | ProjectIndex | Root order is changed in manifest | Occurrence ordinals/source facts and topology revision update deterministically. |
| `TCRA-092` | ProjectIndex | Project index receives inventory from a different topology revision | Reject revision conflict. |
| `TCRA-093` | Deletion | Old source `content` declaration text is parsed after migration | Ordinary current-grammar rejection/recovery; no Content AST/CST node. |
| `TCRA-094` | Deletion | HIR/sema/tooling are compiled after source-content removal | No Content declaration/body/symbol variant or producer remains. |
| `TCRA-095` | Deletion | Graph inspection after removal | ContentRoot relation exists only from manifest facts, not a source producer. |
| `TCRA-096` | Cache | Only a binary layer changes | ProgramHash/cache namespace changes despite equal `SourceSetRevision`. |
| `TCRA-097` | Bundle | Reachable Character root is bundled | Existing `BundleCharacterPackage::from_character_package` consumes the exact accepted package. |
| `TCRA-098` | Bundle | Accepted optional absence is present | No Character package or virtual file is emitted. |
| `TCRA-099` | Bundle | Present optional root is unselected and unreachable | Existing partition result controls omission; bundler performs no discovery. |
| `TCRA-100` | Watch | Present Character package accepted | Watch inventory contains manifest and every exact layer path. |
| `TCRA-101` | Watch | Optional Character root accepted absent | Watch inventory contains only expected manifest path as `OptionalMayAppear`. |
| `TCRA-102` | Watch | Absent manifest appears later | New full candidate validates manifest/layers and atomically replaces watch inventory. |
| `TCRA-103` | LSP | Binary-only valid layer update | Fresh accepted generation and empty cache namespace publish. |
| `TCRA-104` | LSP | Binary update corrupts package | No topology/index/catalog/cache namespace/generation publishes; failure is reported. |
| `TCRA-105` | Atomicity | Failure during any layer acquisition/hash/PNG stage | No partial `LoadedProfileTopology` or content candidate escapes. |
| `TCRA-106` | Atomicity | Failure during semantic root resolution | No ProjectIndex, catalog, bundle input, cache namespace, or LSP generation publishes. |
| `TCRA-107` | Atomicity | Failure during runtime-reference reconciliation | Optional absence is not published; all downstream products remain uncommitted. |
| `TCRA-108` | Reuse | Registration/LSP need Character manifest facts | Consume retained `SourceBackedCharacterManifest`; decode counter stays one. |
| `TCRA-109` | Reuse | Bundle/watch need Character layer inventory | Consume accepted package/resource/watch products; no directory rescan. |
| `TCRA-110` | Compatibility | Old manifest path arrays, alternate reader, or source-content fallback input | Ordinary strict/typed rejection; no fallback path executes. |
| `TCRA-111` | Presence | Same Character target occurs in required and optional units and is absent | Shared acquisition fails `RequiredRootMissing`; no optional absence is published. |
| `TCRA-112` | Presence | Same absent Character target occurs in two optional unselected units with no references | One watch/acquisition state is shared; each occurrence receives its own typed absence fact/revision record. |
| `TCRA-113` | Presence | Same Character target occurs in multiple units and one typed runtime reference exists | Every occurrence is marked runtime-referenced; absent target fails deterministically. |
| `TCRA-114` | Family | Migrated `@image.*` or `@voice.*` root has an actual accepted `res` declaration | Accept exact configured `ResourceDeclarationIdentity`; no old source-family owner is revived. |
| `TCRA-115` | Revision | Accepted resource-type registry digest changes with source bytes unchanged | `ProjectTopologyRevision` changes through the typed semantic record. |
