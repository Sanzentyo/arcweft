# Implementation Status

This directory records the current implementation state of Arcweft Engine.

Design specifications remain in the numbered `docs/` chapters. Files here describe what exists in the Rust workspace today, what has been verified, and what is intentionally deferred.

## Current Milestone

Phase 0 / Phase 1 minimal Rust workspace:

- Cargo workspace skeleton.
- Foundational ID, source anchor, Need, and dialogue surface model crates.
- Syntax and CLI crates with Phase 1 parser/HIR/check surfaces and the
  `arcw check <file.arcw>` developer entry point.
- Language responsibilities are now split across `arcweft-lang-syntax`
  (lossless CST, surface AST, parser, syntax lint), `arcweft-lang-hir`
  (HIR types and lowering), `arcweft-lang-sema` (name/symbol/type readiness and
  minimal type checking), and `arcweft-runtime-plan` (HIR to Sans I/O runtime
  plan lowering).
- Entry declarations now parse, lower through HIR, materialize into
  `RuntimePlan.entries`, and can be selected by `arcw run --entry`; `--flow`
  remains available for direct flow selection. When no entry is provided,
  runtime lowering keeps the first flow as the deterministic fallback for
  current headless fixtures.
- `extern capability` declarations parse and lower as structured HIR
  declarations. Capability functions are registered for type checking, their
  declared `effects { ... }` are enforced against the active flow/function
  effect scope, and filesystem capability calls reject direct OS absolute path
  string literals in favor of `VirtualPath` constructors.
- `arcweft-core` no longer depends on dialogue or presentation; the facade
  crate `arcweft` exposes crate-family namespaces instead of a flat prelude.
- Awaited capability calls now carry typed `HostTaskRequest` data through
  `AwaitTarget` into emitted `TaskSpec`s. The core remains Sans I/O; adapters
  consume the request data and later return deterministic `TaskEvent`s.
- `arcw serve --listen` owns a minimal native HTTP adapter in the CLI layer. It
  consumes lowered server route plans and executes matched flows through
  `RuntimeStepMode::Server`; `arcweft-core` remains free of network I/O.
- `arcw check`, `arcw verify`, `arcw unsafe`, and plan/report generation now
  pass the resolved profile adapter `TypeCheckEnv` through both type checking
  and semantic verification. Generic direct-path mode still uses the empty
  Sans I/O environment.
- `AotExecutor` exists as a VM-equivalent `RuntimeExecutor` boundary. Generated
  dispatch remains future work, but tooling can now target the AOT executor
  shape without introducing different semantics.
- `arcweft-core::pure` exposes the pure-helper backend contract used by future
  AOT/JIT adapters. `VmPureFunctionBackend` is the semantic reference,
  candidate backends report deterministic evaluation stats, and conformance
  checks compare candidate output against VM output without recording host
  absolute paths.
- No renderer, Servo, audio, camera, USB, MCP, or Cranelift JIT implementation.

## Files

- `phase-0-1-workspace.md`: current crate layout, public types, verification status, and deferred work.
- `refactor-checklist.md`: direction-package checklist for the runtime boundary,
  entry/capability grammar, RuntimeStep, executor, and fixture-driven gates.

## Verification Snapshot

Last verified for the active workspace after enabling the spec fixture gates:

- `cargo fmt --all`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features`
- `arcw check` over `tests/fixtures/arcw/spec_should_pass/check`
- `arcw run --mode drain --steps 16` over `tests/fixtures/arcw/spec_should_pass/run`
- `arcw check` over `tests/fixtures/arcw/spec_should_fail`, expecting every
  fixture to fail with diagnostics

## Design Reviews Reflected

The implementation notes track accepted syntax decisions from `docs/reviews/` when
they affect parser, HIR, formatter, LSP, or CLI work.

`pro_review21.md` is reflected for the current module-boundary scope, with
explicit evidence tracked in `phase-0-1-workspace.md` under
"pro_review21 Prompt-to-Artifact Checklist".

Current high-confidence state:

- Done: core split + tests, sema public split, syntax AST split, HIR split,
  syntax parser family split, runtime-plan split, dependency cleanup
  (`runtime-plan -> hir`, duplicate `arcweft-test` dependency removal, and
  `arcweft-dialogue -> arcweft-presentation` cleanup), and adapter-frame view
  lifetime APIs.

- `pro_review4.md`: adopted value-producing `{ ... }` blocks, `scope name { ... }`
  blocks for relative ID namespaces, unnamed `scope { ... }` as name-omitted
  sugar, relative IDs only in ID-bearing contexts,
  `self::` / `super::` / `crate::` module-path roots, reserved `parent::`
  normalization, and explicit sugar expansion for `with:`, speaker colon lines,
  speaker-preset calls, and `await?`.
- `pro_review5.md`: adopted structured function signatures with generic params,
  curried parameter groups and `where` clauses; structured hook headers
  (`when`, `priority`, `once`, `effects`); structured dialogue line options; and
  a parsed `dialogue defaults` top-level declaration.
- `pro_review7.md`: adopted rowan-compatible lossless CST as the public parsing
  foundation for `arcweft-lang-syntax`, with `ParsedSource` returning syntax,
  typed syntax views, diagnostics, source text metadata, and line index even for
  malformed files. The typed syntax view is still produced by the existing
  parser builder and should be migrated onto CST/event parsing next.
- `pro_review8.md`: accepted VM / Typed IR as the semantic source of truth.
  Native Cranelift JIT is a pure-function optimization tier in
  `arcweft-lang-jit-cranelift`; Wasmtime is only a native plugin/activity
  sandbox; web uses an AOT compiled Wasm player plus bytecode bundle. Data
  formats, manifests, bundles, schemas, bytecode, and save snapshots must remain
  Sans I/O.
- `pro_review9.md`: adopted `@...` entity references, Rust-like `#[...]`
  attributes, ordinary effectful calls instead of `@` scenario commands,
  color-as-string typing, explicit primitive numeric widths, typed unit-number
  literals such as `100pt`, `2.0f32`, `10i32`, and angle units including `rad`.
  Relative IDs are unified on `@.suffix`, parent-dot forms such as
  `@..suffix` / `@...suffix`, and explicit `@super...` forms; bare `.suffix`
  is not part of the core grammar.
