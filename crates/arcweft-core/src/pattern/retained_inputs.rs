//! Inert input inspection never issues a live pattern-binding capability.

use std::collections::BTreeSet;

use super::{
    RuntimePattern, RuntimePatternKind, pattern_binding_capacity, pattern_matches_view_with_work,
};
use crate::runtime_id::RuntimeLocalDeclarationId;
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeBorrowedValue, RuntimeRetainedValueAdmission,
};

pub(crate) fn admit_retained_pattern_inputs(
    pattern: &RuntimePattern,
    value: RuntimeBorrowedValue<'_>,
    unrestricted: &[RuntimeLocalDeclarationId],
    admission: &mut RuntimeRetainedValueAdmission,
) -> Result<(), AwbcRuntimeValueSnapshotError> {
    admission.validate_metadata_collection(unrestricted.len())?;
    let mut required = unrestricted.iter().copied().collect::<BTreeSet<_>>();
    if required.len() != unrestricted.len()
        || !pattern_matches_view_with_work(pattern, value.view(), &admission.work())?
    {
        return Err(invalid());
    }
    walk(pattern, value, &mut required, admission)?;
    if !required.is_empty() {
        return Err(invalid());
    }
    Ok(())
}

fn invalid() -> AwbcRuntimeValueSnapshotError {
    AwbcRuntimeValueSnapshotError::Message {
        message: "retained invocation input differs from its original formal pattern".into(),
    }
}

fn walk(
    pattern: &RuntimePattern,
    value: RuntimeBorrowedValue<'_>,
    required: &mut BTreeSet<RuntimeLocalDeclarationId>,
    admission: &mut RuntimeRetainedValueAdmission,
) -> Result<(), AwbcRuntimeValueSnapshotError> {
    admission.work().charge(0)?;
    match pattern.kind() {
        RuntimePatternKind::Bind { binding, .. } | RuntimePatternKind::Typed { binding } => {
            if required.remove(&binding.local()) {
                admission.validate_admitted_unrestricted_borrowed(value)?;
            }
        }
        RuntimePatternKind::Whole { binding, pattern } => {
            if pattern_binding_capacity(pattern) != 0 || required.remove(&binding.local()) {
                admission.validate_admitted_unrestricted_borrowed(value)?;
            }
            // Short-circuiting the overlap check must not leave this binding
            // in the required set after its complete value was checked.
            required.remove(&binding.local());
            walk(pattern, value, required, admission)?;
        }
        RuntimePatternKind::Or(alternatives) => {
            let mut selected = None;
            for pattern in alternatives {
                if pattern_matches_view_with_work(pattern, value.view(), &admission.work())? {
                    selected = Some(pattern);
                    break;
                }
            }
            let selected = selected.ok_or_else(invalid)?;
            walk(selected, value, required, admission)?;
        }
        RuntimePatternKind::Tuple(items) => {
            for (index, pattern) in items.iter().enumerate() {
                walk(
                    pattern,
                    value.child_at(index).ok_or_else(invalid)?,
                    required,
                    admission,
                )?;
            }
        }
        RuntimePatternKind::Record { fields, rest } => {
            let overlapping = rest.binding().is_some()
                && fields
                    .iter()
                    .any(|field| pattern_binding_capacity(field.pattern()) != 0);
            let unrestricted_rest = rest
                .binding()
                .is_some_and(|binding| required.remove(&binding.local()));
            if overlapping || unrestricted_rest {
                admission.validate_admitted_unrestricted_borrowed(value)?;
            }
            for field in fields {
                let child = value
                    .child_at(field.field().zero_based() as usize)
                    .ok_or_else(invalid)?;
                walk(field.pattern(), child, required, admission)?;
            }
        }
        RuntimePatternKind::Sequence { items, rest } => {
            let Some(sequence) = value.sequence() else {
                return Err(invalid());
            };
            for (index, pattern) in items.iter().enumerate() {
                walk(
                    pattern,
                    sequence.get(index).ok_or_else(invalid)?,
                    required,
                    admission,
                )?;
            }
            if rest
                .binding()
                .is_some_and(|binding| required.remove(&binding.local()))
            {
                let length = sequence.len().map_err(|_| invalid())?;
                admission
                    .work()
                    .collection(length.saturating_sub(items.len()))?;
                for index in items.len()..length {
                    admission.validate_admitted_unrestricted_borrowed(
                        sequence.get(index).ok_or_else(invalid)?,
                    )?;
                }
            }
        }
        RuntimePatternKind::Variant {
            payload: Some(pattern),
            ..
        } => {
            walk(
                pattern,
                value.child_at(0).ok_or_else(invalid)?,
                required,
                admission,
            )?;
        }
        RuntimePatternKind::Variant { payload: None, .. }
        | RuntimePatternKind::Discard
        | RuntimePatternKind::Literal(_)
        | RuntimePatternKind::Entity(_) => {}
    }
    Ok(())
}
