# FINAL CONTRACT — Lang-01.5.1.2 typed content-root admission

## Pinned basis

- Repository: `Sanzentyo/arcweft`
- Branch: `main`
- Pinned revision: `23ed5d93824630d8ead9092d32f7fc70f0a8f314`
- Sole normative request: `REQUEST_SPEC.md`
- Implementation in this package: **none**

## Normative outcome

1. The existing Sans-I/O `arcweft_character::package::CharacterPackage` becomes the sole complete `.awchar` byte authority. The topology retains one validated package plus source provenance; it does not create a second package DTO or a detached blob catalog.
2. Text overlays remain text-only. Binary layer bytes enter through a separate typed `ProfileTopologyBinaryOverlaySeed`; they never enter `SourceDocument`, `Arc<str>`, or the strict Taplo decoder.
3. `arcweft-project` owns `ProjectTopologyRevision`, a nominal newtype over the existing `BuildDigest`. One canonical v1 transcript covers the accepted manifest, selected source closure, generated metadata, character manifests, character layer bytes, the accepted resource-registry semantic digest, exact logical identities, and explicit optional-absence records.
4. A missing file-backed root is accepted only when its unit is `optional`, the selected profile does not select the unit, and no runtime-reachable reference targets that root. The accepted result contains an explicit typed absence fact. Profile-selected or runtime-reachable optional absence is the distinct `OptionalRootReferencedMissing` failure. Present-but-invalid optional content always fails closed.
5. The root taxonomy is closed. `character` is the only file-backed family in this correction. Source-owned root families are exactly `flow`, `view`, `action`, `activity`, `source`, `asset`, `signal`, `metric`, and `layer`. Any accepted `res` declaration is a configured-resource root only after exact `ResourceDeclarationIdentity` resolution through the already accepted registry. All other built-in, scoped, nested, runtime-only, removed, or unknown families are rejected.
6. `SourceBackedManifest` remains the sole schema-1 decode product. It gains dedicated inherent content-unit/root/profile source accessors; no generic decoder tree is exposed and no consumer reparses TOML.
7. The loader publishes an immutable `ContentAdmissionCandidate`; sema resolves source/configured-resource targets and builds one project-wide typed `ContentRootReferenceInventory`; sema then produces `AcceptedContentInventory`. A runtime reference is any exact typed reference occurrence in the selected accepted source/runtime closure, even if later reachability would prove it dead. This fail-closed rule prevents an accepted typed node from naming absent content. The compiler reuses its existing reachability graph only for partition/bundle inclusion. Topology, project index, catalog, bundle inputs, cache namespace, watch inventory, and LSP generation publish only after the same candidate succeeds.
8. `ProjectIndex` receives manifest-owned content-unit and root facts, including visibility, demand, selected profile policy, exact root occurrence source, canonical target or absence, reference flags, and `ProjectTopologyRevision`. Its content-root graph edges are generated from those facts.
9. Source `content` declaration ownership is deleted atomically after manifest facts are live: parser/AST, HIR, sema symbol production, tooling, fixtures, and source-declaration tests. Final parsing retains no historical node or dedicated removed-spelling diagnostic.
10. Bundle, watch, and LSP consume the accepted typed inventory. They do not enumerate asset directories, reopen package files, reparse the project manifest, reconstruct a character manifest, or accept a last-known-good candidate after a failed rebuild.

## Concrete defect boundary

The accepted strict schema-1 decoder, `SourceBackedManifest`, generated-metadata admission, project containment, Character nominal identity, existing `CharacterPackage` coverage checks, and existing bundle package adapter are retained. Two concrete gaps require owner-local extension rather than redesign:

- `CharacterPackage` currently validates layer membership but not whether exact layer bytes are a decodable PNG with dimensions matching the manifest rectangle. PNG validation is added inside that owning type.
- `LoadedProfileTopology` currently calls its text-only `SourceSetRevision` the topology source revision. It remains valid as a source-document revision, but it cannot be the accepted topology/cache authority once binary resources and absence facts exist. `ProjectTopologyRevision` becomes that sole authority; `SourceSetRevision` remains only for source-document indexes.

## Coordination boundaries

- Lang-01.5.1.1 supplies final typed dialogue presentation products; this contract carries them unchanged and does not reopen their owner.
- Lang-01.4.1/01.4.2 supply exact configured-resource identities and strict extension-manifest admission. This contract consumes the accepted registry; it does not add a resource manifest reader or infer a resource from a raw family string.
- Lang-01.5.1.3 owns generated-artifact runtime binding. This contract revisions accepted metadata bytes only; it does not load or execute providers.

## Compatibility and exclusions

There is one final schema-1 manifest reader and one accepted admission path. No compatibility alias, dual reader, legacy path array, source `content` fallback, last-known-good publication, generic directory scan, source gate, CSS route, or Takumi route is permitted.

OPEN_QUESTIONS=0
IMPLEMENTATION_PERFORMED=NO
COMPATIBILITY_SHIM=FORBIDDEN
DUAL_READER=FORBIDDEN
SOURCE_GATE=FORBIDDEN
LAST_KNOWN_GOOD_FALLBACK=FORBIDDEN
CSS_TAKUMI_PATH=FORBIDDEN
