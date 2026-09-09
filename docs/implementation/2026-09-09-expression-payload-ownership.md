# Expression and candidate payload ownership — 2026-09-09

Inspected base: `314b33d0c8c71215f7759542c122df416b90f826`, existing `main`,
pushed and clean before this cut. The following evidence describes this dirty
cut. Supersedes the unresolved nested-Content stack overflow in the
[frame timing record](2026-09-09-frame-time-sampling.md); its other remaining
acceptance items remain required.

## Cause and ownership

The native Ruby/typewriter tests exhausted the Windows main-thread stack while
checking nested Content and ordinary Fx calls, before renderer construction.
The initial trace stopped at `prepare_content_call_application` entry. Its
70,256-byte local frame and the dialogue dispatcher's 28,104-byte frame were
repeated through ordinary expression evaluation. Expression payloads alone
occupied 848 bytes in both the prepared and complete carriers.

`CheckedExpression` now owns one private heap payload containing its result,
effects, resolution, execution plan, Match evidence and nested-path evidence.
All prepared expression variants own their payloads on the heap as well; the
complete variant uses the allocation already owned by `CheckedExpression`.
Typed `From` implementations on the prepared fact perform construction for
every prepared family. Fact storage, implicit callable bodies, scalar source
provenance, candidate journals, Content extraction and final sealing all move
the same owners. Redundant boxes around already compact facts are removed.

The solved candidate and prepared application transaction each own one heap
payload. `CandidateSemanticProjection` owns its complete graph delta, fact
maps, capture ordering and physical evaluation transcript on the heap. Success,
rejection, replay, application failure and transfer carry that same affine
projection. The former extra boxes on extraction/failure are removed; a caller
does not unbox and rebox the whole projection between phases.

This retains independent cloned expression values and move-only candidate
projections. It does not add shared mutable state, reconstruct a discarded
candidate, change selection order or weaken checkpoint/replay validation.
The existing complete and prepared semantic roles remain distinct. No source
special case, stack-limit increase, larger worker-thread stack, optimization
change, Cargo job count, dependency or contract version is introduced.

## Validation

Logs and diagnostic artifacts remain local and ignored under
`.arcweft-local/validation/2026-09-09-content-evaluation-storage/`.

A temporary Rust layout probe measured prepared expression owners at 16 bytes
and checked expression owners at 8 bytes after migration, compared with 848
bytes each before it. These are inline owner sizes, not total memory usage.
The payload remains present on the heap. The temporary probe was removed before
final tests; it is diagnostic evidence, not a size-threshold acceptance test.

The first expression-only pass still overflowed while probing a nested scalar
argument. LLDB frame-register measurements identified remaining candidate
preparation and transaction frames of 45,744 and 24,400 bytes. The final
transaction/projection ownership migration then allowed all eight CLI capture
subprocesses in the four existing Ruby/typewriter tests to finish successfully.
Those tests now fail at their old metadata assertion:
`assert_typed_typewriter_fx_application` expects a `rich_text.typewriter.*`
function spelling, while the typed builtin emits its specialized definition
identity. The helper also still expects the replaced `parameters` shape.
These are recorded failures; the four tests are not counted as passes.

Direct native mask capture of the same vertical Ruby/typewriter source at
0 and 4 seconds succeeded. Both crops are 39x101 at viewport origin
(1114, 518). The first has zero covered pixels; the second has 867. Counts
match the raw image alpha bytes, and the crop geometry is unchanged.

| Command | Result |
| --- | --- |
| `cargo check -p arcweft-lang-sema --all-targets` | Passed; final 14.66 s. Intermediate checks exposed and then closed the owned-payload consumer mismatches |
| `cargo test -p arcweft-lang-sema --lib --quiet` | 760 passed / the same 7 generic inference/effect failures; final 36.67 s including rebuild |
| `cargo test -p arcweft-cli --features native-capture --test check typewriter_ruby_capture_time_controls_ -- --nocapture` | Final 0 passed / 4 failed on stale Fx metadata assertions, after successful captures; 67.00 s. The preceding expression-only pass still had 4 stack failures, 87.24 s |
| `cargo test -p arcweft-cli --test check agent_script_run_persists_attach_capture_debug_record -- --exact` | 1 passed; 1.18 s |
| Direct native Ruby mask at 0/4 seconds | Both commands and alpha/coverage/geometry checks passed, as detailed above |
| `cargo fmt --all` and final `cargo fmt -p arcweft-lang-sema` | Passed; first 10.45 s, final formatter untimed |
| `cargo check --workspace --all-targets --all-features` | Passed with existing warnings; 39.16 s |
| `cargo clippy --workspace --all-targets --all-features` | Passed with existing warnings; 43.78 s |
| `just test-workspace` | 856 passed / the same 18 callable failures in 84 reports; 279.32 s. The recipe stopped in compiler `callable_execution`, so later workspace/CLI targets did not run |
| `just test-doc` | 95 reports / 8 passed, no failures; 62.44 s |
| `just test-slow-mcp` | 4 passed; 3.31 s |
| Canonical structural audit with `--fail-on-blocking` | 95 packages, 2,250 Rust files, 309 review triggers, 0 blocking violations; 6.54 s |
| Documentation links and `git diff --check` | Passed; 2 documents / 33 local link targets, anchors not checked |

