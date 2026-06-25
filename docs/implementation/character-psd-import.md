# PSD character import implementation

This implementation adds a typed `.awchar` package and a pure PSD conversion
boundary.

## Ownership

- `arcweft-character`: Sans I/O ids, manifest, validation, catalog, look resolution.
- `arcweft-character-psd`: PSD bytes to typed manifest plus PNG byte payloads.
- `arcweft-project-loader`: filesystem loading of manifest files/package directories.
- `arcweft-cli`: source reads and staged package writes only.
- `arcweft-presentation`: renderer-independent `CharacterRenderSpec` and layer content.
- `arcweft-launch`, `arcweft-lang-sema`, and `arcweft-lsp`: profile-selected character
  metadata, per-character look types, diagnostics, and enum-member completion.

## Accepted PSD contract

Version 1 accepts regular PSD (not PSB), 8-bit RGB documents. Top-level groups use:

```text
part:body/
  uniform
part:eyes/
  normal
  smile
look:normal/
  body=uniform
  eyes=normal
look:smile/
  body=uniform
  eyes=smile
```

`part:` direct pixel layers become cropped PNG variants. `look:` direct pixel-layer
names are metadata markers and are not exported.

For existing PSD standing-picture packs that do not use the Arcweft naming
contract, the importer has a non-strict loose group profile. When no `part:`
groups are found, groups whose names start with `!` or `*` are imported as parts,
their direct pixel layers are imported as variants, and top-level pixel layers are
kept under `part_top_level`. Original Unicode PSD names are preserved in
`source_layer`; Arcweft part/variant ids are generated as stable ASCII ids from the
group/layer ordinal and any ASCII slug. When no `look:` group exists, `default` is
inferred from visible variants. Multiple visible variants or no visible variants
produce warnings, and `--strict` rejects those warnings.

Blend mode, opacity, clipping state, source group/layer names, and source layer index
are preserved. The importer deliberately does not call the upstream flattening API.
Unsupported renderer blend/clipping behavior produces warnings; `--strict` rejects
any warning.

## Optional real PSD fixture

The repository must not store third-party PSD assets. The local checkout ignores
`/.arcweft-local/`; copy fixture material there when validating with a real file.

The optional fixture used for this implementation is the Zundamon standing-picture
pack from <https://seiga.nicovideo.jp/seiga/im11206626>. Place the extracted
directory as:

```text
.arcweft-local/
  character-psd/
    zundamon-v3.2/
      readme.txt
      *.psd
```

The test `cargo test -p arcweft-character-psd --test zundamon_fixture` first checks
`ARCWEFT_ZUNDAMON_PSD`, then `ARCWEFT_ZUNDAMON_PSD_DIR`, then the local ignored
directory above. If none exists, the test returns successfully after printing a
skip message. With the fixture present, it imports the largest PSD in the directory
and verifies that the manifest is generated without embedding the local path.

## Language/tooling surface

A launch profile lists packages with `character_manifests`. The LSP loads them via
`arcweft-project-loader`, registers `CharacterLook<character.id>` enum sets in the
standard semantic environment, and validates the expected look type in:

```arcw
show(@character.akane, .smile)
```

The existing enum completion path exposes `.normal`, `.smile`, and other declared
looks with the per-character type in completion detail.

## Structural audit

Revision measured: Jujutsu change `kkxprktnvqrzlstvlwswmsqxvlmnnvxz`,
commit `7bcff73f8b73`.

Command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Result: 1342 files scanned, 747 Rust files, 358799 Rust physical LOC, 86 package
manifests, 0 errors, 88 warnings. No changed production file crosses the
1200-LOC warning threshold. New dependency direction is presentation/tooling
facing: `arcweft-character` is Sans I/O; `arcweft-character-psd` depends on it
plus `psd`/`png`; project-loader/CLI/LSP/sema/presentation consume
`arcweft-character`; `arcweft-character-ui` consumes character and presentation/UI.

