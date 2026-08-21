# Final contract

## 1. Precedence and scope

This contract is Lang-01.1.1.3.2. It is based on repository commit `9138efeeabdfca56809e8ad9c16fc85380ae18c5`, which supersedes the request's observed `6e17c9fafe7c254b27e99f51af52ccc109a3a41d` for source evidence. It changes only runtime-emission admission and the diagnostic boundary at which project nominal schema/layout projection is attempted.

The following remain authoritative and are not redesigned:

- unary `Need<T>`;
- checked `Await` and `Try` facts;
- the shared generation-bound checked callable catalog and final callable resolution;
- accepted opaque producer identity and exact semantic identity for `ImageHandle`, `ArcError`, and every other registered opaque atom;
- `RuntimeCheckedType::Opaque` and its recursive composite acceptance rules;
- project nominal declaration identity, semantic identity, and defining field/variant order;
- `RuntimeNominalRecordLayout`, `RuntimeNominalRecordValue`, and `TypeLayoutHash` validation precedence;
- `RuntimeTypeSchema::try_layout_hash` as the sole closed-schema layout-hash authority;
- current AWBC/runtime/save schemas and every Arcweft-owned version marker at `1`.

## 2. Required-decision result table

| Required decision | Normative result |
|---|---|
| 1. Unreachable suspending ordinary function during `arcw check` | Fully parsed, lowered to final HIR, semantically checked, catalogued, indexed, and visible to tooling; excluded from runtime semantic fact publication unless selected by a runtime root or reached by an exact checked edge. |
| 2. Reachability owner | The existing HIR runtime-owner surface is replaced in place by one generation-bound `HirRuntimeSemanticReachability<'project>`. Roots and checked cross-owner edges are supplied once; HIR owns structural closure. |
| 3. Layout if unreachable function were included | Not applicable because it is excluded. No alternate opaque nominal layout is introduced. |
| 4. Transient versus persisted project nominal layout | They use the same schema-derived `TypeLayoutHash` contract. Therefore a project nominal containing any opaque leaf is not runtime-admissible when reached. There is no transient-to-persisted conversion surface. |
| 5. Pipeline representation | Full semantic state remains complete; runtime facts, RuntimePlan, AWBC, native execution, and save/replay contain only the selected closure. Tooling retains all declarations plus an emission disposition. |
| 6. Fixture 013 and direct suspension | Fixture 013 passes unchanged because its function is unreachable. If reached from a Flow or selected Entry, it fails with a typed unsupported-suspending-function diagnostic before nominal layout projection. |

## 3. Semantic acceptance is not runtime publication

Final semantic analysis remains whole-project and complete. The compiler must not use reachability to suppress syntax, name-resolution, type, effect, callable, `Await`, `Try`, ownership, or other semantic errors. Reachability begins only after final semantic analysis and checked Entry construction have succeeded.

For every ordinary function, `CheckedItemRole::Function { execution, suspension }` and the shared `CheckedCallableFacts` remain published regardless of runtime reachability. This preserves IDE queries, navigation, hover, reference search, static diagnostics, and future lowering inputs.

Runtime publication is a separate generation-bound admission step. Only owners in `HirRuntimeSemanticReachability` may contribute local, expression, statement, type, pattern, capture, call, Flow, callable, nominal-record, or variant facts to `RuntimePlanSemanticFacts`.

## 4. Sole generation-bound reachability owner

`HirRuntimeSemanticOwnerInventory` is deleted, not retained as an alias. Its replacement is `HirRuntimeSemanticReachability<'project>` in:

`crates/arcweft-lang-hir/src/final_project/runtime_semantic_owners.rs`

The owner embeds the exact `HirExecutableProjectView<'project>` and publishes:

- canonical runtime roots;
- canonical checked cross-owner edges;
- the deterministic first path to every reached executable owner;
- reached Item/Impl-method/closure owners;
- closed local/expression/statement/type/pattern/capture owner sets;
- the exact selected-expression type-owner subset;
- a generation/root/edge/closure digest used by runtime-fact admission.

No runtime-plan, compiler, Entry, tooling, AWBC, native, or save consumer may independently walk all declarations, infer roots, rebuild call edges, or subtract presentation owners.

## 5. Root modes

### 5.1 `CheckAll`

Used by `arcw check` and project-wide build validation when no exact Entry selection is supplied.

Roots are:

1. every item whose accepted role is `CheckedItemRole::Flow { .. }`; and
2. every checked Entry item accepted by the single checked Entry catalog.

Each Entry root follows its exact checked target and callable-role edges. A malformed Entry is rejected by Entry checking before reachability.

