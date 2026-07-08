# Functions, closures, currying, and pipeline language stack — 2026-07-07

Source briefs:

- `C:\Users\sanze\.codex\attachments\d352da6f-4ba7-4807-a050-504287f3559f\pasted-text.txt`
- `C:\Users\sanze\.codex\attachments\232a9edf-275f-4c4a-86b9-c447fe38e452\pasted-text.txt`

Current status index:

- `docs/implementation/function-stack-current-status-2026-07-08.md`

## Implemented in the current sequence

- Function type syntax and semantic type substrate:
  - `A -> B`
  - right-associated function types
  - tuple call-group function parameters
- Typed closure parameters in expression and callback-block syntax.
- Closure parameter patterns are preserved through sema: tuple/destructuring
  patterns bind body locals from the parameter type, and pattern `_` remains a
  discard rather than becoming the expression partial placeholder.
- Runtime-plan lowering materializes non-simple closure parameter patterns as
  runtime-only synthetic parameters plus a single-arm `RuntimeExpr::Match`
  body. This keeps `RuntimeExpr::Function`'s stable named-parameter shape while
  making tuple/record/variant closure destructuring executable through the
  existing runtime pattern matcher.
- Closure expressions now type-check as function values instead of returning an
  untyped `None`.
- Curried top-level function/task/dialogue/stream signatures are accepted by
  the parser/HIR/sema surface, and curried `flow` signatures are rejected
  directly.
- Stable language docs now mirror that contract: `FlowDecl` accepts at most one
  `ParamGroup`, while multiple `ParamGroup` entries are documented only for
  function-like declarations.
- Pipe placeholder `^` is scoped to the RHS of `|>`.
- Pipe RHS with `^` substitutes the pipe LHS into the RHS expression before
  type checking and strict runtime lowering.
- Pipe RHS without `^` appends the pipe LHS to RHS call arguments for both sema
  and strict runtime-plan lowering. For example, `2i64 |> add(1i64)` and
  `2i64 |> add(lhs = 1i64)` typecheck as data-last calls rather than as calls
  on the result of `add(...)`.
- Named call RHS forms preserve callable input-name ordering during strict
  runtime-plan lowering. For example, `2i64 |> add(rhs = 1i64)` appends the
  pipe LHS as a synthetic positional input and then uses named-call lowering,
  so the emitted runtime arguments are `[lhs = 2i64, rhs = 1i64]` instead of
  source-order `[1i64, 2i64]`.
- Canonical primitive labels are enforced across sema/runtime-facing surfaces:
  `bool`, `char`, `Unit`, `Never`, and explicit-width numeric primitives use
  the same source labels in diagnostics and LSP-facing type displays; legacy
  `Bool`/`Char` aliases are rejected. Non-canonical primitive spellings such as
  `string` now produce diagnostics pointing to `String` instead of silently
  becoming user-defined nominal types. The advanced `!` spelling parses to the
  same bottom type, but registry/tooling displays keep `Never` as the canonical
  label.
- `_` partial placeholder now works when an expected one-parameter function
  type is available:
  - `let high: i64 -> bool = _ > 80i64`
  - `choices.map(_.label)`
  - repeated `_` in the same abstraction region uses the same generated
    parameter type.
- `Vec`/array/slice/sequence `map` now checks its argument as `item -> _`,
  so explicit closures and `_` placeholder bodies share the same expected-type
  path.
- Strict runtime lowering converts `values.map(_ + 1i64)` into the existing
  executable `RuntimeExpr::Map` form.
- `Vec`/array/slice/sequence `filter` now checks its argument as `item -> bool`.
- Strict runtime lowering converts `choices.filter(_.enabled)` into executable
  `RuntimeExpr::Filter`.
- Core VM pure/runtime expression evaluation executes `RuntimeExpr::Filter`
  over runtime iterators and returns a normalized value sequence.
- Standard prelude-shaped data-last collection pipeline now recognizes
  `choices |> filter(_.enabled) |> map(_.label)` and lowers it through the same
  executable `RuntimeExpr::Filter`/`RuntimeExpr::Map` path.
- No-`^` data-last pipe lowering is helper-aware for named pure helpers:
  `2i64 |> add` typechecks as a pure prefix partial application and lowers to
  function apply when the helper arity is not exact, while
  `2i64 |> add(1i64)` and `2i64 |> add(lhs = 1i64)` can remain exact
  `RuntimeExpr::PureCall` values.
- Runtime function/apply substrate is now typed in the executable runtime:
  - `RuntimeValue::Function` stores parameters, body, and deterministic capture
    bindings.
  - `RuntimeExpr::Function` evaluates explicit runtime closures into captured
    function values.
  - `RuntimeExpr::Apply` applies expression callee values, supports partial
    application when fewer arguments are supplied, and supports curried
    application when an application returns another function value.
- Strict runtime lowering now converts explicit closures into
  `RuntimeExpr::Function`.
- Strict runtime lowering now converts non-path expression callee calls such as
  `make_adder(2i64)(5i64)` into `RuntimeExpr::Apply`.
- Core VM/pure evaluation now executes captured runtime functions, partial
  application, and curried function application.
- Runtime-plan, verify, accelerator, CLI, agent-runner, render-text, and host
  value labels now understand runtime function values instead of relying on
  wildcard handling.
- Runtime-plan expression lowering now carries a pure-helper lookup with both
  IDs and helper bodies. Bare top-level helper paths in value position
  materialize as `RuntimeExpr::Function` when the function lowers through the
  annotated or inferred helper path, and known helper calls with fewer or more
  than the declared helper arity lower through `RuntimeExpr::Apply` rather than
  an invalid exact-arity pure call.
- Sema now accepts prefix partial calls to checked top-level function
  signatures, such as `add(2i64)` and `2i64 |> add`, returning the remaining
  function type. This includes non-annotated top-level `fn` declarations whose
  bodies are lowerable through the existing inferred helper path.
- Missing-input signature partial calls are limited to value-producing
  contexts. Bindings such as `let add_two = add(2i64)` and data-last pipe
  values still infer the remaining function type, while a bare statement such
  as `add(2i64)` reports the missing fixed argument instead of lowering to an
  unused function value.
- Direct top-level signature partial calls now record typed lowering evidence.
  Checked runtime-plan lowering consumes that evidence so helper-backed
  partials lower to runtime functions, while non-helper signatures such as
  unsupported ABI or effectful/suspending top-level functions fail with a
  structured `runtime.plan.lower` diagnostic instead of silently becoming
  adapter-facing incomplete calls.
- Exact-arity known pure helper calls continue to lower to
  `RuntimeExpr::PureCall`, so the existing accelerator/runtime pure call path
  remains available as an optimization.
- Intrinsic non-path callees such as `std.f64.sqrt(...)` are kept as runtime
  calls instead of being mistaken for expression-callee function apply.
- Runtime-plan lowering now turns `_` placeholder abstractions with an explicit
  single-parameter function type annotation into `RuntimeExpr::Function`, for
  example `let high: i64 -> bool = _ > 80i64`. This works for flow lets,
  stream lets, and strict runtime block lets.
- Flow runtime-plan lowering now tracks lexical local bindings that are known
  function values. Calls through those locals, such as
  `let f = add; let add_two = f(2i64); let seven = add_two(5i64)`, lower to
  `RuntimeExpr::Apply` instead of adapter-facing named calls.
- Type-check reports now include stable per-report expression IDs and
  `typed_lowering_evidence` records. Sema records function-valued call evidence
  when a callee path/expression has `TypeKind::Function`, and records
  expected-function evidence when an expression is checked in a function-typed
  context.
- Type-check judgments now optionally retain source byte ranges. The current
  substrate wires parser-provided `let` RHS expression ranges into the root
  expression judgment, giving LSP/tooling a typed expression-to-source bridge
  without inventing traversal-index heuristics.
- Flow-level and statement-level control expressions now also retain authored
  source ranges through syntax, HIR, and sema. `if`/`if let`, `while`/
  `while let`, `for`, and `match` conditions/scrutinees/sources use the same
  `AuthoredExpr` path as control-transfer statements, so type judgments for
  those expressions can be mapped back to the original `.arcw` bytes.
- Structured value-producing `let` forms now retain authored source ranges for
  braced block expressions, computation/memo blocks, `if`/`if let`
  expressions, and `match` expressions. The expression source-range collector
  also uses a delimiter matcher that can find the matching `}` inside those
  expression roots, so nested block values and match arm values receive source
  ranges instead of stopping at the root expression.
- Guarded value-producing `if let` expressions now split the condition source
  at the `when` guard boundary. The matched expression keeps only its authored
  scrutinee range, and the guard receives its own source range instead of being
  absorbed into the scrutinee expression site.
- Desugared pipe RHS checking now keeps authored RHS child expression ranges
  in a temporary checker scope. `^` substitution and data-last call rewriting
  keep ranges for visible RHS children without assigning the `^` token to the
  inserted LHS clone, and method-chain data-last fallback retains the visible
  method-call range.
- Let-binding type judgments now retain the same RHS source range, and LSP
  function-type inlays match `let` sites against sema evidence by pattern and
  RHS range instead of pattern/traversal order alone.
- Function-like final body values now keep authored source ranges through
  syntax, HIR, and sema instead of collapsing to bare `Expr` values. This
  covers top-level `fn`/`task fn`/`dialogue fn`/`stream fn`, Agent function
  bodies, and trait/impl member function bodies. Runtime-plan and verifier
  paths explicitly project those authored values back to `Expr` when they do
  not need source identity.
- Sema now accepts calls through function-valued symbols and locals instead of
  treating those path callees as unknown named functions.
- Function value calls in sema now support partial application by returning a
  remaining `TypeKind::Function` when fewer positional arguments are supplied.
- Bare top-level function names now type-check as first-class function values
  in value position when their `FunctionSignature` has parameter metadata. For
  example, `let f = add; let add_two = f(2i64)` records function-value call
  evidence for `f`.
- Sema now preserves curried declaration call-group boundaries for top-level
  and extern capability function signatures. For example,
  `tuple_tail(a, b)(c) -> (i64, i64, i64)` is modeled as a first call group
  returning `c -> (i64, i64, i64)`, and `chain(a)(b)(c, d) -> i64` retains the
  two remaining groups after `chain(a)`.
- Sema regression coverage now explicitly fixes that `task fn`, `dialogue fn`,
  and `stream fn` declarations preserve multiple curried parameter groups
  through HIR and type checking. The `task`/`dialogue` cases also exercise
  staged calls from a flow, while the `stream` case checks the stream body
  against the final `Stream<T, E>` return contract.
- Sema now preserves curried trait/impl method call-group boundaries during
  method-call checking. For example, `fn above(self, min: i64)(value: i64)
  -> bool` makes `score.above(80i64)` typecheck as `i64 -> bool`, while
  `score.above(80i64, 81i64)` is rejected as a flattened curried call group.
- Compiler lowering now converts sema typed-lowering evidence into
  runtime-plan-local evidence and threads it through `RuntimePlanLowerOptions`.
- Runtime-plan lowering shares one typed expression cursor across flow, stream,
  and source lowering, and Agent bundle compilation now passes typecheck
  evidence into the Agent controller runtime-plan entrypoint.
- Strict runtime expression lowering consumes function-valued call evidence so
  path calls such as `f(1i64)` lower to `RuntimeExpr::Apply` when sema proved
  `f` is a function value. Without that evidence, the same unknown path call
  remains an adapter-facing `RuntimeExpr::Call`.
- Checked compiler runtime-plan lowering now requires the exact typed-lowering
  evidence count exported by sema. If that evidence is omitted from
  `RuntimePlanLowerOptions`, lowering fails with a structured
  `runtime.plan.lower` diagnostic instead of silently producing a plan with
  adapter-style call semantics.
