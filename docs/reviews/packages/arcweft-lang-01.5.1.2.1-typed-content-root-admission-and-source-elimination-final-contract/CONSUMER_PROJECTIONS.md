# Consumer projections

## 1. Bundle

The bundle builder receives `&AcceptedProfileProject`.

1. Iterate accepted units and root facts.
2. Select `Present` Character targets.
3. Deduplicate by `CharacterId` in deterministic order.
4. Fetch the exact `LoadedCharacterPackage` from the accepted topology.
5. Call existing
   `BundleCharacterPackage::from_character_package(package, logical_root)`.
6. Insert the returned package record and exact virtual files.

Absent optional Character roots produce no manifest, layer, directory, or
placeholder entry. Resource and Activity roots select existing typed
resource/runtime-plan products; they do not enter Character package storage.

A bundle consistency check asserts that every Character manifest/layer virtual
file corresponds to one accepted topology resource and that no accepted
present Character resource is omitted.

## 2. Watch

Present exact resources produce `MustExist` entries. Accepted optional absence
produces exactly one `OptionalCharacterManifest` target at the expected
manifest host path with `OptionalMayAppear`.

The watcher SHALL NOT recursively enumerate a `.awchar` directory. When an
optional manifest appears, it schedules a full candidate rebuild; only that
rebuild may discover the manifest-named layer paths. Removal or mutation of a
present manifest/layer similarly schedules a full rebuild.

## 3. LSP

The accepted environment stores:

```rust
pub struct AcceptedProfileEnvironment {
    generation: AcceptedEnvironmentGeneration,
    project: Arc<AcceptedProfileProject>,
    // Existing registered semantic/tooling products remain and must agree.
}
```

The current loose publication of a candidate, Character map, and topology is
replaced atomically. Every request checks generation plus
`ProjectTopologyRevision`; open-document text and binary overlays are captured
before profile discovery and candidate construction.

A failed candidate publishes diagnostics associated with its request version
but cannot expose candidate semantic facts through the previous accepted
generation.

## 4. Cache

Parser/HIR caches may continue to use exact source identities. Any cache whose
value represents accepted content, package inventory, bundle input, signature
help world, definition index, or LSP generation includes
`ProjectTopologyRevision`. Failed candidates create no accepted namespace.

## 5. Compiler, Agent, and CLI

These consumers query `ProjectSemanticIndex::accepted_content()` and never
parse `arcw.toml`. Agent inspection may display content unit, family, target,
visibility, demand, profile policy, presence, source location, and topology
revision. It must clearly display optional absence as absence, not as an empty
package.

## 6. Generated metadata and Activity

Generated metadata remains decoded by its existing accepted owner. Content
admission consumes its already-typed Activity/export references and includes
its exact source bytes in `ProjectTopologyRevision`. Runtime artifact binding
is outside this contract and remains governed by Lang-01.5.1.3.
