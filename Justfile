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

bench-thread:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/001_thread_scheduling.arcw --json --iterations 10 --warmup 2 --samples 5 --steps 64 --max-ops 64

bench-system:
    @cargo run -p arcweft-cli --quiet -- bench tests/fixtures/arcw/spec_should_pass/bench/004_system_info_threads.arcw --json --iterations 1 --warmup 0 --samples 3 --steps 24 --max-ops 24 --mode drain