- Runtime-plan lowering preserves curried pure-helper application as staged
  `RuntimeExpr::Apply`, including `tuple_tail(1i64, 2i64)(3i64)` as `[2, 1]`
  argument groups and `chain(1i64)(2i64)(3i64, 4i64)` as `[1, 1, 2]`.
- `samples/function-curried-call-groups` is now a project-shaped sample for
  those two call-group shapes, so the behavior is visible outside unit tests
  without adding new `flow @flow.*` declaration spelling.
- Strict runtime expression lowering consumes expected-function evidence so
  placeholder abstractions in function-argument positions, such as
  `accept(_ > 80i64)` where `accept` expects `i64 -> bool`, lower the argument
  to `RuntimeExpr::Function`.
- Sema now infers `_` placeholder function values without an explicit expected
  function type for unambiguous binary expressions whose non-placeholder side
  has a local/static type, such as `let high = _ > 80i64` and
  `let high = (_ > 80i64)`.
- Sema now infers partial-call abstraction for known positional callable
  signatures, such as `let add_one = add(_, 1i64)`, without hard-coding the
  callable name. Runtime-plan lowering consumes the inferred evidence and
  lowers both forms to `RuntimeExpr::Function`.
- Sema now accepts named missing-input partial application for checked
  top-level function signatures, such as
  `let add_to_one = add(right = 1i64)`. Runtime-plan lowering emits a
  `RuntimeExpr::Function` whose parameters are the missing helper inputs and
  whose body calls the annotated or inferred helper with provided named
  arguments in helper input order.
- Method-call syntax now has a typed data-last callable fallback for the
  positional case where no real method resolves and a function signature exists
  with the receiver as the last parameter. For example,
  `score.above(80i64)` can lower as `above(80i64, score)`. Sema records
  lowering evidence for this decision so real inherent/env/trait methods still
  win when they exist.
- Data-last method fallback now records an explicit typed runtime argument
  order. Named fallback calls such as `score.above(min = 80i64)` typecheck and
  lower as `above(80i64, score)` without relying on source argument order.
  Spread fallback candidates still produce a structured unsupported-fallback
  diagnostic instead of degrading to a generic `unknown method` error.
- Data-last pipe lowering now covers local function-valued aliases in addition
  to direct pure-helper names. For example,
  `let f = add; let partial = 2i64 |> f; let exact = 2i64 |> f(1i64)`
  typechecks through function-value evidence and lowers both pipe forms to
  `RuntimeExpr::Apply` against `Local("f")`, preserving data-last argument
  order without reclassifying the local as a helper.
- Named data-last pipe lowering now also preserves input-name order for direct
  pure helpers and accepted source-function candidates. A RHS such as
  `choose(right = "tail")` receives the pipe LHS as the next positional input
  and lowers through the same named callable path as direct calls, so accepted
  non-helper source functions emit `RuntimeExpr::Apply` with declaration-order
  arguments.
- Expression lexing now represents operators as `Token::Op(ExprOp::...)`
  rather than raw operator string payloads. Parser branches for `->`, `=>`,
  `|>`, range operators, comparison operators, and closure pipes are checked by
  Rust exhaustiveness/type checking; the spelling strings live only in the
  enum-backed display/canonical spelling tables. The expression token for
  `->` is `ExprOp::ThinArrow`, matching the CST-level
  `ArcweftPunctuation::ThinArrow` naming.
- CST/parser multi-token punctuation now uses `ArcweftPunctuation` helpers for
  grammar-significant `->`, `<-`, `=>`, and `|>` splitting/prefix checks. The
  spelling strings are centralized in the CST punctuation layer rather than
  repeated across type, closure, choice, source, line-plan, view, and statement
  parsers.
- Product AWBC session save/load now rejects `RuntimeValue::Function` payloads
  with structured `BundleSessionSaveError::UnsupportedRuntimeValue` diagnostics
  before export or restore accepts the snapshot. The validator walks fiber
  frames, cleanup stacks, suspension args, await-many state, source/stream
  queues, terminal values, and nested tuple/sequence/record/variant/iterator
  values so captured closures cannot be persisted accidentally before AWBC
  closure allocation has a versioned representation.
- Closure return type annotation is accepted as `|params| -> Type { ... }`
  and `|| -> Type { ... }`. Return-typed closures require block bodies.
  Parser tests cover top-level, zero-arg, call-argument, and missing-block
  cases.
- Sema checks declared closure return types against both expected function
  types and the block body result. Curried closures such as
  `|min: i64| |value: i64| -> bool { value >= min }` typecheck as
  `i64 -> (i64 -> bool)`.
- Sema now treats closure bodies as their own return boundary. `return expr`
  inside `|| -> Type { ... }` checks against the closure return type rather
  than an outer function/flow return type, and unannotated closures still block
  outer return expectations from leaking inward.
- Return statements now compare their value type against the active
  function-like return boundary instead of relying only on tail-expression body
  checking. This catches mismatches such as `|| -> bool { return 1i64 }`.
- Type-check reports now include closure capture inventory keyed by stable
  expression IDs. Sema records deterministic local captures for closure bodies,
  including captures of outer closure parameters by nested closures, while
  excluding locals declared inside the closure itself.
- Compiler checked runtime-plan lowering now projects sema closure capture
  inventories into `RuntimePlanLowerOptions`, and `RuntimePlanLowerReport`
  exposes runtime-plan-local closure capture metadata keyed by the same typed
  expression IDs. Runtime-plan metadata stores deterministic capture names and
  source-style type labels without making runtime-plan depend on sema
  `TypeKind`.
- Sema now records suspension boundaries seen inside each closure capture
  frame and rejects borrowed closure captures that cross those boundaries. The
  diagnostic is structured as
  `sema.typecheck.borrowed_closure_capture_crosses_boundary` and carries the
  capture name, borrowed type, lifetime labels, and owning boundary; tests cover
  `await`, `thread`, and `defer`, while non-borrow captures may cross `await`.
- Sema effect collection now gives closure literals their own synthetic
  private callable, so `let f = || { effectful_call() }` records the body
  effects on `closure.expr.N` instead of treating function-value creation as an
  immediate effect in the enclosing flow/function. Direct calls through the
  local closure binding, such as `f()`, compose that synthetic callable back
  into the caller's effect graph and therefore still respect explicit effect
  bounds. Partial application of a local closure binding carries the same
  synthetic callable to the partial alias without composing effects until that
  alias is called.
- Parenthesized closure literals are now parsed as normal prefix expressions,
  so immediate closure calls such as `(|| -> String { ... })()` and partial
  immediate closure application such as `(|path: String, suffix: String| ->
  String { ... })("story.arcw")` lower to structured call expressions instead
  of raw expression fallbacks. Sema composes closure body effects for exact
  immediate calls and carries the same synthetic closure callable to partial
  immediate aliases without performing the body effects until the alias is
  called.
- Built-in collection higher-order methods now compose closure argument body
  effects into the caller for `map` and `filter`. This covers direct closure
  arguments, local closure aliases, and partial closure aliases, while still
  leaving function-value creation and partial application themselves
  effect-free.
- User-defined top-level higher-order function calls now compose callback
  closure body effects into the caller when sema proves the callee function
  directly invokes the corresponding function-typed parameter. The checker
  records pending call-site edges so this works even though flows are checked
  before top-level function bodies. It covers direct closure arguments and
  local closure aliases, exact later curried call groups, and partial
  later-group callback application once the staged function value is finally
  called. The same pending-edge path is used by data-last method fallback
  calls. Functions that merely retain/pass a callback without invoking the
  parameter remain effect-free at the call site.
- Returned function values now preserve callback effect timing when the
  returned closure invokes a supplied higher-order parameter. Sema registers a
  private `fn.name.return` proxy callable from the function signature before
  flows are checked, connects that proxy to the direct returned closure or
  stored local closure alias when the function body is checked, and then links
  the proxy to the supplied callback only when the returned closure is called.
  Creating the returned function value remains effect-free.
- Sema now emits `sema.numeric.fallback_in_inferred_closure` warnings when an
  unsuffixed numeric literal or numeric sequence falls back to a stable default
  primitive type inside a closure body whose return type is inferred. Explicit
  closure return annotations and concrete expected function return types
  suppress the warning because they provide the numeric contract.
- Stable language docs now match this policy: unsuffixed numeric literals use
  expected types first, then fall back to `i32`/`f64` when no expected type is
  available, with tooling warnings for inferred contracts where the fallback
  would otherwise be hidden. LSP diagnostics publish the sema warning with the
  stable `sema.numeric.fallback_in_inferred_closure` code.
- LSP inlay hints now include inferred function-valued `let` bindings when the
  source binding has no explicit type ascription and sema proves the binding is
  a `TypeKind::Function`. This covers inferred closures such as `let f = || 1`
  and inferred partial-placeholder functions such as `let p = _ > 80i64`,
  using the same canonical `TypeKind::source_label()` surface spelling as sema
  diagnostics.
- Flow statement parsing now keeps multiline return-typed closure literals
  together as a single `let` statement by tracking existing CST punctuation
  depth while consuming statement continuations.
- `expr.rs` was split further by moving closure source splitting and character
  literal decoding into `expr/closure_source.rs` and `expr/char_literal.rs`,
  keeping the expression parser below the structure-audit error threshold.
- View interaction view-surface examples were updated from removed
  `ForEach(...) |item| { ... }` / unsupported `Grid(...)` authoring to the
  current `for item in items key = item.id { ... }` View DSL and supported
  container elements.
- Module-local user-defined enum variant payload selectors now compose
  function-valued payload callbacks for both tuple/newtype payload
  constructors and record payload constructors. Type-qualified tuple
  constructors such as `LoaderSpec.WithLoad(|path: String| -> String { ... })`
  are recognized from the parser's method-call AST when an expected enum type
  is available, while record constructors such as
  `WithLoad { load: |path: String| -> String { ... } }` project through the
  same nominal enum payload catalog. Destructured callee parameters such as
  `.WithLoad(load): LoaderSpec` and `.WithLoad { load }: LoaderRecordSpec`
  bind the callback with the declared payload function type and compose the
  caller-supplied closure body effects when the callee invokes that binding.
- `TypeCheckEnv` now carries typed enum variant payload metadata through
  `EnumVariantPayload`. External checker environments can register tuple or
  record payload contracts with `with_enum_variant_payload`, and sema uses the
  same selector path for destructured function-valued payload bindings even
  when the enum declaration is not present in the checked module.

## Current boundaries

- `_` without an expected function type is inferred only when the parameter type
  is available without speculative expression checking. The current cut covers
  unambiguous binary expressions and positional calls to known function
  signatures, including parenthesized binary placeholder expressions.
- Partial call abstraction such as `add(_, 1)` is inferred for known signatures
  with positional arguments and fixed named arguments. Repeated `_`
  placeholders in one partial-call region use the same generated parameter when
  all placeholder positions infer the same parameter type. Runtime lowering
  reorders named helper arguments by helper input name before emitting the call
  body. Named missing-input partial application is implemented for checked
  top-level function signatures that lower through the annotated or inferred
  helper path. Spread mixed with fixed signature partial-call forms now reports
  `sema.typecheck.unsupported_signature_partial_call` instead of degrading to
  generic spread or missing-argument errors, and rejected spread partials do not
  record `SignaturePartialCall` lowering evidence. Executable spread
  partial-call inference remains split to
  `docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`
  because runtime spread expansion, rest parameters, and fixed missing-input
  function construction need a shared evidence contract. Ambiguous
  multi-candidate callables remain open.
- `_` expected-type runtime lowering consumes explicit syntax-level function
  annotations and sema expected-function evidence threaded through compiler
  options.
