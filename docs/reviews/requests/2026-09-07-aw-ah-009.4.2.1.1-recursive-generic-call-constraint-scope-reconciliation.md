# Design request: AW-AH-009.4.2.1.1 recursive generic call constraint scope reconciliation

- Date: 2026-09-07
- Status: `OPEN_DESIGN_REQUEST`
- Parent: [AW-AH-009.4.2.1](2026-09-01-aw-ah-009.4.2.1-project-callable-attached-content-declaration-and-runtime-abi-correction.md)
- Assignment: design only; do not edit production code, tests, fixtures, Cargo
  manifests, branches, patches, or implementation overlays.
- Output language: English.

## Required outcome

Close the constructibility conflict between a declaration-owned generic type
parameter in the caller and an inference parameter of a new application of
that same declaration. Produce one complete constraint-scope, substitution,
seal, and closed-instance contract. It must support ordinary self-recursion
without allowing inference to specialize the caller's rigid generic type.

This is a child of the project-callable ABI request because that request
explicitly requires self/mutual recursion, generic instantiation, attached
defaults, and one ordinary executable function authority. This correction
does not reopen the selected ProjectCall ABI. Its boundary is the typed
generic application that must exist before that ABI can be constructed.

Design caller scope, candidate scope, inferred solutions, curried continuation
inheritance, and recursive instance admission together. Splitting them would
leave different consumers interpreting the same parameter ID differently.

## Repository evidence and precedence

- Checkout: `D:/git/arcweft`, branch `main`.
- Inspected accepted Git commit:
  `3ca699521d49fc54c6e4e0163f53c1097955bb10`.
- The implementation checkout is dirty and contains an inherited Content/Fx
  and ProjectCall migration. Its uncommitted changes are not accepted mainline
  evidence. Read the current checkout and record its full Git SHA, dirty/index
  state, and the exact files used; do not infer those facts from this note.
- Dispatch must provide that existing checkout, including its dirty Rust
  sources and the untracked parent request and AW-AH-009.4.2.1 design named
  below. A main-only clone does not contain those inputs. If they are absent,
  identify the missing source at preflight and do not declare readiness from
  the reproducer alone.
- [Continuation evidence](../../implementation/2026-09-07-content-callable-continuation.md)
  distinguishes accepted commits from working-copy results.
- Read root, workspace, documentation, implementation, and review `AGENTS.md`,
  the applicable Rust skill, and `docs/README.md` before work.
- Maintained [functions and pipeline](../../01-language/functions-and-pipeline.md)
  and [converged language surface](../../01-language/converged-language-surface.md)
  outrank historical surface spellings.
- Read the complete parent request and its AW-AH-009.4.2 parent, the current
  [project-callable contract](../designs/aw-ah-009.4.2.1-project-callable-attached-content-declaration-runtime-abi-final-resolution/FINAL_CONTRACT.md),
  its Rust shapes, acceptance/validator matrices, consumer inventory, and
  implementation order.
- Reconcile the [call-application authority amendment](../designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.2.1-final-semantic-owner-construction-and-seal-correction/CALL_APPLICATION_AUTHORITY_AMENDMENT.md),
  especially its exact rigid/bindable parameter scope, sole preparation gate,
  affine solution ownership, and strict final validation.

Accepted and validated substrate must remain unless an exact current-source
contradiction is demonstrated. All Arcweft-owned contract versions remain `1`.

## Reproducer and observed contradiction

```arcw
fn repeat<T>(value: T, count: i64) -> T {
    if count == 0i64 { value } else { repeat(value, count - 1i64) }
}
```

The source is accepted as executable HIR. In the current working copy,
`final_analysis::tests::generic_calls::recursive_generic_call_keeps_its_enclosing_type_rigid`
fails with `CallConstraintFailure` containing
`Invariant(Constraint(MalformedSchemaInventory))`.

The exact failure is:

1. HIR semantic topology identifies `repeat` as the enclosing declaration.
2. Its accepted callable signature owns one `GenericTypeParameterId` for `T`.
3. The enclosing scope admits that ID as `Rigid`.
4. The recursive candidate's schema admits the same declaration-owned ID as
   `Candidate`, which the current group classifies as `Bindable`.
