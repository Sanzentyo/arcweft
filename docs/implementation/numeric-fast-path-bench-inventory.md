# Numeric Fast Path Bench Inventory

This inventory records the checked-in benchmark and verification surface for
exact-width scalar numeric fast paths. Paths and commands are relative to the
workspace root; generated reports should keep the JSON `source` field to a file
name or relative fixture label and must not record host absolute paths.

## Bench Fixtures

| fixture | primary path | numeric coverage | expected hot boundary |
| --- | --- | --- | --- |
| `010_dense_i32_sum.arcw` | VM dense reduction | `i32` storage and `sum()` | dense storage, no pure helper calls |
| `011_dense_u64_sum.arcw` | VM dense reduction | `u64` storage and `sum()` | dense storage, no pure helper calls |
| `012_dense_integer_widths_sum.arcw` | VM dense reduction | `i8`, `i16`, `i32`, `u8`, `u16`, `u32`, `u64` | dense storage, no flattening |
| `013_dense_scalar_len.arcw` | VM dense length | unit, bool, char, duration, `u8` | `RuntimeSeq::len()` without materialization |
| `014_dense_textual_scalar_len.arcw` | VM dense length | string, entity refs, typed float scalar storage | `RuntimeSeq::len()` without materialization |
| `015_dense_wide_numeric_len.arcw` | VM dense length | `i128`, `u128`, `isize`, `usize` | `RuntimeSeq::len()` without materialization |
| `016_dense_i32_map_pure_batch.arcw` | pure helper batch | `i32` input/output | native JIT when requested, AOT/VM selectable |
| `017_dense_u32_map_pure_batch.arcw` | pure helper batch | `u32` input/output | native JIT when requested, AOT/VM selectable |
| `018_dense_u64_map_pure_batch.arcw` | pure helper batch | `u64` input/output | native JIT when requested, AOT/VM selectable |
| `019_dense_i128_map_pure_batch.arcw` | pure helper batch | `i128` input/output | native batch JIT when requested, AOT/VM selectable |
| `020_dense_u128_map_pure_batch.arcw` | pure helper batch | `u128` input/output | native batch JIT when requested, AOT/VM selectable |
| `022_dense_f32_map_pure_batch.arcw` | pure helper batch | `f32` input/output | native JIT when requested, AOT/VM selectable |
| `023_dense_f64_map_pure_batch.arcw` | pure helper batch | `f64` input/output | native JIT when requested, AOT/VM selectable |
| `029_dense_i8_map_pure_batch.arcw` | pure helper batch | `i8` input/output | native JIT when requested, AOT/VM selectable |
| `030_dense_i16_map_pure_batch.arcw` | pure helper batch | `i16` input/output | native JIT when requested, AOT/VM selectable |
| `031_dense_u8_map_pure_batch.arcw` | pure helper batch | `u8` input/output | native JIT when requested, AOT/VM selectable |
| `032_dense_u16_map_pure_batch.arcw` | pure helper batch | `u16` input/output | native JIT when requested, AOT/VM selectable |
| `033_mixed_for_iter_pure_jit.arcw` | mixed scalar and batch pure helper calls | `i32`, `u32`, and `f32` across `.map` and `for` | exact-width native JIT for scalar and flat batch calls |

## Current Bench Commands

VM dense sequence coverage:

```bash
just bench-numeric-vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/010_dense_i32_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/011_dense_u64_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/012_dense_integer_widths_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/013_dense_scalar_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/014_dense_textual_scalar_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/015_dense_wide_numeric_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
```

VM pure helper coverage:

```bash
just bench-numeric-pure-vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/019_dense_i128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/020_dense_u128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/029_dense_i8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/030_dense_i16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/031_dense_u8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/032_dense_u16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
```

AOT exact-width pure helper coverage:

```bash
just bench-numeric-aot
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/019_dense_i128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/020_dense_u128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/029_dense_i8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/030_dense_i16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/031_dense_u8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/032_dense_u16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
```

Native JIT exact-width pure helper coverage:

```bash
just bench-numeric-jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/029_dense_i8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/030_dense_i16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/031_dense_u8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/032_dense_u16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw --json --iterations 2 --warmup 1 --samples 1 --steps 128 --max-ops 128 --pure-backend jit
```

Per-fixture convenience targets now also exist for `just bench-016` through
`just bench-020`, `just bench-022`, `just bench-023`, and `just bench-029`
through `just bench-033`. Those targets request `--pure-backend jit`.

## Verification Inventory

