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
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw --json --iterations 2 --warmup 1 --samples 1 --steps 128 --max-ops 128 --pure-backend jit
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
| 033_mixed_for_iter_pure_jit.arcw | bytecode_vm | jit | 114200 | 40 | 0 | 0 | 0 | 320 | 40 | 24 |

The mixed fixture compiles three exact-width helpers (`i32`, `u32`, and `f32`)
and exercises both `.map` flat-batch calls and scalar `for` loop calls. The
same run reported `jit_successes = 3`, `pure_batch_calls_median = 3`,
`pure_flat_batch_bytes_borrowed_median = 192`, `pure_vm_calls_median = 0`,
`pure_fallbacks_median = 0`, and `pure_result_bytes_copied_median = 96`.

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
and `aot_attempts = 1`. The repeated-row map+sum specialization now uses the
same Auto promotion policy: `008_large_map_pure_batch.arcw` reported median
elapsed time `8700 ns`, `pure_batch_items_median = 4096`,
`pure_jit_calls_median = 4096`, `pure_aot_calls_median = 0`,
`auto_jit_deferred = 1`, and `auto_jit_promotions = 1`. The same fixture had
previously stayed on typed AOT in this path, so the change removes a repeated
batch policy gap rather than adding a compatibility path.
Hot scalar loops now use the same deferred policy family: each AutoAOT helper
accumulates scalar work units and promotes to native JIT once the hot scalar
loop crosses the JIT work threshold. `039_hot_for_pure_auto_jit.arcw` starts
with typed AOT during warmup, then measures ordinary `for` loops over `i128` and
`f32` on native JIT. A path-free local run reported median elapsed time
`1093100 ns`, `auto_jit_deferred = 2`, `auto_jit_promotions = 2`,
`jit_successes = 2`, `pure_calls_median = 256`,
`pure_jit_calls_median = 256`, `pure_aot_calls_median = 0`,
`pure_vm_calls_median = 0`, `pure_fallbacks_median = 0`,
`pure_arg_vec_allocations_median = 0`, and
`pure_arg_bytes_borrowed_median = 5120`.

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
| 006_linear_aot.arcw | aot | 12 | 1700 | 3 | 3 | 1 | 0 |
| 006_linear_aot.arcw | bytecode_vm | 12 | 2500 | 3 | 0 | 1 | 0 |
| 034_mixed_aot_prefix.arcw | aot | 12 | 2700 | 5 | 2 | 1 | 0 |
| 034_mixed_aot_prefix.arcw | bytecode_vm | 12 | 3100 | 5 | 0 | 1 | 0 |

