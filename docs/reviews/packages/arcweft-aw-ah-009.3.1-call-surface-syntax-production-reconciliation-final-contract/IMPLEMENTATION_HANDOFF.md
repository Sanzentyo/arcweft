# Implementation handoff

## 1. Implementation rule

Implement the final model directly. Do not land a revision that exposes both the old struct-like call variant and the final private-payload variant. Do not add a compatibility constructor to keep tests compiling.

Cut 1 is independently mergeable. Cuts 2 through 4 form one unmerged direct-replacement series. Each subcut has a compiling package frontier, and the series is committed or merged only after Cut 4 restores workspace compilation. This satisfies small compiling increments without creating a public compatibility interval.

## 2. Cut 1 — introduce syntax-owned range types and validation

Owner: `crates/arcweft-lang-syntax`.

### 2.1 Add final types

Add the following to `src/expr.rs` or a focused `src/expr/call_syntax.rs` module re-exported from `expr`:

- `CallExpr`;
- `CallSurfaceSyntax`;
- `ParenthesizedCallSyntax`;
- `CallbackBlockCallSyntax`;
- `ArgumentListSyntax`;
- `ArgumentListTerminatorSyntax`;
- `CallRecoveryBoundarySyntax`;
- `CallRecoveryTokenKind`;
- `CallArgumentSyntax`;
- `CallArgumentFormSyntax`;
- `CallArgumentRecoverySyntax`;
- `CallbackBlockSyntax`;
- `CallbackParameterHeaderSyntax`;
- `CallbackParameterSyntax`;
- `CallbackParameterTypeSyntax`;
- crate-private init records and `CallSyntaxInvariantError`.

Keep all invariant-bearing fields private. Add the read-only accessors and crate-private checked constructors frozen in `FINAL_CONTRACT.md`.

### 2.2 Construction-time validation

Validation receives the exact source slice plus absolute base and verifies known token spans by direct indexing. It must not scan for punctuation. Add unit tests for every invariant error, punctuation length, UTF-8 boundary, offset overflow, count mismatch, form mismatch, and callback shape mismatch.

### 2.3 Cut 1 gate

```bash
cargo fmt --all -- --check
cargo check -p arcweft-lang-syntax --all-targets
cargo clippy -p arcweft-lang-syntax --all-targets -- -D warnings
cargo test -p arcweft-lang-syntax call_syntax
```

The workspace still uses the old call variant in this cut; the new types are final and unused outside their direct tests. No compatibility API is introduced.

## 3. Cut 2 — parser, callback, dialogue/speaker, and recovery replacement

Cut 2 begins the unmerged direct-replacement series.

### 3.1 Replace the syntax variant

In `crates/arcweft-lang-syntax/src/expr.rs` replace:

```rust
Call { callee: Box<Expr>, args: Vec<CallArg> }
```

with:

```rust
Call(CallExpr)
```

Do not retain the old spelling behind a feature, alias, second enum, or deprecated API.

### 3.2 Pratt parser

Primary files:

- `crates/arcweft-lang-syntax/src/expr/pratt.rs`;
- `crates/arcweft-lang-syntax/src/expr.rs`;
- lexer/token modules used by the Pratt parser.

Required changes:

1. Change internal Pratt values from bare `Expr` to a named parsed value containing `Expr` plus exact `TextRange`.
2. Change call-argument parsing to return semantic `Vec<CallArg>` plus `ArgumentListSyntax`.
3. Record the opening token, each argument full/value/form range, each between-argument comma, optional trailing comma, and closed or recovered terminator as tokens are consumed.
4. Build `ParenthesizedCallSyntax` and then `CallExpr::try_parenthesized`.
5. Preserve callee range across selection and postfix operations.
6. Extend the token grammar for current static generic/turbofish callees before deleting `parse_static_generic_call`.
7. Use checked absolute-range projection throughout.

### 3.3 Callback parser

Primary file: `crates/arcweft-lang-syntax/src/expr/closure_parse.rs`.

Required changes:

1. Return semantic closure plus `CallbackBlockSyntax` from the same token walk.
2. Record exact braces, explicit parameter patterns, optional type ascriptions, parameter commas, `=>`, and body range.
3. Preserve current implicit-zero, nonempty body, typed parameter, nested delimiter, and required-close grammar.
4. Construct the outer call with `CallExpr::try_callback_block`.
5. Keep unclosed braces invalid; do not add callback missing-close recovery in this task.

### 3.4 Recovering expression API

Primary files:

- `crates/arcweft-lang-syntax/src/parser/fragment.rs`;
- `crates/arcweft-lang-syntax/src/parser/helpers.rs`;
- full-parser call sites that currently use `parse_expr_lossy`.

Required changes:

