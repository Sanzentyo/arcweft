# Dialogue project-default positive-fixture closure

Date: 2026-08-21

Inspected baseline: `947cd6b98`

Working-tree state at validation: dirty only with the Character locale-policy,
runtime-ID source boundary, compiler regression, documentation, and tests in
this cut.

## Performed

- Added `CharacterNameLocalePolicy::engine_default()` to the owning Character
  presentation-name domain. The default is the maintained Japanese-first
  `ja-JP` policy with no fallback locales.
- Changed executable dialogue projection to resolve an authored manifest
  `CharacterNameLocalePolicySpec` into the Character-owned runtime policy, or
  select the owning engine default when the optional profile field is absent.
  No synthetic manifest policy or secondary profile reader was introduced.
- Kept source-entity conversion distinct from canonical runtime-ID parsing.
  After the leading `say` family is validated and removed, the checked line ID
  payload may retain opaque family-looking segments such as the complete
  `flow.*` public-ID body required by the accepted line-identity contract.
- Continued to reject runtime-only `__agent_controller` and `__checked_flow`
  owner segments at the source boundary.

## Passed

- `cargo fmt --all -- --check`
- `cargo test -p arcweft-character -p arcweft-core -p arcweft-compiler -p arcweft-runtime-plan --all-features --no-fail-fast`
  - Character unit: 46 passed; its 6 + 1 + 1 integration suites passed;
  - compiler unit: 53 passed; all API and integration suites passed;
  - core unit: 217 passed; all API and integration suites passed, including
    13 runtime-ID boundary tests;
  - runtime-plan unit: 48 passed; all API and integration suites passed;
  - all four crates' doc tests passed.
- `cargo clippy -p arcweft-character --all-targets --all-features --no-deps -- -D warnings`
- `cargo run -p arcweft-cli -- check tests/fixtures/arcw/current_pass/check/010_dialogue_basic.arcw`
- `cargo run -p arcweft-cli --quiet -- check tests/fixtures/arcw/current_pass/check/011_dialogue_with_plan.arcw`
  - both check and verification paths passed without warnings or obligations.
- `git diff --check`

## Failed

- `cargo clippy -p arcweft-core -p arcweft-compiler --lib --tests --all-features --no-deps -- -D warnings`
  - stopped on five existing `arcweft-core` size/line-count/argument-count
    warnings outside this cut before compiler linting completed.
- The deterministic next fixture,
  `current_pass/check/012_function_final_expr.arcw`, is parsed as a malformed
  Choice because the ordinary path expression `choice.label` begins with the
  contextual `choice` keyword. This is the next positive gate.

## Explicit non-goals

- An omitted policy does not create or mutate a launch manifest. It selects
  the Character domain's language-owned default only after the optional
  authored policy is absent.
- Canonical runtime-ID constructors still reject source-family spellings.
  Only the explicit, already-family-checked source conversion preserves opaque
  interior public-ID segments.
- Dialogue line-plan handle/result execution beyond the existing check fixture
  remains governed by the open typed runtime authority request.
