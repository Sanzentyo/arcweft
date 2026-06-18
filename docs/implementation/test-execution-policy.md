# Test Execution Policy

This document records the local validation policy for Arcweft development. It is
based on profiling from 2026-06-12 on Windows after the native rich-text capture
work landed and was re-measured while tuning the test policy.

## Profiling Snapshot

The slow path is not ordinary Rust unit testing or native rendering itself. The
current slow path is the broad
`agent_observe_writes_layer_png_and_object_raw_images` CLI resource matrix,
which launches `arcw agent observe` many times in one test to cover
png/raw/object-id/mask/read-uri/MCP output shapes.

MCP stdio end-to-end capture tests used to be another slow path because
`resources/list` eagerly rendered every layer/object capture resource. MCP now
lists capture refs lazily and renders only when `resources/read` or
`arcweft.capture` requests a specific image. Keep the MCP suite in Tier 2
because it still starts `arcw agent mcp` subprocesses and exercises native
image capture/resource reads through JSON-RPC, but it is no longer a
multi-minute local command.

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

Re-measured after adding rich-text glyph-cluster observation work:

| Command | Scope | Time | Result |
| --- | --- | ---: | --- |
| `cargo test --workspace --no-run --quiet` | compile all workspace test binaries after local edits | 28.6s wall | passed |
| `cargo test --workspace --quiet` | full workspace, Tier 2 ignored | 25.872s wall / `check.rs` 6.51s test body | passed, 13 ignored |
| `cargo test -p arcweft-cli --test check --quiet` | CLI integration test binary, Tier 2 ignored | 14.788s wall / 7.79s test body | passed, 13 ignored |
| `cargo test -p arcweft-cli --test check agent_observe_native_renderer --quiet` | direct native observe group | 5.241s wall / 4.43s test body | passed, 1 ignored |
| `cargo test -p arcweft-cli --test check agent_observe_json --quiet` | Agent observe JSON report tests | 0.800s wall / 0.05s test body | passed |
| `cargo test -p arcweft-cli --test check run_json --quiet` | CLI runtime JSON tests | 0.903s wall / 0.28s test body | passed |
| `cargo test -p arcweft-cli --test check bench_json --quiet` | CLI bench JSON tests | 1.875s wall / 1.18s test body | passed |
| `cargo test -p arcweft-cli --test check jit_check_json --quiet` | CLI JIT comparison tests | 1.993s wall / 1.26s test body | passed |
| `cargo test -p arcweft-cli --test regression_harness --quiet` | checked-in source-tree regression harness | 1.687s wall / 0.41s test body | passed |

Re-profiled after the local loop was found too slow:

| Command | Scope | Time | Result |
| --- | --- | ---: | --- |
| `cargo test --workspace --quiet` | full workspace, Tier 2 ignored, after local edits | 42.900s wall / `check.rs` 6.80s test body | passed, 13 ignored |
| `cargo test -p arcweft-cli --test check --quiet` | CLI integration binary, Tier 2 ignored | 20.610s wall / 7.64s test body | passed, 13 ignored |
| `cargo test -p arcweft-player-native --lib --quiet` | native player library tests | 7.550s wall / 1.38s test body | passed |
| `cargo test -p arcweft-cli --test check agent_observe_native_renderer --quiet` | direct native observe group | 5.550s wall / 4.73s test body | passed, 1 ignored |
| `cargo test -p arcweft-cli --test regression_harness --quiet` | checked-in source-tree regression harness | 1.850s wall / 0.38s test body | passed |
| `cargo test --workspace --no-run --quiet` | compile workspace test binaries only | 9.490s wall | passed |
| `cargo test --workspace --quiet` | warm full workspace, Tier 2 ignored | 22.100s wall / `check.rs` 6.42s test body | passed, 13 ignored |
| `cargo test -p arcweft-core -p arcweft-render-text -p arcweft-text-layout -p arcweft-player-native --lib --quiet` | proposed smoke route | 13.870s wall / native body 1.37s | passed |

Re-profiled after the full workspace command was still too expensive for the
routine local loop:

| Command | Scope | Time | Result |
| --- | --- | ---: | --- |
| `cargo test -p arcweft-core -p arcweft-render-text -p arcweft-text-layout -p arcweft-player-native --lib --quiet` | current `just test-fast` smoke route, warm build | 1.970s wall | passed |
| `cargo test -p arcweft-cli --test check agent_observe_native_renderer --quiet` | current `just test-cli-native` direct native observe group | 12.452s wall | passed, 1 ignored |
| `cargo test -p arcweft-cli --test check --quiet` | current `just test-cli-check` CLI integration binary, Tier 2 ignored | 7.260s wall | passed, 13 ignored |
| `cargo test --workspace --lib --tests --quiet` | workspace lib and integration tests, Tier 2 ignored, doc-tests excluded | 22.210s wall | passed, 13 ignored |
| `cargo test --workspace --doc --quiet` | workspace doc-tests only | 117.769s wall | passed |
| `cargo test --workspace --quiet` | workspace default after the doc-test run warmed its artifacts | 24.801s wall | passed, 13 ignored |

