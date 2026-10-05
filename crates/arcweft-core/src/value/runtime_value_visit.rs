use super::{RuntimeIterator, RuntimeSeq, RuntimeValue};

/// Visits a runtime value and every value retained below it.
///
/// The explicit stack stores borrowed nodes, so traversal does not recursively
/// clone or drop deeply nested runtime values. Opaque nodes are passed to the
/// visitor before their payload is queued; a typed owner can reject malformed
/// opaque payloads without traversal inspecting their contents first.
pub fn visit_runtime_value_graph<E>(
    root: &RuntimeValue,
    mut visitor: impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    let mut pending = vec![root];
    while let Some(value) = pending.pop() {
        visitor(value)?;
        match value {
            RuntimeValue::Tuple(values) => pending.extend(values),
            RuntimeValue::Seq(sequence) => push_sequence_values(sequence, &mut pending),
            RuntimeValue::Record(record) => {
                pending.extend(record.fields().iter().map(|field| field.value()));
            }
            RuntimeValue::NominalRecord(record) => pending.extend(record.fields()),
            RuntimeValue::Opaque(value) => pending.push(value.payload()),
            RuntimeValue::Reduction(value) => {
                pending.push(value.state());
                pending.extend(value.commands().iter().map(|command| &command.payload().0));
            }
            RuntimeValue::Agent(value) => pending.extend(
                value
                    .nested_runtime_values_with_depth()
                    .into_iter()
                    .map(|(_, nested)| nested),
            ),
            RuntimeValue::Callable(value) => pending.extend(value.retained()),
            RuntimeValue::NeedHandle(handle) => pending.extend(handle.request_values()),
            RuntimeValue::Iterator(RuntimeIterator::Values { items, .. }) => pending.extend(items),
            RuntimeValue::Iterator(RuntimeIterator::Witness { state, .. }) => {
                pending.push(state);
            }
            RuntimeValue::Variant {
                payload: Some(value),
                ..
            } => pending.push(value),
            RuntimeValue::Unit
            | RuntimeValue::Bool(_)
            | RuntimeValue::Int(_)
            | RuntimeValue::UInt(_)
            | RuntimeValue::F32(_)
            | RuntimeValue::F64(_)
            | RuntimeValue::MatrixF32(_)
            | RuntimeValue::MatrixF64(_)
            | RuntimeValue::TensorF32(_)
            | RuntimeValue::TensorF64(_)
            | RuntimeValue::String(_)
            | RuntimeValue::Color(_)
            | RuntimeValue::Char(_)
            | RuntimeValue::Duration(_)
            | RuntimeValue::Progress(_)
            | RuntimeValue::Range(_)
            | RuntimeValue::Iterator(RuntimeIterator::Range(_))
            | RuntimeValue::EntityRef(_)
            | RuntimeValue::Variant { payload: None, .. } => {}
        }
    }
    Ok(())
}

fn push_sequence_values<'a>(sequence: &'a RuntimeSeq, pending: &mut Vec<&'a RuntimeValue>) {
    let mut sequences = vec![sequence];
    while let Some(sequence) = sequences.pop() {
        match sequence {
            RuntimeSeq::Values(values) => pending.extend(values),
            RuntimeSeq::Dense(_) => {}
            RuntimeSeq::TupleColumns(columns) => sequences.extend(columns.columns()),
            RuntimeSeq::RecordColumns(columns) => {
                sequences.extend(columns.fields().iter().map(|field| field.values()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visits_deeply_nested_runtime_values_without_cloning_them() {
        let mut root = RuntimeValue::String("leaf".to_owned());
        for _ in 0..16_384 {
            root = RuntimeValue::Tuple(vec![root]);
        }

        let mut visited = 0;
        visit_runtime_value_graph(&root, |_| {
            visited += 1;
            Ok::<_, ()>(())
        })
        .expect("visits nested value graph");

        assert_eq!(visited, 16_385);
        // Dropping an arbitrarily deep recursive enum is recursive too; this
        // test is about traversal, so avoid making that unrelated behavior a
        // part of its stack-safety assertion.
        std::mem::forget(root);
    }
}
