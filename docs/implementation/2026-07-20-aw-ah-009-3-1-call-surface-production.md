# AW-AH-009.3.1 call-surface production reconciliation

## Status

The implementation-ready, non-superseded AW-AH-009.3.1 Cut 2–4 boundary is
implemented on the shared `main` working copy. Focused syntax, HIR, sema,
runtime-plan, verifier, Agent REPL, LSP, and CLI tests pass. This note does not
claim that the parent AW-AH-009.3 sequence or the concurrently changing
workspace is green: the normal workspace, Clippy, CLI, and Tier 2 gates expose
separately owned integration drift recorded below.

The package source is
`arcweft-aw-ah-009.3.1-call-surface-syntax-production-reconciliation-final-contract.zip`.
Cut 1 is recorded separately in
[`2026-07-20-aw-ah-009-3-1-cut1-call-surface-types.md`](2026-07-20-aw-ah-009-3-1-cut1-call-surface-types.md).

## Implemented syntax boundary

- `Expr::Call(CallExpr)` is the only authored source-call representation.
- Parenthesized and callback-block calls carry distinct immutable typed
  surfaces with checked, document-absolute UTF-8 byte ranges.
- Pratt parsing owns argument forms, commas, trailing commas, call delimiters,
  malformed-argument recovery, and missing-close recovery.
- Callback parsing owns braces, parameter headers and type ascriptions, body
  ranges, and its one semantic closure argument.
- Source-less authored-call constructors and the call-specific post-parse
  reconstruction path are removed.
- Full function/trait/impl bodies, View expression owners, and range-preserving
  line-plan expressions retain recovered typed calls and parser diagnostics.
- Hard recovering-parser failures do not publish a typed call through a lossy
  fallback.
- Raw strings are tokenized by the canonical expression lexer, including
  arbitrary checked hash delimiters and exact unterminated-token ranges. The
  former lossy pre-parser special case is removed.
- Runtime-generated applications use `RuntimeExpr`; they do not reconstruct a
  source `Expr::Call` with fabricated syntax.
- Typed-evidence partial-function-value admission is owned by
  `RuntimePureHelperLookup`, while `CallExpr` function-value admissibility is a
  separate runtime-plan policy. Both consumers use the immutable call
  accessors.

Recovery responsibilities were split into focused modules:

- `parser/statements/expr_context.rs` owns statement-expression parse mode and
  diagnostic/statistics accumulation;
- `parser/control_flow/recovery.rs` owns callback and authored scope-body
  recovery, including the lexer-provided semicolon boundary bridge; and
- `expr/source_ranges/scan.rs` owns generic source delimiter/operator scanning.

The split keeps `parser/statements.rs`, `parser/control_flow.rs`, and
`expr/source_ranges.rs` below the 1,200-physical-LOC production warning
threshold in the current checkout.

## Explicitly unresolved or superseded boundaries

### CharacterDialogue supersedes the package's dialogue special forms

The package's D-01 through D-04 clauses require exact argument-list carriers on
the old colon speaker head and `alice.say(...)[...]` content-call surface.
Those clauses are not implemented and are not counted as accepted. The later
AW-AH-009.4 CharacterDialogue direction removes `say` and replaces those
speaker/content-call forms. Adding `SpeakerLineSurface::argument_list()` or a
new `ContentCallSurface` now would preserve the superseded language surface and
conflict with that sequence.

The low-level `Colon` recovery-token carrier remains tested as a generic parser
boundary. No production speaker-head parser is deepened around it. Exact
CharacterDialogue call ownership must be completed by the CharacterDialogue
production sequence.

### Normalized line-plan fragments have no exact source map

Line-plan paths that retain an authored body base and a checked source
subslice use the recovering expression parser. Flat-block normalization and
the legacy multiline timed-cue/colon-block joins synthesize strings and do not
currently preserve a one-to-one source map. Those paths retain their previous
lossy expression semantics rather than fabricating ranges or regressing every
expression to `Expr::Raw`; they do not provide exact call recovery.

Closing this gap requires a typed source projection for normalized line-plan
fragments. It is not treated as implemented by this cut.

## Verification status

Direct tests cover:

- complete parenthesized/callback syntax and recovery invariants;
- full-source function owners at `]`, `}`, and `;` boundaries;
- return-typed closure bodies with document-absolute nested-call ranges;
- View arguments retaining a recovered call at their authored owner boundary;
- static-generic transactional lookahead, comparison rollback, malformed
  suffix rejection, limits, diagnostics, and checked overflow; and
- canonical raw strings with embedded quote/hash pairs, insufficient closing
  hashes, and exact unterminated-token ranges.

