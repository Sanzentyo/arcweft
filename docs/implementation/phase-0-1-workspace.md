# Phase 0 / Phase 1 Workspace Status

## Workspace Layout

Implemented workspace members:

- `crates/arcweft-core`
- `crates/arcweft-id`
- `crates/arcweft-source`
- `crates/arcweft-need`
- `crates/arcweft-dialogue`
- `crates/arcweft-lang-syntax`
- `crates/arcweft-cli`

The workspace is intentionally dependency-light. Backend features are declared but empty in `arcweft-core` so adapter crates can be added later without changing the core boundary.

## Implemented Types

Identity and source:

- `EntityId`
- `PublicId`
- `TextKey`
- `SourceAnchor`

Async task state:

- `Need<T, E>`
- `Progress`

Dialogue surface model:

- `DialogueLine`
- `SpeakerRef`
- `TextBoxRef`
- `DialogueContent`
- `DialogueTag`
- `LinePlan`
- `TimelineAnchor`
- `CancelScope`
- `LineExit`

Supporting dialogue model types include speaker presets, voice references, content parts, line-plan steps, plan calls, and plan expressions. These are enough to represent the initial `alice2[...] with { ... }` example as typed Rust data without implementing a parser.

Builder API:

- `SpeakerPreset`
- `SayOptions`
- `DialogueLineBuilder`
- `LinePlanBuilder`
- `TimelineCue`
- `CancelRule`
- `OutPayload`
- `CancelOnDrop`

The builder API supports fluent construction of a dialogue line with speaker defaults, line id, lossy dialogue content parsing, timeline cues, and input cancellation rules.

Syntax parser:

