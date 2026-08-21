# Final contract

## 1. Scope, precedence, and fixed parent decisions

This correction is narrow. Current production and maintained documentation at `4bda1cdcea63fdf7aac32691d756c1c0e1fc693e` take precedence over stale source observations in the retained parent. The retained parent remains authoritative for unary `Need<T>`'s four states, ordinary nested `Result`/`Option`, producer and observer identities, lazy transactional start with `JoinSameKey`, producer-owned cancellation, static contamination, bounded persistence, transactional publication/replacement, and strict version-1 deletion.

The following are not reopened: timeout, Stream/Watch, Dialogue/RichText, line-plan, Choice, CSS, Takumi, or unrelated task outcome classification.

Normative words **MUST**, **MUST NOT**, **SHALL**, and **SHALL NOT** are binding.

## 2. D01–D03: the sole selector result ABI

A View Match selector SHALL have one parameter and one result: `AwbcSignature { params: [state_type], result: Some(result_type), effects }`. `state_type` is the exact verified four-case `NeedState<T>` projection owned by the retained parent. `result_type` SHALL reference one synthetic nominal `AwbcRuntimeType::Variant` containing one case per source arm in dense source order. A case ordinal SHALL equal the source arm ordinal. Every case SHALL have `payload: Some(tuple_type)`, including an empty `Tuple([])` for an arm with zero bindings. Tuple items SHALL be the arm's pattern locals in exact HIR `locals` order.

No other representation is admitted: not `Choice`, nested pair tuples, multiple results, retained frames, exported registers, globals, side channels, presentation values, or a View VM.

The selector function SHALL be `AwbcFunctionKind::Synthetic`, deterministic, non-suspending, effect-compatible with its ordinary guard calls, and SHALL return only the constructed variant. The selector type, state input, and function/result digests SHALL follow `WIRE_TYPE_DIGEST_ALLOCATION.md`.

Compiler projection SHALL convert `CheckedMatchRef` plus the one accepted final analysis into `RuntimeViewMatchSelectorSeed`. Runtime-plan construction SHALL finalize that seed into `RuntimeViewMatchSelector` by rewriting `RuntimeSemanticTypeId` references through the existing `RuntimePlan` type table. `arcweft-runtime-plan` SHALL NOT depend on `arcweft-lang-sema`, `arcweft-view`, or `arcweft-bundle`, and the selector builder SHALL NOT receive `CheckedMatch` directly.

## 3. D04–D06: dependency-safe join, private decode, and validation

`arcweft-view` SHALL remain independent of `arcweft-core`. Its rows may contain only `ViewMatchSiteId`, `ViewMatchArmOrdinal`, `ViewMatchBindingOutputOrdinal`, `ViewLocalRef`, `ViewInstructionRange`, and their containing coordinate rows. They SHALL NOT contain `AwbcRegisterId`, `AwbcTypeId`, `RuntimeValue`, an AWBC type table, or a core-owned value/digest table.

`arcweft-bundle` SHALL own `ViewReactiveBindingSectionV1`, the sole static join between View coordinates and AWBC function/type/task coordinates. The section SHALL include canonical source-map role rows and SHALL be covered by the bundle content root. `arcweft-runtime-driver` SHALL own `VerifiedViewReactiveBindings`, private `DecodedViewMatchSelection`, read-only decode, and `LocalInstallTransaction`. The prior public `arcweft-view::ViewMatchSelection` SHALL be deleted, not moved or aliased.

Selector decode and local installation SHALL validate site, checked-match digest, function signature, state/result type digests, nominal owner, case/arm equality, tuple presence, exact count, output ordinals, local coordinates, recursive value type, ownership admission, body range, active generation, and duplicate targets before mutation. Installation SHALL stage all values and perform one revision-checked commit; any error SHALL preserve all prior locals, observer state, arm state, and body selection.

The selector result SHALL survive normal callee-frame removal as an owning `RuntimeValue::Variant` containing an owning `RuntimeValue::Tuple`. No bundle/View/runtime-driver row SHALL retain a callee register coordinate.

## 4. D07–D10: typed Need producer ABI

The sole semantic/runtime projection for `Need<T>` SHALL be:

