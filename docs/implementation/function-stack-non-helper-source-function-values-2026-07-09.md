# Function Stack Non-Helper Source Function Values - 2026-07-09

## Status

First accepted 07.7 expansion implemented.

Runtime-plan lowering can now materialize a narrow subset of ordinary
source-local top-level `fn` declarations as runtime function values without
using the pure-helper table. The first follow-up expands that subset from one
parameter group to multiple curried `ParamGroup`s by lowering each group to a
nested `RuntimeExpr::Function`. The second follow-up accepts source function
bodies that return simple closure literals, so a source-local function can now
return another runtime function without being forced through pure-helper
lowering. The third follow-up accepts direct calls to function-typed
parameters, lowering them to `RuntimeExpr::Apply` against the local function
value instead of adapter-style `RuntimeExpr::Call`. The fourth follow-up tracks
function-valued `let` bindings derived from aliases, closure literals, and
partial calls to function-typed parameters, so a source function can bind a
partially applied callback and invoke it later in the same accepted body.
The fifth follow-up allows those function-valued closure literals to use
destructuring parameter patterns. The lowered closure keeps the stable runtime
function parameter list by using a synthetic runtime argument name and a
single-arm `RuntimeExpr::Match` body, matching the shared closure-parameter
lowering path. The sixth follow-up threads the existing pure-helper lookup
through this accepted source-function candidate pass, so exact calls to
already-lowered pure helpers are accepted inside those bodies and lower as
`RuntimeExpr::PureCall`.
The seventh follow-up closes the parser/sema/runtime-plan evidence gap for
value-producing control expressions in this family. Function-body `let` values
now parse authored `if` / `if let` / `match` expressions before the let-else
fallback, value `if let` guards can see the pattern bindings they guard, and
compiler regressions prove accepted source functions materialize
`RuntimeExpr::If`, `RuntimeExpr::IfLet`, and `RuntimeExpr::Match` bodies.
The eighth follow-up changes source-function candidate discovery from an
independent per-function pass to a deterministic fixed point. Once a
source-local `fn` has been accepted, later candidate passes may use it inside
another accepted source-function body. Exact calls to those accepted
source-local candidates lower as runtime `Apply` expressions, including named
arguments emitted in declaration input order.
The ninth follow-up tightens the rejection boundary for source functions that
type-check as function values but have no executable runtime candidate. Sema
now records top-level function path references as typed lowering evidence, and
checked runtime-plan lowering rejects those references with
`source_function_value_without_runtime_candidate` when the function is neither
a pure helper nor an accepted source-function candidate. This prevents an
unsupported callable from falling through as `RuntimeExpr::Local`.

## Accepted Contract

The accepted family is intentionally small:

- `FunctionKind::Function` only.
- One or more parameter groups.
- Fixed parameters only; no rest/default/receiver parameters.
- Source function declaration parameter patterns must be simple identifier
  bindings.
- Body must be a final expression or final `return` expression, optionally
  preceded by simple `let` statements whose RHS is an accepted value
  expression.
- Body expressions must lower through strict runtime expression lowering.
  Calls are accepted only when they are local function-value calls, exact calls
  to already-lowered pure helpers resolved through `RuntimePureHelperLookup`,
  or exact calls to source-local `fn` candidates that were already accepted by
  the fixed-point candidate pass. Host/adapter calls,
  effectful calls, suspending calls, pipes, `await`, `try`, threads, dialogue
  calls, placeholders, raw syntax, and lifetime paths remain outside this
  subset. Pure value control expressions are accepted when their child
  expressions are accepted: `if`, `if let`, and `match` lower to
  `RuntimeExpr::If`, `RuntimeExpr::IfLet`, and `RuntimeExpr::Match`.
- Closure literal expressions are accepted when their parameter patterns do not
  bind names that shadow known function-valued locals and their body
  recursively satisfies this accepted contract. Simple identifier parameters
  lower directly; destructuring parameters lower through synthetic runtime
  arguments plus `RuntimeExpr::Match`.