### 5.2 `SelectedEntry`

Used when `ProjectEntrySelection` is present and has passed `validate_entry_selection`.

The sole root is the exact selected checked Entry item. Its checked target and callable-role edges are followed transitively. Other Flows and Entries are not implicit roots.

There is no source-name root, public-visibility root, “main” spelling rule, or fallback to all functions.

## 6. Edge authority

The compiler projects edges only from already accepted typed products:

| Edge | Source authority | Target |
|---|---|---|
| Entry target/role | checked Entry catalog plus validated selection | exact Flow, ordinary function, impl method, or other executable HIR owner |
| Direct project call | `CallTargetFact::Selected` and its selected `ResolvedCallable` | exact checked project callable owner |
| Trait dispatch | accepted conformance and exact selected impl-member declaration | exact `ImplMethodDeclarationId` |
| Flow transfer | checked Flow/goto target fact | exact Flow item |
| Closure structural descent | final HIR expression/scope ownership | exact closure body and captures; no external edge row |

Builtin, registered standard, intrinsic, host, constructor, literal, and externally lowered callables terminate the project-executable edge; their checked runtime call fact remains in the reached source owner.

Ambiguous, rejected, missing, recovered-without-executable-target, or conflicting target facts cannot create edges. The semantic phase must already have rejected them; a forged admission input produces a typed reachability error.

## 7. Closure rules

HIR performs one deterministic breadth-first fixed point:

1. Validate every root and edge source/target against the embedded executable project generation.
2. Sort roots and edges by typed ID order and fixed discriminant order.
3. Enqueue roots.
4. When an executable owner is first reached, retain its signature type roots and executable scope/body roots.
5. Close scope children, locals, local annotations, expressions, statements, type children, patterns, owned scopes, and captures using final-HIR identities.
6. For postfix ambiguity, follow only the candidate selected by final semantic analysis.
7. For selected calls, retain arguments and either the accepted runtime receiver or no callee subtree according to `HirRuntimeCallCalleeDisposition`.
8. When a reached edge source is visited, enqueue the exact target executable owner.
9. Do not enter View/Style presentation products; an edge targeting one is an error.
10. Stop after the queue is empty and publish the atomic closure.

The first path is the lexicographically first shortest path under the canonical root/edge order. This path is stable across hash-map insertion order and is used in diagnostics and tests.

## 8. Missing-edge proof

After closure and before callable-emission classification, the compiler iterates every reached call/transfer/Entry source. If its accepted semantic target is a project executable, the reachability owner must expose one matching edge. Absence or mismatch is:

`RuntimeReachabilityProjectionError::MissingCheckedEdge { source, expected_target }`

This check prevents a producer bug from silently dropping a selected function. There is no fallback scan that adds the target later.

## 9. Reachable ordinary-function emission matrix

The compiler classifies every reached ordinary function before projecting any runtime type:

| Checked execution | Suspension | Effect row | Result |
|---|---|---|---|
| `DirectFrame` | `NonSuspending` | empty and accepted by current helper lowering | admitted as the existing project pure-helper path |
| `DirectFrame` | `NonSuspending` | non-empty or otherwise unsupported | typed `EffectfulDirectFrameUnsupported` error; never skipped |
| `DirectFrame` | `MaySuspend` | any | typed `SuspendingDirectFrameUnsupported` error |
| `StreamFactory { .. }` | either value | any | typed `StreamFactoryUnsupported` error |

The decisive request case is `DirectFrame + MaySuspend`. The diagnostic is emitted after generation/edge validation and before `runtime_type`, `project_nominal_schema`, `RuntimeSchemaProjection`, RuntimePlan construction, AWBC lowering, or native lowering.

No synthetic Flow, helper trampoline, fixture exception, or source-text rewrite is permitted.

## 10. Project nominal layout decision

Transient and persisted project nominals retain one contract:

```text
checked project nominal
  -> FinalSemanticAnalysis::project_nominal_schema
  -> canonical RuntimeTypeSchema
  -> RuntimeTypeSchema::try_layout_hash
  -> TypeLayoutHash
  -> RuntimeResolvedNominal / RuntimeNominalRecordLayout / RuntimeNominalRecordValue
```

Consequences:

- A closed project struct/enum remains admissible.
- A project struct/enum with an opaque leaf cannot produce a `RuntimeTypeSchema` and is rejected when reached.
- The error retains the exact nominal declaration and a typed field/variant/composite path to the opaque leaf, plus the accepted producer and semantic identities.
- The opaque atom itself is not “unresolved” and is not converted to a schema.
- Structural runtime composites such as `Option<ImageHandle>`, `Result<ImageHandle, ArcError>`, `Vec<ImageHandle>`, and tuples of opaque leaves continue to use `RuntimeCheckedType::Opaque` recursively where the existing runtime checked-type algebra permits them. This does not make them project nominal schemas or Entry data schemas.
- A project enum whose payload contains an opaque leaf is a project nominal and is rejected by the same schema rule.

