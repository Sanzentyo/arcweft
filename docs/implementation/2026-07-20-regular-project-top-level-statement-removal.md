# Regular-project top-level statement removal

Date: 2026-07-20

## Outcome

Regular `.arcw` project documents are declaration-only. Executable dialogue,
choice, control-flow, and statement forms must be owned by a current
declaration such as `flow` or `fn`.

The former project-root execution path was removed directly:

- the public syntax AST no longer exposes `Item::FlowItem`;
- the private grammar no longer exposes `TopLevelFlowItem`;
- unrecognized project-root text becomes generic recovery-only `Item::Raw`
  with the ordinary `syntax.parse` diagnostic;
- HIR no longer stores or publishes `HirModule::top_level_items`;
- semantic analysis, symbol collection, verification, runtime-plan lowering,
  render-text inventory, tooling, LSP, CLI, and View-mount discovery no longer
  scan a project-root executable body;
- persistent HIR facts no longer encode a top-level executable-item count.

There is no compatibility parser, removed-spelling recognizer, dedicated
removed-syntax diagnostic, dual reader, alias, or migration shim. The
unreleased persistent object shape was corrected in place.

## Behavior evidence

The parser behavior test feeds a bare dialogue line followed by a valid
declaration. It observes one ordinary top-level parse error, retains the first
line only as generic recovery data, and resumes at the following declaration.
The LSP projects the same generic declaration-only diagnostic and does not
offer a code action that makes the bare item executable.

`RawItem` is explicitly recovery-only and is rejected by HIR lowering. Nested
`RawSyntaxFamily::FlowItem` remains valid recovery infrastructure inside a real
flow body; it is not a project-root execution path.

## Fixture migration

Parser/sema fixtures that previously placed speaker lines, content calls,
choices, source-locale blocks, or line-plan attachments directly at project
root now place them inside an explicit fixture `flow`.

The unclosed brace-form line-plan recovery invariant was not deleted. Its old
project-root fixture could only observe the enclosing flow's unclosed-block
diagnostic after the declaration-only switch, so the invariant moved to an
owner-local parser test of `take_optional_line_plan`. The direct test still
requires the exact line-plan owner diagnostic, expected closing brace, and
nonempty recovery guidance.

## Tooling reconciliation

The first combined runtime/tooling/LSP run exposed two tooling fixtures rather
than a remaining project-root execution path.

The Agent formatter fixture found a real parser defect: standalone line and
block comments in an ordinary function body were being passed to expression
recovery as executable logical items. Recovering function, trait-function,
impl-function, and callback bodies now use one linear pass over the lossless
token stream to omit trivia-only logical items from the typed statement list.
Comments remain byte-for-byte present in the lossless document. Range
projection is checked and fails closed; the filter does not reinterpret an
invalid range as executable source.

The other fixture asserted canonical RichText rewriting inside
`alice.say()[...]` in an ordinary function body. `.say` is explicitly excluded
from the accepted AW-AH-009.4 direction, while
`AW-AH-009.4.2` still owns the final non-`.say` dialogue-content application
node for callable bodies. Direct bracket and colon spellings do not currently
produce that typed callable-body node, so substituting either would invent the
unresolved surface. The obsolete fixture was removed without changing the
production visitor or restoring `.say`. Current colon-form RichText traversal
inside authored flow branches remains covered. Callable-body dialogue-content
coverage must be added with the final typed node when AW-AH-009.4.2 is
implemented.

## Interactive boundary

This change does not create a script dialect. `.awfagent` project documents
share the declaration-only document grammar.

Interactive REPL parsing remains separately owned by `FragmentKind`.
Statement fragments continue to be embedded in a synthetic ordinary function
by `arcweft-agent-repl`; they do not re-enter `TypedSyntaxTree` as executable
project-root items.

## Verification

All final commands below completed successfully against the integrated working
copy with `CARGO_INCREMENTAL=0` for Cargo validation:

```bash
cargo test -p arcweft-lang-syntax --all-targets
cargo test -p arcweft-lang-hir \
  lower::tests::lowering_rejects_recovery_only_project_root_items \
  -- --exact --nocapture
cargo test -p arcweft-lsp \
  session::parser_diagnostic_tests::project_root_recovery_diagnostic_has_no_executable_code_action \
  -- --exact --nocapture
cargo test -p arcweft-cli \
  app::bundle::tests::custom_dialogue_view_role_lowers_and_evaluates_through_the_bundle_runtime \
  -- --exact --nocapture
cargo test \
  -p arcweft-lang-hir \
  -p arcweft-lang-sema \
  -p arcweft-runtime-plan \
  -p arcweft-tooling \
  -p arcweft-lsp \
  -p arcweft-project \
  -p arcweft-project-loader \
  -p arcweft-compiler \
  -p arcweft-verify \
  --all-targets
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
just test-tier2
cargo fmt --all -- --check
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs \
  --root . \
  --write target/codex-validation/proof-structure-final-20260720
```

The syntax suite includes direct lossless-CST coverage proving that braces
inside strings do not consume the next declaration and that callback bodies
retain their value and exact ranges with CRLF input. The HIR test feeds a
recovery-only root item followed by a valid flow, then directly proves that the
root `Item::Raw` cannot lower. The LSP test requires an empty action list, so
neither an edit nor a command can make the recovered root text executable.

The final workspace fast path passed after that HIR negative test was added.
Tier 2 passed the 23-test MCP stdio group, Agent observation and native image
cases, animated image cases, text-combine Mask and ObjectId cases, typewriter
and ruby capture-time cases, visual smoke, and four checked-in IMQ vertical
goldens.

The current structural audit scanned 3,309 files, including 1,696 Rust files,
782,609 physical Rust lines, and 92 package manifests. It reported zero errors
and 128 warning-level review triggers. The detailed reports are under
`target/codex-validation/proof-structure-final-20260720`.

## Stage boundary

This completes the regular-project top-level-statement row of Proof
concurrency v6.1.1 Stage 1. Stage 1 remains partial because retained global
identity declarations and other independently owned grammar rows are still
unresolved. This deletion does not authorize Proof Stage 2 by itself.