- Direct calls to parameters whose declared type is a function type are
  accepted when all provided arguments are positional accepted expressions and
  the call does not exceed the declared function arity.
- Simple `let` bindings become local function values inside the accepted body
  when their expression is a function-typed parameter alias, a simple closure
  literal, a destructuring closure literal, or a partial call to an existing
  local function value whose result type is still a function.
- `let`, `if let`, `match`, and closure parameter patterns that would shadow a
  function-typed parameter name keep the source function outside this accepted
  subset.

Accepted functions lower to `RuntimeExpr::Function` values. Curried groups
lower to nested functions so evaluating an inner group captures earlier group
arguments through the existing runtime closure environment. Direct calls lower
to `RuntimeExpr::Apply`. Named missing-input partial calls in the current group
synthesize a wrapper function whose parameters are the missing inputs and whose
body applies the materialized source function with arguments in declaration
order.
Direct calls to function-typed parameters lower as `RuntimeExpr::Apply` with a
local callee, preserving higher-order source functions without pretending the
call is an adapter or top-level runtime call. Function-valued `let` bindings
lower as ordinary `RuntimeExpr::Let` values, and later calls to those locals
also lower as local `RuntimeExpr::Apply`. Exact pure-helper calls lower through
the same strict named-call lowering as flow expressions, so named helper
arguments are emitted in helper input order rather than source order.
Exact calls to already-accepted source-local candidates use the same
declaration-order argument lowering and materialized `RuntimeExpr::Function`
value path, but remain exact-only inside source-function bodies; missing-input
source-call partials inside those bodies are still outside this cut.
Value-producing control expressions inside the accepted body preserve their
runtime shape. Guarded `if let` and `match` expressions bind pattern locals
before checking guards, matching statement control-flow semantics.

Pure helpers keep priority when a function is also accepted by the pure-helper
candidate pass. Local function-valued bindings keep priority over both
top-level families.

## Behavior

Function-value creation is effect-free. The accepted subset does not contain
host/effect/suspension syntax outside recursively accepted closure literals,
direct calls to supplied function values, and local aliases/partials of those
function values, plus exact pure-helper calls whose helper bodies were already
accepted by the pure-helper pass and exact source-local candidate calls whose
bodies already satisfy this same contract, so creating the value cannot perform
hidden host, adapter, or suspension work in this cut. Calling a supplied or
materialized function value composes that value's behavior at invocation time
through the existing runtime `Apply` path.
Returning a closure only allocates another `RuntimeExpr::Function`; its body is
still constrained by the same accepted subset.
Destructuring closure parameters do not widen the callable family by
themselves; they are a runtime-local destructuring step over the value supplied
to the closure, and unsupported body syntax remains unsupported.

Product AWBC save/load behavior is unchanged: any escaped runtime
`RuntimeValue::Function` is still rejected by the existing structured
unsupported-runtime-value path.

## Remaining 07.7 Boundaries

These are still not accepted:

- source function values whose bodies contain host/adapter/effect calls, calls
  to source-local functions outside the accepted candidate set, pipes, closure
  bodies with host/effect calls, `await`, `try`, or other suspension-capable
  constructs;
- statement-style control flow such as loops, `while`, `for`, branch
  statements, and statement blocks that cannot be represented as strict
  runtime value expressions;
- `task fn`, `dialogue fn`, and `stream fn` values;
- trait/impl method values and receiver binding extraction;
- adapter/host-call-backed callable thunks;
- persisted source function values.

Unsupported signature partial calls still fail as
`signature_partial_without_helper` when no pure helper or accepted source
function candidate exists.
Bare top-level source-function value references outside the same executable
families fail as `source_function_value_without_runtime_candidate`.

## Validation