- `pro_review11.md`: adopted canonical dialogue `look` line options, extended
  `stage` / `portrait` / `focus` / `cleanup` line options, `[mark .name]`
  zero-width dialogue markers, line-plan `on mark(.name):` handlers, generic
  line-scoped `thread` blocks, scoped `defer { ... }`, outcome-guarded
  `defer on completed|cancelled|failed`, flat `=== ... ===` fence sugar,
  `wait(mark(...))` / `wait(duration)` waits, and `'line.*`
  lifetime registry paths with optional `?` reads. Local dialogue `[hook ...]`
  and `#[hook ...]` syntax is removed; top-level engine hooks remain.

## Current Direction

- Parser work now starts from a lossless rowan CST: `SyntaxKind`,
  `ArcweftLanguage`, `SyntaxNode`, source text retention, line index, source
  hash, and always-returning `ParsedSource`.
- The typed parser now receives `CstLineEvents` projected from CST `Line`
  nodes through `From<&SyntaxNode>` instead of splitting raw source
  independently. Each projected line carries a `CstLineKind` classification for
  blank/comment/doc/code handling, and top-level dispatch now starts from
  `CstTopLevelLineKind` / `CstTopLevelItemKind` event classifications owned by
  the CST layer instead of an open-ended parser string chain. This keeps
  declaration detection distinct from AST construction while the grammar moves
  toward rowan events. Flow-body dispatch now likewise starts from CST-owned
  `CstFlowItemKind`, `CstStructuredFlowBlockKind`, and `CstLetFlowItemKind`
  classifications so the typed parser receives a syntax-family event before it
  calls the existing AST builders. Shared balanced
  scans for delimiters, top-level punctuation, top-level keywords, top-level
  whitespace, leading identifiers, lifetimes, entity refs, relative IDs, and
  matching punctuation live in the CST layer so expression, type, pattern, and
  top-level parsing do not grow separate ad hoc splitters. Current line-event
  parsing uses those CST helpers for multiline delimiter recovery, `let`/type
  binding splits, associated-type generic heads, pattern guard splits,
  multi-token separators such as `=>` / `->` / `<-` / `::`, `borrow ... as ...`,
  await grouping, await `with` heads, extern module headers, event fields,
  scenario command args, labels, entity refs, and shared pattern/type delimiter
  parsing.
- Balanced brace-block collection for ordinary blocks and function-body blocks
  now lives on `CstLineEvents` and returns a `CstBlockEvent`. The typed parser
  still consumes the result, but brace recovery and body-open detection are no
  longer duplicated in parser methods.
- Flow-like block collection also lives on `CstLineEvents`. It keeps contract
  and `effects { ... }` prelude lines in the header while collecting the
  following brace body as the block event, so flow/callable/entity/source
  builders no longer own header-prelude scanning.
- Parser-facing grammar delimiter decisions have been moved out of the typed
  parser's local string scans and into CST helpers. The remaining raw
  character scans live in the CST lexer / CST text utilities themselves, where
  they tokenize source text or implement named text utilities such as line
  splitting, documentation-prefix extraction, wiki-link extraction, and
  string-literal extraction. Future grammar behavior should continue to enter
  through CST helpers or grammar-level rowan events rather than parser module
  scans.
- Parser recovery for flow items, choice-body items, choice-plan items, and
  line-plan items now uses a typed `RawSyntax` recovery node with grammar
  family and source span metadata. Statement parsing also enters through a
  CST-owned `CstStmtKind` classifier, and remaining unsupported statements use
  `RawSyntaxFamily::Stmt` instead of opaque strings. These nodes are
  diagnostics carriers only: HIR lowering rejects raw flow recovery nodes, and
  semantic/verifier/runtime-plan passes report raw recovery as typed
  obligations instead of treating it as executable syntax.
