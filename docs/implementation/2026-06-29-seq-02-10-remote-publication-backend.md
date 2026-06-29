# Seq-02.10 Remote Publication Backend

Date: `2026-06-29`
Package: `arcweft-seq02.10-remote-publication-backend-2026-06-29.zip`

## Scope

This implementation adds the first production-shaped remote publication backend
for Arcweft release artifacts. The backend is intentionally a remote-like
filesystem/object-directory backend so deterministic local tests can exercise
object-store semantics without private credentials.

The implementation keeps credentials, retry policy, deadlines, object handles,
and filesystem I/O in `arcweft-project-loader` / `arcweft-cli`. No remote
publication behavior is added to `arcweft-bundle`.

## Ownership

- `arcweft-project-loader::release_adapter::publish` continues to own local
  atomic publication.
- `arcweft-project-loader::release_adapter::publish::remote` owns:
  - `ReleaseRemotePublicationBackend`, the adapter-owned backend trait;
  - `ReleaseRemotePublishPlan` and report/state models;
  - dry-run planning and committed publication orchestration.
- `arcweft-project-loader::release_adapter::publish::remote::object_directory`
  owns `ReleaseObjectDirectoryBackend`, the first concrete backend.
- `arcweft-cli::app::release` exposes the backend with
  `arcw release publish --backend object-directory` and `--dry-run`.

## Protocol

The remote publisher preserves the existing local publish ordering by using
`ReleasePublishArtifactKind::commit_rank()` as the commit order. The final AWFR
archive has the largest rank and is written after payloads, patches, bundles,
and signatures.

For committed publication:

1. Validate object keys, duplicate destinations, byte budget, signing
   requirements, and final AWFR count.
2. Write every artifact into a private `.arcweft-remote-staging/<run-id>/...`
   key.
3. Read each staged object back and verify digest and size.
4. In commit order, check that the destination key does not already exist.
5. Copy staged object to its final object key.
6. Read final object back and verify digest and size.
7. Keep staged objects until every final object has been committed and verified.
8. Delete all staged objects best-effort after the full commit succeeds.
9. If any failure occurs, delete visible committed objects and staged objects in
   reverse recovery order and return a typed report.

Dry-run uses the same plan model and ordering, but does not instantiate or touch
a backend.

## Recovery model

`ReleaseRemoteArtifactState` records typed states:

- `planned`: dry-run or pre-write state;
- `staged`: object staged but not committed;
- `uploaded`: object copied into its final key but not yet verified;
- `committed`: final key verified by digest and size;
- `rolled_back`: cleanup removed a visible or staged object after failure;
- `abandoned`: cleanup failed and manual operator action is required.

Failures are surfaced as `ReleaseRemotePublishFailure`, which contains both the
root `ReleaseRemotePublishErrorKind` and the recoverable
`ReleaseRemotePublishReport`.

## Credential handling

The CLI only accepts a profile id and the name of an environment variable
containing a backend secret:

```bash
arcw release publish \
  --backend object-directory \
  --credential-profile ci-release \
  --credential-secret-env ARCWEFT_REMOTE_TOKEN \
  ...
```

The secret value is never serialized. Reports contain only `"<redacted>"` when a
secret was present, and backend error messages are scrubbed through the
credential redactor before they are recorded in the report.

## Applied changes

- `crates/arcweft-project-loader/src/release_adapter/publish/remote.rs`
  defines the remote publication contract, typed plan/report/failure models,
  dry-run planning, and publish orchestration.
- `crates/arcweft-project-loader/src/release_adapter/publish/remote/object_directory.rs`
  implements the deterministic object-directory backend.
- `crates/arcweft-project-loader/src/release_adapter/publish/remote/tests.rs`
  keeps unit coverage out of the production owner module.
- `crates/arcweft-cli/src/app/release.rs` adds `--backend object-directory`,
  `--dry-run`, remote policy/credential options, and a Windows-safe
  `kind:source_path:relative_publish_path` parser.
- Release trust fixtures now write a deterministic `game.awfr.sig` detached
  signature artifact so remote publication tests do not reuse the AWFR archive
  bytes as a signature placeholder.
- The package's proposed standalone integration-test files were folded into the
  existing release trust integration tests. This avoids per-test-crate
  `dead_code` warnings from partially reused fixture support and keeps the
  release trust matrix in one place.

## Tests added

- Unit tests in `publish/remote/tests.rs` cover:
  - dry-run stability;
  - successful object-directory publish;
  - final AWFR committed last;
  - checksum mismatch after upload/write;
  - retryable staging failure;
  - non-retryable commit failure with rollback and abandoned cleanup state;
  - credential redaction from JSON and Debug output.
- `crates/arcweft-project-loader/tests/release_trust_e2e.rs` publishes a
  seq02.9 file-mirror trust fixture to an object-directory, includes signature
  publication before final AWFR publication, and verifies the published AWFR
  through `verify_release_archive`.
- `crates/arcweft-cli/tests/release_trust_json.rs` exercises CLI dry-run JSON,
  committed JSON, Windows absolute artifact source path parsing, and
  `arcw release verify` on the published AWFR.

## Validation status

Applied locally on Windows and validated with:

```bash
cargo fmt --all -- --check
cargo check -p arcweft-project-loader -p arcweft-cli --all-targets --all-features
cargo test -p arcweft-project-loader release --all-features
cargo test -p arcweft-cli release --all-features
cargo test -p arcweft-cli cache --all-features
cargo clippy -p arcweft-project-loader -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

All commands passed. The structure audit reported 0 errors and 119 warnings.

## Design deviations from package overlay

- The object-directory backend and unit tests were split out of the generated
  single `remote.rs` overlay so no new production Rust file crosses the 1,200
  LOC structural audit warning threshold.
- The remote publish integration tests were merged into existing release trust
  tests instead of adding standalone test crates with partially used fixture
  support.
- The CLI artifact spec parser was changed from `splitn(3, ':')` to
  first-colon / last-colon parsing so Windows absolute source paths such as
  `C:\...` work with the existing `kind:source:dest` syntax.
- Successful remote publication now keeps staged objects until all final
  objects are committed and verified, then performs best-effort staged cleanup.
  This is more robust for object-store style commit protocols and still keeps
  final AWFR publication last.
