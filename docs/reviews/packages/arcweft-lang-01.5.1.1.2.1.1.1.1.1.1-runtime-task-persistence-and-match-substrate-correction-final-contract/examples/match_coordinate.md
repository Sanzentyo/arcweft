# Example: stable Match coordinate and transcript

Source-level shape:

```text
match user.status {
    .Ready(profile) if profile.enabled => show(profile),
    .Pending(progress) => skeleton(progress),
    _ => fallback(),
}
```

Compiler-local lookup:

```rust
CheckedMatchRef {
    snapshot: current_module.snapshot_id(),
    expression: match_expr_id,
}
```

The two fields above are a lease into the exact `FinalSemanticAnalysis`. They
are **not** written to a bundle.

A stable coordinate for the first guard's `profile.enabled` value is formed
from:

```text
AcceptedDeclarationSemanticId(owner declaration)
CheckedExpressionChildRolePath([
  MatchArm { ordinal: 0 },
  Guard,
  FieldTarget,
])
```

The field selection itself commits the accepted record-field semantic identity.
It does not commit `ExprId`, field spelling, source span, or arena allocation.

The persistent bundle row stores only:

```text
program
accepted_revision
site = digest(ViewProgramId, AcceptedDeclarationSemanticId, child-role path)
checked_match = digest(semantic Match transcript)
view_admission
need_admission
ownership
producer_contract
payload_type
plan
arguments
resource_dependency?
```

Reallocating every HIR arena entry or moving the source span while preserving
the accepted declarations, child roles, checked types, literals, callables,
patterns, guards and bodies produces the same semantic digests. Changing the
accepted field identity, arm order, guard class, body callable, or pattern
constructor changes the relevant digest.