Rechecked on 2026-06-16 after the native renderer test prefix had grown to
cover JLREQ and vertical-text matrices:

| Command | Scope | Time | Result |
| --- | --- | ---: | --- |
| `cargo test -p arcweft-cli --test check agent_observe_native_renderer --quiet` | broad prefix, 204 selected tests | 230.6s wall | failed on stale textbox object crop expectations |
| `cargo test -p arcweft-render-text -p arcweft-text-layout -p arcweft-player-native --lib --quiet` | rich-text/native library route | 3.6s wall | passed |

The broad `agent_observe_native_renderer` prefix is no longer a Tier 1 shortcut.
`just test-cli-native` now runs an exact smoke list for framebuffer capture,
dialogue layer crop, textbox object crop, textbox mask, and textbox object-id
capture. Use targeted exact tests for JLREQ or vertical-text work, and keep
large prefix runs for explicit profiling or milestone validation.

`arcweft-cli --test check` is now a purpose-specific CLI integration suite, not
part of the workspace fast path. `just test-workspace` excludes `arcweft-cli`
from the workspace-wide lib/test command, then runs `arcweft-cli` lib/bin tests,
the source-tree regression harness, and the checked-in fixture check runner.
Use `just test-cli-check`, exact `check.rs` tests, or the relevant Tier 2 target
when a change intentionally touches broad CLI command behavior.

Re-profiled after the vendored glyphon fork became part of the vertical text
acceptance evidence:

| Command | Scope | Time | Result |
| --- | --- | ---: | --- |
| `cargo check --manifest-path vendor/glyphon/Cargo.toml` | vendored glyphon fork outside the workspace | 42.09s cold | passed |
| `cargo test --manifest-path vendor/glyphon/Cargo.toml --lib` | vendored glyphon lib tests for GlyphArea transforms, clipping, custom glyphs, and color alpha | 47.42s cold / 0.00s test body | passed |
| `cargo clippy --manifest-path vendor/glyphon/Cargo.toml --lib --tests -- -D warnings -A clippy::too_many_arguments` | vendored glyphon lib/test lint gate, allowing the upstream public API shape | 2.38s warm | passed |

Re-profiled after MCP resource listing was changed from eager capture
generation to lazy capture-ref descriptors:

| Command | Scope | Time | Result |
| --- | --- | ---: | --- |
| `cargo test -p arcweft-cli --test check agent_mcp_stdio_observes_and_reads_rich_text_child_image -- --ignored --nocapture --exact` | observe, resources/list, capture, and resource readback through one MCP stdio session | 21.244s wall including rebuild / 3.43s test body | passed |
| `just test-slow-mcp` | 14 ignored MCP stdio native capture/resource tests | 20.2s wall / 18.38s test body | passed |

Release-profile testing was checked for the long native/JLREQ integration path:

| Command | Scope | Time | Result |
| --- | --- | ---: | --- |
| `cargo test -p arcweft-cli --test check agent_observe_native_renderer_reports_published_jlreq_numeric_abbreviation_geometry -- --nocapture --exact` | one published JLREQ native geometry test, debug warm | 14.330s wall / 13.37s test body | passed |
| `cargo test -p arcweft-cli --test check agent_observe_native_renderer_reports_published_jlreq_numeric_abbreviation_geometry --release -- --nocapture --exact` | same exact test, release warm | 10.960s wall / 10.18s test body | passed |
| `cargo test -p arcweft-cli --test check --release --quiet` | full CLI integration binary, release warm after initial release build | 147.968s wall / 147.02s test body | passed, 16 ignored |

Release mode helps CPU-heavy native integration tests, but the measured win was
only about 16-24% while the first release build cost 2m44s locally. Do not make
release-profile tests the default local policy. Use them only when repeated
long-running native matrix runs are unavoidable; otherwise split broad
integration matrices into focused Tier 1 coverage plus explicit Tier 2
validation.

This pass changes the policy entrypoint: `just test-workspace` now runs
`cargo test --workspace --lib --tests --quiet` instead of the Cargo default.
The Cargo default can include the expensive doc-test path depending on cache
state, and therefore is not the routine local command. Use `just test-doc` for
that path explicitly.

Exact native observe tests are not the main bottleneck. In a warm loop, most
single native observe tests finished with about 0.9s to 2.2s of test body time;
the first exact run paid extra process/cache startup wall time. Keep native
coverage focused, but do not move production-facing native geometry tests to
Tier 2 merely because they touch the renderer.

