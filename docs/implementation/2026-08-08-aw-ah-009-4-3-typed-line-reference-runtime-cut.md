# AW-AH-009.4.3 typed line-reference and runtime projection cut

- Date: 2026-08-08
- Inspected Git base: `d0df29287cd90f141a3c250d2f4b6f7f8a094558`
- Working tree: dirty with only this line-reference/runtime cut and this
  pre-commit evidence note
- Builds on:
  `2026-08-08-aw-ah-009-4-3-cache-lifecycle-cut.md`
- Completion credit: typed absolute line-reference semantic/index projection,
  exact source evidence, and checked path-only runtime lowering; not complete
  AW-AH-009.4.3 Frontier 6 or rename/LSP closure

## Performed

- `HirProjectView` and `HirExecutableProjectView` now borrow the one accepted
  dialogue-line inventory owned by their exact project generation.
- Final semantic analysis recognizes a `HirIdRef` as a line reference only when
  the contextual expected type is `Ref<DialogueLine>`. It validates an absolute
  `DialogueLineId` against the accepted inventory and publishes a closed
  `CheckedExpressionResolution::DialogueLineReference` fact.
- Missing targets fail typed semantic analysis. Untyped `@say.*` values are not
  recovered by spelling, source scans, or the ordinary project symbol table.
- Generation validation rechecks each typed target against the same accepted
  inventory.
- `ProjectSemanticIndex` now owns the returned
  `AcceptedDialogueLineReference` record shape: typed target, exact HIR source
  span, package-qualified module key, and exact `ExprId`. Records are projected
  only from checked facts and exact HIR source roles.
- Compiler/runtime lowering converts the durable `say.*` identity exactly once
  into `RuntimeLineId`. `RuntimeResolvedValue::DialogueLine` retains that typed,
  path-only value until final runtime expression lowering; generic constant or
  string paths do not perform the conversion.
- The runtime semantic-fact validator accepts the dedicated line value only for
  an entity-reference HIR expression and rejects it for ordinary paths.

## Passed validation

All Cargo commands used the normal shared target with
`CARGO_BUILD_JOBS=4` and one Cargo process at a time.

- `dialogue_line_reference_uses_accepted_project_inventory` — passed.
- `dialogue_line_reference_rejects_target_outside_accepted_inventory` — passed.
- full `arcweft-lang-sema` library suite — 165 passed, 0 failed.
- `dialogue_line_fact_owns_the_checked_path_only_runtime_identity` — passed.
- `dialogue_line_reference_reaches_runtime_lowering_from_one_accepted_generation`
  — passed.
- `cargo check --workspace --all-targets --all-features` — passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — 1,995 Rust
  files, 982,986 physical Rust LOC, 181 review triggers, 0 blocking violations.
- `cargo fmt --all` — passed.

One initial broad runtime-plan test command completed its 1 minute 51 second
four-job build but the outer 120-second command timeout closed the pipe while
the harness was listing unrelated integration tests. The two intended library
tests were rerun with `--lib` and passed; this was an interrupted command, not a
test assertion failure.

## Structural review

The semantic line target is retained once in the closed checked-expression
resolution and projected once into the existing project index. No second line
inventory or LSP-only reference table was introduced. Runtime lowering owns a
dedicated `RuntimeLineId` variant instead of broadening generic string/entity
constants.

The production files touched here remain in their existing owners. The audit's
181 size/review triggers are pre-existing review triggers and contain no
blocking dependency, compatibility, source-scan, or parallel-authority finding
for this cut.

## Not run and remaining work

- `just test-tier2`, `just test-workspace`, and the complete AW-AH-009.4.3
  100-row matrix were not run. Frontier 6 still lacks LSP definition/reference
  and transactional rename closure, so Tier 2 completion is not claimed.
- Relative/family-relative line references are not guessed here. Their exact
  owner/scope projection, especially structural methods, remains coupled to the
  returned-design request
  `2026-08-08-aw-ah-009.4.3.1-callable-key-method-owner-line-prefix-reconciliation.md`.
- Localization, generated-ID materializing rename, View/Agent/MCP/CLI query
  closure, full runtime dialogue-plan publication, codec/save-replay identity,
  and remaining limit/property rows remain open.

This cut adds no compatibility alias, dual reader, source gate, source parse,
removed-syntax diagnostic, CSS/Takumi path, generic string fallback, or guessed
method prefix.
