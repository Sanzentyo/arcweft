# Solver-backed Arcweft contracts

These examples use the existing Arcweft contract surface. No Rust-side sample
builder is involved.

```bash
cargo run -p arcweft-cli -- verify \
  examples/verification/solver-contracts/valid.arcw \
  --mode test --backend oxiz --json

cargo run -p arcweft-cli -- verify \
  examples/verification/solver-contracts/valid.arcw \
  --mode test --backend z3 --json
```

The Z3 route requires a `z3` executable on `PATH`, `--z3-command`, or one of
these environment variables:

```bash
ARCWEFT_Z3_COMMAND=/path/to/z3
ARCWEFT_Z3_BIN=/path/to/z3/bin
```

On Windows PowerShell, set only your local environment, not a repository file:

```powershell
$env:ARCWEFT_Z3_BIN = "D:\path\to\z3\bin"
```

The OxiZ route is pure Rust and remains the default smoke path for this
directory.

All obligations in `valid.arcw` should be `unsat`, which proves their
postconditions. `mutants.arcw` is expected to fail: each mutated implementation
has at least one `sat` counterexample, and the report includes requested input
and `result` values when the backend exposes a model.

The initial lowering is deliberately explicit:

- ordinary, expression-bodied `fn` items only;
- `bool` and signed integer scalar parameters/results;
- Boolean logic, equality, integer comparisons, linear `+`/`-`, multiplication
  by an integer literal, `if`, `old(expr)`, and integer `clamp`/`min`/`max`;
- mathematical integers rather than machine-overflow semantics;
- unsupported syntax remains a visible verifier obligation and is never replaced
  by an uninterpreted predicate.

OxiZ 0.2.1 handles these examples but is intentionally documented as a smoke
backend, not as complete branch-heavy LIA coverage.
