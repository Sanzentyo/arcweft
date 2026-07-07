# Request: Function Stack AWBC Closure and Apply Semantics

## Context

Runtime `Function` and `Apply` now execute in the core VM/pure evaluator, but
AWBC still treats `RuntimeExpr::Function` as unsupported and represents
`RuntimeExpr::Apply` only as a bytecode inventory placeholder. This is the main
remaining runtime parity gap for function values.

## Required Decisions

- Define the AWBC representation for runtime closure values, including
  parameter names, body expression/code pointer, and deterministic capture
  bindings.
- Decide whether closure bodies are encoded as nested runtime expressions,
  separate function tables, or bytecode subprograms.
- Specify partial application storage and invocation semantics.
- Specify how pure-helper-backed function values interact with existing pure
  helper tables and accelerator-friendly `PureCall`.
- Specify serialization/versioning for save snapshots that may contain
  function values, or explicitly reject persisted closures with a structured
  diagnostic for this cut.

## Implementation Order

1. Add AWBC schema/table design for function values.
2. Implement bytecode verification for closure/apply instructions.
3. Implement AWBC product VM execution for function apply, partial apply, and
   curried apply.
4. Add parity tests against the existing core VM runtime-function tests.
5. Update save/load policy for function values.

## Tests To Specify

- Captured closure application.
- Partial application returns a function value.
- Curried application `make_adder(2i64)(5i64)`.
- Pure-helper function value used through apply.
- AWBC verifier rejects malformed arity/capture tables.

## Constraints

- Do not redesign the existing core runtime function/apply substrate without a
  concrete bug.
- Preserve deterministic capture ordering.
- Keep data-format crates Sans I/O.

## Expected Output

- AWBC schema/code changes.
- Product VM and verifier tests.
- Documentation of persistence behavior for function values.
