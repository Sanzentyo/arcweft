# Generic call scope: design completion and implementation boundary

- Date: 2026-09-07
- Inspected accepted Git SHA:
  69b30b530f5f1da39f5c8f0d7ee7f0f3de70bd29
- Checkout: existing main, HEAD/origin agreement after fetch, initially empty
  index and 658 dirty/untracked status entries.
- Supersedes the unresolved generic-scope design status in
  [the preceding continuation note](2026-09-07-content-callable-continuation.md).
  It does not supersede that note's historical Rust validation results or
  establish that the parent implementation is complete.

## Result

The [AW-AH-009.4.2.1.1 request](../reviews/requests/2026-09-07-aw-ah-009.4.2.1.1-recursive-generic-call-constraint-scope-reconciliation.md)
has a locally authored, decision-complete
[contract](../reviews/packages/arcweft-aw-ah-009.4.2.1.1-recursive-generic-call-constraint-scope-reconciliation-final-contract/FINAL_CONTRACT.md),
with Rust ownership, 20 validator rows, 30 acceptance rows and an ordered
replacement plan. Status is READY_FOR_IMPLEMENTATION; production status is
NOT_IMPLEMENTED. This is not a new external returned-package claim.

Selected boundaries are:

- declaration IDs remain declaration identities; one TypeKind algebra uses
  Free, Bound and per-application Inference references for types/consts/effects;
- the schema owns its binder and the existing preparation gate owns opening,
  source transactions, normalization and the exact inherited solution;
- frozen prefixes own canonical residual binders; caller-rigid references
  cannot alias their still-unbound callee parameters;
- closed runtime function schemes are admitted through the existing type
  graph, and continuation input retains its original scheme identity;
- closed instances keep one normalized substitution, with a bounded
  deterministic worklist replacing recursive discovery and layer history.

The stable [functions chapter](../01-language/functions-and-pipeline.md) records
the selected scope/partial-application semantics. Its previously dirty pipe
and closure edits are preserved and excluded from this design commit.

## Additional reproduction

During initial implementation investigation, a temporary compiler integration
probe compiled:

~~~arcw
fn choose<A, B>(first: A)(second: B) -> B { second }

flow main() -> i64 {
    let prefix = choose(1i64)
    let text = prefix("text")
    return prefix(2i64)
}
~~~

Sema accepts this source. Runtime-plan projection rejects the still-open B
with compiler.runtime_semantic_projection. This proves that separating the
recursive map keys alone would leave future-generic continuation values
unrepresentable. The probe's own test edit was removed before the design
artifact was composed; no inherited test or Rust change was discarded.
The complete reproducer and actual command output are retained inside the ZIP.

The existing recursive sema test remains enabled and failing. This cut adds
no Rust implementation and does not claim a repaired compiler baseline.

## Validation actually performed

| Command / inspection | Actual result |
|---|---|
| Git status/index/fetch/HEAD comparison | Existing main; empty initial index; accepted SHA above; no divergence |
| cargo test -p arcweft-lang-sema --lib final_analysis::tests::generic_calls | FAILED: 3 passed, 1 failed, 681 filtered out; recursive MalformedSchemaInventory |
| cargo test -p arcweft-compiler --test project_function_instances one_curried_prefix_supports_distinct_later_generic_arguments -- --exact | FAILED: 0 passed, 1 failed, 6 filtered out; runtime type projection rejects B |
| Current source/contract inspection | Declaration IDs, scopes, normalization, frozen rows, caller-layer closure, type graph, continuation ABI and callback consumers inspected |
| Package member/path/hash verification | PASSED: 16 members, 14 substantive member hashes, exact extracted/source bytes, no manifest self-entry |
| Repository input inventory | 65 input paths recorded with SHA-256 and byte length inside the package |
| Existing review ZIP comparison | 70 previous ZIPs rehashed; zero hash-multiset changes from the preceding inventory; no root inbox ZIP |

An initial staging whitespace check and an ordinal-sort attempt failed; the
authored ZIP was regenerated from its source, re-extracted and reverified.
Final member order, hashes, mirror bytes and staging whitespace checks pass.

No explicit Cargo job count was supplied. The focused failures are evidence,
not passing acceptance for the proposed replacement.

Not run in this documentation cut: workspace test/check, Clippy, doctests,
structure audit, full parent matrices, and runtime/codec/snapshot execution for
the proposed binder model. The proposed Rust declarations have not been
compiled. Prior passing workspace and runtime results remain in the preceding
note; they do not validate this new design.

## Retained package and intake

- [Retained ZIP](../reviews/packages/zips/arcweft-aw-ah-009.4.2.1.1-recursive-generic-call-constraint-scope-reconciliation-final-contract.zip)
- Bytes: 45039
- SHA-256:
  6c6d2f0885a7f9e76e815e09be397556bac99b3097d290255eee66f75ad32185
- Searchable mirror:
  docs/reviews/packages/arcweft-aw-ah-009.4.2.1.1-recursive-generic-call-constraint-scope-reconciliation-final-contract/
- Classification: locally authored design, ready for implementation.
- Request copy, source hashes, command evidence, manifest, member hashes,
  readiness and open-question sidecars are all inside the ZIP.
- OPEN_QUESTIONS.md contains exactly none.

The source mirror is frozen to those ZIP bytes. The ZIP hash above is its
byte authority. The existing parent design and request remain inherited
untracked inputs and were not silently accepted by committing this child.

## Design deviations and scope

The parent assumption that a remaining function type must have only
monomorphic descendants is amended: a function scheme closed by its own
binder is a closed runtime type. Unapplied generic function values also require
typed group-zero continuation validation instead of the unconditional
group-zero rejection. The ProjectCall operand/materialization/outcome layout,
same-fiber return protocol and continuation snapshot fields remain.

The selected inference boundary is predicative. A published quantified prefix
is instantiated through its checked call application, not implicitly cast to a
monomorphic callback. A complete expected type at the producing application
can close future parameters before freezing. No new forall syntax, general
let-polymorphism, higher-rank inference, type-application opcode, wrapper
FunctionSite or dynamic generic registry is selected.

Inspection also confirmed that native synchronous function-value application
rejects Executable FunctionSites, while AWBC ApplyFunction accepts the Function
carrier. That existing concrete-callback execution issue remains a parent
obligation. The scope design does not introduce a partial bridge through it.

## Remaining work and structural review

Implement the complete package through all affected sema/compiler/runtime/
codec consumers in one uncommitted replacement, then pass its matrices and
the parent gates. Do not commit the inherited Content/Fx/ProjectCall Rust WIP
on the strength of this design result.

The production structural seams are the large existing callable graph,
checked application, analyzer constraint adapter, compiler lowering,
runtime semantic facts and core type graph. The design assigns generic
behavior to the types-owned fold/generics module and instance graph behavior
to compiler/lower/project_instances. This is a planned decomposition, not a
claim that LOC or a structure gate improved in this cut.

First-class runtime Need values, Content grammar, line identity, host
scheduling and unrelated snapshot semantics remain explicit non-goals.
