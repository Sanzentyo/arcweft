# Agent host response admission — 2026-09-09

Inspected base: `99278c225dcd474e51ad17308c4d60745c9707af`, existing `main`,
pushed and clean before this cut. This continues the
[host-call type repair](2026-09-09-host-call-type-authority.md), which moved the
MCP script past AWBC type verification to host response admission.

## Evidence and correction

`CliAgentSession::observation` omitted the required objects array. This is the
deterministic CLI producer used by `arcw agent script run`; it has no displayed
objects and now publishes an explicit empty array. The native bundle producer
already serializes its typed observation report, including objects. The
runner's existing observation validation remains strict. Transport-only mock
envelopes in REPL/MCP tests were inspected but do not cross this runtime-value
admission boundary; they are not evidence of a valid executable observation.

With that field present, the real script reached its second VM step and failed
with `pattern did not match`. Its host call declares
`Result<Observation, AgentError>`, but the runner supplied a bare Observation
record as the complete host-call result. The existing task response path
already constructed a checked Result. Transport success and the declared
language value were being conflated on direct host calls.

`RuntimeCheckedType` now owns complete payload admission and typed Result
construction. `TaskOutcomeContract` delegates to that owner; its duplicated
wrapping and validation are deleted. Agent direct host calls dispatch on their
declared result type before validation: Result contracts construct `Ok(payload)`;
infallible contracts admit the payload directly. Failed Result admission never
retries the bare payload. This also preserves Unit-returning commands such as
checkpoint/note, without an operation-spelling allowlist. Existing host failure
control behavior is unchanged.

The controller outcome error now identifies a request, covering both task and
host-call outcomes. The task-specific getter, path and rejection label are
replaced by the shared checked-type diagnostic contract. This is an unreleased
internal API replacement; no compatibility reader or version change is added.

The direct-observe runtime fixture now models the actual Result signature and
binds through its Ok payload, sharing the same result-pattern fixture as tasks.
Core task tests also reject wrong Ok/Err payload types and bare values supplied
to a Result contract. The CLI source/bundle test checks the empty objects array
and reads the current entry/controller manifest through the bundle codec. Its
old `agent_id` assertion referred to a deleted manifest field and prevented it
from reaching execution; that expectation is replaced with the authored entry
and exact checked controller identity.

After that assertion was corrected, the same test exposed that the Agent build
command still wrote inspection JSON to its `.awfb` output. The product reader
correctly rejected the missing AWFB container magic. The writer now uses the
existing canonical `BundleFormat::Awfb` encoder, like the normal project build;
the test reads it through `from_product_path_slice` before running it. No JSON
fallback is added to product ingress. This completes the producer/consumer
path exercised by the Agent source/bundle/trace smoke.

Canonical decoding then exposed a separate identity/text confusion:
`FlowRuntimeId::from_runtime_contract` reparsed the public label as an authored
Flow ID. A generated controller label contains a reserved runtime-only segment,
so the writer's valid output could not be read. Runtime contract decoding now
validates the runtime identity through its existing owner and retains the
separate `RuntimePublicLabel` as text. It does not reconstruct an identity from
that label. The codec regression covers checked and generated controller IDs,
while still rejecting their reserved identity segments at authored/canonical
construction. No wire fields or source-name admission rules change.

## Validation

Logs and command-result JSON are under
`.arcweft-local/validation/2026-09-09-cli-observation-contract/`. The initial
source/bundle smoke failed on the stale `agent_id` assertion. A direct CLI run
with the objects fix then reproduced the bare-response pattern failure. These
failures are retained as evidence, not counted as successful execution.

Cargo uses its normal concurrency. The validation sequence runs commands
serially; Rust is not edited while Cargo or formatting is live.

| Command / scope | Actual result |
| --- | --- |
| `cargo test -p arcweft-agent-runner --lib --quiet` | Passed 62/62 after Result response repair, 41.88 s. |
| Checked/generated Flow identity codec regression | Passed 1/1, 27.74 s. |
| CLI `agent_script_run_json_executes_cli_session_smoke`, `native-capture` | Final run passed 1/1, 61.31 s. Builds and admits AWFB, executes source/bundle, compares replay, reads the trace and checks RAG/privacy behavior. |
| CLI `agent_script_run_admits_infallible_host_result`, `native-capture` | Passed 1/1, 7.96 s. The initial test used the wrong file extension and was rejected before compilation; its corrected `.awfagent` input reaches the Unit host result. |
| `cargo test -p arcweft-core -p arcweft-agent-runner --lib --quiet` | Passed: core 361/361, Agent runner 62/62, 43.93 s. Includes the final codec/Result changes. |
| `cargo fmt --all -- --check` | Passed, 10.23 s. |
| `cargo check --workspace --all-targets --all-features` | Passed, 46.02 s. |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings, 53.33 s. |
| `just test-workspace` | Failed: 851 passed / 18 failed / 0 ignored across 83 result reports, 366.62 s. The callable execution target retains its 53 passes and 18 known failures; later workspace targets and the recipe's CLI commands were not run. |
| `just test-tier2` | Failed overall, 13.85 s. MCP passed 4/4, including the trace resource test. The next native observation test failed 0/1 with `hir.lower.project_publish` before capture. Auxiliary capture, visual-golden and proof-boundary commands were not run after the fail-fast stop. |
| `just test-doc` | Passed: 95 result reports, 8 executed doctests, 50.68 s. |
| Canonical structural audit with `--fail-on-blocking` | Passed: 95 packages, 2,239 Rust files, 309 review triggers, zero blocking violations, 7.17 s. |
| Documentation relative-file links | Passed: 4 documents, 29 targets, zero missing files; anchors not checked. |
| `git diff --check` | Passed. |

