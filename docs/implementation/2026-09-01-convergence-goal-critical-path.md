# Convergence goal critical path — 2026-09-01

Inspected revision: `fed4102e50c90b06e2dbabf1e7259ee09352008d`

Inspection boundary: the accepted `main` reachable through GitHub. The local
working tree, index, and uncommitted agent work are not observable through this
inspection and receive no completion credit here.

This note is an implementation-status and sequencing reference. It does not
replace maintained language/runtime chapters, accepted review designs, or the
current production types.

## Status vocabulary

- `MERGED_AND_EVIDENCED`: present on the inspected `main` with implementation or
  validation evidence.
- `MAIN_CLOSEOUT_REMAINING`: substantial production authority exists on `main`,
  but the final publication/deletion boundary is not yet evidenced as complete.
- `DESIGN_READY`: accepted design is ready for production implementation.
- `OPEN_DESIGN_REQUEST`: a maintained request still requires a current-source
  accepted design before production implementation.
- `BLOCKED_BY_PREDECESSOR`: no final public row may publish until the named
  predecessor is complete.
- `MAIN_UNPROVEN`: the intended result may exist in a local working tree, but it
  is not established by the inspected `main`.

## Scope of the convergence goal

This rollup covers the currently connected convergence chain:

1. canonical Dialogue Content and shared Fx publication;
2. generic Match complete transcript and exhaustive publication (`.1.2`);
3. retained View operation/value-slot completeness (`.1.4`);
4. current RuntimePlan semantic owner and task-plan seal (`.1.3.1`);
5. accepted structural nominal runtime carrier;
6. current host scheduler and Sans-I/O restore transaction; and
7. final deletion, workspace, codec, structural, and applicable Tier 2 gates.

Work outside this chain is not reclassified by this note.

## Current baseline

The inspected `main` ends at:

- `fed4102e50c90b06e2dbabf1e7259ee09352008d` — dependency-direction test
  synchronized with the accepted HIR `sha2` dependency;
- `e67ab0972e319a635b5ee06100eb823d6b90fa36` — obsolete HIR dialogue/speaker
  tombstone cases deleted;
- `14e0ad4fd7ea510ac437533107a9bdfe4afaa825` — withdrawn rich-text formatter and
  source-action rewrites deleted; and
- `4c92d24d0fb90345a875c004b616aba80376b08e` — maintained Dialogue Content/Fx
  surface converged in documentation.

These commits establish policy, deletion, and test synchronization. They do not
by themselves prove that the complete Content/Fx public vertical has published
through every consumer.

## Workstream ledger

| Workstream | Design state | Production state on inspected `main` | Immediate blocker | Next accepted outcome |
|---|---|---|---|---|
| Canonical Content and shared Fx | maintained surface selected | `MAIN_UNPROVEN` final switch | no main-level completion evidence for the full producer/consumer vertical | one final typed catalog/identity and shared producer consumed through all affected layers, with obsolete paths deleted |
| Generic Match `.1.2` | request resolved by accepted design | `MAIN_CLOSEOUT_REMAINING` | complete atomic semantic transcript and exhaustive-only publication | C3 transcript graph plus C5 atomic `CheckedMatch` publication and deletion gate |
| Retained View `.1.4` | `OPEN_DESIGN_REQUEST` | not implementation-ready | `.1.2` production completion | request refreshed against the actual predecessor commit, accepted design, then executable View product implementation |
| RuntimePlan/task-plan `.1.3.1` | `OPEN_DESIGN_REQUEST` | not implementation-ready | `.1.4` | current-source accepted design and one-time RuntimePlan/task-plan seal implementation |
| Structural nominal carrier | `DESIGN_READY` | final accepted success not evidenced | current-source gap reconciliation and remaining C1-C6 work | exact Rust ADT join through runtime/AWBC/snapshot restore and C6 ownership success |
| Host scheduler/Sans-I/O restore | `DESIGN_READY` | `BLOCKED_BY_PREDECESSOR` | `.1.2`, `.1.4`, `.1.3.1`, and structural nominal completion | accepted A-F scheduler/restore switch with old driver/registry/persistence paths deleted |
| Final convergence gate | defined by current policies/designs | not run as one final goal gate | all rows above | workspace, Clippy, tests, codecs/goldens, structure, Tier 2, and deletion evidence |

