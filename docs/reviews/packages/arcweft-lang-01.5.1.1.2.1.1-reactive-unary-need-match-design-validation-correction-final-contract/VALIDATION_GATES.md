# Production implementation validation gates

These command families are normative admission evidence, not claims that this
design-only return executed Arcweft production tests. Resolve exact project
aliases/scripts from then-current root and nested AGENTS before running.

## Focused

```text
cargo test -p arcweft-lang-sema view_need_match
cargo test -p arcweft-core need_publication awbc_match
cargo test -p arcweft-view match mount
cargo test -p arcweft-bundle view_resource_codecs
cargo test -p arcweft-runtime-driver view_need
```

## Workspace quality

```text
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo doc --workspace --all-features --no-deps
cargo test --doc --workspace
```

## Structural/tamper/differential

Run all old-API absence scans; old-byte/unknown-field/digest/index/cursor/payload/
transaction tamper cases; exact/one-over limits; live-versus-replay traces;
save/restore and replacement matrices.

## Consumers

Run native, Web, headless, Agent/MCP, and generated-artifact parity. Headless is
the differential oracle. Regenerate committed artifacts and require a clean diff
with no old Await spelling/tag/discriminant.

## Tier-2

Run all platform/feature/Tier-2 commands mandated by current repository
instructions. An unpermitted skip blocks full implementation readiness.
