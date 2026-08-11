# Dynamic-dispatch disposition

## 1. Final decision

```text
PARENT_ROW_E017=SUPERSEDED_FOR_LANG_01_1_1
DYNAMIC_TRAIT_OBJECTS=FUTURE_LANGUAGE_FEATURE
REPLACEMENT_ROW=E017S_STATIC_WITNESS_EFFECT_DISPATCH
```

This is the second disposition explicitly permitted by the request. It is
fully closed and does not make the package `NOT_READY`.

## 2. Repository/design basis

The maintained trait design says dynamic trait objects are deferred. Current
syntax/HIR/sema has trait declarations, impls, predicates, concrete witnesses,
and static witness method resolution, but no source value type, object-safety
contract, erased layout, runtime witness table, dynamic call target, save/wire
shape, or tooling owner for a trait object.

Static witness dispatch is not dynamic object dispatch. Counting it as E017
would be false evidence.

## 3. Normative effect on parent E017

The parent's row:

```text
E017 trait object/dynamic method call -> call site uses trait signature row
```

is not an acceptance row for Lang-01.1.1. No production test may mark it passed,
failed, or implemented through static-witness evidence. Its status is
`SUPERSEDED_FOR_LANG_01_1_1` and its implementation gate is removed from this
sequence.

This disposition does not prohibit a future independently designed dynamic
trait-object feature.

## 4. Replacement row E017S

```text
ID=E017S
Area=effects/traits
Case=static witness method dispatch or static witness bound method value
Required result=the call/value carries the original trait requirement
                CheckedCallableId and its substituted exposed effect row;
                a concrete impl lookup is not required at the generic call site
Evidence=typed sema target facts, compiler lowering facts, signature/hover row,
         and effect propagation behavior
```

E017S rules:

1. Generic `T: Trait` resolution selects the original requirement ID.
2. Static witness/predicate evidence is typed and retained.
3. Type, associated-type, and effect substitutions are applied once.
4. The requirement exposed row is propagated at the final invocation.
5. A bound method value stores that same requirement ID, witness, and row.
6. No concrete impl row is substituted at the generic source call site.
7. Concrete monomorphization/runtime selection must satisfy the already checked
   conformance record.

## 5. Current grammar behavior

The implementation MUST NOT add:

- a `dyn` parser branch;
- a dynamic-trait-object AST/HIR/type variant;
- an erased string type;
- a placeholder witness/vtable ID;
- a non-executable HIR node;
- a runtime opcode or serialization field;
- a compatibility node; or
- a dedicated “dynamic trait objects removed/unsupported” diagnostic.

A source type spelling that the current ordinary type grammar cannot represent
continues to fail through ordinary type-syntax rejection. Tests assert that no
executable typed object/call target is produced; they do not source-scan the
repository and do not freeze a removed-syntax-specific code.

## 6. Future prerequisite contract

A future dynamic trait-object design must independently close all of the
following before implementation:

1. source syntax and exact spans;
2. object-safety/admissibility rules;
3. type/HIR value identity;
4. lifetime, ownership, borrow, and mutation semantics;
5. erased data layout and witness/vtable identity;
6. dynamic method-call selection and effect-row contract;
7. runtime-plan/AWBC execution and cancellation behavior;
8. save/replay/hot-reload and wire/version behavior;
9. visibility/import/reexport behavior;
10. compiler, formatter, hover, completion, navigation, and diagnostics; and
11. positive/negative/runtime/tooling tests.

Lang-01.1.1.3 defines none of those shapes. This avoids accidentally freezing a
partial public language surface.

## 7. Test evidence

- `parse_type_ref("dyn Effectful")` (or the current equivalent type parser)
  yields ordinary rejection and no typed dynamic-object node.
- A Rust compile-fail API test cannot construct a dynamic trait-object type or
  dynamic method target from public Arcweft APIs.
- E017S sema, compiler, method-value, substitution, hover/signature, and effect
  propagation tests pass.
- No E017 test is satisfied by E017S.