- Top-level functions that lower through the annotated or inferred helper path
  now materialize as function values in runtime expression lowering, and flow
  lowering tracks local aliases/partial applies that are known function values.
  Sema function-valued path call evidence is now threaded into flow, stream,
  source, and Agent bundle runtime-plan lowering. Prefix partial calls to
  checked top-level function signatures are accepted in sema; checked runtime
  execution follows the helper-backed path and rejects non-helper partials
  until general top-level callable allocation is implemented.
- AWBC now has first-class closure allocation/apply instructions for
  expression-local function values that complete without suspension.
  `RuntimeExpr::Function` reserves a synthetic AWBC function and emits
  `MakeFunction` with deterministic captures; pending synthetic closure bodies
  are lowered after the enclosing function body so AWBC function/block ranges
  stay contiguous and owned by one function. `RuntimeExpr::Apply` now emits
  `ApplyFunction` instead of the former `function.apply` inventory intrinsic.
  The compact VM supports exact application, partial application, and chained
  application of AWBC-backed `RuntimeValue::Function` values. If the synthetic
  closure body suspends or yields budget during expression apply, the VM reports
  a runtime error; suspension-aware dynamic apply still needs a terminator and
  resume-point lowering design.
- Pipe no-`^` runtime lowering is helper-aware for named annotated/inferred
  helpers. Method syntax fallback now has typed lowering evidence for
  data-last helper signatures, including named method arguments. No-`^` pipe lowering also
  preserves local function-valued aliases as `RuntimeExpr::Apply` targets for
  both bare and call RHS forms. Spread fallback candidates are diagnosed as
  unsupported rather than silently becoming unknown methods. Multiple viable
  data-last fallback candidates from the module and external type-check
  environment now report `sema.typecheck.ambiguous_data_last_method_fallback`
  with all candidate labels instead of selecting one by merge order. Real
  env/inherent/trait methods still win, and viable data-last fallback
  candidates hidden by that real method now produce
  `sema.typecheck.shadowed_data_last_method_fallback` warnings. Executable
  spread fallback lowering, curried call-group runtime fallback metadata, and
  effectful or suspending top-level callable runtime allocation remain open.
- Curried declaration call-group metadata is now preserved for sema/runtime-plan
  callable application and sema trait/impl method calls. AWBC closure/apply now
  covers non-suspending expression closures. Function-value calls, including
  later curried call groups, now report
  `sema.typecheck.unsupported_function_value_call` for spread/named argument
  syntax that cannot be represented by the runtime apply contract, and rejected
  calls do not record `FunctionValueCall` lowering evidence.
  Suspension-aware dynamic function calls and persisted closure state remain
  open.
- Runtime identifiers no longer use `FlowRuntimeId(String)`,
  `EntryRuntimeId(String)`, `RuntimeLineId(String)`, or
  `StreamRuntimeId(String)` tuple string newtypes in the migrated
  runtime-plan/core/player/CLI call sites. The seq-07.6 cut uses typed owned
  `RuntimeIdPath` values, explicit source-boundary conversion, and
  `RuntimePublicLabel` for debug/AWBC/report strings. The supplied atom-table
  variant remains deliberately deferred until profiling shows ID equality,
  hashing, serialization size, or allocation cost is worth carrying
  `RuntimeIdTable` context through the data-format boundaries. See
  `docs/implementation/relative-runtime-id-boundaries-2026-07-07.md`.
- Closure capture lifetime diagnostics now cover borrowed local captures that
  cross checked suspension boundaries. Closure body effect composition is
  implemented for synthetic closure callables, direct calls through local
  closure bindings, immediate closure calls, partial application aliases, and
  built-in collection higher-order execution through `map`/`filter`. Direct
  calls and data-last method fallback calls to user-defined top-level
  higher-order functions also compose closure body effects for function-typed
  parameters that the callee body directly invokes, including exact later
  curried call groups, local aliases to those staged function values, and
  partial later-group callback application that composes only when the staged
  partial is finally called. Tuple-destructured callback parameters now project
  function-valued local bindings through semantic signature metadata, so a
  caller-supplied local closure alias or inline closure expression inside a
  nested tuple argument composes when the destructured binding is directly
  invoked by the callee and stays effect-free when the binding is only kept.
  Record destructured callback parameters now also project explicitly typed
  function-valued field patterns such as `Spec { load: load: String -> String
  }: Spec` from record/record-literal call arguments, and untyped field
  bindings such as `Spec { load, path }: Spec` use nominal struct field types
  when the struct is available in the checked module. Variant destructured
  callback parameters now project the single payload of the built-in
  `Option`/`Result` constructors used in expression calls, so `.Some(load):
  Option<String -> String>` and `.Err(load): Result<String, String -> String>`
  compose a caller-supplied closure when the callee invokes `load(...)`.
  Module-local and `TypeCheckEnv`-provided user-defined enum tuple/newtype and
  record payload constructors now use the same callback-selector path for
  destructured function-valued payload bindings. Callback invocations hidden
  inside direct returned closures and stored local closure aliases returned
  from the function now compose only when the returned closure is called. LSP
  diagnostics now surface the same effect trace evidence through related
  information notes, including returned-closure callback paths through
  `fn.name.return` proxy callables and higher-order arguments captured by the
  returned closure. Save/load
  currently has an explicit Product
  AWBC policy: runtime function values are
  rejected as non-persistable until AWBC closure allocation and snapshot
  versioning are designed. Numeric fallback lints
  inside inferred closure bodies are implemented for scalar integer/float
  fallback and numeric sequence fallback. LSP inlays are implemented for
  inferred function-valued `let` bindings and opt-in source-backed expression
  judgments.
- Closure `return expr` now binds to the nearest closure/function-like sema
  boundary for type checking. Strict runtime block lowering already preserves
  simple early-return shape by discarding later block statements after a
  lowered `return`, but structured closure control-flow lowering beyond the
  current `RuntimeExpr::Function` subset remains tied to the AWBC closure/apply
  work.
- Method-chain fallback sugar resolves after existing env/builtin/integer/
  handle/trait method checks, preserving real methods before data-last fallback.
  Ambiguity diagnostics for multiple viable data-last fallback candidates are
  implemented for module/environment callable overlap. Real method versus
  fallback overlap is surfaced as a non-fatal shadowing warning while preserving
  real method priority.
- Closure capture inventory collection, borrowed-capture suspension-boundary
  lifetime diagnostics, and runtime-plan capture metadata projection exist.
  Effect-row integration for closure captures remains open.
- LSP inlays now cover inferred function-valued `let` bindings and opt-in
  arbitrary expression type hints for source-backed expression judgments. The
  policy suppresses literals, paths, placeholders, function values, `Never`,
  aggregate literal sites, and duplicate `(source end, type label)` positions so
  the default profile stays quiet and enabled hints do not become noisy while
  source-range coverage is still expanding. Full source identity for every
  generated/desugared expression site remains open under
  `docs/reviews/requests/2026-07-07-seq-07.4.1-function-stack-expression-source-range-inlays.md`.

## Follow-up request

- `docs/reviews/requests/2026-07-07-seq-07-function-closure-runtime-apply-capture-and-method-sugar.md`
- `docs/reviews/requests/2026-07-07-seq-07.1-function-stack-typed-expression-lowering-evidence.md`
- `docs/reviews/requests/2026-07-07-seq-07.2-function-stack-placeholder-inference-and-method-fallback.md`
- `docs/reviews/requests/2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`
- `docs/reviews/requests/2026-07-07-seq-07.4-function-stack-capture-effect-lsp.md`
- `docs/reviews/requests/2026-07-07-seq-07.4.1-function-stack-expression-source-range-inlays.md`
- `docs/reviews/requests/2026-07-07-seq-07.5-function-stack-awbc-closure-apply.md`

## Validation

