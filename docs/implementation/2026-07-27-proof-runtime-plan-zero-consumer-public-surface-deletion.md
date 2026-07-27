# Proof convergence: runtime-plan zero-consumer public-surface deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

Jujutsu change audited: `pyuokymkwlnp`

## Boundary

This deletion-driven cut removes unreleased runtime-plan surfaces that had no
production consumer:

- `flow::lower_runtime_plan`, whose only behavior was to discard the report
  returned by `lower_runtime_plan_with_stats`;
- `fx::lower_fx_definitions`, whose only behavior was to inject the provisional
  package label `crate`;
- `RuntimePlanLowerOptions::new`, which duplicated `Default::default`;
- the unused `RuntimePlanLowerOptions::trait_methods` reader; and
- the unused `RuntimeTypedExpressionId::index` reader.

The internal `audio`, `expr`, `host_request`, `labels`, `pattern`,
`render_text`, and `source` modules expose no public item to another crate, so
their empty public namespaces are now private. Their active lowering
implementations remain unchanged.

Runtime-plan tests and the CLI bundle fixture now call
`lower_runtime_plan_with_stats` directly and deliberately select its `plan`
field. Fx tests call `lower_fx_definitions_for_package` with the source-local
package identity explicitly. No renamed wrapper, extension trait,
compatibility export, dual reader, or default-package shim replaces the
deleted APIs.

The new runtime-plan compile-fail suite proves through Rust type checking that
both wrappers, all three readers, and all seven former public namespaces are
unavailable. This is public-API evidence, not a source-text gate.

## Retained production owners

This cut keeps the following active owners unchanged:

- `flow::lower_runtime_plan_with_stats` and its typed lowering report;
- `fx::lower_fx_definitions_for_package` and package-qualified Fx identity;
- the `RuntimePlanLowerOptions` builder methods that supply checked evidence;
- `RuntimeTypedExpressionId::from_index` and the evidence carrier itself; and
- every implementation inside the seven newly private responsibility modules.

Raw numeric, Duration, compact-sequence, linked-HIR, and old Dialogue readers
remain frozen. The corrected Proof 01.1.1.4.1 archive is mechanically valid,
but repository intake remains `PARTIALLY_IMPLEMENTATION_READY` pending
Proof 01.1.1.4.1.1 source-owner and semantic-consistency correction. This cut
does not repair those readers or guess the blocked leaf schema.

## Validation

Completed:

- `cargo fmt --all` and final `cargo fmt --all -- --check`: passed;
- `cargo check -p arcweft-runtime-plan --all-targets --all-features`: passed;
- `cargo test -p arcweft-runtime-plan --test api_compile --all-features`:
  passed after recording the new compile-fail contract;
- `cargo test -p arcweft-runtime-plan --all-targets --all-features`: passed,
  including 114 unit tests, the compile-fail suite, assertion identity, 58
  AWBC parity tests, three iterator-witness tests, and 51 integration tests;
- `cargo test -p arcweft-cli --lib --all-features bundle_a`: passed both
  bundle-patch and patched-execution tests;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed; and
- `git diff --check`: passed.

Two initial `just test-workspace` invocations were interrupted by their shell
command ceilings at 124 seconds and 1,204 seconds respectively, not by test
failures. The latter was still actively compiling the CLI suite with
validation-only `CARGO_BUILD_JOBS=1`. A cached rerun with a sufficient command
ceiling passed the changed crate, its new compile-fail fixture, and every
preceding workspace stage, then stopped only at the established
`arcw_fixtures_check_run` baseline. The exact suite reported three passes and
the same two failures present at the parent revision:

- `spec_should_pass_check_fixtures_pass_after_refactor` for
  `010_capability_fs_read.arcw`; and
- `spec_should_pass_run_fixtures_pass_after_refactor` for
  `002_file_read_task.arcw`.

Both require final attached-HIR publication of the capability-owned `FsError`.
This public-surface deletion changes neither that owner nor any fixture and
adds no fallback nominal, source gate, compatibility reader, or bypass.

The final design-package ledger compared all 30 retained
`docs/reviews/**/*.zip` archives against implementation records: zero
unrecorded or changed archives and zero root-inbox ZIPs.

Tier 2 is not applicable. This cut removes zero-consumer APIs and namespace
visibility only; it does not change runtime behavior, rendering, Agent, MCP,
capture, persistence, serialization, or an executed public contract.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-runtime-plan-zero-consumer-public-surface-deletion-2026-07-27/`](structure-audits/proof-runtime-plan-zero-consumer-public-surface-deletion-2026-07-27/).
It scanned 3,779 files, including 1,957 Rust files and 906,020 physical Rust
LOC across 95 manifests. It reported zero errors plus 146 existing warnings;
the warning-heading inventory is identical to the preceding audit. The full
changed-file and workspace-hotspot inventories are in `file_metrics.csv`, and
the complete fan-in/fan-out inventory is in `dependency_edges.csv`.

Representative changed metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-runtime-plan/src/lib.rs` | 346 | 20 | production facade |
| `arcweft-runtime-plan/src/flow.rs` | 76,219 | 2,083 | production |
| `arcweft-runtime-plan/src/fx.rs` | 22,334 | 624 | production with embedded tests |
| `arcweft-runtime-plan/src/typed_evidence.rs` | 9,811 | 294 | production |
| `arcweft-runtime-plan/src/flow/tests.rs` | 20,304 | 601 | unit test |
| `arcweft-runtime-plan/tests/runtime_plan.rs` | 63,982 | 2,078 | integration test |
| `arcweft-runtime-plan/tests/ui/removed_zero_consumer_runtime_plan_facades.rs` | 475 | 18 | compile-fail test |
| `arcweft-cli/src/app/bundle/tests.rs` | 98,223 | 2,946 | unit test |

`flow.rs` and the CLI bundle test already exceeded their applicable warning
thresholds before this cut; this change shrinks the production file and adds
no responsibility to either. The only dependency change is the workspace-owned
`trybuild` development dependency used for the public-API removal proof; no
production crate edge or feature changes.

## Next deletion boundary

The next independent cut is the three zero-consumer semantic `TypeKind`
helpers. Runtime function-effect evidence payload removal is a separate
compiler/runtime-plan authority change and must not be mixed into that small
semantic deletion. Active raw leaf readers stay frozen until the correction
contract closes their final owner.
