# Proof convergence: semantic zero-consumer type-helper deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

Jujutsu change audited: `wmnqnuxnkolq`

## Boundary

This deletion-driven cut removes three unreleased `TypeKind` readers with no
workspace consumer:

- `speaker_preset_entity_kind`;
- `is_speaker_preset_for`; and
- `is_unsigned_integer`.

No renamed helper, local match wrapper, extension trait, compatibility alias,
or string classifier replaces them. A new compile-fail row proves through Rust
type checking that all three deleted methods remain unavailable.

## Retained semantic owners

This cut deliberately keeps the active semantic classification boundaries:

- `speaker_line_classification` and its typed `SpeakerLineType` result;
- `is_integer` and the actively consumed `is_signed_integer` predicate;
- the `SpeakerPreset` and unsigned integer `TypeKind` variants themselves; and
- every registered-call, signature-help, and nominal-resolution consumer.

The corrected Proof 01.1.1.4.1 archive remains only partially
implementation-ready pending the 01.1.1.4.1.1 source-owner and semantic
consistency correction. Active raw leaf readers remain frozen; this cut does
not infer or repair their final schema.

## Validation

Completed:

- `cargo fmt --all` and final `cargo fmt --all -- --check`: passed;
- `cargo check -p arcweft-lang-sema --all-targets --all-features`: passed;
- the new `removed_zero_consumer_type_helpers_are_unavailable` trybuild row:
  passed;
- `cargo test -p arcweft-lang-sema --all-targets --all-features`: passed,
  including 1,118 unit tests, all seven public-API compile-fail groups, and all
  semantic integration matrices;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed; and
- `git diff --check`: passed.

`just test-workspace` ran for 1,282.4 seconds with validation-only
`CARGO_BUILD_JOBS=1`. It passed the changed semantic crate, the new compile-fail
row, and every preceding workspace stage, then stopped at the established
`arcw_fixtures_check_run` baseline. The exact suite reported three passes and
the same two failures present at the parent revision:

- `spec_should_pass_check_fixtures_pass_after_refactor` for
  `010_capability_fs_read.arcw`; and
- `spec_should_pass_run_fixtures_pass_after_refactor` for
  `002_file_read_task.arcw`.

Both require final attached-HIR publication of the capability-owned `FsError`.
This helper deletion does not touch that owner and adds no fallback nominal,
source gate, compatibility reader, or fixture bypass.

The final design-package ledger compared all 30 retained
`docs/reviews/**/*.zip` archives against implementation records: zero
unrecorded or changed archives and zero root-inbox ZIPs.

Tier 2 is not applicable. The cut removes unused semantic readers only and
changes no runtime, render, Agent, MCP, capture, persistence, serialization, or
executed public contract.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-sema-zero-consumer-type-helper-deletion-2026-07-27/`](structure-audits/proof-sema-zero-consumer-type-helper-deletion-2026-07-27/).
It scanned 3,782 files, including 1,958 Rust files and 906,015 physical Rust
LOC across 95 manifests. It reported zero errors plus 146 existing warnings;
the warning-heading inventory is identical to the preceding audit. The audit
records all changed files, workspace Rust hotspots, and package fan-in/fan-out.

Changed metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-lang-sema/src/types.rs` | 34,687 | 1,037 | production |
| `arcweft-lang-sema/tests/api_compile.rs` | 1,648 | 45 | compile-fail driver |
| `arcweft-lang-sema/tests/ui/removed_zero_consumer_type_kind_helpers.rs` | 322 | 12 | compile-fail test |

The production owner remains below the 1,200-line structural warning
threshold. This cut adds no dependency, feature, re-export, crate edge, or
responsibility.

## Next deletion boundary

`NominalResolutionIndex::recovered_node_type` is another zero-consumer wrapper,
but it is a distinct nominal-resolution API cut. Runtime function-effect
evidence payload removal remains a separate compiler/runtime-plan authority
change. Neither should be mixed with active raw leaf replacement while the
correction contract is open.
