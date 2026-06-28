# Seq-02.9 Release Trust E2E Fixtures

Date: `2026-06-28`
Applied locally: `2026-06-29`

This note records the applied Seq-02.9 release trust hardening slice. The
uploaded patch files were malformed as unified diffs, so the overlay files and
patch contents were reconciled manually against current `main`.

## Implemented by overlay

- Deterministic test-only Ed25519 fixture key and generated release graph.
- Typed release trust evidence in `arcweft-project-loader::release_adapter::trust`.
- `ReleaseConsumeVerificationReport.success` and `trust` evidence list.
- `arcw release verify --json` stable success/failure JSON with non-zero exit on
  `success=false`.
- E2E fixture tests for signed base, signed patch, materialized target, target
  signature, external payload cache/mirror state, AWFR transcript verification,
  wrong policy, and typed failure evidence.
- Static fixture policy metadata under `fixtures/release-trust/`, including the
  explicit test-only key policy and expected matrix.

## Boundary decisions

- `arcweft-bundle` remains Sans I/O; it only gains deterministic unsigned AWFR
  transcript helpers on `AwfrArchiveManifest`.
- Filesystem reads, cache state, local HTTP fixture servers, and fixture private
  key material stay in adapters/tests.
- No remote publication backend or production key-management backend is added.

## Validation status

Passed in this checkout:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-bundle release --all-features
cargo test -p arcweft-project-loader release --all-features
cargo test -p arcweft-project-loader external_payload --all-features
cargo test -p arcweft-project-loader release_trust --all-features -- --nocapture
cargo test -p arcweft-cli release --all-features
cargo test -p arcweft-cli cache --all-features
cargo test -p arcweft-cli --test release_trust_json --all-features -- --nocapture
cargo check -p arcweft-bundle -p arcweft-project-loader -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-bundle -p arcweft-project-loader -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

Structural audit result: `1901` files scanned, `975` Rust files,
`464603` Rust physical LOC, `0 error(s), 115 warning(s)`.
The audit report was written to `target/structure-audit-seq02-9`.

## Non-goals

- Real remote publication backend, credential storage, KMS, CDN, registry, or
  production signing-key management remain outside Seq-02.9.
- The deterministic Ed25519 seed is fixture-only material and must not be copied
  into any production signing profile.