MCP now reads the real script trace successfully; this supersedes the MCP
failure status in the preceding host-call record. It does not establish native
capture success: `agent_observe_writes_layer_png_and_object_raw_images` reports
that a staged typed source-component index failed final HIR publication. Its
source fixture is in `check/agent_observe_native/native_vertical.rs`; this cut
does not modify that fixture or the HIR publisher. No validation was externally
blocked. Clippy still reports workspace warnings; no warning-free claim is made.

The expanded source/bundle/trace smoke and checked/generated Flow codec test
now pass. A separate resource/attach probe fails earlier, while lowering a Try
residual containing `Agent(Resource)`; it never reaches host response admission.
That runtime-type projection gap remains required callable work. A dedicated
checkpoint source test exercises the infallible host-result path independently.

Read-only follow-up located the Try failure in
`RuntimeNormalizedType::variant_selection`: it calls `checked_type` on the full
owner and selected payload. That restricted projection rejects Agent shapes,
although the normalized type already carries its plan identity and case payload.
Both expression and Flow Try lowering consume this selection through
`final_variant.rs`. Repair must retain exact case/tuple/type checks at their
owning boundary rather than add a Resource-specific exception. No change to
those sources is included in this cut.

The subsequent [normalized variant selection repair](2026-09-09-normalized-variant-selection.md)
removes that restricted projection, retains exact payload identities, and
passes the resource/attach/checkpoint execution test. It separately records
the remaining capture failure; resource success is not capture success.

## Ownership review

The inspected base plus these working changes is the source pin. There is no
Cargo manifest, feature or dependency edge change. Core remains Sans I/O.

`RuntimeCheckedType` owns value predicates and now constructs the complete
payload that those predicates admit. The task outcome owner delegates to it;
this deletes its separate validation and result-construction algorithm. The
plan owner decodes a Flow contract with the existing identity parser and the
public-label type, without source-name reconstruction. Step input documentation
states the same complete-value requirement. These cohesive core boundaries
gain no host I/O, extra schema, parallel registry or source traversal.

The Agent runner owns protocol-response adaptation and the existing controller
step loop. It now uses the declared result type for both fallible and direct
results, retaining the host request identity in failures. Its error owner
reports that common contract instead of assuming every outcome is a task.
The CLI owner already coordinates Agent compilation, artifact writing and
session observations. It uses the canonical bundle encoder and emits the
required empty observation table without introducing another codec or reader.

Core's pattern owner is above the upper production threshold; plan/task and
the CLI script owner are above the ordinary threshold. This cut follows their
existing type, plan admission, task outcome and CLI orchestration boundaries;
it does not merge unrelated mutable state or I/O responsibilities. No API was
widened to support a physical file split. The large AWBC, Agent runner and CLI
test modules retain shared fixtures for their existing codec/controller/CLI
seams. New coverage exercises those seams, with no embedded test body added to
a production owner. These are the cohesion dispositions for the touched size
and test review triggers.

Existing embedded tests remain with their admission owners: pattern/type and
field identity checks, task producer/catalog identities, and CLI removed-role
rejection/persisted assertion reporting. They are unchanged by this cut; new
outcome and codec execution coverage lives in the dedicated test modules.

| Owner/path (under `crates/`) | Class | Bytes | Base → current physical LOC | Embedded tests |
| --- | --- | ---: | ---: | ---: |
| `arcweft-agent-runner/src/error.rs` | production | 15351 | 417 → 417 | 0 |
| `arcweft-agent-runner/src/runner.rs` | production | 35977 | 905 → 920 | 0 |
| `arcweft-agent-runner/src/tests.rs` | test | 128483 | 3547 → 3551 | 0 |
| `arcweft-cli/src/app/agent/script.rs` | production | 79576 | 2265 → 2266 | 77 |
| `arcweft-cli/tests/check/agent_script_debug.rs` | test | 170942 | 4536 → 4586 | 0 |
| `arcweft-core/src/awbc/tests.rs` | test | 179054 | 5025 → 5029 | 0 |
| `arcweft-core/src/pattern.rs` | production | 105972 | 2905 → 2930 | 630 |
| `arcweft-core/src/plan.rs` | production | 49603 | 1402 → 1405 | 0 |
| `arcweft-core/src/step.rs` | production | 23573 | 670 → 675 | 0 |
| `arcweft-core/src/task.rs` | production | 45310 | 1568 → 1548 | 100 |
| `arcweft-core/src/tests/task.rs` | test | 7589 | 224 → 231 | 0 |

The current Cargo metadata graph retains these fan-in/fan-out values:

| Crate | Normal in/out | Development in/out |
| --- | ---: | ---: |
| arcweft-core | 29/6 | 3/6 |
| arcweft-agent-runner | 4/4 | 1/4 |
| arcweft-cli | 0/53 | 0/3 |

## Remaining scope

The [convergence goal](2026-09-08-convergence-goal-plan.md) remains active.
Constructor inference, higher-order effects, continuation value execution,
CharacterDialogue projection, Generic Match, retained View, task-plan,
nominal C1–C6, and scheduler/restore remain required work. This cut preserves the
existing Agent protocol-record carrier predicate; it does not claim completion
of the separately pending nominal/program-bound restore contracts. There is no
external blocker or new design deviation from the declared Result contract.