This cut changes private storage within sema and the representation of its
opaque checked-expression owner. It does not change a public method signature
or render contract. The matching native capture and MCP paths were selected;
the exhaustive auxiliary/visual-golden/proof Tier 2 matrix is not repeated for
this storage correction. The preceding cut's image-animation parse failure
still prevents exhaustive Tier 2 completion. No test expectation is weakened
to turn the known failures into passes.

## Structure and cohesion review

All measured files belong to `arcweft-lang-sema` and have production
classification. Paths below are relative to its `src/final_analysis/`.
Measurements are final dirty-checkout values on the base above.

| Path | Base → final LOC | Bytes | Embedded test LOC |
| --- | --- | --- | --- |
| `analyzer/call_seal.rs` | 2015 → 2015 | 88105 | 0 |
| `analyzer/calls/constraints.rs` | 4921 → 4940 | 201838 | 480 |
| `analyzer/calls.rs` | 4304 → 4304 | 186452 | 185 |
| `analyzer/dialogue_line_plan.rs` | 1669 → 1669 | 72690 | 66 |
| `analyzer/evaluated_effects.rs` | 1890 → 1890 | 86067 | 0 |
| `analyzer/expressions.rs` | 4037 → 4037 | 175676 | 88 |
| `analyzer/state.rs` | 3064 → 3080 | 116768 | 503 |
| `analyzer/text_proxy.rs` | 777 → 777 | 38654 | 0 |
| `model.rs` | 2794 → 2807 | 91614 | 0 |
| `nominal_schema.rs` | 2946 → 2946 | 120500 | 0 |
| `prepared.rs` | 1511 → 1566 | 51244 | 0 |

Normal dependency fan-in/out is 8/14; development fan-in/out is 3/0. No package,
dependency edge, feature, I/O owner or cross-layer export is added. The new
payload structs are private and do not expose fields to enable file splitting.

The touched size triggers retain these cohesive owners:

- `prepared.rs` owns the complete pre-seal expression family and its typed
  conversions; `model.rs` owns the opaque completed fact. Payload indirection
  changes ownership transport while retaining all semantic atoms and clone
  independence. There is no second expression authority.
- `state.rs` owns the affine journal, projection/replay checks and transfer.
  Its projection payload keeps the same issuer, epoch, graph, facts and
  transcript together. Extraction and failure paths consume or return that
  complete owner; the existing state tests follow the same rollback boundary.
- `calls/constraints.rs` owns the analyzer/constraint-machine seam and the
  solved/prepared transactions. `calls.rs` owns candidate orchestration and
  selection; `call_seal.rs` owns final correlated publication. They now pass
  compact owners through the existing phase boundaries. Neither callback order
  nor the lower solver algebra changes, and their tests retain those scopes.
- `expressions.rs` retains expectation-aware dispatch and implicit callable
  body preparation; `dialogue_line_plan.rs` retains Content/Dialogue preparation;
  `evaluated_effects.rs` retains final effect/Content sealing. These edits only
  construct or consume the new storage representation at their existing typed
  seams, adding no source-specific branch or unrelated state cluster.
- `nominal_schema.rs` retains nominal-coordinate sealing, including scalar
  provenance. Its conversion consumes the same original prepared fact after
  removal of the redundant box. `text_proxy.rs` remains the typed scalar
  admission consumer below the size threshold.

These are cohesion dispositions for the touched review triggers, not claims
that large modules are generally exempt from future ownership review. There
is no independent blocking structural finding in this cut. Generated metrics
remain in the local validation directory.

## Remaining acceptance

The stale typed-Fx test helper will be migrated to the current owning API in
its own test correction. The image-animation sample parse failure, recovered
closure candidate publication, sema 7/callable 18 failures, and the full
Match/View/task-plan/nominal/scheduler sequence remain part of the
[active goal](2026-09-08-convergence-goal-plan.md).

This storage correction is not a global source-depth or native-stack bound.
The goal's complete traversal/accounting requirements remain in force. There
is no design deviation and no external blocker.
