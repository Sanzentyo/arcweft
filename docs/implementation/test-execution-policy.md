# Test execution policy

This policy selects validation by affected behavior and unresolved risk.
[Justfile](../../Justfile) and [just/verify.just](../../just/verify.just) own the
executable recipes. A push, a compaction, or the word "Rust" does not by itself
require a workspace run. Explicit task/contract acceptance checks remain required.

## Select evidence, then finish

Exercise the changed invariant and its affected consumers. Use existing meaningful
coverage where sufficient; add a regression when a bug or new behavior lacks it.
Do not add tests that merely mirror implementation syntax for a reversible,
low-impact edit. Parser families still need success, malformed, recovery-span,
and ambiguity coverage; codecs and public boundaries need their observable
round-trip/rejection/compile evidence where affected.

Run appropriate local checks, fix change-caused failures, and rerun affected
checks without asking for each step. Ordinary dependency resolution and build
artifacts in the existing development environment are part of this workflow.
Tests that access real devices, external services, credentials, or non-disposable
user data depend on their actual authorization; do not declare every test safe
or ask about every test merely because some may have external effects.

Select a stable feature/target combination and exact test names or owner groups.
Broaden or repeat only for changed inputs, failures, unresolved coverage, or an
applicable acceptance requirement. Once the evidence is sufficient, deliver the
result instead of starting another speculative verification loop.

## Scope of checks

- **Isolated Rust implementation:** changed-crate checks, meaningful owner tests,
  and Clippy on affected packages with `--all-targets` and relevant features.
  Format changed Rust. No automatic workspace or Tier 2 run for pushing this cut.
- **Shared semantics or workspace integration:** changes to shared language,
  runtime, serialized/public contracts with broad consumer impact, workspace-wide
  build settings, or dependency/features affecting shared consumers require
  `cargo check --workspace --all-targets --all-features`,
  `cargo clippy --workspace --all-targets --all-features`, and
  `just test-workspace`, plus affected contract tests. File/crate count alone is
  not this trigger; a private rename or leaf-only dependency change can use its
  affected dependency/consumer closure.
- **Specialized surfaces:** select affected CLI integration tests, crate doctests
  for executable/public API documentation, `just verify-vendor-glyphon` for that
  fork or its adapter contract, and generated-artifact checks for changed
  generators/data. A prose-only Rust documentation correction need not run every
  workspace doctest. Use the full named matrix for an explicitly requested
  milestone or a change affecting that entire surface.
- **Structure:** use [structural-audit-policy.md](structural-audit-policy.md).
  A relevant blocking dependency/ownership violation must be fixed; a small
  Rust edit or an existing size warning is not a blanket scanner trigger.

These are selection rules, not four consecutive steps. `just test-fast`,
`just test-rich-text`, and `just test-cli-native` are available bundles, not
mandatory additions to equivalent owner tests. Use `just verify` or
`just verify-full` only when their whole coverage is warranted. Do not run a
bundle and all its constituent commands again for the same inputs.

## Tier 2 and environment-sensitive evidence

Run the matching narrow Tier 2 targets when changing MCP protocol/resource URIs,
subprocess stdio, Agent observe, capture lifetime/readback, auxiliary attachments,
visual output, or production-limit boundaries covered by ignored tests.

Use exhaustive `just test-tier2` for a milestone requiring it or when changes to
shared scheduling, lifetime, rendering, or protocol machinery can affect multiple
Tier 2 families and narrower coverage cannot establish the contract. A cross-crate
edit somewhere on a runtime path is not enough on its own. State the affected
families and why a broader run was selected or unrelated families were excluded;
a short validation note suffices, not a separate approval or risk document.

Use the pinned platform/artifact procedure when native visual acceptance requires
it; the retained reference is
[test execution measurements](test-profiling/test-execution-measurements-2026-06-12-to-2026-07-10.md).

## Reuse and failure handling

Reuse a recorded pass when its relevant source, tests/fixtures, manifests/lockfile,
features/target, toolchain, and environment are unchanged. Preserve the original
command, result, and revision/patch identity; never present reuse as a new run.
Committing the tested bytes or resuming after compaction does not invalidate them.
Rerun after relevant changes, integration with newer `main`, flakiness, or missing
evidence. A failed, blocked, or not-run check is not a pass.

Diagnose a failure rather than asking whether to fix a regression from this task.
Do not lower assertions, bypass a gate, or restore obsolete behavior merely to
make tests pass. Identify pre-existing failures with actual baseline evidence
where practical; do not assert they are pre-existing just because they seem
unrelated. An unresolved change-caused failure blocks acceptance of that behavior.

A missing platform, permission, or toolchain blocks that check, not unrelated
implementation/design or a requested artifact. Complete feasible work and report
exactly which acceptance remains unverified. Do not promote "not run" to complete,
or publish unvalidated production WIP under the coherent-cut rule.

## Documentation-only cuts

Review changed content, links, formatting, and affected schema/example consistency;
check repository status and `git diff --check`. Only validate executable examples
when the edit affects them. Instructions and prose alone do not require Rust,
Clippy, Tier 2, or structural-scanner execution.

For connector-only edits, use pinned before/after content, whitespace and changed
link checks, then verify the published commit's changed paths/blob identities,
parent, and non-forced ref update. Report scratch-content/remote evidence, not a
local checkout's dirty/clean state. Documentation checks are not behavioral tests.

## Existing test profile

The workspace test profile retains line-table debug information; `dev` retains
full debug information. For variable-level test debugging, temporarily use
`CARGO_PROFILE_TEST_DEBUG=2` for that invocation. Do not change profiles, Cargo
concurrency, or feature selection as an incidental testing optimization. See the
[Cargo profile reference](https://doc.rust-lang.org/cargo/reference/profiles.html#debug).
