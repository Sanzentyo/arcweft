# Conditional recovery and selected program admission

Date: 2026-09-10. Inspected base:
`a44835906f70d0b12343448e24192e5556cc22d6`, existing `main`, initially clean.
This records the selected implementation model for the
[conditional recovery request](../reviews/requests/2026-09-10-conditional-syntax-recovery-readiness.md).
It supersedes the unselected investigation in local validation scratch.
The [implementation and validation record](2026-09-10-conditional-recovery-validation.md)
contains native/AWBC execution evidence and the remaining parent-goal failures.
This design note does not complete the parent convergence goal.

## Ownership and admission

1. Syntax owns recovery as a composition of required children and alternative
   interpretations. The states are clean, conditional recovery, and required
   recovery. Required composition preserves any required error; alternative
   composition admits a possible completion only when at least one alternative
   has one. Neither operation selects a semantic interpretation. Missing or
   failed parses remain distinct from retained recovered parses.
2. Retained candidate graphs own their diagnostics and missing-token evidence,
   in addition to the existing tokenless typed nodes. The graph remains bound
   to the exact source snapshot. Diagnostics of a candidate are queried through
   that candidate; they do not become unconditional document diagnostics.
   The same source error outside an alternative still blocks compilation.
3. HIR source freeze issues candidate provenance while checking the complete
   attached-to-HIR preorder. The existing temporary descendant sets become that
   sealed provenance rather than feeding a second membership reader. Each
   expression, statement, type, pattern, scope, and local records its immediate
   interpretation region. Each region names its selector, retained root and
   enclosing region. Generated Ruby descendants use the same region. Shared
   targets belong to the enclosing region, never to either alternative merely
   because that alternative references them.
   Semantic-only Content nominal roots inherit that region only after the
   existing exact content-call recipe validator admits their producer relation.
4. Existing source-qualified synthetic keys and the aggregate 1,024-descendant
   limit keep their meaning. Nested alternatives do not reset the source owner,
   role, counters, or budget. Provenance describes containment, not a replacement
   identity or a source-range lookup.
5. HIR analysis admission replaces the current claim that a raw HIR view is
   executable. It accepts clean modules and modules whose recovery is wholly
   conditional and source-validated. Unconditional syntax or HIR poison remains
   rejected. Tooling keeps the complete immutable module. No poison bit is erased.

The same source freeze now completes the existing HIR source index with the
canonical expression, Type and Pattern components of retained candidate nodes.
It reuses the ordinary component requirements and typed grammar projections;
the rows move into the sole index only after candidate and arena validation.
Candidate paths, generated nominal references, and named scope components are
therefore queryable without a second reader or inferred source spelling.
Block statement/tail components come from the typed block view. Hash content
with an explicit empty bracket body retains its bracket components even when
the missing content itself is recovered.

The source-line inventory includes authored Dialogue applications nested in
either retained interpretation. A selector still owns the source occurrence
of its alternative root; selection filters the inventory before generated line
IDs and collisions are accepted. This preserves named scope and source order
without assigning a line ID to a rejected interpretation.

## Semantic closure

Candidate-owned type preparation and local/pattern seeding run in the owning
candidate probe. Type resolution may memoize context-independent source type
facts, but final publication admits only selected owners. Candidate-local
binding, expression, callable and effect facts retain the existing affine
transaction and rollback authority. Budget/cancellation/invariant failures
remain fatal; they are never reclassified as an ordinary candidate mismatch.

A candidate with required syntax recovery is rejected explicitly before its
body is analyzed. An otherwise valid candidate still has to satisfy the target
type and all semantic constraints. Two valid interpretations remain ambiguous;
zero valid interpretations remain an error. Nested selection uses the same
rule. A malformed shared target is not made conditional by its references.

The existing HIR evaluation topology remains the structural evaluation owner.
Authored control-target failures are retained as typed resolution outcomes on
its control-transfer rows; invalid scope graphs and duplicate/missing topology
owners remain structural failures. Semantic probes admit or reject authored
outcomes in their exact region. Ordinary control-flow errors remain errors.

The selected expression graph closes the selected inventory for every HIR
arena family, not just expressions. Its existing chosen edges supply selection;
the module's sealed provenance supplies conditional ownership. Final semantic
inventory checks, type reports, runtime-plan lowering and executable cache
admission consume this closed inventory. Missing selections, extra facts,
poisoned selected owners and stale generations fail before atomic publication.

## Captures

