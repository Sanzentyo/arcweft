# Design readiness and dispatch order

- Date: 2026-07-16
- Repository baseline: Git `5a36cd0af83085179c299ef50ec8aa786ed731aa`
- Jujutsu change: `nowqxzku`
- Status: current dispatch authority for remaining package-driven design work

## Landed substrate

The CSS/Takumi route, native logical-axis core, logical-axis host seed/provider
invalidation, Character nominal registration production reconciliation, and the
safe proof-concurrency identity/reference/assertion substrate are implemented,
validated, and pushed to `main`.

The requests below must consume those owners directly. They must not recreate
source identity, project symbol, Character registration, axis revision,
resolver, or removed-syntax compatibility authorities.

## Dispatch now

The following final-design tasks are independent and may run in parallel. Give
each request to a separate worker. Do not combine their return archives and do
not give one request or ZIP to more than one worker.

| Task | Sole requirements brief | Required outcome |
| --- | --- | --- |
| AW-AH-007/008 | [typed RichText attribute validation](../reviews/requests/2026-07-14-aw-ah-007-008-typed-rich-text-attribute-validation.md) | One decision-complete schema, diagnostic/recovery, migration, codec, and test contract. |
| AW-AH-009.2.1 | [Character definition/source-index](../reviews/requests/2026-07-16-aw-ah-009.2.1-character-nominal-definition-source-index-contract.md) | One typed descriptor-to-definition/use-site index contract over the landed 009.1.1 world. |
| AW-AH-009.3 | [Character signature help](../reviews/requests/2026-07-14-aw-ah-009.3-character-nominal-signature-help-contract.md) | One position-aware semantic contract or a complete evidence-backed non-goal. |
| proof 01.1.1 | [typed-AST identity, proof block, and HIR reconciliation](../reviews/requests/2026-07-16-seq-proof-01.1.1-typed-ast-syntax-identity-proof-block-reconciliation.md) | One final contract for the remaining proof cut-1 identity/body/arena/fault seams. |
| View d.2.1.1.1 | [exported-part production reconciliation](../reviews/requests/2026-07-15-seq-06.11d.2.1.1.1-view-exported-part-authoring-production-reconciliation.md) | One corrected authored-to-product-to-runtime contract. |
| Style d.4.1.2 | [native physical box geometry](../reviews/requests/2026-07-14-seq-06.11d.4.1.2-native-physical-box-geometry-contract.md) | One physical layout/render/input/focus/scroll geometry contract. |
| Style d.4.2.1 | [native environment conditions](../reviews/requests/2026-07-15-seq-06.11d.4.2.1-native-environment-style-condition-production-reconciliation.md) | One corrected environment syntax/product/session/invalidation/tooling contract. |

These are design tasks, not permission to edit production Rust. A final design
may contain API sketches and implementation order, but must not include a
speculative repository overlay or claim implementation validation.

## Gated requests

- [AW-AH-009.2.2 Character rename/atomic edit](../reviews/requests/2026-07-16-aw-ah-009.2.2-character-nominal-rename-atomic-edit-contract.md)
  follows acceptance of the AW-AH-009.2.1 definition/source-index design. Its
  later implementation must consume the landed source index rather than run a
  workspace text replacement.
- Style d.4.3 design follows implementation and validation of d.4.1.2 and
  d.4.2.1. The already landed d.4.1.1 is also fixed input.
- [Style d.5.1.1 trace reconciliation](../reviews/requests/2026-07-14-seq-06.11d.5.1.1-native-style-trace-contract-reconciliation.md)
  follows implementation and validation of d.4.3. It must consume the landed
  geometry, environment, and container identities and revisions.
- Style d.5.2 Agent observation and d.5.3 LSP/formatter follow the accepted and
  implemented d.5.1.1 trace contract. Their designs may then run in parallel.
- proof cuts 2 through 11 remain sequentially gated. Do not dispatch cut 2
  production work until proof 01.1.1 is accepted and the remaining cut-1
  implementation has landed and passed its full completion matrix.

