# Phase 0 / Phase 1 Workspace Status

## Workspace Layout

Implemented workspace members:

- `crates/arcweft`
- `crates/arcweft-core`
- `crates/arcweft-dialogue`
- `crates/arcweft-id`
- `crates/arcweft-lang-hir`
- `crates/arcweft-lang-sema`
- `crates/arcweft-lang-syntax`
- `crates/arcweft-need`
- `crates/arcweft-presentation`
- `crates/arcweft-runtime-plan`
- `crates/arcweft-source`
- `crates/arcweft-test`
- `crates/arcweft-verify`
- `crates/arcweft-verify-lsp`
- `crates/arcweft-verify-oxiz`
- `crates/arcweft-verify-z3`
- `crates/arcweft-cli`

The workspace is intentionally dependency-light. `arcweft-core` keeps only placeholder non-JIT adapter feature flags and has no Cranelift/Wasmtime dependency or Cranelift feature. Native JIT belongs in a future `arcweft-lang-jit-cranelift` adapter selected by player/build crates.

The largest runtime and semantic crates are being split by responsibility per
`docs/reviews/pro_review21.md`. `arcweft-core` now exposes public module
boundaries instead of a flat root API:

- `time`
- `frame`
- `value`
- `pattern`
- `effect`
- `task`
- `source`
- `stream`
- `plan`
- `line_task`
- `observation`
- `engine`

Downstream crates import core runtime data through those modules. The
`arcweft-lang-sema` split has started with public `check`, `checker`, `types`,
`env`, `diagnostics`, `borrow`, and `lifetime` modules, while the larger
language-family checker split now includes `choice`, `expr`, `flow`,
`line_plan`, `source`, and `stmt` child modules. `arcweft-runtime-plan` is split across lowering-family
modules including `errors`, `expr`, `flow`, `labels`, `line_task`, `pattern`,
`source`, and `stream`.
`arcweft-lang-hir` now separates public HIR data into `model.rs`, public
lowering entry points into `lower.rs`, and lowering responsibilities into
`lower_flow.rs`, `lower_dialogue.rs`, `lower_choice.rs`, `lower_ids.rs`, and
`lower_context.rs`. Downstream crates no longer rely on flat crate-root HIR
compatibility re-exports; they import through `model`, `lower`, `id_context`,
or the namespaced `syntax` module.
`arcweft-lang-syntax` has moved AST family definitions out of the root AST
facade: `ast/items.rs` owns top-level declarations/tree/recovery wrappers,
`ast/common.rs` owns range/module/use/doc primitives, `ast/ids.rs` owns
entity/reference IDs, `ast/pattern.rs` owns structured binding syntax,
`ast/flow.rs` owns flow/control-transfer syntax, `ast/dialogue.rs` owns dialogue
surface syntax, `ast/line_plan.rs` owns line-plan syntax, `ast/choice.rs` owns
choice syntax, `ast/proof.rs` owns proof/test/bench declarations, and
`ast/source.rs` owns declarative source-stream syntax.
The parser split has also started with `parser/recovery.rs` for parse
diagnostic/recovery types, `parser/source.rs` for source-item
header/handler/body parsing, and `parser/proof.rs` for proof/test item
clause parsing; remaining family parser modules remain a follow-up.
Runtime plan lowering imports syntax-owned surface types through
`arcweft-lang-hir`'s `syntax` namespace and no longer declares a direct
dependency on `arcweft-lang-syntax`.

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

Dialogue options now use `look` for the line expression/portrait default. The
previous `face` option is not preserved as a compatibility API in unfinished
parser/dialogue code.

Presentation model:

- `PresentationScope`
- `PresentationTarget`
- `PresentationSlot`
- `PresentationHandle<T>`
- `SlotRef<T>`
- `SlotValue<T>`
- `ClearPresentation<T>`
- `PresentationRegistry<T>`
- `BackgroundSurface`
- `CharacterSurface`

`arcweft-presentation` owns these Sans I/O types. `PresentationRegistry<T>`
enforces scope lifetime by removing registered values for a scope when
`exit_scope` is called. `arcweft-dialogue` keeps only dialogue-specific adapter
helpers that turn `SpeakerRef` into presentation character IDs.

Syntax parser:

