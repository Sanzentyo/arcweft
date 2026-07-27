# Proof convergence: project-loader character reader and registration projection deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

Jujutsu change audited: `lppvkzmvorsovlkylqlruokoyzummknw`

## Boundary

This deletion-driven cut removes the unreleased project-loader character
manifest loading island and two zero-consumer registration projections.

Removed character-manifest surfaces are:

- `LoadedCharacterManifest`, including its duplicate document/path/manifest
  ownership and every accessor;
- filesystem readers `character_manifest::load` and
  `character_manifest::load_for_project`;
- the forwarding `character_manifest::decode` helper;
- the five-variant `character_manifest::LoadError` wrapper; and
- the unreachable `ProjectRegistrationLoadError::CharacterManifest` variant.

The `character_manifest` module is now private and owns only the lexical
`.awchar` manifest-path rule used by the profile topology loader. The topology
already owns the exact retained `SourceDocument` and manifest path, so it now
calls
`SourceBackedCharacterManifest::decode_registration_json` directly and retains
the typed `CharacterRegistrationDecodeError` in
`ProfileTopologyLoadError::CharacterManifest`. No file is reread and no source
identity is reconstructed.

Removed registration projections are:

- `LoadedProjectRegistration::facts`; and
- `LoadedProjectRegistration::file_documents`.

The consuming `LoadedProjectRegistration::into_parts` operation remains the
single public owner. LSP already used that operation; the loader's internal
tests were migrated to the same boundary. No renamed reader, compatibility
alias, wrapper, dual loader, source gate, or source-string parser replaces any
deleted surface.

## Retained owners and non-goals

This cut retains:

- profile-topology acquisition, aggregate source/work limits, overlay
  precedence, and exact document identity;
- `LoadedFileDocument` and its ownership/access projections;
- `LoadedProjectRegistration::into_parts`, used by accepted LSP profile
  publication;
- `SourceBackedCharacterManifest`, its source map, registration fingerprint,
  and typed decoder; and
- the active `ProfileTopologyLoadError::CharacterManifest` diagnostic with its
  exact resource ID and path.

It does not modify the active old Dialogue production carriers or guess the
blocked Proof semantic-leaf source-owner schema. The corrected Proof
01.1.1.4.1 archive remains only partially implementation-ready pending
[`01.1.1.4.1.1`](../reviews/requests/2026-07-27-seq-proof-01.1.1.4.1.1-source-owner-and-semantic-consistency-correction.md).

## Validation

Completed:

- `cargo fmt --all` and final `cargo fmt --all -- --check`: passed;
- `cargo check -p arcweft-project-loader --all-targets --all-features`:
  passed;
- the extended `removed_zero_consumer_project_facades` trybuild row: passed,
  proving the character-manifest facade and both borrowed registration
  projections and the obsolete registration-load error variant are
  unavailable;
- `cargo test -p arcweft-project-loader --all-targets --all-features`: passed,
  including 134 unit tests, four dependency-direction tests, both public-API
  compile-fail rows, and six release-trust end-to-end tests;
- `cargo clippy -p arcweft-project-loader --all-targets --all-features -- -D
  warnings`: passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `git diff --check`: passed; and
- the final ZIP ledger compared all 30 retained `docs/reviews/**/*.zip`
  archives against implementation records: zero unrecorded hashes and zero
  root-inbox ZIPs.

`just test-workspace` ran for 595.9 seconds. It passed the changed loader, its
updated trybuild row, and every preceding suite, then stopped only at the
established `arcweft-cli --test arcw_fixtures_check_run` baseline. The exact
suite was rerun and reported three passes plus the same two failures present at
the parent revision:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both rows require final attached-HIR publication of the capability-owned
`FsError`. This cut does not touch that owner and adds no fallback nominal,
fixture bypass, compatibility reader, or source gate.

Tier 2 is not applicable. This cut narrows an unused project-loader API and
removes a duplicate filesystem reader; it changes no runtime, render, Agent,
MCP, capture, persistence, network protocol, or serialized data contract.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-project-loader-zero-consumer-character-reader-deletion-2026-07-27/`](structure-audits/proof-project-loader-zero-consumer-character-reader-deletion-2026-07-27/).
It scanned 3,789 files, including 1,959 Rust files and 905,885 physical Rust
LOC across 95 manifests. It reported zero errors and 146 existing warnings.
The warning inventory is unchanged except that the existing
`topology/loader.rs` size warning reports 1,342 rather than 1,341 physical LOC:
the direct typed decoder import replaces the deleted helper call without
adding a responsibility. The character-manifest owner shrank from 179 to 34
physical LOC.

Current changed-file metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-project-loader/src/lib.rs` | 391 | 14 | facade |
| `arcweft-project-loader/src/character_manifest.rs` | 1,036 | 34 | production with focused unit tests |
| `arcweft-project-loader/src/environment.rs` | 27,882 | 815 | production with embedded tests |
| `arcweft-project-loader/src/topology/loader.rs` | 51,792 | 1,342 | production topology orchestration |
| `arcweft-project-loader/src/topology/model.rs` | 35,179 | 1,105 | production model/error owner |
| `arcweft-project-loader/tests/ui/removed_zero_consumer_project_facades.rs` | 877 | 31 | compile-fail test |

No manifest, dependency edge, feature, serialized payload, or crate dependency
changed. The existing loader hotspot remains a future responsibility-review
candidate; this cut removes 145 lines from its adjacent obsolete owner and
does not broaden the orchestration module.

## Next boundary

The Agent REPL `source` module visibility and compiler/runtime typed-evidence
payloads are independently audited deletion candidates. They remain separate
cuts. Active numeric, Duration, compact-sequence, Dialogue, and Proof leaf
readers stay frozen until their typed replacement/correction boundary is ready.
