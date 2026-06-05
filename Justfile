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

toolchain-profile-pure-jit-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command bench-003 --command bench-009 --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-aot-object-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command bench-009-aot-object --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-width-fast-path-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command bench-033-width-jit --command bench-033-width-aot --command bench-033-width-vm --command bench-040-width-jit --command bench-040-width-aot --command bench-040-width-vm --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-width-object-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command bench-033-width-aot-object --command bench-040-width-aot-object --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-math-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command math-matmul-bias --command math-matrix-add --command math-tensor-add --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-math-f64-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command math-matmul-f64 --command math-matrix-add-f64 --command math-tensor-add-f64 --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-math-wgpu-reuse-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command math-matmul-bias-wgpu-reuse --command math-matrix-add-wgpu-reuse --command math-tensor-add-wgpu-reuse --repeat {{repeat}} --warmup {{warmup}} --json

toolchain-profile-math-auto-wgpu-benches repeat="3" warmup="1":
    @cargo run -p arcweft-cli --quiet -- toolchain-profile --command math-matmul-auto-wgpu --command math-matmul-bias-auto-wgpu-reuse --command math-matrix-add-auto-wgpu-reuse --command math-tensor-add-auto-wgpu-reuse --repeat {{repeat}} --warmup {{warmup}} --json

bench-009:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit --pure-workers 4 --pure-batch-min-len 64

bench-002:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/002_map_pure_jit.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-003:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-005:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/005_inferred_pure_jit.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-007:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/007_branching_iter_pure_jit.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 128 --max-ops 128 --pure-backend jit

bench-008:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/008_large_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend auto

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

bench-029:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/029_dense_i8_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-030:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/030_dense_i16_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-031:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/031_dense_u8_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-032:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/032_dense_u16_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-033:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw --json --iterations 2 --warmup 1 --samples 1 --steps 128 --max-ops 128 --pure-backend jit

bench-036:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/036_dense_isize_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-037:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/037_dense_usize_map_pure_batch.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --pure-backend jit

bench-038:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/038_wide_for_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit

bench-039:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/039_hot_for_pure_auto_jit.arcw --json --iterations 4 --warmup 1 --samples 3 --steps 512 --max-ops 512 --pure-backend auto

bench-040:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw --json --iterations 2 --warmup 1 --samples 1 --steps 128 --max-ops 128 --pure-backend jit

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
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/029_dense_i8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/030_dense_i16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/031_dense_u8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/032_dense_u16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/036_dense_isize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/037_dense_usize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend vm

bench-numeric-aot:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/019_dense_i128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/020_dense_u128_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/029_dense_i8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/030_dense_i16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/031_dense_u8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/032_dense_u16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/036_dense_isize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/037_dense_usize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend aot

bench-numeric-jit:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/002_map_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/005_inferred_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/007_branching_iter_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 128 --max-ops 128 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/008_large_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend auto
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend auto
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/029_dense_i8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/030_dense_i16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/031_dense_u8_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/032_dense_u16_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw --json --iterations 2 --warmup 1 --samples 1 --steps 128 --max-ops 128 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/036_dense_isize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/037_dense_usize_map_pure_batch.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/038_wide_for_pure_jit.arcw --json --iterations 8 --warmup 2 --samples 5 --steps 64 --max-ops 64 --pure-backend jit
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/039_hot_for_pure_auto_jit.arcw --json --iterations 4 --warmup 1 --samples 3 --steps 512 --max-ops 512 --pure-backend auto
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw --json --iterations 2 --warmup 1 --samples 1 --steps 128 --max-ops 128 --pure-backend jit

bench-024:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/024_matrix_matmul_f32.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend glam --value lhs=matrix/f32/4x4:1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1 --value rhs=matrix/f32/4x4:2,0,0,0,0,2,0,0,0,0,2,0,0,0,0,2

bench-024-wgpu-auto:
    @cargo run -p arcweft-cli --features math-wgpu --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/024_matrix_matmul_f32.arcw --json --iterations 5 --warmup 2 --samples 5 --steps 64 --max-ops 64 --math-backend auto --math-wgpu-min-elements 1 --value lhs=matrix/f32/8x8:1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1 --value rhs=matrix/f32/8x8:2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2

bench-025:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/025_matrix_add_f32.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=matrix/f32/4x4:1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16 --value rhs=matrix/f32/4x4:16,15,14,13,12,11,10,9,8,7,6,5,4,3,2,1

bench-026:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/026_tensor_add_f32.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=tensor/f32/2x2x2:1,2,3,4,5,6,7,8 --value rhs=tensor/f32/2x2x2:8,7,6,5,4,3,2,1