```bash
cargo test -p arcweft-core --lib --all-features
cargo test -p arcweft-lang-sema --lib --all-features
cargo test -p arcweft-runtime-plan --lib --all-features
cargo test -p arcweft-core --all-features runtime_function
cargo test -p arcweft-runtime-plan --all-features closure_to_function_expr
cargo test -p arcweft-runtime-plan --all-features expression_callee_call_to_apply
cargo test -p arcweft-runtime-plan --all-features strict_runtime_lowers
cargo test -p arcweft-runtime-plan --all-features strict_runtime_value_lowering_can_emit_pure_calls
cargo test -p arcweft-runtime-plan --all-features expected_partial_placeholder
cargo test -p arcweft-runtime-plan --all-features data_last_pipe
cargo test -p arcweft-runtime-plan --all-features runtime_plan_lowers_local_function_value_calls_to_apply
cargo test -p arcweft-lang-sema --all-features records_function_value_call_lowering_evidence
cargo test -p arcweft-lang-sema --all-features typechecks_partial_function_value_application
cargo test -p arcweft-lang-sema --all-features typechecks_partial_placeholder_function_and_vec_map
cargo test -p arcweft-lang-sema --all-features infers_partial_placeholder_function_without_expected_type
cargo test -p arcweft-lang-sema --all-features infers_parenthesized_partial_placeholder_function_without_expected_type
cargo test -p arcweft-lang-sema --all-features infers_partial_call_abstraction_without_expected_type
cargo test -p arcweft-lang-sema --all-features let_rhs_type_judgments_carry_source_ranges
cargo test -p arcweft-lang-sema --all-features function_like_body_value_judgments_carry_source_ranges
cargo test -p arcweft-lang-sema --all-features method_chain
cargo test -p arcweft-lang-sema --all-features data_last_pipe_through_local_function_value_records_call_evidence
cargo test -p arcweft-lang-sema --all-features curried_function_declaration
cargo test -p arcweft-lang-sema --all-features curried_task_dialogue_and_stream_functions_preserve_param_groups
cargo test -p arcweft-lang-sema --all-features closure_return
cargo test -p arcweft-lang-sema --all-features closure_body_effects_do_not_leak_on_function_value_creation
cargo test -p arcweft-lang-sema --all-features local_closure_call_composes_body_effects_into_caller
cargo test -p arcweft-lang-sema --all-features partial_local_closure_application_does_not_compose_until_called
cargo test -p arcweft-lang-sema --all-features partial_local_closure_alias_composes_body_effects_when_called
cargo test -p arcweft-lang-sema --all-features user_higher_order_function_argument
cargo test -p arcweft-lang-sema --all-features curried_higher_order
cargo test -p arcweft-lang-sema --all-features partial_curried_higher_order
cargo test -p arcweft-lang-sema --all-features method_chain_data_last_fallback_composes_higher_order_callback_effects
cargo test -p arcweft-lang-sema --all-features
cargo test -p arcweft-lsp --all-features inlay_hint_request
cargo test -p arcweft-lsp --all-features inlay_hint_request_reports_inferred_function_types
cargo test -p arcweft-lsp --all-features
cargo test -p arcweft-compiler --all-features runtime_plan_uses_typecheck_evidence_for_function_value_calls
cargo test -p arcweft-compiler --all-features checked_runtime_plan_reports_missing_typed_lowering_evidence
cargo test -p arcweft-compiler --all-features runtime_plan_uses_expected_function_evidence_for_placeholder_args
cargo test -p arcweft-compiler --all-features runtime_plan_uses_typecheck_evidence_across_stream_and_source_exprs
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_inferred_partial_placeholder_functions
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_typed_data_last_method_fallback
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_local_function_data_last_pipe_to_apply
cargo test -p arcweft-compiler --all-features runtime_plan_preserves_curried_call_group_application_samples
cargo test -p arcweft-runtime-plan --all-features runtime_plan_lowers_closure_return_statement_to_function_body
cargo test -p arcweft-lang-syntax --all-features select_and_index_are_structured_for_later_typechecking
cargo test -p arcweft-lang-sema --all-features numeric_primitive_types_keep_explicit_widths
cargo test -p arcweft-lang-sema --all-features expected_type_resolves_user_enum_short_variant
cargo test -p arcweft-lang-syntax --all-features trait_and_impl_members_preserve_curried_param_groups
cargo run -p arcweft-cli --all-features -- check samples/function-curried-call-groups/src/main.arcw
cargo test -p arcweft-compiler --all-features
cargo test -p arcweft-runtime-driver --all-features --test awbc_product_session
cargo test -p arcweft-lang-syntax --all-features closure
cargo test -p arcweft-runtime-plan --all-features
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

All listed validation passed after updating the lingering
`spec_should_pass/check/025_view_body_structured.arcw` fixture from `Bool` to
canonical `bool`. The structure audit reported 0 errors and 147 existing
warnings for the first cut. After the executable `filter` cut and structural
split, the structure audit reports 0 errors and 146 warnings. After the
user-defined higher-order function argument effect cut, focused sema tests,
full `arcweft-lang-sema` tests, workspace clippy, and structure audit passed;
the structure audit reports 0 errors and 148 warnings. Workspace clippy still
reports the existing `TraitMember` / `ImplMember` large enum warnings in
`arcweft-lang-syntax`. After the exact curried later-call-group callback
effect cut, focused `curried_higher_order` tests, full `arcweft-lang-sema`
tests, workspace check, workspace clippy, and structure audit passed; the
structure audit still reports 0 errors and 148 warnings, and clippy still only
reports the same existing large enum warnings.
The partial later-call-group callback cut adds focused
`partial_curried_higher_order` coverage for effect-free partial creation,
alias final call composition, and immediate final call composition. Full
`arcweft-lang-sema` tests, workspace check, workspace clippy, and structure
audit passed for this cut; clippy still only reports the existing large enum
warnings and structure audit still reports 0 errors and 148 warnings.

The function-like body value source-identity cut has focused passing coverage
for authored source ranges on top-level function final values and impl method
final values (`function_like_body_value_judgments_carry_source_ranges`), plus
the existing let RHS range regression. Focused syntax/HIR/sema/runtime-plan/
compiler check and `cargo fmt --all --check` passed. The current structure
audit run reports the existing `crates/arcweft-cli/src/app/bundle_view.rs`
SIZE001 error at 2622 physical LOC and 148 warnings; this slice did not touch
that CLI bundle view file.

The runtime function/apply cut has focused passing coverage for captured
function application, partial application, curried application, closure strict
lowering, and expression-callee call lowering. Workspace validation for this cut
is recorded in the commit/final response.

The named pure helper function-value cut has focused passing coverage for bare
helper path materialization, partial helper call lowering through
`RuntimeExpr::Apply`, exact helper calls that remain `RuntimeExpr::PureCall`,
and intrinsic non-path call preservation.

The expected partial-placeholder runtime lowering cut has focused passing
coverage for expression lowering and whole-flow runtime-plan lowering of
explicit single-parameter function annotations.

The helper-aware data-last pipe cut has focused passing coverage for direct
fallback calls, pure prefix partial helper apply, exact helper pure calls, and
sema/runtime alignment for call RHS forms including bare, positional, and named
helper calls.

The local function-valued data-last pipe cut has passing sema coverage for
function-value call evidence on both bare and call RHS pipe forms through a
local function alias, and compiler/runtime-plan coverage that those forms lower
to `RuntimeExpr::Apply` with `Local("f")` as the callee rather than a helper or
adapter call.

The Product AWBC save/load policy cut has passing `awbc_product_session`
coverage that a snapshot containing a runtime function value inside a cleanup
argument is rejected on both direct restore and encoded import with
`BundleSessionSaveError::UnsupportedRuntimeValue`, preserving the current
non-persistable closure boundary explicitly.
The cut also passed `cargo check --workspace --all-targets --all-features`,
`cargo clippy --workspace --all-targets --all-features` with only the existing
`TraitMember`/`ImplMember` large enum warnings, and structure audit with 0
errors / 147 warnings.

The AWBC non-suspending closure/apply cut has passing focused coverage for
hand-built closure bytecode (`closure_instructions_capture_and_apply_awbc_function_value`)
and runtime-plan-generated closure application
(`lowers_runtime_function_apply_to_awbc_closure_instructions`). The generated
plan test verifies the lowered program, confirms `MakeFunction` and
`ApplyFunction` are emitted instead of a `function.apply` intrinsic, and runs
the produced AWBC in the VM to return `"ok"`.
Follow-up focused runtime-plan tests also cover generated AWBC partial
application returning a function value
(`generated_awbc_partial_apply_returns_function_value`), nested curried closure
application in the shape `make_adder(2i64)(5i64)`
(`generated_awbc_curried_closure_apply_executes_returned_function`), and a
function value whose body calls a lowered pure helper
(`generated_awbc_function_value_apply_can_call_pure_helper_body`).

The local function-valued call cut has focused passing coverage for a flow that
aliases a pure helper, partially applies that local function value, and applies
the resulting local function value again.

The sema typed-lowering evidence cut has passing coverage for function-valued
symbol calls, bare top-level function names in value position, local
function-value calls after partial application, expected-function evidence for
`_` placeholder abstraction, and full `arcweft-lang-sema` tests.

The compiler/runtime-plan typed evidence cut has passing coverage for
function-valued path calls lowering to `RuntimeExpr::Apply` only when typecheck
evidence is supplied, and for expected-function placeholder arguments lowering
to `RuntimeExpr::Function`. The shared-cursor follow-up has passing coverage
for function-valued calls inside stream and source lowering after earlier flow
expressions have consumed typed expression IDs.

The inferred partial-placeholder and method-fallback cuts have passing sema
coverage for unannotated binary placeholder inference, parenthesized binary
placeholder inference, partial-call abstraction from known function signatures,
repeated positional partial-call placeholders, named partial-call placeholders,
typed data-last method fallback, and real method priority. Compiler
coverage confirms the inferred placeholder forms lower to `RuntimeExpr::Function`,
repeated and named partial-call placeholders reuse one generated runtime local
at each placeholder site, named helper arguments are reordered by helper
input name, and typed data-last method fallback lowers to a helper-backed call
using sema-proven argument order with the receiver appended as the last argument.

The data-last fallback diagnostic/order cut has passing sema coverage for named
method-call syntax that matches a data-last fallback candidate, ensuring it
records deterministic argument order evidence instead of falling back to source
order. Spread syntax still reports `UnsupportedDataLastMethodFallback` instead
of a generic unknown method. It also covers module/environment data-last fallback ambiguity with
`method_chain_reports_ambiguous_data_last_fallback_candidates`, which verifies
both candidate labels are reported and no arbitrary fallback lowering evidence
is recorded. The shadowed-fallback warning cut covers real env-method and trait
method priority with `method_chain_prefers_real_method_over_data_last_callable_fallback`
and `method_chain_prefers_trait_method_over_data_last_callable_fallback`.

The spread partial rejection coverage now also fixes the plain fixed-signature
spread shape requested by
`2026-07-07-seq-07.2.1-function-stack-spread-partial-and-fallback-contract.md`:
`rejects_fixed_signature_partial_call_with_spread` verifies that
`let f = add(values...)` reports the structured
`sema.typecheck.unsupported_signature_partial_call` diagnostic, does not
degrade into generic non-rest spread or missing-argument diagnostics, and does
not record `SignaturePartialCall` lowering evidence.
`curried_function_rejects_first_group_spread_partial_with_structured_diagnostic`
adds the same guarantee for the first call group of a curried top-level
function, complementing the existing later-group function-value rejection.
Ambiguous spread fallback is also fixed by
`method_chain_reports_ambiguous_spread_data_last_fallback_candidates`, which
reports every viable data-last fallback candidate and records no selected
fallback lowering evidence.

The unsupported function-value argument diagnostic cut has passing sema
coverage for rejected spread arguments on a later curried call group
(`curried_function_value_rejects_later_spread_group_with_structured_diagnostic`)
and rejected named arguments on a function-valued local
(`function_value_rejects_named_arguments_with_structured_diagnostic`). Both
tests verify the structured `UnsupportedFunctionValueCall` diagnostic and that
no `FunctionValueCall` lowering evidence is recorded for rejected calls.
Function-value arity mismatch now reports
`sema.typecheck.function_value_arity_mismatch` and suppresses rejected apply
lowering evidence, and function-value argument type mismatch now uses the
shared `ArgumentTypeMismatch` diagnostic fields.

The closure return type cut has passing parser coverage for `|params| -> Type
{ ... }`, `|| -> Type { ... }`, call-argument closures, and the required block
body diagnostic. Sema coverage confirms declared closure return types typecheck
against body values, mismatch diagnostics are produced, curried closure return
types preserve remaining function values, `return expr` binds to the nearest
closure/function boundary, and multiline return-typed closure lets are consumed
as one statement. Runtime-plan coverage confirms a closure block return
statement lowers into the generated function body while calls to that local
closure lower through `RuntimeExpr::Apply`.
The Unit/Never canonical type cut has passing parser coverage that both `!`
and `Never` parse to `TypeRef::Never`, and sema coverage that
`TypeKind::primitive_name("Never")` returns `TypeKind::Never` while source
labels continue to display the canonical `Never` spelling. The same sema
coverage fixes canonical function type labels as right-associated
`i64 -> String -> bool` and distinct tuple call-group labels such as
`(i64, String) -> bool`. `!` remains syntax only and is not registered as a
primitive name.
The enum shorthand evidence cut has passing sema coverage that user-defined
unit enum variants such as `.Calm` / `.Alert` resolve from expected types in
`let` ascriptions, function arguments, and nested value expressions. The
follow-up coverage extends the same user-defined enum fixture to tuple payload
and record payload constructors such as `.WithScore(7i64)` and
`WithMeta { label = "ready" }`, and runtime-plan coverage now verifies that
unit, tuple-payload, and record-payload short constructors lower to
`RuntimeExpr::Variant`. This locks the expected-type enum catalog path rather
than relying on a `DataFormat.Json` special case.
Focused validation for the follow-up passed with
`cargo test -p arcweft-lang-sema --all-features expected_type_resolves_user_enum_short_variant -- --nocapture`,
`cargo test -p arcweft-runtime-plan --all-features runtime_plan_lowers_user_enum_shorthand_payloads_to_variants -- --nocapture`,
`cargo check -p arcweft-lang-sema -p arcweft-runtime-plan --all-targets --all-features`,
and
`cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan --all-targets --all-features`.
The structure audit was recorded under
`docs/implementation/structure-audits/function-enum-shorthand-2026-07-08`;
after moving enum-constructor lowering into its own responsibility module, the
audit returned to the existing single production-size error in
`crates/arcweft-cli/src/app/bundle_view.rs`.

The closure capture and pattern-parameter cuts have passing sema coverage for
borrowed closure captures crossing `await`, `yield`, thread, and defer
boundaries. Suspension boundaries are now represented internally as typed
`SuspensionBoundary` values and converted to stable diagnostic labels only at
the diagnostic boundary. Closure parameter pattern coverage confirms tuple
destructuring and discard parameters typecheck without confusing pattern `_`
with expression `_` placeholder abstraction.
Runtime-plan and VM execution coverage for destructured closure parameters is
now fixed by
`cargo test -p arcweft-runtime-plan --lib --all-features strict_runtime_lowers_destructured_closure_param_to_match_body -- --nocapture`,
`cargo test -p arcweft-compiler --all-features runtime_plan_lowers_destructured_closure_parameter_application -- --nocapture`,
and
`cargo test -p arcweft-core --all-features vm_pure_backend_applies_runtime_function_with_destructured_param_body -- --nocapture`.

The closure effect-composition cut has passing sema coverage that closure body
effects do not leak into the enclosing flow when the closure value is merely
created, while a direct call through the local closure binding composes those
body effects into the caller and triggers the existing explicit effect-bound
diagnostic.
The partial closure alias follow-up has passing sema coverage that partial
application of a local closure binding does not compose the closure body
effects at partial-value creation time, and that calling the partial alias
composes the body effects into the caller.
The collection higher-order effect cut has passing sema coverage that
`map`/`filter` closure arguments compose body effects into the enclosing flow,
including direct closure arguments, local closure aliases, and partial closure
aliases.
The closure effect-composition cut and partial-alias follow-up both passed
`cargo check --workspace --all-targets --all-features`,
`cargo clippy --workspace --all-targets --all-features` with only the existing
`TraitMember`/`ImplMember` large enum warnings, and structure audit with 0
errors / 148 warnings.

The curried trait method metadata cut has passing sema coverage for preserving
the remaining call-group function type after a method call and rejecting
flattened curried trait method arguments.
Parser coverage now also fixes that both trait members and impl members
preserve multiple `FnSignature` parameter groups before HIR/sema lowering.

The canonical primitive spelling cut has passing coverage for rejecting
`Bool`, `Char`, and `string` in type annotations/signatures with direct
canonical replacement diagnostics. It also rejects widthless primitive-family
aliases `int`, `uint`, `float`, and `Number` with diagnostics that direct
authors to explicit-width numeric primitives. The native text input sample now
uses `String` return annotations.

The latest local function-valued data-last pipe validation reports structure
audit 0 errors / 147 warnings. `cargo clippy --workspace --all-targets
--all-features` still reports the existing `TraitMember` and `ImplMember`
`large_enum_variant` warnings in `arcweft-lang-syntax/src/ast/items.rs`; no
new clippy warning remains from the function/closure changes.

The built-in variant payload callback selector cut has focused sema coverage
for `.Some(load): Option<String -> String>` and `.Err(load): Result<String,
String -> String>` destructured callback parameters. These compose a
caller-supplied closure argument when the callee directly invokes the
destructured payload binding. The follow-up user-defined enum payload selector
cut extends the same selector model to module-local tuple/newtype and record
enum payload constructors. The built-in cut passed `cargo check -p
arcweft-lang-sema --all-features`, focused `variant_destructured_callback`
coverage, full `cargo test -p arcweft-lang-sema --all-features --quiet`,
`cargo clippy -p arcweft-lang-sema --all-targets --all-features --quiet`,
`cargo check --workspace --all-targets --all-features --quiet`, and
`cargo clippy --workspace --all-targets --all-features --quiet`. Workspace
clippy still reports unrelated warnings from the in-progress view/style dirty
worktree and existing `arcweft-lang-syntax` large enum warnings. Structure
audit still reports one unrelated error for
`crates/arcweft-cli/src/app/bundle_view.rs` exceeding 2500 physical LOC in the
current dirty worktree; no structure-audit error is introduced by the sema
function-stack slice.

The user-defined enum variant payload callback selector cut has focused sema
coverage for tuple/newtype payload constructors and record payload constructors
with destructured function-valued payload parameters. It passed
`cargo test -p arcweft-lang-sema --all-features user_enum_ -- --nocapture`
and full `cargo test -p arcweft-lang-sema --all-features --quiet` in the
current all-features validation slice. It also passed
`cargo check -p arcweft-lang-sema --all-features` and
`cargo clippy -p arcweft-lang-sema --all-targets --all-features --quiet`
with only the existing `TraitMember` / `ImplMember` large enum warnings from
`arcweft-lang-syntax`. The same slice split enum-constructor expression checks
into `checker/expr/enum_variant.rs`; after that split, structure audit no
longer reports `checker/expr.rs` above the 2500 LOC error threshold. Current
workspace check/clippy is no longer blocked by the previous tree-aware
view/style cut; the old surface-style fallback reference was removed from
`runtime_control_style_resolution.rs` and replaced with
`ViewProgramResource::runtime_element_styles_with_style`. Structure audit still
reports the unrelated existing
`crates/arcweft-cli/src/app/bundle_view.rs` 2500 LOC error.

The `TypeCheckEnv` enum payload metadata cut adds the public
`EnumVariantPayload` boundary type and `with_enum_variant_payload`
registration API. Focused sema tests cover tuple/newtype and record payload
callback selectors without a module-local enum declaration:
`cargo test -p arcweft-lang-sema --all-features env_enum_ -- --nocapture`
passed in the current all-features validation slice.

The returned/stored closure callback effect cut adds focused sema coverage for
effect-free returned closure creation, effect composition when the direct
returned closure is called, and effect composition when a stored local closure
alias is returned and then called. It passed
`cargo test -p arcweft-lang-sema --all-features returned_closure_callback -- --nocapture`
and full `cargo test -p arcweft-lang-sema --all-features --quiet` in the
current all-features validation slice. It also passed
`cargo clippy -p arcweft-lang-sema --all-targets --all-features --quiet` with
only the existing `TraitMember` / `ImplMember` large enum warnings from
`arcweft-lang-syntax`. Structure audit still reports the unrelated existing
`crates/arcweft-cli/src/app/bundle_view.rs` 2500 LOC error. Workspace
`cargo check --workspace --all-targets --all-features` was attempted in the
same dirty worktree and is blocked by unrelated in-progress
`crates/arcweft-player-scene/src/input.rs` edits: missing
`text_control_write_back_from_editor` and a focused text-editor borrow conflict.

The LSP-facing closure effect evidence cut keeps the trace contract in the
shared sema diagnostic representation: `TypeCheckError::diagnostic()` and
`TypeCheckWarning::diagnostic()` now preserve `EffectTrace` as diagnostic notes,
which LSP maps into `Diagnostic.relatedInformation`. Focused LSP coverage
checks that an explicit empty flow effect bound reports a returned-closure
callback trace through the `flow -> fn.make_loader.return -> closure -> captured
higher-order argument -> fs.read_text` path:
`cargo test -p arcweft-lsp --all-features diagnostics_surface_returned_closure_effect_trace -- --nocapture`
and the existing direct effect diagnostic test
`cargo test -p arcweft-lsp --all-features diagnostics_surface_upper_bound_exceeded_effect_error -- --nocapture`
passed in the current all-features validation slice, followed by full
`cargo test -p arcweft-lsp --all-features --quiet` and
`cargo test -p arcweft-lang-sema --all-features --quiet`. Focused clippy for
the modified crates,
`cargo clippy -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features --quiet`,
exited successfully; it still reported existing `TraitMember` / `ImplMember`
large enum warnings and unrelated dirty view/clipboard warnings from the shared
workspace. A current full workspace check,
`cargo check --workspace --all-targets --all-features`, also passed with one
unrelated dirty presentation test unused-import warning. Structure audit still
reports the unrelated existing
`crates/arcweft-cli/src/app/bundle_view.rs` 2500 LOC error. Arbitrary
expression inlays are no longer blocked on the initial LSP rendering policy;
the profile-gated source-range path below is the active implementation.
Completion for the split request still requires auditing any remaining
expression families that do not yet produce stable source identity.

The expression source-range substrate cut adds `TypeJudgment::source_range` and
threads parser-provided `let` RHS ranges into root expression judgments. Focused
coverage passed with
`cargo test -p arcweft-lang-sema --all-features let_rhs_type_judgments_carry_source_ranges -- --nocapture`,
followed by full
`cargo test -p arcweft-lang-sema --all-features --quiet` and
`cargo clippy -p arcweft-lang-sema --all-targets --all-features --quiet`.
Clippy still reports only the existing `TraitMember` / `ImplMember`
large-enum warnings from `arcweft-lang-syntax`. The current focused
all-features check for `arcweft-lang-syntax`, `arcweft-lang-sema`, and
`arcweft-bundle` passes. Structure audit still reports the unrelated existing
`crates/arcweft-cli/src/app/bundle_view.rs` 2500 LOC error.

The nested expression source-range cut extends the same substrate beyond the
root `let` RHS judgment. `arcweft-lang-syntax::expr::collect_expr_source_ranges`
now lives in `expr/source_ranges.rs` and collects syntax-owned subtree ranges
for authored expression nodes such as call arguments, selectors, pipes, binary
expressions, closures, block values, and match/if subexpressions. Sema consumes
the dormant `Stmt::Let::expr_source` field before type checking and registers
those ranges against the current HIR expression nodes, so nested judgments can
carry their own source slices without falling back to judgment traversal order.
Pipe expressions now pass the authored RHS range to the desugared data-last or
`^`-substituted RHS root, so generated call judgment ranges point at the
authored RHS expression and do not pretend the inserted LHS was written inside
that RHS. Focused coverage passed with
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
and the broader validation passed with
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-bundle --all-targets --all-features`
and
`cargo test -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-bundle --all-features`.
Focused clippy passed with
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-bundle --all-targets --all-features`;
only the existing `TraitMember` / `ImplMember` `large_enum_variant` warnings
remain. Structure audit for this cut now reports only the unrelated existing
`crates/arcweft-cli/src/app/bundle_view.rs` error and 148 warnings. This cut
does not enable arbitrary expression LSP inlays by default; the profile/policy
and display-placement contract remains with
`docs/reviews/requests/2026-07-07-seq-07.4.1-function-stack-expression-source-range-inlays.md`.

The first profile-gated arbitrary expression inlay cut wires that source-range
substrate into `arcweft-lsp`. `LspConfig::with_arbitrary_expression_type_inlays`
sets an opt-in policy that is carried through `LspProfileResolver` into each
`LspProfile`; the default remains unchanged and only emits the existing inferred
function-valued `let` inlays. When enabled, LSP inlay hints consume sema
`TypeJudgmentSubject::Expr` records with authoritative `source_range` evidence
and place a type inlay after the authored expression. The conservative policy
suppresses literal, path, entity, lifetime, short-variant, placeholder, raw,
function-valued, and `Never` sites, so nested call/binary/pipe expressions can
be inspected without turning every token into noise. Focused coverage passed
with
`cargo test -p arcweft-lsp --all-features expression_type_inlays_are_profile_gated_and_skip_trivial_sites -- --nocapture`
and preserves the existing function-valued binding behavior with
`cargo test -p arcweft-lsp --all-features inlay_hint_request_reports_inferred_function_types -- --nocapture`.
Full LSP validation passed with `cargo test -p arcweft-lsp --all-features`.
Focused clippy for `arcweft-lsp` completed; it reports only existing dependency
warnings from `arcweft-lang-syntax` large enum variants and unrelated
`arcweft-runtime-host` clipboard lifetime names. Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the current dirty worktree reports 2443 scanned files, 1170 Rust files, 572965
Rust physical LOC, and the same unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 148 warnings.

The selector source-range follow-up fixes the 07.4.1 selector policy with
explicit evidence: ordinary selector expressions retain the full visible range
such as `choice.label`, while the selector receiver keeps its own authored
range for nested judgments. The profile-gated LSP expression inlay test now
also verifies that a non-trivial selector can emit `: String` when arbitrary
expression inlays are enabled, while default profiles remain quiet. Focused
validation passed with
`cargo test -p arcweft-lang-sema --all-features desugared_function_stack_expression_judgments_keep_authored_source_ranges -- --nocapture`
and
`cargo test -p arcweft-lsp --all-features expression_type_inlays_are_profile_gated_and_skip_trivial_sites -- --nocapture`.
`cargo fmt --all --check` also passed. Remaining expression-inlay work before
closing the split request is to audit additional expression families against the
source identity contract rather than relying on the earlier let-RHS-only
boundary.

The non-annotated top-level prefix partial cut removes the sema-only
`#[pure]` gate from signature partial calls. Sema now types `add(2i64)` from a
plain top-level `fn add(lhs: i64, rhs: i64) -> i64` as `i64 -> i64`, and
compiler/runtime-plan lowering uses the existing inferred helper path to emit
`RuntimeExpr::Apply` over a materialized runtime function. Focused validation
passed with
`cargo test -p arcweft-lang-sema --all-features non_annotated_function_prefix_call_typechecks_as_partial_application`
and
`cargo test -p arcweft-compiler --all-features runtime_plan_lowers_non_annotated_function_prefix_partial_with_typecheck`.
The same slice also passed
`cargo check -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features`
and
`cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features`;
clippy still reports the existing `TraitMember` / `ImplMember`
large-enum warnings from `arcweft-lang-syntax`. Structure audit was run with
`cargo +nightly -Zscript tools/structure-audit.rs --root .`; in the current
dirty worktree it still reports the unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` 2500 LOC error and 148 warnings.
The changed Rust files measured for this cut are
`checker.rs` 77,240 bytes / 2,111 LOC, `checker/expr.rs` 93,338 bytes / 2,319
LOC, `checker/expr/signature_call.rs` 12,253 bytes / 325 LOC,
`checker/module.rs` 69,574 bytes / 1,753 LOC,
`tests/function_stack.rs` 91,843 bytes / 2,653 LOC, and
`arcweft-compiler/src/tests.rs` 81,282 bytes / 2,577 LOC; none crossed a new
error threshold in this slice.

The named missing-input/local-alias follow-up fixture cut removes lingering
`#[pure]` annotations from named missing-input and local function data-last
fixtures. Focused validation passed with
`cargo test -p arcweft-lang-sema --all-features non_annotated_function_named_missing_input_typechecks_as_partial_application`,
`cargo test -p arcweft-compiler --all-features runtime_plan_lowers_named_missing_inferred_helper_input`,
and
`cargo test -p arcweft-compiler --all-features runtime_plan_lowers_local_function_data_last_pipe_to_apply`.
The same slice passed
`cargo check -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features`
and
`cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features`;
clippy still reports only the existing `TraitMember` / `ImplMember`
large-enum warnings from `arcweft-lang-syntax`. Structure audit still reports
the unrelated `crates/arcweft-cli/src/app/bundle_view.rs` 2500 LOC error and
148 warnings. The changed Rust files measured for this follow-up are
`tests/function_stack.rs` 91,844 bytes / 2,652 LOC and
`arcweft-compiler/src/tests.rs` 81,270 bytes / 2,575 LOC; neither crosses a
new error threshold in this slice.

