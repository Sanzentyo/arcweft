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

## Remaining package work

The following remain part of AW-AH-009.2.1.1 and are not completion claims for
this cut:

- immutable document snapshots and exact import-closure loading;
- overlay-first topology resolution without directory enumeration in the LSP
  request path;
- workspace-keyed profile slots, input tokens, permits, and accepted-candidate
  identity rules;
- begin, commit, fail, and capture transaction APIs;
- failed-rebuild eligibility and accepted-pointer preservation;
- package diagnostics, limits, tamper cases, concurrency cases, and the complete
  acceptance matrix.

The subsequent AW-AH-009.2.1.2 diagnostics reconciliation and AW-AH-009.2.1.3
shared request-budget reconciliation remain ordered after this package.
