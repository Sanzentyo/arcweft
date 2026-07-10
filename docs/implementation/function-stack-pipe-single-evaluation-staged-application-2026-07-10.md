# Pipe single evaluation and staged application — 2026-07-10

## Corrected semantic contract

The earlier implementation rewrote a no-`^` pipe by appending the left
expression to an existing RHS call group. It also replaced every `^` node with
a clone of the authored left expression. Those rewrites were observably wrong:

- `x |> f(a)` became `f(a, x)` instead of `f(a)(x)`;
- a curried declaration could be rejected as a flattened call group;
- two `^` occurrences evaluated an effectful left expression twice; and
- a `^` captured by an RHS closure captured an expression to run later rather
  than the value produced when the pipe itself ran.

The corrected contract is:

```arcw
x |> f(a)       // f(a)(x)
x |> use        // use(x)
value |> (^, ^) // one evaluation of value, two reads
```

Runtime-plan lowering owns one impossible-to-author lexical binding for each
active pipe depth. It emits the left expression as the binding initializer and
lowers each `^` as a read of that binding. A closure in the RHS therefore
captures the produced value through the ordinary runtime closure path. Nested
pipe RHS scopes shadow the outer binding; a nested pipe LHS remains in the
outer RHS scope.

For a no-`^` pipe, lowering first constructs the RHS callable expression and
the left expression in the same order used by typed-evidence traversal, then
emits a lexical binding whose runtime evaluation order is left value, RHS
callable, one-argument apply. Existing RHS call groups are never merged with
that final apply.

Method-chain fallback uses the same model after inherent and trait resolution:
`receiver.method(args...)` lowers as `method(args...)(receiver)`. Named and
fixed-spread evidence belongs to the first stage; the receiver is a distinct
second stage. Candidate ambiguity remains a semantic error.

## Evidence and regression boundary

The syntax-owned `Expr::contains_pipe_left` traversal is the authority for pipe
placeholder scope. Sema no longer clones a desugared AST to infer a pipe.
Regression coverage distinguishes staged `f(a)(b)` from grouped `f(a, b)`,
checks curried source callables and method fallback, checks `^` inside a
closure/control expression, inspects the single lexical runtime binding, and
executes lowered AWBC with a counting intrinsic to prove two `^` reads call the
left producer once.

This is a breaking correction to an unreleased language/runtime contract. No
compatibility alias, legacy lowering branch, serializer version bump, or
migration shim is retained.

## Validation

- syntax pipe-scope focused tests: 2 passed;
- sema function-stack tests, including pipe, source-range provenance, and
  curried method fallback: 111 passed;
- runtime-plan strict expression tests: 32 passed;
- compiler pipe tests: 8 passed;
- compiler method-fallback tests: 3 passed;
- counting-intrinsic AWBC exact-once regression: 1 passed;
- all-target/all-feature check and strict clippy for syntax, sema,
  runtime-plan, compiler, LSP, verify, and the OxiZ adapter: passed;
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: zero
  error-level findings.