The signature-partial typed-evidence cut adds
`TypedLoweringEvidenceKind::SignaturePartialCall` and the runtime-plan-local
equivalent. Sema records the evidence when a direct top-level signature call
returns a partial function; checked runtime-plan lowering now rejects such
calls when no annotated/inferred helper exists, preventing unsupported
top-level partial callables from lowering as incomplete adapter calls. Focused
validation passed with
`cargo test -p arcweft-lang-sema --all-features non_annotated_function_prefix_call_typechecks_as_partial_application`,
`cargo test -p arcweft-compiler --all-features runtime_plan_lowers_non_annotated_function_prefix_partial_with_typecheck`,
and
`cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_source_function_partial_when_body_calls`;
the compiler tests were rerun serially after an initial parallel target-lock
timeout. The same slice passed
`cargo check -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features`
and
`cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features`;
clippy still reports only the existing `TraitMember` / `ImplMember`
large-enum warnings from `arcweft-lang-syntax`. Structure audit still reports
the unrelated `crates/arcweft-cli/src/app/bundle_view.rs` 2500 LOC error and
148 warnings. The changed Rust files measured for this cut are
`checker.rs` 77,429 bytes / 2,117 LOC, `checker/expr.rs` 93,373 bytes / 2,320
LOC, `checker/expr/signature_call.rs` 12,724 bytes / 338 LOC,
`typed_evidence.rs` 4,655 bytes / 125 LOC, `runtime-plan/src/expr.rs` 87,074
bytes / 2,341 LOC, `compiler/src/lower.rs` 11,798 bytes / 278 LOC,
`tests/function_stack.rs` 92,643 bytes / 2,673 LOC, and
`compiler/src/tests.rs` 82,443 bytes / 2,611 LOC; none crosses a new error
threshold in this slice.

