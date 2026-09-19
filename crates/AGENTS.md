# Rust workspace instructions

This is the Rust/Cargo policy for the workspace, including Rust tools, build
scripts, tests, benches, and Rust API documentation outside `crates/`.
Repository-wide design, Git, and completion rules live in the root instructions.

## Task-specific references

- Dependency, feature, public boundary, facade export, or cross-crate owner
  changes: [crate map](../docs/00-overview/crate-map.md).
- Language behavior: the relevant maintained chapter in `docs/01-language/`
  and affected syntax, HIR, sema, runtime-plan, verifier, compiler, and tooling
  consumers. A Rust-only wording fix does not require a language survey.
- Validation selection: [test policy](../docs/implementation/test-execution-policy.md).
- Structural changes and reviewable Rust push cuts:
  [structural policy](../docs/implementation/structural-audit-policy.md).

## Ownership and APIs

- Preserve `syntax -> HIR -> sema -> runtime-plan/verify -> tooling` direction;
  the crate map owns the details. Syntax owns parsing, lossless CST, surface
  syntax, ranges, recovery, and syntax lints, not lowering or semantic policy.
- Runtime/data core and data formats remain Sans I/O. Filesystem, network,
  clocks, processes, storage, GPU, audio, and devices belong to hosts/adapters.
  Backend dependencies stay optional in adapters, out of lower-layer defaults.
- Use a facade for broad application preludes. Elsewhere use responsibility
  modules, narrow visibility, and deliberate documented public APIs. Prefer
  `module.rs` plus its child directory; do not introduce `mod.rs`.
- Use `From`/`TryFrom` for context-free conversions. Allocation, interning,
  diagnostics, policy, and shared-state conversion belong to named owning
  contexts. Do not add free-standing conversion helpers, extension traits, or
  wrappers merely to avoid completing an Arcweft-owned type.
- Keep one-use error conversions inline unless extraction names a reusable
  domain rule or stable structured diagnostic. Use `thiserror` unless the
  boundary requires manual implementation; preserve kinds, ranges, anchors,
  and related evidence.
- Typed schemas/registries express general rules. A closed enum represents its
  exhaustive domain algebra, not a collection of builtin or nominal examples.
  Simplicity only distinguishes equally complete models; it does not justify
  collapsing distinct domain roles.

## Migration and Cargo

Delete replaced internal variants, helpers, readers, aliases, counters, and
fallbacks, and migrate all affected consumers. Remove transitional removed-syntax
diagnostics and exact-code tests before completing an unreleased migration.
Update stale tests/fixtures to the final contract, not production to obsolete
expectations. Do not leave scratch directories looking like active crates,
tests, or fixtures; retain history only where useful under documentation.

Centralize dependencies in root `[workspace.dependencies]`; members inherit with
`workspace = true`. Document concrete standalone-fixture exceptions. Keep
features stable within a validation slice; exercise extra combinations when the
changed path warrants them. Do not set Cargo `--jobs`, `-j`, or `CARGO_BUILD_JOBS`
for ordinary builds/checks/lints or a single test command. An explicit count is
allowed only to coordinate intentionally parallel independent test commands;
record that intent.

## Evidence

Use the test policy once to select the cut's evidence; do not copy its command
matrix here. Run focused tests in the edit loop and the applicable mainline,
workspace, lint, and structural gates at the coherent cut. Ordinary local checks
and disposable-fixture tests may be run, fixed, and rerun without per-step
approval. Do not infer that device, external-service, or user-data tests are
disposable; apply the selected target's actual boundary.

No automated source-spelling/file-placement gates, including ones requested by
older packages. Replace them with typed behavior, codec round trips, compile-fail
or parser/compiler rejection evidence, lints, deterministic artifact comparison,
or Cargo dependency graphs; delete checks with no observable invariant.
One-off source inspection is a review aid, not behavior evidence or a new gate.

Add focused tests for new crates and stable boundaries. Parser-family changes
cover success, malformed input, recovery spans, and ambiguity; use explicit
CST/AST nodes and document grammar/recovery decisions. Snapshots/goldens are
appropriate when the artifact itself is the contract. Structural LOC triggers
require ownership review, not arbitrary splitting of cohesive algorithms.