Changed Rust file measurements:

| Path | Crate | Bytes | LOC | Kind |
|---|---:|---:|---:|---|
| `crates/arcweft/src/lib.rs` | arcweft | 841 | 38 | facade |
| `crates/arcweft-character/src/catalog.rs` | arcweft-character | 4881 | 133 | production |
| `crates/arcweft-character/src/id.rs` | arcweft-character | 5206 | 137 | production |
| `crates/arcweft-character/src/lib.rs` | arcweft-character | 521 | 12 | facade |
| `crates/arcweft-character/src/manifest.rs` | arcweft-character | 26684 | 792 | production |
| `crates/arcweft-character-psd/src/lib.rs` | arcweft-character-psd | 24111 | 696 | facade |
| `crates/arcweft-character-psd/tests/zundamon_fixture.rs` | arcweft-character-psd | 2889 | 85 | integration test |
| `crates/arcweft-character-ui/src/lib.rs` | arcweft-character-ui | 16865 | 465 | facade |
| `crates/arcweft-cli/src/app/commands.rs` | arcweft-cli | 3224 | 107 | production |
| `crates/arcweft-cli/src/app/import.rs` | arcweft-cli | 6317 | 190 | production |
| `crates/arcweft-cli/src/app/project.rs` | arcweft-cli | 16447 | 438 | production |
| `crates/arcweft-cli/src/app.rs` | arcweft-cli | 4509 | 103 | production |
| `crates/arcweft-lang-sema/src/checker/presentation.rs` | arcweft-lang-sema | 18736 | 451 | production |
| `crates/arcweft-lang-sema/src/env.rs` | arcweft-lang-sema | 28150 | 774 | production |
| `crates/arcweft-lang-sema/src/types.rs` | arcweft-lang-sema | 6713 | 272 | production |
| `crates/arcweft-lang-sema/tests/character_manifest_types.rs` | arcweft-lang-sema | 3734 | 95 | integration test |
| `crates/arcweft-launch/src/lib.rs` | arcweft-launch | 12751 | 379 | facade |
| `crates/arcweft-launch/tests/character_manifests.rs` | arcweft-launch | 721 | 24 | integration test |
| `crates/arcweft-lsp/src/profiles.rs` | arcweft-lsp | 34531 | 935 | production |
| `crates/arcweft-lsp/tests/character_manifest_profile.rs` | arcweft-lsp | 2658 | 91 | integration test |
| `crates/arcweft-presentation/src/character.rs` | arcweft-presentation | 5087 | 152 | production |
| `crates/arcweft-presentation/src/layer.rs` | arcweft-presentation | 8651 | 305 | production |
| `crates/arcweft-presentation/src/lib.rs` | arcweft-presentation | 11834 | 403 | facade |
| `crates/arcweft-presentation/src/replay.rs` | arcweft-presentation | 12428 | 376 | production |
| `crates/arcweft-project-loader/src/character_manifest.rs` | arcweft-project-loader | 1504 | 42 | production |
| `crates/arcweft-project-loader/src/lib.rs` | arcweft-project-loader | 283 | 7 | facade |

Largest Rust files in the current checkout:

| Path | Crate | Bytes | LOC |
|---|---:|---:|---:|
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | arcweft-text-layout | 357456 | 12394 |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | arcweft-cli | 255424 | 7445 |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | arcweft-cli | 225209 | 5838 |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | arcweft-cli | 222475 | 5760 |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | arcweft-cli | 209852 | 5285 |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | arcweft-cli | 195828 | 5034 |
| `crates/arcweft-render-native/src/tests.rs` | arcweft-render-native | 153634 | 4172 |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | arcweft-cli | 143360 | 3947 |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | arcweft-cli | 137938 | 3791 |
| `crates/arcweft-cli/src/toolchain_profile.rs` | arcweft-cli | 75712 | 2356 |
