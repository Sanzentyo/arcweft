# Function Stack Method-Value Rejection - 2026-07-09

This cut tightens the 07.7 non-helper callable allocation boundary for
value-position method references.

## What Changed

- Sema now distinguishes selected method-value references such as
  `score.above` from ordinary field selection when the selected member resolves
  to an environment method, inherent method, or trait/impl method.
- The selected value is rejected with structured
  `UnsupportedMethodValueReference` diagnostics and stable code
  `sema.typecheck.unsupported_method_value_reference`.
- Normal method calls such as `score.above(80i64)` still use the existing
  resolution order: environment methods, built-in methods, presentation handle
  methods, trait/inherent methods, then data-last callable fallback.
- Field selection remains authoritative before method-value detection, so this
  cut does not turn fields into methods.

## Contract

Arcweft does not yet materialize method values. Receiver binding for
trait/impl methods must remain explicit until 07.7 defines whether method
values lower to bound receiver closures, unbound method descriptors, adapter
thunks, or another callable representation.

Authors can still call the method directly or write an explicit closure once
the surrounding language surface supports the desired receiver binding shape.

## Evidence

Focused sema regressions:

- `trait_method_value_reference_reports_unsupported_method_value`
- `environment_method_value_reference_reports_unsupported_method_value`

Validation for this cut:

```bash
cargo test -p arcweft-lang-sema --all-features method_value_reference_reports_unsupported_method_value -- --nocapture
```

The broader reviewable-cut validation is recorded in the final command log for
the associated commit.

## Remaining Work

This does not implement first-class method values. The broader 07.7 contract
still needs an accepted representation for any callable family that is allowed
to escape as a value, including effect/suspension behavior, AWBC lowering, and
save/load policy.
