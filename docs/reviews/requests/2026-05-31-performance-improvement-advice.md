# Arcweft Performance Improvement Advice Request

## Request

Please review Arcweft's current compiler/runtime performance path and propose
concrete optimization steps. Focus on changes that improve measured throughput
without adding compatibility shims, deprecated spellings, removed whitespace
command DSL support, or unstable Rust features.

We want advice that can be turned into small, verifiable Rust commits.

## Project Context

Arcweft is a Rust narrative engine with these relevant layers:

- `arcweft-lang-syntax`: CST, parser, surface AST, expression/type/pattern parsing.
- `arcweft-lang-hir`: HIR lowering.
- `arcweft-lang-sema`: semantic type checking, borrow/lifetime checks.
- `arcweft-runtime-plan`: runtime-plan lowering and pure-helper discovery.
- `arcweft-core`: Sans I/O bytecode VM, runtime state, flow execution.
- `arcweft-runtime-accelerator`: VM/AOT/JIT pure-function backend dispatch.
- `arcweft-lang-jit-cranelift`: Cranelift JIT adapter for pure integer helpers.
- `arcweft-runtime-scheduler`: Sans I/O scheduler for host task batches.
- `arcweft-cli`: CLI, native host task bridge, bench/profile/test surfaces.

The current development rule is destructive internal cleanup, not backward
compatibility preservation. Removed syntax should fail through structured
diagnostics; do not add parser/tooling branches that silently accept obsolete
syntax.

## Constraints

- Keep `arcweft-core` Sans I/O.
- Keep parser/syntax work in `arcweft-lang-syntax`; HIR/sema/runtime lowering
  must remain in their own crates.
- Prefer typed APIs and explicit modules over stringly typed APIs or broad
  root re-exports.
- Do not use `unsafe`, unstable Rust, `#[allow(...)]`, deprecated APIs,
  compatibility aliases, compatibility modules, wrapper APIs, migration shims,
  or compatibility shims.
- Do not record host absolute paths in docs, logs, snapshots, or bench output.
- Use path-free fixture paths and checked-in benches.
- Preserve flat fence authoring sugar; it is not legacy syntax.

## Current Reproduction Commands

Use these from the repository root:

```bash
just verify
just bench-009
just bench-thread
just bench-system
```