- `parse_source` and `parse_stub` now parse real `.awft` surface syntax into `SyntaxTree`.
- The parser records module/use headers, attributes, wiki links, flows, fragments, flow items, scenario commands, speaker lines, content calls, choice blocks, hooks, memo functions, parser items, line plans, and dialogue tokens.
- Diagnostics use structured `ParseError` values with spans, expected fragments, found text, recovery suggestions, and source anchors.
- Parser and semantic diagnostics implement `Display` and `std::error::Error` directly without external error-derive dependencies.
- Expression syntax now has an `Expr` AST for entity references, literals, tuples, calls, named arguments, method calls, dialogue calls, indexes, pipes, unary `!`, binary comparisons, and placeholders.
- Expression syntax also preserves float literals, half-open/inclusive ranges, and `in` membership expressions used by documented contracts.
- Type syntax now has `TypeRef`/`LifetimeName` support for lifetime-bearing borrow types such as `&'asset [Rgba8]`, and function signature lifetime parameters such as `fn first<'a>(...)`.
- Top-level `fn` items are parsed as structured syntax items with visibility, lifetime-bearing signature heads, parameter patterns/types, return types, contract clauses, source ranges, original body text, structured body statements, and optional final block expression. HIR lowering now carries these function bodies, and the minimal checker walks their contracts, parameters, statements, final value, and return type for parser/typecheck-readiness coverage.
- Top-level ADT declarations (`enum`, `struct`, `type`) are parsed as structured syntax items with visibility, variant/field/type information, and type-alias `where` clauses.
- Top-level `state`, `reducer`, and `view` declarations are parsed as structured syntax items. State fields keep visibility, type, and default expressions; reducers/views keep signature tails, contracts, bodies, and source ranges.
- Top-level `trait` and `impl` declarations are parsed as structured syntax items. Trait members keep associated type/function signatures; impl items keep generics, trait target, implementation target, body, and source ranges.
- Zero-copy `borrow expr as name: Type { ... }` blocks are parsed into AST/HIR, and the checker treats their non-`'static` lifetimes as active only inside the borrow body.
- Dialogue `#[...]` expressions, record expressions, compact scenario command arguments, same-line and multiline timed-cue anchors/bodies, line-plan options, line-plan `let`/`return`, line-plan assertions, line-plan cancellation actions, line-plan expression items, nested `start`/`together` groups, choice option fields, choice lifecycle plans, source-locale blocks, and `await ... with` carry parsed expressions/statements for later type checking and HIR lowering.
- Line-plan memo declarations such as `memo rich_text key=(line.id, locale, theme.text_hash) cache=flow` preserve the memo name and typed option expressions for symbol collection and checking.
- Line-plan cancellation command statements such as `stop voice fade=40ms` and `flush text instant` parse as structured command statements, and the checker allows `continue` inside line cancellation continuations as specified by the dialogue docs.
- Choice syntax covers static arm sugar (`->` as `goto`, `=>` as `out`), full `option` blocks, `ui { ... }` state, structured `select { ... }` statement blocks, dynamic `for` options, `match`-gated option groups, `option pattern in expr` sugar, `label(id=#text...)`, `value = expr`, and `with { ... }` / `with:` choice plans.
- Choice lifecycle plans parse option assignments, `timeout`, `cancel on`, `on select`, and `select` statements into structured expressions/statements for HIR readiness and minimal type checking.
- Flow `let`/`return`/`goto`/`emit`/`bail`/`ensure` statements and statement-block `if`/`match` bodies now lower to structured `Stmt` and `Pattern` values instead of opaque strings.
- Flow `if` and `match` blocks lower to structured HIR nodes, and their nested flow items participate in symbol collection and type checking. Statement-style `match` arms preserve `when` guards, validate them as `Bool`, and scope supported pattern bindings to the selected arm body.
- Flow `if let PAT = EXPR when GUARD { ... }` blocks lower to structured HIR nodes. The checker validates guard expressions as `Bool`, binds supported option payload patterns only inside the if-let body, and keeps outer locals unchanged afterward.
- Value-producing `let PAT = if COND { ... } else { ... }` expressions parse into structured expression nodes with block-expression branches. The minimal checker validates the condition as `Bool`, scopes branch-local statements, and rejects mismatched branch result types.
- Value-producing `let PAT = if let BIND = EXPR when GUARD { ... } else { ... }` expressions parse into structured expression nodes. The minimal checker validates guards as `Bool`, scopes successful pattern bindings to the then branch, and rejects mismatched branch result types.
- Value-producing `let PAT = match EXPR { PAT when GUARD => EXPR ... }` expressions parse into structured expression nodes. The minimal checker validates guards as `Bool`, walks arm patterns and values for symbol collection, scopes arm-local bindings, and rejects mismatched arm result types.
- Flow `loop { ... }` blocks and `let name = loop { ... }` expression bindings lower to structured HIR nodes. The minimal checker tracks loop contexts, accepts `break expr` only in `loop`, infers a simple unified break type for loop expression bindings, and rejects `break` outside loop contexts.
- Flow `for` loops and source-aware `select` blocks lower to structured HIR nodes, and their nested flow items participate in symbol collection and type-check readiness checks.
- Flow `while` and `while let` loops lower to structured HIR nodes. The minimal checker validates `while` conditions and `while-let` guards as `Bool`, keeps pattern bindings scoped to the loop body, and treats both loop forms as statement-oriented constructs.
- `let PAT = EXPR else { ... }` parses as a structured statement, keeps the else body as typed statements, and the checker rejects else blocks that do not leave the current continuation. `return`, `goto`, `break`, `continue`, `panic`, and `fail` are recognized as diverging statements for this minimal checker.
- Pattern syntax now preserves documented structured shapes including `mut` bindings, literals, entity-ref patterns, record/struct patterns with `..`, list/rest patterns, structured enum variant tuple/record payloads, and whole-pattern bindings such as `ev .ChoiceSelected { id }`.
- Named `scope name { ... }` blocks lower to structured HIR nodes. Relative choice IDs such as `choice .first` and relative option IDs such as `.listen` normalize through the current flow and scope path during HIR lowering.
- `let name = scope name { ... }` parses as a named scope expression binding, preserves nested typed statements and final expression separately, and lets the checker infer the bound value while keeping inner locals scoped to the block.
- Plain `let name = { ... }` block expression bindings parse as structured expression blocks with scoped statements and an optional final value.
- Dialogue call options are parsed enough to expose `id`, `text_key`, and `source_locale` to HIR. Relative dialogue line IDs such as `alice(id=.comment)` normalize through the current flow, speaker, and scope path. When `id` is omitted, HIR lowering allocates a stable per-flow/speaker/scope ordinal such as `say.opening.narrator.rain.001`, and omitted `text_key` is derived from the normalized `say...` line ID.
- `let name = choice ... { ... }` parses as a choice expression binding, lowers to HIR with normalized relative choice/option IDs, and the minimal checker can infer `Ref<Flow>` when every option uses `=> #flow...`.
- Module and import paths accept `crate::`, `self::`, `super::`, and reserved `parent::` roots as source syntax.
- The documentation from `docs/reviews/pro_review4.md` is reflected in the language specs: ordinary `{ ... }` blocks remain value-producing in expression position; `scope name { ... }` is both lexical scope and ID namespace; `.suffix` IDs are limited to line, text-key, choice, and option contexts; module-relative paths use `self::`, `super::`, or `crate::`; and `parent::` is a reserved alias that formatter/canonicalizer work should normalize to `super::`.
- `await ... with` keeps `pending`/`ready`/`error`/`denied` branches as structured AST/HIR, and branch bodies participate in symbol collection and type checking.
- Ordinary Rust-like propagation syntax is represented in expression AST. `expr?` and prefix `try expr` parse as structured try expressions, participate in symbol collection, and the minimal checker unwraps `Result<T, E>`-like types while rejecting non-result expressions.
- Flow/function contract clauses (`requires`, `ensures`, `invariant`, `assume`, `reads`, `effects`, `no_effect`, `modifies`, `decreases`) are parsed separately from the body and participate in symbol collection and type checking where applicable.
- `lower_to_hir` verifies that parsed edge-case flow syntax can be converted to HIR-facing structures and rejects raw syntax that still needs parser coverage.
- `collect_symbol_uses` walks HIR without reparsing source snippets so name resolution can see dialogue callees, entity references, paths, calls, methods, dialogue text expressions, timed cues, and choice-condition references.
- `registry_from_hir` and `validate_hir_references` provide minimal name resolution over HIR declarations and entity references.
- `validate_typecheck_ready` rejects lowered HIR that still contains raw expression fragments before the future type checker sees it.
- `typecheck_hir` provides a minimal semantic checker over HIR with an explicit environment. It validates flow/fragment entity reference families, dialogue callees, `Need<T, E>` awaits, `Duration` timeline anchors, indexed expressions, calls, and methods for parser/HIR integration tests.
- Typed let patterns and borrow blocks preserve borrow types, and the checker rejects non-`'static` borrowed values crossing `await`, `yield`, `spawn`, and `defer` suspension boundaries.

## Deferred

Not implemented in this milestone:

- wgpu renderer
- Servo / DOM UI
- audio backend
- camera / capture devices
- USB / HID / gamepad backends
- MCP / agent protocol runtime
- Cranelift JIT
- full HIR ownership/region model
- full function signature parameter destructuring, generic substitution, and effect-aware return checking
- full type environment, name resolution, and type checking
- inference, overload resolution, traits, generics, contracts, and effect checking
- full nested-scope borrow lifetime analysis and precise borrow end tracking
- full semantic expression resolution and type-directed ambiguity resolution
- full choice expression type unification beyond the current `=> #flow...` case, lifecycle runtime execution, reactive option-state reevaluation, localization extraction, formatter/canonicalizer output, and LSP diagnostics for dynamic labels and unordered map-backed options
- full localization extraction manifests and formatter/canonicalizer normalization for `parent::` and relative `.suffix` IDs

## Verification

Last verified during the Phase 0 / Phase 1 workspace pass:

```bash
cargo fmt
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
```

Result:

- `cargo fmt`: passed
- `cargo clippy --workspace --all-targets --all-features`: passed
- `cargo test --workspace`: passed
