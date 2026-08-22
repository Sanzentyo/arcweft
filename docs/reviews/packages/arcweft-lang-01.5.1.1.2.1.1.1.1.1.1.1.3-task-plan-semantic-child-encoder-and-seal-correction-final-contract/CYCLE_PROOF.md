# Cycle and termination proof

## 1. Dependency graph

Let:

- `R` be the finite set of non-task executable rows;
- `F_i` be the producer-function digest for task row `i`;
- `Q_i` be its request-template digest;
- `C_i` be its control/effect-contract digest;
- `B_i` be its static core binding shape;
- `E` be the structured executable digest;
- `V_i` be the actual upper View binding payload when `B_i = View`;
- `P_i` be the completed `TaskPlanSemanticDigest`; and
- `K` be the final digest-to-index table.

The directed dependencies are:

```text
accepted runtime types/functions/flows/effects
  -> row digests R
  -> F_i, Q_i, C_i

R + source-order task base rows(i, F_i, family, class, Q_i, C_i, B_i)
  -> E

E + F_i + family + class + Q_i + C_i + B_i
  -> P_i                    (non-View)

E + F_i + family + class + Q_i + C_i + View marker + V_i
  -> P_i                    (View)

all P_i in source order
  -> expected-key comparison
  -> uniqueness map K
  -> public RuntimeTaskPlanTable / RuntimePlan
```

There is no edge from `P_i`, `K`, or an expected key back to `R`, `F_i`, `Q_i`,
`C_i`, a task base row, or `E`.

## 2. Why no task-plan cycle exists

The former invalid shape would have been:

```text
P_i -> E -> task-plan table key/self digest -> P_i
```

The final model deletes both possible return edges:

1. executable rows refer to task plans by source-order construction coordinate,
   never by completed digest key; and
2. `RuntimeTaskPlan` has no `semantic_digest` or `expected_digest` field.

Table 14 encodes only the static base. The final table is allocated after every
`P_i` is complete. Therefore the graph is a DAG.

## 3. View revision is not a hash edge

`AcceptedViewProgramRevision` participates in validation:

```text
current View resource stamp
  -> authority freshness check
  -> permission to use V_i
```

It does not participate in hashing:

```text
V_i = ViewProgramId + stable site + checked admission
```

Consequently revision replacement cannot create a cycle through the accepted
View program's semantic revision, and a revision-only change does not change
`P_i`.

## 4. Expected keys are assertions, not inputs

A decoded expected key is a private 32-byte assertion. The decoder computes
`P_i` without reading the assertion, then compares bytes. No typed digest is
constructed from expected bytes. A mismatch aborts the private image. Thus:

```text
P_i -> compare(P_i, expected_i)
```

and never `expected_i -> P_i`.

## 5. Finite termination

The candidate has bounded finite table counts. Every owner row visitor either:

- visits a finite source-order child list;
- follows a dense reference through a memoized `Unvisited/Visiting/Done` state;
  or
- treats an accepted nominal/opaque semantic identity as a leaf.

Each visit and emitted atom is charged to the exact work meter. Checked
arithmetic precedes every increment. Therefore the encoder terminates by one of
three outcomes:

1. all finite rows reach `Done` and all digests are produced;
2. a forbidden structural cycle is observed at `Visiting`; or
3. a count, byte, or semantic-work limit rejects.

No path can wait for a completed task-plan digest while computing `E`.

## 6. Proof obligations enforced by tests

- a test-only shadow task-plan key/self/expected value can be mutated without
  changing `E`;
- a task launch body changes only when its build coordinate or static semantics
  change;
- two complete runs over the same candidate produce byte-identical child, `E`,
  and `P_i` values;
- an intentionally cyclic private expression fixture rejects before hashing a
  task plan; and
- a maximum-size acyclic fixture completes, while the first over-limit atom
  rejects deterministically.