The non-helper callable inventory cut closes the first implementation-order
step of request 07.7. The inventory lives in
`docs/implementation/function-stack-non-helper-callable-inventory-2026-07-08.md`
and classifies expression closures, local aliases, helper-backed top-level
functions, data-last helper/local paths, non-suspending AWBC generated
closures, helper-less signature partials, effectful/suspending callables,
task/dialogue/stream functions, trait/impl methods, adapter-backed callables,
and persisted function values. Runtime-plan diagnostics for helper-less
signature partials now carry the unsupported-family marker
`signature_partial_without_helper`, so the rejection boundary is auditable
without pretending the broader non-helper allocation contract is implemented.
The follow-up source-function-value cut accepts the first narrow non-helper
family: ordinary source-local `fn` declarations with simple identifier
parameters and expression bodies that contain no call/effect/suspension-capable
syntax. Curried `ParamGroup`s in that family lower to nested runtime functions.
Named missing-input partial calls for that family now synthesize wrapper
functions that preserve declaration argument order. Returned simple closure
literals in that family now recursively lower to nested runtime functions when
the closure body stays inside the same no-call/no-suspension accepted subset.
Direct calls to function-typed parameters in that family now lower as local
`RuntimeExpr::Apply`, preserving higher-order source functions without
pretending the callback invocation is a host or adapter call. Function-valued
`let` aliases and callback partial calls are now tracked inside the same
accepted source function body, so a partially applied callback can be invoked
later without leaving the runtime-function substrate.
Focused validation passed with
`cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_named_missing_source_function_partial_call -- --nocapture` and
`cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_curried_source_function_value -- --nocapture` and
`cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_returned_closure -- --nocapture` and
`cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_callback_param_call -- --nocapture` and
`cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_callback_partial_let -- --nocapture` and
`cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_source_function_partial_when_body_calls -- --nocapture`.

The named data-last pipe preservation cut has passing focused coverage for
both helper-backed and accepted non-helper source functions:
`cargo test -p arcweft-compiler --all-features runtime_plan_lowers_data_last_pipe_call_with_typecheck -- --nocapture`
and
`cargo test -p arcweft-compiler --all-features runtime_plan_lowers_source_function_named_data_last_pipe_to_apply -- --nocapture`.

The runtime-plan closure capture metadata cut adds
`RuntimeClosureCaptureInventory` / `RuntimeClosureCapture` under the
runtime-plan flow API and threads sema `TypeCheckReport::closure_captures`
through compiler checked lowering into `RuntimePlanLowerReport`. The metadata
keeps runtime-plan independent from sema `TypeKind` by storing source-style type
labels. Focused validation passed with
`cargo test -p arcweft-compiler --all-features runtime_plan_report_carries_closure_capture_metadata`,
followed by
`cargo check -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features`
and
`cargo clippy -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features`;
clippy still reports only the existing `TraitMember` / `ImplMember`
large-enum warnings from `arcweft-lang-syntax`. Structure audit was run after
splitting closure capture metadata out of `runtime-plan/src/flow.rs` into
`runtime-plan/src/flow/closure_metadata.rs`; it returned to the unrelated
single `crates/arcweft-cli/src/app/bundle_view.rs` 2500 LOC error and 148
warnings. The changed Rust files measured by the structure-audit CSV for this
cut are `runtime-plan/src/flow.rs` 90,518 bytes / 2,492 physical LOC,
`runtime-plan/src/flow/closure_metadata.rs` 1,018 bytes / 34 physical LOC,
`compiler/src/lower.rs` 12,613 bytes / 317 physical LOC, and
`compiler/src/tests.rs` 84,104 bytes / 2,871 physical LOC; no new
structure-audit error remains from this slice.

The function-kind curried ParamGroup evidence cut adds focused sema coverage
for `task fn`, `dialogue fn`, and `stream fn` declarations with multiple
curried parameter groups. The fixture verifies HIR keeps two call groups for
each function kind, `task`/`dialogue` staged calls typecheck from a flow, and
the curried stream function body still satisfies its final `Stream<T, E>`
return contract. Focused validation passed with
`cargo test -p arcweft-lang-sema --all-features curried_task_dialogue_and_stream_functions_preserve_param_groups`,
followed by
`cargo check -p arcweft-lang-sema --all-targets --all-features` and
`cargo clippy -p arcweft-lang-sema --all-targets --all-features`; clippy still
reports only the existing `TraitMember` / `ImplMember` large-enum warnings from
`arcweft-lang-syntax`. Structure audit still reports the unrelated single
`crates/arcweft-cli/src/app/bundle_view.rs` 2500 LOC error and 148 warnings.
The changed Rust file measured by the structure-audit CSV for this cut is
`crates/arcweft-lang-sema/src/tests/function_stack.rs` 94,633 bytes / 2,924
physical LOC; it remains a test file and no production structure-audit error is
introduced by this slice.

The statement expression source-range cut extends the existing `let` RHS
source-range substrate into function body statements. The syntax parser now has
a base-aware logical block item collector, and function/agent body parsing uses
the CST block body range when building typed statements. Sema registers authored
expression slices for `return` statements and expression statements, including
their nested child expression ranges, before emitting `TypeJudgment` records.
Focused coverage passed with
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
including `return_and_expression_statement_judgments_carry_source_ranges`.
The same slice passed
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`
and the LSP profile-gated arbitrary expression inlay smoke
`cargo test -p arcweft-lsp --all-features expression_type_inlays -- --nocapture`.
Focused clippy,
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`,
completed successfully; it still reports existing warnings from
`TraitMember`/`ImplMember` large enum variants plus unrelated dirty
runtime-plan/runtime-host warnings, with no new warning from this cut.
This does not yet claim complete source identity for every generated/desugared
expression site; those remain governed by
`docs/reviews/requests/2026-07-07-seq-07.4.1-function-stack-expression-source-range-inlays.md`.

The dialogue interpolation source-range cut extends the expression source-range
substrate into dialogue text tokens. `DialogueToken::Expr` now stores a
`DialogueExpr` wrapper carrying the parsed expression, the trimmed authored
expression source, and the absolute document byte range. Full-document dialogue
parsing uses a base-aware tokenizer so `#[...]` and `$()` interpolation ranges
line up with the original `.arcw` source instead of the isolated dialogue
string. Sema registers those ranges before type checking line-plan dialogue
content, which lets interpolation expression judgments participate in the same
source-backed tooling path as `let`, `return`, and expression-statement
judgments. Focused validation passed with
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-lsp --all-targets --all-features`,
`cargo test -p arcweft-lang-syntax --all-features --test parser_dialogue_syntax_and_defaults dialogue_interpolation_tokens_carry_document_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features dialogue_interpolation_judgments_carry_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features dialogue_tokenizer -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-syntax --all-features --test parser_dialogue_syntax_and_defaults -- --nocapture`,
and
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-lsp --all-targets --all-features`.
Focused clippy still reports only existing dependency warnings from
`TraitMember`/`ImplMember` large enum variants plus unrelated dirty
runtime-plan/runtime-host warnings. Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the current dirty worktree reports 2445 scanned files, 1170 Rust files, 573543
Rust physical LOC, and the same unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 148 warnings.

