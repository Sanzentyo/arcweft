//! One borrowed structural predicate for live and retained input inspection.

use super::{
    RuntimePattern, RuntimePatternKind, RuntimeScalarView, RuntimeValueView,
    runtime_value_views_equal,
};
use crate::entry::RuntimeSchemaError;
use crate::entry::schema::value_budget::ValidationWork;

pub(super) fn pattern_matches_view(pattern: &RuntimePattern, value: RuntimeValueView<'_>) -> bool {
    matches(pattern, value, &mut || {
        Ok::<(), std::convert::Infallible>(())
    })
    .expect("the unmetered structural predicate has no resource failure")
}

pub(super) fn pattern_matches_view_with_work(
    pattern: &RuntimePattern,
    value: RuntimeValueView<'_>,
    work: &ValidationWork,
) -> Result<bool, RuntimeSchemaError> {
    let mut work = work.clone();
    matches(pattern, value, &mut || work.charge(0))
}

fn matches<E>(
    pattern: &RuntimePattern,
    value: RuntimeValueView<'_>,
    step: &mut impl FnMut() -> Result<(), E>,
) -> Result<bool, E> {
    step()?;
    Ok(match pattern.kind() {
        RuntimePatternKind::Or(alternatives) => {
            let mut accepted = false;
            for pattern in alternatives {
                if matches(pattern, value, step)? {
                    accepted = true;
                    break;
                }
            }
            accepted
        }
        RuntimePatternKind::Bind { .. }
        | RuntimePatternKind::Typed { .. }
        | RuntimePatternKind::Discard => true,
        RuntimePatternKind::Literal(expected) => runtime_value_views_equal(expected.view(), value),
        RuntimePatternKind::Entity(expected) => {
            matches!(value, RuntimeValueView::Scalar(RuntimeScalarView::EntityRef(actual)) if actual == expected)
        }
        RuntimePatternKind::Whole { pattern, .. } => matches(pattern, value, step)?,
        RuntimePatternKind::Tuple(patterns) => {
            let RuntimeValueView::Tuple(values) = value else {
                return Ok(false);
            };
            if patterns.len() != values.len() {
                return Ok(false);
            }
            for (index, pattern) in patterns.iter().enumerate() {
                let Some(value) = values.get(index) else {
                    return Ok(false);
                };
                if !matches(pattern, value, step)? {
                    return Ok(false);
                }
            }
            true
        }
        RuntimePatternKind::Record { fields, rest } => {
            let count = match value {
                RuntimeValueView::Record(record) => record.len(),
                RuntimeValueView::NominalRecord(record) => record.fields().len(),
                _ => return Ok(false),
            };
            if !rest.accepts_len(fields.len(), count) {
                return Ok(false);
            }
            for field in fields {
                let index = field.field().zero_based() as usize;
                let nested = match value {
                    RuntimeValueView::Record(record) => record
                        .get(index)
                        .filter(|(id, _, _)| *id == field.field())
                        .map(|(_, _, value)| value),
                    RuntimeValueView::NominalRecord(record) => record.fields().get(index),
                    _ => None,
                };
                let Some(nested) = nested else {
                    return Ok(false);
                };
                if !matches(field.pattern(), nested, step)? {
                    return Ok(false);
                }
            }
            true
        }
        RuntimePatternKind::Sequence { items, rest } => {
            let RuntimeValueView::Sequence(sequence) = value else {
                return Ok(false);
            };
            if !sequence
                .len()
                .is_ok_and(|length| rest.accepts_len(items.len(), length))
            {
                return Ok(false);
            }
            for (index, pattern) in items.iter().enumerate() {
                let Some(value) = sequence.value_view(index) else {
                    return Ok(false);
                };
                if !matches(pattern, value, step)? {
                    return Ok(false);
                }
            }
            true
        }
        RuntimePatternKind::Variant {
            ordinal, payload, ..
        } => {
            let RuntimeValueView::Variant(variant) = value else {
                return Ok(false);
            };
            if *ordinal != variant.ordinal() {
                return Ok(false);
            }
            match (payload.as_deref(), variant.payload()) {
                (Some(pattern), Some(value)) => matches(pattern, value, step)?,
                (None, None) => true,
                _ => false,
            }
        }
    })
}
