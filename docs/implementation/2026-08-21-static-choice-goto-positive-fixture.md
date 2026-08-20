# Typed static Choice/goto positive-fixture closure

Date: 2026-08-21

Inspected baseline: `3fac70e5c`

Working-tree state at validation: dirty only with the static Choice semantic,
compiler/runtime-plan, regression-test, and documentation cut recorded here.

## Performed

- Added a checked Choice resolution that owns the canonical Choice and option
  public IDs plus exact project Flow selections for compact `goto` arms.
- Derived relative Choice IDs once from the checked enclosing Flow identity and
  final-HIR named-scope chain. Runtime lowering consumes those checked IDs and
  does not reconstruct a Flow target from a source or display string.
- Typed compact labels as `String`, enabled conditions as `Bool`, and all-goto
  Choice results as `Never`; value-output arms retain common result-type
  inference in semantic analysis.
- Projected the checked Choice fact through the compiler's generation-bound
  semantic boundary and validated every arm ordinal, option ID, Flow item,
  Flow runtime identity, and accepted HIR generation before publication.
- Lowered the supported static all-goto form to `RuntimeFlowOpSeed::Choice`,
  preserved it through sealed RuntimePlan construction, and verified its AWBC
  lowering.

## Passed

- `cargo fmt --all -- --check`
- `cargo check -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-features`
- `cargo test -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-features --no-fail-fast`
  - compiler unit and all integration/API suites passed, including the new
    static Choice/AWBC test;
  - sema unit: 208 passed; 10 API and 4 mismatch integration tests passed;
  - runtime-plan unit: 48 passed; its API, assertion, AWBC parity, iterator,
    and doc-test suites passed.
- `cargo clippy -p arcweft-runtime-plan --lib --tests --all-features --no-deps -- -D warnings`
- `cargo run -p arcweft-cli -- check tests/fixtures/arcw/current_pass/check/009_choice_static_goto.arcw`
  - check and verification passed with two flows and no warnings or
    obligations.
- focused sema canonical-ID/Flow-target test and compiler RuntimePlan/AWBC test.
- `git diff --check`

## Failed

- `cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-compiler --all-targets --all-features -- -D warnings`
  - stopped on existing warnings outside this cut in `arcweft-lang-syntax` and
    `arcweft-core`.
- the narrower `--no-deps` lint attempt still stopped on existing sema and
  compiler warnings outside this cut. The independently runnable
  `arcweft-runtime-plan` lint passed.
- The deterministic next fixture,
  `current_pass/check/010_dialogue_basic.arcw`, reaches runtime semantic
  projection and fails because executable dialogue has no selected
  `localization.character_names` policy. This is the next positive gate.

## Explicit non-goals

- Full option blocks, dynamic option generation, compact enabled-state
  execution, value-producing Choice continuation storage, and lifecycle plans
  are not claimed by this static all-goto cut. They continue to fail closed at
  their unimplemented typed runtime boundaries.
- Named-scope runtime execution is not added. Named scopes participate in the
  checked Choice ID test, while the executable fixture uses its supported
  unnamed Flow body.
- No Choice or option ID is inferred in runtime-plan lowering, and no legacy
  flattened/source-string Choice reader was restored.