```bash
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_named_missing_source_function_partial_call -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_curried_source_function_value -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_returned_closure -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_callback_param_call -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_callback_partial_let -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_destructured_closure_let -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_pure_helper_call_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_exact_source_call_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_control_expression_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_if_let_expression_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_source_function_partial_when_body_calls -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_bare_source_function_value_when_body_calls -- --nocapture
cargo test -p arcweft-compiler --all-features runtime_plan_lowers_non_annotated_function_prefix_partial_with_typecheck -- --nocapture
```

2026-07-09 status-cleanup validation for the destructured closure local-alias
follow-up:

```bash
rustfmt --edition 2024 --check crates\arcweft-runtime-plan\src\function_values.rs crates\arcweft-compiler\src\tests.rs
git diff --check -- crates\arcweft-runtime-plan\src\function_values.rs crates\arcweft-compiler\src\tests.rs docs\implementation\2026-07-07-functions-closures-pipeline-language-stack.md docs\implementation\current-work-status-2026-07-09.md docs\implementation\function-stack-goal-completion-audit-2026-07-08.md docs\implementation\function-stack-non-helper-source-function-values-2026-07-09.md docs\implementation\function-stack-status-rollup-2026-07-09.md docs\reviews\requests\2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_destructured_closure_let -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_callback_partial_let -- --nocapture
cargo check -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

All commands passed. Clippy still reports the existing large-enum warnings in
`arcweft-lang-syntax` and the existing `too_many_lines` warning in
`arcweft-lang-sema::semantic::analyze_stmt`; no new warning is attributed to
this slice. The structure audit scanned 2464 files / 1176 Rust files /
581407 Rust physical LOC and reported 0 errors / 151 warnings.

Structural measurement at revision `a19fe72e3` before this cleanup commit:

| Path | Crate | Bytes | Physical LOC | Classification | Embedded test LOC | Responsibilities |
| --- | --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-runtime-plan/src/function_values.rs` | `arcweft-runtime-plan` | 17269 | 491 | production | 0 | accepted runtime function-value family classification and source-local function materialization support |
| `crates/arcweft-compiler/src/tests.rs` | `arcweft-compiler` | 109501 | 3283 | unit-test module | 3283 | compiler/runtime-plan regression fixtures |

Largest workspace Rust files measured at the same checkout, unchanged by this
slice:

| Path | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357456 | 12394 | production generated/lookup-heavy vertical text data |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255354 | 7443 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 243053 | 6285 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222475 | 5760 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 222425 | 5659 | integration test |

2026-07-09 exact source-local candidate call validation:

```bash
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_exact_source_call_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_pure_helper_call_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_source_function_partial_when_body_calls -- --nocapture
cargo check -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
rustfmt --edition 2024 --check crates\arcweft-runtime-plan\src\expr.rs crates\arcweft-runtime-plan\src\expr\enum_constructor.rs crates\arcweft-runtime-plan\src\expr\named_callable.rs crates\arcweft-runtime-plan\src\flow.rs crates\arcweft-runtime-plan\src\function_values.rs crates\arcweft-runtime-plan\src\source.rs crates\arcweft-runtime-plan\src\stream.rs crates\arcweft-compiler\src\tests.rs
git diff --check -- crates\arcweft-runtime-plan crates\arcweft-compiler\src\tests.rs docs\implementation docs\reviews\requests\2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs\implementation\structure-audits\function-stack-source-function-exact-call-fixed-point-2026-07-09
```

All commands passed. Clippy still reports only existing large-enum warnings in
`arcweft-lang-syntax` and the existing `too_many_lines` warning in
`arcweft-lang-sema::semantic::analyze_stmt`. The structure audit scanned 2474
files / 1179 Rust files / 582815 Rust physical LOC and reported 0 errors /
151 warnings.

Structural measurement after the exact source-local candidate call cut:

| Path | Crate | Bytes | Physical LOC | Classification | Embedded test LOC | Responsibilities |
| --- | --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-runtime-plan/src/expr.rs` | `arcweft-runtime-plan` | 77151 | 2187 | production | 0 | strict runtime expression lowering and helper/function-value lookup with independent source-candidate lifetimes |
| `crates/arcweft-runtime-plan/src/expr/enum_constructor.rs` | `arcweft-runtime-plan` | 2772 | 98 | production | 0 | enum constructor strict lowering over the widened helper lookup type |
| `crates/arcweft-runtime-plan/src/expr/named_callable.rs` | `arcweft-runtime-plan` | 4703 | 142 | production | 0 | declaration-order named callable lowering for pure helpers and runtime function-value candidates |
| `crates/arcweft-runtime-plan/src/flow.rs` | `arcweft-runtime-plan` | 90757 | 2485 | production | 0 | runtime-plan flow lowering with helper/evidence/function-value lookup lifetimes separated |
| `crates/arcweft-runtime-plan/src/function_values.rs` | `arcweft-runtime-plan` | 21199 | 644 | production | 0 | fixed-point accepted source-local function candidate discovery and exact source-candidate call admission |
| `crates/arcweft-runtime-plan/src/source.rs` | `arcweft-runtime-plan` | 8097 | 222 | production | 0 | source plan lowering over the widened helper lookup type |
| `crates/arcweft-runtime-plan/src/stream.rs` | `arcweft-runtime-plan` | 4318 | 119 | production | 0 | stream plan lowering over the widened helper lookup type |
| `crates/arcweft-compiler/src/tests.rs` | `arcweft-compiler` | 130635 | 4019 | test | 4019 | compiler/runtime-plan function-stack regression fixtures |

2026-07-09 exact pure-helper call validation:

```bash
rustfmt --edition 2024 --check crates\arcweft-runtime-plan\src\expr.rs crates\arcweft-runtime-plan\src\function_values.rs crates\arcweft-runtime-plan\src\flow.rs crates\arcweft-runtime-plan\src\flow\pure_helpers.rs crates\arcweft-compiler\src\tests.rs
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_pure_helper_call_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_rejects_source_function_partial_when_body_calls -- --nocapture
cargo check -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check -- crates\arcweft-runtime-plan\src\expr.rs crates\arcweft-runtime-plan\src\function_values.rs crates\arcweft-runtime-plan\src\flow.rs crates\arcweft-runtime-plan\src\flow\pure_helpers.rs crates\arcweft-compiler\src\tests.rs docs\implementation\2026-07-07-functions-closures-pipeline-language-stack.md docs\implementation\current-work-status-2026-07-09.md docs\implementation\function-stack-current-gap-map-2026-07-09.md docs\implementation\function-stack-goal-completion-audit-2026-07-08.md docs\implementation\function-stack-non-helper-source-function-values-2026-07-09.md docs\implementation\function-stack-status-rollup-2026-07-09.md docs\reviews\requests\2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md
```

All commands passed. Clippy still reports only the existing large-enum warnings
in `arcweft-lang-syntax` and the existing `too_many_lines` warning in
`arcweft-lang-sema::semantic::analyze_stmt`. The structure audit scanned 2467
files / 1177 Rust files / 581600 Rust physical LOC and reported 0 errors /
151 warnings. During this cut, `flow.rs` briefly crossed the production-file
error threshold; pure-helper inventory construction was split into
`flow/pure_helpers.rs`, bringing `flow.rs` back below the 2,500 LOC error
threshold.

Structural measurement after the exact pure-helper call cut:

| Path | Crate | Bytes | Physical LOC | Classification | Embedded test LOC | Responsibilities |
| --- | --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-runtime-plan/src/expr.rs` | `arcweft-runtime-plan` | 88235 | 2483 | production | 0 | strict runtime expression lowering, helper lookup, function/partial/call expression lowering |
| `crates/arcweft-runtime-plan/src/flow.rs` | `arcweft-runtime-plan` | 91315 | 2485 | production | 0 | runtime-plan flow lowering, optimization, entry lowering, agent-controller lowering |
| `crates/arcweft-runtime-plan/src/flow/pure_helpers.rs` | `arcweft-runtime-plan` | 751 | 23 | production | 0 | runtime pure-helper inventory and lookup-id construction for flow lowering |
| `crates/arcweft-runtime-plan/src/function_values.rs` | `arcweft-runtime-plan` | 19768 | 604 | production | 0 | accepted runtime function-value family classification, source-local function materialization, pure-helper exact-call admission |
| `crates/arcweft-compiler/src/tests.rs` | `arcweft-compiler` | 113891 | 3622 | unit-test module | 3622 | compiler/runtime-plan regression fixtures |

