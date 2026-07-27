# Proof convergence: EntityKind owner API and Agent effects helper deletion

Date: 2026-07-28

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

Jujutsu change audited: `nyvswokokzylnstyxmkpxuzsplktoxlt`

## Boundary

This deletion-driven cut removes the complete compiler-owned
`agent_effects` module and its free-standing `entity_kind_label` helper.

The helper was the only item in the module, was `pub(crate)`, had no external
consumer, and manually matched every `EntityKind` variant even though
`EntityKind` is owned by `arcweft-lang-sema`. The unchanged behavior now lives
on the owner as `EntityKind::as_str()`.

The compiler's Agent project graph and required-entity projections call the
owner API directly. Every fixed family keeps the exact former lowercase or
snake-case label, and `EntityKind::Other` still borrows its authored value.
The existing consumer-owned `String` allocations remain where the Agent
protocol records require owned strings.

The following obsolete surface is absent:

- `arcweft_compiler::agent_effects`;
- `agent_effects.rs`; and
- `entity_kind_label`.

No forwarding module, wrapper, extension trait, alias, duplicate match,
compatibility shim, source gate, or renamed helper replaces it.

## Direct evidence

The semantic owner test exhaustively compares `EntityKind::AUTHORED_FAMILIES`
with the retained ordered label inventory and separately checks `Other`.

The compiler API trybuild suite imports only
`arcweft_compiler::agent_effects`. That import compiled before deletion and now
fails because the module no longer exists. This is direct type/API evidence,
not a source-text gate.

The full compiler integration suite exercises the Agent project graph and
required-entity projections through the owner method.

## Contract boundary

This cut changes ownership, not the accepted semantic or Agent protocol
values. It does not change project graph records, required-entity records,
serialization, runtime behavior, or stable entity identities.

The corrected Proof 01.1.1.4.1 package remains only partially
implementation-ready pending
[`01.1.1.4.1.1`](../reviews/requests/2026-07-27-seq-proof-01.1.1.4.1.1-source-owner-and-semantic-consistency-correction.md).
No blocked PatternId/TypeId source owner, pathless variant, Duration,
overflow-owner, region, or leaf-expression decision is inferred here.

## Validation

Completed:

- `cargo fmt --all` and final `cargo fmt --all -- --check`: passed;
- exact semantic owner label test: passed;
- compiler API compile-fail suite: passed, including the removed module row;
- `cargo check -p arcweft-lang-sema -p arcweft-compiler --all-targets --all-features`:
  passed;
- `cargo test -p arcweft-lang-sema --all-targets --all-features`: passed,
  including 1,119 unit tests and all integration/compile-fail suites;
- `cargo test -p arcweft-compiler --all-targets --all-features`: passed,
  including all 92 unit tests and every integration/compile-fail suite;
- strict changed-crate Clippy for `arcweft-lang-sema` and
  `arcweft-compiler`: passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `just test-tier2`: passed in 212.5 seconds, including Agent/MCP stdio,
  native capture, animated-image, object-ID/mask, typewriter/ruby, visual
  smoke, and IMQ golden rows; and
- `git diff --check`: passed.

`just test-workspace` ran for 860.1 seconds. It passed the changed sema and
compiler suites, the new compile-fail row, and every preceding workspace
suite. It stopped only at the established
`arcweft-cli --test arcw_fixtures_check_run` baseline. The exact suite was
rerun and reported three passes plus the same two failures present at the
parent revision:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both rows await final attached-HIR publication of the capability-owned
`FsError`. This cut does not touch that owner and adds no fallback nominal,
fixture bypass, compatibility reader, or source gate.

The final ZIP ledger contains 30 retained `docs/reviews/**/*.zip` archives,
zero unrecorded hashes, and zero root-inbox ZIPs. No returned
Proof 01.1.1.4.1.1 correction archive exists.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-entity-kind-owner-and-agent-effects-deletion-2026-07-28/`](structure-audits/proof-entity-kind-owner-and-agent-effects-deletion-2026-07-28/).
It scanned 3,795 files, including 1,961 Rust files and 905,949 physical Rust
LOC across 95 manifests. It reported zero errors and 146 existing warnings.

Current changed-file metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-lang-sema/src/types.rs` | 37,624 | 1,132 | production semantic type owner with unit tests |
| `arcweft-compiler/src/agent_project.rs` | 13,020 | 331 | production Agent projection |
| `arcweft-compiler/src/lib.rs` | 457 | 26 | compiler facade |
| `arcweft-compiler/tests/api_compile.rs` | 519 | 10 | API compile-fail driver |
| `arcweft-compiler/tests/ui/agent_effects_module_removed.rs` | 51 | 3 | API compile-fail fixture |
| `arcweft-compiler/tests/ui/agent_effects_module_removed.stderr` | 232 | 5 | deterministic compiler diagnostic |

The deleted `agent_effects.rs` was 44 physical LOC. All current production
owners remain below structural warning thresholds. No dependency edge,
manifest, feature, opcode, or serialized format changed.

## Next boundary

The sema `borrow`, `fact_layer`, and `lifetime` namespaces are separately
audited zero-external-consumer public-module candidates. They remain active
crate-internal owners and should be made private together, without deleting
their files or reducing their `pub(crate)` items. Agent REPL `command` and its
production consumers remain public. Proof semantic leaf readers and accepted
Dialogue exteriors remain frozen until their correction/replacement authority
is ready.
