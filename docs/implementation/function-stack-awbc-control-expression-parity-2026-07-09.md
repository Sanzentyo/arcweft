# Function Stack AWBC Control Expression Parity - 2026-07-09

## Status

Implemented and ready for review as part of the active
function/closure/currying/pipeline goal.

This cut removes the remaining eager `select.bool` / `match.value` lowering
shape for value-position AWBC control expressions that can appear inside
runtime function bodies. `RuntimeExpr::If`, `RuntimeExpr::IfLet`, and
`RuntimeExpr::Match` now lower to real AWBC branch blocks, pattern tests,
pattern scopes, jumps, returns, and pattern-mismatch traps. This matters for
the function-stack goal because destructuring closure parameters lower through
`RuntimeExpr::Match`, and accepted source-local function values may now contain
local closure aliases whose bodies depend on that match executing lazily and
binding only the selected arm.

## Implemented

- `RuntimeExpr::If` / `IfLet` / `Match` lower through synthetic
  non-suspending AWBC functions and `ApplyFunction`, so the value expression can
  return from branch-specific AWBC blocks instead of evaluating every branch
  eagerly.
- Closure/pending-function bodies can now own multiple AWBC blocks, not only a
  single return block. Function metadata records the whole block range.
- Pattern candidates reserve frame slots while lowering the pattern graph, then
  restore the frame scope depth until a branch is actually selected.
- Selected `if let` / `match` branches enter an AWBC lexical scope, bind the
  matched pattern, run guard/value code with the bindings visible, move the
  returned value to a root temporary when needed, and exit the scope before
  returning.
- Guard-false paths exit their temporary pattern scope and jump to the next
  candidate/else branch.
- Flow-level `if`, `if let`, `let else`, and `match` now use AWBC
  `Branch`/`Jump`/`Trap` control blocks instead of running branch bodies
  linearly. The existing product-step parity tests now prove only the selected
  branch emits effects.
- Entry-parameter free-local collection now treats pattern guard expressions as
  inside the selected pattern binding scope. This prevents guard reads of
  pattern-bound names from being misclassified as root entry parameters.
- Closure capture selection is narrower: generated function values capture only
  frame locals that are actually free in the function/control-expression body,
  instead of capturing every local frame slot except parameters.

## Remaining 07.5 Boundary

This does not implement suspension-aware dynamic apply. The generated AWBC
functions remain non-suspending expression functions. If a dynamic function
apply suspends or budget-yields, the existing 07.5 request boundary still
applies.

This also does not design persisted closure/function snapshots. Product AWBC
save/load continues to reject escaped function values with the existing
structured unsupported-runtime-value path.

## Validation

```bash
cargo test -p arcweft-runtime-plan --all-features --test awbc_product_parity -- --nocapture
cargo check -p arcweft-runtime-plan --all-targets --all-features
cargo clippy -p arcweft-runtime-plan --all-targets --all-features
rustfmt --edition 2024 --check crates\arcweft-runtime-plan\src\awbc_lower\expr.rs crates\arcweft-runtime-plan\src\awbc_lower\flow.rs crates\arcweft-runtime-plan\src\awbc_lower\frame.rs crates\arcweft-runtime-plan\src\awbc_lower\pattern.rs crates\arcweft-runtime-plan\src\line_task.rs crates\arcweft-runtime-plan\tests\awbc_product_parity.rs
git diff --check -- crates\arcweft-runtime-plan\src\awbc_lower\expr.rs crates\arcweft-runtime-plan\src\awbc_lower\flow.rs crates\arcweft-runtime-plan\src\awbc_lower\frame.rs crates\arcweft-runtime-plan\src\awbc_lower\pattern.rs crates\arcweft-runtime-plan\src\line_task.rs crates\arcweft-runtime-plan\tests\awbc_product_parity.rs
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

All commands passed. Clippy still reports only the existing warnings in
`arcweft-lang-syntax` and `arcweft-lang-sema`. The structure audit scanned
2464 files / 1176 Rust files / 581407 Rust physical LOC and reported
0 errors / 151 warnings.

Focused parity tests added in this cut include:

- `awbc_product_parity_if_executes_only_selected_branch`
- `awbc_product_parity_nested_else_if_executes_only_selected_branch`
- `awbc_product_parity_if_let_guard_binds_before_guard`
- `awbc_product_parity_let_else_skips_else_when_pattern_matches`
- `awbc_product_parity_match_executes_only_selected_guarded_arm`
- `awbc_product_parity_match_guard_false_continues_to_next_arm`
- `awbc_product_parity_if_let_expression_guard_binds_before_guard`
- `awbc_product_parity_match_expression_executes_selected_guarded_arm`

## Structural Audit Notes

Revision before this cut: `917f3a1ad`.

Changed files:

| Path | Crate | Bytes | Physical LOC | Classification | Embedded test LOC | Responsibilities |
| --- | --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-runtime-plan/src/awbc_lower/expr.rs` | `arcweft-runtime-plan` | 43626 | 1144 | production | 0 | runtime expression to AWBC instruction/block lowering, generated expression function bodies, free-local capture collection |
| `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs` | `arcweft-runtime-plan` | 79345 | 2050 | production | 0 | flow op to AWBC lowering, branch block construction, resume/block metadata, entry parameter collection |
| `crates/arcweft-runtime-plan/src/awbc_lower/frame.rs` | `arcweft-runtime-plan` | 5932 | 183 | production | 0 | AWBC frame slot and lexical scope-depth allocation |
| `crates/arcweft-runtime-plan/src/awbc_lower/pattern.rs` | `arcweft-runtime-plan` | 5956 | 153 | production | 0 | runtime pattern to AWBC pattern graph lowering and binding-name collection |
| `crates/arcweft-runtime-plan/src/line_task.rs` | `arcweft-runtime-plan` | 30265 | 778 | production | 0 | line-task runtime lowering |
| `crates/arcweft-runtime-plan/tests/awbc_product_parity.rs` | `arcweft-runtime-plan` | 73312 | 2061 | integration test | 2061 | structured-vs-product AWBC runtime parity fixtures |

Largest workspace Rust files measured at the same checkout, unchanged by this
slice:

| Path | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `crates/arcweft-text-layout/src/vertical_orientation.rs` | 357456 | 12394 | production generated/lookup-heavy vertical text data |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255354 | 7443 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 243053 | 6285 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 222475 | 5760 | integration test |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 222425 | 5659 | integration test |

`awbc_lower/flow.rs` remains a warning-level production hotspot below the
2,500 LOC error threshold. The branch-lowering work is cohesive with the
existing flow-body builder and product parity harness; a future split should
move reusable branch-block helpers into a dedicated AWBC control-flow module
rather than creating compatibility wrappers.
