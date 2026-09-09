# Agent enum case authority — 2026-09-09

Inspected base: `417cfe2b6eaa188c873fa0efa0922925eed5b0a0`, existing `main`,
clean and equal to `origin/main` before this cut. This record describes the
validated working-tree changes at that base. Supersedes the capture type
failure's diagnosis in the
[normalized variant selection record](2026-09-09-normalized-variant-selection.md).

## Defect and authority

The checked CaptureFormat/CaptureKind/PointerButton inventories described
closed enums, but their normalized types projected to operational Agent DTO
leaves. Variant facts independently supplied nominal case domains. The plan
builder correctly rejected their mismatched owner. Direct capture script check
failed with `type 12 is not a nominal owner for a variant domain`.

The same mismatch reproduced through a function argument and exhaustive Match
in both native and AWBC regression tests. Moving the enums to the existing core
builtin variant schema exposed separate incomplete case readers in plan
construction, native evaluation, runtime patterns, and AWBC case naming.

`RuntimePlan::variant_case` now returns a borrowed case view from the admitted
type and nominal domain. The builder uses the same declaration-owned resolver.
It retains the value owner, canonical name and exact payload type ID for every
plan variant family. Construction, literal admission, runtime value checking,
pattern validation, native/pure evaluation and AWBC naming consume it. The
duplicated case-selection matches and ordinal/name tables are deleted.
Resolution failures preserve typed unknown-type, wrong-kind, missing-domain
and unknown-case causes.

CaptureFormat, CaptureKind and PointerButton join AgentBinaryEncoding in the
existing core builtin registry. Sema's Agent type vocabulary owns its mapping
to that registry. Standard source case inventories and ownership admission
derive from the registry, and compiler normalized types retain those builtin
identities. The three obsolete operational type leaves, their projections and
wire decoders are removed. Contract versions remain 1; no compatibility reader
or source-label reconstruction is added.

Runner request conversion consumes semantic case identities for capture
format/kind and pointer buttons. It rejects strings, foreign owners, forged
names/ordinals and invalid payloads. The generic runtime string reader no
longer interprets enum cases. Existing external command-label parsing remains
at its separate protocol input boundary.

## Validation collected

Logs and command timing records are local and ignored under
`.arcweft-local/validation/2026-09-09-agent-capture-values/`.

| Command or evidence | Result |
| --- | --- |
| Compiler enum execution baseline | 0 passed / 2 failed at nominal-domain admission |
| `cargo test -p arcweft-compiler --test agent_enum_execution --quiet` | 2 passed; all 8 unit cases across 4 enums, native and AWBC function arguments/Match; canonical AWBC encode/decode; 98.05 s including rebuild |
| `cargo test -p arcweft-core -p arcweft-runtime-plan -p arcweft-agent-runner --lib --quiet` | 361 + 64 + 65 = 490 passed; 35.19 s |
| `arcw agent script check samples/agent-script/cli-attach-capture-smoke.awfagent --json` | Passed, `ok: true`, one Agent entry |
| `cargo test -p arcweft-cli --test check agent_script_run_persists_attach_capture_debug_record -- --exact` | 0 passed / 1 failed, source subprocess stack overflow; the earlier module-qualified filter selected 0 tests and is not a pass |
| `cargo test -p arcweft-lang-sema --lib agent_builtin_ --quiet` | 3 passed; 68.62 s including rebuild |
| Sema exact `env::tests::standard_closed_enum_inventories_preserve_owner_authored_order` | 1 passed; 0.57 s |
| Compiler `try_pipe`, `evaluated_effects`, `flow_effects` integration tests | 22 passed; 46.88 s |
| `cargo fmt --all` | Passed; 10.35 s |
| `cargo check --workspace --all-targets --all-features` | Passed with existing warnings; 66.38 s |
| `cargo clippy --workspace --all-targets --all-features` | Passed with existing warnings; 77.68 s |
| `just test-workspace` | Failed: 856 passed / the same 18 callable failures, 84 test reports; 499.70 s. The recipe stopped in compiler `callable_execution`; later workspace/CLI commands were not run |
| `just test-tier2` | MCP 4 passed, native observe 0 passed / 1 failed at the previously recorded `hir.lower.project_publish` source-component admission; 83.85 s. Later native auxiliary, visual-golden and proof targets were not run |
| `just test-doc` | 95 reports, 8 doctests passed; 58.19 s |
| Structural audit with `--fail-on-blocking` | 95 workspace packages, 2,243 Rust files, 309 review triggers, 0 blocking violations; 5.99 s |
| Documentation links and `git diff --check` | Passed; 3 changed documents, 26 local links; anchors not checked |

