# Seq-02.6 / Seq-02.7 AWFR × Signing Boundary Implementation Note

## Boundary decision

The split is:

- `arcweft-bundle` owns deterministic data models, validation, digest transcripts, signature-disposition decisions, and metadata-only rewrite planning.
- `arcweft-project-loader` owns filesystem/cache fetch adapters and cache key materialization.
- CLI/release-publish adapters own key access, clocks, network credentials, mirror upload, signature creation, and atomic file publication.
- Runtime-driver/player layers consume typed verification/inspection states and must not reimplement policy logic.

This keeps AWFR and signing policy coordinated without making release trust depend on payload fetching convenience.

## Implemented Sans I/O schemas

### AWFR archive

`AwfrArchiveManifest` binds:

- `schema_version`
- `channel`
- existing `ReleaseManifest`
- optional publication metadata
- patch artifact references
- external payload carriers
- archive-level signatures

Canonical JSON serialization sorts release bundles, mirrors, patch refs, payload carriers, and signatures before encoding. `unsigned_identity_digest()` clears archive signatures and hashes the canonical unsigned archive. `external_payloads_digest()` separately hashes the external carrier set for signing transcripts.

### External payload carrier

`ExternalPayloadCarrier` binds:

- descriptor id
- bundle content root
- bundle kind
- section kind code and section schema version
- residency and required flags
- media type
- compression
- decoded size/digest
- compressed size/digest
- cache-key epoch fields
- mirrors

`verify_stored_bytes()` validates compressed bytes by size and digest, then decodes if needed and validates decoded size/digest. `verify_decoded_bytes()` validates already-decoded payload bytes. Both are Sans I/O checks.

### Release manifest rewrite

`ReleaseManifestRewritePlan` supports:

- optional target bundle replacement/addition in the release manifest
- external carrier add
- external carrier replace with old decoded digest guard
- external carrier remove with old decoded digest guard

This is metadata mutation only. Fetching bytes, staging outputs, publishing mirrors, and rollback are adapter responsibilities.

## Implemented signing policy model

`SigningPolicy` has explicit modes:

- `local_dev`
- `ci`
- `release_publish`
- `release_consume`
- `offline_inspection`
- `test_fixture`

`SigningSubjectKind` covers:

- AWFB bundle
- patch v2 artifact
- materialized target bundle
- AWFR release archive
- external payload

`SigningDigestTranscript` deterministically binds:

- subject kind
- channel
- signer id
- key epoch
- bundle kind
- artifact identity
- target artifact identity where relevant
- content root / target content root
- manifest digest
- whole-file digest
- AWFR archive identity digest
- AWFR external-payload set digest

`SignatureDisposition` makes target materialization explicit:

- unchanged targets may preserve the existing signature
- changed targets never report the base signature as valid for target bytes
- release policies require an adapter-generated target signature
- local-dev policies may allow unsigned materialized targets, but only as an explicit state

## Implemented adapter model

`cache::external_payload` reads an AWFR archive, finds one `ExternalPayloadCarrier`, and fetches bytes through:

- `arcweft-cache:` object lookup
- `file:` mirrors relative to the archive directory or absolute paths

It verifies compressed and decoded payload bytes through the carrier and writes immutable cache object/record entries with:

- key epoch
- bundle content root
- descriptor id
- compressed digest
- decoded digest
- media type

Network mirrors are explicitly skipped in this adapter cut; the existing release bundle fetch adapter remains the reference for HTTP(S) policy handling.

## Error and rollback behavior

Sans I/O code returns typed errors for:

- unsupported schema version
- invalid channel/media type
- missing release bundle
- duplicate carriers
- cache key mismatch
- digest and byte-length mismatch
- old-digest mismatch during rewrite
- invalid patch/signature publication metadata

Adapter code records per-mirror attempts and only stores validated bytes. Because `FilesystemCacheStore` uses immutable object and record writes, failed validation does not mutate a successful record for that payload key.

## Test matrix covered by new source tests

- external payload carrier binds an external section descriptor and validates bytes
- deterministic AWFR archive JSON round-trip
- external carrier add/replace/remove rewrite guards
- release policy rejects unsigned materialized targets
- local-dev policy explicitly allows unsigned materialized targets
- transcript digest changes when channel changes
- changed materialized targets never preserve base signature validity
- file-mirror external payload fetch populates the cache

## CLI smoke commands

This repository application also wires the local/cache external payload path into
the CLI:

```bash
# Release bundle fetch path
arcw cache fetch --manifest game.awfr --content-root <bundle-root> --root target/arcweft/cache/v1 --json

# External-payload fetch path
arcw cache fetch-external \
  --archive game.awfr \
  --bundle-content-root <bundle-root> \
  --descriptor-id <section-id> \
  --root target/arcweft/cache/v1 \
  --json

# Existing AWFB signing adapter
arcw sign-bundle --input target.awfb --output target.signed.awfb --signer-id release-key-main --key-epoch 1 --signing-key-file key.hex --json
```

The package's separated draft patch was not applied directly because it was not
a valid unified patch for this checkout. The equivalent command was implemented
against the current CLI file and verified by `cargo test -p arcweft-cli cache`.

## Dependency notes for signing policy

AWFR archive signing must include:

- archive unsigned identity digest
- channel
- key epoch
- signer id
- release manifest digest
- external payload set digest
- whole AWFR file digest if the adapter signs a serialized archive file

Patch signing must include:

- patch artifact identity
- target artifact identity
- target content root
- patch whole-file digest
- channel and key epoch

Materialized target signing must include:

- target artifact identity
- target content root
- target manifest digest
- target whole-file digest
- channel and key epoch

## Repository validation

After applying the package in this repository checkout, validation was run with:

```bash
cargo fmt --all
cargo test -p arcweft-bundle release::archive
cargo test -p arcweft-bundle release::signing_policy
cargo test -p arcweft-project-loader cache::external_payload
cargo test -p arcweft-cli cache
cargo clippy -p arcweft-bundle -p arcweft-project-loader -p arcweft-cli --all-targets --all-features -- -D warnings
cargo test -p arcweft-bundle --all-targets --all-features
cargo test -p arcweft-project-loader --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
just test-workspace
```

The attempted combined command
`cargo test -p arcweft-bundle -p arcweft-project-loader -p arcweft-cli --all-targets --all-features`
timed out at the tool boundary before returning a usable result, so the same
coverage was taken through separated target-crate commands plus
`just test-workspace`.

The structural audit reported:

```text
files scanned: 1607
Rust files: 887
Rust physical LOC: 431785
package manifests: 90
violations: 0 error(s), 107 warning(s)
```

Changed Rust file sizes in the applied checkout:

| Path | Bytes | LOC | Role |
| --- | ---: | ---: | --- |
| `crates/arcweft-bundle/src/release.rs` | 81068 | 2065 | production, existing release manifest/signature module |
| `crates/arcweft-bundle/src/release/archive.rs` | 37329 | 926 | production, AWFR archive and external carrier model |
| `crates/arcweft-bundle/src/release/signing_policy.rs` | 26405 | 670 | production, signing policy and transcript model |
| `crates/arcweft-project-loader/src/cache.rs` | 297 | 10 | facade |
| `crates/arcweft-project-loader/src/cache/external_payload.rs` | 15753 | 431 | production, local/cache external payload adapter |
| `crates/arcweft-cli/src/app/cache.rs` | 20471 | 548 | production/test, cache CLI commands |

The newly introduced responsibility modules remain below the 1200 LOC warning
threshold. `release.rs` is an existing broad module in the structural warning
band; this cut only adds module declarations there.
