# Host-call type authority — 2026-09-09

Inspected base: `743b8e67d8749edbf5311a489f36b49d8c7bf7a2`, existing `main`,
pushed and clean before this cut. The preceding
[Flow effect work](2026-09-09-flow-effect-publication.md) exposed the next
MCP script failure: AWBC pattern binding expected artifact-local type 14 but
received type 21.

## Evidence and chosen boundary

Extending the existing selected-Agent-controller test to verify its AWBC
reproduced the failure. Inspecting the unverified emission for diagnosis showed
two `Result<Observation, AgentError>` shapes whose Observation leaves had
different semantic identities. This diagnostic emission still explicitly ran
the verifier and failed; it was never accepted as executable evidence. The
temporary dump is removed from the final test, which uses the default verified
lowerer and executes the VM to its host boundary.

The host inventory converted the already admitted result through
`RuntimePlan::checked_type`, then re-interned the reduced `RuntimeCheckedType`
shape. That projection cannot recover every original semantic identity.
Host signatures now reference the preflighted type IDs for all argument
expressions and the result. The erased result conversion and dynamic argument
signatures are removed. No Agent spelling, nominal type, or verifier exception
is introduced.

The host descriptor itself is now the typed interning key. Its existing fields
own the public identity, capability, operation, contract, signature, mode,
determinism, and argument naming/spread metadata. Operand values remain at each
call instruction; different values no longer manufacture duplicate descriptor
rows through debug-string keys. `AwbcHostCall` and `AwbcHostArgument` support
structural ordering for this deterministic build-time index. The wire shape and
contract versions are unchanged.

The remaining checked-type inspection sites were inspected. Constant-choice
selection classifies values without creating a new type; return-shape checking
classifies Unit. The remaining standalone `intern_runtime_type` consumer is
the separately owned `TaskOutcomeContract` payload. It does not start from a
RuntimePlan type ID. This host cut does not invent a plan identity for that
contract or claim completion of the pending task-plan/nominal work.

## Validation

Logs and result JSON are under
`.arcweft-local/validation/2026-09-09-host-result-abi/`. Cargo used its normal
concurrency; commands ran sequentially, and Rust was not edited during them.

| Command / scope | Actual result |
| --- | --- |
| Initial selected-Agent-controller AWBC regression | Failed: 0 passed / 1 failed, preserving the pattern-binding type mismatch. |
| `cargo test -p arcweft-runtime-plan --lib host_ -- --nocapture` | Final focused run passed: 3/3, including exact immediate/suspended signatures, codec round trip, and two suspended calls sharing one descriptor while retaining distinct values. |
| `cargo test -p arcweft-compiler --lib --quiet` | Passed: 82/82, including selected Agent native and verified AWBC execution to the host boundary. |
| `cargo fmt --all -- --check` | Passed, 10.17 s. |
| `cargo check --workspace --all-targets --all-features` | Passed, 64.53 s. |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings, 24.06 s. |
| `cargo test -p arcweft-core -p arcweft-runtime-plan --lib --quiet` | Passed: core 361/361 and runtime-plan 60/60, 52.07 s. |
| `just test-workspace` | Failed: 851 passed / 18 failed / 0 ignored across 83 result reports, 430.09 s. The callable execution target contains the same 53 passes and 18 known failures as the preceding cut. Later workspace targets and the recipe's CLI commands were not run after its fail-fast stop. |
| `just test-tier2` | Failed in the first MCP command: 3 passed / 1 failed / 418 filtered, 118.37 s. The script passes AWBC verification but its CLI session response omits the required `observation.payload.objects` array. Subsequent Tier 2 commands were not run. |
| `just test-doc` | Passed: 95 result reports, 8 executed doctests, 51.39 s. |
| Canonical structural audit with `--fail-on-blocking` | Passed: 95 packages, 2,239 Rust files, 309 review triggers, zero blocking violations, 3.38 s. |
| Documentation relative-file links | Passed: 4 documents, 59 targets, zero missing files; anchors not checked. |
| `git diff --check` | Passed. |

