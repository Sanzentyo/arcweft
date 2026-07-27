# Proof convergence: compiler zero-consumer facade deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

## Boundary

This deletion-driven cut removes six unreleased compiler APIs that had no
workspace consumer and no persisted or external compatibility requirement:

- `hir::resolve_hir_references`;
- `link::missing_entry_names`;
- `lower::lower_source_runtime_plan_with_options`;
- `lower::lower_source_runtime_plan_with_typecheck_and_options`;
- `lower::lower_source_pure_helper_candidate`; and
- `ReachabilityReport::all_domains`.

The surviving owners are the registered/environment-aware HIR validation
paths, the stats-bearing runtime-plan reports, the plural pure-helper
inventory lowerer, and per-node reachability lookup. The unused link helper
had never participated in `link_project` and has no replacement. No renamed
wrapper, compatibility alias, dual authority, source reparse, or source gate
was introduced.

A downstream compile-fail row proves that all six removed paths are absent.
This is type-checking evidence for the public API boundary, not a scan of
repository source text.

The one unit-test name that still spelled the removed non-stats runtime-plan
facade was renamed to the behavior it tests. Its body already exercised the
surviving stats-bearing final owner; no test behavior changed.

## Validation

Completed:

- `cargo fmt --all`;
- `TRYBUILD=overwrite cargo test -p arcweft-compiler --test api_compile
  --all-features -- --nocapture`: all five compile-fail rows passed;
- `cargo test -p arcweft-compiler --all-targets --all-features`: passed,
  including 92 unit tests and every compiler integration/compile-fail suite;
- `cargo test -p arcweft-compiler --lib --all-features
  runtime_plan_lowering_preserves_admitted_dialogue_profile -- --nocapture`:
  the final-owner naming row passed after the cleanup;
- `cargo check --workspace --all-targets --all-features`: passed; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed.

`just test-workspace` ran for 662.5 seconds. It passed the changed compiler
crate, the new compile-fail row, and every preceding downstream suite, then
stopped at the established `arcweft-cli --test arcw_fixtures_check_run`
baseline. The exact suite was rerun and reported three passed and the same two
failed rows:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both rows still require publication of the capability-owned `FsError` nominal
through the final attached HIR authority. This public-facade deletion does not
change parsing, HIR construction, nominal publication, or execution. No
fallback nominal, compatibility reader, fixture bypass, or source gate was
added.

The push-cut checks also passed:

- `cargo fmt --all -- --check`;
- `git diff --check`; and
- the review ZIP ledger contains 30 retained archives, zero unrecorded hashes,
  and zero ZIP files directly in the `docs/reviews/` inbox.

Tier 2 is not applicable. This cut removes isolated compiler public APIs but
does not change runtime, rendering, Agent, MCP, capture, persistence, or
serialized behavior.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-compiler-zero-consumer-facade-deletion-2026-07-27/`](structure-audits/proof-compiler-zero-consumer-facade-deletion-2026-07-27/).
It scanned 3,758 files, including 1,952 Rust files and 906,353 physical Rust
LOC across 95 manifests. It reported zero errors and 146 existing warnings;
the complete `violations.md` is byte-identical to the parent
`proof-hir-syntax-facade-deletion` audit.

Current changed-file metrics are:

| Owner | Classification | Bytes | Physical LOC |
| --- | --- | ---: | ---: |
| `arcweft-compiler/src/hir.rs` | production | 2,950 | 74 |
| `arcweft-compiler/src/link.rs` | production | 4,711 | 148 |
| `arcweft-compiler/src/lower.rs` | production | 10,837 | 259 |
| `arcweft-compiler/src/reachability.rs` | production | 6,757 | 262 |
| `arcweft-compiler/src/tests.rs` | test | 150,813 | 3,987 |
| `arcweft-compiler/tests/api_compile.rs` | test | 451 | 9 |
| `arcweft-compiler/tests/ui/removed_zero_consumer_compiler_facades.rs` | test | 404 | 15 |

No Cargo manifest, dependency edge, feature, crate boundary, or production
responsibility was added. `link.rs` retains a small embedded unit-test module
well below every structural threshold.

## Next boundary

The corrected Proof `01.1.1.4.1` semantic leaf/expression package remains
`DESIGN_BLOCKED`: the repository-retained return is a 1,305-byte `NOT_READY`
package-build stub. While waiting, the next independently provable deletion is
the zero-consumer project-loader convenience surface. The bound
`ParsedSource`, Items fragment, linked project, and final leaf-expression
authority switches remain frozen rather than guessed.