2026-07-09 value-control expression validation:

```bash
rustfmt --edition 2024 --check crates\arcweft-lang-syntax\src\parser\control_flow.rs crates\arcweft-lang-syntax\src\parser\statements.rs crates\arcweft-lang-sema\src\checker\expr.rs crates\arcweft-lang-sema\src\tests\control_flow.rs crates\arcweft-compiler\src\tests.rs
git diff --check -- crates\arcweft-lang-syntax\src\parser\control_flow.rs crates\arcweft-lang-syntax\src\parser\statements.rs crates\arcweft-lang-sema\src\checker\expr.rs crates\arcweft-lang-sema\src\tests\control_flow.rs crates\arcweft-compiler\src\tests.rs docs\implementation\function-stack-non-helper-source-function-values-2026-07-09.md docs\implementation\function-stack-current-state-2026-07-09.md docs\implementation\function-stack-status-rollup-2026-07-09.md docs\implementation\function-stack-current-gap-map-2026-07-09.md docs\implementation\2026-07-07-functions-closures-pipeline-language-stack.md docs\reviews\requests\2026-07-08-seq-07.7-function-stack-non-helper-callable-allocation.md
cargo test -p arcweft-lang-sema --all-features value_if_let_guard_can_use_pattern_binding -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_control_expression_body -- --nocapture
cargo test -p arcweft-compiler --all-features checked_runtime_plan_materializes_source_function_if_let_expression_body -- --nocapture
cargo test -p arcweft-lang-syntax --all-features --test parser_flow_statements_and_body -- --nocapture
cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

All commands passed. Clippy still reports only existing large-enum warnings in
`arcweft-lang-syntax` and existing `too_many_lines` warnings in
`arcweft-lang-sema`. The structure audit scanned 2468 files / 1177 Rust files /
581839 Rust physical LOC and reported 0 errors / 151 warnings.

Structural measurement before the value-control expression commit, at parent
revision `ea8619bb` with the current working-copy slice applied:

| Path | Crate | Bytes | Physical LOC | Classification | Embedded test LOC | Responsibilities |
| --- | --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-lang-syntax/src/parser/control_flow.rs` | `arcweft-lang-syntax` | 44695 | 1114 | production | 0 | structured control-flow and value-control expression parsing |
| `crates/arcweft-lang-syntax/src/parser/statements.rs` | `arcweft-lang-syntax` | 30761 | 842 | production | 0 | statement parsing, let binding value parsing, control-transfer value parsing |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | `arcweft-lang-sema` | 94840 | 2362 | production | 0 | expression type checking, branch expression typing, function-value expression checking |
| `crates/arcweft-lang-sema/src/tests/control_flow.rs` | `arcweft-lang-sema` | 41915 | 1329 | unit-test module | 1329 | control-flow parsing/type-checking regressions |
| `crates/arcweft-compiler/src/tests.rs` | `arcweft-compiler` | 122902 | 3560 | unit-test module | 3560 | compiler/runtime-plan regression fixtures |

Largest workspace Rust files measured at the same checkout, unchanged by this
slice:

| Path | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357456 | 12394 | production generated/lookup-heavy vertical text data |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255354 | 7443 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 243053 | 6285 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222475 | 5760 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 222425 | 5659 | integration test |
