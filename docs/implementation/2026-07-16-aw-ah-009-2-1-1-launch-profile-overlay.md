# AW-AH-009.2.1.1 launch profile overlay reconciliation

## Status

Implementation is in progress. This note records reviewable cuts without
treating the package as complete before its full acceptance matrix is covered.

## Completed cut: deterministic profile selection

`arcweft-launch` now owns the typed `LaunchProfileSelection` policy and selects
profiles with the package-defined precedence:

1. explicit selection is exact and never falls back;
2. automatic selection uses a valid manifest default;
3. otherwise it retains an existing previous profile;
4. otherwise it chooses the lexicographically first declared profile;
5. an invalid declared default and an empty profile map are distinct errors.

Direct tests cover each branch. Focused tests, all-target/all-feature checking,
clippy with warnings denied, and formatting have passed for this cut.

## Completed cut: source-backed resource decoding

`arcweft-project-loader` now returns source-backed adapter and Rust metadata
products and exposes decode entry points that consume an already captured
`Arc<SourceDocument>`. Character manifests use the same exact-document route.
The disk loaders read once, construct the owning document, and delegate to the
same decoders. Adapter format dispatch follows the declared path extension,
and `.awchar` resolution is lexical rather than dependent on `Path::is_dir`.

`AdapterRegistry::try_with_manifest` rejects duplicate stable adapter IDs. The
existing CLI and LSP disk callers consume the new source-backed results without
introducing a second reader; the later topology cut will replace the LSP disk
route itself.

Validation for this cut:

- adapter-context and project-loader library tests: 103 passed;
- LSP all-target/all-feature check: passed;
- CLI production library check with no default features: passed;
- adapter-context, project-loader, and LSP all-target/all-feature clippy with
  warnings denied: passed;
- CLI production library clippy with no default features and warnings denied:
  passed.

The broader all-feature check reaches an existing missing
`web/assets/noto-sans-jp-vf.ttf` compile-time fixture. CLI no-default all-target
checking also reaches existing tests that unconditionally import the optional
runtime-driver crate. Neither failure is caused by this loader change; the
production libraries and changed call paths pass their isolated checks.

## Completed cut: bounded exact topology transaction

`arcweft-project-loader::topology` now owns validated workspace/dependency
resource IDs, immutable retained resource records, overlay and dependency
seeds, fixed production limits, checked counters, and the all-or-nothing
`LoadedProfileTopology` product. Loading is overlay-first and admits only the
selected manifest, the exact typed `use` closure, declared character, adapter,
Rust resources, and exact dependency seeds. It does not call `read_dir`,
`collect_arcw_files`, or `project::load`.

The selected source must map to the crate root. Module candidates are derived
from typed import prefixes and probed at one exact `<segments>.arcw` path.
Malformed resources, unresolved imports, duplicate logical IDs, duplicate
paths, duplicate adapter IDs, and missing selected adapters are fatal. Rust
metadata is applied only after every declared metadata document decodes. The
frozen source revision and consumed-overlay IDs cover the complete retained
resource registry.

Direct topology tests cover overlay-only and overlay-over-disk manifests,
unsaved import closure members, unrelated-file exclusion, `.awchar` and direct
character paths, adapter/Rust authority, fatal partial-input cases, exact
dependency ownership, duplicate rules, bounded parser diagnostics, byte limits,
and retained-byte behavior after disk deletion. The production resource test
admits exactly 4,095 resources and rejects the 4,096th.

## Completed cut: topology-only registration input

`ProfileRegistrationLoadRequest` and `load_profile_registration` construct
registration facts solely from one `LoadedProfileTopology`. Every file-backed
topology resource becomes an accepted file record with its retained ownership
and access class. Character manifests are decoded again from retained documents
without I/O, while registration receives exactly the final selected adapter
after Rust metadata application. A direct test deletes the manifest, root
module, and character file after topology loading and still completes semantic
registration from the retained values.

Validation for these cuts:

- `cargo test -p arcweft-project-loader --lib`: 119 passed;
- topology-focused direct tests: 27 passed;
- `cargo check -p arcweft-project-loader --all-targets --all-features`: passed;
- `cargo clippy -p arcweft-project-loader --all-targets --all-features --
  -D warnings`: passed;
- `cargo check -p arcweft-lsp --lib`: passed;
- `cargo fmt --all`: passed;
- structural audit: 0 errors and 128 threshold warnings; reports are under
  `structure-audits/aw-ah-009-2-1-1-exact-topology-2026-07-16/`.

Changed topology production modules are below the repository review threshold:
the largest is `topology/loader.rs` at 37,124 bytes and 954 physical lines.
The split keeps identity, model/error ownership, checked budgets, orchestration,
and tests in separate responsibility modules.

## Remaining package work

The following remain part of AW-AH-009.2.1.1 and are not completion claims for
this cut:

- immutable LSP document snapshots and manifest ancestor discovery;
- joining the exact topology loader to the LSP rebuild snapshot;
- workspace-keyed profile slots, input tokens, permits, and accepted-candidate
  identity rules;
- begin, commit, fail, and capture transaction APIs;
- failed-rebuild eligibility and accepted-pointer preservation;
- package diagnostics, limits, tamper cases, concurrency cases, and the complete
  acceptance matrix.

The subsequent AW-AH-009.2.1.2 diagnostics reconciliation and AW-AH-009.2.1.3
shared request-budget reconciliation remain ordered after this package.
