# AW-AH-009.4.3 speaker-derived ID tooling deletion

## Status

`IMPLEMENTED_VALIDATED_WITH_INHERITED_PROOF_GATE`

This deletion cut removes the user-facing tooling path that synthesized
Dialogue line IDs from flow, speaker/callee, lexical-scope strings, and mutable
line counters. It adds no temporary replacement.

## Package decision

The accepted AW-AH-009.4.3 package is
`docs/reviews/packages/zips/arcweft-aw-ah-009.4.3-source-site-line-identity-project-diagnostics-production-reconciliation-final-contract.zip`
with SHA-256
`FD9F97D37B857991120DD5E5E5DB27953257121FC48C79BEEF4FA03DF1F23396`.
It is `READY_FOR_IMPLEMENTATION` and explicitly forbids Character, speaker,
callee spelling, alias, or display-name input to line identity. Final hints and
source actions must borrow the accepted project inventory produced from the
typed source application owner.

The previous `collect_id_context` Dialogue branch contradicted that contract:
it scanned old `SpeakerLine`/`ContentCall` typed-tree carriers, derived a
`DialogueSpeakerSlug`, accumulated named scope strings, advanced mutable
counters, and projected `say.*`/`text.*` edits into tooling, CLI, verify-LSP,
and LSP. Keeping it frozen would still publish incorrect identities to users,
so it is deleted before the final inventory is available.

This is not the premature removal of the only execution path. The current
parser/HIR/runtime Dialogue carrier still supplies production execution and is
left frozen until the AW-AH-009.4.2/.3 public authority switch can replace it
atomically. Only the non-executing, misleading materialization/hint branch is
removed here.

## Deleted surface

- removed `IdContextKind` Dialogue variants;
- removed `IdContextOption` and `InsertDialogueOptions`;
- removed typed-tree Dialogue discovery, flat-fence/source head parsing,
  speaker slugging, relative Dialogue option normalization, scope stacks, and
  mutable line counters from `collect_id_context`;
- removed tooling edit/hint projection for the deleted materialization;
- removed tests that required old speaker-derived `say.*`/`text.*` output;
- changed CLI and LSP tests to prove Dialogue is left untouched while
  declaration and choice materialization continues; and
- updated implementation-state documentation to distinguish the frozen
  execution path from the deleted tooling path.

No compatibility alias, dual inventory, source gate, removed-syntax
diagnostic, replacement string scanner, or speculative final identity was
added. `AGENTS.md` already requires this deletion-first behavior, so no policy
edit was needed.

## Focused evidence

```text
rg -n "IdContextOption|InsertDialogueOptions|DialogueMissingOptions|IdContextKind::DialogueLineId|IdContextKind::DialogueTextKey|collect_dialogue_ids|next_generated_line_id" crates --glob '*.rs'
  PASS: no occurrence
cargo check -p arcweft-lang-hir -p arcweft-tooling -p arcweft-verify-lsp -p arcweft-lsp -p arcweft-cli --all-targets
  PASS
CARGO_PROFILE_TEST_DEBUG=0 cargo test -p arcweft-lang-hir id_context -- --nocapture
  PASS: 1 passed, 0 failed
CARGO_PROFILE_TEST_DEBUG=0 cargo test -p arcweft-tooling materializes_top_level_and_choice_ids -- --nocapture
  PASS: 1 passed, 0 failed
CARGO_PROFILE_TEST_DEBUG=0 cargo test -p arcweft-verify-lsp exposes_source_actions_and_inlay_hints -- --nocapture
  PASS: 1 passed, 0 failed
CARGO_PROFILE_TEST_DEBUG=0 cargo test -p arcweft-lsp inlay_hint_request_uses_document_line_index -- --nocapture
  PASS: 1 passed, 0 failed
CARGO_PROFILE_TEST_DEBUG=0 cargo test -p arcweft-cli --test check ids_materialize_leaves_provisional_dialogue_identity_untouched -- --exact --nocapture
  PASS: 1 passed, 0 failed
CARGO_PROFILE_TEST_DEBUG=0 cargo test -p arcweft-lang-hir -p arcweft-tooling -p arcweft-verify-lsp -p arcweft-lsp --all-targets
  PASS
cargo clippy -p arcweft-lang-hir -p arcweft-tooling -p arcweft-verify-lsp -p arcweft-lsp -p arcweft-cli --all-targets --all-features -- -D warnings
  PASS
cargo check --workspace --all-targets --all-features
  PASS
cargo clippy --workspace --all-targets --all-features -- -D warnings
  PASS
cargo fmt --all -- --check
  PASS
git diff --check
  PASS
```

