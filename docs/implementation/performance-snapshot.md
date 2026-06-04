# Performance Snapshot

This file records path-free local measurements for optimization comparisons.
Values are machine- and build-cache-dependent; use them as trend samples, not
portable guarantees.

## 2026-05-31 JST

Host summary reported by Arcweft:

```text
physical_cores = 12
logical_threads = 20
available_parallelism = 20
```

Commands:

```bash
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/001_thread_scheduling.arcw --json --iterations 10 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw --json --iterations 25 --warmup 5 --samples 11 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/004_system_info_threads.arcw --json --iterations 1 --warmup 0 --samples 3 --steps 24 --max-ops 24 --mode drain
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/007_branching_iter_pure_jit.arcw --json --iterations 25 --warmup 5 --samples 11 --steps 128 --max-ops 128 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit --pure-workers 4 --pure-batch-min-len 64
```

Checked-in thread scheduling bench:

| fixture | executor | iterations | steps | median elapsed ns | median executed ops | per executed op ns | median line effects | median task requests |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 001_thread_scheduling.arcw | bytecode_vm | 10 | 64 | 16900 | 19 | 889 | 3 | 3 |

Checked-in system-info threaded native scheduling bench:

| fixture | executor | iterations | median elapsed ns | task requests | task events in | system info ops | scheduler submitted | scheduler max in-flight | parallel system tasks | parallel marker tasks | parallel workers |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 004_system_info_threads.arcw | bytecode_vm | 1 | 919200 | 6 | 6 | 3 | 6 | 6 | 3 | 3 | 6 |

The scheduler completion counters now split native completion work into
normalization checks, sort work, joined-event fanout, and final event volume.
In the latest checked-in system-info threaded native scheduling run, the
completion surface reported `completion_events_in = 6`,
`completion_events_out = 6`, `completion_normalization_passes = 1`,
`completion_normalization_checks = 1`, `completion_sort_performed_items = 6`,
`completion_sort_skipped_items = 0`, and
`joined_completion_events_emitted = 0`. The same run reported
`submitted_by_class.cpu = 6`, `dispatched_by_class.cpu = 6`, and
`completed_by_class.cpu = 6`, giving the scheduler/native bridge a class
breakdown without recording host paths.

The CLI/native bridge now reports host-side phase timing counters separately
from Sans I/O scheduler counters. The same system-info threaded run reported
`scheduler_submit_elapsed_ns = 37500`, `scheduler_dispatch_elapsed_ns = 5900`,
`host_complete_elapsed_ns = 600100`, `event_build_elapsed_ns = 3700`, and
`scheduler_complete_elapsed_ns = 49500`. These values are local trend samples
for bridge overhead attribution, not deterministic replay state.

Scalar for-loop and mixed iterator pure JIT benches:

| fixture | executor | pure backend | median elapsed ns | pure calls | stack packs | arg vec allocs | copied arg bytes | borrowed arg bytes | JIT calls | batch items |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 003_for_pure_jit.arcw | bytecode_vm | jit | 47900 | 16 | 0 | 0 | 0 | 256 | 16 | 0 |
| 007_branching_iter_pure_jit.arcw | bytecode_vm | jit | 106200 | 32 | 0 | 0 | 0 | 512 | 32 | 16 |

Nonuniform map pure batch after review30 observation counters and numeric
sequence AST summary:

| fixture | executor | pure backend | median elapsed ns | parse ns | typecheck ns | runtime plan ns | typecheck exprs | type judgments | pure calls | batch calls | batch items | arg vec allocs | borrowed arg bytes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 009_nonuniform_map_pure_batch.arcw | bytecode_vm | auto | 13300 | 2957200 | 200100 | 261700 | 16 | 21 | 128 | 1 | 128 | 0 | 2048 |

Syntax counters from the same run:

| cst lex passes | punctuation summaries | punctuation bytes | line owned bytes | block owned bytes | wiki scans | raw owned bytes |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 14 | 1330 | 0 | 0 | 0 | 0 |

The scalar pure fixtures remain on borrowed slice calls: `stack_packs = 0`,
`arg_vec_allocs = 0`, and `copied_arg_bytes = 0`. The nonuniform map fixture
now checks the large typed numeric sequence through the expected-item fast path,
so the typechecker records 16 expressions instead of recursively visiting every
literal in the sequence. Expression judgment subjects now use static
expression-kind labels, and expected-type evidence is a rule on the expression
subject instead of a second context-only judgment. The parser also records a compact integer-only
`numeric_bracket_seq` AST node instead of allocating per-item expression nodes
for that literal family. Syntax stats are always present in the JSON schema, but
default parsing updates only counters available as normal parser by-products,
including whether dot-continuation normalization had to allocate and whether
dialogue/index disambiguation had to try parsing bracket content as an
expression. Detailed fields that would require timing, tracing, or extra scans
remain zero until a detailed instrumentation mode is added. CST line punctuation
summaries are built from the existing rowan line-token walk, not by re-lexing
each line for stats. The parser's line events borrow slices from the original
source during normal `parse_source`, so line projection no longer owns a second
copy of every line; `cst_lines(root)` still owns text for standalone CST tooling
that has no source buffer. Balanced brace-block extraction now also reuses those line
summaries for body-open and body-close offsets, so the hot block collector no
longer re-lexes the assembled block text after already walking its lines. For
body fragments that are not CST lines, `CstPunctuationScan` now owns the one
fragment tokenization pass and serves top-level punctuation, matching
punctuation, and bracket-delta queries from that token buffer. Dialogue
same-line `with { ... }` attachments and trailing bare scopes reuse that scan
for the brace-depth check and the split itself; multiline continuations add the
stored per-line punctuation summaries before the final fragment split. Same-line
dialogue attachments now remain borrowed through this path, while multiline
continuations are the only dialogue attachment cases charged to
`block_owned_bytes`. Logical
block item splitting now returns borrowed body slices for single-line items and
allocates only for multiline or method-chain continuations, so block-body
parsers avoid creating a `String` for every item before the AST builder needs
an owned value. The splitter also derives line deltas from one body-fragment
`CstPunctuationScan`, avoiding a separate lex pass for each raw body line.