## 1. Canonical Content and shared Fx

Maintained language authority selects:

- body-bearing Content calls as `#name(args)[content]`;
- `#fx(value)[content]` as the sole Content Fx adapter;
- `.fx(value)` as the reactive View application form;
- `#object(id=..., type=...)[content]` as the explicit TextProxy/object form;
- point controls, marks, and host actions in bracket form; and
- no paired body tags, `#wave`, `#effect`, compatibility reader, formatter
  rewrite, or source action.

Reference:
[Dialogue Content Actions, Ruby, Interpolation, and Line Marks](../01-language/dialogue-content-actions-ruby-and-interpolation.md).

The inspected `main` proves deletion of withdrawn tooling rewrites and obsolete
HIR tombstone tests. It does not contain a dated implementation record proving
the final shared Fx producer plus exact Content callable identity has completed
the full syntax/HIR/sema/compiler/View/bundle/runtime/tooling closure.

### Work that can proceed now

One coherent reviewable vertical may close:

1. the presentation-owned callable head/schema/body-policy authority;
2. the exact HIR semantic evidence that is genuinely HIR-owned;
3. one sema-owned Fx producer seal shared by Content and View contexts;
4. final checked Content/Fx application carriers;
5. compiler, View, bundle, runtime, formatter, LSP, CLI, and fixture consumers;
6. deletion of static mirrors, duplicated resolvers, aliases, fallback paths,
   and obsolete success branches; and
7. focused, changed-crate, workspace, Clippy, structure, and applicable product
   validation.

Do not split this around a temporary public identity, side table, compatibility
alias, or partially consumed producer.

### Completion evidence required

- one dated implementation note naming the inspected and resulting full Git
  SHAs;
- typed success and rejection tests for every maintained Content/Fx form;
- Content and View applications proven to consume the same producer authority;
- no alternate `#wave`/`#effect` or paired-tag success path;
- all affected downstream products compile and their focused tests pass; and
- the final commit is on `main`, not only in a local working tree.

## 2. Generic Match `.1.2`

The maintained request is
[`RESOLVED_BY_ACCEPTED_DESIGN`](../reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure-correction.md).
The accepted implementation graph remains documented in
[the generic Match design](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure/CUTS_TESTS_AND_DELETION.md).

Main-level evidence already includes:

- C1 HIR topology and View callable path work;
- exact checked semantic owners and later statement/effect prerequisites;
- a private bounded Maranget-style coverage engine;
- checked `u64` work accounting;
- layout-free nominal semantic fields/cases;
- exact variant payload shapes and runtime record types; and
- sealed checked statements plus executable ingress.

References:

- [C1 HIR topology and View callable](2026-08-23-generic-match-c1-hir-topology-and-view-callable.md)
- [Generic Match matrix and layout-free nominal semantics](2026-08-28-generic-match-matrix-and-layout-free-nominal-semantics.md)
- [Checked statement transcript prerequisites](2026-08-28-checked-statement-transcript-prerequisites.md)

### Remaining C3 closeout

The remaining transcript boundary is one memoized, cycle-checked graph over
accepted-rooted:

- expressions;
- patterns;
- statements; and
- semantic bodies.

It must:

1. write the complete live checked resolution inventory exhaustively;
2. consume final checked statements, final calls, typed children, and accepted
   semantic coordinates rather than source spellings or raw IDs;
