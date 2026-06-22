# Effect system implementation note

This note records the current implementation slice for the effect-system design
package extracted from `D:/sanze/Downloads/arcweft-effect-system-design.zip`.

## Implemented

- `arcweft-lang-sema` now owns typed effect identifiers, effect sets,
  callable facts, contracts, diagnostics, and fixed-point first-order closure.
- Type checking collects direct effects and call edges while checking HIR bodies.
  User helper effects propagate transitively into callers.
- `effects { ... }` is an upper bound: inferred effects must be a subset of the
  declared set. An explicit `effects {}` is preserved as an explicit empty
  upper bound.
- `ensures no_effect ...` rejects forbidden effects after transitive closure.
- Host availability from `TypeCheckEnv::available_effects` is separate from
  source upper-bound checks. Environment availability does not widen a source
  `effects { ... }` bound.
- Named external calls with typed signatures but no effect metadata are treated
  as pure external calls. External calls with effect metadata participate in
  effect closure.
- Agent artifacts now carry a required `verified_effects` summary with the
  compiler-lowered closed first-order row, inferred closure, analysis version,
  and digest.
- Agent runner bundle execution builds an exact authorization from
  `verified_effects.inferred`, maps verified effect labels to runtime
  capabilities, checks launch grants before execution, and rejects host requests
  absent from the verified artifact.
- Agent runtime policy decisions use verified inferred effects instead of the
  legacy `declared_effects` list.
- Unused source upper-bound members are not emitted into Agent artifact
  `declared_effects` and are not reported as overdeclaration warnings.
- LSP code actions propose quick fixes for source upper-bound excess
  diagnostics. The edits are based on structured effect diagnostics rather than
  string parsing.

## Current boundaries

- Normal non-agent flows are not treated as public artifact boundaries merely
  because they are top-level flows. Omitted `effects` on such flows means
  infer-only; explicit `effects {}` still rejects any inferred effect.
- Agents remain artifact boundary callables, but omitted `effects` means
  infer-only. Bundle lowering uses the closed inferred row rather than the
  source upper bound.
- Public functions do not require explicit non-empty effect declarations merely
  because they infer effects.
- The current closure is first-order. Higher-order/dynamic effect rows have
  typed model support, but full source lowering is not complete in this slice.
- Runtime host adapter availability still uses adapter host-call manifests.
  This remains intentionally separate from source effect declarations.

## Validation

- `cargo test -p arcweft-lang-sema typecheck --lib -- --format=terse`
- `cargo test -p arcweft-agent-runner controller_bundle --lib -- --format=terse`
- `cargo test -p arcweft-compiler compile_agent_bundle_with_project --lib -- --format=terse`
- `cargo test -p arcweft-bundle bundle_agent_manifest_marks_agent_controller_and_round_trips --lib -- --format=terse`
- `cargo test -p arcweft-lsp diagnostics_surface_upper_bound_exceeded_effect_error --lib -- --nocapture`
- `cargo test -p arcweft-lsp code_actions_expand_effect_upper_bound --lib -- --nocapture`
