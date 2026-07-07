# Request: Function Stack Capture, Effect, Lifetime, and LSP Evidence

## Context

Runtime functions currently capture deterministic local bindings at evaluation
time. The remaining language contract needs explicit capture inventory,
effect-row composition, suspension-boundary diagnostics, and tooling evidence.

## Required Decisions

- Define capture inventory format for closures and function values.
- Specify diagnostics for captures crossing `await`, `yield`, thread,
  line-task, defer, save/load, and resource lifetime boundaries.
- Define how closure effects compose into existing effect rows.
- Define LSP inlays for inferred closure/function types.
- Define lints for numeric fallback inside inferred closure bodies.

## Implementation Order

1. Add capture inventory collection in sema.
2. Add lifetime/effect diagnostics for suspension boundaries.
3. Add runtime-plan metadata only where it is needed by runtime diagnostics or
   save/load policy.
4. Add LSP inlays and lints from the same sema evidence.

## Tests To Specify

- Capture of immutable local in pure closure succeeds.
- Capture crossing `await` is rejected when lifetime/effect policy forbids it.
- Capture crossing thread/defer boundaries reports the owning boundary.
- Function type inlay appears for inferred closures.
- Numeric fallback lint appears inside inferred closure body.

## Constraints

- Do not make `arcweft-core` depend on sema, LSP, CLI, or OS adapters.
- Keep diagnostics structured with stable codes.
- Do not redesign existing effect capability policy unless evidence shows a
  concrete flaw.

## Expected Output

- Sema capture/effect data model.
- Structured diagnostic tests.
- LSP/tooling tests and docs updates.