3. compute nested Match facts bottom-up;
4. include pattern and coverage meaning in each Match expression digest;
5. preserve deterministic checked `u64` work and transcript byte limits; and
6. publish no partial catalog after any failure.

The lazy Match-only transcript owner, unsupported-success identity branches,
lossy/sentinel accounting, source reconstruction, and parallel readers must be
deleted in the same completed authority switch.

### Remaining C5 closeout

Final analysis must publish a `CheckedMatch` only when it is both:

- completely transcribed by the final graph; and
- exhaustively accepted by the bounded coverage engine.

No runtime, wire, persistence, View site, or task-plan row belongs in `.1.2`.
The result is the predecessor consumed by `.1.4`.

### `.1.2` completion rule

`.1.2` is production-complete only when:

- C3 and C5 are merged and validated;
- all deleted transcript/coverage compatibility paths are absent;
- a dated implementation record supersedes the current partial records; and
- `.1.4` can point to a concrete implementation commit rather than a design
  status or local worktree.

## 3. Retained View operation/value-slot completeness `.1.4`

Current maintained request:
[retained View operation and value-slot completeness correction](../reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.4-retained-view-operation-value-slot-completeness-correction.md).

Current state: `OPEN_DESIGN_REQUEST` and `BLOCKED_BY_PREDECESSOR`.

The request still describes an older `.1.2` state and older source commit. It
must not be dispatched or implemented verbatim after `.1.2` changes. Once
`.1.2` is complete, refresh the maintained request in place while preserving
its sequence and objective.

### Decisions the refreshed design must close

- exact consumption of the `.1.2` View declaration/body path and Match
  transcript;
- complete View operation inventory;
- retained value-slot and capture inventory for nested scopes, locals,
  closures, Match bindings, View parameters, and implicit captures;
- selector input, source-ordered arm output, invalidation, and cancellation;
- production `CallTargetFacts` admission for direct, spread, compact, receiver,
  function-value, Need, and capture-bearing producers;
- transactional `CheckedViewMatchAdmission` construction;
- compiler instruction and compiler-local catalog consumed by the actual
  `CompiledViewProduct`;
- validated View resource/bundle joins without a bundle-to-compiler edge; and
- one atomic publication of persistent/runtime/replacement consumers.

### `.1.4` completion rule

The row is complete only after the request is current-source, the resulting
design is accepted with no open questions, and the executable product is
implemented through replacement/runtime behavior. A compiler-only side table
or checked row with no executable consumer receives no completion credit.

## 4. RuntimePlan semantic owner and task-plan seal `.1.3.1`

Current maintained request:
[current RuntimePlan semantic owner and View predecessor reconciliation](../reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.3.1-current-runtime-plan-semantic-owner-and-view-predecessor-reconciliation-correction.md).

Current state: `OPEN_DESIGN_REQUEST` and blocked in the exact order:

```text
.1.2 -> .1.4 -> task-plan semantic integration -> final task/runtime switch
```

The request must be refreshed only after `.1.4` publishes the actual stable
lower-layer View product that it is supposed to consume.

### Remaining result-changing boundary

The design and implementation must close, without placeholders:

- final function semantic identity, parameter/receiver modes, result, capture,
  endpoint, effect, and body authority;
- exact task request-template semantics for positional/named/spread,
  receiver/capture, AwaitMany, timeout, and line roles;
- one constructible control/effect contract;
- exact accepted record-field and variant-case projection;
- stable lower-layer View task-plan binding;
- one RuntimePlan construction coordinate and one consumed builder;
- trusted core-prefix plus upper-layer digest completion without a public sink
  or raw digest constructor; and
- atomic one-time seal/publication with deterministic first-error precedence.

No task-plan row may be published before its complete execution consumer.

## 5. Structural nominal runtime carrier

Accepted design:
[Accepted structural nominal runtime carrier](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier/README.md).

Current state: `DESIGN_READY`; final production completion is not established by
this inspection.