- `parse_source` returns `ParsedSource`, an always-available parsed-source
  container with original source text, a lossless rowan CST, the current typed
  `TypedSyntaxTree` view, recoverable parse diagnostics, a line index, and a
  deterministic source hash. `parse_stub` has been removed.
- The typed source model is intentionally named `TypedSyntaxTree` so it is not
  confused with rowan `SyntaxNode` / CST ownership.
- The lossless CST is built from `SyntaxKind`, `ArcweftLanguage`, and rowan
  green nodes. It preserves whitespace, newlines, ordinary comments,
  Markdown-doc comments, entity references, strings, punctuation, and fallback
  text tokens so tooling can keep exact source text even when typed parsing
  reports errors.
- The current typed parser consumes `CstLineEvents`, a small newtype projected
  from rowan `Line` nodes through `From<&SyntaxNode>`. Each line event carries a
  `CstLineKind` (`Blank`, `Comment`, `DocComment`, `Code`) so trivia and doc
  comments are classified by the CST/event layer rather than by repeated parser
  string checks. Nested parser calls also rebuild CST before producing typed
  views.
- Balanced delimiter splitting, delimiter-depth recovery, top-level keyword
  splitting, top-level whitespace splitting, top-level binding splitting,
  leading identifier/lifetime/entity-ref/relative-id splitting, multi-token
  punctuation sequence splitting, and matching punctuation are centralized in
  the CST layer and are reused by the parser, expression parser, pattern
  parser, and type parser. Current parser call sites use these helpers for
  block recovery, line-plan cues, choice/action separators, `::` variant path
  splitting, await grouping, await `with` heads, extern module headers, borrow
  aliases, event fields, scenario command args, labels, entity refs, and nested
  pattern/type delimiters.
- Parser-facing grammar delimiter decisions no longer live in local raw string
  scans in `parser.rs`, `expr.rs`, `pattern.rs`, or `types.rs`. Raw character
  scans remain inside the CST lexer and named CST text utilities, where they
  tokenize source or implement wiki-link and string-literal extraction. New
  grammar-level delimiter or keyword behavior should be added to the CST/event
  layer first.
- Top-level parsing now dispatches through a small `CstTopLevelLineKind` event
  classification (`OldMemoAttribute`, `Attribute`, `Module`, `Use`, `Item`).
  This is still an interim line-event bridge, but it keeps top-level routing
  separate from the item builders and makes the remaining migration path toward
  grammar-level rowan events more explicit.
- Top-level item parsing now also uses a CST-owned `CstTopLevelItemKind`
  classification for declaration families such as flows, functions, ADTs,
  entity declarations, hooks, parser/source declarations, and the flow-item/raw
  fallback. The classifier preserves current parser behavior while making the
  next migration step a typed CST/event replacement rather than another
  string-dispatch chain.
- Flow-body parsing now starts from CST-owned `CstFlowItemKind`,
  `CstStructuredFlowBlockKind`, and `CstLetFlowItemKind` classifications. The
  typed parser still owns AST construction and semantic recovery, but syntax
  family routing for structured blocks, special `let` forms, await wait-views,
  includes, old `@choice` recovery, and generic typed statements no longer lives
  as one parser-local string-dispatch chain.
- Generic statement parsing now starts from CST-owned `CstStmtKind`
  classification for lifetime registry writes, `wait`, `let`, `defer`,
  control transfer, `ensure`, `on`, unsafe-lifetime audit blocks, braced
  statements, presentation calls, scenario commands, and expression-like
  statements. Unsupported statements recover as `RawSyntaxFamily::Stmt` with
  source text and range metadata, so later semantic, verifier, runtime-plan,
  CLI, and LSP passes can report the typed recovery node without rescanning
  source snippets.
- Balanced brace-block collection now lives on `CstLineEvents` as a
  `CstBlockEvent` collector. It covers ordinary first-top-level-open blocks and
  function-body blocks, including body-open recovery, so flow/function/item
  builders no longer maintain separate brace-depth scanners.
- Flow-like block collection is also event-layer owned. It preserves contract
  and `effects { ... }` prelude lines in the returned header before collecting
  the following brace body, which keeps flow/callable/entity/source builders
  from maintaining their own prelude scanners.
- The `TypedSyntaxTree` view is still produced by the existing parser builder.
  That builder remains a temporary compatibility point inside
  `arcweft-lang-syntax`; future parser work should move it onto CST/event
  parsing rather than growing the private line-splitting helpers.
