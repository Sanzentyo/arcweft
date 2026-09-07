# Production reconciliation

Inspected working copy: main at
69b30b530f5f1da39f5c8f0d7ee7f0f3de70bd29; 658 dirty/untracked status entries,
empty index. None of the inherited Rust migration is claimed as accepted main.

## Exact contradictions and selected replacements

| Current authority/evidence | Contradiction | Selected final replacement |
|---|---|---|
| types/nominal.rs declaration owner/ordinal IDs | A declaration coordinate cannot also distinguish a new recursive application | Preserve declaration IDs; add Free/Bound/Inference reference cases |
| callable/schema.rs checked generic inventory | Candidate templates still contain declaration references also used in caller facts | Seal candidate occurrences under a schema-owned binder; retain Free rigid references |
| callable/continuation.rs issue_constraint_initialization | Same T enters one map as Rigid and Bindable; current test fails before solving | Disjoint rigid Free inventory and opened inference slots under one token |
| types/constraints/solution.rs self-binding checks | T -> T is ambiguous between a real inference cycle and recursive T -> caller T | Occurs checks operate on inference IDs; normalized declaration-keyed RHS Free(T) is valid |
| callable/checked_application.rs frozen/deferred rows | Unbound future and caller references need distinct identities; first-use rows alone do not establish what remains unknown | One normalized substitution with an owned residual binder; derive future projections from it |
| callable/join.rs layers and instantiate_type_through | Repeated caller-layer substitution cannot itself distinguish callee keys from RHS caller references | Normalize callee RHSs under a closed caller substitution once; retain one closed substitution |
| compiler/lower.rs runtime_project_continuation_abi | A later-only generic B cannot enter runtime_type_under as a monomorphic function descendant | Admit a closed function scheme whose B is locally bound |
| compiler test probe | choose<A,B>(A)(B) stored once and called at String and i64 passes sema but fails runtime projection | One immutable prefix scheme, two fresh terminal substitutions and two closed instance keys |
| core/plan/project_call.rs group-zero continuation check | A checked generic bare function value has an empty Unapplied prefix, not a completed group | Validate Unapplied/AfterGroup with one prefix-position algebra; Direct remains unit |
| compiler/lower.rs recursive ensure/discover calls | Finite graphs are memoized, but growing specializations have no dedicated bounded transaction; layers retain traversal history | Deterministic worklist with flat closed rows, explicit inclusive limits and atomic publication |
| core/engine/eval/function.rs and awbc/vm.rs | General callback application is a separate existing execution boundary | Preserve its parent obligations; do not route quantified schemes through it by an implicit coercion |

The monomorphic-function assumption is contradicted by an actual compiler
probe, not by the spelling of a filename. Runtime schema closure is therefore
amended to include bound function parameters. No ordinary argument order,
attached-content ABI position, default evaluation order or call/return frame
protocol is replaced.

## Relationship to the authority amendment

Retain its sole preparation gate, types-owned structural relation, guarded
value rules, source projections, exact mapper evidence, affine callbacks,
Choice semantics, failure precedence and strict final seal.

Replace its use of a declaration ID as both a rigid scope key and a mutable
candidate key. Replace its declaration-keyed transitive completion with
normalized scoped rows. Replace current effect-only freshening with one
type/const/effect binder operation. The older amendment's rigid-only const
restriction is already superseded by current const inference; retain current
const inference and apply the same scoped rule to it.

The lower module still does not import callable schema/group/graph types.
The graph supplies a typed opening and exact seed; the lower completion owns
only generic scope and binding evidence; the higher frozen wrapper adds
callable coordinates. Current test-only lower validation hooks are not a
license to expose a production token constructor.

The older source/privacy scan clauses are superseded by repository policy:
typed behavior, compile-time privacy, codecs and dependency evidence are gates.

## Predicative boundary and avoided scope expansion

Function schemes are retained and instantiated at selected call applications.
This contract does not introduce implicit conversion of an already published
quantified prefix to a monomorphic callback, higher-rank or impredicative
inference, new closure generalization or a generic runtime dispatch registry.
A complete expected function type on the producing application can close
future parameters before freezing; this is distinct from mutating a shared
frozen prefix later.

Investigation considered a general callable-value type-application operation.
Current native executable callback and AWBC value-carrier behavior shows that
such an operation would also require a separate general callback execution
design. No such opcode, witness, source-binder extension or partial bridge is
part of this selected contract. The ordinary concrete callback issue remains
a parent implementation obligation, not a hidden success path here.

This is not a ban on the required recursive or future-generic programs.
Their source/semantic/runtime routes are specified in the acceptance matrix.
A generic closure sharing caller-rigid T has an empty new binder and remains
supported. A quantified prefix captured by that closure retains its exact
checked callable provenance and uses the ordinary selected application gate.

## Frozen bytes, versions and unchanged contracts

No retained predecessor archive is modified. This package is a new locally
authored contract; its byte authority is the paired ZIP. Stable language prose
records the selected scope/binder rule separately from implementation evidence.

All owned markers remain 1. Type graph records evolve in place. Continuation
snapshot fields and generation pinning remain. No alternate reader, legacy
domain, source reconstruction, runtime generic registry, generic FunctionSite,
second solver or intermediate public compatibility model is allowed.

No production baseline was repaired by this package. The recursion test and
later-generic probe remain failing implementation evidence.
