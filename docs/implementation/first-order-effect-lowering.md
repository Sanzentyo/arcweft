# First-Order Effect Lowering

Source package: `arcweft-first-order-effect-lowering.zip`

## Implemented Semantics

- Omitted `effects` means infer-only. The type checker computes the first-order
  transitive effect row and does not impose a source upper bound.
- Explicit `effects {}` is an empty upper bound and rejects any inferred
  first-order effect.
- Explicit `effects { a, b }` is an upper bound. Inferred effects must be a
  subset of that set.
- Source upper-bound members that are not used by the body are not warnings and
  are not lowered into Agent artifacts.
- Agent controller artifacts lower the compiler-inferred closed effect row into
  the legacy serialized `declared_effects` field and into
  `verified_effects.declared`.
- Host target availability remains separate from source upper-bound validation.
  It can reject inferred effects unavailable on the target, but it cannot widen
  source `effects { ... }`.
- Lifetime writes to upper scopes are recorded as direct effect facts. They are
  validated by the same source upper-bound and target availability checks as
  other effects.

## Non-Goals

- Higher-order callable row variables, open rows, trait dispatch rows, and
  separate-compilation row constraints are not implemented in this slice.
- The serialized Agent manifest field name `declared_effects` is not renamed.
  Its value semantics are documented as the closed compiler-lowered row.
- Normal flows are not emitted as new artifact boundaries.

## Validation

Completed on 2026-06-22:

```bash
cargo test -p arcweft-lang-sema typecheck --lib -- --format=terse # passed
cargo test -p arcweft-lang-sema --lib # passed
cargo test -p arcweft-compiler --lib # passed
cargo test -p arcweft-lsp --lib # passed
cargo test -p arcweft-agent-protocol --lib # passed
cargo test -p arcweft-agent-runner controller_bundle --lib -- --format=terse # passed
cargo test -p arcweft-bundle bundle_agent_manifest_marks_agent_controller_and_round_trips --lib -- --format=terse # passed
cargo check -p arcweft-lang-sema -p arcweft-compiler -p arcweft-lsp -p arcweft-agent-protocol --all-targets # passed
cargo clippy -p arcweft-lang-sema -p arcweft-compiler -p arcweft-lsp -p arcweft-agent-protocol --all-targets --all-features -- -D warnings # passed
cargo check --workspace --all-targets # passed
cargo clippy --workspace --all-targets --all-features -- -D warnings # passed
cargo fmt --all --check # passed
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . # passed: 0 error(s), 87 warning(s)
just test-workspace # passed
```

Static search also confirmed that the removed Rust identifiers and old LSP test
names are absent from `crates/`; the only remaining textual reference to
`AWF-EFX-008` is the supersession note for the previous warning-diagnostics
slice.
