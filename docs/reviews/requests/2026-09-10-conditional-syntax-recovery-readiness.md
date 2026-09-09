# Conditional syntax recovery and executable readiness

Date: 2026-09-10. Parent work: the
[connected convergence goal](../../implementation/2026-09-08-convergence-goal-plan.md).
This is an independently usable design request, not an implementation-ready
contract or a request for user approval.

## Evidence and precedence

Inspected Git base: `43d4270b4a3b9d813186fa49ed5029591875c416` on `main`.
The working tree contained the shared Pattern completion/source-validation
correction described in the
[implementation record](../../implementation/2026-09-10-recovered-closure-projection.md).
The reproduction below was also run through the compiler on that corrected
working tree. Record the full current Git SHA and dirty state again when
performing this design work; do not assume the base is still the latest source.

Current source, maintained language rules, and accepted typed ownership
contracts outrank historical summaries or filenames. Preserve accepted,
validated substrate unless a concrete current-source flaw requires replacing
it. In particular, preserve the typed Pattern grammar, retained lossless
candidate graphs, exact source identities, and shared Ruby desugaring recipe.

The maintained
[dialogue/index rule](../../01-language/dialogue-line-handles-and-returns.md)
requires interpretation from the target type/context. Its sentence assigning
type-based resolution to HIR lowering must be reconciled with the current
syntax → HIR → sema dependency direction. Ruby remains supported as specified
in [Content/Ruby syntax](../../01-language/dialogue-content-actions-ruby-and-interpolation.md).

## Reproduction and reason for the separate boundary

```arcw
pub character alice { display = "Alice" }
flow main() -> Unit { alice()[|[夢](ゆめ)] }
entry cli @entry.main { goto @flow.main }
```

The bracket owns two retained syntax interpretations. Dialogue contains valid
Ruby. Index contains a recovered Closure: the sequence Pattern `[夢]` is
followed by `(ゆめ)`, and the Closure terminator/body are absent.

After the shared Pattern correction, both syntax projections are internally
consistent and the complete HIR module publishes. It has `Recovered` status.
A real `compile_project` probe then fails in readiness with
`hir.project.execution`, before the semantic candidate selector can select
Dialogue. This is a different failure from the earlier
`hir.lower.project_transaction` source/payload mismatch. Neither successful
HIR publication nor a request document closes executable Ruby acceptance.
The rebuilt CLI independently reproduces this readiness failure. A separate
control with `alice()[｜夢《ゆめ》]` passes `arcw check`; that control does not
establish runtime/AWBC acceptance for the recovered-candidate form.

Current coupled rules include:

- `SyntaxPostfixBracketProjection::has_recovery` reports recovery when either
  retained candidate recovers.
- HIR retains both candidate expression graphs and their Pattern/Type/Local/
  Scope descendants. Candidate poison and source recovery must stay honest.
- Module readiness currently treats any poisoned live slot, or recovered
  parse status, as document-wide recovery.
- Project readiness rejects recovered modules before semantic interpretation
  selection. Cache admission also depends on the module's eligibility.

Correcting Pattern region ownership is independently valid. Selecting how
conditional candidate recovery affects diagnostics, readiness and later seals
is one different, mutually constraining authority. Do not make only one of
those consumers permissive or erase poison to make this example pass.

## Decisions to close together

1. Define the final ownership and meaning of unconditional source recovery,
   alternative-local recovery, candidate viability and selected semantic
   evidence. Decide which owner issues each fact and at what phase it closes.
   Specify how an ordinary recovered document remains inspectable but cannot
   execute or enter executable caches.
2. Define admission and diagnostic behavior for each pair of candidate
   outcomes: both clean, only Index clean, only Dialogue clean, neither clean,
   semantic rejection of a syntactically clean candidate, and two semantically
   valid candidates. A syntax success alone is not permission to reinterpret
   a collection as Dialogue or accept a malformed chosen branch.
3. Close nested alternatives, shared targets, captures, typed parameters,
   local generations, generated Content expressions and branch-local errors.
   Prove exact containment and ownership without deriving identity from
   source ranges, strings, filename patterns or a copied membership catalog.
4. Specify module/project readiness, semantic ingress, selected-expression
   publication and execution/cache evidence as one coherent model. Define
   when rejected alternative diagnostics are visible to tooling and when they
   block compilation; they must neither disappear nor poison an unrelated
   successful interpretation.