5. `PreparedCallGraph::validate_and_issue_constraint_initialization` builds one
   map keyed only by `GenericTypeParameterId`; it cannot retain both roles.
   The construction fails before constraint solving or runtime lowering.

Before the working-copy correction, the analyzer passed an empty enclosing
scope for every call. That also rejects this simpler, nonrecursive source:

```arcw
fn wrap<T>(value: T) -> Result<T, String> {
    Ok(value)
}
```

Its failure was `TypeParameterOutOfScope` for `wrap`'s `T`. Supplying the
accepted enclosing inventory fixes that failure; it does not solve the
recursive role collision. Equal rigid references shared by a closure schema
and its enclosing scope can be coalesced. A rigid/bindable collision cannot
be treated as that same case.

Do not resolve this by passing an empty scope again, dropping enclosing IDs,
preferring one eligibility by map insertion order, treating generics as
wildcards, allowing self-bindings without a defined occurs check, inventing
source names, or adding a recursive-call exception. Each would leave the
general application boundary unsound or incomplete.

## Exact decisions to close

1. Define the final identities and ownership of declaration parameters,
   caller-rigid occurrences, and inference variables for an application.
   State which coordinates are ephemeral and which may enter a sealed
   solution, semantic digest, or closed-instance key.
2. Specify construction and validation through the existing sole call
   preparation gate. Include same-declaration recursion, mutual recursion,
   nested candidates, closures, methods/receivers, and calls in attached
   defaults. A borrowed HIR topology/accepted signature must remain the source
   of lexical ownership; do not introduce a parallel scope registry.
3. Define substitution, normalization, occurs checks, exact rigid comparison,
   structural type descent, const parameters, and effect-row interaction.
   Distinguish solving a callee parameter to the caller's rigid `T` from
   rebinding that caller's `T` to another type.
4. Specify all None/Prepared/Frozen states, explicit arguments, expected
   results, source callback branches, rollback, and curried inherited/future
   parameters. Preserve source operand order and distinct ABI destinations.
5. Define final solution projection back to declaration-owned parameters.
   Explain how canonical digests exclude allocation order, source spelling,
   transient solver IDs, and representative call-site selection.
6. Reconcile self/mutual recursive closed-instance discovery and memoization
   with `(RuntimeCallableId, CallableInstantiationDigest, CallableGroupIndex)`.
   Decide finite polymorphic recursion and unbounded specialization handling
   through one typed, deterministic work/limit contract. Do not silently ban
   ordinary self-recursion to avoid the collision.
7. Close failure classifications and diagnostics: malformed/stale/foreign
   scope evidence is an invariant failure; legitimate type disagreement is a
   candidate rejection. No partial checked catalog may publish after failure.
8. Give exact owning Rust modules, visibility, exhaustive shapes, invariants,
   producer/consumer migration, and the obsolete state to delete. Do not leave
   the implementation to choose between alternative authority models.

## Current consumer inventory

Re-audit these real producers and consumers, adding any affected owner found
through typed APIs and the Cargo graph:

| Owner | Responsibility that must remain coherent |
|---|---|
| `arcweft-lang-hir/src/final_project/semantic_paths.rs` and callable symbols | accepted semantic-path root and declaration identity |
| `arcweft-lang-sema/src/types/generic_use.rs` | exhaustive generic occurrence collection |
| `arcweft-lang-sema/src/types/constraints/context.rs` | parameter scope, eligibility, exact required keys, work accounting |
| `arcweft-lang-sema/src/types/constraints/{shape,normalization,solution,transaction}.rs` | constraint planning, normalization, solutions, branch transactions |
| `arcweft-lang-sema/src/callable/schema.rs` and its children | issuer-bound schema inventory and first-use ownership |
| `arcweft-lang-sema/src/callable/continuation.rs` | sole graph-issued initialization and prepared continuation ownership |
| `arcweft-lang-sema/src/callable/checked_application.rs` | frozen solution, checked application, digest, final ABI projection |
| `arcweft-lang-sema/src/final_analysis/analyzer/calls.rs` and `calls/constraints.rs` | lexical enclosing scope and the single preparation/solver integration |
| `arcweft-lang-sema/src/final_analysis/analyzer/call_seal.rs` | atomic checked-call seal and exact consumer projection |
| `arcweft-compiler/src/lower.rs` | closed function/closure/default instance discovery and substitution |
| `arcweft-runtime-plan/src/semantic_facts/project_function.rs` | exact closed executable fact partition and validation |
| `arcweft-runtime-plan/src/final_flow.rs` and `arcweft-core/src/plan/project_call.rs` | ordinary ProjectCall/function frame consumption |