- The parser records module/use headers, attributes, wiki links, flows, fragments, flow items, scenario commands, speaker lines, content calls, choice blocks, hooks, memo functions, parser items, line plans, and dialogue tokens.
- Bracket ruby spans such as `[ruby rt="..."]base[/ruby]` and function/content ruby such as `#[ruby("base", "ruby")]` normalize to the same `DialogueToken::Ruby` shape as natural Japanese ruby.
- Dialogue raw spans and blocks such as `[raw]...[/raw]` tokenize as literal raw content, so inner `[p]` markers and `#[expr]` interpolations are not parsed until the raw span ends.
- Dialogue markers such as `[mark .release_focus]` tokenize as structured
  `DialogueToken::Mark` values. The checker rejects duplicate line marks,
  removed local hook tags, and `with: on .name:` handlers that do not match a
  mark in the same line.
- Diagnostics use structured `ParseError` values with spans, expected fragments, found text, recovery suggestions, and source anchors.
- Parser and semantic diagnostics implement `std::error::Error` through `thiserror` while preserving structured fields such as `message`, `range`, and `anchor`.
- Expression syntax now has a Pratt-style parser and an `Expr` AST for entity references, literals, tuples, calls, named arguments, method calls, indexes, field access, pipes, prefix `try`, postfix `?`, unary `!`/`-`, arithmetic operators, comparisons, and placeholders.
- Generic expression brackets are always `Expr::Index`; dialogue content brackets become `Expr::DialogueCall` only in dialogue-capable parser contexts such as flow content calls and line-result bindings.
- Expression syntax also preserves float literals, half-open/inclusive ranges, and `in` membership expressions used by documented contracts.
- Pipeline and helper-call expressions preserve `_`/`^` placeholders, placeholder field access such as `_.enabled`, generic method names such as `collect<Vec<T>>()`, and closure arguments such as `with_context(|| "...")` without falling back to raw expressions.
- Bracket sequence expressions such as `[normal, smile, worried]`, `[]`, and nested call arguments parse as structured `Expr::BracketSeq` nodes and participate in symbol collection and minimal type checking. Bare record/map literals such as `{ player_name = state.player_name }` and `{}` parse as structured `Expr::RecordLiteral` nodes for dialogue args and state defaults.
- Type syntax now has `TypeRef`/`LifetimeName` support for lifetime-bearing borrow types such as `&'asset [Rgba8]`, function signature generic parameters such as `fn first<'a, T>(...)`, curried parameter groups, `where` predicates, and the bottom type spellings `!` / `Never`.
- `//` is the ordinary comment form. `#` is reserved for entity references and is not skipped as a comment. Consecutive `///` lines are preserved as Markdown doc comments on supported declarations, fields, enum variants, and function parameters.
- Top-level `fn`, `task fn`, `dialogue fn`, and `stream fn` items are parsed as structured syntax items with visibility, generic signature heads, curried parameter groups, shared `Pattern` AST parameter bindings/types, return types, `where` predicates, contract clauses, source ranges, original body text, structured body statements, and optional final block expression. HIR lowering now carries the function kind and body, and the minimal checker walks their contracts, destructured parameters, statements, final value, and return type for parser/typecheck-readiness coverage.
- Top-level ADT declarations (`enum`, `struct`, `type`) are parsed as structured syntax items with visibility, variant/field/type information, type-alias `where` clauses, and HIR declaration preservation.
- Top-level `state`, `reducer`, and `view` declarations are parsed as structured syntax items. State fields keep visibility, type, and default expressions; reducers/views keep signature tails, contracts, bodies, source ranges, and HIR declaration preservation.
- Top-level `trait` and `impl` declarations are parsed as structured syntax items. Trait members keep associated type information, including GAT-style associated type parameters such as `type Mapped<B>`, and structured function signatures, including `self` receivers. Impl items keep generics, trait target, implementation target, original body text, source ranges, HIR declaration preservation, associated type assignments such as `type Mapped<B> = Option<B>`, and function member signatures with structured body statements/final expressions for later lowering.
- Top-level `hook`, `memo fn`, and `parser` declarations preserve structured body statements and final expressions in AST/HIR, including generic parser-combinator blocks such as `alt { ... }`. Hook headers now preserve canonical `when`, integer `priority`, `once`, and `effects` fields; legacy `check` headers are rejected.
- Top-level `dialogue defaults` declarations parse as structured declarations with optional visibility/id and assignment expressions preserved for later dialogue style/window/voice/hook lowering.
- Bodyless parser declarations such as `pub parser parse_player_command: Parser<PlayerCommand, ParseError>` are accepted and lower as parser declarations with empty bodies, matching the parser API declarations used in the language and device examples.
- Top-level declarative `source` declarations such as `pub source @source.face_camera_frames: Source<VideoFrameHandle, CaptureError> { ... }` are parsed as structured syntax items with source IDs, typed `Source<T, E>` signatures, structured `from` / `backpressure` / `replay` / `privacy` headers, and structured `on ... => ...` event branches. HIR preserves them as declarations, readiness checking walks their structured statements, and minimal type checking requires complete source policy without implementing camera/audio/USB runtime backends.
- Function-like `source name() -> Source<T, E> { loop { ... yield ... } }` declarations still parse for diagnostics, but the checker rejects them as non-canonical authoring syntax. Use `source @source.id: Source<T, E> { ... }` so replay/privacy/backpressure policy remains explicit.
- Top-level `signal`, `character`, `layer`, `activity`, and `component` declarations from the presentation/runtime docs parse as structured entity declarations with visibility, entity ID, optional public name, signature tail, optional body, and source range. HIR preserves them as declarations, registers their entity IDs for name-resolution tests, and minimally checks that the public ID prefix matches the declaration family without implementing rendering, activity, camera, audio, or USB backends.
- Top-level `extern rust mod ... from crate "..." { ... }` declarations from the module docs parse as structured external-module declarations with ABI, module path, import source, body text, and source range. HIR preserves them as syntax-level declarations for later Rust/WASM adapter work without implementing external runtime loading in Phase 0 / Phase 1.
- Zero-copy `borrow expr as name: Type { ... }` blocks are parsed into AST/HIR, and the checker treats their non-`'static` lifetimes as active only inside the borrow body.
- Dialogue `#[...]` content interpolation, record expressions, compact scenario command arguments, same-line and multiline timed-cue anchors/bodies, line-plan options, line-plan `let`/`out`, line-plan assertions, line-plan cancellation actions, line-plan expression items, nested `start`/`together` groups, choice option fields, choice lifecycle plans, source-locale blocks, and `await ... with` carry parsed expressions/statements for later type checking and HIR lowering.
- Line-plan memo declarations such as `memo rich_text key=(line.id, locale, theme.text_hash) cache=flow` preserve the memo name and typed option expressions for symbol collection and checking.
- Line-plan cancellation uses canonical ordinary calls such as
  `voice.stop(fade = 40ms)`, `cues.stop(policy = .CancelPending)`, and
  `text.flush(mode = .Instant)`; the checker allows `continue` inside line
  cancellation continuations as specified by the dialogue docs.
