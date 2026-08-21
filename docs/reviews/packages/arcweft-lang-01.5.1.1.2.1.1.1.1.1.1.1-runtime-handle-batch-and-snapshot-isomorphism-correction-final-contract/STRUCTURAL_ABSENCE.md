# Structural absence contract

These are not optional cleanup notes. Cut 5 is invalid until the forbidden structures are absent from production and from the final public schema.

| ID | Forbidden structure | Required absence proof |
|---|---|---|
| `A01` | compatibility reader | No decoder/restore path accepts pre-final version-1 shapes or translates old tags. |
| `A02` | String fallback | No semantic identity, Host operation, callable, nominal, case, field or RuntimeValue is reconstructed from display/source text. |
| `A03` | identity alias | No old/new NeedId, TaskId, RuntimeCallableId or Host operation alias table. |
| `A04` | dual carrier | Each accepted ownership row and each live runtime state has one exact carrier. |
| `A05` | second RuntimeValue digest grammar | The existing canonical RuntimeValue visitor is extended in its original exhaustive match. |
| `A06` | source-string reconstruction | Restore joins typed catalogs and fails when a join is absent. |
| `A07` | TaskEnsureError::AdapterCommit | Commit is infallible; no error variant or recovery branch exists. |
| `A08` | per-child aggregate commit | AwaitMany uses one EnsureBatchPlan and no child invokes the public committing ensure path. |
| `A09` | ephemeral observer allocation | Every generation snapshot owns next_observer_id. |
| `A10` | scheduler-to-host dependency | Scheduler Cargo/dependency closure excludes adapter implementations. |
| `A11` | lossy snapshot summary | No `{ kind, items }`, `{ source, cursor }`, opaque-byte, or callable/captures summary. |
| `A12` | IntOrUInt | Signed and unsigned carriers are distinct in live, snapshot and ownership matrices. |
| `A13` | sequence-before-TaskId | Every pending/replay/snapshot order key places TaskId before sequence. |
| `A14` | blanket active Host rejection | Restartable active Host rows are persistable and restorable. |
| `A15` | extension-trait enum mirror | New behavior for Arcweft-owned enums is inherent on the enum/impl owner. |

The package validator checks the machine contract and schema text for the blockers it can establish without a production checkout. The implementation CI must additionally scan the repository symbols/dependency graph and run the source-level inventory tests in `TEST_MATRIX.md`.
