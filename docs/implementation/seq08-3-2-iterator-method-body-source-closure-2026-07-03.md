# Seq08.3.2 Iterator Method Body Source Closure

Date: 2026-07-03

## Source Package

Applied from `arcweft-seq08.3.2-iterator-method-body-source-closure-2026-07-03.zip`.

The package patches were not directly applicable because two patch files used malformed hunk headers. The implementation was applied manually against the current `main` while preserving the package acceptance goals.

## Implemented

- Parser function parameters now preserve source receiver mode through `FnReceiverKind`, so `self`, `&self`, and `&mut self` no longer rely on debug-string inspection.
- Semantic analysis records top-level nominal struct fields and checks direct record field reads, record literals, and direct assignment statements against those fields.
- Impl method bodies are type-checked with their declared return type, receiver binding, local parameters, and trait predicates.
- `Some(...)`, `None`, block final values, and `if` expression branches receive the expected return type, allowing `Iterator::next` bodies to type-check through `Option<T>`.
- Unsupported assignment targets produce structured `sema.typecheck.unsupported_assignment_target` diagnostics.
- Runtime-plan lowering supports direct local record field assignment in strict expressions, trait method bodies, and flow statements through existing `RuntimeExpr::AssignField`.
- Flow-body assignment lines classify as typed statements instead of raw recovery nodes.
- Struct fields now parse top-level comma-separated field lists, so `struct Hoge { current: i32, end: i32 }` and multiline forms produce the same nominal field inventory.

## Tests And Fixtures

Added source fixtures under `fixtures/iterator-witness/`:

- `user-defined.arcw`
- `invalid-nested-assignment.arcw`
- `branch-mismatch.arcw`

Added focused tests:

- `crates/arcweft-lang-syntax/tests/assignment_statements.rs`
- `crates/arcweft-lang-sema/tests/iterator_method_body_source.rs`
- `crates/arcweft-compiler/tests/iterator_witness_source.rs`

## Structural Audit

`cargo +nightly -Zscript tools/structure-audit.rs --root . --write target\structure-audit-seq08-3-2` completed and reported 4 error-level size violations plus 125 warnings.

The error-level files were:

- `crates/arcweft-cli/src/app/bundle.rs`
- `crates/arcweft-core/src/value.rs`
- `crates/arcweft-lang-sema/src/checker/expr.rs`
- `crates/arcweft-runtime-plan/src/flow.rs`

`checker/expr.rs` and `flow.rs` are touched by this cut and were already above the error threshold before this package. This cut keeps the source-iterator behavior cohesive and avoids mixing a broad sema-expression/runtime-flow decomposition into the trait-method closure work. The next structural split should extract expression expected-type/branch checking and flow statement lowering into responsibility modules before adding more language-surface behavior there.

## Non-Goals

- No compatibility syntax or fallback path was added.
- AWBC trait method table/schema changes are not part of this cut; seq08.3.1 owns that boundary.
- Nested lvalues such as `self.point.x = ...` remain rejected until a first-class runtime lvalue model exists.
- Nested `if` statement parsing inside flow expression blocks remains outside this cut. The iterator witness fixture avoids depending on that unrelated parser gap.
