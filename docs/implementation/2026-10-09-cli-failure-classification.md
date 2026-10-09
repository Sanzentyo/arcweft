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

## Current central evidence and repair ownership

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
