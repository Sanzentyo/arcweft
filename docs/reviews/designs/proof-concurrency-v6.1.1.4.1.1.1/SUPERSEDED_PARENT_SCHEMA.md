# Superseded affected parent material

**NON-NORMATIVE PROVENANCE.** This file restates the complete affected predecessor material so an implementer does not compare archives manually. The normative replacement is in `RUST_SCHEMAS.md` and `ROLE_OWNER_ORDINAL_MATRIX.tsv`.

## 1. v6.1.1.4.1.1 incomplete schema

The retained package selected the correct final eight-owner shape and key/error records:

```rust
pub enum SyntheticOwner {
    Item(ItemId), Scope(ScopeId), Local(LocalId), Expr(ExprId),
    Stmt(StmtId), Type(TypeId), Pattern(PatternId), Capture(CaptureId),
}

pub struct SyntheticKey {
    owner: SyntheticOwner,
    role: SyntheticRole,
    ordinal: u32,
}

impl SyntheticRole {
    pub(crate) const fn accepts_owner(
        self,
        owner_kind: HirIdKind,
        ordinal: u32,
    ) -> bool;
}

pub enum SyntheticKeyError {
    WrongOwnerKind { role: SyntheticRole, actual: HirIdKind },
    InvalidOrdinal { role: SyntheticRole, ordinal: u32 },
}
```

It explicitly closed only `ElidedRegion = Type + 0`, said every other policy retained inherited behavior, and described fingerprint input only as a version tag plus owner kind/module/slot/role/ordinal. Those omissions are superseded.

## 2. Base role/anchor table requiring typed-owner translation

| role | previous owner wording | ordinal |
|---|---|---:|
| ImplicitUnitTail | proof/block syntax owner | 0 |
| PredicateBoolReturn | predicate item syntax node | 0 |
| ProofUnitReturn | proof item syntax node | 0 |
| ElidedRegion | reference TypeId | 0 |
| RecoveryOperand | poisoned parent expression/statement | child-role ordinal |
| PostconditionResult | ensures contract ScopeId | 0 |
| DesugaredTemporary | lowering owner | deterministic lowering ordinal |
| MissingRequiredTail | predicate/proof/block syntax owner | 0 |
| DestructuredBinding | source PatternId | preorder binding ordinal |
| ClosureEnvironment | closure ExprId | 0 |
| ClosureCapture | closure ExprId | first-use ordinal |
| ContractRequiresScope | callable ItemId | 0 |
| ContractEnsuresScope | callable ItemId | 0 |
| ForIterator | for StmtId | 0 |
| ForNextValue | for StmtId | 0 |
| IfLetScrutinee | if-let ExprId/StmtId | 0 |
| WhileLetScrutinee | while-let StmtId | 0 |
| MatchScrutinee | match ExprId/StmtId | 0 |
| PatternRest | rest PatternId | 0 |
| PostfixIndexCandidateExpression | source-backed postfix ExprId | root/preorder |
| DialogueContentCandidateExpression | source-backed postfix ExprId | root/preorder |

The normative correction replaces all ambiguous owner wording and every arbitrary-`u32` domain. The role names, semantic anchors, and source-order generators themselves remain intact.

## 3. Current implementation boundary

At audited main, the typed eight-variant `SyntheticOwner` plus `kind()` and `module()` are already present. The prior private raw `SyntheticKey` was deleted. This contract adds the final key directly with no raw conversion, dummy compatibility owner, or dual key.
