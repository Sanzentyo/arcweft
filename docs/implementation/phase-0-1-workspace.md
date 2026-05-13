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
- Pipeline and helper-call expressions preserve `_`/`^` placeholders, placeholder field access such as `_.enabled`, generic method names such as `collect<List<T>>()`, and closure arguments such as `with_context(|| "...")` without falling back to raw expressions.
- List expressions such as `[normal, smile, worried]`, `[]`, and nested call arguments parse as structured `Expr::List` nodes and participate in symbol collection and minimal type checking. Bare record/map literals such as `{ player_name = state.player_name }` and `{}` parse as structured `Expr::RecordLiteral` nodes for dialogue args and state defaults.
- Type syntax now has `TypeRef`/`LifetimeName` support for lifetime-bearing borrow types such as `&'asset [Rgba8]`, and function signature lifetime parameters such as `fn first<'a>(...)`.
- Top-level `fn`, `task fn`, `dialogue fn`, and `stream fn` items are parsed as structured syntax items with visibility, lifetime-bearing signature heads, parameter patterns/types, return types, contract clauses, source ranges, original body text, structured body statements, and optional final block expression. HIR lowering now carries the function kind and body, and the minimal checker walks their contracts, parameters, statements, final value, and return type for parser/typecheck-readiness coverage.
- Top-level ADT declarations (`enum`, `struct`, `type`) are parsed as structured syntax items with visibility, variant/field/type information, type-alias `where` clauses, and HIR declaration preservation.
- Top-level `state`, `reducer`, and `view` declarations are parsed as structured syntax items. State fields keep visibility, type, and default expressions; reducers/views keep signature tails, contracts, bodies, source ranges, and HIR declaration preservation.
- Top-level `trait` and `impl` declarations are parsed as structured syntax items. Trait members keep associated type information, including GAT-style associated type parameters such as `type Mapped<B>`, and structured function signatures, including `self` receivers. Impl items keep generics, trait target, implementation target, original body text, source ranges, HIR declaration preservation, associated type assignments such as `type Mapped<B> = Option<B>`, and function member signatures with structured body statements/final expressions for later lowering.
- Top-level `hook`, `memo fn`, and `parser` declarations preserve structured body statements and final expressions in AST/HIR, including generic parser-combinator blocks such as `alt { ... }`.
- Bodyless parser declarations such as `pub parser parse_player_command: Parser<PlayerCommand, ParseError>` are accepted and lower as parser declarations with empty bodies, matching the parser API declarations used in the language and device examples.
- Top-level declarative `source` declarations such as `pub source #source.face_camera_frames: Source<VideoFrameHandle, CaptureError> { ... }` are parsed as structured syntax items with source IDs, signature tails, source policy statements, and `on ... => ...` event branches. HIR preserves them as declarations, readiness checking walks their structured statements, and minimal type checking validates the source ID family without implementing camera/audio/USB runtime backends.
- Function-like `source name() -> Source<T, E> { loop { ... yield ... } }` declarations also parse as source items. Typed statement bodies now preserve `loop`, `while`, `while let`, and `for` statements inside functions, parsers, memo functions, hooks, and source declarations so generator-style source examples can lower and typecheck without raw statement fragments.
- Top-level `signal`, `character`, `layer`, `activity`, and `component` declarations from the presentation/runtime docs parse as structured entity declarations with visibility, entity ID, optional public name, signature tail, optional body, and source range. HIR preserves them as declarations, registers their entity IDs for name-resolution tests, and minimally checks that the public ID prefix matches the declaration family without implementing rendering, activity, camera, audio, or USB backends.
- Top-level `extern rust mod ... from crate "..." { ... }` declarations from the module docs parse as structured external-module declarations with ABI, module path, import source, body text, and source range. HIR preserves them as syntax-level declarations for later Rust/WASM adapter work without implementing external runtime loading in Phase 0 / Phase 1.
- Zero-copy `borrow expr as name: Type { ... }` blocks are parsed into AST/HIR, and the checker treats their non-`'static` lifetimes as active only inside the borrow body.
- Dialogue `#[...]` expressions, record expressions, compact scenario command arguments, same-line and multiline timed-cue anchors/bodies, line-plan options, line-plan `let`/`out`, line-plan assertions, line-plan cancellation actions, line-plan expression items, nested `start`/`together` groups, choice option fields, choice lifecycle plans, source-locale blocks, and `await ... with` carry parsed expressions/statements for later type checking and HIR lowering.
- Line-plan memo declarations such as `memo rich_text key=(line.id, locale, theme.text_hash) cache=flow` preserve the memo name and typed option expressions for symbol collection and checking.
- Line-plan cancellation command statements such as `stop voice fade=40ms` and `flush text instant` parse as structured command statements, and the checker allows `continue` inside line cancellation continuations as specified by the dialogue docs.
- Choice syntax covers static arm sugar (`->` as `goto`, `=>` as `out`), full `option` blocks, `ui { ... }` state, structured `select { ... }` statement blocks, dynamic `for` options, `match`-gated option groups, `option pattern in expr` sugar, `label(id=#text...)`, `value = expr`, and `with { ... }` / `with:` choice plans.
- Choice HIR preserves the source choice-body item tree as well as the flattened option list, so `let`/`if`/`for`/`match` guards and raw malformed choice-body items participate in symbol collection, readiness checks, and minimal type checking.
- Choice lifecycle plans parse option assignments, `timeout`, `cancel on`, `on select`, and `select` statements into structured expressions/statements for HIR readiness and minimal type checking.
- Flow `let`/`return`/`goto`/`emit`/`bail`/`ensure` statements and statement-block `if`/`match` bodies now lower to structured `Stmt` and `Pattern` values instead of opaque strings.
- Flow `if` and `match` blocks lower to structured HIR nodes, and their nested flow items participate in symbol collection and type checking. Statement-style `match` arms preserve `when` guards, validate them as `Bool`, and scope supported pattern bindings to the selected arm body.
- Flow `if let PAT = EXPR when GUARD { ... }` blocks lower to structured HIR nodes. The checker validates guard expressions as `Bool`, binds supported option payload patterns only inside the if-let body, and keeps outer locals unchanged afterward.
- Value-producing `let PAT = if COND { ... } else { ... }` expressions parse into structured expression nodes with block-expression branches. The minimal checker validates the condition as `Bool`, scopes branch-local statements, and rejects mismatched branch result types.
- Value-producing `let PAT = if let BIND = EXPR when GUARD { ... } else { ... }` expressions parse into structured expression nodes. The minimal checker validates guards as `Bool`, scopes successful pattern bindings to the then branch, and rejects mismatched branch result types.
- Value-producing `let PAT = match EXPR { PAT when GUARD => EXPR ... }` expressions parse into structured expression nodes. The minimal checker validates guards as `Bool`, walks arm patterns and values for symbol collection, scopes arm-local bindings, and rejects mismatched arm result types.
- Named computation blocks such as `let route = result { ... }`, `let assets = task { ... }`, `let visible = seq { ... }`, and `let levels = stream { ... }` parse into structured expression nodes with scoped statements and optional final values.
- Memo expression blocks such as `let actor = memo(scope=scene, key=(...)) { ... }` parse into structured expression nodes with memo options, scoped statements, and optional final values.
- Flow `loop { ... }` blocks and `let name = loop { ... }` expression bindings lower to structured HIR nodes. The minimal checker tracks loop contexts, accepts `break expr` only in `loop`, infers a simple unified break type for loop expression bindings, and rejects `break` outside loop contexts.
- Control-transfer statements preserve Rust-like label references for `break 'label expr`, `continue 'label`, and `out 'label expr` so diagnostics can name the intended continuation without treating the statement as raw syntax. `let value = 'label: loop { ... }` and line-plan `with 'label { ... }` also preserve their labels in AST/HIR, and the minimal checker rejects unresolved loop labels and unresolved line-plan `out` labels.
- Flow `for` loops and source-aware `select` blocks lower to structured HIR nodes, and their nested flow items participate in symbol collection and type-check readiness checks.
- Flow `while` and `while let` loops lower to structured HIR nodes. The minimal checker validates `while` conditions and `while-let` guards as `Bool`, keeps pattern bindings scoped to the loop body, and treats both loop forms as statement-oriented constructs.
- `let PAT = EXPR else { ... }` parses as a structured statement, keeps the else body as typed statements, and the checker rejects else blocks that do not leave the current continuation. `return`, `goto`, `break`, `continue`, `panic`, and `fail` are recognized as diverging statements for this minimal checker.
- Pattern syntax now preserves documented structured shapes including `mut` bindings, literals, entity-ref patterns, record/struct patterns with `..`, list/rest patterns, structured enum variant tuple/record payloads, and whole-pattern bindings such as `ev .ChoiceSelected { id }`.
- Named `scope name { ... }` blocks lower to structured HIR nodes. Relative choice IDs such as `choice .first` and relative option IDs such as `.listen` normalize through the current flow and scope path during HIR lowering.
- `let name = scope name { ... }` parses as a named scope expression binding, preserves nested typed statements and final expression separately, and lets the checker infer the bound value while keeping inner locals scoped to the block.
- Plain `let name = { ... }` block expression bindings parse as structured expression blocks with scoped statements and an optional final value.
- Dialogue call options are parsed enough to expose `id`, `text_key`, and `source_locale` to HIR. Relative dialogue line IDs such as `alice(id=.comment)` normalize through the current flow, speaker, and scope path. When `id` is omitted, HIR lowering allocates a stable per-flow/speaker/scope ordinal such as `say.opening.narrator.rain.001`, and omitted `text_key` is derived from the normalized `say...` line ID.
- `let name = choice ... { ... }` parses as a choice expression binding, lowers to HIR with normalized relative choice/option IDs, and the minimal checker can infer `Ref<Flow>` when every option uses `=> #flow...`.
- Module and import paths accept `crate::`, `self::`, `super::`, and reserved `parent::` roots as source syntax, normalize parsed `parent::` roots to canonical `super::`, and reject relative `.suffix` ID syntax in `mod`/`use` paths so ID-relative notation stays limited to line, text-key, choice, and option contexts.
- The documentation from `docs/reviews/pro_review4.md` is reflected in the language specs: ordinary `{ ... }` blocks remain value-producing in expression position; `scope name { ... }` is both lexical scope and ID namespace; `.suffix` IDs are limited to line, text-key, choice, and option contexts; module-relative paths use `self::`, `super::`, or `crate::`; and `parent::` is a reserved alias that formatter/canonicalizer work should normalize to `super::`.
- `await ... with` keeps `pending`/`ready`/`error`/`denied` branches as structured AST/HIR, and branch bodies participate in symbol collection and type checking.
- Bound wait-view expressions such as `let assets = try await load_opening_assets() with { ... }` and `let result = await load_opening_assets() with:` lower to explicit await-binding HIR. The minimal checker validates the awaited expression as `Need<T, E>`, scopes wait-view branch patterns, and binds the outer pattern as `T` for `try await` / `await?` or `Result<T, E>` for plain `await`.
- Bound wait-view parsing accepts documented multi-line context chains before `with:`, such as `let bg = try await asset.image(...)\n    .context(...)\nwith:`, while plain `let bg = try await load_bg()` without a wait-view remains a normal await expression binding.
- Wait-view branch patterns include structured variant payloads, so documented activity forms such as `pending .Realizing(p) => ... p.ratio` bind the payload inside only that branch.
- The minimal checker accepts dotted member access on scoped locals in wait-view bodies, so documented forms such as `pending p => ... progress p.ratio` validate without requiring `p.ratio` to be registered as a global symbol.
- Background task-style `await expr`, `try await expr`, and `await? expr` without a wait-view block parse as structured expression AST. The minimal checker requires the awaited expression to have `Need<T, E>` type, returns `Result<T, E>` for plain `await`, and unwraps to `T` for `try await` / `await?`.
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
- full localization extraction manifests and formatter/canonicalizer normalization for relative `.suffix` IDs

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
