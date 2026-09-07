# Consumer inventory

Paths below are relative to crates/. SOURCE_INPUTS.tsv records the inspected
bytes. Enumerations of remaining exhaustive consumers are implementation work,
not evidence that every named migration has already been performed.

| Owner | Final responsibility and required migration |
|---|---|
| arcweft-lang-hir/src/final_project/semantic_paths.rs | Retain exact accepted semantic owner path; no source-string or enclosing-name reconstruction |
| arcweft-lang-hir/src/final_project/type_roots.rs and runtime_semantic_owners.rs | Keep runtime/semantic/resolution-input partition and exact closure/default ownership |
| arcweft-lang-sema/src/types/nominal.rs and new types/generics.rs | Declaration IDs, scoped reference payloads, generic binder operations and checked construction |
| arcweft-lang-sema/src/types.rs | Single TypeKind Function binder and generic/array reference payloads; migrate constructors and projections |
| arcweft-lang-sema/src/types/generic_use.rs | Exhaustive scope-aware use collection; separate Free, Bound and active references |
| arcweft-lang-sema/src/types/constraints/shape.rs | One structural fold with binder entry/exit and metered descent |
| arcweft-lang-sema/src/types/constraints/context.rs | Exact free/open scope, eligibility, required inherited keys, existing work accounting |
| arcweft-lang-sema/src/types/constraints/transaction.rs | Branch-local inference bindings, pending equations, expected hints, probes and rollback |
| arcweft-lang-sema/src/types/constraints/normalization.rs | Inference-only occurs checks, transitive normalization and capture-avoiding reification |
| arcweft-lang-sema/src/types/constraints/solution.rs | Sole opaque normalized completion; remove raw completed solver-map reuse |
| arcweft-lang-sema/src/types/compatibility.rs and compatibility/binding_plan.rs | Same directional relation, exact Free equality, rigid bound-scheme comparison, strict policy |
| arcweft-lang-sema/src/effect_row.rs | Same effect-row algebra and normalization with typed bound/inference references |
| arcweft-lang-sema/src/callable/schema.rs and schema children | Schema template binder and exact declaration-slot/first-use authority |
| arcweft-lang-sema/src/callable/continuation.rs | Sole opening, ValuePreparation/ApplyGroup position, exact prepared/frozen inheritance |
| arcweft-lang-sema/src/callable/checked_application.rs | Frozen substitution aggregate, derived residual inventory, canonical stable bytes |
| arcweft-lang-sema/src/callable/join.rs | Template/body projections and single flat closed-instance substitution/digest |
| arcweft-lang-sema/src/final_analysis/analyzer/calls.rs | Accepted lexical generic projection and candidate preparation |
| arcweft-lang-sema/src/final_analysis/analyzer/calls/constraints.rs | Sole gate integration; preserve source order, source projections and callback ownership |
| arcweft-lang-sema/src/final_analysis/analyzer/expressions.rs | Parametric expectation intersection, cache/replay scope, callable-value provenance |
| arcweft-lang-sema/src/final_analysis/analyzer/call_seal.rs | Atomic stable reification and strict final projection; no active issuer may publish |
| arcweft-lang-sema/src/final_analysis/tests/generic_calls.rs and types/constraints/tests.rs | Behavioral scope, cycle, rigidity, source replay and inheritance evidence |
| arcweft-compiler/src/lower.rs and new lower/project_instances.rs | Scope-aware runtime types, flat instantiation closure, bounded graph discovery/materialization |
| arcweft-compiler/src/project.rs and source entry points | Checked projection-limit configuration, typed diagnostics and atomic result publication |
| arcweft-runtime-plan/src/semantic_facts.rs and semantic_facts/project_function.rs | Binder-aware normalized types; exact closed root/instance/continuation admission |
| arcweft-runtime-plan/src/final_flow.rs and final_expr.rs | Consume selected input scheme and closed target; no generic-body fallback |
| arcweft-core/src/plan/type_kind.rs and type_table.rs | One runtime type graph, scope-stack joins, closed-root admission and stable identity |
| arcweft-core/src/plan/project_call.rs | Unapplied/AfterGroup prefix validation; preserve source/ABI materialization and outcomes |
| arcweft-core/src/plan/construction/lower.rs | Admit every scoped type reference and selected instance through one aggregate builder |
| arcweft-core/src/value/project_continuation.rs | Exact scheme identity and prefix validation; immutable alias/extension semantics |
| arcweft-core/src/pattern.rs | Closed-root value matching; bound subnodes cannot act as wildcard runtime values |
| arcweft-core/src/awbc/schema.rs, codec and verify owners | Version-1 scoped type wire forms; exact verifier joins and bounds |
| arcweft-runtime-plan/src/awbc_lower and arcweft-core/src/awbc/vm.rs | Preserve original continuation ABI and selected closed function target |
| arcweft-core/src/awbc/fiber.rs and value/awbc_save.rs | Validate restored scheme roots/prefix values under the pinned generation; no solver state |
| arcweft-compiler/tests/project_function_instances.rs | End-to-end reusable future prefix, finite recursive keys, no nonterminal site |
| arcweft-core/src/engine/flow.rs and project-call tests | Native execution, return/unwind/suspension and restored continuation behavior |
| arcweft-core/src/engine/eval/function.rs and awbc/vm.rs ApplyFunction | Existing concrete callback execution obligations remain with parent; quantified schemes do not acquire an implicit path here |

Other exhaustive matches in formatter, LSP/signature/hover, typed diagnostics,
semantic transcripts, generic nominal projection, Agent, Content/Fx and codec
visitors must consume owning reference/binder APIs. Human-readable display can
use original parameter labels, but labels and HIR allocation order never
determine identity.

## Structural review

The active seam already crosses large cohesive owners: callable/continuation,
checked_application, analyzer/calls/constraints, compiler/lower, runtime-plan
semantic_facts and core type admission. Adding private generic behavior to
types/generics and moving compiler instance traversal to lower/project_instances
are the selected decomposition actions. Keep orchestration in callers and
domain behavior in the owning type/fold; do not create generic extension traits,
a bag of recursive match helpers, or an alternate type tree.

The dependency direction remains HIR/accepted identities into sema, compiler
projection into runtime-plan/core, then execution/tooling. New lower generic
code has no callable-module import, runtime scheduling, filesystem or network
dependency. Canonical structured Cargo-graph and structure evidence is required
after implementation; this design does not claim those gates were rerun.
