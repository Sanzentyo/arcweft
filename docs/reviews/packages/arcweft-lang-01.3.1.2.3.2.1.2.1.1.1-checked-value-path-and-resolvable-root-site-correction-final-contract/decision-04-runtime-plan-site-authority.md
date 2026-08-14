# Decision 04 — mechanically resolvable RuntimePlan site authority

## Raw schema replacement

`RuntimePlan` gains two mandatory leading fields and no defaults:

```rust
pub struct RuntimePlan {
    generation_contract: RuntimeGenerationContractDeclaration,
    type_declarations: Vec<RuntimePlanTypeDeclaration>,
    // current entries/callable_executables/flow_executables/flows/helpers/
    // trait_methods/line_task_groups/stream_plans/source_plans follow.
}
```

There is no `typed_root_uses` field. The retry's `RuntimePlanTypedRootUse` type is deleted. A site map is derived only after actual owner traversal succeeds.

`RuntimePlanTypeDeclaration` is a raw claim that must resolve to one existing project or producer fact in `AdmittedRuntimeGeneration`. It cannot create a root. For a project declaration, `RuntimeProjectRootId` must be the lossless byte projection of the declaration's `RuntimeSemanticTypeId`; for a producer declaration, producer identity, producer root, semantic ID, and exact checked type must match one admitted producer-root fact.

## Typed expression and pattern evidence

Every current `RuntimeExpr` field at a typed plan boundary is replaced by `RuntimeTypedExpr`; every current `RuntimePattern` field is replaced by `RuntimeTypedPattern`. `RUNTIME_PLAN_EXPR_RESOLUTION.csv` and `RUNTIME_PLAN_PATTERN_RESOLUTION.csv` define the exact root/child path grammar. Admission performs an exhaustive match over the actual enum and generates the required path set. It rejects:

- missing root `[0]`;
- any path not beginning with `0`;
- path length above 64;
- duplicate node or binding path;
- missing child row;
- extra row for an absent optional child;
- a row whose field ordinal does not belong to the actual enum variant;
- lexical binding, call signature, constructor, pattern, or result inconsistency.

The complete tables are sorted by `RuntimeIndexPath` only for canonical checking after uniqueness is established. Sorting is never used to repair duplicate or malformed input.

## Mandatory fields on current owners

The current owners that erase semantic coordinates receive these exact fields:

- `RuntimeNominalRole.ty: RuntimePlanTypeId`;
- `RuntimeFlowExecutableParameter.ty: RuntimePlanTypeId`;
- `RuntimeCommandContract.payload_ty: RuntimePlanTypeId`;
- `RuntimePureHelper.input_type_ids: Vec<RuntimePlanTypeId>` and `output_type_id: RuntimePlanTypeId`;
- `RuntimeTraitMethod.receiver_type_id`, `input_type_ids`, and `output_type_id`;
- `StreamPlan.item_type` and `error_type` replace `item_ty: String`/`error_ty: String`;
- `SourcePlan.item_type` and `error_type` replace `item_ty: String`/`error_ty: String`;
- `FlowOp::Bind` uses `RuntimeTypedBinding { name, value, ty }`;
- expression/pattern fields use the wrappers above.

All fields are private to their owner module with read-only accessors. Raw Serde uses a private DTO; constructors are `pub(crate)` to the final runtime-plan lowerer and core decoder. There is no public `RuntimePlanTypeId::new`, arbitrary declaration constructor, or post-deserialization mutation.

## Lowering source

The independent lowering source is the accepted `RuntimePlanSemanticFacts`, whose current owner retains the exact HIR snapshot bindings and normalized semantic types. `final_expr`, `final_pattern`, `final_flow`, accepted callable/trait signatures, and registered project/producer facts emit the final fields during one lower operation. Source/display names are not re-resolved during admission.

## Admission

`RuntimePlan::try_admit(self)` uses the exact API in `ADMISSION_AND_PAIR_API.md` and performs:

1. raw limits, duplicate table IDs, nested path shape, and nominal carrier syntax;
2. generation-contract equality with the already admitted generation;
3. type-declaration resolution against generation facts;
4. exact owner traversal and typed wrapper completeness;
5. lexical/scope/signature/operator/constructor checks;
6. derivation of the in-memory `BTreeMap<RuntimePlanTypedSite, RuntimeResolvedType>`;
7. project-root reachability and producer-domain checks;
8. publication of opaque `AdmittedRuntimePlan<'generation>`.

`RuntimeResolvedType` contains borrowed admitted declaration evidence and is non-Serde. A raw artifact that changes a type ID and its declaration together still fails unless the actual owner and all structural constraints independently resolve to that admitted type.

`RUNTIME_PLAN_SITE_RESOLUTION.csv` is the exhaustive current-owner map. Every current `FlowOp`, expression, pattern, stream/source operation, entry role, helper, method, line-plan string field, and deliberate exclusion has a row. No generic `slot: u32` remains.