- CST reference helpers now keep absolute `EntityRef`, ID-context `IdRef`, and
  family-relative `EntityRefSyntax` separate. `@.suffix`, `@..suffix`,
  `@...suffix`, `@super...`, and ID-context family forms such as
  `@say:.suffix` / `@choice:.suffix` are accepted in ID-bearing contexts;
  general relative references use family-qualified forms such as `@flow:.next`
  and `@textbox:.side`. HIR lowering normalizes these structured nodes against
  the current flow, speaker, choice, and named-scope stack.
- Old `@` command and attribute spellings are no longer treated as migration
  syntax. Attributes are `#[...]`; staging operations use canonical ordinary
  calls such as `bg(@asset.bg.room, fade = 300ms)` and
  `show(@character.alice, .normal)`.
- `arcweft-dialogue` contains the current Sans I/O model for scoped
  dialogue lines, speaker presets, content, and line plans. Presentation
  staging helpers live in `arcweft-presentation`; `arcweft-dialogue` no longer
  depends on the presentation crate. Compatibility type aliases such as
  `DialogueOptions` and `VoiceRef` have been removed; Rust callers use the
  canonical `SayOptions` and `VoicePolicy` names directly.
- `arcweft-presentation` contains the Sans I/O model for scoped presentation
  handles. `bg(...)` and `show(...)` return typed
  `PresentationHandle<T>` values registered against a `PresentationTarget`,
  `PresentationSlot`, and `PresentationScope`; slots behave like typed
  static-option cells and expose read-only `SlotRef<T>` plus clear operations.
  `PresentationRegistry<T>` enforces scope lifetime at the data-model level by
  clearing registered slots when `exit_scope` is called.
- `arcweft-lang-syntax` now recognizes presentation set/read/clear calls as
  type-checkable ordinary call syntax: `bg(...)`, `show(...)`,
  `bg.ref(...)`, `show.ref(...)`, `bg.clear(...)`, and `hide(...)`. The checker validates
  `@target.*`, family-correct `@slot.background.*` /
  `@slot.character.*` usage, and reports simultaneous default slot handles
  that should be given explicit slots.
- Runtime observation APIs are ordinary call syntax. `log.info(...)`,
  `log.debug(...)`, `log.warn(...)`, `signal.set(target, value)`,
  `metric.set(target, value)`, and `event.emit(Event, fields)` parse as normal
  method calls; line-plan runtime lowering recognizes those well-known calls
  and emits typed Sans I/O `LineEffectRequest::Log`, `SignalWrite`,
  `MetricWrite`, and `EmitEvent` records.
- Dialogue syntax now parses `look`, `stage`, `portrait`, `focus`, and
  `cleanup` as first-class line options. The first positional line option maps
  to `look`; `face` is rejected as a line option while stage methods such as
  `alice.stage.look(...)` remain ordinary calls.
- Dialogue text now tokenizes `[mark .name]` into a structured marker token.
  The checker rejects duplicate marks, rejects local `[hook ...]`, and verifies
  marker-triggered line-plan `on mark(.name):` handlers against marks in the same
  line.
- Line plans now preserve `init`, generic `thread name` blocks, scoped
  `defer { ... }`, outcome-guarded `defer on completed|cancelled|failed`, `on`
  handlers, `wait` statements, and `'line.* <- expr` lifetime registry writes
  as structured AST/HIR-checkable syntax. Line cleanup now uses `defer` rather
  than a separate cleanup construct; `with:`, `with { ... }`, and flat
  `=== with ===` fences are sugar over the same line-plan model. `spawn` is
  rejected in favor of `thread`.
- Syntax-level ID policy linting exists as `lint_id_policy`. It currently
  reports deep dot-run relative IDs such as `@...suffix` and flow IDs whose
  tail does not match the module tail. Further hierarchy checks should build on
  this pass rather than parser diagnostics.
- `pro_review12.md` P0-P2 work is partially implemented: syntax/checking now
  uses structured `LifetimeScopeKind`/`LifetimeKey`, recognizes upper-lifetime
  write capabilities such as `state.write(flow)`, and accepts source effects
  selectors such as `effects { state.write('flow) }` as capability facts.
  It rejects `'line.*` outside line scope and across thread boundaries, parses
  expression-form `thread`, keeps function parameter defaults, supports `&`
  patch merge parsing/checking, and recognizes surface aliases plus
  voice/se/bgm/bus/mix/ducking/motion/rig entity families.
- `pro_review13.md`: adopted Phase 1.5 as the next execution direction. The
  CLI now provides `arcw check <file.arcw> [--json]` and runs parse, HIR lowering,
  reference validation, ID policy lints, typecheck readiness, minimal typecheck,
  and line-plan runtime lowering. `arcweft-runtime-plan::line_task` exposes
  `lower_line_task_groups`, which converts checked dialogue line plans into
  `arcweft-core::line_task::LineTaskGroup` values without renderer/audio/device backends.
  Scoped `defer` lowers as cleanup on the current runtime scope rather than as
  thread-only syntax.
