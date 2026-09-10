# Completed semantic project leases across executable rejection

Date: 2026-09-10. Base: clean `main` at
`f171b4bfcd04b17da51876341a2b511bd06e20f4`, equal to `origin/main`.
Implementation and validation refer to the working copy at that base; all
changes in this cut started from that clean state. This continues the active
[convergence goal](2026-09-08-convergence-goal-plan.md) and
[callable model](2026-09-10-callable-convergence-model.md).

Supersedes only the interactive semantic-lifetime gap identified by the
[final call diagnostic cut](2026-09-10-final-call-diagnostics.md). That note
remains historical evidence for its implementation and validation.

## Selected ownership and behavior

The compiler previously retained final semantics only in `CompiledProject`.
A rejected call could have complete final facts and a source diagnostic, yet
interactive LSP signature acquisition lost those facts because there was no
executable. Keeping an unrelated report next to an optional executable would
also require joining independently supplied HIR/world/report generations.

`ProjectAnalysisLease` now owns one complete immutable semantic generation:
its exact `ProjectToolingLease` ancestor, registered world, assertion build
profile, final semantic report and project semantic index. Its fields are
private; only the compiler constructs the lease. The report and index retain
the same checked-callable catalog allocation. `CompiledProject` owns that
analysis lease plus verification, style, Fx, View, dialogue and runtime
products. The old semantic getters on `CompiledProject` are removed; compiler,
CLI, Agent and LSP consumers explicitly use the analysis ancestor.

`ProjectCompilationLease` represents the latest completed phase: HIR,
Analyzed, or Compiled. Later phases own their ancestors, so consumers cannot
supply an unrelated tooling/executable pair. Compiler errors before committed
HIR have no lease. Errors during semantic/index construction retain HIR only.
After both are complete, callable-diagnostic admission and subsequent
verification, entry selection or lowering failures retain the analysis lease.
No partial report or second analyzer run is published. The cache transaction
still flushes only after successful executable assembly.

`CompiledSource` and `CompiledAgentBundle` retain the same analysis lease
instead of separate public HIR/report fields. This keeps source and semantic
identity together through runtime developer and Agent handoff. It does not
claim program-bound restore or complete runtime function-value ownership.

The LSP accepted project owns the single phase lease. Separate executable
fields in profile candidates/environments and copied source-registry
world/symbol/character revision fields are deleted. Exact source text,
identity, module, URI and overlay checks remain; semantic admission still
checks HIR generation, shared checked catalog and registered authority, and
source-registry construction still validates the character source set.

Signature acquisition and freshness, hover, inlays, character/entry/nominal
navigation and dialogue references consume completed analysis. Verification
consumers still require compiled products. Invalid source cannot retain an
old executable, and replacing an accepted generation invalidates old signature
cache entries and in-flight responses. Tests that isolate HIR freshness now
alter the request-side retained HIR stamp; they do not construct a project
paired with an unrelated world. Production freshness checks are unchanged.

The maintained [crate map](../00-overview/crate-map.md) and
[LSP chapter](../04-tooling/lsp.md) record this selected ownership. The complete
callable model remains PROPOSED. This cut closes its semantic publication
lifetime, not its complete-program activation or other coupled decisions.

## Validation record

Logs and measured command timings are retained locally under
`.arcweft-local/validation/2026-09-10-semantic-project-lease/`.
Cargo commands ran sequentially with normal Cargo concurrency. Focused
commands use all features; Justfile gates use their declared feature sets.
No additional clean, branch, worktree or job override is part of this cut.

Four compiler tests cover exact ancestor/catalog allocation, no semantic
lease before complete analysis, retention after rejected-call admission with
zero cache stores, and retention after later entry-selection failure. Three
LSP tests exercise actual requests against a rejected source: signature help,
checked callable hover and numeric fallback inlays, and a valid/rejected/
repaired edit cycle with stale response and cache rejection.

The initial focused run passed 4 compiler and 2 LSP tests. The changed-library
run passed 12 Agent REPL and 86 compiler tests, but failed two LSP tests: the
new hover fixture incorrectly expected an unsupported numeric-literal hover,
and the old HIR freshness fixture changed the now-owned world as well. The
fixtures now target supported callable hover and an isolated request-side HIR
mismatch. The full corrected LSP library run passed **221 tests**. No freshness
condition or required positive callable acceptance was weakened.

