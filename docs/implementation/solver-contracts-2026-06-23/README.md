# Solver-backed scalar function contracts

## Goal

Make useful OxiZ/Z3 verification available through ordinary Arcweft function
contracts instead of a Rust-only sample builder.

```arcw
pub fn charge_purchase(balance: i32, price: i32) -> i32
requires price >= 0
requires balance >= price
ensures prove result >= 0
ensures prove result == balance - price
{
    balance - price
}
```

Each proof-mode postcondition is lowered to the counterexample query:

```text
requires ∧ result == body ∧ ¬ensures
```

`unsat` proves the postcondition. `sat` is a counterexample and adapters retain
requested parameter/result values when the solver exposes them.

## Ownership

```text
arcweft-lang-syntax
  ContractClause and Pattern own their classification/accessor behavior

arcweft-verify
  ProofExpr, SmtProblem, proof polarity, validation, SMT-LIB emission,
  and typed lowering from existing HIR contract expressions

arcweft-verify-oxiz / arcweft-verify-z3
  concrete solver term/process execution and model normalization only

arcweft-cli
  adapter selection, artifact I/O, and report recording
```

No parallel `SmtExpr`, `ProofQuery`, or verifier-specific backend trait is
introduced. The existing `SmtBackend` remains the only solver boundary.

## Initial supported subset

- pure expression-bodied ordinary `fn` items;
- `bool` and signed `i8`/`i16`/`i32`/`i64`/`isize` parameters and result;
- `requires` and `ensures prove` (an omitted mode also requests proof);
- Boolean logic, implication, equality, signed integer comparison;
- linear addition/subtraction and multiplication by an integer literal;
- `if`, `old(expr)` for immutable pure-function inputs, and integer
  `clamp`/`min`/`max`;
- mathematical integer semantics; machine overflow remains future work.

Unsupported types, statement bodies, nonlinear arithmetic, indexing, and
proof-mode invariant clauses produce explicit verifier obligations with no SMT
problem. They are never represented by an unconstrained uninterpreted
predicate.

Explicit `assume` clauses remain owned by semantic trust policy and do not
silently strengthen the SMT problem.

## Validation commands

```bash
cargo test -p arcweft-verify
cargo test -p arcweft-verify-oxiz
cargo test -p arcweft-verify-z3
cargo run -p arcweft-cli -- verify \
  examples/verification/solver-contracts/valid.arcw \
  --mode test --backend oxiz --json
cargo run -p arcweft-cli -- verify \
  examples/verification/solver-contracts/valid.arcw \
  --mode test --backend z3 --json
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root .
```

The Z3 command requires a `z3` executable on `PATH`, `--z3-command`,
`ARCWEFT_Z3_COMMAND`, or `ARCWEFT_Z3_BIN`. `ARCWEFT_Z3_COMMAND` points to the
executable; `ARCWEFT_Z3_BIN` points to the directory that contains `z3`.
Do not commit local solver install paths to the repository. OxiZ 0.2.1 is
sufficient for the checked-in smoke examples, but its branch-heavy LIA support
is not treated as a complete replacement for Z3.