bench-027:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/027_matrix_matmul_f64.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=matrix/f64/2x2:1.5,2,3.25,4.5 --value rhs=matrix/f64/2x2:5,6.5,7,8.25

bench-028:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/028_tensor_add_f64.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=tensor/f64/2x2:1.5,2.25,3.75,4.5 --value rhs=tensor/f64/2x2:5,6.25,7.5,8.75

bench-035:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/035_matrix_add_f64.arcw --json --iterations 15 --warmup 3 --samples 9 --steps 64 --max-ops 64 --math-backend ndarray --value lhs=matrix/f64/2x2:1.5,2.25,3.75,4.5 --value rhs=matrix/f64/2x2:5,6.25,7.5,8.75

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

bench-math-matmul-bias:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op matmul-bias-add --size 64 --iterations 10 --warmup 2

bench-math-f64:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op matmul-f64 --size 64 --iterations 10 --warmup 2
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op matrix-add-f64 --size 1024 --iterations 5 --warmup 1
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --quiet -- --backend all --op tensor-add-f64 --size 1024 --iterations 5 --warmup 1

bench-math-matmul-bias-wgpu:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend all --op matmul-bias-add --size 512 --iterations 3 --warmup 1

bench-math-matmul-bias-reuse:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul-bias-add --size 128 --iterations 5 --warmup 1 --reuse

bench-math-matmul-bias-submit-only:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul-bias-add --size 512 --iterations 3 --warmup 1 --submit-only

bench-math-matmul-bias-reuse-update:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul-bias-add --size 128 --iterations 5 --warmup 1 --reuse-update-inputs

bench-math-matmul-bias-reuse-capacity:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul-bias-add --size 128 --iterations 5 --warmup 1 --reuse-capacity

bench-math-inference-matmul-bias-reuse:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op inference-matmul-bias-add --size 128 --iterations 5 --warmup 1 --reuse

bench-math-inference-matmul-bias-reuse-update:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op inference-matmul-bias-add --size 128 --iterations 5 --warmup 1 --reuse-update-inputs

bench-math-matmul-reuse-update:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul --size 128 --iterations 5 --warmup 1 --reuse-update-inputs

bench-math-matmul-reuse-capacity:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matmul --size 128 --iterations 5 --warmup 1 --reuse-capacity

bench-math-matrix-add:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend all --op matrix-add --size 4096 --iterations 5 --warmup 1

bench-math-tensor-add:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend all --op tensor-add --size 4096 --iterations 5 --warmup 1

bench-math-matrix-add-reuse:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matrix-add --size 4096 --iterations 5 --warmup 1 --reuse

bench-math-matrix-add-submit-only:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matrix-add --size 4096 --iterations 3 --warmup 1 --submit-only

bench-math-matrix-add-reuse-update:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matrix-add --size 64 --iterations 5 --warmup 1 --reuse-update-inputs

bench-math-matrix-add-reuse-capacity:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op matrix-add --size 64 --iterations 5 --warmup 1 --reuse-capacity

bench-math-tensor-add-reuse:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op tensor-add --size 4096 --iterations 5 --warmup 1 --reuse

bench-math-tensor-add-submit-only:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op tensor-add --size 4096 --iterations 3 --warmup 1 --submit-only

bench-math-tensor-add-reuse-update:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op tensor-add --size 64 --iterations 5 --warmup 1 --reuse-update-inputs

bench-math-tensor-add-reuse-capacity:
    @cargo run --release -p arcweft-runtime-accelerator --example math_bench --features math-wgpu --quiet -- --backend wgpu --op tensor-add --size 64 --iterations 5 --warmup 1 --reuse-capacity

browser-webgpu-bench-check:
    @cargo check -p arcweft-browser-bench --target wasm32-unknown-unknown --all-features
    @node --test crates/arcweft-browser-bench/web/chrome-smoke-summary.test.mjs

browser-webgpu-bench-build:
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build

browser-webgpu-bench-serve port="8787":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build-and-serve --port {{port}}

browser-webgpu-bench-smoke port="8787":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}}

browser-webgpu-bench-perf port="8788":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}} --preset perf --timeout-ms 180000

browser-webgpu-bench-isolate port="8789":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}} --preset isolate --timeout-ms 180000

browser-webgpu-bench-stability port="8790":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}} --preset stability --timeout-ms 240000

browser-webgpu-bench-capacity-stability port="8791":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}} --preset capacity-stability --timeout-ms 300000

browser-webgpu-bench-submit-only port="8792":
    @cargo run -p arcweft-browser-bench --bin browser_bench_host -- build
    @node crates/arcweft-browser-bench/web/chrome-smoke.mjs --port {{port}} --preset submit-only --timeout-ms 300000