```text
TypeKind::Need(T)
  -> RuntimeTypeShape::Need(T)
  -> RuntimeCheckedType::Need(T)
  -> AwbcRuntimeType::NeedHandle { payload: T }
  -> RuntimeValue::NeedHandle(RuntimeNeedHandle)
```

These variants and mappings SHALL be added directly to the Arcweft-owned enum implementations. `RuntimeTypeShape::Need` SHALL no longer be classified as unsupported. No extension trait, ad hoc type map, opaque wrapper, String branch, or second endpoint table is authorized.

`AwbcFunctionFlags::NEED_PRODUCER` SHALL use bit `1 << 4`. `AwbcInstruction::MakeNeedHandle` SHALL use opcode `0x1e` and name exactly one `AwbcTaskPlanId`, one producer-local site ordinal, and source-ordered argument registers. It constructs a handle but does not start a task.

A verified producer function SHALL:

- be `Synthetic` and have `NEED_PRODUCER | DETERMINISTIC | MAY_ALLOCATE`;
- not have `MAY_SUSPEND` or contain `StartTask`, `Await`, host call, spawn, or dynamic target;
- have exactly one reachable `MakeNeedHandle` definition on every return path and return that value;
- return `NeedHandle<T>` where its task plan's `payload_type` is exactly the same `T`;
- pass arguments whose types and order exactly match the task-plan signature and host-argument metadata;
- bind to exactly one canonical `NeedProducerContractDigest`; and
- use `JoinSameKey` when admitted as a View unary-Need producer.

`AwbcTaskPlan.need_id: AwbcStringId` SHALL be deleted. `NeedId` SHALL be the fixed 32-byte BLAKE3 result over the verified producer contract digest and canonical source-ordered argument digest. The producer contract already commits to the function and `MakeNeedHandle.site`; no source spelling, mount, observer, table index, or arbitrary invocation token participates. Equal contract/arguments produce the same NeedId and join under `JoinSameKey`; a different contract or argument digest produces a different NeedId.

The handle carries `NeedId`, producer contract digest, payload type digest, and immutable arguments. The existing producer journal is the sole mutable endpoint/state authority. The core value SHALL NOT carry runtime-driver `GenerationId`. `extract_need_handle` SHALL validate the active generation and verified bundle binding, then create private `VerifiedNeedHandle { generation, ... }`. Observer publication and start-intent construction are forbidden before this step succeeds.

The final authority switch SHALL delete payloadless tag-19 decoding, NeedHandle-as-String admission, `await_target` string conversion, task-plan static need strings, untyped Need projection, obsolete bundle fields/readers, and tests/fixtures asserting those forms. TaskHandle's current String carrier is deliberately unchanged by this correction and remains separately typed by `AwbcRuntimeType::TaskHandle`.

## 5. D11–D14: constructible semantic and product schemas

Inferred expression, pattern, and local types SHALL remain normalized sema `TypeKind`. No declaration `TypeId` SHALL stand in for an inferred type. The existing owners remain singular:

- enclosing `CheckedExpression.ty()` owns Match result type;
- referenced scrutinee/guard/value `CheckedExpression` rows own their types and effects;
- referenced `CheckedPattern.ty()` owns pattern type; and
- referenced `CheckedBinding.ty()` owns each local type.

`CheckedMatch` SHALL NOT copy those types/effects. It SHALL retain the scrutinee `ExprId`, exact source-ordered arm coordinates, each local's output ordinal and ownership disposition, and one `CheckedMatchCoverage`. Its semantic digest SHALL read normalized type digests through the same `FinalSemanticAnalysis` at digest construction.

The exact arm identity is `CheckedMatchArmId { owner: ExprId, ordinal }`. Every arm fact SHALL retain current HIR `scope`, `pattern`, optional `guard`, `value`, and `locals` in source order. The nonexistent `arm_expression` coordinate SHALL not exist. Source spans remain owned by the HIR source index and are projected only into typed bundle source-map roles.

