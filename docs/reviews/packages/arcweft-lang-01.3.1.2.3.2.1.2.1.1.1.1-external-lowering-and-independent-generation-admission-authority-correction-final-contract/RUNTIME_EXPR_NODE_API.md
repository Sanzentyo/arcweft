# Complete RuntimeExpr node-fact API

Owners:

- `arcweft_core::plan::typed_sites::RuntimePlanTypeDeclaration` and
  `RuntimeTypedExpr`;
- exhaustive traversal behavior on the original
  `arcweft_core::value::RuntimeExpr` inherent implementation;
- construction in `arcweft-runtime-plan/src/final_expr.rs`.

## Exhaustive node traversal on the owning enum

```rust
impl RuntimeExpr {
    pub fn try_visit_nodes<E>(
        &self,
        mut visitor: impl FnMut(&RuntimeIndexPath, &RuntimeExpr) -> Result<(), E>,
    ) -> Result<(), RuntimeExprNodeVisitError<E>>;
}
```

The method visits pre-order. The root path is exactly `[0]` (not the empty
path). Each actual child uses the ordinals in
`RUNTIME_EXPR_NODE_RESOLUTION.csv`; optional absence emits no visit. Ordinal
addition/multiplication is checked before path construction. Maximum path depth
is `64`. This behavior belongs on `RuntimeExpr`; no extension trait or free
recursive helper is introduced.

## Wrapper construction

```rust
impl RuntimeTypedExpr {
    pub fn try_new(
        expr: RuntimeExpr,
        nodes: impl IntoIterator<Item = RuntimeExprTypeFact>,
    ) -> Result<Self, RuntimeTypedExprConstructionError>;
}
```

`try_new` first enumerates the exact path set from the expression, then requires
one sorted fact for every enumerated path and no other fact. The first fact must
be `[0]`. Duplicate, missing, extra, noncanonical order, depth overflow, and
ordinal overflow are distinct errors. It does not resolve type IDs; the
`RuntimePlanBuilder` checks all IDs after type-table construction.

## Checked versus operational declaration

Every fact points to one `RuntimePlanTypeDeclaration`:

```rust
pub enum RuntimePlanTypeKind {
    Checked {
        checked_type: RuntimeCheckedType,
        authority: RuntimeTypeAuthorityDeclaration,
    },
    Operational(RuntimeOperationalType),
}
```

During admission, each declaration is matched by semantic identity against the
independent generation. Checked rows additionally match the exact checked type
and project/producer authority. Operational rows match the exact closed
root-shape tag. The accepted generation's semantic identity represents the
full normalized type, including nested members; the operational tag is only a
closed root-shape discriminator.

A checked declaration may back a `RuntimePlanTypedSite` and AWBC origin. An
operational declaration may back expression/pattern/internal signature facts
where required for type-correct lowering but cannot be used as a checked-value
context, nominal domain, AWBC typed origin, checked constant, frame register,
host payload, save payload, or VM boundary. Such use fails with
`OperationalTypeAtCheckedBoundary`.

## Lowerer output

`arcweft-runtime-plan::final_expr` receives the accepted semantic fact for every
node from the existing final HIR lowering context. It interns either:

- `RuntimePlanTypeDeclaration::try_checked` when semantic projection succeeds;
  or
- `RuntimePlanTypeDeclaration::try_operational` when the accepted semantic fact
  has one exact root shape in `RUNTIME_PLAN_TYPE_KIND.csv`.

The lowerer emits root `[0]` and every present child in one pre-order pass, then
calls `RuntimeTypedExpr::try_new`. Unsupported or unknown semantic shapes are a
lowering error; they are never silently untyped.

## Admission relation checks

After exact path/type-table resolution, plan admission checks the expression
relations in deterministic pre-order: operator operand/result rules, lexical
binding identity, callable signatures, branch equality, pattern scrutinee
relations, field/member identity, and accepted semantic type identities. The
root row is checked before children. An operational node may be an operand or
result only where the accepted semantic operation explicitly names that same
semantic identity. There is no dynamic fallback.