- Phase 1.5 line-plan lowering now preserves line options, line-local `let`
  bindings, `out`, `cancel on`, memo directives, assertions, structured logs,
  signal writes, metric writes, `event.emit(...)` calls, scenario commands, and
  ordinary calls as typed runtime IR categories rather than dropping them or
  collapsing them into a stringly signal placeholder.
- `pro_review16.md`: line-plan runtime data now uses a structured
  `LineTaskScope` / `LineTaskNode` graph instead of flat `init` and `children`
  vectors. `thread`, `on`, and `at` lower to child tasks with stable task IDs,
  task keys, triggers, priority, join policy, and cancel policy. `start` and
  `together` preserve their graph boundaries, and `together` runs an initial
  deterministic access-conflict check for signal/lifetime/control/output writes
  while allowing append-only logs and events. Handler and child-task typecheck
  scopes are isolated so locals, line guarantees, and dropped-lifetime state do
  not leak across task or line boundaries.
- `arcweft-core` now exposes initial Sans I/O task/source event envelopes:
  `TaskSpec`, `HostTaskRequest`, `TaskEvent`, `TaskHost`,
  `normalize_task_events`, `SourcePolicy`, `BackpressurePolicy`,
  `ReplayPolicy`, and `SourceEvent`. `TaskSpec` carries a typed host request
  plus a diagnostics-only `debug_label`; file, HTTP, process, asset, shader,
  audio, TTS, Wasm, and custom capability requests are pure data contracts for
  host adapters. No Tokio/Rayon/filesystem, device, audio, or GPU runtime is
  linked into core.
- Phase 2.0 structured headless runtime work is implemented in `arcweft-core`:
  `RuntimePlan`, `RuntimeFlow`, `FlowOp`, `RuntimeValue`, `RuntimeExpr`,
  `RuntimePattern`, `RuntimeEnv`, `Engine`, `FlowFiber`, and
  `run_line_task_group` can step lowered flow/dialogue task graphs over
  `RuntimeStepInput` into `RuntimeStepOutput` without performing I/O. The spine emits
  child/await `TaskSpec`s and deterministic line effects, evaluates
  let/let-else, if/if-let, match, loop/while/while-let/for, scope, goto, and
  return runtime nodes, runs scope cleanup stacks, and leaves actual
  native/cooperative/web execution to adapters.
- `arcweft-runtime-plan::flow` now exposes `lower_runtime_plan`, which converts
  checked HIR flows to core `RuntimePlan` data for the Phase 2.0 execution
  slice. Runtime lowering supports dialogue, `choice`, `await with`, typed
  `let`, `let else`, structured `if`, `if let`, `match`, `loop`, `while`,
  `while let`, `for`, `scope`, dynamic `goto`, dynamic `return`, flow-level
  ordinary effects, `out`, and line `cancel on` rules. Unsupported executable
  flow items fail lowering explicitly instead of being converted to `Noop`.
- `arcw plan <file.arcw> [--json]` now exposes lowered line task graph metadata
  for CLI, LSP, and Agent inspection. Runtime parallel conflicts are also
  surfaced as verifier obligations so direct verifier users can see the same
  class of graph conflict as `arcw check`.
- `arcw run <file.arcw> [--steps N] [--mode one-op|drain|game|server] [--max-ops N] [--value name=value] [--json]` now
  performs a deterministic dry run through the Phase 2.0 headless runtime slice and
  reports per-step flow events, effects, host requests, diagnostics, stop reason,
  and final fiber status. `--value` injects pure runtime bindings such as
  `ready=true`, `count=3`, or `route=@flow.next`; the CLI owns filesystem I/O
  and runtime execution remains Sans I/O.
- Runtime stepping now uses the shared `RuntimeExecutor` trait. `VmExecutor`
  wraps the semantic `Engine` implementation used by CLI and tests, and
  `Engine::step` enforces `RuntimeStepMode::{OneOp, Drain, Game, Server}` plus
  `RuntimeStepBudget::max_ops` inside the VM loop. `Game` mode returns on
  presentation-visible output while pure observations can drain to a harder
  boundary.
- `arcweft-core::bytecode` provides a pure `BytecodeProgram` bundle and
  deterministic bytecode stats. `BytecodeVmExecutor` executes that bundle through
  the semantic VM so VM, bytecode, AOT, and future JIT tiers have a shared
  conformance boundary before generated dispatch is introduced.
- CLI runtime stepping now routes through `BytecodeVmExecutor`, and
  `arcw profile --json` includes bytecode lowering time plus deterministic
  bytecode flow/instruction/source/stream counters.