The focused Rust regression surface is in `crates/arcweft-cli/tests/check.rs`.
Relevant tests assert both numeric counters and absence of host absolute paths
in JSON output:

| test | verifies |
| --- | --- |
| `bench_json_measures_checked_in_dense_integer_widths_fixture` | VM dense integer width validation and no argument Vec allocation |
| `bench_json_measures_checked_in_dense_scalar_len_fixture` | dense scalar length path and relative source label |
| `bench_json_measures_checked_in_dense_textual_scalar_len_fixture` | dense string/float/entity length path and relative source label |
| `bench_json_measures_checked_in_dense_wide_numeric_len_fixture` | wide integer and target-sized dense length path |
| `bench_json_measures_checked_in_dense_i32_map_pure_batch_fixture` | exact `i32` JIT batch path, borrowed bytes, no result copy |
| `bench_json_measures_checked_in_dense_f32_map_pure_batch_fixture_with_auto_jit` | exact `f32` auto-JIT batch path and typed result copy accounting |
| `bench_json_measures_checked_in_dense_f64_map_pure_batch_fixture_with_auto_jit` | exact `f64` auto-JIT batch path and typed result copy accounting |
| `bench_json_measures_checked_in_dense_u32_map_pure_batch_fixture` | exact `u32` JIT batch path, unsigned ABI, borrowed bytes, no result copy |
| `bench_json_measures_checked_in_dense_u64_map_pure_batch_fixture` | exact `u64` JIT batch path, unsigned ABI, borrowed bytes, no result copy |
| `bench_json_measures_checked_in_small_dense_integer_map_pure_batch_fixtures` | exact `i8`, `i16`, `u8`, and `u16` JIT batch paths, borrowed bytes, no result copy |
| `bench_json_measures_checked_in_dense_i128_map_pure_batch_fixture` | exact `i128` native batch JIT path, borrowed bytes, no result copy |
| `bench_json_measures_checked_in_dense_u128_map_pure_batch_fixture` | exact `u128` native batch JIT path, borrowed bytes, no result copy |
| `bench_json_measures_checked_in_mixed_for_iter_pure_jit_fixture` | mixed `i32`, `u32`, and `f32` `.map`/`for` pure calls use JIT with zero VM fallback and no argument Vec allocation |

Useful check commands:

```bash
cargo test -p arcweft-cli --test check bench_json_measures_checked_in_dense -- --nocapture
cargo test -p arcweft-cli checked_in_docs_and_samples_do_not_record_host_absolute_paths --test regression_harness
just scan-absolute-paths
```

## Current Gaps

Native Cranelift JIT exact-width pure helper coverage is present for `i8`,
`i16`, `i32`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128`, `f32`, and `f64` in
these checked-in fixtures. The `i128` and `u128` JIT entries are batch-only and
use pointer-based flat buffers at the native boundary; scalar by-value
`i128`/`u128` calls remain on VM/AOT paths to avoid target-specific wide-integer
ABI assumptions. Within that batch path, Cranelift lowering handles full-width
wide-integer literals and captured constants by building the `i128` value from
two 64-bit halves with `iconcat`. The VM dense fixtures cover exact-width
storage, length, and integer reduction, while the pure helper fixtures cover
batched helper execution and backend selection counters.

The Auto backend now treats every native JIT entry width as a deferred JIT
candidate after the initial typed AOT plan. Large flat batches can promote
`i8`, `i16`, `i32`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128`, `f32`, and `f64`
helpers to native JIT without routing through the VM fallback.

Generic exact-integer scalar calls now use a typed borrowed slice view from
`RuntimeExactInteger` to recognize width-specific JIT cache entries. This keeps
`call_exact_int_slice::<T>` aligned with the dedicated `call_u32_slice` /
`call_u64_slice` entry points without string matching, downcasts, or VM
fallback when a scalar native JIT entry exists for the width.

The mixed `033_mixed_for_iter_pure_jit.arcw` fixture guards the scalar/batch
boundary: exact-width helper calls inside both `.map` and `for` loops stay on
typed borrowed slices and reach native JIT for `i32`, `u32`, and `f32`. A
path-free local run with two measured iterations reported `jit_successes = 3`,
`pure_calls_median = 40`, `pure_batch_calls_median = 3`,
`pure_jit_calls_median = 40`, `pure_vm_calls_median = 0`,
`pure_fallbacks_median = 0`, `pure_arg_vec_allocations_median = 0`,
`pure_arg_bytes_borrowed_median = 320`, and
`pure_result_bytes_copied_median = 96`.
