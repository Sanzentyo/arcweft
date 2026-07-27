# Proof convergence: LSP zero-consumer public-surface deletion

Date: 2026-07-27

Status: `LANDED_VALIDATED_WITH_EXISTING_WORKSPACE_BASELINE`

## Boundary

This deletion-driven cut removes five unreleased LSP public APIs with zero
production consumers:

- clamping `LineIndex::byte_offset_from_position` and its now-dead private
  `offset_in_line` helper;
- the `publish_diagnostics` wrapper;
- `LspProfileResolver::resolve_for_uri`;
- `LspReplCommandEndpoint::endpoint_mut`; and
- the orphan `CharacterDefinitionSourceError` enum.

The two position round-trip unit tests now call the existing exact
`try_byte_offset_from_position` authority and require `Ok(expected)`. Invalid
positions therefore cannot silently clamp through a plausible public fallback.
The historical AW-AH-009.3 substrate note was updated to identify that its old
clamping-retention statement describes only that earlier cut.

The active owners remain unchanged:

- `try_byte_offset_from_position` for checked position conversion;
- `publish_diagnostics_from_analysis` over the session-owned analysis cache;
- `resolve_candidate_for_uri` and `resolve_for_document_path` for accepted
  profile construction/publication;
- `LspReplCommandExecutor` and the endpoint's direct `result` execution; and
- `CharacterDefinitionRequestError` for the live definition request path.

One compile-fail fixture proves that all five deleted APIs are unavailable.
This is Rust visibility/type-check evidence, not a source-text gate. No alias,
wrapper, compatibility shim, source reader, source reparse, or removed-syntax
diagnostic replaces the deleted surface.

## Validation

Completed:

- `cargo test -p arcweft-lsp --test public_api --all-features -- --nocapture`:
  both compile-fail fixtures passed;
- `cargo test -p arcweft-lsp --all-targets --all-features`: passed, including
  212 unit tests and every LSP integration/trybuild suite;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `cargo fmt --all -- --check`: passed; and
- `git diff --check`: passed.

`just test-workspace` completed every preceding workspace, CLI, integration,
and compile-fail stage, including the new LSP public-API fixture. The recipe
then stopped at the established `arcw_fixtures_check_run` baseline. The exact
suite reported three passes and the same two failures present at the parent
revision:

- `spec_should_pass_check_fixtures_pass_after_refactor` for
  `010_capability_fs_read.arcw`; and
- `spec_should_pass_run_fixtures_pass_after_refactor` for
  `002_file_read_task.arcw`.

Both fixtures require final attached-HIR publication of capability-owned
`FsError`. This LSP surface deletion neither changes that owner nor adds a
fallback nominal, fixture bypass, compatibility reader, or source gate.

The final design-package ledger compared all 30 retained
`docs/reviews/**/*.zip` archives against package-specific implementation
records: zero unrecorded or changed archives and zero root-inbox ZIPs.

Tier 2 is not applicable. This is an isolated zero-consumer API deletion; it
does not change runtime, render, Agent command behavior, MCP, capture,
persistence, or serialization.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-lsp-zero-consumer-public-surface-deletion-2026-07-27/`](structure-audits/proof-lsp-zero-consumer-public-surface-deletion-2026-07-27/).
The final pass scanned 3,772 files, including 1,956 Rust files and 906,126
physical Rust LOC, and reported zero errors plus 146 existing warnings. Its
warning headings are identical to the immediately preceding audit.

Representative changed metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-lsp/src/diagnostics.rs` | 47,036 | 1,270 | production |
| `arcweft-lsp/src/features/character_definition.rs` | 23,388 | 596 | production |
| `arcweft-lsp/src/positions.rs` | 13,187 | 376 | production |
| `arcweft-lsp/src/profiles/load.rs` | 10,343 | 281 | production |
| `arcweft-lsp/src/repl_command.rs` | 9,237 | 271 | production |
| `arcweft-lsp/tests/ui/removed_zero_consumer_lsp_facades.rs` | 582 | 22 | test |

No new structural error or warning category was introduced. The only changed
production file above the 1,200-line warning threshold is the pre-existing
diagnostics responsibility module, which shrank in this cut.
