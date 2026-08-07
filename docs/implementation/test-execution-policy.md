# Test execution policy

This is the current validation-selection authority for Arcweft. The Justfile is
the executable command authority. Historical timings and the older detailed
command inventory are retained under `test-profiling/`.

## General rules

- Match validation to the changed behavior and risk. Do not run the full
  workspace after every small edit.
- Keep the Cargo feature combination stable within a slice so the result is
  comparable and `target/` growth remains bounded.
- Prefer exact test names or narrow crate-owned groups over broad substring
  filters that accidentally select slow matrices.
- Record commands actually run, pass/fail status, and intentionally skipped
  tiers. A planned command is not evidence.
- Do not preserve obsolete production behavior to satisfy a stale test. Update
  expectations and deterministic fixtures to the selected final contract.

## Tight loop

Use the smallest direct evidence for the changed owner:

- changed-crate `cargo check`;
- exact focused unit or integration tests; and
- the narrow parser/sema/runtime/render/codec test family that owns the rule.

Use `just test-fast` for the short core/render-text/text-layout/native-player
smoke route. Use `just test-rich-text` or `just test-cli-native` only when that
surface is touched.

## Reviewable Rust cut

At a coherent Rust cut, run:

1. focused tests for every changed behavior;
2. `cargo check --workspace --all-targets --all-features` when the cut crosses
   crates or public contracts;
3. `cargo clippy --workspace --all-targets --all-features` when feasible;
4. `just structure-audit` when required by `structural-audit-policy.md`, plus
   `just structure-audit-gate` before accepting a structure-gated cut; and
5. the matching runtime/render/Agent/MCP/capture tier described below.

Use `cargo fmt` for changed Rust. Use `just verify` when the cut is broad enough
to require formatter, Clippy, workspace-fast tests, and generated JLREQ data
together.

## Main push cut

- Run `just test-workspace` for normal Rust mainline cuts unless the change is
  docs-only or demonstrably cannot affect Rust behavior.
- Use `just test-cli-check` or exact `check.rs` tests for ordinary CLI behavior.
  Use `just test-cli-check-full` only when the full CLI integration matrix is
  explicitly warranted.
- Run `just test-doc` when Rust documentation comments, doctest examples, or
  public API documentation changed, or for an explicit milestone.
- Run `just verify-vendor-glyphon` when the vendored glyphon fork or its
  adapter-facing contract changed.

## Tier 2

Tier 2 covers ignored or environment-sensitive MCP stdio, broad Agent observe,
auxiliary native capture, and exact visual-golden paths.

Run the matching narrow Tier 2 target when a cut changes Agent MCP protocol,
resource URIs, subprocess stdio, capture lifetime/readback, native auxiliary
attachments, or bounded visual output.

Run exhaustive `just test-tier2` before completing a cut when both are true:

1. it spans multiple crates or materially changes a public contract; and
2. it affects a runtime, render, Agent, MCP, or capture path.

An isolated small public-API edit is not Tier 2 solely because it is public.
For milestone native visual handoff, use the pinned Windows environment and
artifact procedure retained in
`test-profiling/test-execution-measurements-2026-06-12-to-2026-07-10.md`.

## Documentation-only cuts

For instruction, request, or stable-documentation-only changes, validate links,
formatting, repository status, `git diff --check`, and any schema/example
consistency directly affected by the edit. Rust workspace tests and Tier 2 are
not required unless the documentation change accompanies Rust behavior.