The accepted sequence remains C1-C6, but current source must be re-audited
before implementation because later generic-Match/runtime cuts already landed
some shared substrate. Do not duplicate those owners merely to mirror the old
cut wording.

### Work that can proceed now

First produce a current-source gap matrix against
[the accepted C1-C6 contract](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier/CUTS_TESTS_AND_DELETION.md):

| Accepted cut | Reconciliation question |
|---|---|
| C1 core schema | which RuntimeTypeSchema/record/variant/varint requirements are already owned by current core, and which structural ADT requirements remain? |
| C2 sema join | is the exact Rust metadata-to-`RustAdt` bijection, declaration order, generic instantiation graph, and stale-world rejection complete? |
| C3 compiler/plan | does the current plan admit every reachable structural nominal definition atomically and validate live values through one graph? |
| C4 AWBC | are existing tags and canonical version-one rows complete for structural nominal types, constants, patterns, and rejection cases? |
| C5 restore | is every nominal value restored only against the active program's accepted descriptors, across every snapshot-bearing runtime site? |
| C6 ownership | does structural Record/Variant ownership succeed exactly, with the fail-closed temporary branch deleted? |

Implement missing behavior on the legitimate Arcweft-owned enum/type/context.
Do not add `AcceptedRuntimeCarrier`, another runtime value algebra, an extension
trait, a free helper layer, a copied catalog, or new AWBC tags.

### Structural nominal completion rule

All six accepted outcomes, including program-bound restore and final ownership
success, must be evidenced on one current authority chain. Substrate already
landed elsewhere counts only when the current tests prove it satisfies the
accepted structural requirement.

## 6. Host scheduler and Sans-I/O restore transaction

Accepted design:
[current host scheduler and Sans-I/O restore transaction](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2-current-host-scheduler-and-sans-io-restore-transaction/README.md).

Current state: `DESIGN_READY` and `BLOCKED_BY_PREDECESSOR`.

The accepted prerequisite set is:

1. complete generic Match/path authority;
2. retained View operation/product;
3. final task-plan owner/seal;
4. structural nominal carrier; and
5. the already accepted core identity/catalog substrate.

No scheduler-local stand-in row, copied View catalog, provisional task-plan
digest, or nominal placeholder may satisfy these prerequisites.

### Final implementation graph

After the prerequisites are complete, follow
[the accepted A-F sequence](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2-current-host-scheduler-and-sans-io-restore-transaction/CUTS_TESTS_AND_DELETION.md):

- A: core protocol, state algebra, four-outcome event model, and pure snapshot
  codec;
- B: concrete `RuntimeTaskScheduler<A: TaskLaunchAdapter>` and prepared guards;
- C: native, Web, and headless concrete adapter composition;
- D: runtime-driver borrowed `TaskHost` switch and duplicate registry/queue
  deletion;
- E: outer application snapshot/save/restore transaction; and
- F: final old-reader/type/helper/documentation deletion and broad validation.

The last fallible operation remains core journal after-image application.
Scheduler swap and adapter commit after that point must be infallible and must
not allocate, reserve, look up, format, log, or return `Result`.

## Dependency and execution graph

Normative predecessor graph:

```text
Generic Match C3/C5
        |
        v
Retained View .1.4
        |
        v
RuntimePlan/task-plan .1.3.1
        |
        +-----------------------+
                                |
Structural nominal C1-C6 ------+--> Scheduler/restore A-F
                                         |
                                         v
                              Final deletion and validation
```

Operational serialization for the existing checkout:

```text
Canonical Content/shared Fx final switch
        |
        v
Generic Match C3/C5 closeout
```

The Content/Fx arrow is an operational recommendation because these cuts cross
some of the same language/View owners and because uncommitted state must be
made reviewable before adding more overlapping changes. It is not a replacement
for the normative `.1.2 -> .1.4 -> .1.3.1` predecessor graph.

