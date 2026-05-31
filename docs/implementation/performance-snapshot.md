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
rather than runtime value equality.
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
`.map(i64::from)`. Native JIT covers exact i64 and now has the first
width-preserving non-i64 ABI for `i32`; scalar AOT remains the native tier for
the other non-i64 helpers while the width-preserving VM fast path is the
semantic execution tier for the remaining integer widths. Target-sized dense
storage already uses stable `i64`/`u64` backing at the runtime boundary.

The i32 JIT ABI emits `extern "C" fn(i32, ...) -> i32` helpers plus row-major
`*const i32` flat batch and batch-sum entry points. A local path-free bench run
of `016_dense_i32_map_pure_batch.arcw` with the default auto backend promoted
the helper to native i32 JIT, reported median elapsed time 12800 ns, and kept
`pure_jit_calls_median = 128`, `pure_aot_calls_median = 0`,
`pure_vm_calls_median = 0`, `pure_arg_vec_allocations_median = 0`, and
`auto_jit_promotions = 1`. The same fixture with `--pure-backend aot` reported
median elapsed time 43900 ns and `pure_aot_calls_median = 128`, so the i32 JIT
ABI is the expected natural fast tier for that shape.

The dense scalar length fixture covers the non-integer deterministic scalar
storage cases. It lowers unit, bool, char, logical-duration, and `u8` sequences
into dense storage, checks `len()` as `usize`, and evaluates length through
`RuntimeSeq::len()` rather than materializing `RuntimeValue` elements. The local
path-free bench run reported median elapsed time 15400 ns for eight
executed ops, with `pure_flatten_materializations_median = 0`,
`pure_arg_vec_allocations_median = 0`, `pure_result_bytes_copied_median = 0`,
and no source path in the JSON output.

The dense textual scalar length fixture covers homogeneous textual and typed
float scalar runtime values. It keeps `String`, typed native `f64` values,
and entity-reference sequences dense, including entity
references that are only known after runtime evaluation. The local path-free
bench run reported median elapsed time 17200 ns
for seven executed ops, with `pure_arg_vec_allocations_median = 0`,
`pure_result_bytes_copied_median = 0`, and no source path in the JSON output.

The dense wide numeric length fixture covers the remaining integer primitive
spellings. It lowers `i128`, `u128`, `isize`, and `usize` bracket literals to
`DenseSeq::I128`, `DenseSeq::U128`, `DenseSeq::ISize`, and `DenseSeq::USize`,
then reads `RuntimeSeq::len()` without materializing scalar runtime values.
The local path-free bench run reported median elapsed time 13500 ns
for seven executed ops, with `pure_flatten_materializations_median = 0`,
`pure_arg_vec_allocations_median = 0`, `pure_result_bytes_copied_median = 0`,
and no source path in the JSON output.

`DenseSeq::F32`/`DenseSeq::F64` use `DenseSeqStorage<f32>` and
`DenseSeqStorage<f64>` directly. Typed f32/f64 pure helper calls now use
borrowed slice ABI in the VM flow path and scalar AOT path, so a natural
`pure` call in a flow can avoid `Vec<RuntimeValue>` argument allocation when
the helper signature and expression are float-scalar only. Exact-width integer
helpers use the same scalar AOT boundary for non-i64 widths; native Cranelift
JIT covers i64 and the first width-preserving i32 scalar/batch ABI. Record/tuple
storage should be designed separately as columnar storage rather than as scalar
`DenseSeqStorage<T>`. The scalar dense coverage is encoded in `DenseSeqKind`,
so adding another dense class requires extending the typed kind and its borrowed
view/materialization tests together.

## Runtime Matrix And Tensor Math

Arcweft now has built-in dense `f32` matrix and tensor runtime values:
`RuntimeValue::MatrixF32(DenseMatrixF32)` and
`RuntimeValue::TensorF32(DenseTensorF32)`. `arcweft-core` owns only the
deterministic row-major data model and scalar baseline kernels. Native
acceleration stays in `arcweft-runtime-accelerator`, where
`RuntimeMathAccelerator` can select `scalar`, `glam`, `ndarray`, `wgpu`, or
`auto`.

The wgpu path is feature-gated by `math-wgpu` and keeps Windows DX12 enabled
alongside Vulkan, Metal, and GLES. The workspace Rust floor is raised to 1.96
so the latest wgpu stack can be used without pinning older graphics crates.
If a GPU adapter is unavailable, explicit `wgpu` measurement reports a
structured skip/error rather than silently changing the requested backend.

Local path-free measurements:

```bash
just bench-math-cpu
just bench-math-glam
just bench-math-wgpu
just bench-math-matrix-add
just bench-math-tensor-add
just bench-math-matrix-add-reuse
just bench-math-tensor-add-reuse
```

Representative release results on the local machine:

| fixture | backend | status | median ns | note |
| --- | --- | --- | ---: | --- |
| 4x4 matmul_f32 | scalar | measured | 100 | small matrix baseline |
| 4x4 matmul_f32 | glam | measured | 100 | SIMD-friendly game math backend |
| 4x4 matmul_f32 | auto | measured | 100 | selected glam |
| 64x64 matmul_f32 | scalar | measured | 21300 | row-major baseline |
| 64x64 matmul_f32 | ndarray | measured | 26700 | general CPU matrix backend |
| 64x64 matmul_f32 | auto | measured | 24200 | selected ndarray without wgpu feature |
| 128x128 matmul_f32 | ndarray | measured | 98500 | CPU backend with wgpu feature enabled |
| 128x128 matmul_f32 | wgpu | measured | 217800 | upload/download dominates |
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

These numbers show the current backend split: glam is the intended path for
fixed 4x4 matrices, ndarray/scalar win smaller one-shot CPU workloads, and wgpu
becomes useful for larger matmul workloads once arithmetic work amortizes
upload/download cost. Auto therefore keeps one-shot elementwise kernels on the
CPU backend and only considers wgpu for matmul above the configured work
threshold. Repeated elementwise matrix/tensor kernels can now use prepared GPU
buffers, which uploads the fixed inputs once, reuses the storage/bind group
across warmup and measured samples, and downloads only the result for each
dispatch.

The math bench JSON reports the requested backend, measured status,
correctness-checked timing samples, the backend that actually executed last,
and accelerator copy counters split into borrowed bytes, copied bytes, uploaded
bytes, downloaded bytes, GPU buffer creations, GPU buffer reuse hits, and reused
dispatches. Explicit `wgpu` requests remain explicit: unavailable adapters or
disabled features produce a structured skip/error, while `auto` records the
fallback path through the backend counters.
