# Lang-01.5.1.1.2.1 — reactive unary-Need match reconciliation

## Sequence and precedence

This is a narrow mandatory correction of Lang-01.5.1.1.2 final-HIR View
execution and the direct-await rows corrected by Lang-01.3.1.2.3.1. It follows
the maintained unary-Need/Await convergence now present on `main`.

Inspected production baseline:
`680b7c42005febeb2a9f9c8b387669b729b7463c`.

Preserve the accepted parent package's final-HIR checked View catalog,
`ViewInstruction::Match`, ordinary RuntimeValue/AWBC dynamic execution,
ownership checks, static certification, typed resources, transactional product
publication, work limits, save/replay, and hot-replacement decisions. Correct
only the rows that assume direct Await or Need-owned error/denied branches.

Current maintained language authority takes precedence:

- View cannot suspend with `await`;
- reactive `Need<T>` observation uses ordinary `match` grammar projected by
  View sema into one retained subscription and typed branch owner;
- Need owns `NotStarted`, `Pending(Progress)`, `Ready(T)`, and `Cancelled`;
- domain failure is a Result payload inside Ready; and
- `AwaitView` is not a parser, HIR, formatter, LSP, product, or runtime surface.

## Split reason

Current production has a live but unreleased View-product `Await` instruction
whose value program returns an integer discriminant and whose branches are
pending/ready/error/denied. No compiler source path emits it. Final semantic
View facts do not yet retain a Need subscription or typed match, View value
programs cannot carry Need/Progress/arbitrary payloads, and runtime-driver View
evaluation has no canonical Need publication input. Deleting or locally
renaming the old instruction would not select the missing identities, binding
rules, invalidation, snapshot, or failure precedence.

## Required decisions

1. Define the sole checked semantic owner for a View-context match over
   `Need<T>`, including final-HIR expression/pattern identities, accepted
   generation, exact `T`, source-ordered arms, exhaustiveness, bindings, source
   roles, effects, and ownership disposition.
2. Define the retained subscription identity. Decide exactly how a checked
   Need-producing expression maps to `NeedId`/producer identity without source
   strings, copied endpoint tables, or a runtime-value handle surrogate.
3. Define publication input and deterministic selection for View evaluation:
   `NotStarted`, canonical `Pending(Progress)`, `Ready(T)`, and `Cancelled`;
   epoch/sequence ordering; repeated/coalesced publications; first frame;
   invalidation; and multiple mounts observing the same Need.
4. Define how ordinary pattern matching and arm-local bindings execute through
   the parent package's generic `ViewInstruction::Match` and ordinary
   RuntimeValue/AWBC owner. Do not extend presentation-only `FxRuntimeValue`
   into a second generic value model.
5. Define Result/Option payload nesting. There must be no Need-owned error or
   denied branch; admission denial remains outside the Need and cancellation
   remains temporal control.
6. Define start ownership. State whether observing `NotStarted` starts the
   producer, only subscribes, or requires an already-started Need, and identify
   the sole Sans-I/O request owner and deduplication key.
7. Define save/replay and hot replacement for active subscriptions, last
   accepted publication cursor, arm-local retained state, mount occurrence,
   producer identity, and queued invalidation. Keep every Arcweft-owned version
   marker fixed at `1`; if this cut touches a non-`1` marker, reconcile it to
   `1` rather than adding another reader or version.
8. Define strict bundle wire replacement and deletion order for
   `ViewProgramInstruction::Await`, `ViewAwait`, `ViewAwaitBranchSpan`, the
   four-way evaluator, `InvalidAwaitState`, codec rows, merge/fingerprint
   branches, tests, and the stale direct-await parent rows. No compatibility
   alias or old reader may remain.
9. Reconcile static certification: any live Need subscription is dynamic; an
   authored static requirement fails through the parent's ordinary typed proof
   path with the exact first contaminant.
10. Define diagnostic precedence and atomicity across sema, compiler product,
    strict decode, runtime publication, pattern mismatch/exhaustiveness,
    ownership rejection, stale generation, save restore, and replacement.
11. Provide a compile-clean implementation interleave that first lands the
    parent catalog/generic Match substrate needed by this correction, switches
    every consumer atomically, and then deletes the old Await model.
12. Provide exact bounded work accounting for subscriptions, publications,
    patterns/arms, payload depth, mount fanout, queued invalidations, and
    restore/replacement validation.

## Required consumer inventory

Inspect and cover at least:

- maintained View/Need language chapters and ordinary match grammar;
- syntax/HIR match expressions and patterns in View context;
- final semantic View catalog and checked unary Need/Progress/Result owners;
- compiler View product lowering and RuntimePlan/AWBC dynamic programs;
- `arcweft-view` program, dependency graph, mount identity, and local state;
- bundle View model/codec/validation/merge/semantic digest/source maps;
- runtime-driver evaluator/catalog/replacement/save and runtime Need
  publications;
- native/Web/headless/Agent observation consumers; and
- all current Await/Need/View tests plus the parent package matrices whose
  direct-await rows are superseded.

## Required tests

- NotStarted, multiple Pending Progress values, Ready payload, and Cancelled;
- first publication, duplicate/stale/out-of-order publication, same-step
  progress-to-ready, and replay/save restore;
- `Need<T>`, `Need<Result<T,E>>`, and `Need<Option<T>>` with nested ordinary
  patterns and arm-local bindings;
- source-order/exhaustiveness diagnostics and no-match behavior selected by
  the ordinary match contract;
- two mounts and two observers of one Need, remount, hot replacement, and stale
  producer generation;
- producer start/dedup/cancellation ownership and no hidden I/O;
- affine payload/capture rejection under the accepted correction;
- static requirement rejection with exact contaminant;
- codec tamper, invalid pattern/type/Need identity, cursor corruption, and
  transactional no-partial-publication cases;
- compile-fail/API proof that old Await types/variants/discriminants and
  `AwaitView` are unavailable; and
- focused, workspace, Clippy, doc, structure, save/replay, native/Web/headless,
  and Agent parity gates.

## Constraints and non-goals

- Do not redesign accepted final-HIR View catalog, generic Match, RuntimeValue,
  ownership, resource, or static-proof decisions without a concrete flaw.
- Do not restore direct Await in View, `AwaitView`, error/denied Need branches,
  a View VM, a `ViewRuntimeValue`, source reconstruction, string identity,
  copied endpoint catalogs, compatibility aliases, shims, or dual readers.
- Do not implement timeout, Stream/Watch observation, broader mount syntax,
  Dialogue/Ruby, Choice, CSS, or Takumi in this correction.
- Keep lower crates Sans I/O and all behavior deterministic.

## Expected output

Return one independently usable design-only archive named
`arcweft-lang-01.5.1.1.2.1-reactive-unary-need-match-reconciliation-final-contract.zip`.
It must contain `OPEN_QUESTIONS.md` exactly `none`, exact Rust-shaped owners and
APIs, wire/save allocation and deletion matrices, corrected parent-row
supersession, work limits, failure precedence, compile-clean implementation
order, and full positive/negative/tamper/Tier-2 tests. Do not include a
production code overlay.
