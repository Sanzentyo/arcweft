# Contextual `choice` keyword positive-fixture closure

Date: 2026-08-21

Inspected baseline: `c91a5f5ee`

Working-tree state at validation: dirty only with the expression/pattern
grammar, focused tests, and documentation in this cut.

## Performed

- Changed Pratt prefix dispatch so the `choice` spelling enters the dedicated
  Choice expression parser only when the token stream carries Choice grammar
  evidence: a static entity ID, body introducer, or lifecycle `with` head.
- Preserved `choice.label` as an ordinary keyword-like path followed by the
  existing typed Select postfix. No identifier string rewrite or detached
  parser was introduced.
- Admitted `choice` as a contextual binding spelling in Pattern binding
  position while retaining all other reserved-keyword binding rejection.
- Added focused tests proving both sides of the boundary: `choice.label`
  remains Select, and `choice @.first { ... }` remains one Choice expression.

## Passed

- `cargo fmt --all -- --check`
- `cargo check -p arcweft-lang-syntax --all-targets --all-features`
- `cargo test -p arcweft-lang-syntax --all-features --no-fail-fast`
  - 671 unit tests, 1 public-API test, 3 public-parser tests, and 2 compile-fail
    doc tests passed.
- focused contextual binding and Choice/Select dispatch tests: 2 passed.
- `cargo run -p arcweft-cli -- check tests/fixtures/arcw/current_pass/check/012_function_final_expr.arcw`
  - check and verification passed with one flow and no warnings or obligations.
- `git diff --check`

## Failed

- `cargo clippy -p arcweft-lang-syntax --all-targets --all-features --no-deps -- -D warnings`
  - stopped on five existing attachment/size/line-count warnings outside this
    cut.
- The deterministic next fixture,
  `current_pass/check/013_task_fn_await_shape.arcw`, reaches final semantic
  analysis and fails shared callable resolution. This is the next positive
  gate.

## Explicit non-goals

- The complete reserved-word policy is not widened. Only the already
  contextual `choice` spelling is admitted at the two unambiguous positions
  established by the fixture and the dedicated Choice grammar evidence.
- Malformed actual Choice statements continue through their statement-owned
  recovery path; this change only corrects ordinary expression prefix
  classification.