The AOT executor now runs this fully linear flow from pre-lowered
`AotProgram` operations instead of cloning `FlowOp` values from the semantic
runtime plan for each fast-path step. The bytecode VM row is a same-fixture
comparison from the same local run. The mixed-prefix fixture confirms that AOT
can consume a pre-lowered setup prefix and continue through the VM-compatible
branch dispatcher within the same runtime step. That keeps the AOT fast-path
counter tied to lowered operations only while avoiding the previous host
boundary on tiny setup-then-branch flows. A longer 1000-iteration sanity run of
the same fixture measured 2400 ns median for AOT and 2700 ns median for the
bytecode VM.

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
`f64`. The `i128` and `u128` JIT native ABI receives flat row buffers by
pointer and never exposes by-value wide integers at the function boundary;
scalar calls reuse the same compiled artifact as one-row pointer batches.
Full-width wide-integer literals and captured constants are lowered inside
Cranelift with two 64-bit halves plus `iconcat`, so the path no longer depends
on an i64-backed immediate subset. Target-sized dense storage already uses
stable `i64`/`u64` backing at the runtime boundary. A path-free JIT bench run
of the checked-in
`019_dense_i128_map_pure_batch.arcw` and `020_dense_u128_map_pure_batch.arcw`
fixtures reported median elapsed times of 16400 ns and 15500 ns respectively,
with `pure_jit_calls_median = 128`,
`pure_flat_batch_bytes_borrowed_median = 4096`,
`pure_flatten_materializations_median = 0`, and
`pure_arg_vec_allocations_median = 0`.
A scalar wide-integer loop run of `038_wide_for_pure_jit.arcw` with
`--pure-backend jit` reported median elapsed time 83800 ns,
`jit_successes = 2`, `pure_calls_median = 16`,
`pure_jit_calls_median = 16`, `pure_vm_calls_median = 0`,
`pure_fallbacks_median = 0`, `pure_arg_vec_allocations_median = 0`,
`pure_arg_bytes_borrowed_median = 512`, and
`pure_result_bytes_copied_median = 0`.

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
native Cranelift JIT covers i64, width-preserving i8/i16/i32/i128/isize and
u8/u16/u32/u64/u128/usize scalar/batch paths, plus f32/f64 scalar/batch ABI.
Wide `i128`/`u128` scalar calls are represented as one-row pointer batches
rather than by-value ABI calls. Target-sized `isize` and `usize` use
transparent storage newtypes at the native boundary so JIT flat batches borrow
the existing dense buffers instead of copying into fixed-width temporaries. A
local path-free `--pure-backend jit` run reported
median elapsed time 14400 ns for `036_dense_isize_map_pure_batch.arcw` and
14500 ns for `037_dense_usize_map_pure_batch.arcw`, with
`pure_jit_calls_median = 128`, `pure_vm_calls_median = 0`,
`pure_arg_vec_allocations_median = 0`, and
`pure_flatten_bytes_copied_median = 0` in both fixtures. The latest dense
literal length/sum benches reported
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
Native `wgpu` runtime math calls keep a prepared buffer cache inside
`RuntimePureAccelerator`. Cache hits compare the current `f32` input bits
against the stored signature and skip `queue.write_buffer` when values are
unchanged. The comparison now reuses the stored signature allocation and scans
the current input slices directly, so the unchanged-input path no longer builds
a fresh `Vec<u32>` copy of every input element before deciding to skip upload.