The initial host-signature/codec and exact Agent regression each passed 1/1
before the typed descriptor key was added. While adding the sharing/resume
test, its first version called `set_register` on `FiberState` instead of its
active `FiberFrame`; that test compile error also stopped an intermediate
Clippy run. Both failed logs are retained. The final focused, check, Clippy,
and library runs above include the corrected test. No compiler error or
intermediate failure is counted as a pass.

MCP reached host-response admission after the repaired type boundary. Its
remaining failure comes from `CliAgentSession::observation`, the deterministic
CLI session producer used by the real command, rather than a malformed fixture
inside this test. Native bundle observation serializes its report, which owns
the objects field. Correcting the CLI producer is the next independent cut;
the admitted host type or strict response validator must not be weakened.
This cut establishes verified Agent host-call execution, not successful MCP
trace completion. No validation was externally blocked.

The subsequent [Agent host response and bundle repair](2026-09-09-agent-host-response-admission.md)
completes the source/bundle/trace smoke and passes all four MCP tests. It also
records the separately remaining Try projection and native HIR publication
failures; the preceding failed results remain historical evidence.

## Ownership review

The source pin is the inspected base plus this cut's working changes. No Cargo
manifest, feature, dependency edge, I/O boundary, or wire field changed.

The core schema owns the host descriptor algebra; deriving structural ordering
on that row supplies the existing inventory with a deterministic typed key.
The inventory owns table admission and interning. It now consumes the already
preflighted type graph instead of constructing another identity from a reduced
shape. Flow lowering remains the sole host-call producer and consumes the same
inventory. Its method no longer accepts a redundant RuntimePlan argument and
is narrowed to crate visibility. These changes add no independent registry,
second type reader, transport state, persistence state, or public facade.

The schema and Flow lowerer exceed the upper size threshold, and the inventory
exceeds the ordinary production threshold. Their responsibilities remain
cohesive for this cut: the schema holds the wire algebra, the inventory holds
table interning, and the Flow lowerer emits the instruction stream. Splitting
the host row's ordering or type admission into a second owner would fragment
that boundary. This is the disposition for the touched triggers, not a claim
that all large repository owners have been decomposed.

Both test modules reuse their owning boundary's fixtures. The runtime-plan
tests inspect exact semantic IDs and execute/resume the VM; the compiler test
selects the exact admitted entry target and compares native/AWBC host behavior.
No production owner gains an embedded test module.

| Owner/path (under `crates/`) | Class | Bytes | Base → current physical LOC | Embedded tests |
| --- | --- | ---: | ---: | ---: |
| `arcweft-core/src/awbc/schema.rs` | production | 92112 | 3040 → 3040 | 0 |
| `arcweft-runtime-plan/src/awbc_lower/inventory.rs` | production | 86790 | 2170 → 2158 | 0 |
| `arcweft-runtime-plan/src/awbc_lower/flow.rs` | production | 126847 | 3248 → 3247 | 0 |
| `arcweft-runtime-plan/src/awbc_lower/tests.rs` | test | 40942 | 1033 → 1161 | 0 |
| `arcweft-compiler/src/project/entry_tests.rs` | test | 36253 | 1112 → 1148 | 0 |

Dependency fan-in/fan-out from the current Cargo metadata graph:

| Crate | Normal in/out | Development in/out |
| --- | ---: | ---: |
| arcweft-core | 29/6 | 3/6 |
| arcweft-runtime-plan | 5/9 | 5/1 |
| arcweft-compiler | 3/23 | 1/5 |

## Remaining scope

The known constructor inference, higher-order effect rows, continuation value
execution, and CharacterDialogue projection failures remain required work in
the [active callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md).
Generic Match, retained View, task-plan, nominal C1–C6, and scheduler/restore
remain in the [convergence goal](2026-09-08-convergence-goal-plan.md).
No external blocker or compatibility exception was introduced. This host
boundary repair does not mark those contracts or the overall goal complete.
The maintained runtime chapter records the type-identity and descriptor
interning rule; the verifier and language contracts are preserved.
