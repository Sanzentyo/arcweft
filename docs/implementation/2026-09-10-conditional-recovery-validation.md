# Conditional recovery: implementation and validation

Date: 2026-09-10. Initial inspected base on the existing `main` checkout:
`a44835906f70d0b12343448e24192e5556cc22d6`, initially clean. Verification below
was performed with this implementation dirty at that base. Failed and unrun
gates are listed explicitly rather than counted as passed. The independent
hover-fixture correction was committed and pushed first as
`11bbdff0471881480e731abcf15605234ac3979e`.

This implements the [selected model](2026-09-10-conditional-recovery-design.md)
for the [conditional recovery request](../reviews/requests/2026-09-10-conditional-syntax-recovery-readiness.md).
It continues the [convergence goal](2026-09-08-convergence-goal-plan.md), whose
higher-order effects, nominal, Match, View, RuntimePlan and scheduler work is
not completed by this cut. The earlier
[Pattern correction](2026-09-10-recovered-closure-projection.md) remains a
separate accepted prerequisite.

## Implemented boundary

- Syntax retains conditional recovery, its original diagnostics, missing
  tokens and exact fragment rebasing. Required recovery remains fatal. Shared
  diagnostic identities consume the document budget once, including retained
  alternatives. Invalid primary, related, edit and missing-token ranges fail
  before source publication.
- HIR source freeze issues immutable containment for all candidate descendant
  arenas. Generated Ruby and validated Content nominal roots inherit their
  producer's region. It completes the existing source index with typed
  candidate expression, Type and Pattern components. No second source reader,
  reconstructed spelling or range-derived identity was added.
- Raw HIR now grants analysis admission. The removed executable-view API has
  no compatibility alias. Selected semantic evidence closes the actual program;
  ordinary recovery, selected poison and missing decisions remain errors.
- Capture owners retain every typed use. One HIR projection selects capture
  order and access, and sema binds exact choice receipts to its topology and
  final selected graph. Compiler and runtime use that projection for the ABI.
  Nested instances obtain captured source-local types within their own catalog.
- Function body result constraints close before final execution roles. Stream
  return and generator interpretations use the existing affine fact transaction;
  ordinary non-Stream bodies keep their ordinary publication context. Failed
  probes retain physical work and cannot publish facts.
- Candidate-local authored control failures remain typed HIR evidence and are
  rejected only if selected. Verifier and semantic-index consumers use selected
  statement inventories. Current candidate grammar emits neither Include nor
  On nor `Select::Branches`; the eager ingress seeders therefore do not consume
  those candidate producers. No new grammar was inferred for this cut.
- Authored dialogue nested in selected candidates enters the existing line
  inventory, including named scopes. Runtime postfix decisions remain required
  even for selectors without an independently retained runtime type. Flow value
  blocks use their statement sequencing path before yielding their tail value.
- Source/HIR cache reuse and executable publication remain distinct. Failed
  source revisions cannot store a compiled module or invalidate an older accepted
  generation. The maintained dialogue chapter now assigns type-based selection
  to sema.

All touched contract versions remain `1`. Dependency direction and Sans-I/O
ownership are unchanged. The implementation resolves the request in repository;
no returned design ZIP or external approval is being claimed.

## Acceptance evidence

The compiler's eight candidate tests include both Ruby spellings with exact
base/reading templates, unknown annotation rejection, three rejected control
targets, losing capture removal, capture order/access, nested closure crossings,
function execution-role closure and generated Content nominal types. Native
and canonical-codec-round-tripped verified AWBC execute the Ruby cases, three
capture cases and two typed Content cases through completion. Noncommutative
capture arithmetic and runtime assertions check actual captured values. The
typed Content cases explicitly select Index while Dialogue has required
recovery; both plain and named value blocks emit their line exactly once.

HIR tests retain both alternatives and exercise typed parameters, patterns,
scope/local generations, generated children, source order and inclusive
descendant limits. Missing/excess identities, reordered operands, forged local
names/generations and capture uses reject module publication. The selected
graph test rejects an internally valid capture receipt for another interpretation
without changing the accepted receipt. Existing sema tests cover zero-winner
and two-winner rollback and the affine argument-evaluation budget.

The cache transition test applies seven source states in one attached compiler
session, alternating accepted Ruby/Index and malformed ordinary/selected/shared
source. Accepted repeats reuse their exact HIR lease; failed repeats store
nothing and leave the older accepted generation valid. The LSP transition test
checks conditional → required recovery → conditional edits, source-lease
invalidation, retained diagnostic accounting and ordinary diagnostic publication.

