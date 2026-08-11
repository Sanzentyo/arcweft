# Delta from Lang-01.3.1.2.1

This is a surgical supersession. Rows marked **replace** are no longer valid in
the returned 01.3.1.2.1 contract. Rows marked **preserve** remain authoritative.

| 01.3.1.2.1 area | Final 01.3.1.2.2 decision | Action |
| --- | --- | --- |
| Ordinary `fn -> Stream<T, E>` external callable surface | Currying remains supported through the shared callable semantics | preserve |
| Shared callable declaration identity | Compiler projects the accepted typed declaration to one runtime digest | preserve/clarify |
| Shared resolver and accepted call facts | Coordinates and slot dispositions are consumed directly; no lookup is repeated | preserve/clarify |
| External parameter metadata as one flat vector | Replace with group-aware signature tables whose parameters retain `(group, parameter)` | **replace** |
| Statement that only the final selected group is projected | Every group is projected and participates in the open product | **replace** |
| Runtime open arguments as a flat final-group vector | Replace with `completed_groups + coordinate vector + parallel value vector` | **replace** |
| Partial application behavior left to ordinary flat closure capture | Add `RuntimeFunctionValue::ExternalStreamPartial` as the sole typed owner | **replace** |
| Non-final external application | Evaluate/capture once, advance one group, emit no open request, allocate no instance | add |
| Final application | Join exact prefix/current group, validate complete product, atomically allocate/open once | replace/complete |
| Passing/presence metadata | Preserve positional/named/default/rest policy and carry it per coordinate | preserve/complete |
| Optional/default/rest runtime carriers | Closed argument disposition enum with one canonical cell per parameter | add |
| Empty parameter group | Preserve through explicit `completed_groups`; never infer group progress from cells | add |
| RuntimePlan lowering | One compiler projection from accepted sema facts into core-owned group plans | clarify |
| Public AWBC frame signature | Existing flat frame signature remains for ordinary function frames | preserve |
| Public external callable signature | Add group/signature/parameter metadata tables | add |
| AWBC partial application | Add opcode `0x27 ApplyExternalStreamGroup` | add |
| AWBC Stream open | Use opcode `0x28 OpenStream` with group-aware operand vectors | replace |
| AWBC runtime types | Add tags 21 `StreamHandle` and 22 `ExternalStreamCallable` in ABI 2 | integrate |
| AWBC callable constant | Add tag 18 `ExternalStreamCallable` | add |
| Source opcode/table compatibility | Codec 8 rejects removed Source bytes/tables; no dual reader | preserve |
| Runtime open request identity | Add declaration, signature, generation, completed groups, and coordinate product | replace/complete |
| Native/Web/Agent argument JSON | One strict shared representation; no host-specific flattening | replace/complete |
| Integer JSON policy | Structural `u16` values are JSON numbers; wide/runtime integers remain decimal strings | preserve/clarify |
| Earlier-group effects | Occur when that group is applied; defaults occur in their owning group | add |
| External open effect | Occurs only on successful final atomic commit | preserve/clarify |
| Suspension/restore | Persist exact partial/frame/cursor; never replay earlier evaluation or open early | complete |
| Function-value snapshot | Add external partial snapshot variant and captured product | add |
| Save schema | Include correction in the single schema-2 cut; no schema-1 migration | integrate |
| External-live save blocker | A partial is not live; final Opening/Open instance follows parent blocker | preserve/clarify |
| Signature fingerprint | Include group shape, coordinates, names, passing, presence/default, types, result, effects, provider ABI | replace/complete |
| Captured value fingerprint | Add generation-aware canonical argument fingerprint | add |
| Hot reload | Identical signature accepted; layout changes are generational; provider/adapter ABI changes follow restart rule | complete |
| Stream definition/instance/handle lifecycle | No redesign | preserve |
| Stream policy, event sequencing, queueing, close, terminal behavior | No redesign | preserve |
| Branch/match/`for await` selection and scheduling | No redesign | preserve |
| Core/data Sans-I/O dependency direction | No change | preserve |

## Deleted shape checklist

Before the integrated Stream cut is reviewable, all of the following must be gone:

- any external Stream definition field containing only the final group's
  parameters;
- any `Vec<RuntimePayload>` open argument field without coordinates;
- any lowering path that maps arguments by parameter names after sema acceptance;
- any application path that uses ordinary closure arity to identify an external
  Stream group;
- any host adapter that rebuilds a flat request;
- any save/bundle field that omits group progress or declaration/signature identity;
- any codec-8 reader branch for codec 7, Source layout, or the flat request; and
- any test that treats a final-group-only request as accepted.

Deletion is established by typed API replacement, codec behavior, dependency
metadata, and positive/negative tests. It is not established by a source-text gate.
