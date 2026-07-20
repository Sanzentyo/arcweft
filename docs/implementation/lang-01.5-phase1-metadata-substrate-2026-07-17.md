# Lang-01.5 Phase 1 metadata substrate — 2026-07-17

## Scope

This cut implements the Phase 1 portions of
`arcweft-lang-01.5-build-profile-metadata-final-contract-95a36314.zip` that are
independent of the later ownership migration. The input archive SHA-256 is
`36871A25668A04F7DDE0FF2B30B5B2765B0336DB8CF92ED56406A096A3D58BDC`.

Implemented:

- one lower Sans-I/O `arcweft-manifest-model` owner for final package/profile/
  content/import/implementation IDs, normalized project paths, exact semantic
  versions, raw/semantic digests, closed manifest enums, and canonical JSON;
- exact `blake3:<64 lowercase hex>` codecs and BLAKE3 derive-key semantic hashes;
- one neutral Sans-I/O `arcweft-adapter-metadata` envelope for Rust, WASM
  component, and process adapters;
- a single strict JSON parse that retains object-key/value and array-element
  ranges, rejects duplicate keys with both occurrences, and rejects explicit
  null and floating-point values;
- strict typed metadata decoding with unknown-field rejection, duplicate
  requirement/export rejection, normalized set ordering, and canonical ABI and
  payload hash verification;
- the package's canonical Rust fixture as a repository-owned golden, plus Rust,
  WASM, and process semantic-hash checks.

No filesystem reads, adapter invocation, artifact loading, process execution,
WIT parsing, Cargo invocation, source-language migration, or runtime registry
publication occurs in either new crate.

## Phase-order conflict and explicit non-goal

The final single `ArcweftManifestDocument` decoder and deletion of the existing
project/launch readers are not included in this cut. This is an incomplete item
from the package's Phase 1 acceptance criteria, not a declaration that the
current readers are final.

At revision `39d487a0f8e7`, the active launch profile still carries
`adapter-manifests`, `character-manifests`, `rust-metadata`, and
`dialogue-defaults` into compiler/runtime/tooling call sites. The final
Lang-01.5 `ProfileSpec` deliberately removes those fields in favor of verified
generated external modules and typed dialogue selection. Deleting the old
reader before those consumers migrate would silently discard selected runtime
inputs. Adding the final decoder beside it would create the explicitly
prohibited dual-reader state.

The final profile also embeds the existing `InlineFailurePolicy`, whose owner is
currently `arcweft-render-text`. Copying that enum into the lower manifest model
would create a second owner, while moving or adding manifest serde at its owner
belongs to the later dialogue migration.

The required atomic integration boundary is captured in
`docs/reviews/requests/2026-07-17-lang-01.5.1-single-manifest-decoder-production-reconciliation.md`.

## Verification

