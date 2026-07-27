# Proof convergence: semantic nominal-index wrapper deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

Jujutsu change audited: `lsmkznoxmuko`

## Boundary

This deletion-driven cut removes the unreleased
`NominalResolutionIndex::recovered_node_type` convenience wrapper. The method
had no workspace consumer and only delegated to
`NominalResolutionIndex::node(...).and_then(ResolvedTypeNode::recovered)`.

The accepted node inventory, typed `ResolvedTypeNode` fact, and its
`recovered` reader remain the discoverable owners. No renamed wrapper,
extension trait, compatibility export, source-path reconstruction, or local
string reader replaces the deleted method. A compile-fail row proves through
Rust type checking that the wrapper is unavailable.

## Retained semantic owners

This cut keeps:

- `NominalResolutionIndex::node`, `nodes`, `report`, and `recovered_type`;
- `ResolvedTypeNode::recovered`, including its non-type-node `None` result;
- exact revision-bound `NominalTypeNodeKey` identity; and
- all nominal aggregation, diagnostic, source, and work-accounting behavior.

The corrected Proof 01.1.1.4.1 archive remains only partially
implementation-ready pending 01.1.1.4.1.1 source-owner and semantic
consistency correction. Active raw leaf readers remain frozen; this wrapper
deletion does not guess the blocked schema.

## Validation

Completed:

- `cargo fmt --all` and final `cargo fmt --all -- --check`: passed;
- `cargo check -p arcweft-lang-sema --all-targets --all-features`: passed;
- the new `removed_zero_consumer_nominal_index_helper_is_unavailable`
  trybuild row: passed;
- `cargo test -p arcweft-lang-sema --all-targets --all-features`: passed,
  including 1,118 unit tests, all eight public-API compile-fail groups, and all
  semantic integration matrices;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed; and
- `git diff --check`: passed.

The first `just test-workspace` invocation ran for 1,097.9 seconds with
validation-only `CARGO_BUILD_JOBS=1` and stopped at
`arcweft-runtime-host --test bundle_runner` rather than the established final
fixture baseline. The exact suite was rebuilt and passed all six tests, showing
that the broad-run stop was a non-reproducing build/resource event rather than
a runtime-host regression. No code or fixture was changed in response.

A cached `just test-workspace` rerun completed every preceding workspace stage,
including `bundle_runner` and the new nominal-index compile-fail row, then
stopped only at the established `arcw_fixtures_check_run` baseline after 386.4
seconds. The exact fixture suite reported three passes and the same two
failures present at the parent revision:

- `spec_should_pass_check_fixtures_pass_after_refactor` for
  `010_capability_fs_read.arcw`; and
- `spec_should_pass_run_fixtures_pass_after_refactor` for
  `002_file_read_task.arcw`.

Both require final attached-HIR publication of the capability-owned `FsError`.
This wrapper deletion does not touch that owner and adds no fallback nominal,
source gate, compatibility reader, or fixture bypass.

The final design-package ledger compared all 30 retained
`docs/reviews/**/*.zip` archives against implementation records: zero
unrecorded or changed archives and zero root-inbox ZIPs.

Tier 2 is not applicable. The cut removes one unused semantic wrapper and
changes no runtime, render, Agent, MCP, capture, persistence, serialization, or
executed public contract.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-sema-zero-consumer-nominal-index-helper-deletion-2026-07-27/`](structure-audits/proof-sema-zero-consumer-nominal-index-helper-deletion-2026-07-27/).
It scanned 3,785 files, including 1,959 Rust files and 906,022 physical Rust
LOC across 95 manifests. It reported zero errors plus 146 existing warnings;
the warning-heading inventory is identical to the preceding audit. The audit
records the complete workspace hotspot and dependency inventories.

Changed metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-lang-sema/src/nominal/index.rs` | 12,723 | 362 | production with embedded tests |
| `arcweft-lang-sema/tests/api_compile.rs` | 1,850 | 51 | compile-fail driver |
| `arcweft-lang-sema/tests/ui/removed_zero_consumer_nominal_index_helper.rs` | 315 | 13 | compile-fail test |

The production owner remains well below the 1,200-line warning threshold.
This cut adds no dependency, feature, re-export, crate edge, or responsibility.

## Next boundary

The remaining audited deletion candidates cross compiler/runtime evidence or
project-loader topology boundaries and require their own consumer inventory.
They must not be folded into the blocked final leaf-source authority switch.
