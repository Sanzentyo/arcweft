//! Complete omitted binding rows using the same shape in each effect domain.

use std::collections::BTreeSet;

use crate::types::{
    TypeEffectDomain, TypeInstantiationError, TypeKind, TypeProjectionControl, TypeProjectionError,
    TypeProjectionNodeKind, constraints::TypeConstraintShape,
};

type BindingEffectsResult<T, C> =
    Result<T, TypeProjectionError<<C as TypeProjectionControl>::Error>>;

/// Copy an initializer row only at an omitted annotation. The surrounding
/// binding check owns directional compatibility and the original scopes.
pub(super) fn complete<D: TypeEffectDomain, C: TypeProjectionControl>(
    declared: &TypeKind<D>,
    actual: &TypeKind<D>,
    row_needs_inference: fn(&D::Row) -> bool,
    control: &mut C,
) -> BindingEffectsResult<Option<TypeKind<D>>, C> {
    enum Task<'a, D: TypeEffectDomain> {
        Enter(&'a TypeKind<D>, &'a TypeKind<D>, u64),
        Finish(TypeConstraintShape<'a, D>, usize, Option<&'a D::Row>),
    }

    control.check().map_err(TypeProjectionError::Control)?;
    if !contains_omission(declared, 1, row_needs_inference, control)? {
        return Ok(Some(declared.clone()));
    }
    visit_type(1, control)?;
    let mut pending = vec![Task::Enter(declared, actual, 1)];
    let mut completed = Vec::new();
    while let Some(task) = pending.pop() {
        match task {
            Task::Enter(declared, actual, depth) => {
                let shape = declared.constraint_shape();
                let supplied = actual.constraint_shape();
                if !shape.same_header(supplied) {
                    if contains_omission(declared, depth, row_needs_inference, control)? {
                        return Ok(None);
                    }
                    completed.push(declared.clone());
                    continue;
                }
                if matches!(
                    shape,
                    TypeConstraintShape::Unresolved(_)
                        | TypeConstraintShape::Leaf(_)
                        | TypeConstraintShape::Never
                        | TypeConstraintShape::Generic(_)
                ) {
                    if matches!(shape, TypeConstraintShape::Unresolved(_)) {
                        // An unresolved projection can still own a full subject tree.
                        contains_omission(declared, depth, |_| false, control)?;
                    }
                    completed.push(declared.clone());
                    continue;
                }
                visit_header(shape, depth, control)?;
                let inferred = match (shape, supplied) {
                    (
                        TypeConstraintShape::Function { effects, .. },
                        TypeConstraintShape::Function {
                            effects: actual, ..
                        },
                    ) if row_needs_inference(effects) => {
                        if row_needs_inference(actual) {
                            return Ok(None);
                        }
                        Some(actual)
                    }
                    _ => None,
                };
                let child_depth = child_depth(depth)?;
                let children = match (shape, supplied) {
                    (
                        TypeConstraintShape::Choice(declared),
                        TypeConstraintShape::Choice(actual),
                    ) => {
                        let Some(pairs) =
                            match_alternatives(declared, actual, child_depth, control)?
                        else {
                            return Ok(None);
                        };
                        pairs
                    }
                    _ => {
                        let mut matched = Vec::new();
                        for (declared, actual) in shape.children().zip(supplied.children()) {
                            visit_type(child_depth, control)?;
                            matched.push((declared, actual));
                        }
                        matched
                    }
                };
                pending.push(Task::Finish(shape, children.len(), inferred));
                pending.extend(
                    children
                        .into_iter()
                        .rev()
                        .map(|(declared, actual)| Task::Enter(declared, actual, child_depth)),
                );
            }
            Task::Finish(shape, count, inferred) => {
                let Some(first) = completed.len().checked_sub(count) else {
                    return Ok(None);
                };
                let children = completed.split_off(first);
                let rebuilt = shape.rebuild_with(
                    children,
                    &mut (),
                    |(), length| Ok::<_, TypeProjectionError<C::Error>>(length.clone()),
                    |(), row| Ok(inferred.unwrap_or(row).clone()),
                    |(), predicate| Ok(predicate.clone()),
                )?;
                completed.push(rebuilt);
            }
        }
    }
    Ok(completed.pop())
}

fn child_depth(depth: u64) -> Result<u64, TypeInstantiationError> {
    depth
        .checked_add(1)
        .ok_or(TypeInstantiationError::DepthOverflow)
}

fn visit_type<C: TypeProjectionControl>(
    depth: u64,
    control: &mut C,
) -> BindingEffectsResult<(), C> {
    control
        .visit_node(TypeProjectionNodeKind::Type, depth)
        .map_err(TypeProjectionError::Control)
}

fn visit_header<D: TypeEffectDomain, C: TypeProjectionControl>(
    shape: TypeConstraintShape<'_, D>,
    depth: u64,
    control: &mut C,
) -> BindingEffectsResult<(), C> {
    let (kind, count) = match shape {
        TypeConstraintShape::Array { .. } => (TypeProjectionNodeKind::Const, 1),
        TypeConstraintShape::Function { .. } => (TypeProjectionNodeKind::Effect, 2),
        _ => return Ok(()),
    };
    for _ in 0..count {
        control
            .visit_node(kind, depth)
            .map_err(TypeProjectionError::Control)?;
    }
    Ok(())
}

