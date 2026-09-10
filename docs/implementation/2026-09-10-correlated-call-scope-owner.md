# Correlated call scope ownership — in progress

Status: **IN_PROGRESS; not accepted as callable-component completion**.

The inspected base is `main` at
`c00992a791d55fea70de6a3e736c0527ea913482`. The path-scope and source-frontier
implementation below is an uncommitted working-copy change on that base. The
source callback ownership migration was committed and pushed in
`fdad7ab45fdb3098b7049ba719e8334d70b45772`; its
[separate record](2026-09-11-source-callback-context.md) describes that cut.
The independent [materialization receipt correction](2026-09-11-materialization-receipts.md)
is committed in `9bf29c04153ca6fe574378688e60e5d5e3a8ad1f`. The
[borrowed driver/context lifetime](2026-09-11-borrowed-constraint-driver.md)
is committed and pushed in `7cfb6727a714c5db06a1e8c83eb6a1d1e9df4822`.
All 29 unfinished files were restored and raw-hash verified after that cut.
The subsequent [canonical count correction](2026-09-11-canonical-transcript-counts.md)
preserved and restored all 30 files of this scope/source work. The later
[ordered record storage correction](2026-09-11-record-storage-admission.md) is
the current base; it likewise preserved and restored all 30 files, with complete
raw-byte hash verification. Earlier validation counts below retain their original
scope and base. The
[convergence goal](2026-09-08-convergence-goal-plan.md) and
[coupled callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md)
remain open. This is evidence for a lower boundary, not source-language or
runtime acceptance.

After restoring this WIP on
`fc6bba31548b34391fd70595c8de148476bb4f2b`, the complete sema test command
(`cargo test -p arcweft-lang-sema --all-features`) recorded **801 passed and
the same 9 known failures**. Exact failure names match the preceding qualified-
source run, with no additions or removals. This combined-working-copy evidence
is in `2026-09-11-canonical-transcript-counts/restored-wip-sema-tests.log` and
`restored-wip-failure-comparison.json` under the local validation directory;
it is distinct from the isolated canonical-count cut's accepted validation.

On 2026-09-11, after restoration on
`c00992a791d55fea70de6a3e736c0527ea913482`,
`cargo test -p arcweft-lang-sema --lib --all-features` again recorded
**801 passed / 9 failed**. The exact failure names match the preceding restored
WIP run, with no additions or removals. The log and comparison are
`2026-09-11-record-storage/restored-wip-sema-tests.log` and
`restored-wip-failure-comparison.json` under local validation. This checks the
unfinished component with the accepted record storage change; it does not
accept that component or complete the callable integration below.

The later [effect-completion investigation](2026-09-11-callable-effect-completion.md)
adds two still-failing acceptance cases for an escaping invoked callback and
independent effect instantiation from a shared prefix. Its focused higher-order
suite is 6 passed / 6 failed (four existing failures plus the two new cases).
The earlier 801/9 full-library result predates those additions; no newer full
library result is claimed here.

## Implemented lower boundary

`ConstraintPath` owns the exact application scopes admitted on that path,
together with its type/constant bindings, effect environment, equations and
source trace. `TypeConstraintContext` retains work accounting, cancellation and
lexical traversal state. It no longer supplies an independent parameter or
effect scope. Projection, template opening, reference validation, equality,
normalization and solution sealing read the path's scope and bindings together
through `ConstraintProjectionView`.

`ConstraintApplicationScope` distinguishes a domain application coordinate
from its live generic opening. Scope inventories are shared immutably when
paths fork. Admission extends only the selected path; it rejects a repeated
application coordinate, a repeated opening and overlapping effect ownership.
The existing effect environment admits disjoint inventories while retaining
its bounds, inherited rows and subset edges. The same work context charges
scope insertion and observes cancellation before mutation.

Path comparison includes application membership. Empty or equal binding maps
must not erase distinct admitted applications. Forks retain the same scope
authority; separately admitted copies of one prepared scope can compare equal.
This does not establish a candidate-ranking rule or semantic equivalence of
independently prepared application openings.

The old test-only context constructors were removed. Fixtures keep preparation
inputs separate from the context and either enter the production transaction
initializer or construct a real scoped path for isolated projection tests.
The production API taking independent lookup maps was removed. Existing alias,
scope, completion, budget and cancellation tests retain their original inputs
and behavioral assertions.

## Validation observed

Local command records are under
`.arcweft-local/validation/2026-09-10-correlated-call-components/`.

