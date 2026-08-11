# Lang-01.3.1.2.2 — final contract

`OPEN_QUESTIONS=0`

## 1. Scope and selected policy

This contract corrects only the runtime argument projection for curried external
Stream callables. It preserves the shared callable resolver, declaration identity,
accepted-HIR lifecycle, query budgets, external binding path, ordinary
`fn (...) (...) -> Stream<T, E>` surface, Stream lifecycle, single-owner handle
model, and Sans-I/O boundary selected by the preceding cuts.

**Currying is supported.** This contract does not supersede the Lang-01.3.1.1
currying requirement. Every parameter group accepted by the shared resolver is a
runtime-visible part of the external callable signature.

The prohibited flat final-group projection is deleted. There is no compatibility
projection, alias, legacy reader, source-text recovery path, or resolver rerun.

## 2. Normative identity

A runtime parameter is identified only by:

```rust
RuntimeCallableParameterCoordinate {
    group: RuntimeCallableGroupIndex,
    parameter: RuntimeCallableParameterIndex,
}
```

The compiler constructs that coordinate by a checked, value-preserving conversion
from the accepted sema `CallableParameterCoordinate`. It does not derive a
coordinate from names, source order, source text, spans, provider metadata, or a
new search.

Coordinates are ordered lexicographically by `(group, parameter)`. Group and
parameter vectors are zero-based, contiguous, and match their stored indices.
Aliases and re-exports never change declaration identity or coordinates.

The callable declaration, static Stream definition, program generation, and
signature fingerprint accompany every partial value and every complete argument
product. A product belonging to another declaration, definition, generation, or
signature is foreign even when all visible names and types happen to match.

## 3. Signature and product representation

### 3.1 Signature

The checked signature is nested by group. Each parameter records:

- its exact coordinate;
- optional display/name metadata from the accepted schema;
- passing mode: positional-only, positional-or-named, named-only,
  positional-rest, or named-rest;
- presence: required, optional, or defaulted with a default-expression
  fingerprint;
- runtime type-layout fingerprint; and
- canonical rest element/value type where applicable.

The signature has 1 through 16 groups and at most 128 parameters in total. One
group may contain zero parameters. An empty group is still semantically applied.

### 3.2 Runtime argument product

Runtime values use a **canonical coordinate table plus a parallel value vector**,
not a nested value array:

```rust
RuntimeExternalStreamArgumentProduct {
    definition,
    declaration,
    generation,
    signature,
    completed_groups,
    coordinates,
    values,
}
```

`completed_groups` is a count in `0..=group_count`. It is required because an
empty group has no coordinate cell but must not disappear.

`coordinates.len() == values.len()`. The coordinate vector is strictly increasing
in canonical order. For every group less than `completed_groups`, exactly one cell
exists for every declared parameter coordinate, including optional, defaulted,
and rest parameters. No cell exists for a later group.

Value dispositions are closed:

1. `Explicit` — one checked authored value;
2. `Defaulted` — one checked value plus the exact default fingerprint;
3. `OmittedOptional` — no runtime payload;
4. `RestPositional` — one aggregate cell whose items preserve authored source
   order; and
5. `RestNamed` — one aggregate cell whose entries are unique and sorted by the
   canonical UTF-8 byte order of their names.

A rest parameter always has exactly one product cell, including when its aggregate
is empty. Product cell order is coordinate order, not authored argument order.
Authored evaluation order is retained separately by the executable application
plan.

## 4. Partial external Stream function values

The sole runtime owner is the existing function-value owner:

```rust
RuntimeFunctionValue::ExternalStreamPartial(
    RuntimeExternalStreamPartialFunction
)
```

The former closure struct becomes the `Closure` variant of that same enum. No
sidecar table maps ordinary functions to Stream partials.

An external partial stores the definition, declaration, owning generation,
signature fingerprint, next group, complete prefix product, and computed ownership
class. Its invariants are:

- `next_group == captured.completed_groups`;
- `next_group < signature.group_count`;
- the prefix product is exact for groups `0..next_group`;
- every captured value has already been evaluated and type-checked; and
- every affine token has one owner in the complete runtime state.

The initial callable value has `next_group = 0` and an empty product. After a
successful non-final group application, `next_group` increases by exactly one.
A caller cannot skip, repeat, or reorder groups.

A partial is unrestricted only when every capture is unrestricted. If any capture
is affine, the partial is affine. Unrestricted partials may be reused. Applying or
dropping an affine partial transfers or releases its captures according to the
existing affine-value rules; use-after-move and duplicate snapshot ownership are
rejected.

## 5. Non-final group application

Let `g` be the partial's `next_group`. An application is non-final when
`g + 1 < group_count`.

The runtime/compiler path performs these steps in order:

1. Validate static definition, declaration, signature, expected group, generation,
   coordinate shape, and operand/register types without mutating runtime state.
2. Evaluate authored argument expressions exactly once in source order. Their
   ordinary evaluation effects occur at this point.
3. Evaluate each required default that remains absent, in ascending parameter
   coordinate order. A default is evaluated once and records its default
   fingerprint.
4. Construct empty or populated rest aggregates once, preserving the rules in
   section 3.2.
5. Assemble a temporary group product in canonical coordinate order.
6. Join it with the existing prefix in a temporary candidate product and validate
   the complete prefix.
