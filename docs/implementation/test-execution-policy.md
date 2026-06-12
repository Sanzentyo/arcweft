# Test Execution Policy

This document records the local validation policy for Arcweft development. It is
based on profiling from 2026-06-12 on Windows after the native rich-text capture
work landed and was re-measured while tuning the test policy.

## Profiling Snapshot

The slow path is not ordinary Rust unit testing or native rendering itself. The
slow paths are:

- MCP stdio end-to-end capture tests, which start `arcw agent mcp`
  subprocesses and exercise native image capture/resource reads through
  JSON-RPC.
- The broad `agent_observe_writes_layer_png_and_object_raw_images` CLI resource
  matrix, which launches `arcw agent observe` many times in one test to cover
  png/raw/object-id/mask/read-uri/MCP output shapes.

Exact PNG/imq golden checks are fast enough, but they are environment-sensitive
because local font/rasterization differences can produce small non-zero image
metrics.

Measured commands:

| Command | Scope | Time |
| --- | --- | ---: |
| `cargo test -p arcweft-cli --test check agent_observe_native_renderer -- --nocapture` | 10 native `agent observe` capture tests | 4.573s |
| `cargo test -p arcweft-cli --test check agent_mcp_stdio -- --nocapture` | 11 MCP stdio tests | 353.251s |
| `cargo test --workspace` | full workspace after warm build | 426.2s |

Re-measured commands:

| Command | Scope | Time | Result |
| --- | --- | ---: | --- |
| `cargo test -p arcweft-cli --test check agent_observe_native_renderer -- --nocapture` | 12 direct native capture tests | 16.320s wall / 4.61s test body | failed on exact imq golden only |
| `cargo test -p arcweft-cli --test check agent_mcp_stdio -- --nocapture` | 11 MCP stdio tests | 346.713s wall / 345.93s test body | passed |
| `cargo test -p arcweft-cli --test check agent_observe_native_renderer -- --nocapture` | 11 direct native capture tests plus 1 ignored visual golden | 4.406s wall / 3.66s test body | passed |
| `cargo test --workspace` | full workspace with Tier 2 ignored | 78.467s wall / `check.rs` 38.14s test body | passed, 12 ignored |

Re-measured while tightening the default policy:

| Command | Scope | Time | Result |
| --- | --- | ---: | --- |
| `cargo test --workspace --quiet` | full workspace before moving the slow observe matrix to Tier 2 | 54.028s wall / `check.rs` 39.78s test body | passed, 12 ignored |
| `cargo test -p arcweft-cli --test check --quiet` | `check.rs` before moving the slow observe matrix to Tier 2 | 59.235s wall / 41.21s test body | passed, 12 ignored |
| `cargo test -p arcweft-cli --test check agent_observe_writes_layer_png_and_object_raw_images --quiet` | broad CLI observe image/resource matrix | 36.736s wall / 35.91s test body | passed |
| `cargo test -p arcweft-cli --test check agent_observe_native_renderer --quiet -- --nocapture` | direct native renderer capture group | 4.507s wall / 3.78s test body | passed, 1 ignored |
| `cargo test -p arcweft-cli --test check bench_json --quiet` | CLI bench JSON group | 1.599s wall / 0.82s test body | passed |
| `cargo test -p arcweft-cli --test check run_json --quiet` | CLI run JSON group | 1.101s wall / 0.30s test body | passed |
| `cargo test -p arcweft-cli --test check jit_check_json --quiet` | CLI JIT check group | 2.209s wall / 1.59s test body | passed |
| `cargo test -p arcweft-cli --test regression_harness --quiet` | CLI source-tree regression harness | 1.620s wall / 0.38s test body | passed |
| `cargo test -p arcweft-cli --test check agent_observe_writes_layer_png_and_object_raw_images --quiet` | default run after moving matrix to Tier 2 | 5.850s wall / 0.00s test body | passed, 1 ignored |
| `cargo test -p arcweft-cli --test check agent_observe_writes_layer_png_and_object_raw_images --quiet -- --ignored --nocapture` | explicit Tier 2 run after policy change | 36.411s wall / 35.67s test body | passed |
| `cargo test --workspace --quiet` | full workspace after moving matrix to Tier 2 | 27.968s wall / `check.rs` 5.47s test body | passed, 13 ignored |