Logs are retained under
`.arcweft-local/validation/2026-09-10-conditional-syntax-recovery-readiness/`.

| Validation | Result |
| --- | --- |
| `compiler-candidates-15.log` | 8 passed, including native/AWBC execution; 25.38 s |
| `changed-libraries-2.log`, HIR | 899 passed, 8 ignored; 152.21 s |
| Same run, sema | 763 passed, 7 preexisting failures; 1.24 s |
| `syntax-lsp-libraries-3.log` | Syntax 679 passed, LSP 217 passed; 64.91 s including rebuild |
| `runtime-library-3.log` | 64 passed, including missing runtime selection rejection; 58.75 s including rebuild |
| `cache-transitions-1.log` | 1 passed; 30.05 s including rebuild |
| `workspace-check-1.log` | Workspace, all targets/features passed; 58.63 s |
| `final-format-clippy.log` | `cargo fmt --all`, formatter check and `cargo clippy --workspace --all-targets --all-features` passed; existing workspace warnings remain; 58.21 s combined |
| `workspace-tests-2.log`, `just test-workspace` | 857 passed, 18 preexisting callable-execution failures across 84 reports; 28.21 s. The recipe stops there; later workspace binaries and CLI recipes were not run by this command |
| `doctests-1.log`, `just test-doc` | 95 reports, 8 tests passed; 118.61 s |
| `structure-audit-gate-2.log` | Canonical screening, retained reports and blocking gate passed after final formatting; 95 packages, 2,258 Rust files, 310 review triggers, zero blocking violations; 2.44 s |
| `tier2-1.log`, `just test-tier2` | MCP stdio 4 passed; basic native capture 1 passed; first auxiliary image-animation test failed on existing sample grammar; 62.17 s combined |
| Documentation review | 22 local Markdown links resolved; `git diff --check` passed |

Final review also renamed the remaining raw-HIR lease validator to
`validate_analysis_lease` and corrected its analysis-view Rustdoc. No behavior
changed after the behavioral gates. `admission-api-final.log` records the final
formatter check, workspace all-target/all-feature Clippy, all 95 doctest reports
(8 tests passed) and regenerated structural blocking gate, all passed in
86.90 s combined. This final run is based on the independent hover commit
listed above, with the integrated implementation still dirty. Cargo commands
were sequential and used Cargo's normal concurrency.

The seven sema failures remain the three contextual constructor-inference
cases in `generic_calls` and four callback effect-row cases in
`higher_order_effects`. They are required parent-goal work, not waived acceptance
for that goal. The three existing generics dead-code warnings remain visible.

The 18 compiler failures are the native/AWBC pairs for contextual constructor
inference, inferred callback effects, curried and generic prefixes used as
callbacks, a shared prefix with distinct later types, an uninvoked inferred
callback and character factory branches. These are the same cases recorded at
the inspected base. The first workspace run also failed an older assertion that
captured source locals had no local type in their own closed instance. That
assertion now verifies both distinct owners: no declared-local row, and the
capture's type in the instance's local-type query. The second run passed all
82 compiler library tests before the preexisting integration failures.

Tier 2 stops at `agent_observe_read_uri_preserves_animated_image_object_frame_metadata`:
`samples/image-animation.arcw:1` begins with `pub image`, which the current
parser rejects as a top-level item. The remaining auxiliary captures, visual
goldens and ignored production Select/Flow boundary suites were not run by
that recipe. No removed grammar or image implementation was restored merely
to change this validation result; that existing failure remains parent-goal work.

Intermediate failures exposed and corrected candidate control publication,
closure statement preparation, capture ordering/type lookup, early function
role inference, nested content ownership, source components and runtime
selection/block sequencing. Test-only failures included obsolete HIR recovery
expectations, a diagnostic-budget range crossing UTF-8, and an existing LSP
hover fixture searching authored source for the displayed ellipsis. Logs retain
these failed runs; later passing runs do not erase them.

## Review intake and structure

Review ZIPs were re-enumerated: 71 archives, 4,802,433 bytes, no inbox archive.
Paths, byte lengths and SHA-256 hashes exactly match the preceding accepted
cut's inventory. No archive was modified, selected or newly adjudicated.

The [structural ownership review](2026-09-10-conditional-recovery-structure.md)
records all 45 touched review triggers, complete base/final file measurements,
state and dependency ownership, public API and test boundaries, and the
decomposition or cohesion disposition. The [generated reports](structure-audits/2026-09-10-conditional-recovery/README.md)
retain the canonical measurements. No new dependency edge or second persistent
source/capture authority was introduced. No external blocker or compatibility
exception has been identified.
