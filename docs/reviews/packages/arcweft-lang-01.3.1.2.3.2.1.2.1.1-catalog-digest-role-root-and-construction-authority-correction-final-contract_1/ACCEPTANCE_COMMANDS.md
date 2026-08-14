# Compile-clean implementation acceptance commands

Run from a clean checkout at the implementation head after the ordered migration. These are specified gates, not checks claimed by this design-only package.

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p arcweft-interaction-model
cargo test -p arcweft-character
cargo test -p arcweft-view
cargo test -p arcweft-core pattern nominal_record plan awbc
cargo test -p arcweft-lang-sema character_dialogue
cargo test -p arcweft-runtime-plan
cargo test -p arcweft-dialogue character_dialogue
cargo test -p arcweft-runtime-driver
cargo test -p arcweft-bundle
cargo test -p arcweft-save
cargo test --workspace --all-features
```

Run the repository-owned structure audit and every Tier 2 command named by the current root/scoped `AGENTS.md`; do not substitute a guessed script name. Add codec/golden/tamper fixtures for Character digest, View digest, generation body, AWBC domain table, `MakeRecord`, record constants, restore, and save. Use compile-fail fixtures for all private authority constructors/fields and the deleted unchecked/boolean surfaces.

## Per-phase rule

After each phase in `decision-15-implementation-order.md`, run format plus the narrow crate check/tests for every changed owner and its direct downstream consumer. No temporary compatibility alias, optional field, fallback, old reader, duplicate enum helper, or source-spelling gate may be used to bridge phases.