fn contains_omission<D: TypeEffectDomain, C: TypeProjectionControl>(
    ty: &TypeKind<D>,
    depth: u64,
    row_needs_inference: fn(&D::Row) -> bool,
    control: &mut C,
) -> BindingEffectsResult<bool, C> {
    visit_type(depth, control)?;
    let mut pending = vec![(ty, depth)];
    while let Some((ty, depth)) = pending.pop() {
        let shape = ty.constraint_shape();
        visit_header(shape, depth, control)?;
        if let TypeConstraintShape::Function { effects, .. } = shape
            && row_needs_inference(effects)
        {
            return Ok(true);
        }
        let depth = child_depth(depth)?;
        for child in shape.children() {
            visit_type(depth, control)?;
            pending.push((child, depth));
        }
    }
    Ok(false)
}

fn match_alternatives<'a, D: TypeEffectDomain, C: TypeProjectionControl>(
    declared: &'a [TypeKind<D>],
    actual: &'a [TypeKind<D>],
    depth: u64,
    control: &mut C,
) -> BindingEffectsResult<Option<Vec<(&'a TypeKind<D>, &'a TypeKind<D>)>>, C> {
    let mut matched = Vec::new();
    let mut used = BTreeSet::new();
    for declared in declared {
        let mut matching = None;
        for (index, actual) in actual.iter().enumerate() {
            if !used.contains(&index) && corresponding_shape(declared, actual, depth, control)? {
                if matching.replace((index, actual)).is_some() {
                    return Ok(None);
                }
            }
        }
        let Some((index, actual)) = matching else {
            return Ok(None);
        };
        visit_type(depth, control)?;
        used.insert(index);
        matched.push((declared, actual));
    }
    Ok(Some(matched))
}

pub(super) fn corresponding_shape<D: TypeEffectDomain, C: TypeProjectionControl>(
    declared: &TypeKind<D>,
    actual: &TypeKind<D>,
    depth: u64,
    control: &mut C,
) -> BindingEffectsResult<bool, C> {
    visit_type(depth, control)?;
    let mut pending = vec![(declared, actual, depth)];
    while let Some((left, right, depth)) = pending.pop() {
        let left = left.constraint_shape();
        let right = right.constraint_shape();
        if !left.same_header(right) {
            return Ok(false);
        }
        let depth = child_depth(depth)?;
        for (left, right) in left.children().zip(right.children()) {
            visit_type(depth, control)?;
            pending.push((left, right, depth));
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        effect_row::{
            DecisionControl, DecisionWork, EffectPredicate, EffectRow,
            prepared::{PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow},
        },
        types::{GenericBinder, PreparedEffects},
    };

    #[derive(Debug, thiserror::Error, PartialEq)]
    #[error("binding projection work exhausted")]
    struct Exhausted;

    struct Work(usize);

    impl DecisionControl for Work {
        type Error = Exhausted;

        fn charge(&mut self, _: DecisionWork) -> Result<(), Exhausted> {
            self.0 = self.0.checked_sub(1).ok_or(Exhausted)?;
            Ok(())
        }
    }

    impl TypeProjectionControl for Work {
        type Error = Exhausted;

        fn check(&mut self) -> Result<(), Exhausted> {
            (self.0 > 0).then_some(()).ok_or(Exhausted)
        }

        fn visit_node(&mut self, _: TypeProjectionNodeKind, _: u64) -> Result<(), Exhausted> {
            self.charge(DecisionWork::Visit)
        }

        fn visit_binding(&mut self) -> Result<(), Exhausted> {
            self.charge(DecisionWork::Node)
        }
    }

    #[test]
    fn seeded_binding_fills_nested_omission_without_replacing_explicit_graph_term() {
        fn function(
            row: PreparedEffectRow,
            result: TypeKind<PreparedEffects>,
        ) -> TypeKind<PreparedEffects> {
            TypeKind::function_with_contract(
                GenericBinder::EMPTY,
                PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                [],
                result,
                row,
            )
        }

        let mut graph = PreparedEffectGraph::default();
        let mut work = Work(10_000);
        let first = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        let second = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        let first = graph.application(&first, vec![], &mut work).unwrap();
        let second = graph.application(&second, vec![], &mut work).unwrap();
        let annotation = function(
            first.clone(),
            function(
                PreparedEffectRow::Canonical(EffectRow::unknown()),
                TypeKind::I64,
            ),
        );
        let inferred = function(first, function(second.clone(), TypeKind::I64));
        assert!(
            annotation
                .completes_seeded_binding(&inferred, &mut work)
                .unwrap()
        );

        let replaced = function(second.clone(), function(second, TypeKind::I64));
        assert!(
            !annotation
                .completes_seeded_binding(&replaced, &mut work)
                .unwrap()
        );
        assert_eq!(
            annotation.completes_seeded_binding(&inferred, &mut Work(1)),
            Err(TypeProjectionError::Control(Exhausted))
        );
    }
}