Equivalent explicit commands:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
cargo test -p arcweft-cli --test regression_harness
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit --pure-workers 4 --pure-batch-min-len 64
```

## Current Measurements

Recent path-free snapshot:

| Fixture | Area | Median |
| --- | --- | ---: |
| `001_thread_scheduling.arcw` | bytecode VM thread scheduling elapsed | 16900 ns |
| `003_for_pure_jit.arcw` | scalar for-loop pure JIT elapsed | 47900 ns |
| `004_system_info_threads.arcw` | threaded native system-info elapsed | 1227700 ns |
| `007_branching_iter_pure_jit.arcw` | mixed map/for pure JIT elapsed | 106200 ns |
| `009_nonuniform_map_pure_batch.arcw` | nonuniform map pure batch elapsed | 16300 ns |
| `009_nonuniform_map_pure_batch.arcw` | parse phase | 3937100 ns |
| `009_nonuniform_map_pure_batch.arcw` | typecheck phase | 247700 ns |
| `009_nonuniform_map_pure_batch.arcw` | runtime plan lowering | 303600 ns |

Important counters:

| Fixture | Counter | Value |
| --- | --- | ---: |
| `003_for_pure_jit.arcw` | pure calls | 16 |
| `003_for_pure_jit.arcw` | arg vec allocs | 0 |
| `003_for_pure_jit.arcw` | copied arg bytes | 0 |
| `003_for_pure_jit.arcw` | borrowed arg bytes | 256 |
| `007_branching_iter_pure_jit.arcw` | pure calls | 32 |
| `007_branching_iter_pure_jit.arcw` | batch items | 16 |
| `009_nonuniform_map_pure_batch.arcw` | pure calls | 128 |
| `009_nonuniform_map_pure_batch.arcw` | batch calls | 1 |
| `009_nonuniform_map_pure_batch.arcw` | batch items | 128 |
| `009_nonuniform_map_pure_batch.arcw` | arg vec allocs | 0 |
| `009_nonuniform_map_pure_batch.arcw` | borrowed arg bytes | 2048 |
| `009_nonuniform_map_pure_batch.arcw` | typecheck expressions | 16 |
| `009_nonuniform_map_pure_batch.arcw` | type judgments | 21 |

## Recent Optimizations Already Landed

- Pure scalar calls use borrowed slice paths instead of per-call argument Vec
  allocation.
- JIT flat batch sum path avoids output copy for fused map/sum patterns.
- Type checking has a fast path for typed numeric sequence literals, avoiding
  recursive type checking of every numeric literal in large sequences.
- Expression lexing reserves token capacity from source length.
- Flat literal bracket sequences bypass per-item Pratt recursion while
  preserving array repeat and mixed-expression parsing.
- Bracket literal parsing avoids the old pre-parse CST scan for expressions
  that start with `[`, while preserving bracket-postfix rescue for dialogue
  content-like payloads such as `speaker.say()[...]`.
- Native task scheduling and host system summaries are path-free and covered by
  regression harness tests.

## Areas Where Advice Is Needed

1. Parser phase cost

   `009_nonuniform_map_pure_batch.arcw` still spends several milliseconds in
   parsing. Please identify the most likely remaining hot spots in:

   - source line splitting and nested `Parser` construction,
   - repeated CST lexing in top-level punctuation helpers,
   - expression token allocation and literal string cloning,
   - function/flow body block extraction,
   - raw/recovery handling that may re-scan large fragments.

2. Typecheck and borrow-check cost

   Numeric sequence typecheck is now reduced, but please look for:

   - avoidable type clone churn,
   - judgment allocation overhead,
   - repeated compatibility checks that could use typed summaries,
   - borrow state snapshots or merges that can use sharing or smaller deltas.

3. Runtime plan lowering

   Fused map/sum and pure helper rewriting are implemented. Please identify
   further low-risk improvements for:

   - avoiding repeated expression walks,
   - retaining typed sequence summaries from parser/HIR/sema,
   - lowering map/for/iter into VM-friendly loop shapes without extra runtime
     allocations,
   - improving pure-helper candidate discovery without speculative broad APIs.

4. VM / JIT / AOT boundary

   The user-facing goal is that naturally written pure functions are
   automatically accelerated without visible configuration or boundary
   overhead. Please advise on:

   - when to choose VM vs AOT vs JIT vs batched JIT,
   - how to avoid boundary copies for arrays, slices, and repeated scalar calls,
   - where typed memory views should live without breaking `arcweft-core`
     Sans I/O,
   - how to measure and reduce JIT compile overhead separately from steady
     runtime execution,
   - when multithreaded batching pays off or hurts.

5. Thread scheduling and host task bridge

   Please review whether scheduling counters expose enough information to
   distinguish:

   - scheduler sort work,
   - marker-only thread tasks,
   - real host I/O tasks,
   - worker fanout,
   - in-flight task pressure,
   - host completion normalization cost.

## Expected Answer Format

Please provide findings in priority order:

1. Finding title.
2. Affected crates/modules.
3. Why it is likely a bottleneck.
4. Concrete implementation plan.
5. Tests or benchmarks to add/update.
6. Expected risk and expected performance impact.

For parser/compiler recommendations, prefer root-cause refactors over
compatibility layers. For runtime/JIT recommendations, include the data that
should be measured before and after the change.

## Non-Goals

- Do not recommend preserving removed whitespace command DSL.
- Do not recommend compatibility shims or deprecated aliases.
- Do not recommend moving host I/O into `arcweft-core`.
- Do not recommend recording absolute paths in bench or docs output.
- Do not recommend unsafe code unless there is no safe alternative and the
  isolated boundary is explicitly justified.
