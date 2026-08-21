# Lang-01.1.1.3.2 suspended-function runtime emission and opaque nominal layout reconciliation

Date: 2026-08-21

Inspected Git commit: `6e17c9fafe7c254b27e99f51af52ccc109a3a41d`

Working-tree state: dirty only with the in-progress fixture-013 standard-callable
semantic cut in `arcweft-lang-sema` and this request/status documentation.

## Split reason

`tests/fixtures/arcw/current_pass/check/013_task_fn_await_shape.arcw` is now
accepted by final semantic analysis. Its standard `load_bg()` call has the
exact zero-argument signature
`Need<Result<ImageHandle, ArcError>>`, and both opaque atoms retain their
accepted nominal producer identities.

Compilation then fails while projecting the local `OpeningAssets` project
nominal, whose `bg` field is the accepted opaque `ImageHandle`. Two maintained
contracts meet at that boundary without one selected result:

- accepted opaque types project to `RuntimeCheckedType::Opaque` and publish no
  fabricated `RuntimeTypeSchema`, name hash, or layout; and
- project nominal runtime values require one exact nominal layout derived by
  the schema/layout owner.

The function is also an unreferenced suspending ordinary function. Current HIR
runtime-owner inventory includes every non-presentation owner, while the
runtime planner explicitly avoids letting unrelated effectful ordinary
declarations poison plan construction and the ordinary-function status record
classifies authored suspending-function AWBC lowering as externally designed.

This request must select that boundary before implementation continues. A
fixture-specific skip, a dummy schema, or a name-derived layout would violate
current repository invariants.

## Required decisions

1. Decide whether an accepted but unreachable suspending ordinary function is
   included in runtime semantic fact publication during `arcw check`.
2. If it is excluded, define the single generation-bound reachability owner,
   its roots, call/entry edges, closure rules, and the exact point where a
   reachable unsupported suspending function produces a typed diagnostic.
3. If it is included, define the single layout authority for a project nominal
   containing one or more `RuntimeCheckedType::Opaque` fields without adding an
   opaque `RuntimeTypeSchema`, hashing a display name, treating semantic
   identity as layout, or copying a producer schema.
4. State whether transient and persisted project nominals use the same layout
   contract. If they differ, define distinct typed owners and prove that a
   transient value cannot enter persistence through an implicit conversion.
5. Specify how the selected result is represented through final HIR, final
   semantic analysis, compiler runtime semantic projection, RuntimePlan, AWBC,
   verifier, native execution, save/replay, and tooling.
6. Reconcile the answer with the maintained positive fixture 013 and with
   direct ordinary-function suspension. Preserve all Arcweft-owned version
   markers at `1` as required by the current repository policy.

## Precedence and retained substrate

Preserve without redesign unless a concrete repository-evidenced flaw requires
it:

- unary `Need<T>` and checked `Await`/`Try` facts;
- accepted nominal producer identity for `ImageHandle` and `ArcError`;
- `RuntimeCheckedType::Opaque` and its producer/identity acceptance relation;
- exact project nominal declaration identity and defining field order;
- `RuntimeTypeSchema::try_layout_hash` as the current closed-schema layout
  authority; and
- the shared checked callable catalog and final callable resolution.

Later accepted repository policy supersedes returned packages that request
non-`1` ABI, codec, save, or schema versions.

## Prohibited answers

- no fixture-name or source-text gate;
- no `TypeKind::Named` runtime success fallback;
- no dummy `Bytes`, empty record, `Dynamic`, producer schema, or name-derived
  layout for an opaque field;
- no parallel runtime-owner inventories or fallback reachability readers;
- no silently dropping a function that is selected by an entry or reached by a
  checked call;
- no compatibility reader, version bump, `V2` type, or dual wire path; and
- no production code overlay in the returned design package.

## Consumer inventory

At minimum inspect and close:

- `crates/arcweft-lang-hir/src/final_project/runtime_semantic_owners.rs`;
- `crates/arcweft-lang-sema/src/final_analysis/nominal_schema.rs`;
- `crates/arcweft-lang-sema/src/final_analysis/model.rs`;
- `crates/arcweft-compiler/src/lower.rs`;
- `crates/arcweft-runtime-plan/src/semantic_facts.rs`;
- `crates/arcweft-runtime-plan/src/final_flow.rs`;
- `crates/arcweft-core/src/pattern.rs` and nominal-record value/layout owners;
- AWBC schema, lowering, verifier, VM, and native parity consumers;
- save/replay validation; and
- CLI `check` plus fixture gates.

## Required tests

The returned contract must define positive and negative coverage for:

- fixture 013 unchanged;
- the same function reached from a Flow or selected Entry;
- an unreachable suspending function with a primitive result;
- an unreachable and a reachable project nominal containing one opaque field;
- nested `Option`, `Result`, `Vec`, tuple, enum, and project nominal composites
  containing opaque leaves;
- persistence admission/rejection at the selected boundary;
- native/AWBC checked-type parity; and
- stale generation, missing reachability edge, forged producer, wrong semantic
  identity, wrong nominal layout, and deterministic ordering failures.

## Required output

Return exactly one design-only archive named:

`arcweft-lang-01.1.1.3.2-suspended-function-runtime-emission-and-opaque-nominal-layout-reconciliation-final-contract.zip`

It must contain `FINAL_STATUS=READY_FOR_IMPLEMENTATION`,
`OPEN_QUESTIONS=0`, a normative owner/API contract, reachability and layout
algorithms, Rust-shaped types, consumer/deletion inventory, implementation
order, complete test matrix, repository evidence, and internal manifest hashes.
