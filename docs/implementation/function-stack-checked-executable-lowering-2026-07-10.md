# Function stack: checked executable lowering — 2026-07-10

## Scope

This cut removes silent value changes at source, stream, flow return, effect,
and host-request runtime-plan boundaries. Runtime values used by typed effects
remain expressions until execution and are materialized into adapter requests
only after evaluation.

## Result

- Stream-function and source-handler statement lowering is fallible. Every
  unsupported `Stmt` variant is named explicitly and returns a structured
  `RuntimePlanLowerError`; the provisional `StreamOp::Noop` and
  `SourceOp::Noop` variants were removed from the core model rather than kept
  as dead compatibility surface.
- Statements authored directly in a declarative `source` body are rejected;
  executable statements must belong to an `on` handler. Source parser ranges
  now retain document-absolute offsets through handler blocks and actions.
- Errors identify the runtime-plan owner, nested statement path, statement
  kind, expression or pattern role, and the authored expression range when HIR
  retains it. `RuntimePlanLowerError::diagnostic` projects that range to a
  primary `SourceSpan`. Checked pattern lowering rejects raw recovery syntax
  and non-literal literal patterns instead of stringifying them.
- Stream final body expressions are rejected. They were previously stored in
  `HirFunction::value` but omitted from `StreamPlan`, which silently discarded
  authored execution.
- Stream/source expressions no longer retry through lossy expression lowering.
  Pure-value lowering rejects `try` and `await`; their behavior must be owned
  by an error-propagation or suspension-aware statement boundary.
- A stream `return` lowers only when its value is `Unit`, matching the
  value-free `StreamOp::Return` representation rather than discarding an
  arbitrary value.
- Syntax owns one borrowed `SourceHeaderInventory` used by sema and
  runtime-plan lowering. Singular headers are scanned once; duplicates are
  rejected at the second value range. Missing and unknown overflow values are
  distinct typed recovery cases. Both boundaries reject missing headers,
  unknown policy spellings, non-integer or zero bounded capacities, invalid
  overflow policies, and `privacy = private` with `replay = full`; no capacity
  or overflow default is manufactured.
- Ordinary flow returns use the checked, error-recording expression boundary.
- Dynamic arguments for typed built-in log, signal, metric, event, assertion,
  and failure effects lower as `RuntimeExpr` values and are evaluated by both
  structured and AWBC runtimes. Unsupported dynamic effect shapes fail closed
  instead of becoming source-text payloads.
- AWBC verification checks static payload shape, dynamic arity, and assertion
  profile before a product executor is constructed. Product execution now
  relies on that canonical verifier; the obsolete always-empty parity-blocker
  inventory and its tautological tests were removed.
- Host request construction is fallible. Non-call await targets and
  unlowerable positional, named, or spread arguments retain typed target or
  capability/operation/argument context instead of becoming synthetic
  `await.expr` requests or string payloads. Agent Prelude requests propagate
  the same failures through nested predicate and viewport/path shapes.
- Flow signatures separate their top-level `effects { ... }` contract from the
  return type before type parsing. This preserves the documented
  `flow f() -> T effects { ... }` form without weakening the rule that an
  effect row attached to a type must annotate a function type.
- Tooling resolves analyzer-owned open effect rows through
  `TypeCheckReport::resolved_type` before rendering inferred function-valued
  `let` inlays. Internal `eN` variables remain available in raw sema evidence
  but no longer leak into the ordinary LSP type surface.

The intentionally lossy `lower_runtime_expr` remains available only for
adapter-facing labels and legacy non-executable data models. The executable
positions changed by this cut do not call it after checked lowering fails.

Flow optimization, pure-call counting, and local-use analysis were moved from
`flow.rs` to `flow/optimizer.rs`, with expression-use counting in the private
`flow/optimizer/usage.rs` child. Both modules remain private to the flow
lowerer, which exposes one finalization entry point rather than a pass-through
facade.

## Verification

Passing focused coverage includes:

```bash
cargo check -p arcweft-runtime-plan --all-targets --all-features
cargo test -p arcweft-runtime-plan --lib --all-features
cargo test -p arcweft-runtime-plan --lib --all-features host_request_
cargo test -p arcweft-runtime-plan --lib --all-features strict_runtime_rejects_try_and_await_without_control_boundaries
cargo test -p arcweft-runtime-plan --lib --all-features checked_pattern_lowering_rejects_raw_recovery_syntax
cargo test -p arcweft-runtime-plan --test runtime_plan --all-features runtime_plan_rejects_unsupported_
cargo test -p arcweft-runtime-plan --test runtime_plan --all-features runtime_plan_rejects_discarded_stream_final_value
cargo test -p arcweft-runtime-plan --test runtime_plan --all-features stream_expression_failure_preserves_role_and_authored_range
cargo test -p arcweft-runtime-plan --test runtime_plan --all-features source_header_expression_failure_is_not_stringified
cargo test -p arcweft-runtime-plan --test runtime_plan --all-features source_
cargo test -p arcweft-runtime-plan --test runtime_plan --all-features runtime_plan_lowers_stream_and_source_plans_separately_from_flow_ops
cargo test -p arcweft-runtime-plan --test runtime_plan --all-features stream_unit_return_remains_executable
cargo test -p arcweft-lang-sema --lib source_
cargo clippy -p arcweft-runtime-plan --all-targets --all-features --no-deps -- -D warnings
cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema --all-targets --all-features --no-deps -- -D warnings
```

The final runtime-plan suites passed with 95 library tests, 62 runtime-plan
integration tests, and 58 AWBC product-parity tests. Source-focused sema
coverage passed with 38 tests. The complete normal workspace route also
passed, including 487 sema tests, 123 compiler tests, and 92 LSP tests.

Final checkout validation:

```bash
just test-workspace
cargo check --workspace --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/function-stack-checked-executable-lowering-2026-07-10
```

The structural audit scanned 2,520 files, including 1,175 Rust files and
597,937 Rust physical LOC, and reported 0 errors / 149 warnings.

## Remaining work

No compatibility layer is retained for the discarded fallback behavior.
Callers that require `try`, `await`, a new stream/source statement, or a new
host argument shape must add an explicit runtime representation and direct
success/rejection tests.