- Canonical runtime observation uses ordinary calls: `log.info(...)`,
  `log.debug(...)`, `log.warn(...)`, `signal.set(target, value)`, and
  `metric.set(target, value)`, and `event.emit(Event, fields)`.
- `metric gauge @metric.*: T`, `metric counter @metric.*: T`, and generic
  `metric @metric.*: T` parse as metric entity declarations so metric writes
  can be resolved and observed by the headless runtime.
- Flow/function `effects { ... }` contracts and hook header `effects` entries
  are lowered into typed semantic capability facts. These facts discharge known
  write-call obligations such as `signal.set(...) -> signal.write` and
  `metric.set(...) -> metric.write`, plus upper-lifetime registry writes such
  as `state.write('flow) -> state.write(flow)`; missing facts still surface
  through the verifier report rather than being hidden by the type checker.
- Phase 1.5 line-plan lowering preserves line options, `let`, `out`,
  cancellation rules, memo directives, assertions, structured log calls,
  signal writes, metric writes, `event.emit(...)` calls, scenario commands, and
  ordinary effect calls as typed `arcweft-core` runtime request categories.
- Stream/source lowering now produces `StreamPlan` and `SourcePlan` data
  separate from flow and line-plan effects. `LineEffectRequest::Yield` was
  removed; source event queueing, frame-boundary normalization, replay/privacy
  policy data, and backpressure handling live in Sans I/O core data structures.
