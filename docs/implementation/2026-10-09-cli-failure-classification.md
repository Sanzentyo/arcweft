# CLI failure classification and repair — 2026-10-09

Inspected `main` at full Git SHA
`df6b213f1cb31d7db5ce878501e4a24371c6987c`. The working tree contained the
root agent's 13 frozen Core codec/preflight files and the existing convergence
goal note. Those files are excluded from this agent's edits. Subsequent
concurrent CLI test edits belong to the three explicitly assigned agents.

The exact CLI baseline ran 605 tests: **328 passed, 277 failed, 23 skipped**.
It is retained in
`C:/Users/sanze/AppData/Local/Temp/arcweft-1009-task-preflight-cli.log`, with
full failure diagnostics in
`C:/Users/sanze/AppData/Local/Temp/arcweft-1009-codec-preflight-cli-failure-diagnostics.json`.

The [machine-readable inventory](2026-10-09-cli-failure-classification.json)
contains every distinct baseline ID, diagnostic owner, repair owner, observed
failure envelope, typed root evidence, repair direction, status and validation.
It records the diagnostic file's SHA-256. `CLI001` through `CLI277` follow the
retained diagnostic array order; the binary and full test name are the test
identity. No baseline failure is excluded. Unknown causes remain explicitly
`pending_typed_owner_trace`; a matching error string is not root-cause evidence.

## Historical initial ownership and execution state

| Assigned test owner | Failures | Files |
| --- | ---: | --- |
| `cli_observe_repair` | 166 | `agent_observe_native/` test family and assigned presentation samples |
| `cli_bench_repair` | 98 | `cli_runtime_bench.rs` and assigned bench/spec fixtures |
| `cli_remaining_classify` | 13 | `agent_script_debug.rs` (8), `profile_entry_selection.rs` (1), `toolchain_jit.rs` (4) |

The root agent runs Cargo, nextest, graph/selection checks, commit and push.
During the existing full reverse-dependency nextest execution, only assigned CLI
test files and this report may change. Shared samples and production sources
remain held until the root sends the terminal clearance. This is a validation
input freeze, not a request for another user approval.

At this initial checkpoint, new repair evidence was **not run**. Subsequent
central results are recorded below; applied patches alone do not establish
passing behavior.

## Observed first-failure envelopes

These counts describe where the baseline stopped; they do not claim all members
share one root cause. In particular, final semantic analysis includes stale
Flow result contracts, nominal primitive spellings, and distinct unresolved
owner paths. Later failures may become observable after the first is repaired.

| Envelope | Count |
| --- | ---: |
| Explicit source entry required | 101 |
| Final semantic analysis | 70 |
| Assertion or other contract mismatch | 56 |
| Syntax rejection | 15 |
| HIR recovery | 13 |
| Callable signature viability | 4 |
| Removed CLI argument | 4 |
| HIR arena publication | 3 |
| Rich-text owner origin | 3 |
| AWBC empty source map | 3 |
| HIR component publication | 1 |
| CharacterDialogue custom field | 1 |
| Unknown method target | 1 |
| Source JIT candidate | 1 |
| Missing fixture | 1 |
| **Total** | **277** |

## Traced contract migrations

- Source launch commands must select an authored typed entry. Observe fixtures
  still omit `--entry`; the repair belongs to the common command producer and
  matching fixture declarations, preserving capture behavior.
- An omitted Flow return annotation is `OmittedUnit`. The choice-dispatch
  sample's terminal Flows return `String`, the CLI profile returns
  `Vec<String>`, and the runtime JIT/AOT fixtures return `i64`, `i32` or
  `String`. They must declare those actual result contracts. Empty effects
  express purity; source `#[pure]` metadata does not replace the typed row.
- `AgentResource.body` is the typed `Json`, `Text` or `BytesBase64` carrier.
  The metadata smoke source's `resource.body.kind` record projection is stale.
  Match the body carrier and preserve URI/kind/MIME/hash/content assertions in
  the executed controller.
- An Agent controller completes with `Result<Unit, AgentError>`. Raw resource
  JSON and `record/2` final-status expectations are stale. Resource content,
  response and trace checks remain, followed by success-carrier completion.
- The persisted Agent project index contains the authored Agent entry and
  controller; old empty entity/graph expectations omit accepted facts.