| Command / evidence | Result |
| --- | --- |
| `cargo fmt --all`, `fmt-source-context.log` | Passed |
| `cargo check -p arcweft-lang-sema --all-features --tests --message-format short`, `source-context-check.log` | Passed with warnings |
| `cargo test -p arcweft-lang-sema --all-features --lib`, `source-context-sema-tests.log` | **Failed:** 784 passed, 9 failed |
| Lower constraint tests within that run | 122 passed, including 6 new application-scope cases |
| `cargo test -p arcweft-lang-sema --all-features --lib callable::constraints::tests`, `source-context-driver-tests.log` | 16 passed |
| Earlier `cargo clippy -p arcweft-lang-sema --all-targets --all-features`, `sema-scope-clippy-fixed.log` | Passed with warnings before the callback migration below; not validation of that later change |
| Earlier migration checks | Failed on removed test APIs; migrated before the final test run |
| Earlier focused run | 115 passed, 1 failed because a fixture binding had not been transferred; restored before the final run |
| Workspace check, compiler/native/AWBC, structural audit, doctests and Tier 2 | Not run for this working-copy change |

The nine failing positive tests are the existing five contextual/correlated
generic-call cases and four inferred callback-effect cases. They remain
required work; their failure assertions were not weakened. The six new cases
exercise parent/child type, constant and effect closure, sibling isolation,
application/opening uniqueness, effect ownership, shared limits/cancellation,
and scope-sensitive path comparison through typed lower APIs.

Warnings still include the production child-admission entry points that are
not connected to the analyzer yet. They are not suppressed or counted as
integration evidence. The later source-frontier and rigid-reference sections
below record the subsequent tests and Clippy runs.

## 2026-09-11: source callback context

This callback ownership step is committed in
`fdad7ab45fdb3098b7049ba719e8334d70b45772`; the path-scope work is not.

`CandidateConstraintSourceContext` now borrows the exact lower `ProbeTicket`
and the existing `TypeConstraintContext` for the callback. The callable driver
alone constructs it. The separate source, hint and accountant arguments were
removed from `TypeConstraintClient::probe_source` and the analyzer operation
adapter. The analyzer obtains source identity and borrowed hints from this
capability and charges physical source work through its existing accountant.
All production and test implementations of that callback use the new entry.

The 16 driver cases retain single checkpoint closure on success/rejection/fatal
failure, cancellation and work limits, invalid-ticket rejection and source
failure precedence. The full sema run retains the same nine positive failures.
This establishes the callback ownership migration; it does not yet make
recursive expression evaluation use a child transaction on the borrowed path.

`AnalyzerExpressionExpectation::contextual_shape` already exposes a known
constructor head from a parametric expectation. Replacing constructor lookup
or treating those variables as rigid would not repair the missing relation:
`PreparedCandidateRequest::expected_result` still receives only
`complete_type()`, and the child run still completes independently.

## 2026-09-11: application-qualified sources and completion

`ConstraintSourceId` pairs a domain-local source with its admitted application
opening. Probe inputs, callback/checkpoint authorities, closed source requests,
ordinary/fatal source errors and rejected source projections retain that pair.
Prepared source schemas remain local to their application. Prepared source and
equation ordinals also carry their application, so closing a child source cannot
rewrite a parent's equation with the same local ordinal. Closing rejects an
unadmitted source application or an ordinal owned by another application.

Materialization failure precedence uses the ordered component trace. It does
not compare the issuance order of application openings. The new regression
prepares the child's opening first, retains the parent source first in the
trace, and submits the child's fatal before the parent's fatal; the parent
source still wins by trace order.

`ClosedConstraintSourceTrace` retains the path's one immutable application
inventory with the selected opening and all closed source rows. Analyzer
ranking, argument projection and final call sealing inspect its selected
application's rows. Nested rows stay in the trace. Replay comparison resolves
each opening through that same inventory to the domain application identity;
it compares the local source/schema and closed observations without requiring
fresh openings to be identical. Callback receipt checks still require the
exact opening. The analyzer's semantic branch keeps its local branch coordinate;
application ownership is supplied by the lower trace, not duplicated there.
This remains an analyzer preparation product, not a published runtime value.

Validation logs are under
`.arcweft-local/validation/2026-09-11-qualified-constraint-sources/`:

- The first full sema run exposed 16 additional replay failures because the
  initial equality compared transient openings. After the ownership correction,
  `sema-final-tests.log` records **801 passed, 9 failed**. Failure names exactly
  match the prior application-bound run: no additions or removals. This includes
  **138 passing lower constraint tests** and **17 passing callable driver tests**.