A closure's first captured source span and unioned access mode are insufficient
evidence for alternative selection: a later use may occur in the winning
interpretation, and a losing reassignment must not upgrade a selected read.
The capture owner therefore retains complete typed use sites with access and
source evidence. Ordinary paths, record shorthand and reassignment feed that
same inventory. First-use order and aggregate access are projections of those
uses, not independently authored fields.

Capture closure selects uses by the same interpretation evidence. Nested
closure uses retain every crossed lexical capture boundary. Selection removes
neither lexical captures from ordinary runtime branches nor captures needed by
a selected nested closure. The local closure proof is reconciled with the final
project selection before execution; another generation or inconsistent choice
cannot validate it.

Runtime postfix selection is structural evidence and is projected even when the
selector forwards its child's value without retaining a runtime type of its
own. In Flow, value blocks with statements follow the existing block sequencing
lowerer; their statements cannot be hidden by a traversal of only direct value
children. Independent Seq/Stream and latent closure boundaries retain their
existing ownership.

## Callable body closure

A callable's registered signature and effect contract are available before its
body is checked. Its final execution role is not. Pending body records therefore
leave execution unassigned, alongside pending effect, suspension and control
evidence. Fixed and dispatch records retain their fixed roles. Final catalog
validation rejects any body with an unassigned or duplicate execution role.

The existing function rules require zero selected own-scope yields for direct
return and at least one for a Stream factory. Postfix selection also consumes an
expected result type, so counting raw yields before checking that result creates
a cycle. Close these constraints together: a non-Stream result admits only direct
return, checked through the ordinary publication context. A declared Stream
result admits direct-return typing and generator typing with a Unit body result.
Those alternatives run in the existing affine fact transaction and must satisfy
their own selected yield conditions. Exactly one successful interpretation is applied
atomically; none is an invalid body and multiple self-consistent interpretations
are ambiguous. No failed probe publishes facts or refunds its physical work.
Latent closures, independent sequence/stream computations and thread bodies keep
their existing execution boundaries.

This is an inference constraint over the existing two function execution rules,
not a new source spelling or a provisional DirectFrame runtime fact. The final
execution role is derived from the selected body and assigned during callable
catalog completion. Statement expressions with discarded values are completed
in their containing body before capture or execution-role sealing.

## Diagnostics, caching and limits

Unconditional parser diagnostics keep their current compiler/IDE publication.
Candidate diagnostics remain available from source-bound tooling views even
when another candidate succeeds. A rejected interpretation does not publish
runtime facts or an unconditional compile error on a successful program.
Neither parser work nor diagnostic work is refunded when a probe loses.
Retained diagnostic validation uses structured identity and the existing
document limits; no source text is reparsed to reconstruct evidence.

Fragment attachment rebases retained graphs, diagnostics, missing-token anchors,
dialogue recovery boundaries, raw bodies and mark components directly. The same
rebase context preserves shared authored Type and Pattern trees across ordinary
events and nested candidate graphs. Candidate preorder identities and edges do
not change when their source offset changes.

Source/HIR reuse and executable cache admission are separate contracts. The
former can retain conditionally recovered syntax/HIR after exact snapshot
validation. The latter requires the selected semantic program and complete
execution seals. Edits invalidate both positive and negative selection evidence
through the existing source/snapshot/symbol generations. All contract versions
remain `1`.

## Migration and acceptance

Migrate syntax graph construction, attachment and recovery publication; HIR
source freeze, module/project admission, capture lowering/validation and
evaluation topology; semantic preparation/probes/selected inventory/captures;
compiler cache publication and runtime-plan consumers; and source/tooling
queries. Delete the two-state candidate-quality model, raw-HIR executable
claims, first-use-only capture authority and unconditional arena completion
checks they replace. Do not leave aliases or an old reader.

Acceptance remains the complete request matrix: both Ruby spellings through
native and codec-round-tripped verified AWBC; ordinary and selected malformed
closures rejected; valid Index with invalid Dialogue; nested alternatives in
both directions; shared-target and zero-winner rejection; two-winner ambiguity;
source order, captures, generated children, transaction replay/rollback; forged
membership/recovery and stale generation rejection; incremental cache changes
in both directions; focused tests and the applicable workspace, documentation,
structure and Tier 2 gates.

No external blocker or compatibility exception has been identified. The
remaining convergence work (higher-order effects, Match, View, RuntimePlan,
nominal contracts and scheduler/restore) is unchanged.