- Phase 2.0 headless observation state is implemented for the current runtime
  slice. `arcweft-core` records
  cumulative log, signal, metric, and event observations from emitted
  `LineEffectRequest` values without performing host I/O, and
  `arcw run --json` exposes those observations for CLI, test, LSP, replay, and
  Agent tooling.
- Source and stream runtime execution now has a first Sans I/O slice.
  `RuntimeStepInput.source_events` are normalized, dispatched through lowered
  `SourcePlan` handlers, and `yield` pushes structured `RuntimePayload` items
  through the declared
  backpressure policy. `StreamPlan` can drain source/stream queues through
  `ForNext` and emit deterministic stream events within a per-step budget.
  `arcw plan --json` reports generation plans, and `arcw run --json` reports
  source/stream events and queue state. CLI output renders payload labels for
  display while the Sans I/O boundary keeps `RuntimeValue` shape for replay and
  downstream runtime consumers. Device acquisition, permissions, and native
  callbacks remain adapter responsibilities.
- The Phase 2.0 runtime now has an explicit `FlowFiber` control stack for lexical
  scopes and loop continuations. `break` and `continue` discard queued body ops,
  pop body-local scopes, and transfer to the nearest loop/while/while-let entry.
  Branch, match, and while-let pattern bindings are scoped to the selected body;
  guard evaluation uses temporary bindings and restores the previous runtime
  environment. `RuntimeStepInput::bindings` bind into the root runtime scope so
  ambient per-step values are not lost when a nested scope exits.
- Gap audit result: broad runtime docs still exceed the Phase 2.0 headless
  target. Full story VM value execution, complete expression evaluation, source
  adapter execution, hook/memo runtime tables, save/replay traces, activities,
  layered input routing, and full stream operators remain TODOs beyond the
  current Sans I/O runtime slice. The implemented slice executes
  `let name = scope { ... }` value bindings and `let name = loop { break expr }`
  result binding in the headless runtime.
- `pro_review14.md` / `pro_review15.md`: adopted proof-aware
  lifetime/thread/drop direction and Agent-friendly tooling diagnostics.
  Formal `proof @proof.*` items, `trusted axiom @axiom.*` declarations,
  explicit proof references such as `proof = @proof.id`, and audited
  `unsafe lifetime @unsafe.*` regions with required `reason` and `SAFETY`
  documentation are the accepted design. The syntax crate preserves proof and
  trusted-axiom items as HIR metadata and parses `unsafe lifetime` audit blocks
  as structured statements. `arcweft-lang-hir` is now the public HIR facade.
  `arcweft-lang-sema` now owns the first `SemanticReport` pass for CFG-aware
  lifetime/drop/thread/write analysis. The pass carries path-sensitive
  `FlowFacts`, applies `defer` cleanup by completed/cancelled/failed outcome,
  runs bounded fixed-point loop analysis for `break`/`continue`, checks proof
  references against the promoted lifetime target, validates that unsafe audit
  blocks contain the unchecked promotion they justify, and prefers
  semantic-owned obligations over the older verifier scan.
  `arcweft-verify` merges that report with shared JSON diagnostics for lifetime
  promotion, unsafe audits, upper-lifetime writes, effect capabilities, thread
  capture, thread join typing, MustDrop discharge, trusted assumptions, raw
  syntax, and simple runtime write conflicts. Solver dependencies are isolated in
  `arcweft-verify-z3` and `arcweft-verify-oxiz`; CLI/LSP consume verifier
  reports rather than reimplementing validation.
- `Char` / `TextCluster` are now part of the accepted primitive model. `Char`
  is a Unicode scalar value and is not a visual character; `TextCluster` is the
  display/reveal/ruby/effect unit. The syntax crate parses `"x"c` char
  literals and typechecks `Char` separately from `String`.
- Capacity traits are accepted for owning collections: `WithCapacity` and
  `Reservable` expose `with_capacity`, `reserve`, `shrink`, and `shrink_to`.
  Capacity is non-observable and may be a no-op on constrained/Wasm targets.
  The syntax checker recognizes these methods for `Vec<T>`, `String`, and
  `Bytes`.
- Top-level `test @test.* KIND { ... }` and `bench @bench.* { ... }` are now
  parsed as structured declarations and lowered into HIR metadata. The
  `arcweft-test` crate extracts a Sans I/O manifest. `arcw test` now executes
  `scenario` declarations through the headless runtime when they contain
  `start @flow.id`, evaluates initial signal/log/no-assertion expectations, and
  reports pass/fail/skipped JSON. `arcw bench` now validates headless bench
  plans, requires `measure`, accepts `setup`/`measure`/`assert`/`report`
  sections, measures `measure` bodies that name `start @flow...`, and reports
  measured/validated/skipped/failed JSON. Visual, audio, fixture, and allocation
  execution remain player/headless adapter responsibilities.