Paths in this table are relative to `crates/`; where a responsibility is split
into children, enumerate actual current files in the returned evidence.

## Required behavior matrix

- Generic `Ok(value)` and another unrelated generic constructor preserve the
  caller's exact parameter; incompatible concrete payloads reject.
- Ordinary self-recursion at the same type succeeds; mutual recursion and
  recursion from a nested closure/default preserve the correct lexical owner.
- Passing a caller-rigid `T` to a different concrete type rejects rather than
  binding `T`; wrong-owner generics and conflicting role evidence fail closed.
- Generic closure/function-value schemas sharing rigid references with the
  enclosing scope succeed without duplicate ownership.
- Explicit and inferred type arguments, nominal arguments, receiver types,
  nested Result/Option/Need/function types, and const/effect occurrences obey
  the same scope rule.
- Curried prefixes retain one exact inherited solution across later groups;
  sibling applications cannot mutate it.
- Source-equivalent projects with different HIR allocation history produce
  equal final solutions/digests and the same closed-instance inventory.
- Finite recursive instance graphs terminate deterministically; a growing
  specialization graph fails at its selected typed inclusive limit, publishes
  no partial catalog, and leaves a prior accepted generation intact.
- Existing generic Content/Fx, dialogue, closure, Agent, named/rest ABI, and
  suspending ProjectCall regressions retain their behavior.

Use semantic analysis, executable compiler/runtime behavior, codec/digest
evidence, structured dependency checks, and compile-time privacy checks where
appropriate. Source spelling or module placement must not become a test gate.

## Implementation order and non-goals

Select the complete final scope algebra first. Then replace the lower
constraint and graph construction authority, migrate analyzer and final seals,
migrate closed-instance consumers, delete obsolete state, and run focused plus
workspace/Clippy/structure/applicable runtime gates. These are stages within
one coherent cut, not permission to commit a temporary public model.

Do not redesign Content grammar, Fx production, the ordinary ProjectCall ABI,
line identity, retained View, task-plan sealing, host scheduling, or snapshot
wire formats. First-class runtime Need values are outside this correction.
Do not add compatibility readers, aliases, source reconstruction, callable
special cases, a second type solver, or a public raw scope/solution constructor.

## Returned artifact and readiness

Return exactly:

```text
arcweft-aw-ah-009.4.2.1.1-recursive-generic-call-constraint-scope-reconciliation-final-contract.zip
```

Include `README.md`, `FINAL_CONTRACT.md`, `RUST_SHAPES.md`,
`PRODUCTION_RECONCILIATION.md`, `CONSUMER_INVENTORY.md`, `VALIDATOR_MATRIX.md`,
`ACCEPTANCE_MATRIX.md`, `IMPLEMENTATION_ORDER.md`, `REPOSITORY_EVIDENCE.md`,
`OPEN_QUESTIONS.md`, `FINAL_STATUS.md`, and sorted manifest/member SHA-256
evidence. Keep sidecars inside the archive; do not include a manifest self-entry
or production patch.

Record actual repository preflight and validation outcomes, including known
failing regression tests. Do not claim a clean or validated production
baseline, manufacture final-package evidence, or overwrite user changes to
obtain one. Readiness refers to a decision-complete design: use
`READY_FOR_IMPLEMENTATION` only if every result-changing decision is closed
and `OPEN_QUESTIONS.md` contains exactly `none`. Otherwise identify the exact
unresolved authority and return no final-contract archive.
