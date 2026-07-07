# Request: Function Stack Placeholder Inference and Method Fallback

## Context

Expected-type `_` is implemented for explicit function annotations and the
collection `map`/`filter` optimization paths. No-expected-type inference such
as `let is_high = (_ > 80i64)` and partial call abstraction such as
`let add_one = add(_, 1i64)` remain intentionally unimplemented.

Method-chain fallback is also not implemented: real inherent/trait methods
should win first, and only unresolved method syntax should fall back to
data-last callable application.

## Required Decisions

- Define inference rules for `_` regions without an expected function type.
- Define multi-placeholder behavior, including whether repeated `_` means one
  generated parameter or multiple parameters in any context.
- Define partial call abstraction lowering for `add(_, 1i64)` without
  hard-coding callable names.
- Define method-chain resolution order:
  1. inherent method;
  2. trait method;
  3. data-last callable fallback.
- Define ambiguity diagnostics that report the conflicting method/callable
  candidates.

## Implementation Order

1. Extend sema inference for no-expected `_` regions.
2. Implement partial call abstraction as function values.
3. Implement method-chain fallback using typed callable evidence, not string
   labels alone.
4. Add diagnostics for ambiguous or unsupported fallback cases.

## Tests To Specify

- `let is_high = (_ > 80i64)`.
- `let add_one = add(_, 1i64)`.
- Repeated `_` in one abstraction region.
- Inherent method wins over data-last fallback.
- Trait method wins over data-last fallback.
- Ambiguous fallback reports all candidates.

## Constraints

- Do not accept removed syntax.
- Do not add formatter normalization shims.
- Do not special-case a single callable such as `add`.

## Expected Output

- Sema inference tests.
- Runtime-plan function/apply lowering tests.
- Structured diagnostics and docs updates.
