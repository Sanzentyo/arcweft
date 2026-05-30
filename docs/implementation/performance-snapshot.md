# Performance Snapshot

This file records path-free local measurements for optimization comparisons.
Values are machine- and build-cache-dependent; use them as trend samples, not
portable guarantees.

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

| fixture | executor | iterations | median elapsed ns | task requests | task events in | system info ops | scheduler submitted | scheduler max in-flight | parallel marker tasks |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 004_system_info_threads.arcw | bytecode_vm | 1 | 460900 | 6 | 6 | 3 | 6 | 6 | 0 |

Checked-in map pure JIT bench:

| fixture | executor | pure backend | iterations | median elapsed ns | pure calls | batch calls | batch items | arg vec allocs | borrowed arg bytes |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 002_map_pure_jit.arcw | bytecode_vm | jit | 8 | 29000 | 16 | 1 | 16 | 0 | 256 |

Checked-in inferred pure JIT bench:

| fixture | executor | pure backend | inferred helpers | jit helpers | iterations | median elapsed ns | pure calls | batch calls | borrowed arg bytes | result bytes copied |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 005_inferred_pure_jit.arcw | bytecode_vm | jit | 1 | 1 | 4 | 13500 | 4 | 1 | 64 | 32 |

Checked-in linear AOT executor bench:

| fixture | executor | iterations | median elapsed ns | executed ops | AOT fast-path ops | line effects | pure calls |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 006_linear_aot.arcw | aot | 4 | 4800 | 3 | 3 | 1 | 0 |

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