1. Add the crate-private `ParsedExpr`, `ExprParseScope`, and `parse_expr_recovering_at` contract.
2. Make strict fragment parsing reject nonempty recovery diagnostics while full-source parsing retains the typed expression and appends diagnostics.
3. On missing `)`, stop before the exact owner boundary and store `RecoveredMissing`.
4. On one malformed nonempty argument segment, retain the current semantic form with an `Expr::Raw` value and mark its syntax recovered.
5. Remove raw normalization that changes byte offsets. Dot-continuation handling moves into token trivia/newline handling while preserving original offsets.
6. Project dialogue-mapped token spans through the existing source map before constructing call syntax.

### 3.5 Speaker and content surfaces

Primary files:

- `crates/arcweft-lang-syntax/src/ast/dialogue.rs`;
- `crates/arcweft-lang-syntax/src/parser/dialogue.rs`;
- `crates/arcweft-lang-syntax/src/parser/helpers.rs`;
- `crates/arcweft-lang-syntax/src/parser.rs`.

Required changes:

1. Change `SpeakerLineSurface.arguments: Option<TextRange>` to `argument_list: Option<ArgumentListSyntax>`.
2. Remove `Copy` from `SpeakerLineSurface`; return it by reference.
3. Add `ContentCallSurface` and store it in `ContentCall`.
4. Replace tuple `ContentCallParse` with a named parser result that owns the parsed call head and surface.
5. Replace `split_call_head` and repeated comma/equal splitting with the shared token-level argument-list parser.
6. Build `LineOptions` from the parallel semantic arguments and syntax entries.
7. Preserve no-paren shorthand as `None`; do not synthesize an empty list.
8. Pass exact option-value ranges to nested expression recovery.

### 3.6 View, line-plan, and dialogue expression producers

Migrate current call-producing paths in:

- `crates/arcweft-lang-syntax/src/parser/view.rs`;
- `crates/arcweft-lang-syntax/src/parser/line_plan.rs`;
- `crates/arcweft-lang-syntax/src/parser/flow.rs`;
- dialogue interpolation helpers;
- assertion and other syntax traversals that parse expression fragments.

They must call the exact recovering parser with source coordinates. None may construct a source call directly.

### 3.7 Delete source reconstruction

In this cut delete call-specific source recovery from `crates/arcweft-lang-syntax/src/expr/source_ranges.rs`. The collector reads `CallExpr::range()`, `callee_range()`, and the owned surface. It may retain unrelated range collection for other expression families, but it cannot search for a call delimiter.

Delete after their callers are migrated:

- `parse_static_generic_call`;
- `split_call_head` for call syntax;
- old call-argument text splitting that rediscover commas/equal signs;
- `normalize_dot_continuations` on range-bearing expression paths;
- any call-specific `find_matching_*` or source-scan branch used after parsing.

### 3.8 Cut 2 package-frontier gate

```bash
cargo fmt --all -- --check
cargo check -p arcweft-lang-syntax --all-targets
cargo clippy -p arcweft-lang-syntax --all-targets -- -D warnings
cargo test -p arcweft-lang-syntax --all-targets
```

Downstream crates are not merged at this point. The syntax crate itself must compile and its parser tests must use only the final model.

## 4. Cut 3 — remove source-less construction and migrate direct callers

### 4.1 Delete constructors

Delete from `arcweft-lang-syntax::expr::Expr`:

- `Expr::call`;
- `Expr::selected_call`.

Do not replace them with a public `CallExpr::new`, builder, trait, alias, or test-only public escape.

### 4.2 Migrate direct source-AST callers

The direct migration rule is:

| Caller intent | Final construction route |
|---|---|
| Test authored syntax | `parse_expr` / full source parser with literal source |
| Test recovered source | full recovering parser fixture with exact owner range and diagnostic sink |
| Dialogue/model fixture representing authored expression | parse the authored expression text and retain the result |
| Runtime/compiler-generated executable call | existing `RuntimeExpr::Call` / runtime-plan owner |
| Syntax-crate parser internals | crate-private validated constructors only |

Known current direct-call test/helper families include syntax parser tests, callback/View tests, and dialogue model tests. Use compiler errors after constructor deletion to enumerate every remaining source-AST caller. Do not rewrite unrelated `RuntimeExpr::Call` or other crate-owned call enums merely because the variant spelling matches.

### 4.3 Cut 3 package-frontier gate

Compile the syntax crate and every package that previously invoked a public source-call constructor. At minimum:

```bash
cargo check -p arcweft-lang-syntax --all-targets
cargo check -p arcweft-dialogue --all-targets
cargo check -p arcweft-runtime-plan --all-targets
cargo test -p arcweft-lang-syntax --all-targets
cargo test -p arcweft-dialogue --all-targets
```

The implementation records the exact additional packages exposed by compiler errors. No old constructor remains to hide a caller.

## 5. Cut 4 — exhaustive syntax/HIR/sema/tooling direct replacement

This cut restores one compiling workspace and ends the unmerged replacement series.

### 5.1 Mechanical match rule

Every source-AST match migrates as follows:

