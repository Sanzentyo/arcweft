# Reference model

This dependency-free crate exercises the structural invariants selected by the
final contract: group/parameter coordinates, exact prefix products, disposition
and rest validation, stale/foreign/type failures, empty-group progress, atomic
final open, and hot-reload classification.

It is not an Arcweft production patch and its clonable fixture data is not the
production affine runtime API. Production owners and no-clone affine rules are
specified in `../RUST_TYPES_AND_OWNERS.md`.

Expected validation commands:

```bash
cargo fmt --manifest-path model/Cargo.toml -- --check
cargo test --manifest-path model/Cargo.toml
cargo clippy --manifest-path model/Cargo.toml --all-targets -- -D warnings
```
