# Repository evidence

## 1. Baseline

- Repository: `Sanzentyo/arcweft`
- Latest main inspected for this return: `0c8cb74dd96116a8b987cc419c9a280b6cabe4a4`
- Commit subject observed: `Clarify Match temporary lifetime ownership`
- Repository date observed: 2026-08-07/08 UTC boundary
- Root `AGENTS.md` SHA-256: `90bae8bface6d390246538c60842da7d71d1ebd576ae3fa403019caa35a91498`
- `docs/AGENTS.md` SHA-256: `40ef407718c7b6f44b1f79d8dcd92c562d3ff4649557ecfeff37a33d7fd86c2d`
- `docs/reviews/AGENTS.md` SHA-256: `49d35db3276b2f2efe4d7e2343cf888455b610f5704a9607ec0acd93a3cb130a`
- `docs/reviews/README.md` SHA-256: `0727936af481b2eb78e158763048128a68044d2cc351277c1cf067aab0de27a8`
- `crates/AGENTS.md` SHA-256: `9dc887815ef05b1e7c2f63937926a908cdd7d9e36b916600fe9a278787966565`
- Attached request SHA-256: `5a318c3499ef3082aff829eafc00e9259b37bc200beb273ffa3c143dcb618065`
- Rust skill SHA-256: `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`
- Project premise SHA-256: `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1`

## 2. Exact current-source observations

| Current owner | Verified observation | Design consequence |
| --- | --- | --- |
| `arcweft-project::content` | `ProjectBinaryResource` retains `Arc<[u8]>` and existing `BuildDigest`; `ProjectTopologyRevision` canonicalizes present resources, resource-registry digest, and explicit optional Character absence | reuse the existing bytes/digest/revision; no second hash |
| project topology model | text and binary payload variants, text/binary overlay seed types, complete `LoadedCharacterPackage`, and watch expectation enum already exist | preserve substrate; add aggregate validation, revision, logical paths, absence watch target |
| `arcweft-character::package` | `CharacterPackage::from_source_backed_manifest` retains exact manifest bytes and validates duplicate/missing/unreferenced layers, complete PNG decode, and dimensions | use one complete package as accepted carrier |
| sema `ProjectSemanticIndex` | current final struct has entities/callables/nominals/types/relations but no manifest content facts | add mandatory accepted content authority |
| project graph | current `ProjectGraphRelationKind::ContentRoot` and `index_content_root_relations` derive roots from source HIR | delete atomically with manifest-owned fact publication |
| sema types | current `EntityKind::Source` and `TypeKind::Source` remain | deletion inventory under Lang-01.5.1.2.1 |
| current loader behavior | Character roots are preloaded by a string `@character.` prefix before final typed admission | replace with typed resolver target; do not preserve shortcut |
| manifest source map | internal content-unit/profile-content/index path substrate exists under one generic source map | extend owning public token enum/inherent mapping, not a side map |
| bundle Character path | existing inherent `BundleCharacterPackage::from_character_package` consumes a complete package | bundle projects accepted package directly |
| LSP rebuild/publication | current flow retains separable candidate/Character/topology products and text-focused overlays | publish one accepted carrier and add binary overlay capture |

## 3. Local evidence file identities

```json
{
  "content.rs": {
    "bytes": 18948,
    "sha256": "de06268d1b742ae221f79011849923b02ee155c4f3023c9de20143eee7694d8e"
  },
  "model.rs": {
    "bytes": 35179,
    "sha256": "6a427888cb05d96a0cd68e0b84b03debf7f1c1416e17dc15583843b4119777a0"
  },
  "project_index.rs": {
    "bytes": 37155,
    "sha256": "a6673111144efe1a9b8fef7a9b33d9c810298fb2d11a51025f62dbad1bcb60af"
  },
  "project_index_entities.rs": {
    "bytes": 37115,
    "sha256": "cd25a1359a8b2c47b3e0ff79952d39147323b4838fffbb58e4f8e01924353c51"
  },
  "project_index_relations.rs": {
    "bytes": 42544,
    "sha256": "c51956ceada3c6db2d13b125a4d39efc6c578cf7dae4fd9495187b5135afefcb"
  },
  "repo/arcweft-character/src/package.rs": {
    "bytes": 11278,
    "sha256": "fd33f3964ad673957167ff502cdff62f618a03091bd13017adba1dd937da3bbc"
  },
  "sema_types.rs": {
    "bytes": 37623,
    "sha256": "f12ed18905c6f37e61c894e749df743fa0b75cff71e97420b3173dad9e54e734"
  }
}
```

These hashes identify the exact source snapshots inspected locally. They are
not a claim that the complete repository was downloaded.

## 4. Applicable policy evidence

The inspected root, documentation, reviews, and Rust-workspace instruction files
require latest accepted main, one final typed authority, deletion-driven migration, inherent behavior on Arcweft-owned
boundary types, no source-string reconstruction, typed validation, deterministic
behavior, preserved layer direction, and explicit reporting of not-run
validation. This package applies those rules directly.

The Rust skill was read in full. The contract uses private fields, checked
constructors, newtypes/enums for domain values, restrained public APIs, no
unsafe/unstable/macros, and mandates fmt/clippy.
