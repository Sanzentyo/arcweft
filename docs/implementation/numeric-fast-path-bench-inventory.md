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
| `002_map_pure_jit.arcw` | pure helper batch | `i64` input/output through `.map` | native JIT when requested |
| `003_for_pure_jit.arcw` | scalar pure helper calls | `i64` input/output through `for` | native JIT when requested, no argument Vec allocation |
| `005_inferred_pure_jit.arcw` | inferred pure helper batch | inferred `i64` input/output | native JIT when requested |
| `007_branching_iter_pure_jit.arcw` | mixed scalar and batch pure helper calls | branching `i64` input/output | native JIT when requested |
| `008_large_map_pure_batch.arcw` | large repeated-row pure helper batch | `i64` input/output | Auto promotes repeated-row map+sum from typed AOT to native JIT |
| `009_nonuniform_map_pure_batch.arcw` | nonuniform pure helper batch | `i64` input/output | Auto promotes from typed AOT to native JIT |
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
| `038_wide_for_pure_jit.arcw` | scalar pure helper calls | `i128` and `u128` across `for` | native JIT via one-row pointer batches, no by-value wide integer ABI |
| `039_hot_for_pure_auto_jit.arcw` | hot scalar pure helper calls | `i128` and `f32` across `for` | Auto starts on typed AOT, promotes hot scalar loops to native JIT |
| `040_mixed_width_for_iter_pure_jit.arcw` | mixed scalar and batch pure helper calls | `i16`, `u16`, `isize`, `usize`, and `f64` across `.map` and `for` | exact-width native JIT for small, target-sized, and floating scalar/flat batch calls |

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
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/002_map_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/005_inferred_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/007_branching_iter_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 128 --max-ops 128 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/008_large_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend auto
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend auto
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
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/038_wide_for_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/039_hot_for_pure_auto_jit.arcw --json --iterations 4 --warmup 1 --samples 3 --steps 512 --max-ops 512 --pure-backend auto
cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw --json --iterations 2 --warmup 1 --samples 1 --steps 128 --max-ops 128 --pure-backend jit
```

Per-fixture convenience targets now also exist for the i64 helper fixtures
(`just bench-002`, `just bench-003`, `just bench-005`, `just bench-007`,
`just bench-008`, and `just bench-009`), `just bench-016` through
`just bench-020`, `just bench-022`, `just bench-023`, and `just bench-029`
through `just bench-033`, plus `just bench-038` through `just bench-040`.
Those targets request `--pure-backend jit` where the fixture directly selects
native JIT, while the large i64 Auto fixtures use `--pure-backend auto` to
exercise deferred JIT promotion. `bench-039` also uses Auto to exercise hot
scalar-loop promotion.

The path-free toolchain profile targets `just toolchain-profile-width-fast-path-benches`
and `just toolchain-profile-width-object-benches` run the mixed-width `bench-033`
and `bench-040` fixtures under JIT, AOT, VM, and AOT object artifact emission.
Their compact `arcweft_bench` summaries keep exact-width runtime call counts,
fallbacks, argument-vector allocations, borrowed bytes, compile counters, and
object bytes in the same JSON schema used for workspace timing trends.

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
| `bench_json_measures_checked_in_dense_target_size_integer_map_pure_batch_fixtures` | exact `isize` and `usize` native JIT batch paths, borrowed bytes, no result copy |
| `bench_json_measures_checked_in_mixed_for_iter_pure_jit_fixture` | mixed `i32`, `u32`, and `f32` `.map`/`for` pure calls use JIT with zero VM fallback and no argument Vec allocation |
| `bench_json_measures_checked_in_mixed_width_for_iter_pure_jit_fixture` | mixed `i16`, `u16`, `isize`, `usize`, and `f64` `.map`/`for` pure calls use exact-width JIT with zero VM fallback and no argument Vec allocation |
| `bench_json_measures_checked_in_wide_for_pure_jit_fixture` | scalar `i128` and `u128` `for` pure calls use native JIT through one-row pointer batches with zero VM fallback and no argument Vec allocation |
| `bench_json_measures_checked_in_hot_for_pure_auto_jit_fixture` | hot scalar `i128` and `f32` `for` pure calls promote from Auto AOT to native JIT before measured iterations |
| `generic_exact_int_scalar_call_recognizes_width_specific_jit_entry` | generic `call_exact_int_slice<T>` recognizes every non-`i64` exact integer width through typed runtime scalar metadata, with native JIT calls and zero VM fallback |
| `bench_json_measures_profile_inference_matmul_bias_adapter_fixture` | profile-selected `infer.matmul_bias_add_f32` lowers to the external math boundary for both scalar and explicit ndarray backends, with fused-call counters and no absolute paths |

Useful check commands:

```bash
cargo test -p arcweft-cli --test check bench_json_measures_checked_in_dense -- --nocapture
cargo test -p arcweft-cli checked_in_docs_and_samples_do_not_record_host_absolute_paths --test regression_harness
just scan-absolute-paths
```

## Current Gaps

Native Cranelift JIT exact-width pure helper coverage is present for `i8`,
`i16`, `i32`, `i64`, `i128`, `isize`, `u8`, `u16`, `u32`, `u64`, `u128`,
`usize`, `f32`, and `f64` in these checked-in fixtures. The `i128` and `u128`
JIT entries use pointer-based flat buffers at the native boundary; scalar calls
are lowered to one-row batches so by-value wide integers never cross the native
ABI. Cranelift lowering handles full-width wide-integer literals and captured
constants by building the `i128` value from two 64-bit halves with `iconcat`.
Target-sized `isize` and `usize`
helpers use `repr(transparent)` storage newtypes and route native JIT scalar and
flat-batch calls through the fixed `i64`/`u64` Cranelift lowering without
materializing `RuntimeValue` elements or copying into widened temporary buffers.
The VM dense fixtures cover exact-width storage, length, and integer reduction,
while the pure helper fixtures cover batched helper execution and backend
selection counters.

The Auto backend now treats every native JIT entry width as a deferred JIT
candidate after the initial typed AOT plan. Large flat batches can promote
`i8`, `i16`, `i32`, `i64`, `i128`, `isize`, `u8`, `u16`, `u32`, `u64`,
`u128`, `usize`, `f32`, and `f64` helpers to native JIT without routing through
the VM fallback. Scalar calls also accumulate per-helper work units and promote
from Auto AOT to native JIT once the hot scalar loop crosses the same work
threshold family, preserving cold scalar startup while allowing natural `for`
loops to become native without requiring an explicit `--pure-backend jit`.

Generic exact-integer scalar calls now use a typed borrowed slice view from
`RuntimeExactInteger` to recognize width-specific JIT cache entries for `i8`,
`i16`, `i32`, `i128`, `isize`, `u8`, `u16`, `u32`, `u64`, `u128`, and `usize`.
The `i64` case remains on the dedicated i64 fast path rather than the generic
exact-int trait. This keeps `call_exact_int_slice::<T>` aligned with the
dedicated `call_u32_slice` / `call_u64_slice` entry points without string
matching, downcasts, or VM fallback when a scalar native JIT entry exists for
the width.

A local path-free measurement of the target-sized fixtures with
`--pure-backend jit` and fifteen measured iterations reported `036_dense_isize`
at `elapsed_ns.median = 14400` and `037_dense_usize` at
`elapsed_ns.median = 14500`; the same run shape for `018_dense_u64` reported
`elapsed_ns.median = 15400`. All three reported `pure_jit_calls_median = 128`,
`pure_vm_calls_median = 0`, `pure_fallbacks_median = 0`,
`pure_arg_vec_allocations_median = 0`, `pure_flatten_bytes_copied_median = 0`,
and `pure_result_bytes_copied_median = 0`.

The mixed `033_mixed_for_iter_pure_jit.arcw` fixture guards the scalar/batch
boundary: exact-width helper calls inside both `.map` and `for` loops stay on
typed borrowed slices and reach native JIT for `i32`, `u32`, and `f32`. A
path-free local run with two measured iterations reported `jit_successes = 3`,
`pure_calls_median = 40`, `pure_batch_calls_median = 3`,
`pure_jit_calls_median = 40`, `pure_vm_calls_median = 0`,
`pure_fallbacks_median = 0`, `pure_arg_vec_allocations_median = 0`,
`pure_arg_bytes_borrowed_median = 320`, and
`pure_result_bytes_copied_median = 96`.

The mixed-width `040_mixed_width_for_iter_pure_jit.arcw` fixture extends that
same language-level boundary to `i16`, `u16`, `isize`, `usize`, and `f64`. A
path-free local run with two measured iterations reported `jit_successes = 5`,
`pure_calls_median = 80`, `pure_batch_calls_median = 5`,
`pure_jit_calls_median = 80`, `pure_vm_calls_median = 0`,
`pure_fallbacks_median = 0`, `pure_arg_vec_allocations_median = 0`,
`pure_flat_batch_bytes_borrowed_median = 448`,
`pure_arg_bytes_borrowed_median = 896`, and
`pure_result_bytes_copied_median = 224` on the local 64-bit target.

The wide scalar `038_wide_for_pure_jit.arcw` fixture confirms that `for` loop
calls over `i128` and `u128` now reach native JIT without by-value wide integer
ABI calls. A path-free local run with eight measured iterations reported
`jit_successes = 2`, `pure_calls_median = 16`,
`pure_batch_calls_median = 0`, `pure_jit_calls_median = 16`,
`pure_vm_calls_median = 0`, `pure_fallbacks_median = 0`,
`pure_arg_vec_allocations_median = 0`, `pure_arg_bytes_borrowed_median = 512`,
`pure_result_bytes_copied_median = 0`, and `elapsed_ns.median = 83800`.

The hot scalar Auto `039_hot_for_pure_auto_jit.arcw` fixture confirms that
ordinary `for` loops over `i128` and `f32` promote from deferred Auto AOT to
native JIT after warmup. A path-free local run with one warmup and four measured
iterations reported `auto_jit_deferred = 2`, `auto_jit_promotions = 2`,
`jit_successes = 2`, `pure_calls_median = 256`,
`pure_batch_calls_median = 0`, `pure_jit_calls_median = 256`,
`pure_aot_calls_median = 0`, `pure_vm_calls_median = 0`,
`pure_fallbacks_median = 0`, `pure_arg_vec_allocations_median = 0`,
`pure_arg_bytes_borrowed_median = 5120`, and
`elapsed_ns.median = 1093100`.
