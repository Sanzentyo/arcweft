# Proof convergence: compiler test-only HIR facade deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

## Boundary

This deletion-driven cut removes five unreleased compiler surfaces with no
production consumer:

- `validate_hir_with_env`;
- its `resolve_hir_references_with_env` and `typecheck_hir_with_env` wrappers;
- the wrapper-only `ValidateHirError`; and
- `lower_source_runtime_plan_with_stats_and_options`.

The compiler integration test that existed only to exercise
`validate_hir_with_env` was deleted. The same missing-associated-type
invariant remains owned directly by the semantic layer's structured trait
diagnostic test. Compiler unit tests now:

- test function-value lowering only through the typecheck-evidence owner;
- invoke `arcweft-runtime-plan`'s actual lowerer directly when testing its
  missing-evidence rejection; and
- pass the accepted `TypeCheckReport` into the compiler's surviving typed
  lowering owner when checking Dialogue profile preservation.

The existing compiler compile-fail fixture proves that all five deleted
surfaces remain unavailable. This is Rust type-check evidence, not a
source-text gate. No renamed wrapper, aggregate error, compatibility alias,
extension trait, dual reader, source reparse, or fixture bypass replaces them.

## Retained production owners

This cut does not remove or repair active readers. The following current
authorities remain unchanged:

- `lower_source_document`, which is still used by project compilation and the
  accepted RAG source-index path;
- registered-world reference resolution, typecheck-readiness validation, and
  registered-project type checking;
- `lower_source_runtime_plan_with_typecheck_stats_and_options`; and
- `arcweft_runtime_plan::flow::lower_runtime_plan_with_stats` as the lower
  layer's real implementation owner.

Raw numeric, Duration, compact-sequence, linked-HIR, and old Dialogue readers
remain frozen. Their replacement still requires the corrected Proof
01.1.1.4.1.1 source-owner and semantic-consistency contract; this cut neither
repairs them nor guesses that blocked schema.

## Validation

Completed:

- `cargo fmt --all` and final `cargo fmt --all -- --check`: passed;
- `cargo check -p arcweft-compiler --all-targets --all-features`: passed;
- `cargo test -p arcweft-compiler --test api_compile --all-features --
  --nocapture`: passed with the expanded removal fixture;
- `cargo test -p arcweft-compiler --all-targets --all-features`: passed with
  92 unit tests and every remaining integration/compile-fail suite;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed; and
- `git diff --check`: passed.

The first `just test-workspace` attempt reached compiler integration-test
linking and stopped because Windows could not memory-map an existing
`arcweft_bundle` rlib with pagefile error 1455. The same `view_product` suite
had already passed in the focused compiler run, so this was an environment
resource failure rather than invalid Rust metadata.

The recipe was rerun with validation-only `CARGO_BUILD_JOBS=1`. It ran for
775.2 seconds, passed the changed compiler crate, its expanded compile-fail
fixture, and every preceding workspace stage, then stopped at the established
`arcw_fixtures_check_run` baseline. The exact suite reported three passes and
the same two failures present at the parent revision:

- `spec_should_pass_check_fixtures_pass_after_refactor` for
  `010_capability_fs_read.arcw`; and
- `spec_should_pass_run_fixtures_pass_after_refactor` for
  `002_file_read_task.arcw`.

Both require final attached-HIR publication of the capability-owned `FsError`.
This compiler facade deletion does not change that owner or add a fallback
nominal, compatibility reader, source gate, or fixture bypass.

The final design-package ledger compared all 30 retained
`docs/reviews/**/*.zip` archives against implementation records: zero
unrecorded or changed archives and zero root-inbox ZIPs.

Tier 2 is not applicable. The cut removes test-only compiler APIs and changes
no runtime, rendering, Agent, MCP, capture, persistence, or serialization
behavior.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-compiler-test-only-hir-facade-deletion-2026-07-27/`](structure-audits/proof-compiler-test-only-hir-facade-deletion-2026-07-27/).
It scanned 3,775 files, including 1,955 Rust files and 906,022 physical Rust
LOC across 95 manifests. It reported zero errors plus 146 existing warnings;
the warning-heading inventory is identical to the preceding audit.

Representative changed metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-compiler/src/error.rs` | 2,175 | 50 | production |
| `arcweft-compiler/src/hir.rs` | 1,739 | 40 | production |
| `arcweft-compiler/src/lower.rs` | 10,478 | 251 | production |
| `arcweft-compiler/src/tests.rs` | 150,358 | 3,973 | unit test |
| removed `arcweft-compiler/tests/traits.rs` | 1,660 | 40 | integration test |
| `arcweft-compiler/tests/ui/removed_zero_consumer_compiler_facades.rs` | 649 | 22 | compile-fail test |

No changed production file crosses a structural warning threshold. The cut
adds no dependency, feature, re-export, crate edge, or new responsibility.

## Next deletion boundary

The next independent candidates are zero-consumer runtime-plan wrappers and
empty public namespaces, followed separately by unused runtime evidence
payload. Do not combine those public runtime-plan changes with the small sema
helper deletion or with the blocked final leaf/source-owner switch.