The direct native capture group failure was
`agent_observe_native_renderer_vertical_tutr_matches_checked_in_imq_golden` with
`mse=0.0003445231568253663` where the checked-in exact-image assertion expected
zero. That test remains useful as an explicit visual-regression check, but it is
not reliable enough for the default local or CI fast path.

Representative exact tests:

| Test | Time |
| --- | ---: |
| `agent_mcp_stdio_lists_resource_templates_before_observe` | 22.559s including initial compile; test body 1.19s |
| `agent_observe_native_renderer_writes_framebuffer_png` | 2.038s |
| `agent_observe_native_renderer_writes_rich_text_layer_png_crop` | 1.663s |
| `agent_mcp_stdio_captures_source_with_native_renderer` | 28.907s |
| `agent_mcp_stdio_captures_source_layer_with_native_renderer` | 54.179s |
| `agent_mcp_stdio_captures_source_ruby_object_id_with_native_renderer` | 29.122s |
| `agent_observe_native_vertical_capture_matches_imq_reference` | 2.802s |

## Local Execution Tiers

Use tiered validation instead of running the full workspace test suite after
every small edit. Default `cargo test --workspace` intentionally skips Tier 2
tests marked with `#[ignore]`.

Tier 0 is for tight implementation loops:

```bash
cargo fmt -p <changed-crate>
cargo check -p <changed-crate>
cargo test -p <changed-crate>
```

When a change crosses crate boundaries, run the directly affected crates in the
same command. For example, rich-text layout/native work should usually run:

```bash
cargo fmt -p arcweft-text-layout -p arcweft-glyphon -p arcweft-player-native
cargo test -p arcweft-text-layout -p arcweft-glyphon -p arcweft-player-native
```

Tier 1 is the normal pre-push cut-point validation:

```bash
cargo check --workspace
cargo clippy --workspace --all-targets --all-features
cargo test -p <changed-crate-or-crates>
```

Add focused CLI integration tests that directly cover the changed behavior. For
native rich-text capture work, prefer the fast direct `agent observe` native
group first:

```bash
cargo test -p arcweft-cli --test check agent_observe_native_renderer -- --nocapture
```

Do not use broad filters such as `agent_observe` as a routine Tier 1 shortcut.
That filter also selects slow resource-matrix coverage. Use the narrow exact
test or prefix for the behavior being changed.

Tier 2 is opt-in validation for changes that touch Agent MCP protocol semantics,
resource URI handling, subprocess stdio behavior, capture resource lifetime,
exact visual golden output, or before a milestone handoff that explicitly needs
full end-to-end evidence:

```bash
cargo test -p arcweft-cli --test check agent_mcp_stdio -- --ignored --nocapture
cargo test -p arcweft-cli --test check agent_observe_writes_layer_png_and_object_raw_images -- --ignored --nocapture
cargo test -p arcweft-cli --test check agent_observe_native_renderer_vertical_tutr_matches_checked_in_imq_golden -- --ignored --nocapture
```

The equivalent Justfile targets are:

```bash
just test-slow-mcp
just test-slow-agent-observe
just test-visual-golden
just test-tier2
just verify-full
```

Because Tier 2 currently takes several minutes on Windows when the MCP stdio
suite is included, and because the visual golden is environment-sensitive, do
not run it automatically for every small mainline cut point unless the changed
code is in that risk area.

## CI Direction

MCP stdio, the broad Agent observe resource matrix, and exact visual golden
coverage are marked `#[ignore]`, so `cargo test --workspace` is the normal fast
test job rather than the exhaustive job. CI should keep fast crate tests and
focused native renderer tests in the normal job, then run Tier 2 in an
explicitly named slow job or scheduled job.

Local reports should state which tier was run and whether the slow MCP stdio,
broad Agent observe resource-matrix, and exact visual-golden suites were
intentionally skipped or completed.
