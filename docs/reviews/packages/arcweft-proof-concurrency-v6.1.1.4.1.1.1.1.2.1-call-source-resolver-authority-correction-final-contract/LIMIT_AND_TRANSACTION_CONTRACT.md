# Limits, transaction, rollback, and retry

## Central limits

Final-HIR limits are variants of the original `HirLimit` owner:

| `HirLimit` | inclusive maximum |
|---|---:|
| `CallArguments` | 128 |
| `RichTextCallArguments` | 32 |
| `CallTypeArguments` | 128 |
| `CallDepth` | 32 |
| `TypeDepth` | 32 |
| `TypeWork` | 4,096 |
| `CallDiagnostics` | 128 |
| `SyntheticDescendantsPerRoot` | 1,024 |
| accepted arena totals/source bytes | unchanged |

Semantic limits remain in `CallableLimits`:

- candidates/call 256;
- parameters/callable 128;
- nested calls 32;
- recovery nodes 256;
- diagnostics 128;
- query work 4,096;
- source bytes 8 MiB.

No `CallLoweringLimits`, `ResolverLimits`, or second candidate ceiling exists.

## Reachable recovery ordinals

For a Call with at most 128 arguments:

- missing callee: `RecoveryOperand(0)`;
- missing value at argument ordinal `0..=127`:
  `RecoveryOperand(1..=128)`.

Therefore E12 exact producer tests use ordinal 128 and have no ordinal 129
because argument preflight rejects 129 arguments before child generation.
The general `RecoveryOperand` 1023/1024 constructor/generator test remains in the
tail/generator predecessor and is not repeated through Call.

## Atomic transaction

The transaction spans:

- attached-node acceptance for the target generation;
- root/child/type reservation;
- synthetic key generation;
- final source-index staging;
- root poison and diagnostics;
- candidate-neutral checks;
- shared resolver probe/replay state;
- complete `CallTargetFacts`;
- work reports;
- Proof witness projection;
- accepted project generation publication.

Any hard failure publishes none of these. A recoverable typed Call issue commits
the known Call payload, poison, source rows, and facts together.

## Retry

Retry under the same accepted source identity and unchanged project generation:

- reuses deterministic owner/role/ordinal synthetic identity;
- produces identical structural payload and canonical primary issue;
- produces identical candidate order and Proof witness order;
- does not retain rolled-back diagnostics/work/facts;
- diagnostic scheduling perturbation cannot change equality/hash/root poison.

A changed source revision or project generation is a distinct transaction and
must fail or allocate under the new qualified identity; it may never revive a
retired owner.
