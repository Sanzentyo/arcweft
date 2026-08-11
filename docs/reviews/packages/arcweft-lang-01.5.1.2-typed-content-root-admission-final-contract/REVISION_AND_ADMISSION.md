# Topology revision and admission semantics

## Canonical topology-revision transcript

`ProjectTopologyRevision` is `BuildDigest::of(transcript)`. The transcript is manual typed binary data, never Rust debug output or generic Serde output.

### Header

```text
bytes  "arcweft.project-topology.v1\0"
u32le  1
string package_id
string package_version
string selected_profile_id
u32le  present_record_count
present_record[present_record_count]
u32le  semantic_record_count
semantic_record[semantic_record_count]
u32le  absence_record_count
absence_record[absence_record_count]
```

`string` is `u32le byte_length` followed by exact UTF-8 bytes. Counts and string lengths that do not fit `u32` are typed arithmetic-overflow failures. Resource byte lengths are `u64le`.

### Present record

```text
u8     kind_tag
string owner_package_id
string owner_package_version
string semantic_key
string normalized_logical_path
u64le  byte_length
[32]   BuildDigest::of(exact_bytes)
```

Tags are frozen:

| Tag | Kind | Semantic key |
|---:|---|---|
| `0x01` | project manifest | `manifest` |
| `0x02` | selected Arcweft module | canonical module path |
| `0x03` | accepted external-module metadata | `ExternalModuleImportId` |
| `0x04` | character manifest | canonical `CharacterId` |
| `0x05` | character layer | `CharacterId`, NUL, `CharacterAssetPath` |


### Semantic record

Semantic records bind accepted typed products whose authority is already a canonical semantic digest rather than one source file:

```text
u8     kind_tag
string semantic_key
[32]   semantic_digest_bytes
```

The v1 semantic tag is frozen:

| Tag | Kind | Semantic key |
|---:|---|---|
| `0x20` | accepted resource-type registry | `resource-type-registry` |

The bytes are the exact `ResourceTypeRegistryDigest::semantic_digest()` bytes, embedded under the topology transcript's typed tag. They are not rehashed from debug/JSON text before embedding.

### Absence record

Only an absent optional Character package has an absence record in v1:

```text
u8     0x80
string content_unit_id
u32le  root_ordinal
string canonical_authored_root
string character_id
string expected_package_root
string expected_manifest_path
```

The absence tag is semantic data. `BuildDigest::ZERO` alone is not an absence representation.

### Canonical order and duplicate policy

Present records sort by the byte tuple:

```text
(owner_package_id, owner_package_version, kind_tag, semantic_key, normalized_logical_path)
```

Semantic records sort by `(kind_tag, semantic_key)`. Absence records sort by:

```text
(content_unit_id, root_ordinal, canonical_authored_root)
```

Two records with the same canonical key are rejected, even when their bytes match. Input insertion order never chooses a winner.

### Included and excluded inputs

Included:

- exact schema-1 manifest bytes;
- exact selected source-module closure bytes;
- exact accepted generated metadata bytes;
- exact character manifest bytes;
- every exact manifest-named PNG payload;
- owner package coordinates, semantic identity, and normalized logical path;
- the accepted resource-type registry semantic digest;
- explicit absent optional file-backed roots.

Excluded:

- absolute host paths and URIs;
- disk versus overlay origin;
- timestamps, permissions, inode/file IDs, watcher generation, and directory order;
- unrelated files not named by the accepted topology;
- diagnostics, docs, and cache contents.

The compiler build identity remains a separate project fingerprint input; it is not duplicated in this topology revision.

An overlay affects the revision through the bytes it supplies. Replacing disk bytes with identical overlay bytes does not change the revision; changing one overlay byte does.

## Character package acquisition

For `@character.a.b`:

```text
package root:    assets/a/b.awchar
manifest path:   assets/a/b.awchar/character.awchar.json
layer path:      assets/a/b.awchar/<CharacterAssetPath>
```

The algorithm is exact:

1. Read or overlay the one manifest path.
2. Decode once to `SourceBackedCharacterManifest` and require manifest Character ID equality.
3. Enumerate asset paths from the typed manifest, not the filesystem.
4. For each unique manifest asset path, consume a binary overlay or read the exact disk path.
5. Construct `CharacterPackage::from_source_backed_manifest`.
6. Fully validate membership, PNG stream, and dimensions.
7. Publish the package and its manifest/layer resource records together, or publish none.

An unrelated file on disk is outside the topology and ignored. An explicitly supplied unreferenced binary overlay is rejected because it was offered as candidate input but was not consumed.

## Presence-state machine

### Candidate stage

| Demand | Profile selects unit | File-backed root | Candidate result |
|---|---:|---|---|
| required | either | present and valid | present candidate |
| required | either | absent | `RequiredRootMissing` |
| optional | yes | present and valid | present candidate |
| optional | yes | absent | `OptionalRootReferencedMissing(Profile)` |
| optional | no | present and valid | present candidate |
| optional | no | absent | explicit optional-absence candidate |
| either | either | present but invalid | exact validation failure |

Source-owned/configured-resource roots are `SemanticPending` and never become absence candidates.

### Semantic stage

1. Classify every pending root using the closed built-in table and accepted resource registry.
2. Resolve source-owned roots through the sole accepted symbol table and configured resources through the sole accepted declaration index.
3. Build one `ContentRootReferenceInventory` from typed HIR/judgment/runtime-plan facts in the selected accepted source closure. Aliases and reexports resolve to the original canonical target while exact occurrence spans are retained.
4. Supply optional-absent Character IDs as typed reservations so matching references produce the content-admission diagnostic rather than a generic unknown-owner error. Reservations never enter the symbol table, Character catalog, or runtime plan.
5. Count every exact typed occurrence as runtime-referenced, including a dead/unreachable branch. This deliberately keeps the accepted typechecked project free of references to absent content.

Source-owned roots do not trigger new file discovery: they must already resolve in the exact accepted source/import closure. A manifest root does not cause a project-directory scan.

### Cross-unit grouping

File-backed occurrences are grouped by canonical `CharacterId` before acquisition:

- any `required` occurrence makes the shared target required;
- otherwise any profile-selected occurrence makes the shared target profile-referenced;
- when all occurrences are optional and unselected, one acquisition/watch state is shared;
- if absent and unreferenced, each manifest occurrence receives its own absence fact and revision record;
- a typed runtime reference marks every occurrence of the same canonical target as runtime-referenced;
- one validated `Arc<CharacterPackage>` is shared by all present occurrences.

### Final stage

- A present target is accepted with `referenced_by` set from profile selection and the typed reference inventory.
- An optional-absence candidate is accepted only when it has no typed runtime reference occurrence.
- A referenced optional absence fails with `OptionalRootReferencedMissing(Runtime)` or `ProfileAndRuntime`.
- Failure leaves the previously accepted environment internally untouched but does not report or use it as the new result.

## Budgets

No new unbounded path is introduced. The current production limits remain the default:

- at most `4_095` topology resource records;
- at most `8_388_608` bytes per text or binary resource;
- at most `8_388_608` total supplied overlay bytes across text and binary seeds;
- at most `128` diagnostics;
- at most `1_048_576` charged work units.

Layer resources count toward the resource limit. Work is charged for every root classification, exact path derivation, acquisition, digest, PNG decode, semantic lookup, reference edge, and absence reconciliation. Arithmetic overflow is distinct from an ordinary limit failure.
