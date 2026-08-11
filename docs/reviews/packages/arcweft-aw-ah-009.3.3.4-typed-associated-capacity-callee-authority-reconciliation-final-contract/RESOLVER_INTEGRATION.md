# Resolver integration

## 1. One resolver entry

All static associated capacity calls use the existing entry:

```rust
pub(crate) fn resolve_call_target(
    request: CallResolverRequest<'_>,
) -> ResolveCallOutcome;
```

No static-capacity resolver, non-registered resolver, signature resolver, source-label resolver, or LSP resolver is added.

## 2. Request shape

```rust
pub(crate) struct CallResolverRequest<'a> {
    callee: CallCallee<'a>,
    authority: CallResolverAuthority<'a>,
    lexical: &'a LexicalCallableScope,
    expected: Option<&'a TypeKind>,
    traits: &'a TraitCatalog,
    trait_predicates: &'a [TraitPredicate],
    source: CallSourceContext<'a>,
    call_group: CallableGroupIndex,
    expression: TypeExpressionId,
    cancellation: &'a AtomicBool,
    work: &'a mut ResolverWork,
    signature_work: Option<&'a mut SignatureQueryWorkMeter>,
    signature_control: Option<&'a dyn SignatureQueryStepControl>,
    limits: &'a CallableLimits,
}
```

`CallResolverAuthority` owns the accepted/detached distinction:

```rust
pub(crate) enum CallResolverAuthority<'a> {
    Accepted {
        current_module: &'a CanonicalModulePath,
        symbols: &'a ProjectSymbolTable,
        world: &'a RegisteredSemanticWorld,
    },
    Detached {
        environment: &'a TypeCheckEnv,
    },
}
```

The enum implements its own validation and typed method lookup. No trait or free helper duplicates behavior for its variants.

Validation rules:

- `Accepted` retains world/revision/environment/source identity checks.
- `Detached` is accepted by this correction only with `CallCallee::AssociatedType` and detached/local source context.
- `Detached` cannot resolve project free calls or fabricate an accepted world.
- both authorities expose typed environment `Method` records using the same lookup key and deterministic ordering.
- both use the same trait catalog/predicates already supplied to the request.

## 3. Callee distinction

```rust
pub(crate) enum CallCallee<'a> {
    Free { /* unchanged */ },
    Selected {
        receiver_expression: TypeExpressionId,
        receiver_type: &'a TypeKind,
        method: &'a CallableName,
        arguments: &'a [CallArg],
    },
    AssociatedType {
        receiver: ResolvedAssociatedTypeReceiver<'a>,
        member: &'a CallableName,
        arguments: &'a [CallArg],
    },
    Dialogue { /* unchanged */ },
    FunctionValue { /* unchanged */ },
}
```

`Selected` always has an evaluated receiver expression. `AssociatedType` never has one. The resolver does not infer the distinction from the member name or receiver label.

## 4. Associated resolver algorithm

For `CallCallee::AssociatedType`, the resolver performs the following bounded sequence:

```text
validate request authority/source/cancellation/limits
charge one resolver invocation
lookup typed environment Method records
  if records exist: materialize deterministic viable candidates and return
resolve CapacityMethodId from exact TypeKind/member/authored arity
  if Some: materialize one CapacityMethod candidate and return
run existing associated/inherent/visible trait resolution
  inherent or unique: materialize one TraitMethod candidate and return
  ambiguous: return typed terminal ambiguity
  missing: return UnknownCallKind::AssociatedType
```

A cancellation/work check occurs before every lookup, candidate materialization, and trait step. Failure returns no partial candidate slice.

## 5. Typed environment precedence

Only `EnvironmentCallableKind::Method` records participate at associated step 1. The lookup key contains the exact resolved receiver and member. Standard remains before Adapter according to the accepted catalog authority/specificity ordering.

`UntypedMethodFallback` is excluded because it lacks a typed receiver identity. It cannot be promoted by this correction.

If one or more typed environment method records exist:

- normal viability/specificity/overload selection applies;
- an accepted method result stops capacity and trait lookup;
- typed environment ambiguity is terminal;
- argument work remains transactional and exactly once after selected replay.

## 6. Capacity candidate construction

The resolver calls the Arcweft-owned identity method:

```rust
let id = CapacityMethodId::resolve_associated(
    receiver.ty(),
    member,
    arguments.len(),
)?;
```

When `Some(id)`:

```rust
let callable = ResolvedCallable::try_new(
    CallableCandidateId::CapacityMethod(id.clone()),
    SignatureOrigin::Language {
        family: LanguageCallableFamily::CapacityMethod,
    },
    Arc::new(id.signature_schema()),
    CallableInstantiation::TypeReceiver {
        receiver: id.receiver().clone(),
    },
    Vec::new(),
    None,
    request.limits(),
)?;
```

Required facts:

- candidate: `CallableCandidateId::CapacityMethod(id.clone())`;
- family: `CallableFamily::CapacityMethod` / `LanguageCallableFamily::CapacityMethod` as already accepted by the existing IDs;
- origin: language capacity family;
- receiver in ID and instantiation: exact normalized type;
- method: exact `CallableName("with_capacity")`;
- arity: exact authored `CallArg` entry count;
- result: exact receiver clone;
- effects: existing empty/fixed capacity effect row;
- equivalent sources: empty;
- validator: `CallableValidator::Capacity(id.clone())`;
- schema: accepted `variadic_unchecked` behavior;
- no `BuiltinCallableId`, new family, placeholder type, or source label.

## 7. Capacity schema contract

`CapacityMethodId::signature_schema` is the sole schema owner. It uses the existing family-owned `variadic_unchecked` constructor with the receiver as the result.