The control-transfer statement source-range cut introduces
`arcweft_lang_syntax::ast::flow::AuthoredExpr` and changes `Stmt::Goto`,
`Stmt::Yield`, `Stmt::Close`, and `Stmt::Select` from bare expression payloads
to source-aware expression payloads. Parser statement lowering records the
trimmed authored expression and absolute range when a base offset is available,
and sema registers that source before checking `goto`, `yield`, `close`, and
`select` statement expressions. Read-only traversal/lowering layers now unwrap
the payload through `AuthoredExpr::expr()`, so symbol collection, semantic
project indexing, verifier collection, runtime-plan lowering, line-task/source/
stream lowering, tooling scanners, and view-mount extraction keep the same
expression semantics while preserving source identity for tooling. Focused
validation passed with
`cargo test -p arcweft-lang-sema --all-features control_transfer_statement_judgments_carry_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-syntax --all-features --test parser_flow_statements_and_body -- --nocapture`,
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-lsp -p arcweft-cli -p arcweft-tooling -p arcweft-verify --all-targets --all-features`,
and
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-lsp -p arcweft-cli -p arcweft-tooling -p arcweft-verify --all-targets --all-features`.
Focused clippy exits successfully with existing unrelated warnings from
`arcweft-render-wgpu::font_system`, `TraitMember` / `ImplMember` large enum
variants, `runtime-plan/src/line_task.rs`, runtime-host clipboard lifetime
names, and player-native clipboard code. Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the current dirty worktree reports 2445 scanned files, 1170 Rust files, 573743
Rust physical LOC, and the same unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 148 warnings. The
line-plan simple statement rows are now parsed through the same base-aware
statement path, so full-document `select @choice.primary` expressions preserve
their authored range as well. This cut verifies `goto`, `close`, stream
`yield`, and line-plan `select` judgments.

The control-statement expression source-range cut extends `AuthoredExpr`
through both `Stmt` and HIR flow block payloads. Syntax AST and HIR keep
existing `condition()`/`expr()`/`source()` accessors returning `&Expr` for
read-only traversal/lowering code, while checker-facing authored accessors
carry the original expression text and absolute range. Flow block parsing now
uses the trim-adjusted head base for `if`, `if let`, `while`, `while let`,
`for`, and `match`, and match arm body statements are parsed with their real
body-line base rather than a zero-origin nested parser range. Sema checks
flow-level and statement-level control expressions through the authored path,
so type judgments for `if` conditions, loop conditions, `for` sources, and
`match` scrutinees retain source ranges. Focused validation passed with
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-syntax --all-features --test parser_flow_statements_and_body -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features control_flow -- --nocapture`,
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-lsp -p arcweft-cli -p arcweft-tooling -p arcweft-verify --all-targets --all-features`,
and
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-lsp -p arcweft-cli -p arcweft-tooling -p arcweft-verify --all-targets --all-features`.
Focused clippy exits successfully with existing warnings from
`arcweft-render-wgpu::font_system`, `TraitMember` / `ImplMember` large enum
variants, line-plan `Stmt` enum size after source-range payload retention,
`runtime-plan/src/line_task.rs`, runtime-host clipboard lifetime names, and
player-native clipboard code. Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the current dirty worktree reports 2445 scanned files, 1170 Rust files, 574101
Rust physical LOC, and the same unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 148 warnings.

The desugared pipe/function-stack expression source-range cut fixes cloned RHS
nodes created during `|>` lowering. Sema now copies authored RHS subtree ranges
onto the desugared expression only for the duration of checking, restoring the
pointer-keyed range map afterward so temporary clone addresses cannot leak into
later judgments. This preserves child ranges for `add(^, 11i64)` and
`add(22i64)` after pipe lowering, keeps `_ > threshold` partial-placeholder
and `|value| value + 1i64` closure body judgments source-backed, and verifies
that `threshold.above(70i64)` method-chain data-last fallback retains the
visible call expression range. Focused validation passed with
`cargo fmt --all --check`,
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-lsp --all-targets --all-features`,
and
`cargo clippy -p arcweft-lang-sema --all-targets --all-features`. Clippy exits
successfully with the existing `TraitMember` / `ImplMember` / `LinePlanItem`
large-enum warnings from `arcweft-lang-syntax`. The source-range transfer code
is split into `checker/source_ranges.rs` to avoid adding more responsibility to
the already-large checker module. Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the current dirty worktree reports 2446 scanned files, 1171 Rust files, 574710
Rust physical LOC, and the same unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 148 warnings.

The assignment statement source-range cut changes `Stmt::Assign` target and RHS
payloads from bare expressions to `AuthoredExpr`. Parser statement lowering now
records the trimmed target and RHS text with absolute ranges, while traversal,
tooling, verifier, runtime-plan lowering, view-mount extraction, and sema unwrap
through `AuthoredExpr::expr()` at their existing expression boundaries. Sema
registers both assignment-side expression source trees and checks the RHS
against the inferred assignment target type at the authored RHS range, so
expected-type judgments now point back to the visible source instead of an
anonymous parser expression. The regression
`assignment_statement_rhs_judgments_carry_source_ranges` verifies the RHS root
and child literal ranges for `counter.value = counter.value + 2i64`. Focused
validation passed with
`cargo fmt --all --check`,
`cargo test -p arcweft-lang-sema --all-features assignment_statement_rhs_judgments_carry_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-lsp -p arcweft-cli -p arcweft-tooling -p arcweft-verify --all-targets --all-features`,
and
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-lsp -p arcweft-cli -p arcweft-tooling -p arcweft-verify --all-targets --all-features`.
Focused clippy exits successfully with existing unrelated warnings from
`arcweft-lang-syntax` large enum variants, `arcweft-lang-sema::symbols`
`collect_stmt`, `arcweft-runtime-plan/src/line_task.rs`, runtime-host clipboard
lifetimes, `arcweft-render-wgpu::font_system`, and player-native clipboard
mapping code. Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the current dirty worktree reports 2446 scanned files, 1171 Rust files, 574836
Rust physical LOC, and the same unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 148 warnings.

The action receive and single-line defer statement source-range cut changes
`Stmt::LetActionReceive::action` and `Stmt::Defer::expr` from bare `Expr`
payloads to `AuthoredExpr`. `receive action(...)` now records the inner action
target range rather than using suffix-based statement range math, and flow-level
single-line `defer expr` parsing now routes through the base-aware statement
parser instead of the range-less fallback path. Sema checks both payloads
through the authored expression helpers, while project indexes, symbol
collection, verifier collection, runtime-plan lowering, tooling scans, and view
mount extraction unwrap through `AuthoredExpr::expr()` at read-only boundaries.
The regression `action_receive_and_defer_judgments_carry_source_ranges`
verifies the action target range for
`receive action(@action:.feedback.submit)` and the defer expression root/child
ranges for `defer 3i64 + 4i64`. Focused validation passed with
`cargo fmt --all --check`,
`cargo test -p arcweft-lang-sema --all-features action_receive_and_defer_judgments_carry_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-syntax --all-features --test parser_flow_statements_and_body -- --nocapture`,
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-lsp -p arcweft-cli -p arcweft-tooling -p arcweft-verify --all-targets --all-features`,
and
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-lsp -p arcweft-cli -p arcweft-tooling -p arcweft-verify --all-targets --all-features`.
Focused clippy exits successfully with the same existing unrelated warnings from
`arcweft-lang-syntax` large enum variants, `arcweft-runtime-plan/src/line_task.rs`,
runtime-host clipboard lifetimes, `arcweft-render-wgpu::font_system`, and
player-native clipboard mapping code. Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the current dirty worktree reports 2446 scanned files, 1171 Rust files, 574915
Rust physical LOC, and the same unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 148 warnings.

The LSP expression-inlay stabilization cut keeps arbitrary expression type
inlays profile-gated and source-backed. It deduplicates identical labels at the
same source end, suppresses aggregate literal sites such as `Choice { ... }`,
and preserves the default quiet profile. Focused validation passed with
`cargo fmt --all --check`,
`cargo test -p arcweft-lsp --all-features expression_type_inlays_are_profile_gated_and_skip_trivial_sites -- --nocapture`,
`cargo test -p arcweft-lsp --all-features inlay_hint_request_reports_inferred_function_types -- --nocapture`,
and
`cargo check -p arcweft-lsp -p arcweft-lang-sema --all-targets --all-features`.

The structured container/control expression source-range cut closes the next
part of request
`docs/reviews/requests/2026-07-07-seq-07.4.1-function-stack-expression-source-range-inlays.md`.
Flow parser value-producing `let` forms now reconstruct source slices from the
`CstBlockEvent` range instead of leaving `expr_source` empty for structured
braced expressions. This covers `let x = { ... }`, computation/memo blocks,
`let x = if ... { ... } else { ... }`, `let x = if let ...`, and
`let x = match ... { ... }`. The syntax expression source collector also fixes
delimiter matching for control-expression roots so `if` branch block children
and `match` arm values are collected under their authored ranges. Focused
validation passed with
`cargo test -p arcweft-lang-syntax --all-features match_expression_arm_values_keep_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features container_and_control_expression_judgments_carry_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features desugared_function_stack_expression_judgments_keep_authored_source_ranges -- --nocapture`,
`cargo test -p arcweft-lsp --all-features expression_type_inlays_are_profile_gated_and_skip_trivial_sites -- --nocapture`,
`cargo test -p arcweft-lsp --all-features inlay_hint_request_reports_inferred_function_types -- --nocapture`,
`cargo fmt --all --check`, and
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`.

The container child source-range audit tightens the same regression fixture for
expression families that were implemented but not yet explicitly evidenced:
array-repeat length expressions, index expressions, range bounds, nominal
record field values, and anonymous record literals in call-argument position.
The anonymous record case is intentionally exercised as an expression argument
because a naked `{ field = value }` after `let` is parsed by the flow statement
block path rather than as an expression literal. Focused validation passed with
`cargo test -p arcweft-lang-sema --all-features container_child_expression_judgments_carry_source_ranges -- --nocapture` and
`cargo test -p arcweft-lang-sema --all-features container_and_control_expression_judgments_carry_source_ranges -- --nocapture`.

The computation/closure block-value source-range follow-up fixes two concrete
coordinate bugs found while auditing the authored expression wrapper path.
Structured `let` parsing now derives braced value starts from the top-level
binding `=` instead of searching the whole line for the RHS head, so bindings
like `let from_result = result { ... }` no longer anchor the expression range
inside the local name. Closure body source collection also accounts for the
stripped `->` token before locating a braced return-typed closure body, and
block-value collection now descends through prefixed braced roots such as
`result { ... }`, `task { ... }`, and `memo(...) { ... }`. The regression
`computation_and_braced_closure_judgments_carry_source_ranges` verifies
source-backed judgments for computation block roots, computation block final
values, return-typed braced closure roots, and closure body final values.
Focused validation passed with
`cargo test -p arcweft-lang-sema --all-features computation_and_braced_closure_judgments_carry_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-syntax --all-features --test parser_flow_statements_and_body -- --nocapture`,
`cargo fmt --all --check`,
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`, and
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`.
The focused clippy command exits successfully with existing unrelated warnings
from `arcweft-lang-syntax` large enum variants, `arcweft-runtime-plan`, sema
line-count lints, and runtime-host clipboard lifetime names.

The guarded if-let source-range follow-up fixes the remaining condition split
inside value-producing `if let ... when ...` expressions. The syntax collector
now uses the language-level `when` boundary, so the scrutinee range stops before
the guard and the guard expression gets its own source-backed judgment. The
regression lives in
`container_and_control_expression_judgments_carry_source_ranges` and asserts
that `maybe`, `ready && true`, and the full `if let` root each carry distinct
authored source ranges. Focused validation passed with
`cargo test -p arcweft-lang-sema --all-features container_and_control_expression_judgments_carry_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
`cargo test -p arcweft-lsp --all-features expression_type_inlays_are_profile_gated_and_skip_trivial_sites -- --nocapture`,
`cargo test -p arcweft-lang-syntax --all-features match_expression_arm_values_keep_source_ranges -- --nocapture`,
`cargo fmt --all --check`, and
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`.

