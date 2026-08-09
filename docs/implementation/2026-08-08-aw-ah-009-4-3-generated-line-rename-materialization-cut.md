# AW-AH-009.4.3 generated line rename materialization cut

- Date: 2026-08-08
- Inspected Git base: `11a0cdc562bbddd6a718b8cdbecef96f51af534b`
- Working tree: dirty with only this generated-rename cut and this pre-commit
  evidence note
- Completion credit: AW-AH-009.4.2 coordinate-free outer-call semantic
  admission and AW-AH-009.4.3 TM-071 generated-ID materialization behavior;
  not complete AW-AH-009.4.2/.3 matrix closure

## Performed

- Final semantic analysis now treats every immediate outer ordinary `CallExpr`
  of a dialogue application as typed `DialogueConfiguration`, whether or not
  the call contains `id` or `text_key`. The Character callee remains the typed
  Character authority and the call remains semantic metadata rather than an
  executable ordinary call.
- Generated line rename is exposed only from the parser/HIR-owned bracket or
  colon component. It does not claim the Character target or arbitrary content
  bytes as a line-identity symbol.
- Rename materializes one immediate absolute `id` coordinate through the
  accepted module's typed HIR source-role map:
  - an empty outer call uses `CallArgumentListEmptyInsertion`;
  - a nonempty call inserts before `CallArgumentListClose`, accounting for the
    typed trailing-separator role; and
  - a non-call target is wrapped at the exact typed target boundary after a
    bracket/colon component has been proven.
- The replacement is still checked for line-ID validity, accepted project
  collision, and derived text-key length before any edit is returned. Typed
  reference edges receive the same replacement. Explicit text keys remain
  untouched.
- Poisoned or incomplete source roles, stale open documents, and missing clean
  insertion facts make rename unavailable. No source scan or fabricated source
  range is used.

## Passed validation

All Cargo commands used the ordinary shared target with
`CARGO_BUILD_JOBS=4` and one Cargo process at a time.

- coordinate-free dialogue-call semantic test — passed.
- generated empty-call materialization and explicit reacceptance — passed.
- generated nonempty-call append and path-target wrapping — passed.
- full `arcweft-lang-sema` library suite — 167 passed, 0 failed.
- LSP dialogue-line focused set — 3 passed, 0 failed.
- `cargo check --workspace --all-targets --all-features` — passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — 1,996 Rust
  files, 983,812 physical Rust LOC, 181 review triggers, 0 blocking violations.
- `cargo fmt --all` — passed.

## Structural review

The edit planner reads the same accepted `Arc<HirProject>` as definition,
references, and explicit rename. It queries `HirModule::source_site` with
closed source roles and does not retain another component map. The only new
semantic behavior is the contract-required removal of the erroneous
"coordinates must be nonempty" condition on the immediate outer call.

## Not run and remaining work

- The complete LSP suite was not rerun. Its known eight Character-definition
  fixtures remain blocked by the unreturned typed Presentation command ABI,
  independently of dialogue-line rename.
- `just test-workspace`, `just test-tier2`, and the complete 100-row
  AW-AH-009.4.3 matrix were not run for this cut.
- Relative/family-relative line references remain pending the design return for
  request
  `2026-08-08-aw-ah-009.4.3.1-callable-key-method-owner-line-prefix-reconciliation.md`.
- Agent/MCP/CLI query closure, localization, complete runtime dialogue-plan,
  codec/save-replay, deletion proof, and remaining matrix rows remain open.

This cut adds no compatibility alias, dual reader, source parser fallback,
source gate, removed-syntax diagnostic, CSS/Takumi route, Presentation shim,
or guessed callable owner prefix.
