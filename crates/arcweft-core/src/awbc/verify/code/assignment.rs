//! Post-fixed-point admission of the old-value cleanup contour.

use super::{
    AwbcMutablePlace, AwbcRegisterId, AwbcVerifyError, FlowState, Verifier, invalid_type,
    record_child_type, register_type,
};
use crate::value::{
    RuntimeDisplacedField, RuntimePlaceDisplacement, RuntimePlaceInitialization,
    RuntimeRecordFieldId,
};

pub(in crate::awbc::verify) enum AssignmentAdmission<'a> {
    Sealed,
    Prepared(&'a mut Vec<(usize, RuntimePlaceDisplacement<RuntimeRecordFieldId>)>),
}

impl AssignmentAdmission<'_> {
    fn accepts(
        &self,
        expected: RuntimePlaceInitialization,
        actual: RuntimePlaceInitialization,
    ) -> bool {
        expected == actual
            || (matches!(self, Self::Prepared(_))
                && expected == RuntimePlaceInitialization::Conditional)
    }
}

pub(super) fn verify_displacement(
    verifier: &Verifier<'_, '_>,
    function: usize,
    block: usize,
    instruction: usize,
    place: &AwbcMutablePlace,
    displacement: &RuntimePlaceDisplacement<RuntimeRecordFieldId>,
    state: &FlowState,
    admission: &mut AssignmentAdmission<'_>,
) -> Result<(), AwbcVerifyError> {
    let at = format!("instruction {instruction} assignment displacement");
    let RuntimePlaceDisplacement::Reachable {
        initialization,
        fields,
    } = displacement
    else {
        return invalid_type(&at, "reachable assignment cleanup contour");
    };
    let base = place.base();
    let target = place.fields();
    let mut target_ty = register_type(verifier, function, block, base)?;
    for field in target {
        target_ty = record_child_type(verifier.program, target_ty, *field, &at)?;
    }
    let actual = initialization_at(state, base, &target);
    if !admission.accepts(*initialization, actual) {
        return invalid_type(
            &at,
            &format!(
                "cleanup initialization agrees with post-RHS dataflow: annotation {initialization:?}, dataflow {actual:?}, register {}",
                base.0
            ),
        );
    }
    if fields
        .windows(2)
        .any(|pair| pair[0].fields >= pair[1].fields)
    {
        return invalid_type(&at, "cleanup field paths are unique and ordered");
    }
    let mut refined = matches!(admission, AssignmentAdmission::Prepared(_))
        .then(|| Vec::with_capacity(fields.len()));
    for field in fields {
        if field.fields.is_empty() {
            return invalid_type(&at, "cleanup field path is nonempty");
        }
        let mut ty = target_ty;
        for child in &field.fields {
            ty = record_child_type(verifier.program, ty, *child, &at)?;
        }
        let path: Vec<_> = target.iter().chain(field.fields.iter()).copied().collect();
        let child = initialization_at(state, base, &path);
        if !admission.accepts(field.initialization, child) {
            return invalid_type(
                &at,
                "cleanup child initialization agrees with post-RHS dataflow",
            );
        }
        if let Some(refined) = &mut refined {
            refined.push(RuntimeDisplacedField {
                fields: field.fields.clone(),
                initialization: child,
            });
        }
    }
    let candidates = refined.as_deref().unwrap_or(fields);
    if actual == RuntimePlaceInitialization::Initialized {
        for ((root, path), _) in &state.moved_fields {
            if *root == base
                && path.starts_with(&target)
                && !candidates.iter().any(|field| {
                    field.initialization != RuntimePlaceInitialization::Initialized
                        && path[target.len()..].starts_with(&field.fields)
                })
            {
                return invalid_type(
                    &at,
                    "cleanup contour accounts for every partially moved child",
                );
            }
        }
    }
    if let AssignmentAdmission::Prepared(updates) = admission {
        let displacement = RuntimePlaceDisplacement::Reachable {
            initialization: actual,
            fields: refined
                .expect("prepared contour fields were projected")
                .into_boxed_slice(),
        };
        updates.push((instruction, displacement));
    }
    Ok(())
}

fn initialization_at(
    state: &FlowState,
    root: AwbcRegisterId,
    path: &[RuntimeRecordFieldId],
) -> RuntimePlaceInitialization {
    let initial = state.initialized[root.index()];
    if initial != RuntimePlaceInitialization::Initialized {
        return initial;
    }
    state
        .moved_fields
        .iter()
        .filter(|((register, moved), _)| *register == root && path.starts_with(moved))
        .max_by_key(|((_, moved), _)| moved.len())
        .map_or(initial, |(_, initialization)| *initialization)
}
