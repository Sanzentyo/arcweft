# AW-AH-009.4.2 coordinate sema and AW-AH-009.4.3 explicit rename cut

- Date: 2026-08-08
- Inspected Git base: `ab578403bcf13e1a21bd0823a642f959214c625f`
- Working tree: dirty with only this coordinate-sema/LSP cut and this
  pre-commit evidence note
- Builds on:
  `2026-08-08-aw-ah-009-4-3-typed-line-reference-runtime-cut.md`
- Completion credit: immediate dialogue-coordinate semantic classification,
  typed LSP definition/references, and explicit-ID rename; not complete
  AW-AH-009.4.2 or AW-AH-009.4.3 matrix closure

## Performed

- Final semantic analysis now recognizes a dialogue target configured by an
  ordinary value call. The call is retained as typed dialogue configuration,
  while its exact Character path remains the Character authority.
- Immediate `id` and `text_key` coordinate expressions receive closed semantic
  facts borrowed from the accepted dialogue-line inventory. Coordinate values
  are not reparsed from source and are not lowered as runtime values or calls.
- Generation validation rechecks the Character and both coordinate families
  against the same accepted HIR project inventory.
- LSP definition and references consume the accepted project dialogue inventory
  plus `ProjectSemanticIndex` reference edges directly. No LSP-owned line or
  reference table was added.
- An explicit authored line ID can be renamed transactionally across its exact
  coordinate and typed references. The replacement must be a full valid
  `DialogueLineId`, must not collide with the accepted inventory, and must
  remain capable of generating a text key when the existing text key is
  derived. Explicit text keys are left unchanged.
- Rename rejects stale open documents. Generated line IDs remain unavailable
  for rename until the contract supplies a typed insertion/materialization
  fact; the application span is not misrepresented as an authored coordinate.

## Validation

All Cargo commands use the normal shared target with `CARGO_BUILD_JOBS=4` and
one Cargo process at a time. Before validation, `cargo clean` removed 85,750
files and 164.6 GiB from the only registered worktree.

- `cargo check -p arcweft-lang-sema -p arcweft-compiler -p arcweft-lsp
  --all-targets --all-features` — passed after the clean rebuild.
- full `arcweft-lang-sema` library suite — 166 passed, 0 failed.
- full `arcweft-lsp` library suite — 202 passed, 8 failed. All eight failures
  are the existing `session::character_definition_tests` fixtures whose
  projects contain the fail-closed Presentation `Show` callable; runtime-plan
  lowering correctly reports that its typed Presentation command ABI remains
  pending. The dialogue-line focused test passed in the same run.
- `cargo check --workspace --all-targets --all-features` — passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed after factoring target classification and coordinate publication into
  their owning helpers; no lint allowance was added.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — 1,996 Rust
  files, 983,481 physical Rust LOC, 181 review triggers, 0 blocking violations.
- full sema suite after the Clippy-driven structural refactor — 166 passed,
  0 failed.
- focused LSP dialogue navigation/rename test after the refactor — passed.
- `cargo fmt --all` — passed.

## Structural review

Dialogue configuration and coordinates are new closed variants of the existing
checked-expression authority. Compiler lowering explicitly treats those facts
as semantic metadata. LSP reconstructs neither identity nor references from
source spelling and does not introduce a compatibility surface.

## Not run and remaining work

- `just test-tier2`, `just test-workspace`, and the complete AW-AH-009.4.2/.3
  matrices are not run by this cut.
- Generated-ID materializing rename remains open because a typed insertion fact
  is not yet available.
- Relative and family-relative line-reference ownership remains excluded pending
  `2026-08-08-aw-ah-009.4.3.1-callable-key-method-owner-line-prefix-reconciliation.md`.
- Localization, View/Agent/MCP/CLI query closure, runtime dialogue-plan
  publication, codecs/save-replay, and remaining limit/property rows remain
  open.

This cut adds no source scan, source parser fallback, compatibility alias, dual
reader, source gate, removed-syntax diagnostic, CSS/Takumi path, or guessed
method owner/prefix.