No `RuntimeTransientLayoutHash`, plan-local layout substitute, dummy schema, producer schema copy, display-name hash, semantic-identity-as-layout, or alternate value variant is added.

## 11. Persistence and save/replay

There is no implicit transient/persisted project nominal conversion because there is no second project nominal layout family. All project nominal values already carry the same schema-derived `TypeLayoutHash`.

A reachable opaque-containing project nominal fails compilation before RuntimePlan/AWBC creation, so it cannot enter a frame, save, replay, or native value through any path.

Opaque values that are legal outside project nominals retain the existing producer-owned AWBC snapshot path. Save/replay must validate:

1. artifact/generation/ABI identity;
2. AWBC type-table row;
3. exact opaque producer;
4. exact semantic identity;
5. recursive payload snapshot validity;
6. enclosing checked type and frame slot.

A forged producer, semantic identity, nominal identity, layout hash, field count/order, or stale artifact is rejected before live publication. Unreachable functions have no emitted function/frame/register rows and therefore no save state.

All of `AWBC_ABI_VERSION`, `AWBC_CODEC_VERSION`, bundle-session save schema version, and every other Arcweft-owned version remain `1`.

## 12. Pipeline result

| Stage | Unreachable ordinary function | Reached supported helper | Reached suspending function | Reached opaque-containing project nominal |
|---|---|---|---|---|
| Final HIR | retained | retained | retained | retained |
| Final semantic analysis | fully checked | fully checked | fully checked | fully checked |
| Tooling/index | retained, marked `NotSelected` | `Reachable` | path available for diagnostic | path available for diagnostic |
| Runtime reachability | absent | present | present | present |
| Runtime semantic facts | absent | present | not constructed | type projection fails |
| RuntimePlan | absent | present | not constructed | not constructed |
| AWBC/native | absent | present | not constructed | not constructed |
| Save/replay | no frame/value | existing path | not applicable | not applicable |

## 13. Fixture 013

For `tests/fixtures/arcw/current_pass/check/013_task_fn_await_shape.arcw`:

- final semantic analysis keeps `load_bg(): Need<Result<ImageHandle, ArcError>>`;
- `try await load_bg()` keeps checked unary `Await` and `Try` facts;
- `ImageHandle` and `ArcError` retain accepted opaque identities;
- `load_opening_assets()` is classified as a suspending ordinary function;
- `flow main()` is a `CheckAll` root;
- no checked edge connects `main` to `load_opening_assets`;
- the function body, locals, return nominal, and `OpeningAssets` field layout do not enter runtime semantic projection;
- the fixture succeeds unchanged.

If `main` calls `load_opening_assets`, or a selected Entry binds to it, reachability includes the function and compilation returns `SuspendingDirectFrameUnsupported` before observing the `OpeningAssets.bg` layout problem.

## 14. Tooling contract

The semantic index/LSP retains every declaration and accepted checked fact. It additionally projects one non-authoritative display fact derived from the reachability owner:

- `Root`;
- `Reachable { first_path }`;
- `NotSelected`.

Tooling may display this state, but it may not rebuild roots/edges or feed it back into compilation. `NotSelected` is not a warning by default and does not excuse semantic errors.

## 15. Deletion closure

The implementation cut deletes in one compile-clean change:

- `HirRuntimeSemanticOwnerInventory` and `HirRuntimeSemanticOwnerInventoryError`;
- `HirExecutableProjectView::runtime_semantic_owner_inventory()`;
- every import/call/test helper using that zero-argument all-non-presentation inventory;
- unconditional Flow publication in compiler runtime-fact projection;
- any runtime-plan helper scan that treats all semantic declarations as candidate executable owners;
- the `continue`/skip behavior for a reached ordinary function whose execution/effects are unsupported;
- any broad fallback that repairs a missing checked edge by rescanning declarations;
- any generic string-only nominal-schema error for an opaque leaf where producer/semantic/path facts are available.

No compatibility alias or dual path remains.

## 16. Readiness

The owner, root modes, edges, closure, diagnostics, layout result, persistence result, pipeline representation, deletion cut, and tests are fully specified. `FINAL_STATUS=READY_FOR_IMPLEMENTATION`; `OPEN_QUESTIONS=0`.