Auto pure backend tiering now separates cold scalar execution from warm flat
batch execution. With `--pure-backend auto`, the scalar for-loop pure fixture
kept one helper on typed AOT and did not attempt JIT compilation:
`003_for_pure_jit.arcw` reported median elapsed time `47600 ns`,
`pure_calls_median = 16`, `pure_aot_calls_median = 16`,
`pure_jit_calls_median = 0`, `auto_aot_selected = 1`,
`auto_jit_deferred = 1`, and `jit_attempts = 0`. The large nonuniform map
fixture started on AOT but promoted the same helper to JIT once the flat-batch
work crossed the Auto threshold: `009_nonuniform_map_pure_batch.arcw` reported
median elapsed time `14200 ns`, `pure_batch_items_median = 128`,
`pure_jit_calls_median = 128`, `auto_jit_promotions = 1`, `jit_attempts = 1`,
and `aot_attempts = 1`.

Dense scalar sequence benches:

```bash
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/010_dense_i32_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/011_dense_u64_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/012_dense_integer_widths_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/013_dense_scalar_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/014_dense_textual_scalar_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/015_dense_wide_numeric_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/021_columnar_record_projection.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
```

| fixture | status | median elapsed ns | executed ops | per op ns | parse ns | typecheck ns | runtime plan ns | typecheck exprs | type judgments | arg vec allocs | flatten materializations |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 010_dense_i32_sum.arcw | measured | 7900 | 4 | 1975 | 2416500 | 192600 | 301900 | 6 | 9 | 0 | 0 |
| 011_dense_u64_sum.arcw | measured | 8000 | 4 | 2000 | 2726500 | 237500 | 411000 | 6 | 9 | 0 | 0 |
| 012_dense_integer_widths_sum.arcw | measured | 21900 | 10 | 2190 | 4332600 | 405300 | 600000 | 30 | 39 | 0 | 0 |
| 013_dense_scalar_len.arcw | measured | 16000 | 8 | 2000 | 2002000 | 335600 | 572800 | 54 | 61 | 0 | 0 |
| 014_dense_textual_scalar_len.arcw | measured | 17200 | 7 | 2457 | 1317400 | 203500 | 381800 | 21 | 27 | 0 | 0 |
| 015_dense_wide_numeric_len.arcw | measured | 13600 | 7 | 1942 | 1424200 | 164300 | 263000 | 18 | 24 | 0 | 0 |
| 016_dense_i32_map_pure_batch.arcw | measured | 73300 | 3 | 24433 | 3141200 | 226100 | 315900 | 16 | 21 | 0 | 0 |
| 021_columnar_record_projection.arcw | measured | 19500 | 3 | 6500 | 2445400 | 509000 | 673700 | 30 | 30 | 0 | 0 |

The dense eligibility rule is scalar-first: deterministic homogeneous scalar
runtime values use `RuntimeSeq::Dense(DenseSeq::...)`, with generic
`DenseSeqStorage<T>` behind each typed variant and borrowed views for hot
numeric/byte/text paths. `Tuple`, `Record`, and `Variant` remain heterogeneous
runtime values and are not forced into the scalar dense layer. Repeated-shape
tuple and record literal sequences now use `RuntimeSeq::TupleColumns` and
`RuntimeSeq::RecordColumns`, with each column lowered back through the same
scalar dense eligibility rule. Shape changes fall back to `RuntimeSeq::Values`.
Record field projection over a columnar record sequence returns the existing
field column without row materialization. Runtime expressions now include
`ProjectRecord { ordinal }` and `ProjectTuple { ordinal }`; runtime-plan
lowering emits them when the record field or tuple index ordinal is known from
literal shape. Runtime-plan flow optimization also rewrites local projections
such as `rows.score` to `ProjectRecord { ordinal }` when the local is bound to a
known columnar record sequence. General non-local field projection still needs
sema-owned ordinal evidence.
The checked-in columnar projection fixture projects `rows.score` from a stable
record sequence and sums the returned column with
`pure_flatten_materializations_median = 0`, confirming the field column is
reused rather than materializing row records during runtime execution.
The checked-in benches above confirm that `i32`,
`u64`, all supported integer widths, unit/bool/char/duration/bytes, textual
values, native `f32`/`f64` sequences, entity
refs, and wide integer length paths run without argument-vector allocation or
dense flatten materialization. Typed floats are stored as native Rust `f32`
and `f64`; exact bit identity is measured through explicit `to_bits` checks
rather than runtime value equality. The VM reference runtime now dispatches
`std.f32.*` and `std.f64.*` constants/functions through typed runtime
intrinsics, including explicit `to_bits`/`from_bits`, predicates, math helpers,
and explicit `to_f64`/`to_f32` casts.
After scalar integer materialization was changed to preserve width tags,
path-free local remeasurement with the `just bench-010` and `just bench-012`
targets reported median elapsed times of 7900 ns for
`010_dense_i32_sum.arcw` and 21900 ns for
`012_dense_integer_widths_sum.arcw`. Both runs kept
`pure_flatten_materializations_median = 0`,
`pure_arg_vec_allocations_median = 0`, and path-free source names, confirming
that width-preserving scalar fallback did not force dense sequence
materialization in these benches.

Backend-aware pure batch parallel policy checks:

```bash
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw --json --iterations 6 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit --pure-workers 4 --pure-batch-min-len 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw --json --iterations 4 --warmup 1 --samples 5 --steps 64 --max-ops 64 --pure-backend aot --pure-workers 2 --pure-batch-min-len 1
```

| fixture | backend | median elapsed ns | batch items | policy checks | parallel batches | skipped backend | skipped small | weighted work units | thread pool jobs |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 009_nonuniform_map_pure_batch.arcw | jit | 11700 | 128 | 1 | 0 | 1 | 0 | 896 | 0 |
| 009_nonuniform_map_pure_batch.arcw | aot | 72800 | 128 | 1 | 1 | 0 | 0 | 896 | 2 |

The JIT run keeps the compiled flat batch as one native call and records the
backend skip instead of touching the worker pool. The AOT run uses the weighted
work threshold and creates two jobs for the same 128-row workload. The source
field in both JSON reports is the fixture filename only.

## 2026-05-30 JST

Host summary reported by Arcweft:

```text
physical_cores = 12
logical_threads = 20
available_parallelism = 20
```

Commands:

```bash
cargo run -p arcweft-cli --quiet -- jit check --json --case accumulation-mix --iterations 5000 --warmup 500 --samples 5 --input-seed 7 --julia
cargo run -p arcweft-cli --quiet -- jit check --json --case branch-mix --iterations 5000 --warmup 500 --samples 5 --input-seed 11 --julia
cargo run -p arcweft-cli --quiet -- toolchain-profile --command check --repeat 3 --json
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/001_thread_scheduling.arcw --json --iterations 10 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/004_system_info_threads.arcw --json --iterations 1 --warmup 0 --steps 24 --max-ops 24 --mode drain
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/002_map_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 128 --max-ops 128 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/005_inferred_pure_jit.arcw --json --iterations 4 --warmup 1 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/006_linear_aot.arcw --executor aot --json --iterations 4 --warmup 1 --steps 8 --max-ops 8
cargo run -p arcweft-cli --quiet -- profile tests/fixtures/arcw/spec_should_pass/bench/002_map_pure_jit.arcw --flow flow.map_pure --mode drain --steps 64 --max-ops 64 --pure-backend jit --json
```

JIT check summaries:

| case | VM ns/iter | AOT ns/iter | JIT ns/iter | JIT batch ns/iter | Julia ns/iter | JIT speedup vs VM | JIT batch speedup vs VM | JIT batch vs Julia |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| accumulation-mix | 3296 | 360 | 115 | 1 | 3 | 28.474x | 1734.852x | 1.905x |
| branch-mix | 1991 | 306 | 115 | 2 | 2 | 17.255x | 930.672x | 1.196x |

Toolchain profile:

| command | repeat | min ns | median ns | max ns |
| --- | ---: | ---: | ---: | ---: |
| cargo check --workspace | 3 | 320584800 | 359898600 | 453985700 |

Checked-in thread scheduling bench:

| fixture | executor | iterations | steps | median elapsed ns | median executed ops | per executed op ns | median line effects | median task requests |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 001_thread_scheduling.arcw | bytecode_vm | 10 | 64 | 17600 | 19 | 926 | 3 | 3 |

Checked-in system-info threaded native scheduling bench:

| fixture | executor | iterations | median elapsed ns | task requests | task events in | system info ops | scheduler submitted | scheduler max in-flight | parallel system tasks | parallel marker tasks | parallel workers |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 004_system_info_threads.arcw | bytecode_vm | 1 | 946000 | 6 | 6 | 3 | 6 | 6 | 3 | 3 | 6 |

Checked-in map pure JIT bench:

| fixture | executor | pure backend | iterations | median elapsed ns | pure calls | batch calls | batch items | arg vec allocs | borrowed arg bytes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 002_map_pure_jit.arcw | bytecode_vm | jit | 8 | 17800 | 16 | 1 | 16 | 0 | 256 |

Checked-in inferred pure JIT bench:

| fixture | executor | pure backend | inferred helpers | jit helpers | iterations | median elapsed ns | pure calls | batch calls | borrowed arg bytes | result bytes copied |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 005_inferred_pure_jit.arcw | bytecode_vm | jit | 1 | 1 | 4 | 13500 | 4 | 1 | 64 | 32 |

Checked-in linear AOT executor bench:

| fixture | executor | iterations | median elapsed ns | executed ops | AOT fast-path ops | line effects | pure calls |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 006_linear_aot.arcw | aot | 4 | 7600 | 3 | 3 | 1 | 0 |

The map pure JIT bench reports helper compile time separately from measured
elapsed time. In this run the helper compile counter was 4621600 ns, while the
steady-state sample median above stayed at 29000 ns.

Checked-in for-loop pure JIT bench:

| fixture | executor | pure backend | iterations | median elapsed ns | executed ops | pure calls | stack packs | copied arg bytes | borrowed arg bytes | line effects |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 003_for_pure_jit.arcw | bytecode_vm | jit | 8 | 50200 | 68 | 16 | 0 | 0 | 256 | 16 |

This for-loop sample uses scope and pattern binding capacity derived from the
structured runtime pattern enum, and `RuntimeEnv` reuses popped temporary
scopes. The short local sample remains noisy, but the path-free counters show
the same executed-op and pure-call shape without reintroducing argument vector
allocation.
Runtime bench elapsed time now starts after bytecode/AOT executor artifacts are
prepared for the selected flow, keeping compiler and executor-preparation cost
in the phase counters instead of the measured runtime loop.
The native task bridge is lazy in measured sections, so pure and short
runtime-only benches do not construct adapter state unless emitted host tasks
must be completed.
Flow cursors carry a resolved flow vector index, avoiding per-op flow-id map
lookups during normal VM/AOT stepping.
After scalar result-copy accounting was tightened, the same fixture reports
`pure_result_bytes_copied_median = 0`; only batch output buffers count result
bytes.

Profile phase split for the checked-in map pure JIT fixture:

| fixture | executor prepare ns | run ns | pure compile ns | run executed ops | run pure batch calls |
| --- | ---: | ---: | ---: | ---: | ---: |
| 002_map_pure_jit.arcw | 3216000 | 329800 | 3158600 | 5 | 1 |

The JSON outputs above reported no source file paths and included only command
argv tokens, host core/thread counts, timing counters, and deterministic
accumulators.

Parser cold-path work now skips wiki-link collection entirely when the source
does not contain a `[[` opener, keeps multiline expression sources borrowed
unless a dot-continuation line is actually present, and avoids rescue
expression parsing for dialogue bracket payloads that are obvious narrative
text. The checked-in `009_nonuniform_map_pure_batch.arcw` bench still reports a
path-free source name, `wiki_scan_performed = 0`,
`dot_normalization_owned = 0`, `dialogue_rescue_expr_parse_attempts = 0`,
`line_owned_bytes = 0`, `block_owned_bytes = 0`, and
`pure_flatten_bytes_copied_median = 0`; the local run after source-backed block
fragments, map/sum suffix-use optimization, and pure-aware flow/source/stream
expression lowering reported parse phase 3250000 ns, runtime-plan lowering
phase 396400 ns, and runtime median 11500 ns.

