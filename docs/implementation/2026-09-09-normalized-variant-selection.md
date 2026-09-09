# Normalized variant selection — 2026-09-09

Inspected base: `3b0dfd56967e274eb44c0e4a9243bd8f4cdad41b`, existing `main`,
pushed and clean before this cut. The
[Agent host response work](2026-09-09-agent-host-response-admission.md) left a
reproduced resource/attach failure in Try lowering: the normalized
`Agent(Resource)` type could not pass the restricted checked-type projection.

## Boundary and deletion

`RuntimeNormalizedType::variant_selection` converted its full owner and
selected payload to `RuntimeCheckedType` before selecting a case. That
projection intentionally covers a smaller value domain than the admitted
RuntimePlan graph; Agent, range and function types cannot pass it. It also
erases distinctions between equal shapes with different semantic identities.
The new baseline tests reproduced both defects: 2 passed / 2 failed.

Case selection now stays on the normalized type. Result and Option require
their structural unary tuple payloads to reference the exact declared item
IDs, including unselected cases. The core builtin case schema supplies case
count and payload-presence requirements for every builtin family. Selection
then returns the borrowed owner/case/payload without projecting or copying a
second type model. The aggregate type inventory remains the owner of each
identity's complete definition and its admission to RuntimePlan.

The calls to the restricted checked-type projector and their obsolete
selection-error variant are deleted. The limited projector remains for its
other legitimate consumers; its domain is not enlarged with Agent-specific
exceptions. Both expression and Flow Try lowering consume the same selection
through `final_variant.rs`, preserving their exact type IDs and tuple seeds.
No new runtime variant, side table, source reconstruction, compatibility
reader or version marker is introduced.

The tests cover Agent Resource, range and function payloads under Result and
Option; same-shaped but differently identified payloads; malformed unselected
payloads; canonical builtin case counts and payload presence; invalid ordinals;
and non-variant owners. These are normalized-selection tests, not a claim that
all function-value execution or nominal restore requirements are complete.
The actual CLI resource/attach/checkpoint test now passes through execution
and persisted debug records, providing the source-to-runtime evidence.

## Validation

Logs and result JSON are under
`.arcweft-local/validation/2026-09-09-normalized-variant-selection/`.
Cargo uses normal concurrency, commands run sequentially, and Rust is not
edited while Cargo or formatting is live.

| Command / scope | Actual result |
| --- | --- |
| Baseline `variant_selection_tests` | Failed: 2 passed / 2 failed. Agent payload rejection and identity-erasing acceptance are reproduced. |
| Repaired `variant_selection_tests` | Passed: 4/4, 11.90 s. |
| CLI `agent_script_run_persists_attach_resource_debug_record`, `native-capture` | Passed: 1/1, 49.63 s. |
| `cargo test -p arcweft-runtime-plan --lib --quiet` | Passed: 64/64, 0.59 s. |
| Compiler `try_pipe`, `evaluated_effects`, `flow_effects` integrations | Passed: 8 + 9 + 5 = 22 tests, 31.58 s. |
| CLI `agent_script_run_persists_attach_capture_debug_record`, `native-capture` | Failed: 0/1, 5.43 s. Its CLI subprocess reports a main-thread stack overflow. |
| `cargo fmt --all -- --check` | Passed, 10.39 s. |
| `cargo check --workspace --all-targets --all-features` | Passed, 30.70 s. |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings, 28.60 s. |
| `just test-workspace` | Failed: 851 passed / 18 failed / 0 ignored across 83 result reports, 187.91 s. The callable execution target retains its 53 passes and 18 known failures. Subsequent workspace targets and the recipe's CLI commands were not run. |
| `just test-slow-mcp` | Passed: 4/4, 1.41 s. |
| `just test-doc` | Passed: 95 result reports, 8 executed doctests, 47.13 s. |
| Canonical structural audit with `--fail-on-blocking` | Passed: 95 packages, 2,240 Rust files, 309 review triggers, zero blocking violations, 2.34 s. |
| Documentation relative-file links | Passed: 4 documents, 29 targets, zero missing files; anchors not checked. |
| `git diff --check` | Passed. |

No validation was externally blocked. The workspace/capture failures are not
waived or counted as passes, and Clippy remains a pass with workspace warnings.

The capture probe is not counted as a pass. A separate direct `agent script
check` on `cli-attach-capture-smoke.awfagent` rejects a variant domain whose type
is not nominal (`type 12` in that build). The exact relation between that
diagnostic and the run command's stack overflow still needs investigation. This cut
does not change the capture fixture, add an enum-spelling exception, increase
the process stack, or infer successful capture execution from resource success.

This cut changes the private normalized-case selection inside one production
crate. The matching MCP Tier 2 target is rerun. The full Tier 2 sequence is not
repeated: the preceding accepted cut already reached the native observation
fixture's HIR publication failure, and this change touches neither its HIR
publisher nor capture, rendering, resource URI, or subprocess implementation.
Auxiliary capture, visual-golden and proof-boundary tiers remain outstanding
for the full convergence goal; no success is inferred from their omission.

## Ownership review

The source pin is the inspected base plus this cut's working changes. The
canonical audit reports no manifest, feature, or dependency edge change.
Runtime-plan's normal workspace fan-in/fan-out remains 5/9; development is 5/1.

The normalized type owns its borrowed variant view and exact item/payload
association. Its builtin case validation consumes the core schema; it does not
copy a registry, require a second checked-type model, or traverse source/HIR.
The expression and Flow seed producers continue to consume that one view.
There is no new mutable state, I/O responsibility, persistence boundary,
transport dependency, or facade API. The large semantic-facts owner remains
the accepted-generation vocabulary and atomic admission boundary; this cut
repairs behavior on its owned normalized type rather than creating a parallel
case authority. That is the cohesion disposition for its upper size trigger.

New tests live in a dedicated module for normalized selection instead of
extending the existing large general semantic-fact test module. Their positive
and negative cases follow the same type/case responsibility. No production
owner gains embedded test implementation.

| Owner/path (under `crates/`) | Class | Bytes | Base → current physical LOC | Embedded tests |
| --- | --- | ---: | ---: | ---: |
| `arcweft-runtime-plan/src/semantic_facts.rs` | production | 396400 | 10380 → 10414 | 0 |
| `arcweft-runtime-plan/src/semantic_facts/variant_selection_tests.rs` | test | 5886 | 0 → 189 | 0 |

## Remaining scope

The [convergence goal](2026-09-08-convergence-goal-plan.md) remains active.
Contextual constructor inference, application-specific effect rows, complete
continuation/function-value execution, CharacterDialogue projection, Generic
Match, retained View, task-plan, nominal C1–C6 and scheduler/restore remain
required. Native observation's HIR publication failure also remains open.
This cut implements the existing typed variant contract and introduces no
new language rule, design deviation, or external blocker.