- The debug graph fixture seeds `uses_view`, while its RAG query still asks for
  `uses_dialogue_view`. Query the seeded relation and retain depth-two, history,
  diagnostic and test-result retrieval checks.
- The profile source fixture's initializer and reducer contain unconditional
  self-recursion, and its bench references an absent GameState fixture/helper.
  Use executable state/reducer values and measure the selected authored Flow.

## Production defects identified separately

1. **Source JIT and runtime acceleration consume an obsolete inventory.**
   `crates/arcweft-cli/src/app/jit.rs::jit_check_source_target` reads
   `plan.pure_helpers()`. Current `final_flow` lowering emits ordinary pure
   functions as admitted `RuntimeFunctionSite` expression bodies; it creates no
   source helper row. The same source-site authority must reach VM/AOT/native
   requests and runtime/batch acceleration. Copying source bodies into a second
   helper authority or changing acceleration expectations to zero is not a
   repair. The remaining agent owns this complete production boundary.
2. **Rich-text child ranges lose their owner origin.** Three clear/page capture
   tests reach valid runtime content but report child range `0..1` before owner
   origin `6`. The root owns the typed display-range repair; capture assertions
   remain the acceptance behavior.
3. **Metadata-only AWBC products emit a source map for absent block zero.** Three
   tests/benches fail verifier bounds checks on an empty block inventory. The
   root owns the source-map producer repair; verifier rejection remains intact.

The bench agent separately reported the missing maintained `Traversable`
catalog path and the absence of typed dense Matrix/Tensor source construction
needed by four removed bench `--value` invocations. Their exact per-ID traces
and repairs remain pending integration into the JSON inventory.

## Historical initial applied and deferred work

Initially applied, unvalidated: exact Flow result/effect contracts and explicit runtime
entries in `toolchain_jit.rs`; executable CLI/test/bench profile sources in
`profile_entry_selection.rs`; Agent success-carrier final statuses and the
seeded RAG relation query in `agent_script_debug.rs`.

Initially deferred under the input freeze: the approved choice-dispatch and three
read-resource samples, exact persisted graph metadata expectations, remaining
owners' classifications, the function-site JIT/acceleration implementation,
and all post-change executable checks.

The final acceptance requires actual selected CLI results for the complete
277-ID baseline, meaningful new owner rejection/conformance coverage for
production API changes, the full affected reverse-dependency closure, required
check/Clippy/fmt evidence, and distinct reporting of failures or unavailable
checks. No completion or green result is claimed here.

## Historical central evidence and repair ownership

The latest complete CLI run is `f30a6772-270d-4280-b4d6-0f3fe826a961`:605 executed,399 passed,206 failed,23 skipped in283.648s. Selected IDs have no missing, extra or duplicates, and all frozen Rust inputs were unchanged. Of the original277 failures,71 passed under the same ID,1 obsolete motion fixture migrated to a maintained compile-rejection contract and passed, and205 still failed. Every original ID is accounted for. The additional MCP stdio tail-publication failure belongs to the original605 suite but is outside the original277 failures. Its readiness/collector repair is applied and awaits an actual CLI rerun.

The preceding full run executed605:392 passed,213 failed. Its2 new duplicate-entry failures were repaired and both pass in the latest full run. The JSON retains both full-run histories and exact evidence hashes; targeted passes and owning-crate suites never replace these full CLI outcomes.

Current repair ownership follows the production boundary: root owns field/place inspection, checked availability, Never let-else and source maps; observe owns retained text contexts, range/catalog/capture and Web consumers; remaining owns FunctionSite execution/JIT, continuation and MCP transport; bench owns SystemInfo, dense/traversal, labeled-loop and runtime measurement consumers. Root alone runs Cargo, commits and pushes coherent validated cuts.

Delivered components include Never let-else, named argument paths, code-only AWBC source maps, rejected-call HIR evidence, the owning pure FunctionSite API, scalar JIT Scope lowering, and native SystemInfo request projection. Current compiler/Host/protocol validation executed95:94 passed,1 failed; Host56/56, protocol34/34, compiler Map3 and whole-formal1 passed. The compiler logical-sequence equality regression still rejects an Executable structured.function call in native execution. Core sequence equality tests pass, but that source-level failure remains open. The full CLI has not yet been rerun after these changes.

