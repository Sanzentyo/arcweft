# Nominal projection authority query cleanup

Date: 2026-08-22
Inspected Git commit: `c985767aee150648ff43bbaadd97fc7c1f81f387`

## Result

The generation-bound nominal projections remain explicit
`FinalSemanticAnalysis` context methods rather than `From`/`TryFrom`
implementations. They require the accepted symbol generation, declaration
owner and arity, final type facts, substitutions, and typed failure paths;
modeling those inputs as a conversion tuple would hide the admission
authority.

The pure `TypeShape -> RuntimeTypeSchema` bridge also remains a named free
function. Both types are foreign to `arcweft-lang-sema`, so a local `From`
implementation is forbidden by Rust coherence. Moving the implementation to
`arcweft-core` would promote `arcweft-data` from a dev-dependency to a
production dependency solely for this compiler/reflection bridge and reverse
the intended layer ownership.

## Performed

- Added one narrow `checked_project_nominal` query that validates the exact
  semantic/symbol lease, declaration, and arity without requiring a runtime
  snapshot layout.
- Made raw-nominal runtime projection reuse that checked owner.
- Made ownership record/variant projection reuse the same checked nominal
  instead of reconstructing it separately in each branch.
- Separated project-nominal semantic identity admission from full runtime
  schema/layout admission, so a `Need<ProjectNominal>` can retain the exact
  payload identity without claiming that the payload itself is snapshot-safe.
- Deleted the layout-only projection wrapper; the sole consumer now uses the
  complete runtime projection and its typed `layout()` getter.

## Validation

- Focused identity-without-layout regression: 1 passed, 0 failed.
- `cargo test -p arcweft-lang-sema -p arcweft-compiler --lib
  --all-features`: sema 262 and compiler 55 passed; 0 failed.
- `cargo check -p arcweft-lang-sema -p arcweft-compiler -p
  arcweft-runtime-plan --all-targets --all-features`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Structure audit: 2,183 files, 2,055 Rust files, 199 review triggers, 0
  blocking violations.
- Strict sema Clippy remains failed on the same 24 baseline/separately owned
  diagnostics; no diagnostic names `nominal_schema.rs`, `ownership.rs`, or the
  new regression.

## Non-goals

- No `From`/`TryFrom` request wrapper, local extension trait, conversion
  newtype, second schema mapping, fallback resolver, compatibility path, or
  version change was added.