- `RuntimeStepResult` now carries deterministic `RuntimeStepStats` for executed
  VM ops, pending queue depth, incoming task/source events, emitted source/stream
  events, line effects, and diagnostics. `arcw profile --json` reports compiler
  phase timings plus those VM counters without recording absolute local source
  paths.
- `arcweft-lang-sema` now exposes `TypeCheckReport` / `TypeCheckStats`.
  `arcw profile --json` surfaces deterministic typecheck counters and integrated
  borrow-check counters, including expression/statement counts, borrow binding
  groups, borrow state snapshots/restores/merges, boundary checks, escape
  checks, and maximum active borrows.
- Declaration ID positions whose family is known now accept current-scope and
  family-relative IDs. `flow @.opening`, `flow @flow:.opening`, and bare
  `flow opening` normalize to `flow.opening`; declarations such as
  `character @.alice`, `hook @.visible`, `source @source:.events`, and
  `dialogue defaults @dialogue:.opening` follow the same rule. Empty declaration
  markers are accepted when a declaration name follows them: `flow @. opening`,
  `flow @flow:. opening`, `character @. alice Alice`, `signal @signal:. ready`,
  and `source @source:. metrics()` normalize through that following name.
- Remaining P2 semantic work is now refinement rather than missing surface
  coverage: fixed-point loop analysis is bounded and syntactic, proof discharge
  is target-aware and checks structured proof-body `ensures`/`check` targets,
  unjustified `assume` clauses, and unknown trusted axiom references; unsafe
  audits validate shape but not memory
  semantics, and thread result inference is based on current syntactic result
  labels. Effect capabilities are now represented as typed semantic facts:
  `effects { signal.write, metric.write }` on flows/functions and hook header
  `effects` entries grant the corresponding known write calls such as
  `signal.set` and `metric.set`. Semantic conflict checks now use typed
  resource access facts for lifetime/signal/metric writes. Ownership/region
  checking rejects borrowed values escaping through block finals, returns,
  line-plan `out`, or upper-lifetime registry writes. Direct explicit
  `drop(local)`, `drop_optional(local)`, `on_drop(local)`, and local `.drop()`
  statements end the tracked local borrow before suspension boundaries such as
  `await`; branch merges keep one-sided drops as maybe-dropped so they remain
  rejected before suspension or reuse. Full solver-backed proof term checking
  and type-directed effect inference remain beyond the current Phase 2.0
  semantic/verifier slice.
- Verifier JSON uses a stable adjacent-tagged representation for proof
  expressions, including string-carrying variants such as
  `{ "kind": "var", "value": "signal.write" }`.
- Phase 2.1 tooling has a first Sans I/O crate, `arcweft-tooling`, for source
  edit reports, sugar expansion, ID materialization edits, source code actions,
  and inferred-ID hints. The CLI now wires `arcw fmt` and
  `arcw ids materialize` as dry-run-by-default adapter commands with `--write`
  and `--json`; `arcweft-verify-lsp` exposes the same source actions and hints
  without owning an LSP transport. The current ID materialization table covers
  top-level declarations, explicit and omitted dialogue line `id=` /
  `text_key=` options, flat `=== line ... ===` dialogue heads, and
  choice/choice-option IDs. Canonical `with { ... }`, `with:`, and flat
  `=== with ===` line-plan attachments share the same materialization context.
- The old `arcweft-tooling` dialogue-ID line scanner has been removed from the
  tooling crate. ID materialization now flows through
  `arcweft-lang-hir::collect_id_context`, which emits typed source operations
  for declarations, choices, choice options, explicit dialogue `id` /
  `text_key` options, and omitted dialogue options. Speaker-preset discovery
  now walks the parsed typed tree instead of source lines. Tooling, CLI, and
  LSP convert typed operations into edits, hints, and actions instead of
  keeping scanner-specific logic.
- `pro_review19.md` is reflected with Rust-like collection names. The facade
  crate exposes minimal Sans I/O standard data crates through explicit
  namespaces rather than a flat compatibility prelude:
  `arcweft-adt` (`Unit`, `Never`, `Vec<T>`, `Array<T,N>`,
  `OrderedMap`/`SortedMap`/`OrderedSet`/`SortedSet`, `SmallList`, state paths, patch/diff/version/log/queue/cache
  types, source/stream descriptors, arena/slot/generational ID structures,
  deterministic tree/graph structures, ring buffers, signal buses, compiler
  node IDs, and rich-text/localization data),
  `arcweft-ref` (`Id<T>`, `Ref<T>`, `Handle<T>`, `WeakHandle<T>`,
  `Borrow`, `Slice`, `Lease`), and `arcweft-memory` (`Bytes`, `Blob`,
  `BlobRef`, `SharedSliceDesc`, `SharedSlice<T>`, `MemoryLease`,
  `PodSlice<T>`), while `arcweft-source` provides `SourceRange`, `SourceSpan`,
  and shared diagnostic bags. The language docs use `Vec<T>` for growable ordered sequences,
  `Array<T,N>` for fixed-length sequences, and `[value; N]` for fixed-length repeat
  literals. Adapter-backed implementations remain outside the Sans I/O prelude
  slice; the exported structures are data contracts only.