Structural nominal gap analysis can proceed in parallel as read-only work. Its
production edits should begin only when they can preserve the existing working
tree and form coherent reviewable cuts.

## Reviewable outcome order

The next durable outcomes should be ordered as follows:

1. **Content/Fx final public vertical** — one typed producer/catalog/identity
   authority, all consumers migrated, obsolete paths deleted.
2. **Generic Match final transcript graph** — expressions, patterns,
   statements, and bodies sealed by one accepted-rooted graph.
3. **Generic Match atomic publication** — only exhaustive and fully
   transcribed Match rows publish; old transcript/publication paths deleted.
4. **`.1.4` request refresh and accepted design** — based on the concrete
   `.1.2` implementation commit.
5. **`.1.4` executable View product** — slots, captures, subscriptions,
   invalidation, bundle/runtime/replacement consumers.
6. **`.1.3.1` request refresh and accepted design** — based on the concrete
   `.1.4` product.
7. **Task-plan semantic seal implementation** — one consumed builder and
   atomic product publication.
8. **Structural nominal current-source C1-C6 closure** — scheduled as coherent
   cuts and completed before scheduler integration.
9. **Scheduler/restore A-F** — final runtime host/driver/snapshot switch.
10. **Goal-wide final gate** — all broad tests, codecs/goldens, structure,
    applicable Tier 2, and deletion evidence.

Do not create a commit for a temporary public carrier merely to make an
intermediate subset compile. Private construction stages may exist only inside
a cut whose final authority and deletion set land together.

## Request handling

No new external request is currently required merely to continue the known
chain.

- `.1.2` is already design-resolved; finish its production closeout.
- Refresh `.1.4` in place after `.1.2` production completion.
- Refresh `.1.3.1` in place after `.1.4` production completion.
- Structural nominal and scheduler/restore already have accepted designs; do
  not redispatch them unless current source proves a result-changing
  contradiction.
- Create a new correction request only when a current-source constructibility,
  ownership, dependency, or phase contradiction cannot be resolved by the
  accepted authority. Record the exact blocker and forbidden guessed
  implementation.

A refreshed request must record the actual full predecessor commit, current
owner/consumer inventory, applicable `AGENTS.md`, and the no-final-package rule
when repository preflight or validation fails.

## Validation ledger for future implementation cuts

Each reviewable Rust cut records the commands actually run and distinguishes
passed, failed, blocked, and not run. The selected scope is derived from current
[test execution policy](test-execution-policy.md) and
[structural audit policy](structural-audit-policy.md).

Expected final goal evidence includes, as applicable:

- `cargo fmt --all -- --check`;
- workspace/all-target/all-feature compile checks;
- focused and changed-crate test suites;
- workspace tests;
- workspace Clippy at the selected gate;
- canonical structure audit and fail-on-blocking gate;
- deterministic version-one codec and whole-product goldens;
- wrong-version, noncanonical-varint, duplicate, stale-generation, foreign
  owner, and transactional rollback negatives;
- applicable runtime/View/bundle/native/Web/Agent Tier 2 tests; and
- a deletion audit showing no old reader, alias, fallback, parallel catalog,
  provisional owner, or version-not-`1` marker remains.

Environment failures such as linker or paging-file exhaustion remain failures
or blocked validation; they are never reported as passed.

## Performed for this status note

- inspected the latest accepted `main` commit and recent relevant commits;
- read root, workspace, documentation, and implementation evidence
  instructions;
- read the maintained Content/Fx surface;
- read the `.1.2`, `.1.4`, and `.1.3.1` request/design boundaries;
- read the structural nominal and scheduler/restore accepted cut graphs; and
- separated main-level evidence from unobservable local worktree claims.

## Not run for this status note

- Cargo formatting, checks, Clippy, tests, benchmarks, or Tier 2;
- generated artifact comparison;
- local working-tree, index, or untracked-file inspection; and
- production implementation.

This is a documentation-only status cut.