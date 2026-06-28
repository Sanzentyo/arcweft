# Remaining Work Cut-Out: seq-02.6 / seq-02.7

This file deliberately separates what is not fully implemented or not locally verified in this package.

## Not fully implemented

### CLI external-payload command

Resolved in this repository application. `arcw cache fetch-external` is wired to
`cache::external_payload::fetch_external_payload_to_cache` and covered by
`cargo test -p arcweft-cli cache`.

The package's separated draft patch was not applied directly because it is not a
valid unified patch for this checkout. The equivalent command was implemented
against the current `crates/arcweft-cli/src/app/cache.rs`.

### HTTP(S) external payload fetch adapter

The project-loader external payload adapter supports `file:` and `arcweft-cache:` mirrors only. HTTP(S), auth profiles, proxy profiles, client profiles, retry budget, and cancellation should reuse the boundary style of the existing release bundle fetch adapter.

### Publish adapter and atomic remote publication

The Sans I/O rewrite plan exists, but a release-publish adapter still needs to:

1. stage target AWFB and payload bytes,
2. upload mirrors,
3. generate signatures,
4. write the final AWFR archive atomically,
5. roll back or leave an explicitly recoverable staging directory on failure.

### Target signature regeneration adapter

`SigningPolicy` can decide that a changed materialized target requires an adapter signature, but this package does not implement key access or signing. The existing CLI `sign-bundle` path is the closest adapter to reuse.

### Full release trust verifier

The package defines typed inspection states and transcripts, but the project-loader/player path still needs a shared verifier that combines:

- AWFR archive signature validation,
- release manifest validation,
- signed base bundle validation,
- signed patch validation,
- materialized target validation,
- external payload digest validation.

### Patch materialization payload mode wiring

Existing patch materialization can preserve metadata-only external descriptors. This package adds carrier and policy types, but it does not yet thread `ExternalPayloadMaterializationMode` through `apply_patch_bundle` or a higher-level adapter materialization workflow.

## Not locally verified

The original packaging environment had no local Rust toolchain and no local clone of the private repo. In this repository checkout, the focused Rust validation was run as part of application:

```bash
cargo fmt
cargo test -p arcweft-bundle release::archive
cargo test -p arcweft-bundle release::signing_policy
cargo test -p arcweft-project-loader cache::external_payload
cargo test -p arcweft-cli cache
cargo clippy -p arcweft-bundle -p arcweft-project-loader -p arcweft-cli --all-targets --all-features -- -D warnings
```

The repository-side validation commands are listed in `VERIFICATION.md`.

## Suggested next implementation slices

1. Apply and compile the main patch.
2. Fix any formatting/clippy findings.
3. Add HTTP(S) external payload fetch using the same policy checks as release bundle fetching.
4. Add an adapter-level materialization command that fetches required external payloads, applies the patch, signs the target, rewrites AWFR, and commits atomically.
5. Add a shared release verifier that returns only typed `SigningInspectionResult` / payload-cache reports to callers.
