# Proof convergence: HIR internal-surface deletion

Date: 2026-07-27

Status: `IMPLEMENTED_VALIDATED_WITH_KNOWN_WORKSPACE_BASELINE`

## Boundary

This deletion-driven cut removes unused, unreleased HIR public surface without
adding replacement wrappers:

- `cache_facts` is now a private implementation module. The compiler-owned
  `HirTopLevelDecl::cache_fact_tag` and `HirFlowItem::cache_fact_tag` inherent
  APIs remain the typed cache-fact boundary; callers never consumed the module
  path itself.
- `lower_choice`, `lower_context`, `lower_dialogue`, `lower_flow`, and
  `lower_ids` are now private modules. Every item they contain was already
  crate-private and workspace-wide discovery found no external module-path
  consumer.
- unused `HirModule::source_len` and `HirModule::top_level_ranges` accessors are
  deleted. The fields remain owned by `HirModule` because
  `safe_top_level_insertion_range` and linked-module invalidation still consume
  them internally.
- unused `HirProjectModule::into_parts` is deleted. `HirProject::new` now
  destructures its owned module directly in the owner module.

Two compile-fail rows prove that downstream crates cannot reopen the internal
lowering modules or call the deleted accessors. No alias, forwarding module,
extension trait, compatibility shim, source scan, or dual reader was added.

The implementation is Jujutsu change `ollzztzkospv` over parent Git commit
`41df2669c4b5`.

## Deliberately retained boundaries

The public `lower` module remains the current HIR lowering entry owner. Its
removal requires the accepted `ParsedSource` lease to become the compiler/HIR
input in the same public authority switch; hiding it first would remove the
only production lowering path.

The `arcweft_lang_hir::syntax` forwarding facade also remains for the next
independent deletion. Its consumers already depend directly on
`arcweft-lang-syntax`, so they can be migrated to the owning crate without a
compatibility re-export. This cut keeps that broader 52-file mechanical
migration separate from the zero-consumer surface.

`HirProject::linked_module` and `HirModule::append_module_body` remain active
production readers. They are frozen until the corrected Proof
`01.1.1.4.1` contract supplies the final semantic leaf payload and the
project-aware compiler/checker authority can replace linked/cloned HIR
atomically.

## Validation

Completed:

- `cargo fmt --all`;
- `cargo test -p arcweft-lang-hir --test public_api --all-features --
  --nocapture`: all eight compile-fail rows passed, including the two new
  deletion rows;
- `cargo test -p arcweft-lang-hir --all-targets --all-features`: passed with
  85 unit tests and all integration/compile-fail suites;
- `cargo test -p arcweft-compiler persistent --all-targets --all-features`:
  passed, including nine compiler persistent-cache unit tests and three
  persistent-query integration tests;
- `cargo check --workspace --all-targets --all-features`: passed; and
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed.

`just test-workspace` ran for 946.6 seconds. It passed the changed HIR crate,
the new compile-fail rows, and all preceding downstream suites, then stopped at
the established `arcweft-cli --test arcw_fixtures_check_run` baseline. The
exact suite was rerun and reported three passed and the same two failed rows:

- `spec_should_pass/check/010_capability_fs_read.arcw`; and
- `spec_should_pass/run/002_file_read_task.arcw`.

Both still require publication of the capability-owned `FsError` nominal
through the final attached HIR authority. This change does not touch their
syntax, HIR payload, sema publication, or CLI execution path, and does not add
a global nominal, fallback, compatibility reader, or fixture bypass.

Tier 2 is not applicable: this cut changes HIR API visibility and removes
unused accessors, but does not alter runtime, renderer, Agent, MCP, capture,
persistence, or a serialized contract.

## Structural audit

The canonical audit is retained under
[`structure-audits/proof-hir-internal-surface-deletion-2026-07-27/`](structure-audits/proof-hir-internal-surface-deletion-2026-07-27/).
It scanned 3,750 files, including 1,950 Rust files and 906,411 physical Rust
LOC across 95 manifests. It reported zero errors and 146 existing warnings;
the warning-heading inventory is identical to the parent audit.

Changed production owners are:

| Owner | Bytes | Physical LOC | Responsibility |
| --- | ---: | ---: | --- |
| `arcweft-lang-hir/src/lib.rs` | 937 | 34 | intentional public module boundary |
| `arcweft-lang-hir/src/model.rs` | 30,044 | 1,120 | HIR model and source-bound insertion invariant |
| `arcweft-lang-hir/src/project.rs` | 33,651 | 919 | module-preserving project owner and linked-view transition |

`project.rs` contains a 413-LOC embedded unit-test module but remains below the
1,200-LOC production warning threshold. `arcweft-lang-hir` has five dependency
edges out and eleven workspace dependents in. This cut changes neither set and
adds no dependency, feature, crate boundary, or responsibility.

## Next boundary

After this cut is pushed, migrate every consumer of the
`arcweft_lang_hir::syntax` forwarding facade directly to
`arcweft_lang_syntax`, delete the facade in the same compiling cut, and prove
the removed path with compile-fail evidence. Do not introduce a renamed
re-export or transitional alias.