Intermediate failed commands are retained. The new test initially used an
unsupported Rust integer conversion and a reserved Arcweft function name;
both fixture errors were corrected before the domain baseline. The first
schema edit required named static arrays, and a removed import was restored.
Later behavior failures identified the separate case readers described above;
none were bypassed or converted into success branches.

## Stack overflow and remaining scope

The original direct source `run` command also exited with Windows stack
overflow before producing a compile diagnostic. LLDB on the pre-cut CLI located
it during sema's selected-call publication, inside BTreeMap insertion of a
large PreparedCallNode. Selected frame allocations were 6,312 bytes for
`slice_insert`, 23,328 bytes for `insert_recursing`, and 24,480 bytes for
`seal_selected_application`. This is measured stack pressure during source
analysis, before renderer or host capture execution. No stack-size override or
production workaround was applied in this enum cut. LLDB initially lacked its
Python DLL; `uv python find 3.11` found the already-installed compatible runtime
and only the debugger process environment was adjusted.

The broader goal remains active. Callable inference/effects and execution,
the native HIR publication failure and all later Match/View/task-plan/nominal/
scheduler acceptance remain required. This record does not award completion
for those boundaries or for unexecuted capture tiers.

## Structure

The audit ran on the above base with this cut dirty. Its generated reports are
in the local validation directory. These are the touched review triggers and
new modules; paths below are relative to `crates/`.

| File | Classification | Base → current LOC | Bytes | Embedded test LOC |
| --- | --- | ---: | ---: | ---: |
| `arcweft-compiler/src/lower.rs` | production | 7,855 → 7,855 | 331,975 | 0 |
| `arcweft-core/src/engine/eval.rs` | production | 1,349 → 1,304 | 50,822 | 124 |
| `arcweft-core/src/pattern.rs` | production | 2,930 → 2,835 | 103,131 | 633 |
| `arcweft-core/src/plan.rs` | production | 1,405 → 1,407 | 49,698 | 0 |
| `arcweft-core/src/plan/construction.rs` | production | 2,756 → 2,753 | 109,280 | 256 |
| `arcweft-core/src/plan/construction/lower.rs` | production | 5,361 → 5,248 | 217,050 | 208 |
| `arcweft-core/src/plan/variant_case.rs` | production | 0 → 136 | 5,187 | 0 |
| `arcweft-core/src/pure.rs` | production | 3,067 → 2,992 | 111,105 | 132 |
| `arcweft-core/src/value.rs` | production | 3,800 → 3,798 | 135,199 | 0 |
| `arcweft-lang-sema/src/env/base.rs` | production | 2,274 → 2,256 | 82,175 | 0 |
| `arcweft-lang-sema/src/ownership.rs` | production | 2,266 → 2,281 | 89,191 | 311 |
| `arcweft-lang-sema/src/types.rs` | production | 1,718 → 1,760 | 58,025 | 108 |
| `arcweft-runtime-plan/src/semantic_facts.rs` | production | 10,414 → 10,402 | 395,731 | 0 |
| `arcweft-runtime-plan/src/semantic_facts/tests.rs` | test | 2,865 → 2,853 | 103,331 | 0 |
| `arcweft-agent-runner/src/runtime_args/tests.rs` | test | 0 → 138 | 5,084 | 0 |
| `arcweft-compiler/tests/agent_enum_execution.rs` | test | 0 → 45 | 1,378 | 0 |

Normal dependency fan-in/fan-out is core 29/6, sema 8/14, runtime-plan 5/9,
compiler 3/23 and Agent runner 4/4. No dependency, Cargo feature, crate or
cross-layer direction changes were introduced.

The new plan case module is the decomposition along the actual shared schema
boundary. It owns a borrowed projection and typed errors, with no persisted
side table. The aggregate plan/builder still owns type and nominal-domain
admission. Native and pure evaluation retain their respective execution state;
their case tables are deleted. `pattern.rs` retains checked-value schemas,
pattern validation and recursive value admission, with all plan case lookup
delegated. Its existing schema/admission tests stay with that responsibility.
`value.rs` only changes the evaluation error carrier.

Compiler lowering and normalized semantic facts remain the two existing
source-to-plan projection owners; their obsolete DTO cases are removed. The
sema type vocabulary owns enum classification, its environment owns source
registration, and its ownership classifier owns snapshot-value admission.
These are coherent existing boundaries, not new unrelated state clusters.
The large normalized-type test module only loses obsolete DTO cases; new
execution/request tests are separately scoped to compiler integration and
runner conversion. These dispositions justify retaining the touched larger
owners without splitting their remaining cohesive algorithms for a LOC target.

No design deviation was introduced. Broad validation failures remain explicit
goal work and were not treated as passing gates.