Run with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-launch -p arcweft-manifest-model -p arcweft-adapter-metadata
cargo clippy -p arcweft-launch -p arcweft-manifest-model -p arcweft-adapter-metadata --all-targets -- -D warnings
```

The Rust golden asserts the package-published raw file hash, ABI hash, and
payload hash. Mutations distinguish ABI identity failures from payload-only
identity failures. Strict JSON tests cover duplicate first/later ranges and
array-element ranges.

`cargo check --workspace` compiled the new crates and downstream launch,
project, compiler, project-loader, LSP, and runtime crates before stopping at an
unrelated pre-existing missing checked-in asset:
`web/assets/noto-sans-jp-vf.ttf`, included by
`arcweft-player-scene/src/fonts.rs:11`. No error from this cut preceded that
missing-file failure.

## Structural audit

The canonical audit ran against the working cut on parent `bd51f4c34d3d`. It
scanned 3,129 files, 1,571 Rust files, 718,198 Rust physical
LOC, and 92 package manifests. It reported 0 errors and 128 workspace warnings;
the violation report names neither new crate. The new dependency graph is
intentionally one-way: `arcweft-adapter-metadata -> arcweft-manifest-model` and
`arcweft-launch -> arcweft-manifest-model`; model fan-in/fan-out is 2/5 and
metadata fan-in/fan-out is 0/5 at this cut.

Changed Rust measurements:

| File | Owner / class | Bytes | Physical LOC | Embedded test LOC | Responsibility |
|---|---|---:|---:|---:|---|
| `arcweft-manifest-model/src/lib.rs` | manifest-model / facade | 1,140 | 24 | 0 | deliberate public surface |
| `arcweft-manifest-model/src/identity.rs` | manifest-model / production | 8,397 | 282 | 25 | validated nominal IDs and exact SemVer |
| `arcweft-manifest-model/src/path.rs` | manifest-model / production | 2,428 | 85 | 12 | lexical project-relative paths |
| `arcweft-manifest-model/src/digest.rs` | manifest-model / production | 4,358 | 146 | 24 | raw/semantic digest type separation |
| `arcweft-manifest-model/src/canonical.rs` | manifest-model / production | 3,660 | 110 | 30 | canonical semantic JSON |
| `arcweft-manifest-model/src/schema.rs` | manifest-model / production | 8,983 | 330 | 19 | Phase-1-safe final manifest records/enums |
| `arcweft-adapter-metadata/src/lib.rs` | adapter-metadata / facade | 809 | 18 | 0 | deliberate public surface |
| `arcweft-adapter-metadata/src/model.rs` | adapter-metadata / production | 7,827 | 266 | 0 | neutral generated envelope |
| `arcweft-adapter-metadata/src/strict_json.rs` | adapter-metadata / production | 8,317 | 271 | 37 | one parse, duplicate/span-preserving JSON tree |
| `arcweft-adapter-metadata/src/codec.rs` | adapter-metadata / production | 9,248 | 267 | 0 | validation, normalization, ABI/payload hashing |
| `arcweft-adapter-metadata/tests/codec.rs` | adapter-metadata / integration test | 6,389 | 160 | 0 | golden/tamper/cross-family verification |
| `arcweft-launch/src/lib.rs` | launch / facade | 1,093 | 28 | 2 | re-export the canonical shared policy owner |
| `arcweft-launch/src/model.rs` | launch / production | 21,196 | 689 | 0 | existing launch resolution over shared IDs/enums |
| `arcweft-launch/src/tests.rs` | launch / unit test module | 9,516 | 311 | 0 | launch selection and policy regression tests |

The largest current workspace reference points were also recorded: generated
`arcweft-text-layout/src/vertical_orientation.rs` is 357,456 bytes / 12,399 LOC;
integration test `arcweft-cli/tests/check/cli_runtime_bench.rs` is 256,505 bytes /
7,970 LOC; production `arcweft-core/src/value.rs` is 84,017 bytes / 2,500 LOC.
Every new production module is below the preferred 300–800 LOC responsibility
range ceiling, both facades are below 250 LOC, and no new module crosses a
warning or decomposition threshold.

The audit's duplicate-name inventory also lists the manifest `ContentResidency`
and `ContentPlacement` beside the existing AWFB container enums. They remain
different serialized boundaries in this cut: authoring profile policy uses
kebab-case manifest text, while the bundle types own encoded section behavior.
The later content lowering must use an owned conversion and may consolidate the
neutral enum owner only when the AWFB codec dependency direction is preserved.

## Remaining work

- implement and publish the sole span-preserving schema-1 TOML document decoder;
- migrate every active project/launch consumer in one atomic cut and delete
  `TomlScanner`, `toml::from_str` manifest paths, and old source-map types;
- move no source ownership, dialogue defaults, Activity implementation, or
  tooling deletion until that integration cut is explicitly authorized;
- add loader-side path containment, metadata/artifact pins, immutable artifact
  handles, and transaction publication in the later package phases.

The returned Lang-01.5.1 package leaves one later-authority conflict in its
dialogue row: it deletes source `dialogue defaults` while continuing to select
an `@dialogue.*` entity without defining a replacement owner for that entity.
All decoder, source-map, layout, topology, module, Activity, content, and
consumer work that does not depend on that choice may continue. Final dialogue
publication and deletion must use the result of
[Lang-01.5.1.1](../reviews/requests/2026-07-20-lang-01.5.1.1-dialogue-profile-presentation-owner-contract-correction.md).