```rust
Expr::Call { callee, args } => use_call(callee, args)
```

becomes:

```rust
Expr::Call(call) => use_call(call.callee(), call.args())
```

Boolean matches use `Expr::Call(_)`. Traversals visit `call.callee()` and each `call.args()` value. Only source/range and signature-help code reads `call.syntax()`.

### 5.2 Syntax and HIR

Migrate:

- syntax visitors, assertions, source-range collectors, debug/label helpers, and parser classification;
- `arcweft-lang-hir` module/model clones, source-map tests, symbol/fact traversal, and any syntax re-export pattern matches.

Do not add a separate HIR call-surface enum. Add direct tests proving clone/equality preserves each surface.

### 5.3 Sema

Migrate all source-Expr matches in:

- `crates/arcweft-lang-sema/src/checker/expr.rs` and `checker/expr/*`;
- checker signature/effect/Fx/lifetime support;
- semantic facts and fact layer;
- project index and style-value/token-graph traversal;
- any registration or callable analysis that destructures source calls.

Ordinary type/effect checking ignores surface. The AW-AH-009.3 signature site extractor matches only `Parenthesized` and returns `NotApplicable` for `CallbackBlock` before resolver work.

### 5.4 Runtime, verification, CLI, and tooling

Migrate source-Expr matches in current consumers, including the currently audited families:

- `arcweft-runtime-plan` expression lowering, desugaring, effect lowering, pure evaluation, labels, trait methods, audio/Fx/render-text/flow helpers, host requests, and presentation lowering;
- `arcweft-verify` contract lowering;
- CLI/runtime expectation and Agent snapshot paths that inspect source Expr;
- Agent REPL binding paths;
- any formatter, LSP, test utility, or compiler traversal exposed by exhaustive matching.

Do not alter same-named `Call` variants belonging to `RuntimeExpr`, evaluator IR, SMT IR, or unrelated domain enums unless their source input match requires accessor migration.

### 5.5 Cut 4 workspace gate

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Only after this gate succeeds is the direct-replacement series committed or merged. At that point the repository has one public source call model and no compatibility interval.

## 6. Cut 5 — resume AW-AH-009.3 production cuts 1 through 6

Resume the accepted parent implementation sequence without changing its resolver/cache policies. Every parent cut that previously expected `CallExpressionSyntax.argument_list` now receives a reference obtained from:

- `CallExpr::parenthesized_syntax().argument_list()`;
- `SpeakerLineSurface::argument_list()`; or
- `ContentCallSurface::argument_list()`.

Callback-block calls and special forms without authored parentheses return `NotApplicable`. They do not enter candidate resolution or successful cache insertion.

Parent cut tests for named/reordered/duplicate/spread arguments, overloads, partial calls, missing close, recovered arguments, nested calls, presentation/dialogue schemas, native/Rust-adapter precedence, staleness, limits, and deterministic diagnostics use this one carrier.

AW-AH-009.3.2 supplies the accepted-HIR/request-lifecycle carrier separately. This call-surface cut does not add an on-demand LSP parser or a signature-specific syntax database.

## 7. Cut 6 — final validation

### 7.1 Focused tests

```bash
cargo test -p arcweft-lang-syntax --all-targets
cargo test -p arcweft-lang-hir --all-targets
cargo test -p arcweft-lang-sema --all-targets
cargo test -p arcweft-lsp --all-targets
cargo test -p arcweft-dialogue --all-targets
cargo test -p arcweft-runtime-plan --all-targets
```

Run the exact named cases in `TEST_MATRIX.md` even when they reside in a broader integration target.

### 7.2 Workspace gates

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
git diff --check
```

Use repository-standard `just` targets where they are stricter, but do not substitute a narrower target for the commands above.

### 7.3 Canonical structural audit

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/aw-ah-009-3-1-call-surface
```

The implementation note records the before/after audit, affected package tests, exact commands, exit codes, and any environment limitation. It must not claim an unrun command succeeded.

### 7.4 Human migration review, not a source gate

During review, inspect compiler-produced changes for:

- no remaining old source-call constructor;
- no ordinary call with an optional list;
- no callback encoded as parentheses;
- no call-specific post-parse source search;
- no second signature resolver;
- no fake or zero delimiter range.

Do not add a test or CI script that passes/fails by searching checked-in source spellings or file paths. Compiler exhaustiveness, direct behavioral tests, and the structural audit are the gates.

## 8. Required implementation note

After production validation, add the repository-standard implementation note under `docs/implementation/` with:

- exact Git and Jujutsu identities;
- final type ownership and any file movement;
- all deleted constructors/helpers;
- test names and commands actually run;
- workspace check, Clippy, test, format, diff, and structural-audit results;
- explicit statement that callback signature help is inapplicable and nested parenthesized calls remain applicable;
- explicit statement that generated calls use the non-authored runtime representation;
- no deferred result-changing item.
