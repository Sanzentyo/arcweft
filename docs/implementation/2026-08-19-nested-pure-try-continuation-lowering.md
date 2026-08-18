# Nested pure Try continuation lowering — 2026-08-19

## Inspected state

- Base Git revision: `a15e122d0a` (`Establish generic Try callable facts`).
- Branch: `main`, initially matching `origin/main`.
- This cut continues
  [Generic Try checked boundary progress](2026-08-18-generic-try-checked-boundary-progress.md)
  and follows the temporary order in
  [Post-Try convergence implementation order](2026-08-18-post-try-convergence-order.md).

## Performed

- Replaced the direct terminal-function-site-only Try lowering with one private
  pure-expression continuation algebra owned by `FinalExprLowerer`.
- Lowered every admitted Try operand exactly once into a typed Match. The
  success arm continues with the unwrapped payload; the residual arm exits to
  the checked callable/function-site boundary or the nearest checked
  `result {}` / `option {}` carrier boundary.
- Preserved source evaluation order through function and ordinary block
  `let`/assignment statements, strict expression children, and pipe overrides.
  The pipe-left value is bound once before a Try-containing pipe body begins.
- Preserved lazy control locality for If, Match, IfLet, and the short-circuit
  `and`, `or`, and implication operators. A Try in one branch is not hoisted
  into another branch or evaluated before the controlling expression.
- Reused the checked Try carrier, boundary, expression type, pattern, and
  builder-issued local facts. No early-return RuntimeExpr, spelling resolver,
  fallback carrier, or duplicated Try fact table was added.
- Kept normal Try typing unchanged. In an ordinary function returning
  `Result<T, E>`, terminal `try value` still has type `T`; it is not implicitly
  wrapped. Success wrapping is specific to the contextual implicit callable
  boundary already admitted for `try _`.

## Executable evidence

`crates/arcweft-compiler/tests/try_pipe.rs` now compiles source-authored cases
for:

- two sequential Result Try operations in one pure helper;
- Result and Option carrier blocks catching their nearest Try residual;
- a Try inside an ordinary nested block;
- Try remaining local to If branches and Match arms; and
- the existing once-only pipe-left binding; plus
- a negative ordinary-function tail proving `try value: T` is not implicitly
  wrapped into the function's `Result<T, E>` return type.

The focused test also executes the admitted pure helpers through
`VmPureFunctionScratch`: sequential success values produce `Ok(41)`, the first
residual exits unchanged, and a Try-containing pipe body produces `Ok(41)`
after its left binding.

The sequential Try plan is also lowered through `AwbcLowerer`; canonical
Product AWBC verification accepts the resulting helper CFG without a new
opcode or Try-specific product representation.

That verification exposed and closed a pre-existing closure-frame correlation
gap: pending AWBC closures now give each capture/parameter slot the same typed
local-derived string identity carried by `MakeFunction`. The verifier therefore
checks real names instead of comparing the instruction metadata with unnamed
frame slots.

## Passed

- `cargo check -p arcweft-runtime-plan --lib`.
- `cargo clippy -p arcweft-runtime-plan --lib --all-features --no-deps -- -D warnings`.
- `cargo test -p arcweft-compiler --test try_pipe`: 8 passed, 0 failed.
- `cargo test -p arcweft-runtime-plan --lib`: 47 passed, 0 failed.
- `cargo test -p arcweft-compiler --lib`: 52 passed, 0 failed.
- `cargo fmt --all -- --check`.
- `git diff --check`.
- `just structure-audit`: 0 blocking violations.
- `just structure-audit-gate`: 0 blocking violations.

## Structural review

`crates/arcweft-runtime-plan/src/final_expr.rs` is now 1,918 physical lines and
therefore reports non-blocking `SIZE001`. The added code remains one cohesive
private responsibility: converting admitted pure Try facts into the existing
RuntimeExpr seed algebra while sharing the lowerer's exact locals, overrides,
patterns, and type facts. It does not create a second authority.

The continuation algebra is nevertheless a viable private submodule boundary.
Physically splitting it is an independent structural cleanup once the current
behavioral cut is committed; it must not duplicate `FinalExprLowerer`, its
fact maps, or expression reconstruction.

## Remaining work

1. Try in a Match or IfLet guard remains fail-closed. Correct support requires
   guard-local continuation lowering after pattern bindings are established;
   hoisting it before the pattern would change scope and evaluation order.
2. The Await-containing item-1 matrix remains blocked on the unary `Need<T>`
   replacement and generic Await-value lowering. The binary Need/direct-host
   path is not repaired here.
3. Product AWBC lowering and verification are covered. Full structured-vs-AWBC
   execution parity for Result and Option residual paths remains a later
   executable tier.

## Not performed

- No postfix-question syntax, TryAwait, TryPipe, TryPartial, binary-Need
  compatibility route, or new versioned boundary was added.
- No full-workspace strict Clippy claim is made. The changed runtime-plan crate
  passed its strict no-dependency tier; unchanged workspace warnings recorded
  by earlier cuts remain outside this cut.
