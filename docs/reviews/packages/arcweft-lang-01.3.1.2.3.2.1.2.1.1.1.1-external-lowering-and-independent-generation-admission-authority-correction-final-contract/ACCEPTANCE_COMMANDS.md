# Required implementation acceptance commands

These commands are requirements for the future production implementation, not
commands claimed as run for this design-only archive.

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Focused executable evidence must include:

```bash
cargo test -p arcweft-runtime-plan --test ui external_lowerer_builds_plan
cargo test -p arcweft-runtime-plan --test ui external_lowerer_builds_awbc
cargo test -p arcweft-core --test ui
cargo test -p arcweft-core generation_admission
cargo test -p arcweft-core plan_admission
cargo test -p arcweft-core awbc_audio
cargo test -p arcweft-core runtime_product
cargo test -p arcweft-core opaque
cargo test -p arcweft-bundle runtime_generation
cargo test -p arcweft-runtime-driver generation
cargo test -p arcweft-runtime-driver restore
cargo test -p arcweft-runtime-driver replay
cargo test -p arcweft-runtime-driver swap
```

Codec/golden tests must deserialize through private version-1 wire DTOs and the
same checked builders. No command may be replaced by a source-string grep or
file-placement gate. Slow tiers not run must be recorded as not run, never as
passed.
