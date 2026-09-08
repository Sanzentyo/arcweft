# Agent Fx fixture migration — 2026-09-09

Inspected base: `e1d2309c120e5ac2e12d59f0f134c5f98bda4730`, existing `main`,
already pushed and clean before this change. The
[test profile follow-up](2026-09-09-test-profile-memory.md) made workspace
compilation possible and exposed two outdated Agent protocol fixtures.

## Change and ownership

`test_rich_text_ref` now creates its zero-parameter test definition through
`FxDefinition::new`, then uses `FxApplicationDraft` and `FxApplication::bind`.
The definition owner supplies the canonical application layout and storage.
The old handwritten JSON with a `parameters` array is deleted. Existing
observation/image serialization assertions and the `test::shake` identity are
preserved. The empty test graph supplies metadata; it is not a new shake
implementation or evidence of renderer behavior.

`arcweft-presentation` is a workspace-inherited development dependency of
`arcweft-agent-protocol`; the lockfile records that existing local package.
No production dependency, API, codec reader, fallback, or version was added.
The protocol continues to transport the presentation-owned type.

The changed test module is
[agent-protocol/src/tests.rs](../../crates/arcweft-agent-protocol/src/tests.rs):
47,718 bytes, 1,328 physical LOC versus 1,324 at the base. Its responsibility
remains Agent protocol wire/observation fixtures and assertions. The shared
fixture serves the two existing failing tests; it does not duplicate an Fx
schema or introduce unrelated mutable state. Normal workspace dependency
fan-out remains three; the explicit development dependencies increase from
six to seven. The audit found no blocking dependency change.

## Actual validation

Logs and result JSON are under
`.arcweft-local/validation/2026-09-09-agent-fx-fixture/`.

| Command | Result |
| --- | --- |
| `cargo test -p arcweft-agent-protocol --lib -- --nocapture` | Passed 27/27, including both previously failing tests; 48.17 seconds. |
| `just test-workspace` | 144 passed / 1 failed across 16 reports; 13.02 seconds. No memory-mapping/internal-compiler failure. |
| `cargo clippy -p arcweft-agent-protocol --all-targets --all-features` | Passed with dependency warnings; 30.85 seconds. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --fail-on-blocking` | Passed: 95 packages, 2,236 Rust files, 309 review triggers, zero blocking violations; 3.65 seconds. Non-writing screening. |
| `cargo fmt --all -- --check` | Passed, 10.31 seconds; formatting also ran before the focused test. |
| Documentation links and diff whitespace | Six relative targets across two documents exist; anchors not checked. `git diff --check` passed. |

The workspace recipe now stops at
`arcweft-agent-repl::compile::tests::compiler_reuses_the_accepted_synthetic_source_identity`.
Its selected Agent Product fails the same AWBC project-call effect check as
the earlier MCP trace test. `AwbcFlowLowerer::lower_flow` currently emits an
empty signature effect set, while the ordinary controller function retains
its nonempty effects. The required caller/callee subset validation must remain;
the missing Flow effect projection is part of the
[active callable/effect work](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).

Later workspace targets and CLI recipe commands were not run after that
failure. Doctests and Tier 2 were not repeated for this test-fixture-only
change. No Flow/callback inference, callable execution, nominal, or restore
implementation is claimed here; the full convergence goal remains active.