The wgpu path is feature-gated by `math-wgpu` and keeps Windows DX12 enabled
alongside Vulkan, Metal, and GLES. The workspace Rust floor is raised to 1.96
so the latest wgpu stack can be used without pinning older graphics crates.
If a GPU adapter is unavailable, explicit `wgpu` measurement reports a
structured skip/error rather than silently changing the requested backend.
Portable wgpu compute shaders in this workspace are `f32` kernels; `f64`
matrix/tensor calls stay on scalar, glam 4x4, or ndarray CPU backends and
preserve `f64` storage across the runtime boundary. `Auto` never selects wgpu
for `f64`; explicit `wgpu` requests return a structured portability error for
those kernels. `f64` matmul uses the scalar row-major kernel for small general
matrices up to 64^3 multiply-add work items because local benches show the
ndarray setup cost can dominate there; larger `f64` matmul calls use ndarray.
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
just browser-webgpu-bench-isolate
just browser-webgpu-bench-stability
just browser-webgpu-bench-capacity-stability
just browser-webgpu-bench-submit-only
```

The exported browser function returns JSON with Auto dispatch, CPU Wasm,
WebGPU one-shot, prepared upload, prepared resident, prepared-capacity
resident, async resident, pipelined resident, and prepared-capacity pipelined
resident cases. Auto cases go through the typed browser math dispatcher and
record the policy-selected capacity when WebGPU is selected. The
`auto_resident_direct_pipelined` mode is an isolation probe: it still uses the
same Auto policy and prepared resident handle, but submits and reads through the
underlying WebGPU context directly to separate adapter-wrapper overhead from
browser warm-state and ordering effects. Prepared cases include an optional
typed `capacity` field separate from the actual `shape`, so the report can
distinguish exact resident storage from overprovisioned capacity storage without
recording host paths. The same report also includes typed
derived metrics: `effective_gflops`, `submit_median_share`, and
`readback_median_share`. These expose whether an observed browser result is
limited by compute, command submission, or readback rather than only reporting a
single median runtime. Each case records `round_index` and `mode_order_index`;
the `stability` section groups repeated op/shape/mode measurements and reports
median-of-medians, min/max, median absolute deviation, and max/min spread ratio.
The `browser-webgpu-bench-stability` preset runs the `256x256x256` matmul
isolation set for six rounds and rotates mode order each round, so browser
warm-state and fixed ordering effects are visible before browser-side Auto
thresholds are changed. The same report also includes typed
`recommendations` per operation/shape. A recommendation records the selected
mode, selected capacity, CPU median, selected median, speedup, and reason
(`web_gpu_faster`, `cpu_faster_or_equal`, `missing_cpu_baseline`, or
`no_measured_web_gpu_case`). When the same op/shape/mode appears in repeated
rounds, recommendation selection uses the mode's median-of-medians rather than
the fastest single case, so low outlier rounds do not drive backend policy
calibration. Auto cases are reported as policy observations and are not treated
as independent candidate modes for choosing the fastest backend.
Submit-only diagnostic modes are also excluded from recommendations and normal
`best_speedups`; the CLI summary reports them under `diagnostic_speedups`.
These modes submit batches of resident GPU work and defer correctness readback
until after measured samples, giving a lower-bound estimate for future
resident GPU flow chains where intermediate values remain on the GPU.
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
larger. Capacity-prepared pipelining remains available as an explicit policy and
bench mode, but default Auto no longer selects it until path-free repeated
browser evidence shows a shape range where it wins. The browser benchmark
harness uses the same typed capacity growth policy for overprovisioned prepared
cases, so measured `capacity` fields and runtime Auto decisions do not drift
through duplicated arithmetic.
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
Submitted work can also be read directly into a caller-owned `&mut [f32]` with
typed output metadata, avoiding the extra dense-value construction and `Vec`
copy when a browser flow or benchmark already owns the output buffer.
For repeated resident work, `prepare_resident` returns either the CPU policy
selection or a WebGPU prepared resident handle with inputs already uploaded.
Browser hosts can then call `submit_prepared` repeatedly without paying upload
cost for every submission.
This keeps browser GPU work outside the Sans I/O core while letting natural
browser-side math calls use the calibrated policy without duplicating threshold
logic in the player.
The browser harness also exposes submit-only diagnostic modes for prepared
resident work. They are not backend recommendations because they intentionally
exclude per-sample readback completion, but they show how much of the current
value-returning path is spent at the host boundary rather than in command
submission.

Latest path-free browser perf run after direct readback, resident prepared Auto
submission, and the direct Auto-resident isolation mode:

| Case | Mode | CPU median ms | Mode median ms | Speedup | Notes |
| --- | --- | ---: | ---: | ---: | --- |
| `matmul_f32_m256_k256_n256` | prepared resident pipelined | 6.58 | 0.41875 | 15.71x | 80.13 effective GFLOP/s, submit share 0.05, readback share 0.56 |
| `matmul_f32_m256_k256_n256` | auto pipelined direct readback | 6.58 | 0.91375 | 7.20x | policy-selected WebGPU, 36.72 effective GFLOP/s, submit share 0.13, readback share 0.39 |
| `matmul_f32_m256_k256_n256` | auto resident direct pipelined | 6.58 | 1.13625 | 5.79x | Auto policy plus direct context submit/readback, 29.53 effective GFLOP/s, submit share 0.03, readback share 0.30 |
| `matmul_f32_m256_k256_n256` | prepared capacity resident pipelined | 6.58 | 1.16 | 5.67x | overprovisioned capacity `512x512x512`, 28.93 effective GFLOP/s, submit share 0.03, readback share 0.28 |
| `matmul_f32_m256_k256_n256` | auto resident pipelined | 6.58 | 1.265 | 5.20x | WebGPU prepared resident handle, 26.53 effective GFLOP/s, submit share 0.02, readback share 0.27 |
| `matmul_f32_m256_k256_n256` | direct auto dispatch | 6.58 | 3.97 | 1.66x | value-returning path still waits for readback per call |
| `tensor_add_f32_len65536` | CPU Wasm | 0.055 | 0.055 | 1.00x | elementwise add remains CPU-preferred when readback is required |

The `auto_pipelined` and `auto_resident_pipelined` benchmark modes are policy
observations, not backend recommendation candidates. Manual prepared modes
remain the source for backend recommendations, while auto modes show the
overhead and scheduling behavior that natural browser-side calls see. The latest
metrics show that `auto_resident_pipelined` is not submit-bound or
readback-bound by itself. The new direct Auto-resident isolation mode also did
not outperform the wrapper path in the full perf run, which makes adapter
wrapper overhead an unlikely primary cause of the gap.

The dedicated isolate preset runs only the `256x256x256` matmul shape with CPU,
manual prepared resident, manual prepared capacity resident, Auto pipelined,
Auto resident, and direct Auto resident modes. In that smaller path-free run,
`auto_resident_pipelined` measured 0.92625 ms, manual prepared resident
pipelined measured 0.95125 ms, prepared capacity resident pipelined measured
1.00125 ms, direct Auto resident measured 1.0175 ms, and Auto pipelined
measured 1.06375 ms. This shows the large full-perf spread is dominated by
browser benchmark ordering or warm-state effects rather than the typed adapter
wrapper. The remaining tuning target is therefore policy calibration and bench
harness stability, not another compatibility path around the adapter.

The stability preset adds six repeated rounds of the same `256x256x256` matmul
isolation set and rotates mode order in each round. A local path-free run after
switching recommendations and browser Auto policy to median-of-medians evidence
recorded 36 measured cases, no skips, and no correctness failures. Median of
per-round medians and spread ratios were:

| Mode | Rounds | Median-of-medians ms | Min ms | Max ms | Spread ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| `web_gpu_prepared_resident_pipelined` | 6 | 0.61000 | 0.48250 | 0.86000 | 1.78x |
| `web_gpu_prepared_capacity_resident_pipelined` | 6 | 0.68750 | 0.53375 | 0.82000 | 1.54x |
| `auto_resident_pipelined` | 6 | 0.75500 | 0.43625 | 0.91625 | 2.10x |
| `auto_resident_direct_pipelined` | 6 | 0.91875 | 0.48000 | 1.02625 | 2.14x |
| `auto_pipelined` | 6 | 1.04750 | 0.83000 | 1.22875 | 1.48x |
| `cpu_wasm` | 6 | 6.63000 | 6.38000 | 7.50000 | 1.18x |

This confirms that browser WebGPU `256x256x256` matmul is consistently faster
than CPU Wasm. Recommendations now choose the measured backend by
median-of-medians, which selected exact prepared resident pipelining at 10.41x
CPU speedup before the policy update and 10.87x CPU speedup after the policy
update. The browser Auto policy now keeps `256x256x256` matmul on exact
prepared resident pipelining. Individual browser GPU samples can still vary by
more than 1.5x across rounds, so browser-side Auto policy changes should be
calibrated against repeated stability summaries, not one fixed-order perf run.

The capacity-stability preset runs the same repeated-order harness at
`512x512x512` to test whether default Auto should switch to overprovisioned
capacity storage. A local path-free run recorded 20 measured cases, no skips,
and no correctness failures:

| Mode | Rounds | Median-of-medians ms | Min ms | Max ms | Speedup vs CPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| `auto_resident_direct_pipelined` | 4 | 2.24750 | 1.87750 | 2.47750 | 26.90x |
| `auto_resident_pipelined` | 4 | 2.26375 | 2.00625 | 2.31875 | 26.71x |
| `web_gpu_prepared_resident_pipelined` | 4 | 2.41000 | 1.64625 | 2.74000 | 25.09x |
| `web_gpu_prepared_capacity_resident_pipelined` | 4 | 2.49125 | 1.72375 | 2.53375 | 24.27x |
| `cpu_wasm` | 4 | 60.46000 | 56.75000 | 64.91500 | 1.00x |

Among backend recommendation candidates, exact prepared resident pipelining
still beat capacity-prepared pipelining by median-of-medians at this shape. Auto
resident modes are reported as policy observations rather than backend
recommendation candidates; they show natural browser-call overhead but do not
choose the default backend. Default browser Auto therefore keeps using exact
prepared resident buffers for dense matmul and leaves capacity-prepared
pipelining as an explicit tuning mode until repeated measurements show it
consistently improves a larger shape or a different browser/GPU environment.

The submit-only preset measures the resident browser WebGPU lower bounds for
the same `512x512x512` matmul shape. The submit-only mode defers `map_async`
but still copies the output into a staging buffer for every submitted sample.
The dispatch-only mode submits resident compute work without per-sample
GPU-to-staging copies and performs one explicit readback after timing for
correctness. The chained dispatch-only modes use the typed resident `f32` graph
fragment API for prepared `matmul -> add(0)` and `matmul -> bias_add(0)` plans:
intermediate bind groups are created with the plans rather than rebuilt during
each measured submit, and only the final graph output is read back after
timing. The bias-add diagnostic stores only the bias vector and broadcasts it
in the second resident kernel, so it validates a graph fragment that does not
expand the bias into a full host-side add matrix. A local path-free run
recorded 28 measured cases, no skips, and no correctness failures:

| Mode | Rounds | Median-of-medians ms | Submit median ms | Readback median ms | Notes |
| --- | ---: | ---: | ---: | ---: | --- |
| `web_gpu_prepared_resident_dispatch_only_pipelined` | 4 | 0.01250 | 0.01000 | n/a | resident compute dispatch only; no per-sample readback copy/map; one explicit correctness readback after timing |
| `web_gpu_prepared_resident_chained_dispatch_only_pipelined` | 4 | 0.01375 | 0.01500 | n/a | typed resident graph fragment for `matmul -> add(0)`; no intermediate readback or per-submit bind-group rebuild; one final correctness readback after timing |
| `web_gpu_prepared_resident_matmul_bias_dispatch_only_pipelined` | 4 | 0.01500 | 0.01500 | n/a | typed resident graph fragment for `matmul -> bias_add(0)`; stores a last-axis bias vector instead of a full add matrix; one final correctness readback after timing |
| `web_gpu_prepared_resident_submit_only_pipelined` | 4 | 0.03750 | 0.03500 | n/a | measured samples submit compute plus staging copy; final map happens after timing |
| `web_gpu_prepared_resident_pipelined` | 4 | 2.15500 | 0.08500 | 1.50000 | value-returning benchmark path |
| `auto_resident_pipelined` | 4 | 2.22375 | 0.09500 | 1.76000 | natural Auto resident call shape |
| `cpu_wasm` | 4 | 56.87500 | n/a | n/a | CPU baseline |

The diagnostic results are not backend speedup claims. They show that command
submission without a per-sample readback copy is three orders of magnitude
below the value-returning path on this browser/GPU, while staging-copy submit is
still much cheaper than mapping the result. Browser-side optimization should
therefore keep intermediate tensor/matrix values resident across graph edges
and read back only at an explicit host boundary. The chained diagnostic shows
that adding a second resident kernel stays far below the value-returning path
when the intermediate matrix never leaves GPU storage and chain bindings are
prepared once with the graph fragment. The bias-add diagnostic shows the same
lower-bound behavior while avoiding a full host-side add matrix. The
graph-fragment wrappers did not move the diagnostics out of the resident
lower-bound band; the latest chained and bias-add runs measured roughly the
same submit-only floor as the direct prepared-plan path.

`arcweft-runtime-accelerator` also contains the first forward-only inference
graph API. The graph uses typed tensor IDs and validates shapes during graph
construction. The session executes through an `InferenceAdapter`, keeping the
typed graph and backend execution policy separated. The default adapter is
backed by `RuntimeMathAccelerator` for dense tensor matmul and deterministic CPU
kernels for non-matmul forward ops. `InferenceAdapter` also exposes a typed
`matmul_bias_add` hook. `InferenceSession` uses it for adjacent private
`matmul -> bias_add` pairs and leaves the unfused path in place when the matmul
output is observable or shared by another node. This gives native and browser
adapters a single boundary for resident `matmul -> bias_add` execution without
changing the graph's public op set. The default accelerated adapter now routes
that hook through `RuntimeMathAccelerator::matmul_bias_add_f32`; scalar
execution fuses the matmul and bias application loop, Glam and ndarray reuse
their existing matmul backend and then apply the last-axis bias on CPU, and
native wgpu now dispatches matmul plus bias-add in one command encoder without
reading the intermediate matmul result back to the host. `RuntimeMathStats`
records `fused_matmul_bias_add_calls` so bench JSON can distinguish fused
inference execution from separate matmul and bias-add calls.
`math_bench --op matmul-bias-add` now measures that fused path with the same
backend selection report used by the existing matmul, matrix add, and tensor add
probes. Native wgpu also supports prepared resident `matmul -> bias_add` through
`math_bench --op matmul-bias-add --reuse`, `--reuse-update-inputs`, and
`--reuse-capacity`; that path uploads lhs/rhs/bias into persistent buffers,
keeps the intermediate matmul output on the GPU, dispatches the bias pass in the
same command encoder, and performs only one final readback. Native prepared
`matmul` and `matmul -> bias_add` can now split resident GPU submission from
explicit output readback. `math_bench --submit-only` measures submit timing and
then performs one final correctness readback, so per-sample readback cost is no
longer mixed into the compute-submit lower bound. A small path-free smoke run
(`--backend all --op matmul-bias-add --size 16 --iterations 3 --warmup 1`)
reported measured scalar, ndarray, and Auto cases, skipped wgpu when
`math-wgpu` was disabled, and recorded `fused_matmul_bias_add_calls = 4` for
each measured backend. After native one-shot wgpu was switched to the same
fused matmul plus bias-add command sequence, a 512x512 run with
`--backend all --op matmul-bias-add --size 512 --iterations 3 --warmup 1`
measured wgpu at 2385500 ns median and Auto at 3201200 ns median, both with
`wgpu_calls = 4`, `fused_matmul_bias_add_calls = 4`, 28 GPU buffer creations,
one staging buffer creation, and three staging buffer reuse hits; ndarray
measured 4570600 ns median on the same run. A native wgpu prepared run at size 128 with
`--iterations 3 --warmup 1` reported measured medians of 191700 ns for exact
prepared reuse, 208500 ns for `--reuse-update-inputs`, and 257100 ns for
`--reuse-capacity`; each run recorded `wgpu_calls = 4`,
`fused_matmul_bias_add_calls = 4`, one staging buffer creation, three staging
buffer reuse hits, and four reused GPU dispatches. A 512x512 native wgpu
prepared submit-only run with
`--backend wgpu --op matmul-bias-add --size 512 --iterations 3 --warmup 1 --submit-only`
reported 275000 ns median, compared with 2577600 ns median for the same run
with `--reuse`. The submit-only run recorded `wgpu_calls = 4`,
`fused_matmul_bias_add_calls = 4`, `gpu_reused_dispatches = 4`, one final
staging buffer creation, zero staging buffer reuse hits, and only one
1048576-byte result download for correctness instead of downloading every
sample. Native prepared elementwise add has the same submit/readback split. Local
path-free runs with `just bench-math-matrix-add-submit-only` and
`just bench-math-tensor-add-submit-only` measured 4096x4096 resident GPU submit
lower bounds at 286200 ns and 320800 ns median respectively. Each run recorded
`wgpu_calls = 4`, `gpu_reused_dispatches = 4`, `gpu_buffer_reuse_hits = 16`,
one final staging buffer creation, zero staging-buffer reuse hits, and one
67108864-byte result download for correctness after timing; per-sample
readback was not included in the measured submit-only loop.

The current deterministic `f32` op set is:

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
`infer.max_pool2d_f32`, `infer.flatten_outer_f32`, `infer.matmul_f32`,
`infer.matmul_bias_add_f32`, and `infer.argmax_last_dim_f32`. These names are
not Core intrinsics and are not in the default prelude.
`arcweft-adapter-context` contributes the optional type-checking namespace,
while `RuntimePureAccelerator` resolves the named runtime calls through
`RuntimeExternalCallBackend` and uses the configured math backend for rank-2
tensor matmul. The fused external `infer.matmul_bias_add_f32` call uses the
native prepared wgpu matmul-bias cache when backend selection chooses wgpu, so
flow-side adapter calls can reuse resident buffers without requiring parser or
Core intrinsics. The default Rust-side `AcceleratedInferenceAdapter` also owns
that prepared cache, so `InferenceSession` graph fusion for private
`matmul -> bias_add` pairs can reuse the same resident buffers across repeated
session runs. The standalone `math_bench` example now also accepts
`--op inference-matmul-bias-add`; without `--reuse` it measures cold
`InferenceSession` construction plus execution for each sample, while
`--reuse` measures repeated execution through one session and the adapter-owned
prepared GPU cache. `InferenceSession::run_borrowed` is now the implementation
path for graph execution: graph constants and supplied inputs stay borrowed as
per-run values until an operation produces an owned tensor, so adapter
execution does not clone those tensors again inside the session. The owned
`run` API delegates to that borrowed path after collecting its input tensors.

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
just bench-math-matmul-bias
just bench-math-matmul-bias-wgpu
just bench-math-matmul-bias-reuse
just bench-math-matmul-bias-reuse-update
just bench-math-matmul-bias-reuse-capacity
just bench-math-inference-matmul-bias-reuse
just bench-math-inference-matmul-bias-reuse-update
just bench-math-f64
just bench-math-matrix-add
just bench-math-tensor-add
just bench-math-matrix-add-submit-only
just bench-math-tensor-add-submit-only
just bench-024
just bench-024-wgpu-auto
just bench-025
just bench-026
just bench-027
just bench-028
just bench-035
arcw bench tests/fixtures/arcw/spec_should_pass/bench/027_matrix_matmul_f64.arcw --math-backend ndarray --value lhs=matrix/f64/2x2:1.5,2,3.25,4.5 --value rhs=matrix/f64/2x2:5,6.5,7,8.25 --json
arcw bench tests/fixtures/arcw/spec_should_pass/bench/028_tensor_add_f64.arcw --math-backend ndarray --value lhs=tensor/f64/2x2:1.5,2.25,3.75,4.5 --value rhs=tensor/f64/2x2:5,6.25,7.5,8.75 --json
arcw bench tests/fixtures/arcw/spec_should_pass/bench/035_matrix_add_f64.arcw --math-backend ndarray --value lhs=matrix/f64/2x2:1.5,2.25,3.75,4.5 --value rhs=matrix/f64/2x2:5,6.25,7.5,8.75 --json
cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul --size 512 --iterations 3 --warmup 1 --reuse
just bench-math-matmul-reuse-update
just bench-math-matrix-add-reuse
just bench-math-matrix-add-reuse-update
just bench-math-tensor-add-reuse
just bench-math-tensor-add-reuse-update
```

