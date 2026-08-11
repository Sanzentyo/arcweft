# Negative, tamper, boundary, and API matrix

`TEST_MATRIX.csv` contains 438 normative rows:

| Kind | Rows |
|---|---:|
| positive | 109 |
| negative | 121 |
| tamper | 59 |
| boundary | 47 |
| golden | 30 |
| parity | 12 |
| structural | 8 |
| compile-fail | 34 |
| full gate | 18 |

No row is marked executed in this design archive. Implementation evidence must
record real command results and may not infer a pass from package validation.

## 1. Universal failure oracle

Unless a row explicitly concerns successful commit/activation, every negative,
tamper, boundary-overflow, and injected allocation failure must prove:

```text
active execution identity unchanged
active canonical value snapshot unchanged
slot revisions unchanged
live/moved/dropped states unchanged
domain and execution-local persistent cursors unchanged
no destination filled
no source taken
no host/provider/scheduler request emitted
no partial save bytes published
all reservations installed by the failed operation cleared
exact input/transaction/candidate owner returned
```

Transient allocation capacity and cleared reservations are not part of
canonical state.

## 2. Prepare error owner matrix

| Error family | Required returned owner | Mutation before return |
|---|---|---|
| execution mismatch | `RuntimeOwnershipTransaction` | none |
| conflicting participant | transaction | none; repeated compatible CopySource is accepted |
| stale revision | transaction | none |
| slot reserved | transaction | none |
| source not live | transaction | none |
| destination not empty | transaction | none |
| type mismatch | transaction | none |
| invalid path/record identity | transaction | none |
| duplicate affine owner | transaction | none |
| affine Copy | transaction | none |
| revision/identity exhaustion | transaction | none |
| budget | transaction | none |
| allocation | transaction | none |
| reservation install mismatch | transaction | only caller reservations installed during prepare are cleared |

The input plan remains inside the returned transaction. Retrying requires a new
transaction ID after the caller corrects state/plan.

## 3. Commit mismatch owner matrix

Every commit mismatch returns:

```text
RuntimeTransferCommitError {
    kind,
    aborted: RuntimeAbortedOwnershipTransaction,
}
```

The aborted owner retains the plan and staged values/evidence, has no commit
method, and clears all matching reservations. Values/revisions are identical to
commit entry.

Required mismatch families are covered by `CMT-001` through the final CMT row in
`TEST_MATRIX.csv`: wrong execution, missing storage, identity mismatch,
reservation mismatch, revision mismatch, occupancy mismatch, type mismatch,
and affine-owner mismatch.

## 4. Restore tamper stages

The `SNP-*` rows cover all twelve restore stages.

| Stage | Representative tamper | Required result |
|---:|---|---|
| 1 | missing/duplicate/unknown section, trailing bytes | reject before typed scalar decode |
| 2 | zero ID, malformed decimal, unknown tag, duplicate JSON field | strict decode reject |
| 3 | program/bundle fingerprint mismatch | reject before execution structure |
| 4 | missing/extra active identity, wrong nested execution, bad epoch | reject |
| 5 | duplicate occurrence/owner-local ID, unresolved/reduced owner variant | reject |
| 6 | duplicate local/capture/record IDs, missing declaration, nominal arity | reject |
| 7 | zero revision, contradictory state, wrong tombstone evidence | reject |
| 8 | duplicate owner, wrong owner path, path depth/graph error | reject |
| 9 | stale/false-exhausted execution/local/transaction/owner cursor | reject |
| 10 | replay execution/mint/generation mismatch | reject |
| 11 | digest or canonical re-encode mismatch | reject |
| 12 | candidate allocation or persisted reservation | reject; no domain reservation |

For every case with an existing active execution, pre/post canonical active
snapshot bytes and digest must match exactly.

## 5. Boundary matrix

Exact and one-over rows exist for:

- execution, occurrence, local-slot, transaction, affine-owner, revision, and
  activation-epoch exhaustion;
- 4,096 participants;
- 4,096 steps;
- 1,048,576 value nodes;
- 64 path segments;
- 262,144 affine owners;
- 67,108,864 staged bytes;
- configured tightened limits;
- record field count/ordinal width; and
- strict decimal/fixed-width integer overflow.

`u64::MAX`/`u32::MAX` is accepted when valid; the *next* allocation or
one-over count fails. Saturation/wrap/reuse is never accepted.

## 6. Allocation fault matrix

`ALC-*` injects failure at every recoverable explicit allocation boundary:

- participant table;
- prepared transfers;
- traversal stack;
- path buffer;
- affine-owner evidence;
- checked duplicate;
- commit mutations; and
- commit evidence.

Every failure occurs before reservations or source take. After a permit is
created, an armed allocator fault must not be observed because commit allocates
nothing.

## 7. First-error matrix

Rows under `TXN`, `PTH`, `REC`, and `SNP` combine multiple simultaneous faults.

Preparation rank:

```text
identity/participant
stale/reserved
source occupancy
destination occupancy
type/path/record
duplicate owner
affine copy
exhaustion
budget
allocation
```

Commit rank:

```text
wrong execution
missing
identity
reservation
revision
occupancy
type
owner
```

Within one rank: slot, path, owner, step. Restore uses stage, then canonical
section/slot/path/ID/field/byte offset.

Tests must intentionally insert maps/vectors in alternate orders and run on at
least two supported target families to prove ordering is not iteration
accident.

## 8. Compile-fail/API matrix

`API-*` proves absence of:

- raw/default/from/parse constructors;
- identity/revision rebinding setters;
- fake execution/token/handle constructors;
- Clone/Serde for linear/live owners;
- arbitrary value parameters on Move/Drop commit;
- reduced owner variants;
- extension-trait owner behavior;
- core-to-HIR dependencies;
- live-value save serialization;
- floating snapshot `Eq`;
- display-string parsing;
- direct runnable session construction; and
- legacy/dual identity readers.

Use an API/compile-fail harness such as `trybuild` or repository-equivalent.
Do not implement these checks by scanning source text.

## 9. Full-gate rows

`FUL-*` are implementation-time gates. They must record actual:

- tested Git commit and Jujutsu change if available;
- exact command/toolchain/target/environment;
- start/end UTC;
- exit status;
- passed/failed/ignored counts where emitted;
- warnings and disposition;
- structure-audit output; and
- affected native/Web/headless/Agent/Tier-2 results.

This design package's own validator does not satisfy any `FUL-*` row.