- The core engine now dispatches normalized `FrameInput.source_events` through
  lowered source handlers. Handler `yield` applies the source backpressure
  policy, non-item source events update close/error state, and the first
  `StreamPlan` interpreter drains source/stream queues with deterministic
  `ForNext` / `Yield` behavior under a fixed per-frame budget.
- The core engine now records cumulative headless observations for
  `LineEffectRequest::Log`, `SignalWrite`, `MetricWrite`, and `EmitEvent`.
  `arcw run --json` exposes those observations per frame as the basis for
  scenario test expectations and replay/debug inspection.
- Choice syntax covers static arm sugar (`->` as `goto`, `=>` as `out`), full `option` blocks, `ui { ... }` state, structured `select { ... }` statement blocks, dynamic `for` options, `match`-gated option groups, `option pattern in expr` sugar, `label(id=@text...)`, `value = expr`, and `with { ... }` / `with:` choice plans.
- Choice HIR preserves the source choice-body item tree as well as the flattened option list, so `let`/`if`/`for`/`match` guards and raw malformed choice-body items participate in symbol collection, readiness checks, and minimal type checking.
- Choice lifecycle plans parse option assignments, `timeout`, `cancel on`, `on select`, and `select` statements into structured expressions/statements for HIR readiness and minimal type checking.
- Flow `let`/`return`/`goto`/`bail`/`ensure` statements and statement-block `if`/`match` bodies now lower to structured `Stmt` and `Pattern` values instead of opaque strings.
- Flow declarations preserve parsed parameter/return signatures through AST and HIR, and flow parameters are bound as locals during minimal type checking.
- Flow `if` and `match` blocks lower to structured HIR nodes, and their nested flow items participate in symbol collection and type checking. Statement-style `match` arms preserve `when` guards, validate them as `Bool`, and scope supported pattern bindings to the selected arm body.
- Flow `if let PAT = EXPR when GUARD { ... }` blocks lower to structured HIR nodes. The checker validates guard expressions as `Bool`, binds supported option payload patterns only inside the if-let body, and keeps outer locals unchanged afterward.
- Value-producing `let PAT = if COND { ... } else { ... }` expressions parse into structured expression nodes with block-expression branches. The minimal checker validates the condition as `Bool`, scopes branch-local statements, and rejects mismatched branch result types.
- Value-producing `let PAT = if let BIND = EXPR when GUARD { ... } else { ... }` expressions parse into structured expression nodes. The minimal checker validates guards as `Bool`, scopes successful pattern bindings to the then branch, and rejects mismatched branch result types.
- Value-producing `let PAT = match EXPR { PAT when GUARD => EXPR ... }` expressions parse into structured expression nodes. The minimal checker validates guards as `Bool`, walks arm patterns and values for symbol collection, scopes arm-local bindings, and rejects mismatched arm result types.
- Named computation blocks such as `let route = result { ... }`, `let assets = task { ... }`, `let visible = seq { ... }`, and `let levels = stream { ... }` parse into structured expression nodes with scoped statements and optional final values.
- `yield` is checked through an explicit generation context stack. It is accepted only in `seq`, `stream`, `stream fn`, and source handlers; flow bodies and dialogue line plans reject it with guidance to use `return`/`goto` or `out`. `seq` blocks reject runtime effects, and `stream fn` must return `Stream<T, E>` with at least one yield.
- Memo expression blocks such as `let actor = memo(scope=scene, key=(...)) { ... }` parse into structured expression nodes with memo options, scoped statements, and optional final values.
- Flow `loop { ... }` blocks and `let name = loop { ... }` expression bindings lower to structured HIR/runtime nodes. The minimal checker tracks loop contexts, accepts `break expr` only in `loop`, infers a simple unified break type for loop expression bindings, and rejects `break` outside loop contexts. The headless runtime now binds the `break expr` value to the `let` pattern after the loop body scope exits.
- Control-transfer statements preserve Rust-like label references for `break 'label expr`, `continue 'label`, and `out 'label expr` so diagnostics can name the intended continuation without treating the statement as raw syntax. `let value = 'label: loop { ... }` and line-plan `with 'label { ... }` also preserve their labels in AST/HIR, and the minimal checker rejects unresolved loop labels and unresolved line-plan `out` labels.
- Flow `for` loops and source-aware `select` blocks lower to structured HIR nodes, and their nested flow items participate in symbol collection and type-check readiness checks.
- Flow `while` and `while let` loops lower to structured HIR nodes. The minimal checker validates `while` conditions and `while-let` guards as `Bool`, keeps pattern bindings scoped to the loop body, and treats both loop forms as statement-oriented constructs.
- `let PAT = EXPR else { ... }` parses as a structured statement, keeps the else body as typed statements, and the checker rejects else blocks that do not leave the current continuation. `return`, `goto`, `break`, `continue`, `panic`, and `fail` are recognized as diverging statements for this minimal checker.
- Pattern syntax now preserves documented structured shapes including `mut` bindings, literals, entity-ref patterns, record/struct patterns with `..`, list/rest patterns, structured enum variant tuple/record payloads, and whole-pattern bindings such as `ev .ChoiceSelected { id }`.
- Named `scope name { ... }` blocks lower to structured HIR nodes. Relative choice IDs such as `choice @.first` and relative option IDs such as `@.listen` normalize through the current flow and scope path during HIR lowering.
- `let name = scope name? { ... }` parses as a scope expression binding, preserves nested typed statements and final expression separately, and lets the checker infer the bound value while keeping inner locals scoped to the block. Omitting the name creates the same lexical/value scope without adding a generated-ID scope segment. The headless runtime evaluates the final expression before popping the scope, then binds the result outside the scope.
- Plain `let name = { ... }` block expression bindings parse as structured expression blocks with scoped statements and an optional final value.
- Dialogue call options are parsed as structured `LineOptions`: `id`,
  `text_key`, `voice`, `look`, `stage`, `portrait`, `focus`, `cleanup`,
  `window`, `source_locale`, `hooks`, `style`, and additional named args are
  preserved without raw argument strings. The first positional line option maps
  to `look`; removed `face` line options produce parser diagnostics. Relative
  dialogue line IDs such as `alice(id=@.comment)` normalize through the current
  flow, speaker, and scope path. When `id` is omitted, HIR lowering allocates a
  stable per-flow/speaker/scope ordinal such as
  `say.opening.narrator.rain.001`, and omitted `text_key` is derived from the
  normalized `say...` line ID.