Every live final-HIR Match SHALL have `CheckedExpressionResolution::Match(Box<CheckedMatch>)`. A Match expression with `Structural`, a missing fact, duplicate fact, reordered arm, stale HIR owner, or mismatched child fact SHALL make final semantic publication fail. `CheckedViewCatalog` SHALL retain only `CheckedMatchRef`; it SHALL not copy arms, bindings, coverage, types, or effects.

Compiler's `RuntimeViewMatchSelectorSeed` is a one-way generation-bound codegen projection, not a second checked semantic authority: it retains only the exact IDs/type identities needed by runtime-plan lowering, commits to `CheckedMatchSemanticDigest`, and is constructed in the same atomic projection as the existing runtime semantic facts. Runtime-plan finalization uses one `RuntimePlan` type table; View/bundle do not copy it.

## 6. D15: resource registry input

`arcweft-lang-sema` SHALL depend on `arcweft-resource-model`. `FinalSemanticCatalogs::production` SHALL borrow both the accepted `RegisteredSemanticWorld` and immutable `ResourceTypeRegistry`, call `verify_integrity`, and retain its exact `ResourceTypeRegistryDigest`. The constructor becomes fallible and non-`const`.

The compiler SHALL pass its existing `context.resource_types()` registry to semantic analysis and View lowering. Before checked View catalog, runtime-plan selector seed, or product publication, it SHALL require exact equality among analysis, compiler context, compiled View product, and reactive section digests. A mismatch, stale registry input, or integrity error SHALL abort before publication.

## 7. D16: compile-clean implementation order

Implementation SHALL use the five cuts in `COMPILE_CLEAN_SEQUENCE.md`. A cut may introduce final owners that are not yet published, but SHALL NOT introduce an empty catalog, compatibility branch, alias, fallback resolver, second authority, unreachable dummy owner, or feature-version fork. Product construction remains explicitly fail-closed until the atomic final switch.

## 8. D17: guard execution closure

Current AWBC verification validates an optional Match guard as a function from the scrutinee type to `Bool`; current VM Match execution ignores that field. Therefore generated View selectors SHALL NOT use `AwbcTerminator::Match` or `AwbcMatchArm.guard`.

The selector SHALL lower each arm to this exact source-ordered control flow:

```text
TestPattern(state_parameter, arm.pattern) -> matched
Branch(matched, bind_block, next_arm)
bind_block: EnterScope; BindPattern(Declare)
if guard exists: evaluate ordinary guard expression once -> Bool; Branch(guard, select, exit_then_next)
select: MakeTuple(bindings); MakeVariant(case=arm ordinal); Return
exit_then_next: ExitScope; Jump(next_arm)
```

A failed pattern or false guard SHALL exit/clear the arm scope and continue to the next source arm. Guard failure is not no-match until all arms fail. Guard values and bindings use ordinary AWBC/RuntimeValue behavior; no View-owned evaluator or presentation fallback is introduced.

## 9. Persistence, replay, and replacement

Selector results SHALL use existing recursive Variant/Tuple save rows. Need handles SHALL use a dedicated strict version-1 save row. Restore SHALL recursively validate shape, limits, canonical IDs, type/contract/argument digests, static bundle join, resource digest, and active generation before commit.

Replay SHALL preserve journal order and publication cursors. Replacement may carry a producer only when producer contract digest, payload type digest, canonical argument digest, resource registry digest, and replacement policy all match. Otherwise the old producer is retired/cancelled transactionally and no observer or value is reconstructed from source.

## 10. Strict structural absence

Acceptance requires `STRUCTURAL_ABSENCE.md`: no AWBC register or core value in View schemas; no String branch for NeedHandle; no old View Await; no duplicate checked-Match arm owner; no `arm_expression`; no undefined normative type; no compatibility reader; no version other than 1; no dependency cycle; no selector use of the currently split AWBC Match guard field; and no runtime-plan dependency on sema/View/bundle.

## 11. Non-goals and forbidden alternatives

This contract does not authorize multi-result AWBC, retained frames, exported register files, a View VM, presentation value algebra, a core dependency from `arcweft-view`, source/string identity, a copied endpoint/type table, extension traits for Arcweft-owned types, ad hoc helper authorities, fallback resolution, compatibility decoding, dual publication, or any version bump.
