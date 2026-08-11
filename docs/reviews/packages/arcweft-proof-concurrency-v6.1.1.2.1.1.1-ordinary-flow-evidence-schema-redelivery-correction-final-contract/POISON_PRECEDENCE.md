# Flow poison and diagnostic precedence

## Canonical primary issue

A committed recovered Flow may contain many typed issues. Exactly one becomes
the Flow-level primary issue. Selection uses this total order:

1. common item prefix;
2. identity;
3. signature;
4. contract clauses in heterogeneous source order;
5. missing required body;
6. body children in body source order;
7. unclosed body;
8. trailing recovery in source order.

Within one class:

1. smaller source start wins;
2. for equal starts, smaller source end wins;
3. for equal sites, smaller heterogeneous ordinal wins;
4. for equal ordinals, the owning issue enum discriminant wins;
5. a fixed component role precedes a nested recovery component of the same
   owner.

The rule is independent of arena slot order, hash-map iteration, diagnostic
insertion order, thread scheduling, or which consumer first asks for facts.

## Common prefix

Prefix issues include invalid/misplaced attributes, malformed visibility, and
documentation attachment failure. Their typed attached owners remain available.
A prefix issue does not turn a recognized Flow into a generic ErrorItem.

## Identity

Identity issues include missing identity, wrong family, invalid public ID,
reserved/invalid name, and ID/name mismatch. The item and source roles commit,
but project/callable publication is suppressed.

For ID/name mismatch, the name site is primary and the public-ID site is
related. Neither spelling is rewritten.

## Signature

Signature order is:

1. generic group and parameters;
2. first fixed parameter group, parameter order, pattern preorder;
3. rejected second group;
4. return arrow/type;
5. `where` predicates in source order;
6. signature trailing recovery.

A missing type is represented by its actual error `TypeId`; omitted return is
not an issue and creates no type.

## Contracts

Contract order is the parent `ContractClause(ordinal)`, regardless of family.
Inside one clause:

1. keyword/whole shape;
2. mode;
3. opening delimiter;
4. operands in ordinal order;
5. closing delimiter;
6. semantic resolution/checking.

The Flow-level issue is roleful. A poisoned child retains its own terminal
diagnostic; the Flow does not copy it.

Effect allowed/forbidden conflicts use the later selector as primary and the
earlier selector as related. Duplicate `decreases` uses the later keyword as
primary and the first keyword as related.

## Body

`MissingBody` precedes body children because there is no authored body.
For a present body, child issues are ordered by `ThreadFlowItem(ordinal)`.
Within a child, the child's accepted poison precedence applies.

An unclosed body is evaluated only after all retained children. This allows a
more specific malformed child to be primary while preserving the exact missing
close as related evidence.

Trailing recovery is last and never displaces an earlier typed error.

## Poison propagation

- A poisoned header or clause does not suppress the lowering of independent
  source-owned children needed for diagnostics.
- A limit, stale/foreign, cancellation, panic, or invariant failure is not
  committed poison; it aborts the transaction.
- Project publication, checked callable creation, runtime-plan emission, and
  executable verification require the relevant accepted poison gate. They do
  not repair or ignore poison by consulting source text.
- The result local remains allocated when an `Ensures` clause exists even if
  its authored return type is an error `TypeId`; its type carries poison.
- Omitted Unit is never poison.