- Line plans preserve `init`, generic `thread name` blocks, scoped
  `defer { ... }`, `defer on completed|cancelled|failed`, local `on .mark` handlers,
  `wait mark .name`, duration waits, and `'line.* <- expr` lifetime registry
  writes as structured statements/items. The parser accepts canonical
  `with { ... }`, indentation sugar `with:`, and flat `=== with ===` fences
  over the same model; `spawn` is rejected in favor of `thread`. The current
  checker validates guaranteed and optional lifetime reads at a minimal level
  and reports double-drop/use-after-drop cases for line registry keys.
- Dialogue callee checking preserves the documented callee kind: `alice.say()[...]` resolves through `alice: Ref<Character>`, delimited character refs such as `@<character.alice>.say()[...]` generate the same speaker slug, and speaker presets such as `alice2(voice=auto):` / `alice2(voice=auto)[...]` resolve as callable `SpeakerPreset` values rather than being forced through `.say(...)`. Content-call line plans can attach on a following `with { ... }` / `with:` block or on the same line as `with { ... }` / `with: out ...`; line-result bindings such as `let handles = alice.say()[...] with: out (...)` and multiline `let handles = alice.say(...)[ ... ]` followed by `with:` preserve the plan on `Expr::DialogueCall` for symbol collection, HIR lowering, and type checking.
- Bare scopes parse as `scope { ... }`, the name-omitted sugar for `scope name { ... }`. Bare `{ ... }` blocks in flow bodies normalize one step further to that unnamed `scope { ... }` form. A trailing bare block after a dialogue content call, such as `alice.say()[...] { ... }`, is not attached as a line plan; line plans still require `with { ... }` or `with:`.
- Type checking treats both bare `scope { ... }` blocks and named `scope name { ... }` blocks as local scopes, so temporary locals created inside them cannot be read after the block exits. Named scopes still contribute to generated relative IDs while the lowerer is inside that scope; unnamed scopes do not add an ID path segment.
- `let name = choice ... { ... }` parses as a choice expression binding, lowers to HIR with normalized relative choice/option IDs, and the minimal checker can infer `Ref<Flow>` when every option uses `=> @flow...`.
- Dynamic choice options now type-check their scoped fields in place: `option route in opening_routes(state) { id = route.choice_id; label = route.label; enabled = route.enabled; select { out route.target } }` binds `route` for the option body, validates boolean option state, checks label text keys, and keeps `select`/`out` expressions in the correct local scope. Compact choice arms require static option IDs; dynamic leading expressions such as `route.choice_id "..." -> ...` are preserved as raw recovery items and rejected before type checking.
- Module and import paths accept `crate::`, `self::`, `super::`, and reserved `parent::` roots as source syntax, normalize parsed `parent::` roots to canonical `super::`, and reject relative `@.suffix` / `@..suffix` ID syntax in `mod`/`use` paths so ID-relative notation stays limited to line, text-key, choice, and option contexts.
- The documentation from `docs/reviews/pro_review4.md` is reflected in the language specs: ordinary `{ ... }` blocks remain value-producing in expression position; `scope name { ... }` is both lexical scope and ID namespace; `scope { ... }` is the bare scope sugar with the name omitted; relative IDs are limited to line, text-key, choice, and option contexts; module-relative paths use `self::`, `super::`, or `crate::`; and `parent::` is a reserved alias that formatter/canonicalizer work should normalize to `super::`.
- `await ... with` keeps `pending`/`ready`/`error`/`denied` branches as structured AST/HIR, and branch bodies participate in symbol collection and type checking.
- Bound wait-view expressions such as `let assets = try await load_opening_assets() with { ... }` and `let result = await load_opening_assets() with:` lower to explicit await-binding HIR. The minimal checker validates the awaited expression as `Need<T, E>`, scopes wait-view branch patterns, and binds the outer pattern as `T` for `try await` / `await?` or `Result<T, E>` for plain `await`.
- Bound wait-view parsing accepts documented multi-line context chains before `with:`, such as `let bg = try await asset.image(...)\n    .context(...)\nwith:`, while plain `let bg = try await load_bg()` without a wait-view remains a normal await expression binding.
- Try-line syntax now type-checks the documented `try alice.say(...)[ ... ] with:` shape when line-plan branches return `Ok(...)` / `Err(...)` through `out`. The minimal checker treats those constructors as `Result` values, merges placeholder `Ok`/`Err` result sides, unwraps the `try` expression to the success type, and treats `()` as `Unit`.
- Parenthesized await-with forms documented for generated or composed code, including `let bg = (await asset.image(...) with: ...)?` and `let bg = (await asset.image(...) with: ...).context(...)?`, lower to explicit await-binding HIR with `applies_try`.
- Wait-view branch patterns include structured variant payloads, so documented activity forms such as `pending .Realizing(p) => ... p.ratio` bind the payload inside only that branch.
- The minimal checker accepts dotted member access on scoped locals in wait-view bodies, so documented forms such as `pending p => ... progress.set(p.ratio)` validate without requiring `p.ratio` to be registered as a global symbol.
- Background task-style `await expr`, `try await expr`, and `await? expr` without a wait-view block parse as structured expression AST. The minimal checker requires the awaited expression to have `Need<T, E>` type, returns `Result<T, E>` for plain `await`, and unwraps to `T` for `try await` / `await?`.
- Ordinary Rust-like propagation syntax is represented in expression AST. `expr?` and prefix `try expr` parse as structured try expressions, participate in symbol collection, and the minimal checker unwraps `Result<T, E>`-like types while rejecting non-result expressions.
- Flow/function contract clauses (`requires`, `ensures`, `invariant`, `assume`, `reads`, `effects`, `no_effect`, `modifies`, `decreases`) are parsed separately from the body and participate in symbol collection and type checking where applicable.
- `lower_to_hir` verifies that parsed edge-case flow syntax can be converted to HIR-facing structures and rejects raw flow recovery nodes that still need parser coverage. Flow, statement, choice, choice-plan, and line-plan recovery nodes preserve source text, grammar family, and source span metadata through typed syntax so diagnostics, verifier obligations, and LSP actions do not need to re-scan raw source.
- `collect_symbol_uses` walks HIR without reparsing source snippets so name resolution can see dialogue callees, entity references, paths, calls, methods, dialogue text expressions, timed cues, and choice-condition references.
- `registry_from_hir` and `validate_hir_references` provide minimal name resolution over HIR declarations and entity references.
- `validate_typecheck_ready` rejects lowered HIR that still contains raw expression fragments before the future type checker sees it.
- `typecheck_hir` provides a minimal semantic checker over HIR with an explicit environment. It validates flow/fragment entity reference families, dialogue callees, `Need<T, E>` awaits, `Duration` timeline anchors, indexed expressions, calls, and methods for parser/HIR integration tests.
- Presentation calls are parsed and checked through the same AST/HIR/typecheck
  path. `bg(...)`, `show(...)`, `ref bg(...)`, `ref show(...)`,
  `clear bg(...)`, and `hide(...)` are recognized as presentation calls; the
  checker validates `@target.*`, family-correct `@slot.background.*` /
  `@slot.character.*`, typed handle/ref/clear return shapes, and duplicate
  default-slot handles in one lexical scope.