Earlier all-target checks intentionally exposed removed getters, fields and
constructor arguments during deletion-driven consumer migration. They are
failed migration checks, not acceptance evidence. The first completed
workspace check passed with unused copied-revision warnings; those fields
were subsequently removed. Final mainline gates and structural review are
recorded below when run.

## Remaining scope

The required correlated-constructor/callback/effect/scheme/runtime positive
acceptance failures remain in the convergence goal. Generic Match C3/C5,
retained View, task-plan, nominal C1-C6, scheduler and restore are not completed
by this semantic lifetime cut. The independently scoped
[function scheme and callable execution request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
and the callable model remain their design inputs. There is no new external
blocker or whole-goal completion claim. Frozen design packages are unchanged.

## Broad test results and retained failures

The compiler all-feature integration run passed **150 integration tests** plus
**86 library tests**, and failed **26 integration tests**. This includes
assertions 8, evaluated effects 17, cache transactions 20, project function
instances 11, dialogue profile admission 6, Agent enum execution 2, API compile
1, and the remaining successful integration rows. Callable execution remains
**57 passed / 24 failed**, the required positive correlated-source, contextual
constructor, prefix-as-callback, inferred-effect and prefix-return cases tracked
by the convergence goal.

The broader run also exposed **9 passed / 2 failed** in `view_product`:

- `compiler_lowers_checked_on_click_to_typed_bundle_handler_without_fx_conflation`
  is rejected at RuntimePlanLower because its semantic projection references a
  presentation-owned local. Its positive handler execution remains required.
- `compiler_retains_authored_owner_for_signature_and_default_failures` expects
  ViewLower but receives Registration. Registration precedes the changed phase
  publication boundary. This run does not establish a passing baseline for
  either View row; they remain explicit View reconciliation work, not credited
  as passed or converted into negative acceptance.

The CLI all-feature run passed **168 library tests** and **5 check_core_cli
integration tests**; its binary target contained no tests. The reviewed
workspace all-target/all-feature check passed. `just verify` passed formatter,
JLREQ generated-data comparison and workspace Clippy (warnings remain), then
its `test-workspace` recipe failed on the same 24 callable rows. Later commands
in that recipe did not run. Separate CLI and LSP results above are independent
performed evidence, not a claim that the recipe completed.

The initial Clippy run exposed a needless CLI reference and two test helper
Arcs that were unnecessarily cloned; those migration warnings were removed.
Existing workspace warnings remain. Accessor migration also lengthens some
cohesive assertion functions beyond Clippy's advisory line threshold; their
ownership is included in the structural review, not treated as a test failure.

Tier 2 was selected because this public contract crosses compiler, runtime
source handoff, Agent and CLI consumers. `just test-tier2` passed **4 MCP stdio
tests** and **1 broad Agent observe test**, then failed the first auxiliary
capture row at `samples/image-animation.arcw:1:1`: its `pub image` item is
rejected by syntax parsing, and required HIR recovery prevents analysis.
Later auxiliary capture, visual-golden, production Select and Flow boundary
recipes did not run. This matches the previously recorded sample failure; no
capture or visual acceptance is inferred from the passed MCP/observe rows.

## Structural review and final checks

`just test-doc` passed **8 doctests**. Canonical structural screening and
`just structure-audit-gate` both passed: **95 packages, 2,270 Rust files,
310 review triggers, 0 blocking violations**. The screening used
`cargo +nightly -Zscript tools/structure-audit.rs --root . --write` with the
local validation report directory. No source-spelling acceptance gate was
introduced.

The generated [changed-file measurements](structure-audits/2026-09-10-semantic-project-lease/changed-files.csv)
record all **55 changed Rust files / 1,520,790 bytes**, classification, exact
physical LOC, full-file base LOC, embedded test LOC and package dependency
counts at the stated dirty base. They are generated review data, not production
source. Package normal fan-in/fan-out is compiler **3/23**, LSP **0/23**, CLI
**0/53**, and Agent REPL **3/18**. The dependency graph, manifests and feature
edges are unchanged; semantic owner movement stays inside the compiler and
its existing higher-layer consumers.

Touched production review triggers have these dispositions. Paths are relative
to `crates/`; complete measurements and test classifications are in the CSV.

| Owner/path | Bytes | Base → current LOC | Embedded tests | Cohesion disposition |
|---|---:|---:|---:|---|
| compiler `src/project.rs` | 60,473 | 1,723 → 1,665 | 0 | Extracted complete semantic phase state/API into `project/analysis.rs`. The remaining driver retains the single pending-cache transaction across all compilation phases; phase ownership and error propagation stay in one orchestration boundary. |
| LSP `src/profiles/accepted_project.rs` | 42,156 | 1,270 → 1,237 | 0 | One immutable accepted source/module/semantic admission boundary. Removed independent executable joins and copied revisions; exact source admission and indexes stay together. Tests remain in its child modules. |
| LSP `src/diagnostics.rs` | 50,394 | 1,351 → 1,353 | 933 | Source-bound protocol diagnostics with its direct range/encoding assertions. This cut only changes semantic availability and keeps verifier availability separate; no new state cluster, traversal or I/O is introduced. |
| LSP `src/features/nominal_types.rs` | 59,642 | 1,705 → 1,705 | 803 | Nominal symbol query/edit projection and corresponding source tests. Availability now derives from the same analysis owner; namespace, URI and edit validation stay within the existing feature boundary. |
| CLI `src/app/agent/native/repl.rs` | 52,064 | 1,572 → 1,572 | 0 | Host REPL lifecycle and display remain unchanged. HIR/type reads now traverse the artifact's analysis lease; no runtime state or transport responsibility moves. |
| CLI `src/app/agent/rag/source_index.rs` | 55,045 | 1,500 → 1,500 | 0 | Existing source RAG projection reads the compiler's analysis index. The edit adds no independent semantic catalog or indexing traversal. |
| CLI `src/app/agent/script.rs` | 79,726 | 2,266 → 2,272 | 77 | Existing Agent source compilation and artifact handoff retain their host responsibilities. Changes are ancestor access at the same compile/snapshot boundaries; embedded tests still own those handoffs. |
| CLI `src/app/jit.rs` | 58,202 | 1,628 → 1,630 | 0 | JIT orchestration and compiler-stat projection remain together; the sole changed projection obtains its report from analysis. No executable-memory or backend ownership changes. |
| CLI `src/app/project.rs` | 53,672 | 1,486 → 1,490 | 55 | Host project loading/checking, source diagnostics and verification keep their existing source/profile boundary. The migration adds no lower-layer I/O or new mutable state. |
| CLI `src/app/project_commands.rs` | 89,596 | 2,488 → 2,495 | 0 | Build/cache/watch/compile command consumers share the accepted compiler result. This cut changes only semantic ancestor access; command policy and persistence ownership are unchanged. No API was widened to split the file. |

The compiler project tests and cache-transaction tests, LSP profile-state tests,
signature-cache fixture and session tests remain separate test owners; the CSV
records their exact sizes. Added tests live under the compiler analysis-lease
and LSP signature-cache responsibilities. Existing assertions migrated directly
to the new ancestor API, without changing required success/failure expectations.
Clippy's line-count warnings on cohesive test functions are review aids, not a
reason to split one cache transaction or semantic acceptance assertion.

No new schema, codec, wire, ABI or digest-domain version marker was introduced.
No frozen design ZIP was changed. Vendor glyphon validation was not selected
because neither the fork nor its adapter contract changed. Full CLI visual and
unrelated performance matrices were not run beyond the prescribed Tier 2
attempt. Whole-goal completion still requires the explicit callable and View
failures and the remaining convergence stages above.

After the final test-helper ownership cleanup, the LSP profile-state group
passed **12 tests**. Final whitespace inspection caught CRLF introduced by a
Windows line-writing operation in the compiler driver; its original LF form
was restored without changing Rust tokens. Formatter check, whitespace check
and the canonical structural gate were repeated successfully; the CSV and
byte counts above reflect this final form.
