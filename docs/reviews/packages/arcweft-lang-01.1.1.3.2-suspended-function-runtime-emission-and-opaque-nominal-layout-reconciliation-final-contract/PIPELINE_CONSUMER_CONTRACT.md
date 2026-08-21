# Pipeline and consumer contract

## 1. Compile order

The project compiler order becomes:

1. parse/lower all modules to final HIR;
2. build/validate project symbols and shared checked callable catalog;
3. run complete final semantic analysis;
4. run project semantic verification;
5. build the complete checked Entry catalog;
6. validate optional `ProjectEntrySelection`;
7. construct lightweight Entry roots/bindings without schema projection;
8. construct `HirRuntimeSemanticReachability`;
9. prove edge completeness;
10. preflight every reached ordinary function;
11. build tooling/index products, including display-only emission disposition;
12. project View/Style/dialogue products under their existing owners;
13. project runtime semantic facts using the accepted reachability closure;
14. perform full reachable Entry schema/runtime projection;
15. build and verify RuntimePlan;
16. lower AWBC/native products;
17. verify products and publish the compiled project atomically.

Steps 8-10 must precede every call to `runtime_type`, `runtime_nominal`, `project_nominal_schema`, or `RuntimeSchemaProjection::layout_hash` for the runtime product.

## 2. Final HIR

All declarations remain. The reachability owner is an immutable view over the exact accepted final-HIR generation. It owns structural traversal; downstream crates receive no raw “all owners” list.

View and Style continue to use their presentation products. A runtime edge to a presentation owner is invalid.

## 3. Final semantic analysis

All type/item/call/Await/Try/effect/capture facts remain complete. No semantic analysis is skipped due to reachability.

`CheckedFunctionExecution` and `CheckedSuspensionRole` are the sole function classification inputs. The shared checked callable catalog is the sole callable target/effect source.

## 4. Compiler runtime projection

Every current loop over `analysis.locals/types/expressions/statements/patterns/captures/items/calls` must filter through the reachability owner. Flow publication is no longer unconditional.

`runtime_nominal` remains schema-derived. It is called only after callable preflight and only for reached owners.

The compiler never catches an opaque nominal schema error and substitutes another runtime type.

## 5. Runtime semantic facts

`RuntimePlanSemanticFactInput` is initialized with the reachability identity. Push methods verify owner membership and generation. Atomic `RuntimePlanSemanticFacts` admission rejects:

- facts outside the closure;
- missing facts required by reached HIR;
- duplicate/conflicting facts;
- a reached checked project call whose target has no admitted executable fact;
- a Flow/Entry row outside the root mode.

No fallback to all project declarations is permitted.

## 6. RuntimePlan

The plan contains only reached Flows, reached supported project helpers, reached impl methods, their exact local/type/value facts, and reachable Entry rows.

The current helper reservation that skips effectful project functions is changed: for a reached unsupported function, plan construction receives no input because compiler preflight already failed; a defensive plan error remains if forged facts bypass preflight.

Plan ordering is derived from canonical reachability order, not source traversal side effects.

## 7. AWBC lowering

AWBC lowering consumes only RuntimePlan. It does not receive HIR/sema and cannot infer reachability.

- unreachable ordinary functions have no `AwbcFunction`, signature, frame layout, block, instruction, resume point, or callable executable row;
- reached pure direct helpers use the existing helper kind/path;
- reached authored suspension never reaches AWBC in this cut;
- no new `AwbcFunctionKind` is added;
- existing `MAY_SUSPEND` remains for already supported Flow/task machinery;
- type table rows are generated only from admitted plan types.

## 8. AWBC verifier and VM

The verifier ensures every emitted call target exists and every type reference/layout is internally valid. It does not require unreferenced source declarations to be present.

The VM executes only verified rows. There is no runtime fallback to resolve a missing source function by name.

## 9. Native execution

Native lowering/execution consumes the same admitted RuntimePlan or verified AWBC semantic projection as the compact VM. It must not reopen final HIR or build a second root graph.

Parity assertions compare reached callable IDs, recursive checked types, nominal layout hashes, opaque owners, and deterministic outcomes.

## 10. Entry consumers

`RuntimeEntryLoweringInput` is filtered by root mode:

- `CheckAll`: all checked Entries;
- `SelectedEntry`: only the selected Entry.

Entry target/callable bindings participate in reachability before any Entry data schema is projected. Thus a selected Entry that reaches a suspending function reports the suspension error first.

## 11. Save/replay

Session save sees only emitted AWBC state. Validation uses the active artifact and verified program. Unreachable declaration metadata is not serialized.

No save field/version changes are needed. Tests prove absence of unreachable frames and rejection of forged producer/semantic/layout data.

## 12. Tooling and CLI

The semantic index/LSP remains whole-project. It may display root/reachable/not-selected state from the compiler-owned report but cannot decide emission.

`arcw check` uses `CheckAll`. Fixture gates remain source-agnostic: fixture 013 is not special-cased; its graph simply has no edge to the function.

CLI diagnostics render typed reachability paths and nominal schema paths through source-index queries. Codes are stable and do not expose internal enum `Debug` formatting as the contract.
