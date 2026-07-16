# Proof-concurrency v6.1.1 Stage 1 expression events

## Scope

This private Stage 1 slice follows Git
`79bda5694c5f` (`Add private predicate proof grammar descendants`) and advances
only the shadow expression grammar required by the verified proof-concurrency
v6.1.1 package. It changes no public parser, AST, HIR, compiler, cache, or
tooling behavior and allocates no production syntax identity.

## Implemented expression ownership

The private shared document cursor now owns event markers that can wrap an
already completed left child without reparsing or copying source. The Pratt
parser uses those markers to emit:

- precedence-correct nested binary, range, and pipe expressions;
- prefix borrow, dereference, unary, await, and thread operands;
- path expressions with one identity-bearing full `Path` and ID-less path
  segments;
- tuple/bracket expression lists and distinct delimiter nodes;
- call expressions, identity-bearing call arguments, and callee roles;
- index, select, and try postfix chains with exact target/operand roles;
- literal, entity-reference, lifetime-path, short-variant, placeholder, block,
  if, match, and closure outer families; and
- typed missing/error expressions when an operand or unconsumed current-grammar
  suffix cannot be attached cleanly.

Every real token is still emitted exactly once in source order. Parent insertion
changes only event nesting; it cannot reorder or duplicate token ranges. The
event builder's byte-coverage and balance validation remains the final private
construction gate.

Direct tests cover `a + b * c`, two identity-bearing call arguments, index plus
optional select plus try chaining, path identity, and a proof-call statement
using the same call expression authority. The reconstructed green text equals
the exact source bytes.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-syntax --lib
cargo test -p arcweft-lang-syntax parser::predicate_proof_tests --lib
cargo clippy -p arcweft-lang-syntax --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
```

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-expression-events-2026-07-16/`.
It scanned 2,921 files, 1,448 Rust files, 673,484 physical Rust LOC, and 90
manifests with zero errors and 129 repository-wide warnings.

`parser/expression.rs` is 17,086 bytes / 544 physical LOC / 513 code LOC, has
no embedded tests, and remains inside the package's explicit 500-900 LOC Pratt
parser band. `parser/document.rs` is 365 LOC after adding only shared marker
operations. The sibling predicate/proof test file is classified as a test.

## Remaining boundary

This is not the complete expression grammar gate. Named/record/dialogue calls,
record literals, closures, block descendants, if-let, match arms, and detailed
recovery still need exact shared-cursor implementations and direct family
tests. Complete pattern/type/statement/item coverage and all later attachment,
public syntax, HIR, project, and runtime stages remain open.
