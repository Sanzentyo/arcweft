# Rust workspace instructions

These instructions apply to `crates/` and are the common Rust/Cargo policy for
the whole workspace when referenced by the root `AGENTS.md`.

## Required context

- Read every applicable Rust skill completely before editing Rust, Cargo,
  build scripts, tests, benches, Rust tools, or Rust-facing documentation.
- Before changing a dependency, feature, public boundary, facade export, or
  cross-crate type owner, read `docs/00-overview/crate-map.md`.
- Before changing language behavior, read the applicable maintained chapter
  under `docs/01-language/` and reconcile syntax, HIR, sema, runtime-plan,
  verifier, compiler, and tooling consumers that own that behavior.
- Follow `docs/implementation/test-execution-policy.md` for validation scope
  and `docs/implementation/structural-audit-policy.md` for structure gates.

## Architecture and ownership

- Preserve `syntax -> HIR -> sema -> runtime-plan/verify -> tooling` dependency
  direction. Use the crate map as the detailed authority rather than copying
  each crate's responsibilities here.
- Keep runtime/data core and data-format crates Sans I/O. Filesystem, network,
  clocks, processes, platform storage, GPU, audio, and device access belong in
  host or adapter crates.
- Keep syntax parser-only. It may own lossless CST, attached/surface syntax,
  expression/type/pattern parsing, source ranges, recovery, and syntax lints;
  it must not own HIR lowering, semantic checks, runtime plans, or verifier
  policy.
- Put backend-specific dependencies behind adapter crates and feature flags.
- Use a facade crate for broad application-facing preludes. In non-facade
  crates, prefer responsibility modules and narrow visibility over broad root
  re-exports.
- Prefer `module.rs` with a same-named child directory. Do not introduce new
  `mod.rs` files.
- Keep public API deliberate, documented, and no wider than its consumers
  require.
- Prefer the simplest complete domain model, not the smallest edit. When
  several special cases are projections of one rule, express the rule on the
  owning schema or typed context and delete the special-case paths even when
  that requires a broad producer/consumer migration.
- Do not leave workspace-external directories that look like active crates,
  tests, or fixtures. Remove obsolete migration scratch; retain historical
  material under documentation only when explicitly useful.

## Deletion-driven migration

- Remove obsolete internal variants, types, helpers, dispatch branches, and
  readers as soon as a coherent final replacement is ready. Let compile errors
  enumerate the consumers that must migrate.
- Do not leave migration counters, zero-use compatibility types, deprecated
  aliases, V2 wrappers, optional fallbacks, or negative source-spelling tests
  after the old production path is physically gone.
- Do not preserve removed language syntax. A temporary spelling-specific
  diagnostic may exist only during deletion; remove it and its exact-code test
  before completing the cut unless released compatibility explicitly requires
  it.
- Do not preserve an obsolete production model merely to satisfy stale tests.
  Update the tests and deterministic fixtures to the selected final authority.

## APIs, conversions, and errors

- Prefer typed APIs over strings and sentinels.
- For context-free conversions, prefer `From` or `TryFrom`. Put domain behavior
  on the owning type, and put allocation, interning, diagnostics, policy, or
  shared-state conversion on a named lowering/inventory/verifier/adapter
  context.
- Do not create a free-standing `{source}_to_{target}` helper, an extension
  trait, or a wrapper merely to avoid completing the owning type.
- Keep a one-use `map_err`, `ok_or_else`, `match`, or small error conversion
  inline. Extract a helper only when it names a real reusable domain rule or
  centralizes stable structured diagnostics.
- Use `thiserror` for workspace error types unless a concrete boundary requires
  a manual implementation. Preserve structured kinds, ranges, anchors, and
  related evidence.
- Do not hard-code one builtin, enum variant, or nominal name when shared
  grammar or a typed registry can express the rule.
- Do not use a closed enum as a bag of examples. Its variants must form the
  exhaustive domain algebra owned at that layer; otherwise move the behavior
  into the schema, registry, or typed policy that actually distinguishes the
  cases.

## Cargo and modules

- Centralize workspace dependency locations in root `[workspace.dependencies]`.
  Member manifests inherit workspace crates with `workspace = true`; document
  any concrete exception for an excluded standalone fixture.
- Keep feature combinations stable during a validation slice. Use an extra
  feature combination only when it directly exercises the changed path and
  record why.
- Keep backend dependencies optional and out of lower-level default features.
- Add focused tests for each new crate or stable subsystem boundary.
- Use deterministic snapshot/golden tests only when the artifact itself is the
  contract.

## Validation evidence

- Do not add source gates: automated checks must not pass or fail by searching
  checked-in implementation or documentation for symbol spellings, snippets,
  module paths, or file placement.
- Replace an existing source gate with typed behavior, codec round trips,
  compile-fail evidence, parser/compiler rejection, lints, generated-artifact
  comparison, or a structured Cargo dependency graph. Delete a gate without
  replacement when it protects no observable invariant.
- One-off source inspection during review is allowed; it is not acceptance
  evidence and must not become a test or structural-audit rule.
- This source-gate prohibition supersedes older requests and implementation
  notes that prescribe source spelling as acceptance evidence.
- Use focused changed-crate checks and exact tests during the tight loop. At a
  reviewable Rust cut, run the applicable workspace check, Clippy, workspace
  test, Tier 2, and structural gates selected by the test policy.
- Run `cargo fmt` for changed Rust. Use
  `cargo clippy --workspace --all-targets --all-features` when the selected cut
  calls for the workspace lint gate.
- Record every command actually run, its result, and deliberately skipped slow
  tiers. Planned validation is not completed evidence.

## Structure

- Treat ownership and decomposition as part of correctness. Compilation alone
  is insufficient for dependency, public-contract, manual-projection, or
  cross-layer changes.
- Run the canonical audit at the triggers defined in
  `docs/implementation/structural-audit-policy.md`:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

- Do not split cohesive algorithms solely to meet a numeric target. Above an
  upper LOC review trigger, name the owner and responsibility and record either
  a decomposition action based on real boundaries or an explicit cohesion
  justification; LOC alone is not a structural failure.

## Parser changes

- Treat maintained grammar documentation as the language authority. Prefer
  explicit CST/AST nodes, structured recovery, and source spans over strings.
- Cover complete syntax families with success, malformed, recovery-span, and
  ambiguity tests.
- Document public parser/AST APIs concisely. Comments should explain grammar,
  ambiguity, and recovery decisions rather than restating control flow.