Runtime numeric sequence lowering now preserves integer-only bracket literals as
`RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I64(_)))` instead of eagerly
materializing a row-value vector. Scalar integer runtime values are now
width-preserving: `RuntimeValue::Int(RuntimeInt::I8/I16/I32/I64/I128/ISize)`
and `RuntimeValue::UInt(RuntimeUInt::U8/U16/U32/U64/U128/USize)` keep suffix
evidence across materialization, spread, dynamic arguments, and projection
fallbacks. The same `DenseSeqStorage<T>` backing
store also covers fixed-width integer sequences (`i8`, `i16`, `i32`, `i64`,
`i128`, `u8`, `u16`, `u32`, `u64`, `u128`) and target-sized spellings
(`isize`, `usize`) plus unit, bool, byte, char, logical-duration, string,
native `f32`/`f64`, and entity-reference sequences. Target-sized dense storage
uses stable `i64`/`u64` runtime buffers rather than host pointer-width buffers
so path-free bench output remains cross-platform comparable. Pure map/sum fast paths
consume i64 dense storage directly, dense
integer `sum()` consumes the storage without materializing `RuntimeValue`
elements, and byte-oriented host payloads consume dense byte/u8 storage
directly; dynamic sequence operations such as `for`, `await many`, spread
arguments, and bracket patterns materialize only at those dynamic boundaries.
Dynamic bracket evaluation now folds homogeneous scalar results back into dense
storage, so sequences containing locals or entity references are not forced to
stay in `Vec<RuntimeValue>` after evaluation.

The checked-in nonuniform map pure JIT fixture was remeasured after the
`RuntimeSeq::Dense(DenseSeq::I64(DenseSeqStorage<i64>))` migration. With the same
checked-in bench settings, median elapsed time was 11700 ns. That is within
normal short-run noise of the previous dense i64 runtime sequence sample at
12200 ns and still faster than the earlier direct i64 sequence value sample at
14300 ns.
Deterministic counters stayed on the borrowed fast path:
`pure_flat_batch_calls_median = 1`, `pure_flat_batch_items_median = 128`,
`pure_flat_batch_bytes_borrowed_median = 2048`,
`pure_flatten_materializations_median = 0`,
`pure_flatten_bytes_copied_median = 0`, `pure_arg_vec_allocations_median = 0`,
`pure_arg_bytes_borrowed_median = 2048`, and
`pure_result_bytes_copied_median = 0`.

Runtime-plan map/sum optimization no longer builds a borrowed local-use suffix
table for each flow slice. It recognizes concrete adjacent map/sum and
sequence-map-sum candidate windows first, then scans later ops only for the
specific local names whose deadness matters. A regression keeps sequence
bindings live when the map body reads the same local.
Flow, source, and stream expression lowering receive the pure-helper map before
constructing runtime expressions, so pure calls are emitted as
`RuntimeExpr::PureCall` without a later plan-wide rewrite traversal. The
runtime-plan finalization pass now only optimizes flow map/sum windows. Bench
and profile JSON expose this as `compiler.runtime_plan`: the checked-in 009
fixture reports `pure_helpers = 1`, `pure_candidate_functions_seen = 1`,
`pure_candidate_lower_attempts = 1`, `pure_expr_lowered_nodes = 5`,
`pure_expr_cloned_nodes = pure_expr_lowered_nodes`,
`pure_rewrite_expr_visits = 0`, `local_use_tail_scans = 2`,
`local_use_scan_ops = 7`, `sequence_map_sum_fusions = 1`, and
`pure_call_exprs = 1`. The same path-free
run reported median elapsed time 14600 ns, `pure_flat_batch_items_median =
128`, `pure_flat_batch_bytes_borrowed_median = 2048`, and zero flatten
materializations, argument vector allocations, copied argument bytes, and copied
result bytes.

Borrow-state branch tracking now records checkpoint/journal deltas rather than
full branch maps. The borrow-check JSON includes `state_delta_entries`,
`state_full_clones`, and `state_merge_keys` so branch-heavy fixtures can show
whether restore/merge work is proportional to touched borrow locals instead of
the whole borrow map. The targeted branch-drop regression keeps
`state_full_clones = 0` while still reporting a conditional-drop diagnostic.
Runtime line and child-task scopes also snapshot borrow state with the same
checkpoint journal instead of cloning the borrow map, while preserving their
presentation and lifetime scope metadata separately.

The checked-in dense i32 and u64 sum fixtures measure the non-JIT fixed-width
integer path. The i32 fixture lowered `[... i32]` to `DenseSeq::I32`, validated
as `Vec(I32)`, and ran `sum()` with median elapsed time 8200 ns in the latest
local run. The matching u64 fixture lowered `[... u64]` to `DenseSeq::U64`,
validated as `Vec(U64)`, and ran with median elapsed time 8500 ns. The
multi-width fixture lowered i8/i16/i32/u8/u16/u32/u64 literal sequences to the
matching dense storage, validated the first visible JSON samples as `Vec(I8)`,
`Vec(I16)`, `Vec(I32)`, and `Vec(U8)`, and reduced seven dense sequences in one
flow with median elapsed time 21400 ns. The runtime-plan unit test covers all
seven dense storage variants directly. The pure-call counters remained zero for
these fixtures because they intentionally measure the VM dense sequence
reduction path, not the pure helper accelerator.