`CARGO_PROFILE_TEST_DEBUG=0` changes only test debug-information generation. It
was used after repeated Windows `LNK1318` PDB LIMIT failures and does not alter
the feature set or runtime behavior under test.

Before the clean rebuild, the canonical `D:\git\arcweft\target` had only about
71 MB free on `D:`; 572 PDB files under `target/debug/deps` occupied about
117 GB. `cargo clean` removed 234,920 generated files (345.4 GiB), after which
the focused matrix rebuilt and passed. No source, documentation, ZIP, or other
workspace content was removed.

`just test-workspace` was run with `CARGO_BUILD_JOBS=4` and
`CARGO_PROFILE_TEST_DEBUG=0`. It ran for 1104.3 seconds and reached the
unchanged inherited Proof migration gate after all preceding suites passed:

```text
spec_should_pass_check_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/check/010_capability_fs_read.arcw
spec_should_pass_run_fixtures_pass_after_refactor
  tests/fixtures/arcw/spec_should_pass/run/002_file_read_task.arcw
```

An exact rerun of
`cargo test -p arcweft-cli --test arcw_fixtures_check_run -- --nocapture`
confirmed 3 passed and only those 2 failed. They are identical to the gate
recorded before this deletion. This cut does not add a detached source reader
or compatibility path to satisfy the pre-switch fixtures.

Tier 2 is not applicable. This cut changes a tooling edit/hint contract and
its CLI/LSP projections, but no runtime, render, Agent, MCP, capture, or
corresponding transport execution path.

## Structural audit

The audit used parent Git revision `f0e2a260ccd5` and working change
`smrusrrx`.

```text
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/aw-ah-009-4-3-speaker-derived-id-tooling-deletion-2026-07-25
files scanned: 3673
Rust files: 1936
Rust physical LOC: 906140
package manifests: 94
violations: 0 error(s), 146 warning(s)
```

Reports are retained under
`docs/implementation/structure-audits/aw-ah-009-4-3-speaker-derived-id-tooling-deletion-2026-07-25/`.

| Path | Bytes | Physical LOC | Classification | Embedded test LOC | Responsibility |
|---|---:|---:|---|---:|---|
| `crates/arcweft-lang-hir/src/id_context.rs` | 8,040 | 233 | production with unit tests | 21 | declaration/choice materialization only |
| `crates/arcweft-tooling/src/id_context.rs` | 1,462 | 46 | production | 0 | typed ID operations to edits/hints |
| `crates/arcweft-tooling/src/tests.rs` | 41,539 | 1,120 | unit-test module | n/a | tooling behavior |
| `crates/arcweft-verify-lsp/src/lib.rs` | 73,943 | 1,940 | production with unit tests | 849 | Sans-I/O LSP projection and adapter tests |
| `crates/arcweft-lsp/src/session/tests.rs` | 80,461 | 2,335 | unit-test module | n/a | session behavior |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 228,108 | 7,018 | integration-test module | n/a | CLI tooling and runtime smoke matrix |

The changed production `arcweft-verify-lsp/src/lib.rs` remains above the
1,200-LOC warning threshold but below the 2,500-LOC error threshold; this cut
changes only its test expectation and adds no responsibility. The CLI test
module remains above the 2,500-LOC integration warning and below the 8,000-LOC
error threshold. No file crossed a threshold in this cut.

Direct workspace dependency fan-in/fan-out at this revision is:

| Crate | Fan-in | Fan-out |
|---|---:|---:|
| `arcweft-lang-hir` | 11 | 2 |
| `arcweft-tooling` | 4 | 5 |
| `arcweft-verify-lsp` | 1 | 7 |
| `arcweft-lsp` | 1 | 24 |
| `arcweft-cli` | 0 | 53 |

No manifest or dependency edge changed. The deletion follows the existing
`syntax -> HIR -> tooling -> LSP/CLI` direction and removes, rather than
duplicates, a cross-crate payload shape.

## Remaining authority boundary

The old runtime Dialogue identity/lowering path remains explicitly
`SUPERSEDED_FROZEN`, not accepted final design. It must be deleted with the
AW-AH-009.4.2 typed application owner, AW-AH-009.4.3 candidate/project
transaction, sema result, runtime-plan consumer, and accepted project inventory
in one public authority switch. Until then, tooling deliberately publishes no
guessed Dialogue ID.
