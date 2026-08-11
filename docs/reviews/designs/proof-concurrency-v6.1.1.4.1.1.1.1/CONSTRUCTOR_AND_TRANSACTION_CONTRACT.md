# Constructor and transaction contract

## 1. Structural constructor

`SyntheticRole::accepts_owner` is pure, const, allocation-free, and total for all
eight `HirIdKind` values and all `u32` values. `SyntheticKey::try_new` preserves
this precedence:

```text
wrong owner + wrong ordinal -> WrongOwnerKind
wrong owner + valid ordinal -> WrongOwnerKind
right owner + wrong ordinal -> InvalidOrdinal
right owner + valid ordinal -> Ok(SyntheticKey)
```

For both tail roles, `Expr` and `Scope` are right owners and only ordinal zero is
valid. The constructor does not inspect payload family; the lowering producer must
select the exact typed owner in `TAIL_PRODUCER_OWNER_MATRIX.tsv`.

## 2. Tail allocation transaction

For every tail row, the owning lowerer performs this order without variation:

1. reserve the source-backed container `ExprId`, or reserve the existing body/arm
   `ScopeId` required by the retained payload;
2. stage the owner's slot/source/scope metadata in the current transaction;
3. lower preceding statements, pattern, guard, and authored children in their
   retained order;
4. determine from the retained expected-result rule whether the omission is
   `ImplicitUnitTail` or `MissingRequiredTail`;
5. construct `SyntheticKey::try_new(owner, role, 0)`;
6. ask the same transaction for `(key, HirIdKind::Expr)`;
7. stage the child's zero-width source insertion and clean/poison state; and
8. fill the parent payload and commit only after all owner, child, source, limit,
   and diagnostic checks succeed.

A tail is never passed as its own owner. Scope-owned tails require no new body or
arm carrier. The child source row is queried as an Expr row; scope ownership is an
allocation-key fact only.

## 3. Canonical source-ordered generators

The six variable roles retain structural ordinal `0..=1_023` and use checked
`u32` conversion/increment.

- `RecoveryOperand`: the closed semantic child-role mapping owned by the original
  expression/statement family yields the ordinal. Optional absence yields no key.
  Diagnostic or child-vector position is not an input.
- `DesugaredTemporary`: source-causing tokens are sorted by attached source
  `(start, end, stable token role)`. Within one token, the owning desugaring recipe's
  immutable step sequence is used in declaration order. Map iteration is never an
  input.
- `DestructuredBinding`: depth-first authored preorder; tuple/sequence elements are
  left-to-right, record fields are authored order, and a whole binding precedes its
  nested pattern. The first accepted or-pattern alternative creates the ordinal map;
  later alternatives reuse it and cannot append a new binding position.
- `ClosureCapture`: the closure traversal consumes expression uses in attached
  source order. The first use of a distinct outer `LocalId` allocates the next
  ordinal; later uses return the existing `CaptureId`. Environment-map order is
  irrelevant.
- candidate roles: root Expr child is ordinal zero. The shared target is excluded.
  Additional children use deterministic preorder independently for each child
  `HirIdKind`; Expr begins at one because root occupies zero, other kinds begin at
  zero. Index and Dialogue interpretations use different roles and ledgers.

A conversion or increment failure occurs before key construction and before slot,
source, diagnostic, candidate, or counter staging.

## 4. Exact liveness resolution

A structurally valid key then enters the transaction's owner resolver. The current
exact errors are:

```rust
WrongModule { expected: HirModuleId, actual: HirModuleId }
NotYetLive { id: RawHirIdView, snapshot: HirSnapshotId, born: HirRevision }
Retired { id: RawHirIdView, snapshot: HirSnapshotId, retired_at: HirRevision }
KindMismatch { id: RawHirIdView, expected: HirIdKind, actual: HirIdKind }
```

Resolution order remains `WrongModule`, `NotYetLive`, `Retired`, then the private
`KindMismatch` invariant. An owner reserved in the same transaction is admitted
only after that reservation is present. `try_new` never performs these checks.

## 5. Reuse, limits, and rollback

The transaction ledger is keyed by `(SyntheticKey, child HirIdKind)`.

- same pair -> same typed child ID and one descendant charge;
- same key with a different child kind -> distinct child slot and charge;
- different body/arm scopes -> distinct exact-zero tail keys;
- retired/foreign/not-yet-live owner -> no child/source/count staging; and
- rolled-back owner or child -> no public ID or committed ledger entry.

`HirLimit::SyntheticDescendantsPerOwner.maximum()` remains 1,024 across all roles
and child kinds under the exact owner. Preflight uses checked `usize` arithmetic.
Exactly 1,024 fresh pairs commit. The 1,025th returns the existing typed limit error
with `observed: 1_025` and `maximum: 1_024` before publication; the enclosing
transaction rolls back the full prefix.

## 6. Candidate-only checks

Structural construction accepts either candidate role only for an Expr owner. The
postfix candidate transaction additionally verifies the source-backed postfix root,
interpretation role, root zero, shared-target exclusion, per-kind preorder, and
selected-key non-reuse. Candidate keys never become ordinary committed expression
keys. A discarded or failed interpretation rolls back every candidate ID and source
row.
