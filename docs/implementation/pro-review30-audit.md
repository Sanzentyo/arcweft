# pro_review30 Completion Audit

This audit records the current implementation evidence for
`docs/reviews/pro_review30.md`. It is intentionally implementation-state
documentation, not language design text.

## Result

All pro_review30 items are implemented and covered by tests or path-free bench
output. The only Rust `unsafe` sites are the Cranelift native-call conversions
needed to execute finalized JIT code. They are isolated in one adapter module,
carry local `SAFETY` comments, and are guarded by a regression test.

## Unsafe Boundary

The direct Cranelift JIT path needs to call machine code returned by
`JITModule::get_finalized_function`. Rust has no safe standard API for turning
that opaque code pointer into a typed `extern "C"` function pointer, so the
native adapter owns the boundary.

Evidence:

- Boundary file: `crates/arcweft-lang-jit-cranelift/src/native_call.rs`.
- Regression: `rust_unsafe_sites_stay_inside_jit_native_call_boundary`.
- Preconditions checked at the boundary:
  - scalar arity is limited to the emitted function signatures;
  - row-major batch input length must equal `rows * arity`;
  - row counts must fit the native ABI integer;
  - owning `JITModule` is stored in the compiled helper object and outlives
    every call.
- Semantic reference: JIT conformance tests compare candidate output against
  the VM backend.

This keeps native execution outside `arcweft-core`; core remains Sans I/O and
does not depend on Cranelift or executable-memory machinery.

## Requirement Matrix

| Item | Evidence |
| --- | --- |
| P0-1 CST punctuation scan uses one typed summary instead of repeated lexing | `CstLinePunctuationSummary`, `CstPunctuationScan`, `cst_punctuation_scan_reuses_fragment_tokens`, `cst_top_level_matching_punctuation_uses_one_fragment_scan`, and bench syntax counters. |
| P0-2 line/block extraction is range/slice-oriented in the parse path | `cst_line_projection_records_path_free_parse_stats` asserts `line_owned_bytes = 0` and `block_owned_bytes = 0`; `performance-snapshot.md` records the same for the checked bench. |
| P0-3 large numeric bracket sequences use summary expression data | `Expr::NumericBracketSeq`, `parse_stats_count_numeric_sequence_summaries_from_flow_body`, `large_flat_literal_sequences_parse_as_bracket_seq`, and `numeric_sequence_literals_use_expected_item_fast_path`. |
| P1-4 lossy/recovery/wikilink scans are cold-path counted | Syntax stats expose `wiki_scan_performed`, `dot_normalization_owned`, and `dialogue_rescue_expr_parse_attempts`; `bench-009` reports all three as zero. |
| P1-5 runtime-plan pure rewrite and map/sum optimization avoid plan-wide rewrite | `runtime_plan_lowering_reports_pure_and_map_sum_optimization_work` asserts `pure_rewrite_expr_visits = 0`, `local_use_tail_scans >= 1`, and `sequence_map_sum_fusions = 1`. |
| P1-6 pure-helper discovery avoids repeated strict lowering | `PureHelperCandidateReport` and `PureHelperShape` carry candidate counters; tests assert candidate lower attempts and cloned/lowered node counts. |
| P1-7 TypeJudgment and expected checks reduce fixed work | Typecheck stats expose `type_compatibility_checks` and judgment samples; `numeric_sequence_literals_use_expected_item_fast_path` proves large numeric sequences do not recurse per literal. |
| P1-8 borrow state uses delta counters instead of full snapshot cloning | Typecheck stats expose `state_delta_entries`, `state_full_clones`, and `state_merge_keys`; borrow tests assert `state_full_clones = 0` for covered paths. |
| P2-9 VM/AOT/JIT auto policy separates cold compile and steady execution | `RuntimePureCompileStats` exposes JIT/AOT attempts, cache hits, Auto selections, deferrals, promotions, and compile time; accelerator tests cover cold AOT selection and large flat-batch JIT promotion. |
| P2-10 typed memory views stay in core data while native execution stays in accelerator | `RuntimeValue::Seq(RuntimeSeq::Dense(...))` and `DenseSeq` views live in `arcweft-core`; Cranelift/Rayon execution lives in `arcweft-runtime-accelerator` and `arcweft-lang-jit-cranelift`. Dense integer, float, textual, duration, and entity-ref tests cover typed views. |
| P2-11 flat batch is preferred and copy cost is visible | VM/AOT/JIT flat-batch calls expose borrowed/copy counters; tests cover borrowed flat input, zero flatten materialization, and JIT flat-batch sum without result copy. |
| P2-12 multithreaded batch policy uses rows, helper work, and backend | `RuntimePureAccelerator::should_parallelize_batch` uses `rows * helper_work_units` and backend-specific thresholds; stats expose policy checks, work units, backend skips, small skips, parallel batches, worker jobs, and pool build time. |
| P3-13 scheduler/native bridge counters split phase and task class | Scheduler stats expose submitted/dispatched/completed by class, sort counts, pressure, completion counts, and bridge phase timing; thread and native I/O bench tests assert these fields. |
| P3-14 completion normalization separates join amplification and sort cost | Scheduler stats expose normalization passes/checks, events in/joined/out, skipped/performed sort items, and emitted joined events; scheduler tests cover normalized and already-ordered paths. |
| Removed whitespace command DSL stays removed | `source_tree_does_not_reintroduce_removed_whitespace_command_dsl_or_shims` scans checked source, docs, and fixtures outside historical reviews. |
| Path-free outputs | `checked_in_docs_and_samples_do_not_record_host_absolute_paths`, profile tests, and `bench-009` JSON use relative fixture/source names. |

## Verification Commands

These commands were used as the final verification gate:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
just verify
just bench-009
```

Representative `bench-009` counters after the pro_review30 work:

```text
line_owned_bytes = 0
block_owned_bytes = 0
wiki_scan_performed = 0
numeric_seq_summaries = 1
pure_rewrite_expr_visits = 0
local_use_tail_scans = 2
sequence_map_sum_fusions = 1
state_full_clones = 0
pure_flat_batch_bytes_borrowed_median = 2048
pure_flatten_materializations_median = 0
pure_arg_vec_allocations_median = 0
pure_parallel_work_units_median = 896
```
