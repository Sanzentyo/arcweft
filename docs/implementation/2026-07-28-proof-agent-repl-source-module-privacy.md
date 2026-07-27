# Proof convergence: Agent REPL source-module privacy

Date: 2026-07-28

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

Jujutsu change audited: `vuqlqnqyoluwsxvlvlplrykrptmsrpls`

## Boundary

This deletion-driven cut removes the zero-consumer public
`arcweft_agent_repl::source` namespace. The module now remains private to
`arcweft-agent-repl`.

The module did not expose a usable external API before this cut:

- `ParsedReplCell` and all of its fields were already `pub(crate)`;
- `classify_repl_cell` was already `pub(crate)`; and
- every remaining helper was private.

A repository-wide consumer audit found no external
`arcweft_agent_repl::source` use. The retained consumers are the crate-owned
compile/session paths: `compile.rs` reads `ParsedReplCell`, and `session.rs`
classifies cells and retains the result. Their behavior and ownership are
unchanged.

No wrapper, root re-export, alias, compatibility module, duplicate parser,
source gate, or removed-syntax diagnostic replaces the deleted public
namespace.

## Direct evidence

The new trybuild row imports only `arcweft_agent_repl::source`. This import
compiled while the declaration was `pub mod source` and now fails with Rust
error E0603 after the declaration became `mod source`.

The fixture deliberately does not name `ParsedReplCell`: that type was already
crate-private, so naming it would not isolate the module-visibility change.
This is direct type/visibility evidence rather than a source-text gate.

## Contract boundary

This cut does not publish or redesign the Proof typed syntax/HIR/project
authority. The corrected Proof 01.1.1.4.1 archive remains only partially
implementation-ready pending
[`01.1.1.4.1.1`](../reviews/requests/2026-07-27-seq-proof-01.1.1.4.1.1-source-owner-and-semantic-consistency-correction.md).
No source-owner, pathless-variant, Duration comparison, overflow-owner, or
elided-region decision is inferred here.

## Validation

Completed:

- `cargo fmt --all`: passed;
- `cargo check -p arcweft-agent-repl --all-targets --all-features`: passed;
- `cargo test -p arcweft-agent-repl --all-targets --all-features`: passed,
  including 11 unit tests, the new API compile-fail row, and 23 integration
  tests;
- `cargo clippy -p arcweft-agent-repl --all-targets --all-features -- -D warnings`:
  passed;
- `cargo check --workspace --all-targets --all-features`: passed; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed.

`just test-workspace` ran for 542.4 seconds. It passed the changed Agent REPL
crate, the new compile-fail row, and every preceding workspace suite. It
stopped only at the established `arcweft-cli --test arcw_fixtures_check_run`
baseline. The exact suite was rerun and reported three passes plus the same two
failures present at the parent revision:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both rows await final attached-HIR publication of the capability-owned
`FsError`. This cut does not touch that owner and adds no fallback nominal,
fixture bypass, compatibility reader, or source gate.

Tier 2 is not applicable: this is an isolated module-visibility reduction. It
does not change Agent request handling, runtime execution, transport, MCP,
rendering, capture, or persisted data.

The final ZIP ledger contains 30 retained `docs/reviews/**/*.zip` archives,
zero unrecorded hashes, and zero root-inbox ZIPs. No returned
Proof 01.1.1.4.1.1 correction archive exists.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-agent-repl-source-module-privacy-2026-07-28/`](structure-audits/proof-agent-repl-source-module-privacy-2026-07-28/).
It scanned 3,793 files, including 1,961 Rust files and 905,897 physical Rust
LOC across 95 manifests. It reported zero errors and 146 existing warnings.

Current changed-file metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-agent-repl/src/lib.rs` | 2,073 | 51 | facade/module visibility |
| `arcweft-agent-repl/Cargo.toml` | 1,129 | 41 | package manifest |
| `arcweft-agent-repl/tests/api_compile.rs` | 165 | 5 | API compile-fail driver |
| `arcweft-agent-repl/tests/ui/source_module_private.rs` | 46 | 3 | API compile-fail fixture |
| `arcweft-agent-repl/tests/ui/source_module_private.stderr` | 272 | 11 | deterministic compiler diagnostic |

All production owners remain below structural warning thresholds. The added
development dependency is inherited from the root workspace and introduces no
production dependency edge or feature.

## Next boundary

Continue deletion-driven audits of independently provable zero-consumer public
surfaces while the Proof leaf correction remains pending. Do not delete or
extend active raw leaf readers, the accepted private Dialogue exterior, or the
07.8 function-effect callable identity without their typed replacement
authority.
