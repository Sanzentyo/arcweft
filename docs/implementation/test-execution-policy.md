# Test Execution Policy

This document records the local validation policy for Arcweft development. It is
based on profiling from 2026-06-12 on Windows after the native rich-text capture
work landed.

## Profiling Snapshot

The slow path is not ordinary Rust unit testing or native rendering itself. It is
the MCP stdio end-to-end capture suite, which starts `arcw agent mcp` subprocesses
and exercises native image capture/resource reads through JSON-RPC.

Measured commands:

| Command | Scope | Time |
| --- | --- | ---: |
| `cargo test -p arcweft-cli --test check agent_observe_native_renderer -- --nocapture` | 10 native `agent observe` capture tests | 4.573s |
| `cargo test -p arcweft-cli --test check agent_mcp_stdio -- --nocapture` | 11 MCP stdio tests | 353.251s |
| `cargo test --workspace` | full workspace after warm build | 426.2s |

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
every small edit.

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

Tier 2 is for changes that touch Agent MCP protocol semantics, resource URI
handling, subprocess stdio behavior, capture resource lifetime, or before a
milestone handoff that explicitly needs full end-to-end evidence:

```bash
cargo test -p arcweft-cli --test check agent_mcp_stdio -- --nocapture
cargo test --workspace
```

Because Tier 2 currently takes several minutes on Windows, do not run it
automatically for every small mainline cut point unless the changed code is in
that risk area.

## CI Direction

The current test code keeps MCP stdio coverage in the default workspace suite, so
`cargo test --workspace` remains the strongest one-command verification. The
desired CI structure is to split slow end-to-end MCP/native capture coverage into
an explicitly named job while keeping fast crate tests and focused native
renderer tests in the normal job.

Until that split exists, local reports should state which tier was run and
whether the slow MCP stdio/full workspace suite was intentionally skipped or
completed.

