//! Complete immutable project-call packets retain the original continuations.

use super::{Engine, binder};
use crate::plan::{FlowOp, RuntimePlan};
use crate::pure::{
    RuntimeCallBackend, RuntimeNumericBatch, RuntimeNumericBatchResult, RuntimeNumericCallRow,
    RuntimeNumericProjection,
};
use crate::step::RuntimeStepOutput;
use std::sync::Arc;

pub(super) struct NumericProjectCallPacket<'plan> {
    batch: RuntimeNumericBatch<'plan>,
    results: Vec<&'plan crate::pattern::RuntimePattern>,
    operations: usize,
    cost: usize,
}

impl Engine {
    pub(super) fn prepare_numeric_project_calls<'plan>(
        &self,
        owner: &'plan Arc<RuntimePlan>,
        ops: &[&FlowOp],
        remaining_ops: usize,
    ) -> Option<NumericProjectCallPacket<'plan>> {
        let count = ops
            .iter()
            .take_while(|op| matches!(op, FlowOp::ProjectCall { .. }))
            .count();
        if count < 2
            || count > remaining_ops
            || count.checked_mul(std::mem::size_of::<RuntimeNumericCallRow<'_>>())?
                > isize::MAX as usize
            || count
                .checked_mul(crate::pure::RuntimeI64Args::MAX)?
                .checked_mul(std::mem::size_of::<&crate::value::RuntimeValue>())?
                > isize::MAX as usize
        {
            return None;
        }
        let FlowOp::ProjectCall { site } = ops[0] else {
            return None;
        };
        let first = RuntimeNumericCallRow::project(owner, *site)?;
        let cost = count.checked_mul(
            1_usize.checked_add(first.function().exact_scalar_completion_control_ops()?)?,
        )?;
        if cost > remaining_ops {
            return None;
        }
        let mut rows = Vec::with_capacity(count);
        rows.push(first);
        for op in &ops[1..count] {
            let FlowOp::ProjectCall { site } = op else {
                return None;
            };
            rows.push(RuntimeNumericCallRow::project(owner, *site)?);
        }
        let projection = RuntimeNumericProjection::from_calls(&rows)?;
        let mut results = std::collections::BTreeSet::new();
        for row in &rows {
            let local = binder(row.result())?;
            if self.fiber.frame.env.get(local).is_some() || !results.insert(local) {
                return None;
            }
        }
        let admission = projection.admit_calls(&rows)?;
        let batch = admission.materialize();
        let results = rows
            .iter()
            .map(RuntimeNumericCallRow::result)
            .collect::<Vec<_>>();
        Some(NumericProjectCallPacket {
            batch,
            results,
            operations: count,
            cost,
        })
    }

    pub(super) fn commit_numeric_project_calls(
        &mut self,
        packet: NumericProjectCallPacket<'_>,
        output: &mut RuntimeStepOutput,
        backend: &mut impl RuntimeCallBackend,
    ) -> Option<usize> {
        let NumericProjectCallPacket {
            batch,
            results,
            operations,
            cost,
        } = packet;
        backend.prepare_total_numeric_batch(batch.function(), batch.rows())?;
        let executed = self.with_main_flow_transaction(
            output,
            backend,
            move |candidate, staged, backend, _| {
                match batch.execute(backend) {
                    Ok(RuntimeNumericBatchResult::Values(values)) => {
                        assert_eq!(
                            values.len(),
                            results.len(),
                            "one value per admitted complete row"
                        );
                        for (pattern, value) in results.iter().zip(values) {
                            candidate.bind_value(pattern, value, staged);
                        }
                        candidate.consume_projected_ops(operations);
                        candidate.run_child_next = true;
                    }
                    Ok(RuntimeNumericBatchResult::Sum(_)) => {
                        unreachable!("non-reduced call admission cannot produce a Sum packet")
                    }
                    Err(error) => candidate.fail_eval(error, staged),
                }
                cost
            },
        );
        Some(executed)
    }
}