The crate-level profile also exposed one policy issue: `arcweft-test` only
compiled when another workspace member happened to enable `serde/derive`.
Crates that derive serde traits must declare `features = ["derive"]`
themselves so focused crate tests remain trustworthy.

`cargo nextest` was not installed in this local environment during this
measurement. The numbers above therefore use Rust's standard test harness and
coarse command-level wall-clock profiling. If individual-test timing becomes
necessary, add `cargo-nextest` as a local developer tool before reshaping the
test suite around per-test measurements.

The direct native capture group failure was the checked-in native visual golden
comparison with `mse=0.0003445231568253663` where the exact-image assertion
expected zero. That Tier 2 suite remains useful as an explicit
visual-regression check, but it is not reliable enough for the default local or
CI fast path.

Representative exact tests:

| Test | Time |
| --- | ---: |
| `agent_mcp_stdio_lists_resource_templates_before_observe` | 1.212s including harness startup; test body 0.05s |
| `agent_mcp_stdio_captures_profile_selected_source_without_prior_observe` | 2.80s test body for profile-selected observe-before-capture |
| `agent_observe_native_renderer_writes_framebuffer_png` | 2.038s |
| `agent_observe_native_renderer_writes_rich_text_layer_png_crop` | 1.663s |
| `agent_mcp_stdio_observes_and_reads_rich_text_child_image` | 3.43s test body after lazy MCP resource listing |
| `just test-slow-mcp` | 18.38s test body for all 14 ignored MCP stdio tests |
| `agent_observe_native_vertical_capture_matches_imq_reference` | 2.802s |

## Local Execution Tiers

Use tiered validation instead of running the full workspace test suite after
every small edit. `cargo test --workspace` skips Tier 2 tests marked with
`#[ignore]`, but it may still include expensive doc-test work. The routine
workspace entrypoint is therefore `just test-workspace`.

Tier 0 is for tight implementation loops:

```bash
cargo check -p <changed-crate>
cargo test -p <changed-crate>
```

When a change crosses crate boundaries, run the directly affected crates in the
same command. For example, rich-text layout/native work should usually run:

```bash
cargo fmt -p arcweft-text-layout -p arcweft-glyphon -p arcweft-player-native
cargo test -p arcweft-text-layout -p arcweft-glyphon -p arcweft-player-native
```

Tier 0 should stay comfortably under about 5 seconds after the first compile.
If a focused command is routinely slower than that, split the exact behavior
test from the broad integration group and keep the broad group in Tier 1 or
Tier 2.

Tier 1 is the normal reviewable cut-point validation:

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

For routine local execution through Justfile, use:

```bash
just test-fast
just test-cli-native
just test-cli-check
just test-workspace
just test-doc
```

`just test-fast` is now a smoke route, not a full workspace route. It covers
the core/render-text/text-layout/native-player library path used by rich-text
and native capture work. `just test-rich-text` adds the direct native
`agent observe` exact smoke slice. `just test-workspace` is the normal workspace
fast path: it runs workspace lib/integration tests except the large
`arcweft-cli --test check` binary, then runs CLI lib/bin tests plus lightweight
CLI integration harnesses. It intentionally does not run doc-tests.
`just test-doc` is the explicit doc-test path for Rust documentation examples
and milestone validation.
`just test-cli-native` is the normal native rich-text/Agent observe smoke slice;
it must remain exact-test based rather than using the broad
`agent_observe_native_renderer` prefix.
`just test-cli-check` is useful before a CLI-heavy cut point, but it is not
required after every small parser, layout, or protocol edit and should not be
used as the routine workspace fast path.

`just test-rich-text-object-goal` is the milestone gate for the current rich
text typed-presentation-object work. It combines Agent protocol/MCP metadata
tests, native effect/shader/motion/typewriter coverage, exact CLI observe and
hit-test regressions, and the two rich-text sample checks. It is intentionally
broader than `just test-rich-text` and should be run before claiming that
milestone complete or handing it off, not after every small edit. Run this
recipe sequentially; the native animation capture tests exercise GPU/offscreen
readback and can become misleadingly slow when several renderer-heavy Cargo
test processes compete in parallel.
On 2026-06-19 this gate passed locally in 250.9s wall time; the combined
typewriter/effect animation sample accounted for 182.92s of test-body time.
Treat that exact test as milestone evidence unless it is later split or
optimized.

`vendor/glyphon` is patched into the workspace but remains an external manifest,
so ordinary `cargo check --workspace`, workspace clippy, and
`just test-workspace` do not directly test the fork. When a change touches
`vendor/glyphon`, `arcweft-glyphon`, GlyphArea submission, shader clipping,
custom glyph rasterization, or the final vertical-text milestone evidence, run:

```bash
just verify-vendor-glyphon
```

That target runs manifest-path `cargo check`, lib tests, and a lib/test clippy
gate with `-D warnings`. It allows `clippy::too_many_arguments` because the
upstream glyphon renderer API already uses long prepare signatures and Arcweft
should not reshape that public API merely to satisfy a style lint.

Do not use broad filters such as `agent_observe` as a routine Tier 1 shortcut.
That filter also selects slow resource-matrix coverage. Use the narrow exact
test or prefix for the behavior being changed.

Tier 2 is opt-in validation for changes that touch Agent MCP protocol semantics,
resource URI handling, subprocess stdio behavior, capture resource lifetime,
bounded visual golden output, or before a milestone handoff that explicitly
needs full end-to-end evidence:

```bash
cargo test -p arcweft-cli --test check agent_mcp_stdio -- --ignored --nocapture
cargo test -p arcweft-cli --test check agent_observe_writes_layer_png_and_object_raw_images -- --ignored --nocapture
cargo test -p arcweft-cli --test check agent_observe_native_renderer_matches_checked_in_imq_golden_fixtures -- --ignored --nocapture
```

The equivalent Justfile targets are:

```bash
just test-slow-mcp
just test-slow-agent-observe
just test-visual-golden
just native-visual-artifacts
just verify-vendor-glyphon
just test-rich-text-object-goal
just test-tier2
just verify-full
```

Because Tier 2 still includes subprocess/GPU-facing MCP coverage, the broad
Agent observe resource matrix, and an environment-sensitive visual golden, do
not run it automatically for every small mainline cut point unless the changed
code is in that risk area. The MCP stdio part alone is now short enough to run
as the default validation for Agent MCP protocol, resource URI, capture
resource lifetime, and native readback changes.

Operational budget:

- Tight loop: changed-crate `cargo check` plus exact focused tests.
- Reviewable local cut point: `cargo check --workspace`, clippy when feasible,
  and focused tests for touched behavior. Use `just test-fast` or
  `just test-rich-text` when the touched behavior is in the render/layout/native
  path.
- Main push cut point: add `just test-workspace` unless the change is docs-only
  or otherwise demonstrably outside Rust behavior. Use `just verify` when the
  cut point touches generated JLREQ punctuation data or is broad enough that
  formatter, clippy, workspace fast tests, absolute-path scans, removed-DSL
  scans, and `just check-jlreq-punctuation` should all be asserted together.
  Add `just test-cli-check`, a focused exact `check.rs` test set, or a matching
  Tier 2 target when the CLI integration surface changed broadly. Add
  `just test-doc` only when Rust documentation comments, doctest examples, or
  public API documentation changed, or when preparing a milestone validation.
  Add `just verify-vendor-glyphon` when the vendored glyphon fork or its
  adapter-facing GlyphArea contract changed.
- Milestone or risky Agent/MCP/capture change: add the explicit Tier 2 target
  that matches the risk, or `just test-tier2` for an exhaustive slow pass.
- Milestone native visual handoff: run `just native-visual-artifacts` on a
  Windows machine with `imq` available and publish
  `target/arcweft-native-capture-artifacts/` as the CI/job artifact. That target
  builds `arcweft-cli` in release mode once, then uses `target/release/arcw.exe`
  for the image-producing `agent observe` calls because native PNG capture and
  image comparison are often faster through the optimized binary once the
  release build is warm. The target writes fresh native PNG candidates,
  `arcw agent observe` JSON, and `imq` JSON metric reports for every checked-in
  native visual golden.
- Sample visual review issues that are visible in generated PNGs must be kept
  under `docs/implementation/visual-sample-review-issues/` with the issue text
  and the corresponding PNG until the implementation has been improved and the
  issue is explicitly closed or superseded.

## CI Direction

MCP stdio, the broad Agent observe resource matrix, and exact visual golden
coverage are marked `#[ignore]`. The normal fast test job should use
`just verify` or its constituent commands rather than an unqualified
`cargo test --workspace`, so cold doc-test work cannot unexpectedly dominate
every push and JLREQ generated table drift is still caught. CI should keep fast
crate tests, `just check-jlreq-punctuation`, and focused native renderer tests
in the normal job, run doc-tests as a separate named job, then run Tier 2 in an
explicitly named slow job or scheduled job. MCP stdio can also be promoted into
a named Agent/MCP validation job when that surface changes, because it no
longer dominates the slow lane by itself.

Local reports should state which tier was run and whether MCP stdio,
broad Agent observe resource-matrix, exact visual-golden suites, and doc-tests
were intentionally skipped or completed.