Behavior:

- zero arguments are accepted;
- one or more positional arguments are accepted;
- named entries are accepted by the existing open-unchecked policy;
- spread entries are accepted by the existing unchecked spread policy;
- every retained value is checked once with `expected = None`;
- the schema itself does not reject a shape to manufacture a negative row;
- `CapacityMethodId::arity` remains exact identity evidence even though the schema is open;
- no parameter or result uses `TypeKind::Named("_")`.

This corrects baseline implementation drift without changing the accepted parent contract.

## 8. Associated trait step

If no typed environment or capacity candidate exists, the resolver calls the existing trait method resolution with the exact type receiver and member.

- inherent result -> one `TraitCallableId` candidate;
- unique visible result -> one trait candidate;
- ambiguous result -> `ResolveCallError::AmbiguousTraitMethod`, terminal;
- missing result -> unknown associated member.

No data-last fallback follows because a type receiver is not an evaluated value. The existing value-selected trait/data-last behavior remains unchanged.

## 9. Collision precedence table

| Competing authority | Required result |
|---|---|
| lexical/project/environment value vs dot type name | value receiver; associated resolution not entered |
| value ambiguity/error vs dot type name | value error; associated resolution not entered |
| explicit-generic `::member` vs same-name value | type-associated request; value lookup omitted |
| typed Standard method vs typed Adapter method | accepted Standard/Adapter deterministic authority and specificity rules |
| typed environment method vs capacity | environment method |
| typed environment method vs trait | environment method |
| capacity vs trait | capacity |
| unique trait vs data-last | trait; data-last ineligible |
| trait ambiguity vs data-last | terminal trait ambiguity |
| capacity/trait vs untyped method fallback | typed capacity/trait; fallback ineligible |
| near-miss capacity member vs unique trait | trait |
| near-miss capacity member with no trait | unknown associated member |
| value-selected environment/capacity/trait/data-last | unchanged AW-AH-009.3.3 selected ordering |

## 10. Checker integration

Checker call-target preparation performs classification once and constructs exactly one resolver request.

### Value outcome

It allocates/retains the real receiver `TypeExpressionId`, constructs `CallCallee::Selected`, and uses the existing selected transaction. It never retries as a type.

### Type outcome

It constructs `CallCallee::AssociatedType` using the nominal product projection. There is no receiver expression allocation. The existing `check_resolved_call` transaction:

1. registers the call expression once;
2. probes candidate schemas without publishing argument facts;
3. commits one selected candidate or one typed failure;
4. checks each argument expression exactly once during selected replay or recovery;
5. publishes one `CallTargetFacts` value;
6. records `CallableInstantiation::TypeReceiver` and exact capacity ID/result;
7. publishes no partial facts on cancellation or resource failure.

## 11. Signature-query integration

Native signature help does not resolve static capacity independently. It obtains the same call surface and typed receiver seed, invokes the same resolver/checker transaction under signature-query work controls, and projects `SemanticSignatureHelp` from checker-owned facts.

Required equality:

```text
checker primary candidate == signature primary candidate
checker origin == signature origin
checker result == signature result
checker parameter policy == signature parameter policy
checker poison/diagnostics == signature poison/diagnostics
```

LSP remains a transport/projection layer and never parses a callee or type label.

## 12. Registered/non-registered parity

For the same detached source and accepted-world source whose type/environment/trait inputs are semantically equal:

- classification produces the same normalized receiver;
- `CapacityMethodId` is byte-for-byte equal;
- candidate/family/origin/instantiation/schema/result are equal;
- argument facts and diagnostics are equal except accepted source spans carry document identity;
- work counts are equal for resolver and argument checks;
- old-dispatch count is zero in both modes.

No compatibility mode flag controls semantics.

## 13. Work accounting

For one successful associated capacity call with `n` authored argument entries:

| Counter | Exact value |
|---|---|
| call-expression registration | `1` |
| nominal receiver resolution | `1` |
| shared resolver invocation | `1` |
| typed environment lookup | `1` |
| capacity selector invocation | `1` when environment misses |
| capacity candidate materialization | `1` |
| old static dispatch invocation | `0` |
| candidate selected replay | `1` |
| argument expression checks | exactly `n`, one per authored/recovered entry |
| target fact publication | `1` |

For type failure before candidate construction, resolver/capacity counts follow the reached step and every retained argument is still checked once in recovery. Failed candidate probes do not add committed argument checks.

Signature query charges the same resolver steps through its bounded work meter. Cancellation or one-over work failure publishes no partial result and is non-cacheable.

## 14. Recovery accounting by failure

| Failure point | Candidate | Argument check count |
|---|---|---|
| value receiver exists but method missing | existing selected unknown | once per retained argument |
| receiver type missing/ambiguous/wrong arity | none | once per retained argument in typed recovery |
| typed environment ambiguity | ambiguous | once per retained argument |
| capacity miss + trait missing | unknown associated | once per retained argument |
| trait ambiguity | ambiguous | once per retained argument |
| cancellation before argument replay | none | zero committed checks; no partial facts |
| cancellation during probe | none | probe-local work discarded; no committed checks |
| recovered argument value | selected/unknown as applicable | one check for the recovered expression |

## 15. Structural constraints

- `CallResolverAuthority` behavior is implemented on the enum, not through an ad hoc trait.
- `CapacityMethodId` owns associated recognition and schema construction.
- `CallCallee::AssociatedType` owns the explicit request distinction.
- `CallableInstantiation::TypeReceiver` prevents accidental runtime receiver injection.
- source, label, canonical, or Rust display strings are never resolver inputs.
- the 23-family inventory is unchanged.
