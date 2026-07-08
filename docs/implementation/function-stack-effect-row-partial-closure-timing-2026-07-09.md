# Function Stack Effect-Row Partial Closure Timing - 2026-07-09

This cut strengthens the current 07.8 evidence for partial closure timing.

## What Changed

- Added `no_effect_rejects_partial_closure_alias_effect_when_called`.
- The fixture partially applies a local closure whose body performs `fs.read`.
- Creating the partial alias remains effect-free under the existing tests.
- Calling the partial alias now has explicit `no_effect` coverage: the caller
  fails only when the partial value is invoked.

## Contract

Partial application does not perform the closure body effects. The body effects
compose into the caller only when the resulting partial function value is
called.

This is still implemented through the current sema effect graph and closed-row
projection. It does not finalize source-level effect-row syntax, open-row
inference, or row-bearing callable values.

## Evidence

Existing timing regressions:

- `partial_local_closure_application_does_not_compose_until_called`
- `partial_immediate_closure_application_does_not_compose_until_called`
- `partial_local_closure_alias_composes_body_effects_when_called`
- `partial_immediate_closure_alias_composes_body_effects_when_called`

New forbidden-row regression:

- `no_effect_rejects_partial_closure_alias_effect_when_called`

Validation:

```bash
cargo test -p arcweft-lang-sema --all-features no_effect_rejects_partial_closure_alias_effect_when_called -- --nocapture
```
