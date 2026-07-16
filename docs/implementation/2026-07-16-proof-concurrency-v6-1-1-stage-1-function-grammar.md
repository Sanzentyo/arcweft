# Proof-concurrency v6.1.1 Stage 1 ordinary-function grammar

## Scope

This cut extends the private, lossless shadow grammar from predicate/proof and
Flow declarations to the ordinary `fn` declaration family. It does not switch
the public syntax API and does not alter the production parser or HIR.

The parser now emits typed descendants for outer documentation and attributes,
visibility, an ordinary name, generic parameters, every authored curried
parameter group, the return type, `where` predicates, `requires`/`ensures`
clauses, and the function block. Function statements and the final value reuse
the existing private statement/expression grammar over the same token cursor.

The common declaration header grammar moved from the predicate/proof-specific
module to `parser/declaration.rs`. This is an ownership extraction, not a second
signature parser: every declaration consumes the already lexed token slice and
emits into the one document event stream.

## Recovery

- a missing parameter group emits `syntax.decl.invalid_header` and typed missing
  delimiters without consuming the return type or body;
- a missing body emits `syntax.decl.missing_body` and leaves the following
  declaration untouched;
- a missing closing brace emits `syntax.function.missing_block_close` at the
  following declaration boundary and preserves that declaration as a sibling.

No spelling-specific removed-syntax recognizer, source gate, compatibility
alias, CSS route, or Takumi route is added.

## Direct evidence

`parser::function_grammar_tests` covers a documented and attributed public
function with lifetime/type generics, two curried parameter groups, a return
type, `where`, both contract clauses, a typed `let`, and a tail expression. It
also covers missing parameter groups, missing bodies, missing closing braces,
and exact byte-for-byte green-tree round trips.

The extraction is guarded by the existing 28 predicate/proof tests, including
the shared declaration diagnostics and malformed-declaration synchronization.

## Validation

The following commands pass with `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-syntax parser::function_grammar_tests --lib -- --nocapture
cargo test -p arcweft-lang-syntax parser::predicate_proof_tests --lib -- --nocapture
cargo test -p arcweft-lang-syntax --lib
cargo clippy -p arcweft-lang-syntax --all-targets -- -D warnings
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/proof-concurrency-v6-1-1-stage-1-function-grammar-2026-07-16
```

The new focused suite passes 4/4, the predicate/proof suite passes 28/28, and
the complete syntax library passes 180/180. Workspace check and Clippy complete
without errors or warnings.

## Structure

The canonical report is stored under
`structure-audits/proof-concurrency-v6-1-1-stage-1-function-grammar-2026-07-16/`.
It scanned 3,028 files, 1,504 Rust files, 693,991 physical Rust LOC, and 90
manifests with zero errors and 128 pre-existing repository-wide warnings. No
warning names an in-scope file.

- `parser.rs`: 23,857 bytes / 688 physical LOC, production parser facade;
- `parser/declaration.rs`: 15,455 bytes / 435 physical LOC, shared declaration
  header grammar;
- `parser/document.rs`: 18,165 bytes / 571 physical LOC, private document/event
  orchestration;
- `parser/function_grammar.rs`: 2,781 bytes / 94 physical LOC, ordinary function
  declaration grammar;
- `parser/function_grammar_tests.rs`: 4,791 bytes / 162 physical LOC, direct
  test module;
- `parser/predicate_proof.rs`: 3,483 bytes / 111 physical LOC, predicate/proof
  declaration specialization.

All changed production files remain below the repository warning thresholds.
No Cargo dependency, feature, public API, serialization contract, or crate
boundary changes in this cut, so dependency fan-in/fan-out is unchanged.

## Remaining Stage 1 work

Stage 1 remains open. This cut deliberately does not claim typed receiver,
rest-parameter, or default-parameter descendants, and it does not assign final
semantics to the currently disputed `task fn`, `dialogue fn`, or `stream fn`
surface. The remaining declaration families and malformed/recovery
cross-products still need direct events and tests before the atomic syntax
switch. Proof-concurrency stages 2 through 8 are also still open.
