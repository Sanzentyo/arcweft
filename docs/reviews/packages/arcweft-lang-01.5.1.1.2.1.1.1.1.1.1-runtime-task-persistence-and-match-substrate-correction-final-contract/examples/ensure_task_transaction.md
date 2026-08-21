# Example: `ensure_task` ownership and rollback flow

This is Rust-shaped pseudocode for the final owner. It fixes borrow order; it is
not a production patch.

```rust
pub fn ensure_task(
    &mut self,
    spec: TaskSpec,
) -> Result<RuntimeNeedHandle, TaskEnsureError<A::PrepareError>> {
    // 1. Pure validation/derivation uses immutable borrows only.
    let validated = ValidatedTaskSpec::try_from(spec)?;
    self.validate_family_execution(&validated)?;
    self.validate_policy(&validated)?;

    // 2. Inspect journal and choose Join reuse or a staged new launch.
    match self.journal.plan_ensure(&validated, self.config.limits())? {
        EnsurePlan::Reuse(existing) => {
            self.journal.validate_join_equivalence(&validated, &existing)?;
            return Ok(self.journal.handle_for(existing)?);
        }
        EnsurePlan::New(plan) => {
            let correlation = self.journal.derive_correlation(&plan)?;

            // 3. Runtime work never reaches A.
            if let TaskExecution::Runtime(request) = validated.execution() {
                let staged = self.stage_runtime_launch(
                    plan,
                    correlation,
                    validated,
                    request.clone(),
                )?;
                self.publish_runtime_launch(staged);
                return Ok(self.journal.handle_for(correlation)?);
            }

            // 4. Host prepare is the last fallible external reservation.
            let host = validated.execution().as_host()
                .ok_or(TaskEnsureError::FamilyExecutionMismatch)?;
            let prepared = self.adapter.prepare_launch(
                HostTaskLaunchRequest::new(correlation, host.clone()),
            ).map_err(TaskEnsureError::AdapterPrepare)?;

            // 5. All journal state is built and cross-validated privately.
            let staged = match self.stage_host_launch(
                plan,
                correlation,
                validated,
            ) {
                Ok(staged) => staged,
                Err(error) => {
                    self.adapter.rollback_launch(prepared);
                    return Err(error);
                }
            };

            // 6. Journal/counter publication and adapter exposure are both
            // infallible. No `?` is legal below this point.
            self.publish_host_launch(staged);
            self.adapter.commit_launch(prepared);
            Ok(self.journal.handle_for_committed(correlation))
        }
    }
}
```

### Borrow facts

- `plan_ensure` returns owned staging data; it does not retain a borrow into the
  journal across adapter calls.
- `PreparedLaunch` is owned by the caller and cannot borrow `A` or scheduler
  maps.
- `stage_host_launch` builds owned replacement maps or an undo-free delta; it
  does not publish.
- `publish_*` is infallible by construction.
- the AlwaysStart counter changes only in `publish_*`; every prior failure
  leaves it unchanged.
- the Runtime branch has no adapter token and cannot call adapter methods.
- no `unsafe`, global state, `RefCell`, or cross-object rollback coordinator is
  required.