All Cargo commands below used `CARGO_INCREMENTAL=0`.

### Passing focused gates

```bash
cargo test -q -p arcweft-lang-sema --all-targets
# 784 + 4 + 5 + 4 + 3 + 13 + 9 + 4 + 4 passed

cargo test -q -p arcweft-lang-hir --all-targets
# 55 + 2 + 1 + 4 + 1 + 2 + 3 + 1 passed

cargo test -q -p arcweft-verify -p arcweft-agent-repl \
  -p arcweft-lsp --all-targets
# 228 passed across the nonempty target groups

cargo test -q -p arcweft-runtime-plan --all-targets
# 112 + 58 + 3 + 56 passed

cargo clippy -p arcweft-runtime-plan --all-targets --all-features \
  --no-deps -- -D warnings

cargo test -q -p arcweft-cli --lib \
  app::bundle::tests::nested_view_calls_retain_definition_spans_typed_parameters_and_reachability \
  -- --exact
cargo test -q -p arcweft-cli --lib \
  app::bundle::tests::view_generic_callback_block_lowers_to_handler_binding \
  -- --exact
cargo test -q -p arcweft-cli --lib \
  app::agent::native::repl_command_bridge::parse_diagnostic_tests::agent_repl_parse_failure_json_preserves_typed_diagnostics \
  -- --exact
# 3/3 passed

cargo fmt --all -- --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The canonical structural audit scanned 3,303 files, including 1,694 Rust
files, 782,234 physical Rust LOC, and 92 package manifests. It reported zero
errors and 127 warnings. Formatting and whitespace checks passed.

The syntax and runtime-plan all-target suites were also run earlier in this
cut after the direct replacement and passed. The final runtime-plan rerun above
contains 229 passing tests after the typed-evidence/admissibility ownership
split.

### External integration blockers

These failures are retained rather than changing unrelated production
contracts or stale fixtures inside this slice:

- `just test-cli-check`: 51 selected tests, 1 passed and 50 failed. Most
  failures report `product AWBC verification failed: AWBC is missing a
  required public entrypoint`; the remaining representative failures are
  stale scalar-name and numeric-width expectations. The three focused
  call-lowering/diagnostic tests above pass, so this gate does not identify a
  call-surface regression.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  the two runtime-plan findings introduced at this frontier were fixed and the
  focused runtime-plan Clippy gate passes. The workspace invocation remains
  blocked by concurrently owned `dialogue_view.rs` and
  `registration/source_index.rs` findings: needless raw-string hashes,
  `unused_self`, two production/test `too_many_lines` findings, and two
  `cloned_ref_to_slice_refs` findings.
- `just test-workspace`: stopped in `arcweft-bundle` with 113 passed and two
  failed bundle codec round-trip tests. The diff is in concurrently changing
  View/dialogue/resource ordering and payload content, not source call parsing
  or lowering.
- `just test-tier2`: the slow MCP phase ran 23 selected tests; 21 passed and
  two failed (`agent_mcp_stdio_dispatches_semantic_action` expected JSON
  `false` but received `null`, and
  `agent_mcp_stdio_lists_selected_capture_metadata` expected an observe
  `resource_links` array). The recipe stopped at `test-slow-mcp`; later Tier 2
  stages did not run.

The AW-AH-009.2.1 source-index and AW-AH-009.4.1 dialogue-View changes in the
same working copy are separate owners. They are not part of the
AW-AH-009.3.1 commit scope and must not be swept into this slice merely to make
the broad gates green.

## Shared-working-copy path inventory

Before final isolation, the shared working copy contained 135 changed paths.
They were classified exhaustively as 112 `PURE_CALL`, two `MIXED`, three
`EXCLUDE_SOURCE_INDEX`, and 18 `EXCLUDE_DIALOGUE` paths. Before committing this
cut, the dialogue changes were restored out of the working copy and the two
mixed files were reduced to their call-surface hunks. The resulting
call-surface commit contains 114 paths; the three source-index paths remain a
separate uncommitted slice.

### PURE_CALL

```text
crates/arcweft-agent-repl/src/binding.rs
crates/arcweft-cli/src/app/agent/native/repl_snapshot.rs
crates/arcweft-cli/src/app/bundle/view_mounts.rs
crates/arcweft-cli/src/app/bundle_view/lowering/text_controls.rs
crates/arcweft-cli/src/app/bundle_view_schema.rs
crates/arcweft-cli/src/app/cache.rs
crates/arcweft-cli/src/app/runtime/expectations.rs
crates/arcweft-cli/src/app/runtime/profile.rs
crates/arcweft-cli/src/app/runtime/script_bench/run.rs
crates/arcweft-cli/src/output.rs
crates/arcweft-cli/tests/check/cli_runtime_bench.rs
crates/arcweft-cli/tests/native_style_parity_sample.rs
crates/arcweft-compiler/src/persistent.rs
crates/arcweft-lang-hir/tests/style.rs
crates/arcweft-lang-sema/src/checker.rs
crates/arcweft-lang-sema/src/checker/expr.rs
crates/arcweft-lang-sema/src/checker/expr/agent.rs
crates/arcweft-lang-sema/src/checker/expr/partial.rs
crates/arcweft-lang-sema/src/checker/expr/support.rs
crates/arcweft-lang-sema/src/checker/fx.rs
crates/arcweft-lang-sema/src/checker/helpers.rs
crates/arcweft-lang-sema/src/checker/lifetime_access.rs
crates/arcweft-lang-sema/src/checker/line_plan.rs
crates/arcweft-lang-sema/src/checker/signature.rs
crates/arcweft-lang-sema/src/checker/stmt.rs
crates/arcweft-lang-sema/src/effect_contract.rs
crates/arcweft-lang-sema/src/fact_layer.rs
crates/arcweft-lang-sema/src/project_index/entities.rs
crates/arcweft-lang-sema/src/project_index/flow_control.rs
crates/arcweft-lang-sema/src/project_index/relations.rs
crates/arcweft-lang-sema/src/registration/tests.rs
crates/arcweft-lang-sema/src/semantic.rs
crates/arcweft-lang-sema/src/semantic/facts.rs
crates/arcweft-lang-sema/src/semantic/traversal.rs
crates/arcweft-lang-sema/src/style/token_graph.rs
crates/arcweft-lang-sema/src/style/value.rs
crates/arcweft-lang-sema/src/symbols.rs
crates/arcweft-lang-sema/src/tests/await_.rs
crates/arcweft-lang-sema/src/tests/choice.rs
crates/arcweft-lang-sema/src/tests/control_flow.rs
crates/arcweft-lang-sema/src/tests/declarations.rs
crates/arcweft-lang-sema/src/tests/dialogue.rs
crates/arcweft-lang-sema/src/tests/expressions.rs
crates/arcweft-lang-sema/src/tests/line_plan.rs
crates/arcweft-lang-sema/src/tests/parser_basics.rs
crates/arcweft-lang-sema/src/tests/patterns.rs
crates/arcweft-lang-sema/src/tests/support.rs
crates/arcweft-lang-syntax/src/assertion.rs
crates/arcweft-lang-syntax/src/ast/view.rs
crates/arcweft-lang-syntax/src/cst.rs
crates/arcweft-lang-syntax/src/cst/punctuation.rs
crates/arcweft-lang-syntax/src/expr.rs
crates/arcweft-lang-syntax/src/expr/call_syntax.rs
crates/arcweft-lang-syntax/src/expr/call_syntax_tests.rs
crates/arcweft-lang-syntax/src/expr/closure_parse.rs
crates/arcweft-lang-syntax/src/expr/control_parse.rs
crates/arcweft-lang-syntax/src/expr/lexer.rs
crates/arcweft-lang-syntax/src/expr/pipe_scope.rs
crates/arcweft-lang-syntax/src/expr/pratt.rs
crates/arcweft-lang-syntax/src/expr/pratt/call.rs
crates/arcweft-lang-syntax/src/expr/prefix.rs
crates/arcweft-lang-syntax/src/expr/source_ranges.rs
crates/arcweft-lang-syntax/src/expr/source_ranges/scan.rs
crates/arcweft-lang-syntax/src/expr/source_ranges/thread_body.rs
crates/arcweft-lang-syntax/src/expr/tests.rs
crates/arcweft-lang-syntax/src/parser.rs
crates/arcweft-lang-syntax/src/parser/await_.rs
crates/arcweft-lang-syntax/src/parser/control_flow.rs
crates/arcweft-lang-syntax/src/parser/control_flow/recovery.rs
crates/arcweft-lang-syntax/src/parser/dialogue.rs
crates/arcweft-lang-syntax/src/parser/flow.rs
crates/arcweft-lang-syntax/src/parser/helpers.rs
crates/arcweft-lang-syntax/src/parser/items.rs
crates/arcweft-lang-syntax/src/parser/line_plan.rs
crates/arcweft-lang-syntax/src/parser/statements.rs
crates/arcweft-lang-syntax/src/parser/statements/expr_context.rs
crates/arcweft-lang-syntax/src/parser/view.rs
crates/arcweft-lang-syntax/src/text.rs
crates/arcweft-lang-syntax/tests/assignment_statements.rs
crates/arcweft-lang-syntax/tests/parser_callbacks_and_closures.rs
crates/arcweft-lang-syntax/tests/parser_dialogue_syntax_and_defaults.rs
crates/arcweft-lang-syntax/tests/parser_flow_statements_and_body.rs
crates/arcweft-lang-syntax/tests/style_view.rs
crates/arcweft-lang-syntax/tests/ui/session_identity_raw_constructor.stderr
crates/arcweft-lsp/src/features/actions.rs
crates/arcweft-project-loader/src/cache/inspect.rs
crates/arcweft-project-loader/src/cache/persistent_query/tests.rs
crates/arcweft-project/src/persistent_object/codec.rs
crates/arcweft-project/src/persistent_object/payload.rs
crates/arcweft-runtime-plan/src/audio.rs
crates/arcweft-runtime-plan/src/expr.rs
crates/arcweft-runtime-plan/src/expr/desugar.rs
crates/arcweft-runtime-plan/src/expr/effect.rs
crates/arcweft-runtime-plan/src/expr/tests.rs
crates/arcweft-runtime-plan/src/flow/presentation.rs
crates/arcweft-runtime-plan/src/flow/syntax_helpers.rs
crates/arcweft-runtime-plan/src/function_values.rs
crates/arcweft-runtime-plan/src/fx.rs
crates/arcweft-runtime-plan/src/fx/sampler.rs
crates/arcweft-runtime-plan/src/fx/value_lowering.rs
crates/arcweft-runtime-plan/src/host_request.rs
crates/arcweft-runtime-plan/src/labels.rs
crates/arcweft-runtime-plan/src/line_task.rs
crates/arcweft-runtime-plan/src/render_text/fx.rs
crates/arcweft-runtime-plan/src/render_text/inline_failure.rs
crates/arcweft-runtime-plan/src/render_text/raw.rs
crates/arcweft-runtime-plan/src/render_text/speaker_preset.rs
crates/arcweft-runtime-plan/src/render_text/style_expr.rs
crates/arcweft-runtime-plan/tests/runtime_plan.rs
crates/arcweft-verify/src/contract_smt.rs
crates/arcweft-verify/src/lib.rs
docs/implementation/2026-07-20-aw-ah-009-3-1-call-surface-production.md
```

### MIXED

- `crates/arcweft-cli/src/app/bundle_view/lowering.rs`
  - call hunk: `lower_fx_application` migrates `Expr::Call` to immutable
    `CallExpr` accessors;
  - dialogue hunk: imports remove the superseded semantic dialogue projection
    alias.
- `crates/arcweft-lang-sema/src/checker/module.rs`
  - call hunk: `unknown_default_inline_failure_policy` migrates to immutable
    `CallExpr` accessors;
  - dialogue hunks: dialogue projection imports and
    `TypeChecker::check_dialogue_view_text_sources` move to nested character
    projection paths.

These two files require hunk-level staging if the slices are committed
separately.

### EXCLUDE_SOURCE_INDEX

```text
crates/arcweft-lang-sema/src/registration/source_index.rs
crates/arcweft-lang-sema/src/registration/source_index/tests.rs
docs/implementation/2026-07-20-aw-ah-009-2-1-source-index-completion.md
```

### EXCLUDE_DIALOGUE

```text
crates/arcweft-bundle/src/resource_codec/view/dialogue_contract.rs
crates/arcweft-bundle/src/resource_codec/view/model.rs
crates/arcweft-bundle/src/standard_view.rs
crates/arcweft-bundle/tests/standard_dialogue_view.rs
crates/arcweft-bundle/tests/view_resource_codecs.rs
crates/arcweft-cli/src/app/bundle/tests.rs
crates/arcweft-cli/src/app/bundle_view/lowering/content.rs
crates/arcweft-cli/tests/check/agent_observe_native/core.rs
crates/arcweft-lang-sema/src/dialogue_view.rs
crates/arcweft-lang-sema/src/env/base.rs
crates/arcweft-lsp/src/features/completion.rs
crates/arcweft-lsp/src/features/dialogue_view_metadata.rs
crates/arcweft-lsp/src/features/hover.rs
crates/arcweft-lsp/src/session/tests.rs
crates/arcweft-runtime-driver/src/view_runtime/evaluator/text.rs
crates/arcweft-runtime-driver/tests/view_runtime.rs
docs/implementation/2026-07-20-aw-ah-009-4-1-dialogue-view-character-projection-partial-cut.md
tests/fixtures/native_capture/unified_text_effects_migration_baseline.arcw
```