## Integration order after designs return

Design work above may run concurrently, but production cuts that share owners
must remain reviewable and sequential:

```text
View d.2.1.1.1 implementation
  -> environment d.4.2.1 implementation

physical geometry d.4.1.2 implementation ─┐
environment d.4.2.1 implementation ───────┴-> d.4.3 design/implementation
                                                -> d.5.1.1 design/implementation

Character 009.2.1 design/implementation
  -> 009.2.2 design/implementation

proof 01.1.1 design
  -> remaining proof cut-1 implementation
     -> proof cut 2
```

View and environment designs are parallel-safe, but their implementations
overlap source maps, resolver, codec, player, formatter, and LSP owners. Land
View first, then make environment consume the resulting types. Physical
geometry is a separate predecessor branch and may progress concurrently.

AW-AH-009.3 design may run beside proof 01.1.1. If its final query requires
stable typed-node identity, its implementation must wait for proof 01.1.1; if
the design proves exact document/range identity is sufficient, it must state
that evidence and may avoid the dependency.

AW-AH-007/008 is independent of these Character, proof, View, and Style chains.

## Dispatch message

Create one new task per row in **Dispatch now**, attach only the linked request,
give it access to latest `main`, and send:

> Treat the attached request as the sole requirements brief. This is a
> design-only task: do not modify production code. Inspect latest `main`, all
> applicable `AGENTS.md`, and the required Rust skill. Close every required
> decision with exact owned APIs, errors, limits, migration/deletion order, and
> direct behavioral tests. Preserve the request's fixed landed substrate and
> do not add compatibility shims, dual readers, source gates, CSS/Takumi paths,
> or removed-syntax recognizers. Return exactly the independent final-contract
> archive and external status/hash artifacts named by the request. If any
> result-changing choice remains open, return `NOT_READY` and identify it
> instead of guessing.

No historical ZIP is required unless the request explicitly names it as a
required input. Rejected or superseded ZIPs are evidence only and must not
become a second source of truth.

## Intake gate for returned designs

Reject a returned design before implementation when any of the following is
true:

- it contains `TBD`, surviving alternatives, or implementation-selected
  policy that changes an observable result;
- it was not reconciled against the recorded latest repository identity;
- it redesigns a landed predecessor without a reproducible defect;
- prose, Rust sketches, wire shapes, diagnostics, examples, and tests select
  different models;
- ownership, dependency direction, limits, exact/one-over behavior, atomic
  failure, or caller deletion inventory is missing;
- it uses source-text searches as automated correctness evidence;
- `OPEN_QUESTIONS.md` is not `none`, required traceability is incomplete, or
  the machine status does not say `READY_FOR_IMPLEMENTATION`; or
- manifest hashes, archive SHA-256, or claimed validation evidence cannot be
  reproduced.

After acceptance, implement each contract as its own coherent cut, run the
required focused and workspace validation plus structural audit, commit, and
push it before starting an independent implementation package.

## Validation of this request cut

- Changed Markdown files: 13; every relative link resolves and every backtick
  or tilde code-fence sequence is balanced.
- Added-line trailing-whitespace, conflict-marker, truncation-artifact, required
  request term, and required request-file scans: pass.
- Canonical structural audit dry run:
  2,860 files, 1,408 Rust files, 661,972 physical Rust LOC, 90 manifests,
  0 errors and 128 warnings.
- No Rust, Cargo, schema, fixture, or production design chapter changed, so no
  compile or runtime test was required for this documentation-only cut.

The colocated Git index still names the pre-proof parent while Jujutsu owns the
current proof parent and request changes. A reverse `git apply --check` against
that stale index also treats pre-existing CRLF lines in full-document rewrites
as trailing whitespace, so it is not valid evidence for this cut. The final
check instead inspects normalized added lines from `jj diff --git` directly and
passes with no whitespace or conflict finding.