The VM and scalar AOT paths now keep pure-helper integer width information
instead of widening all dense integer storage into the i64 accelerator. Runtime
pure helper metadata preserves signed and unsigned integer input widths plus the
declared output width. The exact i64 flat-batch accelerator owns native JIT, and
scalar AOT covers `i8`, `i16`, `i32`, `i128`, `u8`, `u16`, `u32`, `u64`, and
`u128` without materializing `RuntimeValue` arguments. The VM has matching
exact integer flat-batch sum ABI coverage for the same widths. These paths
borrow the matching `&[T]` storage directly and convert only the scalar result
into the `i64` sum accumulator. The
checked-in `016_dense_i32_map_pure_batch.arcw` and
`017_dense_u32_map_pure_batch.arcw` benches validate the width-preserving path
with `pure_flat_batch_items_median = 128`,
`pure_flat_batch_bytes_borrowed_median = 1024`,
`pure_arg_bytes_borrowed_median = 1024`, and no result copy in the fused
`map(...).sum()` path. `018_dense_u64_map_pure_batch.arcw` covers the same path
with `pure_flat_batch_bytes_borrowed_median = 2048`, and uses checked conversion
when accumulating the `u64` pure-helper result into an `i64` sum. The
`019_dense_i128_map_pure_batch.arcw` and `020_dense_u128_map_pure_batch.arcw`
fixtures cover wide integer storage with
`pure_flat_batch_bytes_borrowed_median = 4096` and the same checked sum
conversion. This confirms the hot boundary is not doing
`.map(i64::from)`. Native JIT covers exact i64 plus width-preserving ABIs for
`i8`, `i16`, `i32`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128`, `f32`, and
`f64`. The `i128` and `u128` JIT path is batch-only: the native ABI receives
flat row buffers by pointer and never exposes by-value wide integers at the
function boundary. Scalar AOT and VM remain the semantic scalar tiers for
wide-integer calls outside the batch shape. Target-sized dense storage already
uses stable `i64`/`u64` backing at the runtime boundary.

The i32 JIT ABI emits `extern "C" fn(i32, ...) -> i32` helpers plus row-major
`*const i32` flat batch and batch-sum entry points. A local path-free bench run
of `016_dense_i32_map_pure_batch.arcw` with the default auto backend promoted
the helper to native i32 JIT, reported median elapsed time 12800 ns, and kept
`pure_jit_calls_median = 128`, `pure_aot_calls_median = 0`,
`pure_vm_calls_median = 0`, `pure_arg_vec_allocations_median = 0`, and
`auto_jit_promotions = 1`. The same fixture with `--pure-backend aot` reported
median elapsed time 43900 ns and `pure_aot_calls_median = 128`, so the i32 JIT
ABI is the expected natural fast tier for that shape.

The float JIT ABI now mirrors the exact integer path for scalar helpers:
it emits native `extern "C" fn(f32, ...) -> f32` and
`extern "C" fn(f64, ...) -> f64` calls plus row-major `*const f32` and
`*const f64` flat batch entry points. A local path-free bench run of
`022_dense_f32_map_pure_batch.arcw` with the default auto backend promoted the
natural flow map to native f32 JIT, reported median elapsed time 16200 ns, and
kept `pure_jit_calls_median = 128`, `pure_aot_calls_median = 0`,
`pure_vm_calls_median = 0`, `pure_arg_vec_allocations_median = 0`,
`pure_arg_bytes_borrowed_median = 1024`, and
`pure_result_bytes_copied_median = 512`. The same fixture with
`--pure-backend aot` reported median elapsed time 46100 ns and
`pure_aot_calls_median = 128`. The checked f64 fixture
`023_dense_f64_map_pure_batch.arcw` reported median elapsed time 15900 ns with
default auto JIT, `pure_arg_bytes_borrowed_median = 2048`, and
`pure_result_bytes_copied_median = 1024`; the same fixture with
`--pure-backend aot` reported median elapsed time 44700 ns. The f32/f64 JIT ABI
is therefore the natural fast tier for these batched map shapes while still
preserving the typed result vector copy as the only measured output movement.

The dense scalar length fixture covers the non-integer deterministic scalar
storage cases. It lowers unit, bool, char, logical-duration, and `u8` sequences
into dense storage, checks `len()` as `usize`, and evaluates length through
`RuntimeSeq::len()` rather than materializing `RuntimeValue` elements. The local
path-free bench run reported median elapsed time 16100 ns for eight
executed ops, with `pure_flatten_materializations_median = 0`,
`pure_arg_vec_allocations_median = 0`, `pure_result_bytes_copied_median = 0`,
and no source path in the JSON output.

The dense textual scalar length fixture covers homogeneous textual and typed
float scalar runtime values. It keeps `String`, typed native `f64` values,
and entity-reference sequences dense, including entity
references that are only known after runtime evaluation. The local path-free
bench run reported median elapsed time 15800 ns
for seven executed ops, with `pure_arg_vec_allocations_median = 0`,
`pure_result_bytes_copied_median = 0`, and no source path in the JSON output.

The dense wide numeric length fixture covers the remaining integer primitive
spellings. It lowers `i128`, `u128`, `isize`, and `usize` bracket literals to
`DenseSeq::I128`, `DenseSeq::U128`, `DenseSeq::ISize`, and `DenseSeq::USize`,
then reads `RuntimeSeq::len()` without materializing scalar runtime values.
The local path-free bench run reported median elapsed time 13900 ns
for seven executed ops, with `pure_flatten_materializations_median = 0`,
`pure_arg_vec_allocations_median = 0`, `pure_result_bytes_copied_median = 0`,
and no source path in the JSON output.

`DenseSeq::F32`/`DenseSeq::F64` use `DenseSeqStorage<f32>` and
`DenseSeqStorage<f64>` directly. Typed f32/f64 pure helper calls now use
borrowed slice ABI in the VM flow path and scalar AOT path, and both f32 and
f64 helpers can promote from auto AOT to native Cranelift JIT for flat batches
without materializing `Vec<RuntimeValue>` arguments. f64 helpers use the same
native JIT promotion shape with double-width borrowed argument/output accounting.
Exact-width integer helpers use the same scalar AOT boundary for non-i64 widths;
native Cranelift JIT covers i64, width-preserving i32 scalar/batch ABI, and
f32/f64 scalar/batch ABI. The latest dense literal length/sum benches reported
8300 ns for `010_dense_i32_sum.arcw`, 8100 ns for
`011_dense_u64_sum.arcw`, 21700 ns for
`012_dense_integer_widths_sum.arcw`, and zero materializations, copied bytes,
or argument Vec allocations in all dense 010-015 runs.
Record/tuple storage should be designed separately as columnar storage rather
than as scalar `DenseSeqStorage<T>`. The scalar dense coverage is encoded in
`DenseSeqKind`, so adding another dense class requires extending the typed kind
and its borrowed view/materialization tests together.

## Runtime Matrix And Tensor Math

Arcweft now has built-in dense `f32` and `f64` matrix and tensor runtime
values: `RuntimeValue::MatrixF32(DenseMatrixF32)`,
`RuntimeValue::TensorF32(DenseTensorF32)`,
`RuntimeValue::MatrixF64(DenseMatrixF64)`, and
`RuntimeValue::TensorF64(DenseTensorF64)`. `arcweft-core` owns only the
deterministic row-major data model and scalar baseline kernels, implemented as
generic dense storage with width-specific runtime value variants. Native
acceleration stays in `arcweft-runtime-accelerator`, where
`RuntimeMathAccelerator` can select `scalar`, `glam`, `ndarray`, `wgpu`, or
`auto`.

Flow execution now routes `math.matmul_f32`, `math.matrix_add_f32`, and
`math.tensor_add_f32`, plus the matching `math.matmul_f64`,
`math.matrix_add_f64`, and `math.tensor_add_f64` calls, through the same Sans
I/O `RuntimePureCallBackend` adapter boundary used by pure helper
acceleration. The VM backend keeps the scalar deterministic baseline.
`RuntimePureAccelerator` owns a
`RuntimeMathAccelerator`, so programs using built-in `math.*` calls naturally
receive the configured math backend without adding GPU or ndarray dependencies
to `arcweft-core`. Runtime stats record `math_calls` and
`math_accelerated_calls` alongside the existing pure helper counters.
`RuntimePureAcceleratorConfig` now carries the math backend policy, so CLI
commands and launch profiles can select `auto`, `scalar`, `glam`, `ndarray`,
or `wgpu` with the same runtime config object that already controls pure helper
VM/AOT/JIT execution. `arcw run`, `profile`, `cli`, `serve`, `test`, `bench`,
and `verify-types --run` accept `--math-backend` and
`--math-wgpu-min-elements`; `[profiles.NAME.pure]` supports
`math_backend` and `math_wgpu_min_elements`, with CLI flags taking precedence.
Executor JSON reports both selected values under `pure_config`.

The wgpu path is feature-gated by `math-wgpu` and keeps Windows DX12 enabled
alongside Vulkan, Metal, and GLES. The workspace Rust floor is raised to 1.96
so the latest wgpu stack can be used without pinning older graphics crates.
If a GPU adapter is unavailable, explicit `wgpu` measurement reports a
structured skip/error rather than silently changing the requested backend.
Portable wgpu compute shaders in this workspace are `f32` kernels; `f64`
matrix/tensor calls stay on scalar, glam 4x4, or ndarray CPU backends and
preserve `f64` storage across the runtime boundary. `Auto` never selects wgpu
for `f64`; explicit `wgpu` requests return a structured portability error for
those kernels.
Compile-time selection now also separates native accelerator code from browser
Wasm builds. `native-jit` is target-specific to non-`wasm32`, and the blocking
native wgpu math dispatch is selected only for non-`wasm32`;
`wasm32-unknown-unknown` builds keep the accelerator API available but route
pure helpers through VM/AOT CPU paths. Browser WebGPU math is available as a
separate async adapter module for dense `f32` matmul, matrix add, and tensor
add. The async path compiles for `wasm32-unknown-unknown --all-features`; it
is measured by the path-free browser benchmark harness before browser-side
`Auto` thresholds are changed.
The browser adapter now exposes structured availability/fallback reasons,
portable WebGPU default-limit validation, async `map_async` readback counters,
prepared/resident buffer APIs, and submitted-work handles that allow browser
players to submit resident GPU work and await readback later. Browser WebGPU is
still an explicit async adapter optimization, not a synchronous VM math backend.

`arcweft-browser-bench` provides the first path-free browser benchmark export:

```bash
just browser-webgpu-bench-check
just browser-webgpu-bench-build
just browser-webgpu-bench-smoke
just browser-webgpu-bench-perf
```

The exported browser function returns JSON with Auto dispatch, CPU Wasm,
WebGPU one-shot, prepared upload, prepared resident, prepared-capacity
resident, async resident, pipelined resident, and prepared-capacity pipelined
resident cases. Auto cases go through the typed browser math dispatcher and
record the policy-selected capacity when WebGPU is selected. Prepared cases
include an optional typed `capacity` field separate from the actual `shape`, so
the report can distinguish exact resident storage from overprovisioned capacity
storage without recording host paths. The same report also includes typed
`recommendations` per operation/shape. A recommendation records the selected
mode, selected capacity, CPU median, selected median, speedup, and reason
(`web_gpu_faster`, `cpu_faster_or_equal`, `missing_cpu_baseline`, or
`no_measured_web_gpu_case`). Auto cases are reported as policy observations and
are not treated as independent candidate modes for choosing the fastest backend.
When WebGPU limits are available, the same recommendation also records the
runtime policy mode, policy capacity, policy reason, and whether the policy
matches the measured winner. This gives browser-side `Auto` threshold tuning a
Rust-produced source of truth instead of a JS-only summary. If WebGPU is
unavailable, the report records a structured skip reason instead of failing the
whole run. The smoke recipe drives local Chrome/Edge through DevTools when
available. If the browser executable is not discoverable from the environment,
set `CHROME` or `CHROME_BIN` before running the recipe.

Local browser WebGPU measurements on the current Windows/Chrome environment:

| Case | Best browser WebGPU mode | CPU Wasm median ms | GPU median ms | Speedup | Notes |
| --- | --- | ---: | ---: | ---: | --- |
| `matmul_f32_m256_k256_n256` | prepared capacity resident pipelined | 6.815 | 0.8575 | 7.95x | recommendation selected capacity `512x512x512`, submit median 0.06 ms, readback median 0.36 ms |
| `matmul_f32_m128_k128_n128` | prepared resident pipelined | 0.81 | 0.58125 | 1.39x | recommendation selected exact capacity `128x128x128`, submit median 0.03 ms, readback median 0.315 ms |
| `matmul_f32_m64_k64_n64` | prepared resident pipelined | 0.305 | 0.47375 | 0.64x | below crossover on this browser |
| `tensor_add_f32_len4194304` | prepared resident async | 10.01 | 19.26 | 0.52x | readback bandwidth dominates |

The measured browser crossover is currently matmul-oriented: `128x128x128` and
larger dense `f32` matmul can benefit from pipelined resident WebGPU, while
simple elementwise add remains CPU-preferred when the result must be read back
to Wasm each iteration.
`arcweft-runtime-accelerator::math::browser_webgpu_policy` now owns the
target-independent browser Auto policy used to preserve that measured
crossover in code. Its default policy keeps elementwise `f32` on CPU Wasm,
selects exact prepared resident pipelined WebGPU for `128x128x128` matmul and
larger, and selects capacity-prepared pipelined WebGPU for `256x256x256`
matmul and larger when storage limits allow it. The browser benchmark harness
uses the same typed capacity growth policy for overprovisioned prepared cases,
so measured `capacity` fields and runtime Auto decisions do not drift through
duplicated arithmetic.
`BrowserWebGpuMathContext` now exposes policy-driven async Auto calls for
`matmul_f32`, `matrix_add_f32`, and `tensor_add_f32`. Browser embeddings can
call these methods at the adapter boundary: CPU-selected work runs through the
deterministic scalar baseline, while WebGPU-selected work uses prepared
resident buffers, submits GPU work asynchronously, awaits readback, and returns
the dense deterministic value plus the policy selection and transfer counters.
The same module also exposes `BrowserWebGpuAutoMathAdapter` with borrowed
`BrowserWebGpuMathRequest` inputs and typed `BrowserWebGpuMathResponse`
outputs, so browser host code can route natural `math.*` operations through one
async dispatch boundary without stringly typed operation switches or
pre-dispatch dense-buffer copies.
The adapter now separates policy selection and submission from readback through
`BrowserWebGpuMathDispatch` and `BrowserWebGpuSubmittedMath`. CPU-selected work
returns an immediate typed response, while WebGPU-selected work returns a
submitted handle that can be read later. The value-returning `dispatch` API is
still built on top of that split path, so host code can either await the final
value directly or batch GPU submissions and overlap browser scheduling with
delayed readback.
This keeps browser GPU work outside the Sans I/O core while letting natural
browser-side math calls use the calibrated policy without duplicating threshold
logic in the player.

Latest path-free browser perf run after the split submission API:

| Case | Mode | CPU median ms | Mode median ms | Speedup | Notes |
| --- | --- | ---: | ---: | ---: | --- |
| `matmul_f32_m256_k256_n256` | auto pipelined | 7.325 | 1.09875 | 6.67x | policy-selected WebGPU, submit median 0.165 ms, readback median 0.43 ms |
| `matmul_f32_m256_k256_n256` | direct auto dispatch | 7.325 | 4.065 | 1.80x | value-returning path still waits for readback per call |
| `matmul_f32_m256_k256_n256` | prepared resident pipelined | 7.325 | 0.52875 | 13.85x | current best measured manual backend |
| `matmul_f32_m128_k128_n128` | auto pipelined | 0.87 | 0.49375 | 1.76x | policy-selected WebGPU with split submission/readback |
| `tensor_add_f32_len65536` | auto pipelined | 0.08 | 0.10 | 0.80x | policy selected CPU immediate work in the same split API |

The `auto_pipelined` benchmark mode is a policy observation, not a backend
recommendation candidate. Manual prepared modes remain the source for backend
recommendations, while auto modes show the overhead and scheduling behavior
that natural browser-side calls see.

`arcweft-runtime-accelerator` also contains the first forward-only inference
graph API. The graph uses typed tensor IDs and validates shapes during graph
construction. The session executes through an `InferenceAdapter`, keeping the
typed graph and backend execution policy separated. The default adapter is
backed by `RuntimeMathAccelerator` for dense tensor matmul and deterministic CPU
kernels for non-matmul forward ops. The current deterministic `f32` op set is:

- `matmul`
- exact-shape `add`
- last-dimension `bias_add`
- valid NCHW/OIHW `conv2d`
- `relu`
- `max_pool2d`
- last-dimension `softmax`
- last-dimension `argmax`
- outer-dimension preserving `flatten`

Arcweft flow execution can call the same op family through adapter-contributed
external calls such as `conv2d.valid_f32`, `infer.relu_f32`,
`infer.max_pool2d_f32`, `infer.flatten_outer_f32`, `infer.matmul_f32`, and
`infer.argmax_last_dim_f32`. These names are not Core intrinsics and are not in
the default prelude. `arcweft-adapter-context` contributes the optional
type-checking namespace, while `RuntimePureAccelerator` resolves the named
runtime calls through `RuntimeExternalCallBackend` and uses the configured math
backend for rank-2 tensor matmul.

The checked tests include a small MNIST-shaped MLP forward graph with input
shape `1x28x28`, flattening to `1x784`, two dense layers, ReLU, and `argmax`;
it verifies that a fixed synthetic image classifies to class `7`. A second
MNIST-shaped CNN test runs `conv2d`, ReLU, `max_pool2d`, flatten, dense matmul,
and `argmax` over a fixed synthetic image and also classifies to class `7`.
This is inference-only: no autograd, optimizer, weight loading, or training
graph is implemented yet.

Local path-free measurements:

```bash
just bench-math-cpu
just bench-math-glam
just bench-math-wgpu
just bench-math-matrix-add
just bench-math-tensor-add
just bench-024
just bench-024-wgpu-auto
just bench-025
just bench-026
arcw bench tests/fixtures/arcw/spec_should_pass/bench/027_matrix_matmul_f64.arcw --math-backend ndarray --value lhs=matrix/f64/2x2:1.5,2,3.25,4.5 --value rhs=matrix/f64/2x2:5,6.5,7,8.25 --json
arcw bench tests/fixtures/arcw/spec_should_pass/bench/028_tensor_add_f64.arcw --math-backend ndarray --value lhs=tensor/f64/2x2:1.5,2.25,3.75,4.5 --value rhs=tensor/f64/2x2:5,6.25,7.5,8.75 --json
cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul --size 512 --iterations 3 --warmup 1 --reuse
just bench-math-matmul-reuse-update
just bench-math-matrix-add-reuse
just bench-math-matrix-add-reuse-update
just bench-math-tensor-add-reuse
just bench-math-tensor-add-reuse-update
```

Representative release results on the local machine:

| fixture | backend | status | median ns | note |
| --- | --- | --- | ---: | --- |
| 4x4 matmul_f32 | scalar | measured | 100 | small matrix baseline |
| 4x4 matmul_f32 | glam | measured | 100 | SIMD-friendly game math backend |
| 4x4 matmul_f32 | auto | measured | 100 | selected glam |
| flow 4x4 matmul_f32 | glam | measured | 6700 | `--value` matrix input, `math_calls_median = 1`, `math_accelerated_calls_median = 1`, `bytes_borrowed = 128`, `bytes_copied = 0` |
| flow 8x8 matmul_f32 | auto wgpu | measured | 473400 | `--features math-wgpu`, `--math-wgpu-min-elements 1`, prepared cache hit after warmup, `bytes_uploaded = 0`, `gpu_buffer_reuse_hits = 4`, `last_auto_reason = matmul_wgpu_work_threshold` |
| flow 4x4 matrix_add_f32 | ndarray | measured | 9900 | `--value` matrix input, one ndarray view add, `bytes_borrowed = 128`, `bytes_copied = 0` |
| flow 2x2x2 tensor_add_f32 | ndarray | measured | 13400 | `--value` tensor input, one ndarray dynamic-view add, `bytes_borrowed = 64`, `bytes_copied = 0` |
| flow 2x2 matmul_f64 | ndarray | measured | 2100 | `--value` matrix input, `math_calls_median = 1`, `math_accelerated_calls_median = 1`, `bytes_borrowed = 64`, `result_bytes_copied = 32` |
| flow 2x2 tensor_add_f64 | ndarray | measured | 2200 | `--value` tensor input, `math_calls_median = 1`, `math_accelerated_calls_median = 1`, `bytes_borrowed = 64`, `result_bytes_copied = 32` |
| 64x64 matmul_f32 | scalar | measured | 21300 | row-major baseline |
| 64x64 matmul_f32 | ndarray | measured | 26700 | general CPU matrix backend |
| 64x64 matmul_f32 | auto | measured | 24200 | selected ndarray without wgpu feature |
| 128x128 matmul_f32 | ndarray | measured | 98500 | CPU backend with wgpu feature enabled |
| 128x128 matmul_f32 | wgpu | measured | 217800 | upload/download dominates |
| 128x128 matmul_f32 | wgpu prepared | measured | 135800 | 4 buffer creations, 16 buffer reuse hits, 3 staging reuse hits |
| 128x128 matmul_f32 | wgpu prepared update | measured | 283000 | `--reuse-update-inputs`, one initial buffer allocation, four upload+dispatch passes, `gpu_buffer_reuse_hits = 28` |
| 128x128 matmul_f32 | wgpu prepared capacity | measured | 204500 | `--reuse-capacity`, capacity 256, one initial upload, five measured dispatches, `gpu_buffer_reuse_hits = 27` |
| 128x128 matmul_f32 | auto | measured | 43700 | selected ndarray |
| 256x256 matmul_f32 | ndarray | measured | 404400 | CPU backend |
| 256x256 matmul_f32 | wgpu | measured | 444300 | still not consistently faster |
| 256x256 matmul_f32 | auto | measured | 536600 | selected ndarray after threshold tuning |
| 512x512 matmul_f32 | ndarray | measured | 2553400 | CPU backend |
| 512x512 matmul_f32 | wgpu | measured | 2115100 | DX12-capable wgpu compute |
| 512x512 matmul_f32 | auto | measured | 2334400 | selected wgpu |
| 4096x4096 matrix_add_f32 | scalar | measured | 48541500 | row-major baseline |
| 4096x4096 matrix_add_f32 | ndarray | measured | 57826700 | CPU elementwise backend |
| 4096x4096 matrix_add_f32 | wgpu | measured | 198692400 | copy dominated one-shot GPU path |
| 4096x4096 matrix_add_f32 | auto | measured | 49352100 | selected ndarray |
| 4096x4096 tensor_add_f32 | scalar | measured | 44410400 | row-major tensor baseline |
| 4096x4096 tensor_add_f32 | ndarray | measured | 41737000 | CPU elementwise backend |
| 4096x4096 tensor_add_f32 | wgpu | measured | 161161500 | copy dominated one-shot GPU path |
| 4096x4096 tensor_add_f32 | auto | measured | 46863800 | selected ndarray |
| 2048x2048 matrix_add_f32 | wgpu one-shot | measured | 53726400 | 16 buffer creations across warmup + samples |
| 2048x2048 matrix_add_f32 | wgpu prepared | measured | 21763000 | caller-owned output buffer, 4 buffer creations, 16 reuse hits |
| 2048x2048 tensor_add_f32 | wgpu prepared | measured | 23814100 | caller-owned output buffer, 4 buffer creations, 16 reuse hits |
| 64x64 matrix_add_f32 | wgpu prepared update | measured | 1345000 | `--reuse-update-inputs`, one initial buffer allocation, three upload+dispatch passes, `gpu_buffer_reuse_hits = 21` |
| 64x64 matrix_add_f32 | wgpu prepared capacity | measured | 157300 | `--reuse-capacity`, capacity 128, one initial upload, five measured dispatches, `gpu_buffer_reuse_hits = 27` |
| 64x64 tensor_add_f32 | wgpu prepared capacity | measured | 448200 | `--reuse-capacity`, capacity 8192 values, one initial upload, five measured dispatches, `gpu_buffer_reuse_hits = 27` |

These numbers show the current backend split: glam is the intended path for
fixed 4x4 matrices, ndarray/scalar win smaller one-shot CPU workloads, and wgpu
becomes useful for larger matmul workloads once arithmetic work amortizes
upload/download cost. Auto therefore keeps one-shot elementwise kernels on the
CPU backend and only considers wgpu for matmul above the configured work
threshold. Repeated matrix multiplication and explicit-wgpu elementwise
matrix/tensor kernels can now use prepared GPU buffers. Exact repeated inputs
keep the fixed input buffers resident and download only the result for each
dispatch. Changed inputs reuse the same storage buffers and bind group, write
the new `f32` input values with `queue.write_buffer`, and then dispatch without
creating new GPU buffers. Native prepared APIs also support capacity-prepared
matrix/tensor buffers, so smaller compatible shapes can run inside one prepared
allocation. The standalone `math_bench` example exposes this with
`--reuse-capacity`, and the Justfile provides `bench-math-*-reuse-capacity`
recipes so the capacity path can be timed without recording host paths. Auto
matmul uses the same
prepared-buffer path when the configured work threshold selects wgpu; Auto
elementwise stays on the CPU backend because the current one-shot GPU path is
copy dominated in local measurements. Runtime/Auto prepared caches use
power-of-two capacity buckets for cache eligibility, so a smaller compatible
shape can reuse an existing prepared allocation. The cache still tracks the
last exact shape and full `f32` bit-pattern fingerprints: shape changes or
value changes update shader params/input buffers, while exact repeated values
skip the upload and only dispatch/read back the result.
The wgpu readback path also keeps a reusable MAP_READ staging buffer in the
adapter context; one-shot calls and prepared dispatches grow it on demand and
remap it instead of allocating a fresh download buffer for every result.

The math bench JSON is emitted through a typed serde report rather than
hand-built string output. It reports the requested backend, measured status,
correctness-checked timing samples, the backend that actually executed last,
and the `last_auto_reason` policy label when `auto` made the choice. Flow bench
JSON reports median numeric math counters and modal categorical backend/reason
labels across samples, so occasional fallback samples do not make categorical
fields depend on first-sample ordering. The same report includes accelerator
copy counters split into borrowed bytes, copied bytes, uploaded bytes,
downloaded bytes, GPU buffer creations, GPU buffer reuse hits, staging buffer
creations, staging buffer reuse hits, and reused dispatches. Explicit `wgpu`
requests remain explicit:
unavailable adapters or disabled features produce a structured skip/error,
while `auto` records the chosen policy and fallback path through the backend
counters. The example-level serialization test keeps the report parseable and
checks that generated JSON does not embed host absolute paths.
