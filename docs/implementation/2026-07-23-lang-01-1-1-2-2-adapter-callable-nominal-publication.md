# Lang-01.1.1.2.2 accepted adapter/Rust nominal publication

## Source package and scope

The implementation source of truth is
[`docs/reviews/packages/2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d.zip`](../reviews/packages/2026-07-22-lang-01.1.1.2.2-final-contract-main-4fd6331d.zip),
with SHA-256
`4518dc6d81a6435b7514ce7bdcd3887df87a857a8bc9eaa5df14df62dbd59c95`.
The package was designed against Git
`4fd6331dc342d30a7f4ac7774852b60801866ef7`; implementation was reconciled
against the newer local `main` parent
`3acc9cfec034d00cee173e41cbfb37cd46115c50`.

This cut replaces the unpublished string/context-free adapter publication
shape directly. It does not add a dual reader, compatibility alias, migration
shim, permanent removed-syntax diagnostic, or source gate.

## Implemented boundary

- Rust ABI manifests use nominal `RustPackageId`, typed Rust paths, typed
  parameter identities, and recursively structured type references. Generic
  declaration metadata is retained while unsupported generic exports fail
  through structured validation.
- Adapter manifests distinguish adapter-native nominal owners from Rust
  package owners, carry typed nominal paths and arguments, and compute their
  manifest digest from canonical typed content.
- Each selected manifest produces one deterministic generated source document,
  an exact source map for every publication item/type node, neutral
  environment-registration input, and its typed manifest digest.
- `ProjectRegistrationFacts` is the sole registration request carrier for
  environment inputs. The former separately prebuilt CLI/compiler callable
  publication route and adapter-context semantic conversion module are gone.
- Registration first builds one accepted nominal world, then projects Rust
  metadata and callable schemas through that world, stamps each publication,
  and commits one immutable registered callable/metadata catalog.
- Adapter-native external declarations retain both their semantic nominal owner
  (`adapter:<selected-adapter>`) and their exact value binding. The two
  identities are no longer conflated in one string.
- Callable parameters, results, curried groups, method receivers, nested
  containers, and generic arguments retain exact `TypeKind::AcceptedNominal`
  values. Failed projection publishes no partial callable or metadata record.
- Semantic type, callable schema, publication, callable catalog, accepted Rust
  metadata, manifest, and registered-environment digests use deterministic
  typed encodings with checked `u32` sequence/string lengths and fixed-width
  tags/scalars.
- Persistent compiler queries use the registered environment digest whenever
  a complete accepted world exists; the existing persistent schema remains at
  its initial version.
- LSP signature requests are keyed by the registered environment digest and
  retain the selected candidate and exact semantic types. Accepted Rust and
  adapter-native nominal hover/navigation/completion use accepted records and
  generated source evidence. Raw manifest type completions are suppressed when
  the accepted world already owns the same type label, and Arcweft source
  labels use dotted paths only at final presentation.

## Identity and presentation invariant

`AcceptedNominalId` remains owner-qualified semantic identity. An
`AcceptedNominalType` source label intentionally renders only the authored
canonical path plus arguments. Thus two owner-distinct types can display the
same short source form while remaining unequal in `TypeKind`, callable keys,
overload selection, and semantic digests. Tooling that needs to distinguish
them exposes the owner-qualified accepted ID as completion/hover detail rather
than reparsing the display string.

## Verification status

The following direct and integration evidence passed in this checkout:

- `cargo test -p arcweft-lsp`: 171 unit tests, 3 character-completion tests,
  and the remaining integration/doc-test targets. This includes native
  registered-adapter signature help, accepted Rust nominal hover/navigation,
  inaccessible completion filtering, stale request rejection, and mounted
  external-module completion/hover.
- `cargo test -p arcweft-lang-sema --lib`: 954 tests.
- `cargo test -p arcweft-adapter-context`: 17 unit tests, one public API
  integration test, and its doc-test target. The earlier all-feature lib run
  also passed its 27-test matrix.
- the `arcweft-rust-abi` unit suite and Rust ABI macro compile-fail/export
  suites.
- `cargo test -p arcweft-project-loader --lib --tests`: 133 unit tests plus
  dependency-direction, public API compile-fail, and release-trust integration
  suites.
- compiler project-cache transaction, dialogue-profile admission, and View
  product suites: 21 tests.
- the seven-row call-surface signature matrix and the four-row external
  nominal projection matrix.
- focused bundle resource codecs, registered-adapter signature help, and the
  verifier/LSP complex Rust-adapter type projection.
- `cargo check --workspace --all-targets --all-features` and
  `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
  The warning-denying Clippy run covers the whole workspace, not only the
  directly affected crates.
- `cargo fmt --all -- --check` and `git diff --check`.

This cut adds two item-local `clippy::too_many_lines` allowances. Both belong
to `types/digest.rs`: one keeps the exhaustive fixed-tag `TypeKind` semantic
digest table together, and the other keeps the exhaustive fixed-tag
`EntityKind` identity table together. Each has an item-local `reason`; the cut
adds no crate/file-wide lint suppression, command-line exception, or unchecked
constructor. The incremental disposition is recorded in
[`lint-allow-tracker.md`](lint-allow-tracker.md).

The canonical structural audit scanned 3,601 files, including 1,897 Rust
files and 885,639 physical Rust lines across 94 manifests. It reported zero
errors and 142 warnings.

The normal workspace route was run with `CARGO_BUILD_JOBS=1` after a parallel
Windows link attempt hit OS page-file error 1455. The single-job run passed
the complete non-CLI workspace suite, the CLI library/binaries, and the
selected CLI integrations, then reached two already documented Proof-switch
gates:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both fail because the still-public detached `ExternCapabilityItem` publishes
functions but not its typed `type FsError` member. The private lossless grammar
already owns that member, but the Proof Stage 3 atomic public AST/HIR switch is
required before it may enter the accepted nominal world. This cut does not add
a global `FsError`, `TypeKind::Named` fallback, partial capability reader, or
other compatibility path to hide that existing sequencing dependency.

`CARGO_BUILD_JOBS=1 just test-tier2` passes all 46 selected tests: 22 MCP
stdio tests, one slow Agent-observe test, 16 native auxiliary-capture cases,
two visual-smoke cases, one checked-in golden-integrity case, and four exact
PNG/imq goldens. The first run exposed seven stale source fixtures whose flows
returned `String` while omitting the now-authoritative result type (three
shared samples and four native golden fixtures). Adding the exact `-> String`
annotations made the four original MCP failures and all four golden captures
pass without changing response schemas, image tolerances, or runtime behavior.

## Follow-up boundary

The runtime consumption of generated artifacts remains intentionally outside
this implementation cut until the independently throwable design request
[`2026-07-20-lang-01.5.1.3-generated-artifact-runtime-binding-contract.md`](../reviews/requests/2026-07-20-lang-01.5.1.3-generated-artifact-runtime-binding-contract.md)
returns with a single binding key/catalog/revision contract. This cut does not
invent that contract or weaken the registered semantic environment boundary.