Standalone `math_bench` JSON includes `build_mode` and a path-free
`host_system` summary so debug-assertion runs and different CPU/thread limits
are not confused with optimized performance evidence. The representative
standalone math rows below are from the release just recipes and report
`build_mode = "optimized"`.

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
| flow 2x2 matrix_add_f64 | ndarray | measured | 1900 | `--value` matrix input, `math_calls_median = 1`, `math_accelerated_calls_median = 1`, `bytes_borrowed = 64`, `result_bytes_copied = 32` |
| 64x64 matmul_f64 | scalar | measured | 39200 | standalone `math_bench`, row-major f64 baseline |
| 64x64 matmul_f64 | ndarray | measured | 52200 | standalone `math_bench`, CPU matrix backend without narrowing |
| 64x64 matmul_f64 | auto | measured | 46000 | selected scalar with `last_auto_reason = matmul_scalar_small_work`; explicit wgpu was skipped as portable f64 unsupported |
| 128x128 matmul_f64 | auto | measured | 4844400 | selected ndarray with `last_auto_reason = matmul_ndarray_cpu_default`, crossing the small-matrix scalar threshold |
| 1024x1024 matrix_add_f64 | scalar | measured | 7077100 | standalone `math_bench`, f64 elementwise baseline |
| 1024x1024 matrix_add_f64 | ndarray | measured | 7591700 | standalone `math_bench`, borrowed f64 inputs and owned f64 output |
| 1024x1024 matrix_add_f64 | auto | measured | 6188600 | selected ndarray with `last_auto_reason = elementwise_ndarray_cpu_default`; explicit wgpu was skipped as portable f64 unsupported |
| 1024x1024 tensor_add_f64 | scalar | measured | 6146700 | standalone `math_bench`, f64 tensor baseline |
| 1024x1024 tensor_add_f64 | ndarray | measured | 6096600 | standalone `math_bench`, dynamic-view f64 add without narrowing |
| 1024x1024 tensor_add_f64 | auto | measured | 6101100 | selected ndarray with `last_auto_reason = elementwise_ndarray_cpu_default`; explicit wgpu was skipped as portable f64 unsupported |
| 64x64 matmul_f32 | scalar | measured | 21300 | row-major baseline |
| 64x64 matmul_f32 | ndarray | measured | 26700 | general CPU matrix backend |
| 64x64 matmul_f32 | auto | measured | 24200 | selected ndarray without wgpu feature |
| 128x128 matmul_f32 | ndarray | measured | 98500 | CPU backend with wgpu feature enabled |
| 128x128 matmul_f32 | wgpu | measured | 217800 | upload/download dominates |
| 128x128 matmul_f32 | wgpu prepared | measured | 135800 | 4 buffer creations, 16 buffer reuse hits, 3 staging reuse hits |
| 128x128 matmul_f32 | wgpu prepared update | measured | 283000 | `--reuse-update-inputs`, one initial buffer allocation, four upload+dispatch passes, `gpu_buffer_reuse_hits = 28` |
| 128x128 matmul_f32 | wgpu prepared capacity | measured | 204500 | `--reuse-capacity`, capacity 256, one initial upload, five measured dispatches, `gpu_buffer_reuse_hits = 27` |
| 128x128 inference matmul_bias_add_f32 | wgpu cold session | measured | 657603500 | `--op inference-matmul-bias-add`, no `--reuse`, each sample creates a session/adapter and prepares GPU buffers |
| 128x128 inference matmul_bias_add_f32 | wgpu reused session | measured | 436100 | `--reuse`, one session and prepared adapter cache, `gpu_buffer_creations = 7`, `gpu_buffer_reuse_hits = 47`, `gpu_reused_dispatches = 6` |
| 128x128 inference matmul_bias_add_f32 | wgpu reused session input update | measured | 348800 | `--reuse-update-inputs`, same prepared buffers with repeated uploads, `gpu_buffer_creations = 7`, `gpu_buffer_reuse_hits = 72`, `gpu_reused_dispatches = 6` |
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
| 4096x4096 matrix_add_f32 | wgpu prepared submit-only | measured | 286200 | `--submit-only`, resident compute submit lower bound; one final 64 MiB correctness download after timing |
| 4096x4096 tensor_add_f32 | wgpu prepared submit-only | measured | 320800 | `--submit-only`, resident compute submit lower bound; one final 64 MiB correctness download after timing |
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
allocation. Native prepared matrix/tensor add can also submit without immediate
readback and read the resident output explicitly later, matching the matmul
submit/readback boundary and keeping readback cost out of submit lower-bound
measurements. The standalone `math_bench` example exposes this with
`--reuse-capacity` and `--submit-only`, and the Justfile provides
`bench-math-*-reuse-capacity` and `bench-math-*-submit-only` recipes so these
paths can be timed without recording host paths. Auto
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