- Runtime value lowering is stricter for executable flow plans. Unsupported
  value-position expressions such as ordinary calls now produce runtime-plan
  lowering errors instead of being coerced into string labels; adapter-facing
  payload labels still use the existing lossy labeler where the runtime treats
  them as observational data rather than executable values.
- `pro_review21.md`: module boundaries are being treated as first-class
  architecture boundaries rather than temporary file organization. `arcweft-core`
  is split into public responsibility modules (`time`, `frame`, `value`,
  `pattern`, `effect`, `task`, `source`, `stream`, `plan`, `line_task`,
  `observation`, and `engine`) without root-level compatibility aliases.
  Downstream crates import core types through those module paths. The runtime
  engine implementation is also split by execution responsibility under
  `engine/`: `eval`, `flow`, `line`, `source`, `stream`, and `suspend`, while
  `engine.rs` owns only the engine state types, construction, frame stepping,
  and shared diagnostics/observation plumbing. The
  `arcweft-lang-sema` split now has public `check`, `checker`, `types`, `env`,
  `diagnostics`, `borrow`, and `lifetime` modules, and the checker body has
  started language-family child modules for `choice`, `effects`, `expr`,
  `flow`, `line_plan`, `presentation`, `source`, `suspension`, and `stmt`,
  plus `lifetime_access` for lifetime registry reads/writes/drops, `module`
  for module/top-level entry checks, and `borrow_state` for borrow binding and
  branch-merge helpers; `helpers` now owns shared
  type/pattern/merge/divergence helper functions used by those checker modules.
  `checker.rs` is reduced to checker state, public entrypoints, and a small
  set of shared local helpers.
  Semantic traversal and flow-fact helper families are now isolated under
  `semantic/facts.rs` and `semantic/traversal.rs`. Additional checker-family
  splits remain tracked work.
  `arcweft-runtime-plan` is split into `errors`,
  `expr`, `flow`, `labels`, `line_task`, `pattern`, `source`, and `stream`
  modules for lowering diagnostics, runtime expression/effect lowering, flow
  and whole-runtime-plan lowering, shared textual label helpers, lowered
  line-task metadata and line-plan graph lowering, runtime pattern lowering,
  source declaration lowering, and stream-function lowering. The crate root is
  now only a public module namespace.
- `arcweft-lang-hir` now exposes responsibility modules instead of flat
  compatibility exports: public consumers import HIR data through `model`,
  lowering through `lower`, ID-context tooling through `id_context`, and syntax
  ownership through the namespaced `syntax` module. The lowering implementation
  is split into public responsibility namespaces `lower_flow`,
  `lower_dialogue`, `lower_choice`, `lower_ids`, and `lower_context`.
- `arcweft-lang-syntax` has started the AST family split requested by
  `pro_review21.md`: top-level tree/item/recovery wrappers live in
  `ast/items.rs`, shared range/module/use/doc primitives live in
  `ast/common.rs`, entity/reference ID syntax lives in `ast/ids.rs`, structured
  binding syntax lives in `ast/pattern.rs`, flow/control-transfer syntax lives
  in `ast/flow.rs`, dialogue surface syntax lives in `ast/dialogue.rs`,
  line-plan syntax lives in `ast/line_plan.rs`, choice syntax lives in
  `ast/choice.rs`, proof/test/bench declarations live in `ast/proof.rs`, and
  declarative source-stream syntax lives in `ast/source.rs`. `ast.rs` is now a
  public module namespace rather than the owner of AST family definitions or a
  flat compatibility re-export layer.