| Full central run | Passed | Failed | Original failures still failing |
| --- | ---: | ---: | ---: |
| Initial baseline |328|277|277|
| First migration checkpoint |392|213|211|
| Latest complete run |399|206|205|

The next acceptance is the actual compiled CLI suite after the complete range/capture checkpoint freezes. Executable body, source acceleration, Include/Choice, dense/traversal and labeled-loop boundaries remain in progress. After major CLI integration converges, work returns to remaining Generic Match acceptance, retained View and the protected atomic publication of all14 task-plan tables. The original goal is incomplete.

## Latest range and policy integration execution

Full central run `ef8ff299-969f-4589-9584-4d788bf7aea6` executed657 cases in409.732s: CLI608 had407 passes/201 failures, policy14 and protocol35 all passed. All selected IDs were executed exactly once and all3,568 frozen source/fixture/asset inputs were unchanged. The latest complete CLI result is now407/201; targeted repair runs must not replace it.

Of the original277 failures,75 passed under the same ID,1 maintained motion-contract migration passed,200 still failed, and1 obsolete await? contract test was retired. Retirement is not a passing test. Two older content-owner unit tests were replaced when the implementation stopped global speaker/content lookup; direct permutation and duplicate-owner admission replacements are required and await actual execution. Six new CLI IDs are retained explicitly in the JSON.

Actual remaining failures distinguish adapter behavior from execution coverage: SystemInfo completes2 requests within its8 one-operation frames before reaching the third; its thread fixture separately lacks sealed child statement local-use evidence. Resource metadata now reaches strict builtin variant admission, whose compiler producer omitted the required tuple payload wrapper. The MCP diagnostic correctly retains tail-end but the cmd fixture exits on fragmented JSON writes. Clear trims the prepared content while remaining on logical page0; the migrated page1 target was a fixture mistake. These precise repairs are underway with their original semantic, pixel, counter and rejection assertions preserved.

Targeted final correction run `8b821922-25d8-4674-a2c8-2b87c708a751`:80 executed,77 passed,3 failed;3,568 input hashes unchanged. All MCP, Thread borrowing/affine, SystemInfo, timed Clear/page1/Ruby and owner-replacement cases pass. Remaining failures are named AgentResourceBody type annotation resolution and two trace-measured expect host-call counts; this does not update the fullCLI407/201 result.

Focused integration `31e1627e-250e-4e85-924c-3d5ebfbe8d40`:9 run,8 passed/1 failed; resource3, all4 ignored MCP stdio E2E and depth-sorted hit pass. The remaining animation fixture later passes `1806296a-453e-4cd7-9c20-0a6202583ab4` with original bounds and raw-pixel separation assertions. Protocol/MCP complete owner run `4af4ec51-7910-44fb-9197-30f6c7808f82` passes62/62 after the exact issued document/layout/catalog fixture repair. Each has3,568 unchanged input hashes. FullCLI remains407/201 until an actual full rerun.

## Verified owning source deliveries

The named AgentResourceBody native/AWBC argument regression passes with its owning compiler cases (3/3). Sema/compiler transitive reverse dependencies pass across all22 selected packages:2,483/2,483 in sequential Windows/default-feature library runs. Current full Core passes1,114/1,114. Exact run IDs, package counts, log hashes and unchanged3,568-input inventories are retained in the JSON owning history. These results do not replace the latest fullCLI407/201.

Published source cuts are MCP bounded stderr retirement `8ecb4b803d384707bca5ad7cf5b6c57d34e93b9c`, accepted builtin case/payload projection `e6dff79c83bfe6eb112f8af186896b8a0a66ca51`, and spawned Thread body/free capture checking `8ebde5e0bfc3b22ffc8c9c665e06c4f96643179c`. Exact remaining Field/range and CLI fixture changes are still separate WIP. Portable source names, labeled loops, scalar executable bodies and batch/fusion candidates remain unvalidated.

Independent static review found a remaining Field WIP parity gap: native/pure whole-owner lookup refuses a Copy child after a disjoint affine sibling was moved, while AWBC ordinal ReadPlace accepts the initialized child. Existing green suites do not exercise this edge. An owning storage-path correction and native/pure/AWBC regression are pending before the Field source cut can be delivered.
