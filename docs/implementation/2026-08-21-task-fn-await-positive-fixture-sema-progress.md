# Task-function await positive fixture semantic progress

Date: 2026-08-21

Inspected Git commit: `6e17c9fafe7c254b27e99f51af52ccc109a3a41d`

Working-tree state at inspection: clean `main`; this coherent cut then changed
three `arcweft-lang-sema` Rust files and this documentation.

## Performed

- Changed the core `load_bg` environment callable from an unchecked variadic
  signature to its exact zero-argument signature.
- Canonicalized every standard callable result, parameter, higher-order
  binding, remaining parameter group, and method receiver against the accepted
  nominal catalog at publication time.
- Reused one recursive `TypeKind` traversal for primitive normalization and
  accepted-nominal joining. No callable-specific opaque spelling branch or
  runtime fallback was added.
- Added final-analysis coverage proving `load_bg()` retains unary
  `Need<Result<...>>` and the exact `std.image_handle` / `std.arc_error`
  producer identities through `await` and prefix `try`.

## Passed

- `cargo check -p arcweft-lang-sema -p arcweft-compiler`
- `cargo test -p arcweft-lang-sema --lib standard_zero_argument_need_callable_uses_accepted_nominal_results --all-features`
  — 1 passed
- `cargo test -p arcweft-lang-sema --all-features --no-fail-fast` — 209 unit
  tests, 10 compile/API tests, 4 nominal-mismatch integration tests, and doc
  tests passed
- `cargo check -p arcweft-compiler --all-targets --all-features`
- `cargo fmt --all -- --check` and `git diff --check`
- final semantic analysis of the unchanged fixture 013 reaches compiler runtime
  semantic projection; the former shared callable resolution failure is gone.

## Blocked

`cargo run -p arcweft-cli --quiet -- check tests/fixtures/arcw/current_pass/check/013_task_fn_await_shape.arcw`
still fails in `compiler.runtime_semantic_projection` while projecting
`OpeningAssets`.

The project nominal contains accepted opaque `ImageHandle`. Current accepted
opaque policy forbids a fabricated schema/layout, while project nominal runtime
facts require an exact schema-derived layout. The declaration is inside an
unreferenced suspending ordinary function, but the current runtime semantic
owner inventory includes every non-presentation owner even though ordinary
suspending-function AWBC execution remains outside the implemented runtime
surface.

The exact design blocker is delegated to
[Lang-01.1.1.3.2](../reviews/requests/2026-08-21-lang-01.1.1.3.2-suspended-function-runtime-emission-and-opaque-nominal-layout-reconciliation.md).

## Not run

- Full workspace tests, runtime execution tiers, and structure-audit commands
  were not selected for this sema-only cut.
- Fixture 014 and later positive rows were not advanced because the maintained
  deterministic gate requires fixture 013 to close first.

## Failed validation

- `cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings`
  stopped in dependency crates on 10 existing warnings.
- `cargo clippy -p arcweft-lang-sema --lib --all-features --no-deps -- -D warnings`
  then exposed 18 existing sema warnings, including size/line-count and
  mechanical style findings outside this behavioral cut. No warning was hidden
  or allowed in production code.

## Structural review

The touched large files were inspected directly. `env/base.rs` remains the
single standard environment/type normalization owner; the new publication-time
join reuses its existing recursive `TypeKind` normalization traversal.
`callable/builder.rs` remains the single accepted callable catalog publication
transaction and only consumes that environment-owned canonicalization.
`final_analysis/tests.rs` grows by one focused behavioral row and receives no
production responsibility. Decomposition would split one owner across files
without reducing a responsibility boundary, so it is not part of this cut.

## Non-goals

- project nominal runtime layout redesign;
- reachability-based runtime fact emission;
- ordinary suspending-function RuntimePlan/AWBC execution;
- opaque schema fabrication or name-derived layout; and
- any fixture allowlist or source-spelling exception.
