set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

default:
    @just --list

fmt:
    @cargo fmt --all

fmt-check:
    @cargo fmt --all --check

clippy:
    @cargo clippy --workspace --all-targets --all-features

test:
    @cargo test --workspace

regression:
    @cargo test -p arcweft-cli --test regression_harness

scan-absolute-paths:
    @cargo test -p arcweft-cli checked_in_docs_and_samples_do_not_record_host_absolute_paths --test regression_harness

scan-removed-dsl:
    @cargo test -p arcweft-cli source_tree_does_not_reintroduce_removed_whitespace_command_dsl_or_shims --test regression_harness

verify: fmt-check clippy test scan-absolute-paths scan-removed-dsl

bench-009:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit --pure-workers 4 --pure-batch-min-len 64

bench-010:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/010_dense_i32_sum.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-011:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/011_dense_u64_sum.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-012:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/012_dense_integer_widths_sum.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-013:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/013_dense_scalar_len.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-014:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/014_dense_textual_scalar_len.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-015:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/015_dense_wide_numeric_len.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64

bench-016:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-017:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-018:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-019:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/019_dense_i128_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-020:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/020_dense_u128_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-022:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-023:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-numeric-vm:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/010_dense_i32_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/011_dense_u64_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/012_dense_integer_widths_sum.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/013_dense_scalar_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/014_dense_textual_scalar_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/015_dense_wide_numeric_len.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64

bench-numeric-pure-vm:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/019_dense_i128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/020_dense_u128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm

bench-numeric-aot:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/019_dense_i128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/020_dense_u128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot

bench-numeric-jit:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit

bench-024:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/024_matrix_matmul_f32.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend glam --value lhs=matrix/f32/4x4:1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 --value rhs=matrix/f32/4x4:2,0,0,0,0,2,0,0,0,0,2,0,0,0,0,2

bench-024-wgpu-auto:
    @cargo run -p arcweft-cli --features math-wgpu --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/024_matrix_matmul_f32.arcw --json --iterations 5 --warmup 2 --samples 5 --steps 64 --max-ops 64 --math-backend auto --math-wgpu-min-elements 1 --value lhs=matrix/f32/8x8:1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1 --value rhs=matrix/f32/8x8:2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2

bench-025:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/025_matrix_add_f32.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=matrix/f32/4x4:1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16 --value rhs=matrix/f32/4x4:16,15,14,13,12,11,10,9,8,7,6,5,4,3,2,1

bench-026:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/026_tensor_add_f32.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=tensor/f32/2x2x2:1,2,3,4,5,6,7,8 --value rhs=tensor/f32/2x2x2:8,7,6,5,4,3,2,1

bench-thread:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/001_thread_scheduling.arcw --json --iterations 10 --warmup 2 --samples 5 --steps 64 --max-ops 64

bench-system:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/004_system_info_threads.arcw --json --iterations 1 --warmup 0 --samples 3 --steps 24 --max-ops 24 --mode drain

bench-math-cpu:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op matmul --size 64 --iterations 10 --warmup 2

bench-math-glam:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op matmul --size 4 --iterations 50 --warmup 5

bench-math-wgpu:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend all --op matmul --size 512 --iterations 3 --warmup 1

bench-math-matrix-add:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend all --op matrix-add --size 4096 --iterations 5 --warmup 1

bench-math-tensor-add:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend all --op tensor-add --size 4096 --iterations 5 --warmup 1

bench-math-matrix-add-reuse:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matrix-add --size 4096 --iterations 5 --warmup 1 --reuse

bench-math-tensor-add-reuse:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op tensor-add --size 4096 --iterations 5 --warmup 1 --reuse