The effect/prefix expression source-range follow-up closes another narrow part
of the same 07.4.1 request. The syntax expression source collector now treats
`await? expr` as its own prefix spelling when advancing the child expression
base offset, so nested ranges start after the question marker instead of one
byte early. Sema coverage now verifies source-backed judgments for flow-level
`await? load_bg()`, postfix `maybe?`, prefix `try Some(unwrapped)`, numeric
unary `-unwrapped`, and boolean unary `!flag`. Focused validation passed with
`cargo test -p arcweft-lang-syntax --all-features await_question_keeps_inner_expression_source_range_after_question_mark -- --nocapture`
and
`cargo test -p arcweft-lang-sema --all-features effect_and_prefix_expression_judgments_carry_source_ranges -- --nocapture`.

The thread/numeric expression source-range follow-up closes two more expression
families under the same 07.4.1 request. `Expr::Thread` collection now descends
into the authored `{ ... }` body using postfix brace bounds, so the spawned
body expression keeps the original document range instead of only checking as
an anonymous child-task statement. Compact `NumericBracketSeq` roots are also
covered as source-backed judgments; the compact summary intentionally has no
child literal `Expr` nodes to attach per-item judgments to. Focused validation
passed with
`cargo test -p arcweft-lang-sema --all-features thread_expression_body_judgments_carry_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features numeric_bracket_sequence_judgments_carry_source_ranges -- --nocapture`,
and
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`.

The memo-block option expression source-range follow-up closes another
source-identity gap in the same 07.4.1 request. `Expr::MemoBlock` option values
are type-checked before the memo body, but the previous source collector only
descended into the final block value. The collector now splits the authored
`memo(...)` head at top-level commas and records each named option value range,
so judgments for `scope=scene` and `key=score + 1i64` point to the visible memo
head expressions. Focused validation passed with
`cargo test -p arcweft-lang-sema --all-features memo_block_option_expression_judgments_carry_source_ranges -- --nocapture`.

The dialogue-call line-plan expression source-range follow-up closes a separate
07.4.1 gap for same-line `with { ... }` and following-line `with:` attachments
on `let` dialogue calls.
`parse_let_dialogue_call` already attached the parsed plan to
`Expr::DialogueCall`, but its statement `expr_source` ended at the dialogue
content bracket, so line-plan-only expression judgments such as `out score +
1i64` had no authored range. The parser now reconstructs the `Stmt::Let`
expression source/range through attached line plans, and the syntax expression
source collector descends into `LinePlanItem::Out`, option, let, timed cue,
assert, plain expression, and simple grouped plan items from the authored
dialogue-call source. Focused validation passed with
`cargo test -p arcweft-lang-syntax --all-features --test parser_dialogue_syntax_and_defaults let_dialogue_call_expr_source_includes_same_line_plan -- --nocapture`,
`cargo test -p arcweft-lang-syntax --all-features --test parser_dialogue_syntax_and_defaults let_dialogue_call_expr_source_includes_following_line_plan -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features dialogue_call_line_plan_expression_judgments_carry_source_ranges -- --nocapture`,
and
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`.
The broader slice checks also passed with
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`,
`cargo test -p arcweft-lang-syntax --all-features --test parser_dialogue_syntax_and_defaults -- --nocapture`,
`cargo test -p arcweft-lsp --all-features expression_type_inlays_are_profile_gated_and_skip_trivial_sites -- --nocapture`,
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`,
and
`git diff --check`. Focused clippy exits successfully with existing unrelated
warnings from large syntax enum variants, `runtime-plan/src/line_task.rs`,
sema test/function length, runtime-host clipboard lifetimes, and an LSP
needless-pass-by-value warning. Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the dirty worktree reports 2446 scanned files, 1171 Rust files, 576263 Rust
physical LOC, and the existing unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 148 warnings.

The control-transfer authored expression follow-up extends the same source
identity substrate to value-carrying `out` and `break` statements. `Stmt::Out`
and value-bearing `Stmt::Break` now store `AuthoredExpr` payloads through
syntax and HIR, while sema, runtime-plan, verify, tooling, symbol collection,
and view-mount collection explicitly project those authored payloads back to
plain `Expr` only where source identity is not needed. This closes the
remaining gap where line-plan `out expr` and loop `break expr` could be
type-checked without a stable source range attached to the expression
judgment. Focused validation passed with
`cargo test -p arcweft-lang-sema --all-features control_transfer_statement_judgments_carry_source_ranges -- --nocapture`
and
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`.
The broader validation slice passed with
`cargo test -p arcweft-lang-syntax --all-features --test parser_flow_statements_and_body -- --nocapture`,
`cargo fmt --all --check`,
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-tooling -p arcweft-verify -p arcweft-cli -p arcweft-lsp --all-targets --all-features`,
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-tooling -p arcweft-verify -p arcweft-cli -p arcweft-lsp --all-targets --all-features`,
and `git diff --check`. Focused clippy exits successfully with existing
unrelated warnings from large syntax enum variants,
`runtime-plan/src/line_task.rs`, sema function/test length,
runtime-host/player-native clipboard helpers, and render-wgpu font docs.
Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the dirty worktree reports 2446 scanned files, 1171 Rust files, 576472 Rust
physical LOC, and the existing unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 148 warnings.

The wait-statement authored expression follow-up extends the same statement
source-identity path to `wait(...)` payloads. `WaitTarget::Duration` and
`WaitTarget::Expr` now carry `AuthoredExpr`, so sema can type-check duration
waits through the expected-type authored expression path and attach the range
of the expression inside `wait(...)` rather than the whole statement.
Runtime-plan, verify,
symbol collection, view action scanning, and view-mount collection explicitly
project the authored payload back to `Expr` where source identity is not
needed. While validating this cut, line-plan colon block parsing also received
an indentation restoration fix: logical line items are stored trimmed, so the
line-plan parser now reconstructs relative indentation from each item's
absolute source base before deciding whether an item belongs to `init:`,
`on:`, `thread:`, `defer:`, `start:`, or nested groups. Focused validation
passed with
`cargo test -p arcweft-lang-sema --all-features control_transfer_statement_judgments_carry_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features line_plan -- --nocapture`,
and
`cargo test -p arcweft-lang-syntax --all-features --test parser_flow_statements_and_body -- --nocapture`.
The broader validation slice passed with
`cargo fmt --all --check`,
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-tooling -p arcweft-verify -p arcweft-cli -p arcweft-lsp --all-targets --all-features`, and
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-tooling -p arcweft-verify -p arcweft-cli -p arcweft-lsp --all-targets --all-features`.
Focused clippy exits successfully with existing unrelated warnings from large
syntax enum variants, `runtime-plan/src/line_task.rs`, sema function/test
length, runtime-host/player-native clipboard helpers, and render-wgpu font
docs. Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the dirty worktree reports 2446 scanned files, 1171 Rust files, 576526 Rust
physical LOC, and the existing unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 148 warnings.

The dialogue-call line-plan colon-block source-range follow-up fixes a smaller
gap discovered while organizing the current status. The `with:` plan source is
trimmed before expression source collection, so a first item such as
`let cue = at(0.42s):` could previously lose its parent indentation and absorb a
following sibling `out` item. The syntax collector now splits line-plan item
sources with colon-block grouping and descends into top-level colon bodies, so
named cue body expressions and later sibling line-plan items both keep their
authored ranges. Focused validation passed with
`cargo test -p arcweft-lang-syntax --all-features line_plan_colon_let_block_does_not_absorb_following_items -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features dialogue_call_line_plan_expression_judgments_carry_source_ranges -- --nocapture`,
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`,
and
`cargo test -p arcweft-lsp --all-features expression_type_inlays_are_profile_gated_and_skip_trivial_sites -- --nocapture`.
The broader validation slice passed with
`cargo fmt --all --check`,
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`,
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`,
and `git diff --check`. Focused clippy exits successfully with existing
unrelated warnings from large syntax enum variants,
`runtime-plan/src/line_task.rs`, sema function/test length, and runtime-host
clipboard lifetime names. Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the dirty worktree reports 2447 scanned files, 1171 Rust files, 576764 Rust
physical LOC, and the existing unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 150 warnings.

The thread-expression body source-range follow-up fixes another localized
07.4.1 gap in the syntax collector. `Expr::Thread` previously descended into
each expression statement with the whole `{ ... }` body source, which made
multiple thread-body expression statements share the same authored range when
the body statements did not carry their own `AuthoredExpr` range. The collector
now splits thread body logical items as a fallback, still preferring statement
or authored expression source ranges when they exist, and also descends through
common nested flow/statement bodies inside thread expressions. The
thread/flow-body descent now lives in the `expr/source_ranges/thread_body.rs`
responsibility module instead of growing the already-large root collector.
Focused validation passed with
`cargo test -p arcweft-lang-syntax --all-features thread_expression_statement_sources_do_not_share_block_range -- --nocapture`
and
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`.
The broader validation slice passed with
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`
and
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features`.
Focused clippy exits successfully with existing unrelated warnings from large
syntax enum variants, `runtime-plan/src/line_task.rs`, sema function/test
length, and runtime-host clipboard lifetime names. Structure audit was rerun
with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the dirty worktree reports 2448 scanned files, 1172 Rust files, 577086 Rust
physical LOC, and the existing unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 150 warnings.

The lifetime-registry statement source-range follow-up closes the next
localized 07.4.1 gap from the status refresh. `Stmt::Signal` and
`Stmt::LifetimeSet` now store `AuthoredExpr` payloads instead of bare `Expr`
values. Parser-produced lifetime registry writes such as
`'flow.flags.score <- score + 1i64` carry authored value ranges through sema,
while runtime-plan, verifier, project-index, tooling, and View scanners project
those authored payloads back to plain expressions only where source identity is
irrelevant. Focused validation passed with
`cargo test -p arcweft-lang-sema --all-features lifetime_set_statement_value_judgments_carry_source_ranges -- --nocapture`.

The expression source-range coverage slice closes the next local 07.4.1 audit
gap. The new coverage matrix lives in
`docs/implementation/function-stack-expression-source-range-coverage-2026-07-08.md`.
The audit found that typed statement `let-else` RHS expressions, statement
`while let` guards, and statement `match` arm guards/bodies could still be
checked without authored source identity. `Stmt::LetElse.expr` now stores
`AuthoredExpr`, `StmtMatchArm.guard` now stores `Option<AuthoredExpr>`, and the
typed statement parser preserves base ranges for inline `let-else` and
statement match arm bodies. `TypeCheckStats` now records source-backed and
source-missing expression judgment counts so the report can be audited without
traversal-order fallbacks. Focused validation passed with
`cargo test -p arcweft-lang-sema --all-features typed_branch_statement_judgments_carry_source_ranges -- --nocapture`
and
`cargo test -p arcweft-lang-sema --all-features source_ranges -- --nocapture`.
The broader validation slice passed with
`cargo test -p arcweft-lang-syntax --all-features --test parser_flow_statements_and_body -- --nocapture`,
`cargo test -p arcweft-lsp --all-features expression_type_inlays_are_profile_gated_and_skip_trivial_sites -- --nocapture`,
`cargo fmt --all --check`,
`cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-tooling -p arcweft-verify -p arcweft-cli -p arcweft-lsp --all-targets --all-features`,
and
`cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-tooling -p arcweft-verify -p arcweft-cli -p arcweft-lsp --all-targets --all-features`.
Focused clippy exits successfully with existing unrelated warnings from large
syntax enum variants, `runtime-plan/src/line_task.rs`, sema line-count lints,
runtime-host/player-native clipboard helpers, and render-wgpu font docs.
Structure audit was rerun with
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-expression-source-ranges-2026-07-08`;
the dirty worktree reports 2450 scanned files, 1172 Rust files, 577326 Rust
physical LOC, and the existing unrelated
`crates/arcweft-cli/src/app/bundle_view.rs` error with 150 warnings.