5. Enumerate producers and consumers to migrate, obsolete contracts to delete,
   transaction rollback, incremental invalidation, diagnostic budgets and
   structural traversal limits. Preserve deterministic ordering, complete
   consumer coverage and version `1` throughout.

Architecture selection must follow domain ownership, layer direction and the
complete consumer needs. Do not choose by patch size, migration cost, type
count or ease of changing the readiness predicate. No source reparsing,
fallback resolver, parallel model, unchecked poison waiver or new compatibility
reader is allowed.

## Consumer inventory to inspect

- Syntax: `expressions/dialogue.rs`, candidate grammar/events in
  `parser/expression/`, `attachment/expression/`, parse status and diagnostics
  in `incremental/`, and shared Pattern/Type projections.
- HIR: `dialogue_application.rs`, `final_lowering/expression_lowering/dialogue/`,
  `final_lowering/pattern_lowering.rs`, `source_index/expression_manifest/`,
  `module.rs`, `final_project.rs`, slot/source inventories and recovery
  diagnostic validation. Include all candidate descendant arena families.
- Sema: `final_analysis/analyzer/expressions.rs` candidate probes/selection,
  `final_analysis/analyzer/executable_ingress.rs`, prepared transaction replay, closed
  expression publication and checked project evidence.
- Compiler/runtime: `project.rs`, `source.rs`, selected-expression consumers,
  runtime-plan lowering, native execution, AWBC lowering/verification/codec
  and executable cache admission.
- Tooling: compiler tooling leases, LSP diagnostic/publication paths, source
  queries, syntax/HIR cache versus executable cache boundaries.

All paths in this inventory are beneath their named workspace crates. Read
the complete relevant owners and their maintained contracts during design;
the inventory is not a claim that an old request already settled them.

## Required acceptance and implementation order

Close the ownership/readiness model first, then migrate every producer and
consumer and delete replaced paths. Add typed evidence and adversarial tests
before granting executable/cache eligibility. Required behavior includes:

- The reproduction above and both retained Ruby spellings compile; exact base
  text and annotation reach the runtime Content catalog, native execution and
  verified, codec-round-tripped AWBC.
- Ordinary `|_ trailing| 0` and `|[value] trailing| 0` remain rejected. The same
  errors inside a selected Index interpretation cannot execute.
- A valid selected Index with invalid Dialogue syntax succeeds by the same
  general rule. Nested alternatives work in both directions; a recovered
  shared target and a document with no viable interpretation remain blocked.
- Both-clean ambiguity, semantic candidate rejection, losing-branch locals/
  captures, source order, generated Ruby children and replay/rollback remain
  correct and deterministic.
- Missing/forged branch membership, poison erasure, changed recovery counts,
  stale snapshot evidence and partial selected publication fail atomically.
- Source/IDE diagnostics remain queryable; only eligible selected programs
  enter runtime or executable caches. Incremental edits invalidate that
  evidence in both directions.
- Run focused syntax/HIR/sema/compiler/LSP tests, workspace check/Clippy/tests,
  required doctests, structural audit and matching Tier 2 under the current
  [test policy](../../implementation/test-execution-policy.md). Distinguish
  passed, failed, blocked and not-run tiers.

The design request covers this complete boundary, not general higher-order
effect inference, nominal C1–C6, retained View or scheduler/restore. Those
remain required under the parent goal. No external authority is currently
identified as a blocker; repository-resolvable decisions must be closed by
the design work rather than returned as questions solely because they are
large.

## Deliverable

Design-only: do not edit production, tests, manifests or implementation
overlays while answering this request. Return one archive named
`arcweft-conditional-syntax-recovery-readiness-v1.zip`, with all sidecars inside.
Include the request copy, final contract/type ownership, consumer migration
and deletion inventory, adversarial acceptance matrix, repository evidence
with full Git SHA, and member hashes/byte lengths in the manifest.

Use `READY_FOR_IMPLEMENTATION` only after every result-changing decision is
closed and `OPEN_QUESTIONS.md` contains exactly `none`. Otherwise name the
specific external unresolved authority. The subsequent implementation and
validation are still mandatory work; archive delivery is not goal completion.
