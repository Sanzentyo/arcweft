# Host/runtime producer-family execution truth table

This table is exhaustive over the retained nine `NeedProducerFamily` values.
The authoritative implementation is an inherent match on that enum. Runtime
code never derives a family from a debug label, operation spelling or request
variant.

| Family | Allowed `TaskExecution` | Allowed policy | Exact restriction |
|---|---|---|---|
| `StructuredTaskPlan` | `Host` | `JoinSameKey`, `AlwaysStart` | The accepted structured RuntimeTaskPlan owns the host request. The scheduler validates family from the producer descriptor; request spelling and debug labels are non-authoritative. |
| `AwbcTaskPlan` | `Host` | `JoinSameKey`, `AlwaysStart` | The accepted AWBC task-plan row owns the host request and task-plan semantic digest. No RuntimeTaskRequest is admitted for this family. |
| `ViewMatchSubscription` | `Host` | `JoinSameKey` | A retained View Match subscription is reusable and generation-scoped. AlwaysStart is rejected because View subscriptions must share one Need cell per producer instance. |
| `AwaitManyBase` | `RuntimeAwaitManyAggregate` | `JoinSameKey` | The aggregate is scheduler-owned and is never prepared by a host adapter. Its child TaskSpec rows may independently select Host or Runtime execution. |
| `AwaitManyChild` | `Host`, `RuntimeAwaitManyAggregate`, `RuntimeTimeout` | `JoinSameKey`, `AlwaysStart` | Each child is a complete TaskSpec whose family is fixed to AwaitManyChild. Runtime execution is explicit in TaskExecution; it is not inferred from the child operation. |
| `Timeout` | `RuntimeTimeout` | `JoinSameKey` | Timeout is a reusable derived Need driven only by RuntimeStepInput.dt. It never reaches TaskLaunchAdapter. |
| `LineTask` | `Host` | `AlwaysStart` | Every accepted line activation launches distinct work and receives a journal-owned ordinal beginning at one. |
| `HostAdapterTask` | `Host` | `JoinSameKey`, `AlwaysStart` | The accepted host/adaptor contract is explicit in NeedProducerContractDigest. Runtime request variants are rejected. |
| `MakeNeedHandle` | `Host` | `JoinSameKey` | This family creates only reusable Join handles. It cannot fabricate an AlwaysStart pre-launch handle. |

## Normative validation API

```rust
impl NeedProducerFamily {
    pub fn validate_execution(
        self,
        execution: &TaskExecution,
        policy: TaskPolicy,
    ) -> Result<(), TaskExecutionPolicyError> {
        match (self, execution, policy) {
            // one exhaustive match corresponding to the table above
            // no wildcard success arm
        }
    }
}
```

`RuntimeAwaitManyAggregateRequest.children` contains complete `TaskSpec`
rows. Therefore a child with runtime execution is validated by the child's own
family row (`AwaitManyChild`) and then staged as a normal scheduler launch. The
aggregate has no hidden host adapter path.

## Negative cases

- `AwaitManyBase + Host` rejects.
- `Timeout + Host` rejects.
- `StructuredTaskPlan + Runtime` rejects.
- `ViewMatchSubscription + AlwaysStart` rejects.
- `LineTask + JoinSameKey` rejects.
- `MakeNeedHandle + AlwaysStart` rejects before identity or journal mutation.
- A Host request whose operation happens to be named `timeout` remains Host and cannot impersonate the `Timeout` family.
- A debug label containing `await_many` cannot select runtime execution.