- `lint_id_policy` provides the first syntax-level ID policy pass. It reports
  accepted-but-discouraged deep dot-run relative IDs such as `@...suffix` and
  obvious module/flow tail mismatches.
- `analyze_semantics` in `arcweft-lang-sema` produces the first structured
  `SemanticReport`. The pass spans HIR plus syntax statements and now carries
  path-sensitive `FlowFacts` through blocks, branches, line plans, cancellation
  rules, fixed-point loop heads, and thread bodies. Lifetime promotion,
  audited unsafe regions, upper-lifetime writes, effect capability writes,
  MustDrop discharge for line focus handles, thread capture, thread join
  result-shape conflicts, raw syntax, trusted assumptions, and sibling thread /
  line child task write conflicts are surfaced as typed obligations for
  verifier, CLI, and LSP tooling. Proof bodies are structured into typed
  clauses before semantic analysis, including checked targets, assumptions, and
  trusted axiom references. Scoped `defer` is applied by outcome, so
  completed-only cleanup does not discharge cancellation paths.
- Typed let patterns and borrow blocks preserve borrow types, and the checker
  rejects non-`'static` borrowed values crossing `await`, `yield`, `thread`,
  and `defer` suspension boundaries. Direct explicit local drop statements now
  end the tracked borrow before those boundaries for `drop(local)`,
  `drop_optional(local)`, `on_drop(local)`, and local `.drop()` calls.
  Branch merges preserve one-sided drops as maybe-dropped so they cannot cross
  suspension boundaries or be reused.
