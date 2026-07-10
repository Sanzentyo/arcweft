# Numeric Literal Final Model — 2026-07-10

## Outcome

Arcweft numeric literals now remain exact from syntax through checked runtime
lowering. Integer syntax owns the authored spelling, radix, and typed suffix;
its non-negative magnitude is parsed as `u128` only when semantic or lowering
work needs it. Invalid digits and magnitudes beyond `u128` are no longer
silently replaced with zero.

Semantic analysis resolves every integer, float, and compact numeric-sequence
literal from an explicit suffix, an expected numeric type, or the stable
`i32`/`f64` fallback. The resolved primitive is stored as typed lowering
evidence and consumed by runtime-plan and compiler lowering. This keeps, for
example, an unsuffixed `u128`-expected literal as `u128` and preserves every
signed minimum through unary-negation handling.

Canonical primitive spellings are enforced at both the standalone type parser
and every full-source type-owning surface. Let ascriptions, declarations,
fields, function/flow signatures, capability/external members, and trait/impl
members preserve structured parse diagnostics instead of dropping an invalid
type into an untyped binding or `Raw` member. There is no formatter rewrite or
compatibility alias.

Expected numeric types propagate into arithmetic operands. The LSP uses typed
numeric-fallback evidence, rather than recognizing only a literal-shaped AST,
so literal, unary, binary, and compact-sequence `let` expressions receive their
resolved type hint. Explicit type ascriptions and fully explicit numeric
expressions do not receive redundant hints.

Solver-neutral proof integers now own canonical arbitrary-precision decimal
text. SMT-LIB lowering therefore preserves the full `u128` range instead of
narrowing contract literals through `i64`; malformed serialized integer text is
rejected by SMT problem validation. The OxiZ adapter parses that canonical
decimal directly into its arbitrary-precision term manager; it does not route
through a host integer width.

The exact integer syntax model was also moved from the general expression
parser into `expr/numeric.rs`. This reduced `expr.rs` from 2,525 to 2,240
physical LOC; the new responsibility module is 302 LOC. The post-change
repository structure audit reports zero error-level findings.

## Diagnostics and coverage

Structured semantic diagnostics distinguish malformed integer digits, integer
range overflow, and float-width overflow. Focused coverage includes radix and
separator preservation, `i128`/`u128` boundaries, signed minima, expected-type
inference through arithmetic, compact dense sequences, fixed-spread traversal
alignment, canonical primitive rejection across full source, finite/overflow
float boundaries, arbitrary-precision SMT emission, and LSP hint policy.

Validation for this cut:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-lang-syntax --all-features --test parser_expressions_literals_select
cargo test -p arcweft-lang-syntax --all-features --test parser_function_signatures_and_types
cargo test -p arcweft-lang-sema --all-features --lib numeric
cargo test -p arcweft-lang-sema --all-features --lib literal_bounds
cargo test -p arcweft-runtime-plan --all-features --lib numeric
cargo test -p arcweft-lsp --all-features inlay_hint_request_reports_unsuffixed_numeric_fallback_types
cargo test -p arcweft-verify --all-features integer_literals_preserve
cargo test -p arcweft-verify --all-features preserves_u128_literals
cargo test -p arcweft-verify-oxiz --all-features
cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler -p arcweft-lsp -p arcweft-verify -p arcweft-verify-oxiz --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

No numeric-literal compatibility or migration TODO remains. General callable,
pipe, closure, and method-fallback work is intentionally tracked separately
because it is not part of numeric representation or inference.