7. Validate the affine ownership-transfer batch.
8. Commit one new partial value and consume the old affine owner if applicable.

The result emits **no** `RuntimeStreamRequest::Open`, allocates no Stream instance,
and mutates no Stream lifecycle state. Failure before step 8 leaves the original
partial, registers, Stream tables, request batch, allocation cursor, and affine
ownership graph unchanged. Argument evaluation effects that already occurred are
not rolled back; therefore every structural check derivable before expression
evaluation must be performed first.

## 6. Final group application

An application is final when `g + 1 == group_count`.

The same group evaluation and temporary assembly rules apply. Before any Stream
mutation or host request, the runtime validates the full product:

- all groups are complete, including empty groups;
- every declared coordinate occurs exactly once;
- coordinates are strictly ordered;
- declaration, definition, generation, and signature match;
- every disposition is legal for its parameter;
- every default fingerprint matches the signature;
- every value and rest member has the expected runtime type layout;
- rest aggregates are canonical and within existing decode/runtime limits; and
- affine ownership transfers are unique and valid.

Only after full validation succeeds does one atomic commit:

1. consume the final affine partial/arguments as required;
2. allocate exactly one generation-owned `StreamInstanceId`;
3. insert exactly one `Opening` Stream instance state;
4. create exactly one typed Stream handle; and
5. append exactly one `RuntimeStreamRequest::Open` containing the full product.

These five changes commit together. Any failure produces none of them. The host
never receives a partial, malformed, or final-group-only request.

Equivalent direct and staged application are defined to produce equal checked
open requests after excluding the newly allocated instance ID. They share the same
canonical product and signature fingerprint.

## 7. RuntimePlan projection

The compiler consumes the already accepted shared-resolver result and emits:

- one `RuntimeExternalStreamCallableSignature` nested by group;
- one external Stream definition that references that signature;
- one initial external partial function value; and
- one `RuntimeExternalStreamGroupApplicationPlan` for each application expression.

Each application plan retains authored evaluation order and the resolver-provided
canonical coordinate/disposition for every slot. It also contains the accepted
default expression plan where the resolver selected a default. The compiler may
convert sema indices to core indices only through checked constructors.

RuntimePlan does not depend on sema in production. The compiler is the one-way
projection boundary. RuntimePlan, AWBC lowering, verifier, VM, structured runtime,
compiled regions, bundle, save, and hosts consume the projected types; none call
back into the resolver.

## 8. Effects and replay

Each group contributes the effects of the expressions actually evaluated during
that application, including selected defaults. Earlier-group effects are not
moved to final application.

The external Stream-open effect belongs only to successful final application.
Creating, cloning an unrestricted, moving an affine, serializing, restoring, or
dropping a partial emits no open effect.

A suspension after a committed group stores the partial value and exact execution
cursor. Resume continues after the committed application. Snapshot/restore
reconstructs that state and never reevaluates captured expressions, reevaluates
defaults, advances `completed_groups`, allocates an instance, or emits an open
request.

## 9. Rejection and atomicity

The following are typed failures, not traps inferred from strings:

- wrong or unknown definition;
- foreign callable declaration;
- stale or unavailable generation;
- signature fingerprint mismatch;
- skipped, repeated, retrograde, or out-of-range group;
- missing, duplicated, unknown, or out-of-order coordinate;
- coordinate/value length mismatch;
- illegal passing/presence disposition;
- missing or wrong default fingerprint;
- wrong value or rest-member type;
- duplicate or noncanonical named-rest entry;
- malformed positional-rest aggregate;
- product, nesting, byte, or work-budget exhaustion;
- affine duplicate, use-after-move, or unsnapshotable capture; and
- Stream instance allocation overflow.

Verifier, runtime, host decoder, save decoder, and restore validation reject their
respective malformed inputs before observable mutation.

## 10. Cross-boundary identity

The same definition, declaration digest, signature fingerprint, generation,
completed-group count, coordinate order, value dispositions, type evidence, and
value product cross:

- structured RuntimePlan execution;
- AWBC VM execution;
- compiled-region/FiberState exchange;
- native host requests;
- Web host requests;
- Agent host requests;
- bundle fingerprints;
- save/restore; and
- hot-reload generation pinning.

There is one semantic product. Hosts may encode it as strict JSON, while AWBC and
save use their canonical binary/serde forms; neither representation changes the
identity or order.

## 11. Versions and replacement policy

The integrated Stream cut is:

```text
AWBC_ABI_VERSION=2
AWBC_CODEC_VERSION=8
BUNDLE_SESSION_SAVE_SCHEMA_VERSION=2
```

Codec 8 has one reader and one writer. It does not read codec 7, Source tables,
Source opcodes, the old flat external Stream parameter vector, or the old closure
function-value save shape. Save schema 2 has one reader and writer and no schema-1
migration shim.

## 12. Explicit non-goals

This contract does not introduce or restore:

- `stream fn`, `source`, or a role attribute;
- a second callable resolver;
- flat final-group compatibility fields;
- source-text or source-span recovery;
- endpoint-specific DTOs or conversion helpers;
- hard-coded capability/function-name behavior;
- source gates;
- dual codecs, readers, writers, aliases, or migrations;
- I/O in core or data-format crates;
- CSS or Takumi paths; or
- a portable external-provider resume token.
