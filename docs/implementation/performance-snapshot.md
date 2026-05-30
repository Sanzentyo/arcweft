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
| 001_thread_scheduling.arcw | bytecode_vm | 10 | 64 | 26600 | 19 | 1400 | 3 | 3 |

The JSON outputs above reported no source file paths and included only command
argv tokens, host core/thread counts, timing counters, and deterministic
accumulators.
