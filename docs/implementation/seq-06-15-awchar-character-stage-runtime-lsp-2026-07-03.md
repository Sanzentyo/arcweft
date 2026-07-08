# seq06.15 `.awchar` implementation note

## Implementation slice

This overlay implements the production contract around existing Arcweft character substrate.

### Added Rust modules

- `crates/arcweft-character/src/package.rs`
  - Sans I/O `.awchar` package representation.
  - Validates manifest bytes, package-relative PNG payloads, missing layer payloads, duplicate payloads, and unreferenced payloads.
- `crates/arcweft-bundle/src/character_package.rs`
  - Typed bundle resource for `.awchar` packages.
  - Converts `CharacterPackage` to bundle virtual files and layer resource metadata.
  - Rejects missing manifest/layer virtual files.
- `crates/arcweft-player-scene/src/characters.rs`
  - Host-neutral character package decode and prepared-frame creation.
  - Produces `CharacterRenderSpec` plus `CharacterViewView` from the same path for native and web.
- LSP feature edits
  - richer completion items for character ids, looks, parts, and variants;
  - hover metadata for character ids and manifest-derived look/part/variant tokens.

### Modified Rust modules

- `crates/arcweft-character/src/lib.rs`: exports `package`.
- `crates/arcweft-character/src/manifest.rs`: adds `CharacterManifest::part` on the owned manifest type for LSP metadata lookup.
- `crates/arcweft-presentation/src/character.rs`: adds source-layer metadata, stable canvas-anchor bbox, and typed render diagnostics.
- `crates/arcweft-bundle/src/lib.rs`: schema version `5`, `character_packages` field, duplicate/missing package validation, and builder/accessor methods.
- `crates/arcweft-bundle/Cargo.toml`: adds `arcweft-character` dependency.
- `crates/arcweft-player-scene/Cargo.toml`: adds `arcweft-character` and `arcweft-character-view` dependencies.
- `crates/arcweft-player-scene/src/lib.rs`: exports `characters`.
- `crates/arcweft-lsp/src/features.rs`: exports character metadata helper.
- `crates/arcweft-lsp/src/features/completion.rs`: character-aware completion enrichment.
- `crates/arcweft-lsp/src/features/hover.rs`: character-aware hover before generic profile hover.
- `crates/arcweft-lang-sema/src/env.rs`: registers manifest-backed character entity symbols and compact speaker names.
- `crates/arcweft-lang-sema/src/resolve.rs`: can build name-resolution registries from HIR plus typed external semantic symbols.
- `crates/arcweft-compiler/src/hir.rs`, `crates/arcweft-compiler/src/project.rs`, and CLI profile validation paths: resolve profile-backed character ids before type checking and project compilation.

## Tests included in overlay

- `crates/arcweft-character/tests/awchar_package.rs`
  - package accepts all referenced layer payloads;
  - missing and unreferenced layer payloads are rejected.
- `crates/arcweft-presentation/tests/character_render_spec.rs`
  - render spec preserves source canvas anchor bbox across normal/smile looks;
  - source PSD layer metadata survives resolution;
  - unsupported blend/clipping produce typed diagnostics.
- `crates/arcweft-bundle/tests/character_package.rs`
  - bundle character resource emits manifest + package-relative layer files;
  - missing layer virtual file is rejected.
- `crates/arcweft-player-scene/tests/character_stage.rs`
  - typed package prepares normal/smile looks without flat pose PNGs;
  - stable bbox is identical across look switching;
  - retained View layers match manifest order;
  - Agent observe metadata reports selected look, stable bbox, and per-layer capture refs.
- `crates/arcweft-lsp/tests/character_completions.rs`
  - `character_manifests` profile loading feeds completions;
  - completions include character ids, looks, parts, and variants;
  - hover includes PSD source layer names;
  - missing manifest diagnostic remains typed.

## Zundamon sample migration

The sample under `samples/zundamon-awchar` declares:

```toml
default = "dev"

[profiles.dev]
kind = "game"
source = "src/main.arcw"
character_manifests = ["assets/zundamon.awchar"]
```

and switches appearance through:

```arcw
show(@character.zundamon, look = .normal)
show(@character.zundamon, look = .smile)
```

The package keeps `character.awchar.json` plus `layers/*.png`.  `validation/visual-evidence/*.png` are rendered evidence snapshots only; they are not runtime assets.

## Validation status

Applied and validated in a repository checkout on 2026-07-03.

Commands run:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-character --test awchar_package --all-features -- --nocapture
cargo test -p arcweft-presentation --test character_render_spec --all-features -- --nocapture
cargo test -p arcweft-bundle --test character_package --all-features -- --nocapture
cargo test -p arcweft-player-scene --test character_stage --all-features -- --nocapture
cargo test -p arcweft-lsp --test character_completions --all-features -- --nocapture
cargo test -p arcweft-lang-sema resolves_hir_entity_references_from_external_semantic_env -- --nocapture
cargo check -p arcweft-character -p arcweft-presentation -p arcweft-bundle -p arcweft-player-scene -p arcweft-lsp --all-targets --all-features
cargo run -p arcweft-cli -- check --profile dev
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\seq06_15_structure_audit
git diff --check
```

All cargo, fmt, clippy, sample profile check, and diff checks passed.  Structural audit scanned 2266 files / 1095 Rust files / 513545 Rust physical LOC and reported 4 existing error-level size hotspots plus 125 warnings; the report was written to `target\seq06_15_structure_audit`.

Known CLI behavior: `arcw check --profile dev` uses launch-profile metadata and validates the sample.  Plain `arcw check` still runs the project-wide path without profile metadata, so it does not load `character_manifests`; deciding whether project-wide check should implicitly apply a default launch profile is left outside this package.

## Explicit deferred list

- Exact GPU equations for every Photoshop blend mode.
- Retained-View clipping-mask composition.
- Full Photoshop group/pass-through compositing.
- Runtime PSD parsing.
- DOM-based browser character rendering.
- Legacy flat `normal.png`/`smile.png` as production character path.

Unsupported PSD/render features are represented as manifest metadata plus typed diagnostics.