- `arcweft-lang-syntax` parser splitting has started with `parser/recovery.rs`
  owning `ParseError` and `RecoverySuggestion`, `parser/source.rs` owning
  source-item header/handler/body parsing, `parser/proof.rs` owning
  proof/test item clause parsing, and `parser/line_plan.rs` owning line-plan
  body, trigger, defer, and thread parsing. `parser/choice.rs` owns choice
  top-level blocks, `let choice` bindings, choice item, arm, option block, and
  choice-plan parsing. `parser/items.rs` owns enum/struct/state field parsing
  and trait/impl member parsing. `parser/hooks.rs` owns hook item parsing and
  hook-header diagnostics. These parser
  family modules are public responsibility namespaces; recovery types are
  addressed as `parser::recovery::ParseError` /
  `parser::recovery::RecoverySuggestion` rather than through a flat
  compatibility re-export. `parser/helpers.rs` owns shared parser helpers for
  module/use path handling and attribute parsing, `parser/source.rs` owns source
  item header/body parsing, source-locale blocks, source handlers, and source
  statement helpers, `parser/top_level.rs` owns module/use/item-family dispatch,
  `parser/flow.rs` owns flow item, flow-body, scope/thread/defer, and
  bare-scope dispatch, `parser/control_flow.rs` owns structured
  flow/control blocks (`if`/`if let`/`match`/`loop`/`while`/`for`/`select`),
  value-producing `let` control-flow expressions, and shared control-flow block
  helpers, `parser/statements.rs` owns statement parsing, `let` statement
  forms, control-transfer statements, unsafe lifetime statement blocks, and
  statement-label parsing, and
  `parser/await_.rs` owns `await ... with` parsing (await `let` bindings,
  multiline await heads, and await-branch parsing), while
  `parser/dialogue.rs` owns dialogue defaults, dialogue-content calls,
  speaker-line sugar, trailing line-plan attachment, and flat dialogue/with
  fence handling. `parser/items.rs` now owns parser methods for function-like,
  enum, struct, state, trait, impl, and type-alias top-level items in addition
  to entity declarations, extern modules, memo functions, parser items, and
  item-member helpers.
  `parser/proof.rs` owns proof, trusted-axiom, test, and bench top-level parser
  methods plus proof/test clause parsing. `parser/headers.rs` owns declaration
  headers, visibility, entity/ID reference parsing, contract clauses, function
  signatures, and related header-level helpers addressed from sibling modules
  as `super::headers::*`. The parser driver still needs further slimming of
  lifecycle/error-plumbing and a small set of cross-cutting helpers, but family
  parsing and statement parsing are no longer owned by `parser.rs`.
- The application-facing `arcweft` facade no longer provides
  `arcweft::prelude::*`. It exposes namespaced crate families such as
  `arcweft::core`, `arcweft::dialogue`, `arcweft::presentation`,
  `arcweft::adt`, `arcweft::need`, and `arcweft::source` so module boundaries
  remain visible to consumers.
- `arcweft-lang-syntax` crate-root exports are module namespaces only. Downstream
  crates now import syntax-owned types through `ast::*`, `expr`, `types`,
  `parser`, `cst`, `lint`, `source`, or `text` instead of flat crate-root
  compatibility re-exports.
- `arcweft-runtime-plan` no longer depends directly on
  `arcweft-lang-syntax`; runtime lowering imports syntax-owned surface types
  through `arcweft-lang-hir::syntax::{ast, expr, types}` so the dependency
  direction remains `runtime-plan -> hir` without a flat HIR syntax prelude.
- `arcweft-core` tests are split by runtime family under `core/src/tests/`:
  frame, task, source, stream, observation, flow, and line-task coverage now
  live in separate files, while the root `tests.rs` only wires modules and
  shared helpers.
- Continue migrating typed AST/HIR/checking APIs into semantic views or lowering
  outputs over the CST instead of extending the current line parser.
- Keep `.awfb`, schemas, manifests, bytecode, and save/debug snapshots as pure
  data models and codecs over bytes/strings. Filesystem, network, path watching,
  embedding, signing, upload, and platform storage live in CLI/build/player
  adapters.
- Use `thiserror` for Rust error types across the workspace while preserving
  structured fields such as `kind`, `range`, `anchor`, and `message`.
- Keep `arcweft-core` free of Cranelift, Wasmtime, filesystem, network, GPU,
  audio, device, and OS dependencies.
- Keep AST, HIR, runtime-plan, schemas, and manifests as owned data models.
  Rust lifetime parameters should stay at adapter/view boundaries unless a local
  crate-internal API clearly benefits; Arcweft lifetime and ownership rules are
  semantic facts checked by `arcweft-lang-sema`, not Rust borrows threaded
  through every intermediate representation.

The stable specification locations for the `pro_review4.md` decisions are:

- `docs/00-overview/decisions.md`: canonicalization and high-level language decisions.
- `docs/00-overview/naming.md`: relative ID naming rules.
- `docs/01-language/block-scopes.md`: value-producing blocks and named/unnamed `scope` blocks.
- `docs/01-language/ids-and-references.md`: `@.suffix`, parent-dot, and `@super...` relative IDs plus module-path roots.
- `docs/01-language/grammar.md`: grammar summary for `scope`, relative IDs, module paths, and await grouping.
- `docs/01-language/scenario-surface-syntax.md`: dialogue, choice, and scenario-facing sugar examples.
- `docs/01-language/modules.md`: `self::`, `super::`, `crate::`, and `parent::` normalization.
- `docs/04-tooling/cli.md`: explicit sugar expansion and ID materialization commands.
- `docs/04-tooling/lsp.md`: sugar expansion and ID materialization code actions.
- `docs/02-runtime/core.md`: VM, effect requests, and data-format Sans I/O boundary.
- `docs/02-runtime/cranelift-jit.md`: native-only pure-function JIT boundary.
- `docs/02-runtime/plugins.md`: WIT/Wasm plugin sandbox boundary.
- `docs/05-build-and-security/native-web-build.md`: native/web runtime target model.
- `docs/05-build-and-security/packaging.md`: Sans I/O bundle format boundary.
- `docs/schemas/README.md`: schemas as data formats rather than I/O APIs.


