# Proof convergence: private HIR name owner relocation

Date: 2026-07-27

Status: `LANDED_VALIDATED_WITH_EXISTING_WORKSPACE_BASELINE`

## Boundary

This deletion-driven private cut starts the accepted Proof 01.1.1.4.1 leaf
ownership migration without publishing a second HIR reader.

`HirName` now lives in the private final `arcweft-lang-hir::expr` responsibility
module. The private Dialogue application/line-plan carrier imports that one
type. Its duplicate local definition was deleted.

The old `HirName::try_new`, `HirName::as_str`, and
`HirNameInvariantError` definitions had no production consumer. They were
deleted rather than copied, wrapped, suppressed as dead code, or retained as a
temporary compatibility API. The attached final expression lowerer will add
the required exact constructor, error, and read-only accessor directly to the
final owner when they have a real production consumer.

Both the new `expr` responsibility module and the provisional
`dialogue_application` module remain private. The existing compile-fail
boundary now names both modules, proving that this substrate is not a public
dual authority.

No alias, re-export, extension trait, source-string parser, compatibility
reader, source gate, or removed-syntax diagnostic was added.

## Explicit exclusions

This cut does not add path, type-region, runtime-registry, literal, expression
arena, source-map, or public query types. In particular:

- pathless variant patterns and PatternId/TypeId source queries remain under
  the Proof 01.1.1.4.1.1 correction;
- elided type regions remain blocked on an exact TypeId `SyntheticOwner`
  contract; and
- active syntax/HIR/project/runtime readers remain frozen rather than repaired.

## Validation

Completed:

- `cargo fmt --all`;
- `cargo test -p arcweft-lang-hir --all-targets --all-features`: passed (87
  unit tests and all HIR integration/compile-fail suites); the first run
  exposed dead private constructor methods and caused their deletion rather
  than a lint allowance;
- `cargo check -p arcweft-lang-hir --all-targets --all-features`: passed;
- `cargo clippy -p arcweft-lang-hir --all-targets --all-features -- -D
  warnings`: passed; and
- `cargo test -p arcweft-lang-hir --test public_api --all-features -- --nocapture`:
  passed after regenerating the direct module-privacy evidence;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `cargo fmt --all -- --check`: passed; and
- `git diff --check`: passed.

The first cold `just test-workspace` invocation exceeded the 904-second outer
runner limit while the all-target compilation was still active. Its child
Cargo process completed normally, and the immediate same-command rerun
completed the workspace suite. Every preceding workspace, CLI, and compile-fail
stage passed before the established `arcw_fixtures_check_run` baseline stopped
the recipe. The exact suite reported three passes and the same two failures
present at the parent revision:

- `spec_should_pass_check_fixtures_pass_after_refactor` for
  `010_capability_fs_read.arcw`; and
- `spec_should_pass_run_fixtures_pass_after_refactor` for
  `002_file_read_task.arcw`.

Both fixtures require final attached-HIR publication of capability-owned
`FsError`. This private owner relocation neither changes that owner nor adds a
fallback nominal, compatibility reader, fixture bypass, or source gate.

The final design-package ledger compared all 30 retained
`docs/reviews/**/*.zip` archives against package-specific implementation
records: zero unrecorded or changed archives and zero root-inbox ZIPs.

Tier 2 is not applicable. This private owner relocation changes no runtime,
render, Agent, MCP, capture, persistence, or serialized behavior.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-private-hir-name-owner-relocation-2026-07-27/`](structure-audits/proof-private-hir-name-owner-relocation-2026-07-27/).
The final pass scanned 3,768 files, including 1,955 Rust files and 906,276
physical Rust LOC, and reported zero errors plus 146 existing warnings.

Representative changed metrics are:

| Owner | Bytes | Physical LOC | Classification |
| --- | ---: | ---: | --- |
| `arcweft-lang-hir/src/dialogue_application.rs` | 16,835 | 560 | production |
| `arcweft-lang-hir/src/expr.rs` | 454 | 9 | production |
| `arcweft-lang-hir/src/lib.rs` | 640 | 28 | production |
| `arcweft-lang-hir/tests/ui/internal_lowering_modules_private.rs` | 158 | 6 | test |

No new structural error or warning category was introduced. The new
responsibility module is intentionally private and contains only the one owner
with an existing consumer; its final constructor and validation API are not
kept alive artificially.