- Six new tests cover repeated parent/child source coordinates, exact equation
  closure, application-qualified fatal/rejection results, foreign application
  and ordinal rejection, replay across fresh openings, trace-order precedence,
  and driver rejection of another application's callback submission.
- Initial migration/fixture compile failures were corrected. Subsequent changes
  only made unit patterns and assertion semicolons explicit in the new fixture.
  Their focused lower run passed all **138 tests**; changed-crate
  all-target/all-feature Clippy passed with **1,392 warnings, 1,191 duplicates**.
  These results are recorded separately in `constraint-tests-lint-fixed.log`
  and `clippy-final.log`. `cargo fmt --all` and `git diff --check` also passed.
- Workspace/runtime/structural gates and doctests have **not been run** for this
  unfinished integration. Earlier accepted cuts' results do not establish it.
  No new clean, destructive Git operation, branch or worktree was used.

The full correlated-call producer and component publication are still required.
This evidence does not accept the coupled callable model or close its goal.

## Required continuation

### 2026-09-11: application-bound transactions and inherited restoration

The current working copy removes the uninitialized `TypeConstraintTransaction::new`
state. `initialize` now returns a transaction with one exact admitted
application identity. `from_path` likewise requires an application already in
the path and runs the same required-inheritance checks before exposing the
transaction. Neither entry can silently append another initial frontier.

Equation template opening, source hint projection, submitted expected types,
final projections and solution sealing use that transaction's application;
they no longer assume it is the path's root. Scope lookup uses the inventory's
typed `require_application` operation. Inherited type and constant rows reopen
and restore into the selected application, together with its exact effect
inventory. This prevents a child continuation from being checked against or
restored into the parent's parameter contract.

All current root construction and direct restore tests were migrated. Three
new lower cases cover child type/constant opening through source hints,
materialization and final projections; rejection of an unadmitted application;
and restoration of inherited child type, constant and effect rows while the
parent's different bindings stay intact. The latter also rejects entry when
required inherited evidence is absent. The source fixture uses an
unconditional unit evidence rule; production evidence rules are unchanged.

Observed validation in
`.arcweft-local/validation/2026-09-11-application-bound-transactions/`:

| Command / evidence | Result |
| --- | --- |
| `cargo fmt --all` | Passed |
| Initial changed-crate check | Failed on new test API spelling / scoped-view comparisons; corrected |
| Initial source test runs | Failed on a borrowed-hint comparison and the fixture's unused rejecting evidence rule; corrected |
| Initial inherited-restoration run | Failed on five old direct restore signatures; all callers migrated |
| `cargo test -p arcweft-lang-sema --all-features --lib types::constraints`, `inherited-constraint-tests-fixed.log` | 133 passed |
| `cargo test -p arcweft-lang-sema --all-features --lib`, `sema-tests.log` | **Failed:** 795 passed, the same 9 known call/effect failures |
| `cargo clippy -p arcweft-lang-sema --all-targets --all-features`, `clippy.log` | Passed with warnings |
| Workspace/runtime/structural gates for this working copy | Not run |

This is still uncommitted scope integration on the inspected base. The actual
analyzer child-contribution producer, application-qualified source obligations
and projections, correlated ranking and residual closure remain required.
The new lower cases do not establish source-language acceptance.

### 2026-09-11: driver/context lifetime accepted

The driver now borrows the context held by
`CandidateConstraintWorkSession::with_driver`. Lower transaction completion
borrows the same context and returns the solved result directly. The old
`TypeConstraintRun` and its additional `complete`/accounting forwarding path
were removed; releasing the context commits its existing accounting
reservation. Finishing or abandoning an individual transaction cannot reset
the shared counters or release that reservation.

The full working-copy check passed, the sema run retained 791 passed and
9 known failures before the added lifetime case, and the final lower run
passed 130 cases including that case. Logs are in
`.arcweft-local/validation/2026-09-11-borrowed-constraint-driver/`.
The [accepted cut record](2026-09-11-borrowed-constraint-driver.md) separately
records exact-cut validation: 781 sema passes, the same 9 failures, 119 lower
passes, 16 driver passes, workspace check/Clippy, 8 doctests and structural
gates. Workspace tests retained the same 24 compiler failures. This lending
boundary is production-wired; do not recreate another owned/borrowed context
model or revive the removed completed-run wrapper. Actual child admission and
component closure remain unimplemented.

### 2026-09-11: owned source frontier and pending receipts

The working-copy source context now owns its `ProbeTicket`. The ticket keeps
an immutable `ProbeInput` separately from its retained constraint paths, so a
callback can borrow a hint while using the same mutable work context. Accepted
source evidence and the semantic branch are shared across retained paths;
rejection removes every retained path. These paths are currently produced by
lower behavioral fixtures, not by nested analyzer applications.

