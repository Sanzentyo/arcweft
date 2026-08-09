# AW-AH-009.4.3 canonical cache and accepted project lifecycle cut

- Date: 2026-08-08
- Inspected Git base: `cb637060c2dad14ce2d6d5d758c35833595c33dd`
- Working tree: dirty with only the cache/lifecycle implementation and this
  pre-commit evidence note
- Builds on:
  `2026-08-08-aw-ah-009-4-3-module-candidate-project-transaction-cut.md`
- Completion credit: AW-AH-009.4.3 TM-051, TM-079, and TM-093 lifecycle/cache
  substrate; not complete package or Frontier 6 consumer closure

## Performed

- `AcceptedDialogueLineInventory` now owns a private BLAKE3 cache fingerprint
  domain-separated by `arcweft.hir.dialogue-line-inventory.v1`.
- The transcript uses fixed tags and length-prefixed canonical fields for line
  ID, text key, origins, package/module/source identity, exact session-qualified
  application and named-scope HIR IDs, typed owner, named-scope declaration
  evidence, source order, and all source spans. It does not use `Debug`, display
  names, source reconstruction, Serde, or a wire-format alias.
- The inventory fingerprint is computed after canonical record sorting and is
  part of structural inventory equality. A module insertion permutation test
  proves equal inventory and fingerprint.
- `ProjectCompilationSession` now owns one private accepted-project cache keyed
  exactly by root package and the canonically sorted tuple of
  `HirPackageModuleKey`, exact `SourceDocumentIdentity`, and `HirSnapshotId`.
- An identical no-op compilation reuses the exact `Arc<HirProject>`.
- A new project is installed in the session cache only after every compiler
  stage succeeds. Project collision or any later failure leaves the previous
  accepted Arc untouched. The cache is not a second project model and does not
  persist or serialize HIR.

## Passed validation

All Cargo commands used the normal shared target with
`CARGO_BUILD_JOBS=4` and one Cargo process at a time.

- `module_input_permutations_produce_equal_inventory_fingerprint` — 1 passed.
- `noop_project_rebuild_reuses_the_exact_accepted_hir_project_arc` — 1 passed.
- `failed_project_build_preserves_the_previous_accepted_hir_project_arc` —
  1 passed.
- `cargo check --workspace --all-targets --all-features` — passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — 1,995 Rust
  files, 982,624 physical Rust LOC, 181 review triggers, 0 blocking violations.
- `cargo fmt --all` and `git diff --check` — passed.

## Structural review

`arcweft-lang-hir/src/final_project/dialogue_lines.rs` remains a cohesive
610-LOC owner below the 1,200-LOC production trigger. It owns the accepted
inventory, indexes, collision transaction, and their private canonical cache
transcript; splitting the transcript into a second model would weaken field
correlation.

`arcweft-compiler/src/project.rs` is now 1,642 LOC and remains above the existing
review trigger. The added state is one private key/Arc pair on the existing
compiler session, and the existing project-construction block performs the
lookup/commit. No new dependency, global state, persistent cache, or parallel
project owner was introduced.

## Not run and remaining work

- Full HIR tests were not repeated in this small cut; the preceding commit's
  full run passed 845 tests with 8 ignored, while this cut ran its three exact
  lifecycle/fingerprint tests plus all-target workspace compile and lint gates.
- `just test-tier2`, `just test-workspace`, and the complete 100-row package
  matrix were not run. Tier 2 remains applicable to the later full consumer
  replacement, not claimed by this cache-only cut.
- Typed line references, rename, localization/runtime-plan, LSP/Agent/MCP/CLI
  consumers, complete one-over/property rows, and the method-owner correction
  request remain open.

This cut adds no compatibility alias, dual reader, source gate, source parse,
removed-syntax diagnostic, runtime/save wire format, or guessed method prefix.
