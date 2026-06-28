# Seq-02.6 / Seq-02.7 Adapter Wiring Validation

This note records the local validation pass after connector-side implementation
of AWFR adapter wiring and release verification reached `main`.

## Source commits

- `2d6e323bf` / `c0d7da4bc`: shared cache network mirror helpers.
- `9d1396f01` range: external payload HTTP(S) fetch, release publish adapter,
  release consume verifier, patch-target materialization payload mode wiring,
  and CLI release publish/verify commands.

## Local fixes

- Restored the `arcweft-cli::app::local_embedding` module declaration that was
  dropped while wiring release commands.
- Formatted the new release adapter modules.
- Fixed the external payload size-mismatch test so the AWFR archive remains
  valid and the mirror bytes are the mismatching input.
- Adjusted the publish adapter test to assert cleanup of the per-run staging
  directory reported by `ReleasePublishReport`.
- Refactored external payload fetch helpers around an internal
  `ExternalPayloadFetchContext` instead of adding a lint allow for argument
  count.
- Made the local HTTP external-payload fixture server read request headers,
  flush the response, and shut down the write side before the test asserts the
  fetch result. This avoids a flaky one-shot test server failure under workspace
  parallelism.
- Reworded the seq-02.8 implementation note so the regression source gate does
  not see the removed compatibility-layer phrase outside `docs/reviews`.

## Validation

Run in this checkout:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-project-loader -p arcweft-cli --all-targets --all-features
cargo test -p arcweft-project-loader --all-targets --all-features
cargo test -p arcweft-cli --all-targets --all-features cache
cargo test -p arcweft-cli --all-targets --all-features release
cargo clippy -p arcweft-project-loader -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
just test-workspace
git diff --check
```

The structural audit reported:

```text
files scanned: 1623
Rust files: 893
Rust physical LOC: 433328
package manifests: 90
violations: 0 error(s), 107 warning(s)
```

## Remaining Follow-up

- The release consume verifier now compiles and is wired, but broader
  end-to-end release trust fixtures with detached signatures, signed patch
  artifacts, materialized targets, and external payload cache state should be
  expanded in a later hardening cut:
  `docs/reviews/requests/2026-06-29-seq-02.9-release-trust-e2e-fixtures-package.md`.
- Live remote publication remains represented by local atomic staging in this
  cut. Real remote upload backends should be adapter-specific and must keep
  credentials, clocks, and network clients out of `arcweft-bundle`:
  `docs/reviews/requests/2026-06-29-seq-02.10-remote-publication-backend-package.md`.
