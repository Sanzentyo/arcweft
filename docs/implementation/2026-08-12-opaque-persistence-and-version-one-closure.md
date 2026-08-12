# Opaque persistence and version-one closure evidence

Date: 2026-08-12

Inspected Git baseline:
`c44c148c0832a0d840031f978a64314be9e8a8ec` on `main`, equal to
`origin/main`, with a clean working tree before the A1.4 implementation began.

## Implemented state

The A1.4 opaque persistence and deletion gate is implemented:

- `RuntimeOpaqueTypeProducerId` now performs validated manual deserialization,
  so empty or control-bearing producer evidence cannot enter through a Serde
  snapshot boundary;
- fiber snapshots retain the complete opaque producer, semantic identity,
  admission, and payload, and program validation rejects a foreign exact owner;
- bundle session saves round-trip an opaque value through the typed save
  envelope and reject invalid producer evidence before publishing any restored
  state; and
- opaque payloads continue to participate in the existing recursive nesting
  and canonical-value traversal rather than creating an uninspected carrier.

All Arcweft-owned version markers touched or found during this closure are now
fixed at `1`. This includes bundle, patch-plan, source-map, session-save,
project-cache, semantic-index, nominal-resolver, web-observation, and existing
AWBC/adapter/Rust-ABI schema or codec constants, plus the callable, generic,
final-HIR, view-state, presentation-routing, FX, text-layout, logical-bundle,
and persistent-object digest domains. The corresponding tests and emitted
identity strings were replaced in place. No old reader, writer, alias, V2/V3
model, migration map, or fallback was added.

This deliberately supersedes the returned package's proposed AWBC codec 11 and
session-save schema 3 bumps. The user selected the repository-wide invariant
that unreleased Arcweft-owned versions remain `1`; root `AGENTS.md` is the
maintained authority for that decision.

Source audits found no production Rust match for a non-1 Arcweft versioned
domain or version constant, and no remaining
`runtime_value_matches_pattern_type`, `accepts_variant_case`,
`RuntimeTypeShape::Named`, or producerless runtime success surface.

## Validation performed and passed

- `cargo fmt --all -- --check` and `git diff --check`.
- `CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
  --jobs 1`.
- `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features
  --jobs 1 -- -D warnings`.
- `cargo test -p arcweft-core value::opaque::tests --lib --all-features`: 8
  focused owner, Serde, canonical-value, and nesting tests passed.
- The focused opaque fiber snapshot test passed.
- The focused full bundle-session opaque save/restore, invalid-producer, and
  atomic-rejection test passed.
- The focused runtime snapshot schema-one test passed.
- `cargo test -p arcweft-bundle --test patch_schema --all-features --jobs 1`:
  8 tests passed, including schema-one-only rejection and deterministic
  round-trip coverage.
- `cargo test -p arcweft-project --lib --all-features`: 36 tests passed.
- `cargo test -p arcweft-lang-sema --lib --all-features`: 188 tests passed.
- `cargo test -p arcweft-presentation --lib --all-features`: 74 tests passed.
- `cargo test -p arcweft-text-layout --lib --all-features`: 25 tests passed.
- `cargo test -p arcweft-compiler --lib --all-features`: 51 tests passed.
- `cargo test -p arcweft-player-web --lib --jobs 1`: 16 tests passed.
- `just structure-audit-gate`: 2,160 files, 2,032 Rust files, 1,006,003 Rust
  physical LOC, 95 packages, 184 review triggers, and zero blocking findings.

## Failed or blocked validation

- The first all-feature bundle test attempt compiled the changed crate but four
  integration targets could not mmap the generated rlib because the Windows
  paging file was too small (OS 1455). The changed patch-plan integration suite
  was then run alone with one job and passed all 8 tests.
- `arcweft-player-web` parity ran 7 tests: 2 passed and 5 failed before the
  changed observation version assertion. The failures are existing fixture
  drift: recovered/non-executable HIR, a removed top-level image declaration,
  and old embedded AWFB JSON missing `artifact_fingerprint`. The player-web
  library suite and workspace check/Clippy pass; repairing those fixtures is
  outside this persistence/version closure.
- A prior retry initially exhausted the D: drive because shared builds had
  accumulated 280.1 GiB under `target`. `cargo clean` removed only generated
  build artifacts; the subsequent focused and workspace validations above ran
  from a clean target.

## Structural review and continuation

The touched large bundle runtime codec remains one cohesive owner for compact
runtime resource projection. Its opaque test module was moved after production
items when Clippy exposed the ordering violation; no production behavior moved.
Session-save validation remains in the runtime-driver save boundary, while the
producer's lexical invariant remains on the core identity type. No parallel
registry or matcher was introduced.

A1.1 through A1.4 are now closed. The parent affine ownership implementation
may resume at A2/G1.2 without treating the package's obsolete version bumps as
pending work.