- Parser/HIR/sema integration tests live under `crates/arcweft-lang-sema/src/tests/`
  while syntax crate unit coverage stays with `arcweft-lang-syntax`; this keeps
  the public crate surfaces separate from broad grammar and semantic coverage.

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
- full VM lowering/execution for line task groups and adapter-side effect
  requests beyond the current Sans I/O data model
- full flow-fiber lowering, `await with` execution IR, choice runtime IR,
  full stream operator execution, hook/memo runtime tables,
  save/replay trace writers, activities, and layered input
  routing
- full generic substitution and effect-aware return checking
- full type environment, name resolution, and type checking
- full completion of the `pro_review21.md` file split plan: `arcweft-lang-sema`
  still needs deeper expression call/control-helper extraction after the
  expression dispatch/value-helper split; syntax still needs larger AST/parser
  family splits beyond the initial proof/source AST modules.
- inference, overload resolution, traits, generics, contracts, and full
  type-directed effect checking
- unbounded/solver-backed loop CFG and full nested-scope borrow lifetime
  analysis beyond the current region escape and explicit-drop tracking
- full solver-backed proof term checking beyond current structured proof-body
  target, assumption, and axiom validation
- full semantic expression resolution and type-directed ambiguity resolution
- full choice expression type unification beyond the current `=> @flow...` case, lifecycle runtime execution, reactive option-state reevaluation, localization extraction, formatter/canonicalizer output, and LSP diagnostics for dynamic labels and unordered map-backed options
- full localization extraction manifests and formatter/canonicalizer normalization for relative `.suffix` IDs
- full migration of typed syntax construction from the current private
  line-event parser builder to grammar-level CST/event parsing

## Verification

Last verified during the Phase 2.0 headless runtime slice:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
cargo tree -p arcweft-core --edges normal
```

Result:

- `cargo fmt --all --check`: passed
- `cargo clippy --workspace --all-targets --all-features`: passed
- `cargo test --workspace`: passed
- `cargo tree -p arcweft-core --edges normal`: passed; no renderer/audio/device
  dependency entered core.
- `cargo tree -p arcweft-lang-syntax --edges normal`: passed; syntax remains on
  rowan, blake3, thiserror, and `arcweft-source`.