The active probe operation retains the exact immutable input as its receipt.
Another operation's ticket with an equal source coordinate is rejected without
consuming the expected receipt. Advancing before submission is also rejected.
The full sema run in
`.arcweft-local/validation/2026-09-11-correlated-probe-frontier/sema-receipt-tests.log`
failed with **788 passed and the same 9 known failures**. The four new frontier
and receipt cases passed. This run preceded the materialization changes below.

Review also found an independent materialization protocol defect: requesting
the next ticket before submitting the current one removed a queued candidate,
and finishing with the last ticket outstanding could publish an earlier
completed candidate. The working-copy fix checks the outstanding receipt
before dequeuing and before selecting a completed candidate. Two behavioral
cases cover queued and last-ticket advancement and premature publication.
The focused lower run, `materialization-ticket-tests-fixed.log`, passed
**128 tests**. The first attempt failed to compile because the new test import
was missing; it was corrected before this run. Broader validation of this
independent fix is in the [committed record](2026-09-11-materialization-receipts.md).
That correction is now accepted in the base commit; the scope and source
frontier changes remain uncommitted.

### 2026-09-11: admitted rigid references

Review exposed another incomplete part of the scope migration: Free type and
constant references were looked up only in the root application inventory.
A child may retain declaration-owned rigid references that are absent from
that inventory. The path's application owner now decides eligibility directly:
an inference reference requires its exact admitted opening, a Free reference
requires an admitted rigid occurrence, and Bound references remain governed
by the context's lexical binder. Multiple applications capturing the same Free
reference agree on its rigid role. No reference is converted from inference
to rigid, and unchosen sibling paths do not gain the child's inventory.

The new typed regression failed before the correction (`None` instead of
`Some(Rigid)`). After correction it checks both type and constant capture,
projection preserving their declaration identities, repeated capture and
sibling isolation. `child-captures-sema-tests.log` records **791 passed and the
same 9 known failures**, including **129 passing lower cases**. This is the
current working copy, including the accepted materialization correction and
11 unfinished scope/frontier cases. Failure names match the exact-cut sema run
with no additions or removals. All logs for this step are in the
`2026-09-11-correlated-probe-frontier` local validation directory above.
After making the new fixture's empty `BTreeSet` explicit to address its Clippy
warning, all **7 application-scope tests passed** and
`cargo clippy -p arcweft-lang-sema --all-targets --all-features` passed with
warnings (`child-captures-final-tests.log`, `child-captures-final-clippy.log`).
The complete workspace/runtime/structural gates have not been run for this
unfinished scope/frontier integration; the accepted materialization cut's
separate gates do not establish that integration.

These source-frontier changes do not establish the missing child producer,
per-application closure, candidate ranking or residual-value representation.

The analyzer still completes a child candidate independently. Its semantic
branch is local to the application-qualified lower source trace; it does not
yet carry a prepared child contribution. The new child-admission operation is
not connected to that production source callback. Lower finish returns one
selected application's solution, rather than a closed solution for every
application in the component. Therefore this change cannot be committed as a
finished correlated callable component.

The next integration must preserve all of the following:

- An authenticated source ticket and the existing prepared-call graph own
  each child contribution, its candidate choice and the corresponding fact
  delta. A pending child is not an already checked expression value.
- Nested work uses the same `TypeConstraintContext`. Merely borrowing the
  accountant into a fresh context is insufficient: node/branch limits are
  checked by the context's accumulated report, whereas the callable accountant
  separately checks total work, source probes and materializations.
- Each surviving path retains every admitted application's source obligations,
  projections and selected candidate. Closure and publication cover the whole
  component. Ranking cannot sum child ranks or prefer an argument's physical
  position; the final correlated comparison remains to be specified.
- Residual closure must cover cross-application dependencies. In particular,
  an admitted child's parameter may be related to an outer future-eligible
  parameter. The current per-application residual binder cannot by itself
  establish the enclosing residual scope of such a retained value. The
  coupled design must decide its admission and scoped representation without
  retaining active issuers, conflating two uses of one declaration, or using
  rejection to avoid the callable-value contract.
- Physical source order, affine rollback, closed semantic publication,
  function-scheme specialization, runtime dispatch and program-bound restore
  remain obligations of the complete model.

No stable language contract or compatibility exception is introduced here.
All contract versions remain `1`. There is no external blocker; implementation
and the coupled result-changing decisions remain in progress.
