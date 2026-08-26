//! Callable-owned overlay that instantiates higher-order effect positions for
//! lower without cloning or rewriting the accepted signature schema.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use crate::{
    effect_row::{EffectRow, EffectRowTail, EffectVar, EffectVarIssuer},
    types::{
        TypeKind,
        constraints::{TypeConstraintShape, TypeConstraintSolution},
    },
};

use super::super::{
    CallConstraintInvariant, CallableGenericFirstUse, CallableGroupIndex,
    CallableParameterCoordinate, CallableSignatureSchema,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CallableEffectPositionRoot {
    Invocation,
    Parameter(CallableParameterCoordinate),
    Result,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum CallableEffectTypeEdge {
    Unary,
    IteratorItem,
    ArrayItem,
    RefValue,
    MapKey,
    MapValue,
    BorrowInner,
    PairFirst,
    PairSecond,
    FunctionParameter(u32),
    FunctionResult,
    NominalArgument(u32),
    TupleElement(u32),
    ChoiceAlternative(u32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreparedEffectSourceTail {
    Unknown,
    Existing(EffectVar),
    ProjectedCallable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedCallableEffectPosition {
    root: CallableEffectPositionRoot,
    path: Box<[CallableEffectTypeEdge]>,
    variable: EffectVar,
    first_use: CallableGenericFirstUse,
    source: PreparedEffectSourceTail,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCallableEffectVariable {
    variable: EffectVar,
    first_use: CallableGenericFirstUse,
}

impl PreparedCallableEffectVariable {
    pub(crate) const fn variable(self) -> EffectVar {
        self.variable
    }

    pub(crate) const fn first_use(self) -> CallableGenericFirstUse {
        self.first_use
    }
}

/// The one prepared constraint overlay owned beside the original schema.
/// Projection methods return ephemeral lower inputs; no instantiated schema is
/// stored as a second authority.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparedCallableEffectInstantiation {
    issuer: EffectVarIssuer,
    positions: Arc<[PreparedCallableEffectPosition]>,
    variables: Arc<[PreparedCallableEffectVariable]>,
}

/// Affine-finalization evidence copied into a selected application before the
/// shared prepared definition arena is detached. It has no projection API and
/// therefore cannot act as a second overlay authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PreparedCallableEffectInstantiationEvidence {
    issuer: EffectVarIssuer,
    positions: Arc<[PreparedCallableEffectPosition]>,
    variables: Arc<[PreparedCallableEffectVariable]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CheckedCallableEffectInstantiation {
    issuer: EffectVarIssuer,
    positions: Box<[CheckedCallableEffectPosition]>,
    variables: Box<[CheckedCallableEffectVariable]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckedCallableEffectPosition {
    root: CallableEffectPositionRoot,
    path: Box<[CallableEffectTypeEdge]>,
    ordinal: u32,
    first_use: CallableGenericFirstUse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedCallableEffectVariable {
    ordinal: u32,
    first_use: CallableGenericFirstUse,
}

struct OverlayBuilder {
    issuer: EffectVarIssuer,
    next: u32,
    existing: BTreeMap<EffectVar, EffectVar>,
    positions: Vec<PreparedCallableEffectPosition>,
    variables: BTreeMap<EffectVar, CallableGenericFirstUse>,
}

impl PreparedCallableEffectInstantiation {
    pub(crate) fn seal(schema: &CallableSignatureSchema) -> Result<Self, CallConstraintInvariant> {
        let issuer = EffectVarIssuer::fresh_prepared()
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        let mut builder = OverlayBuilder {
            issuer,
            next: 0,
            existing: BTreeMap::new(),
            positions: Vec::new(),
            variables: BTreeMap::new(),
        };

        match schema.effects().fixed_row() {
            Some(row) => builder.record_row(
                CallableEffectPositionRoot::Invocation,
                &[],
                CallableGenericFirstUse::Group(CallableGroupIndex::ZERO),
                row.tail(),
            )?,
            None => builder.record_source(
                CallableEffectPositionRoot::Invocation,
                &[],
                CallableGenericFirstUse::Group(CallableGroupIndex::ZERO),
                PreparedEffectSourceTail::ProjectedCallable,
            )?,
        }

        for group in schema.groups() {
            for parameter in group.parameters() {
                let Some(declared) = parameter.declared_type() else {
                    continue;
                };
                let root = CallableEffectPositionRoot::Parameter(CallableParameterCoordinate::new(
                    group.index(),
                    parameter.index(),
                ));
                builder.scan_type(
                    root,
                    CallableGenericFirstUse::Group(group.index()),
                    declared,
                    &mut Vec::new(),
                )?;
            }
        }
        builder.scan_type(
            CallableEffectPositionRoot::Result,
            CallableGenericFirstUse::Result,
            schema.result(),
            &mut Vec::new(),
        )?;

        if builder
            .positions
            .windows(2)
            .any(|rows| (&rows[0].root, &rows[0].path) >= (&rows[1].root, &rows[1].path))
        {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
        let variables = builder
            .variables
            .into_iter()
            .map(|(variable, first_use)| PreparedCallableEffectVariable {
                variable,
                first_use,
            })
            .collect::<Vec<_>>();
        Ok(Self {
            issuer,
            positions: builder.positions.into(),
            variables: variables.into(),
        })
    }

    pub(crate) const fn issuer(&self) -> EffectVarIssuer {
        self.issuer
    }

    /// Prepared issuers are generation-local capabilities, not semantic
    /// identity. Replay compares the complete typed overlay after alpha-
    /// renaming each namespace by ordinal; accepted-schema source tails stay
    /// exact and therefore cannot be hidden by the rename.
    pub(crate) fn replay_eq(&self, other: &Self) -> bool {
        self.positions.len() == other.positions.len()
            && self.variables.len() == other.variables.len()
            && self
                .positions
                .iter()
                .zip(other.positions.iter())
                .all(|(left, right)| {
                    left.root == right.root
                        && left.path == right.path
                        && left.first_use == right.first_use
                        && left.source == right.source
                        && left.variable.issuer() == self.issuer
                        && right.variable.issuer() == other.issuer
                        && left.variable.index() == right.variable.index()
                })
            && self
                .variables
                .iter()
                .zip(other.variables.iter())
                .all(|(left, right)| {
                    left.first_use == right.first_use
                        && left.variable.issuer() == self.issuer
                        && right.variable.issuer() == other.issuer
                        && left.variable.index() == right.variable.index()
                })
    }

    /// Compare two completed lower solutions in their respective prepared
    /// overlay namespaces. Prepared effect issuers are capabilities minted
    /// per resolution, so exact `EffectVar` equality is not semantic replay
    /// equality; the definition-owned overlay is the only authority allowed
    /// to alpha-rebind those variables by its sealed ordinal inventory.
    pub(crate) fn solution_replay_eq(
        &self,
        solution: &TypeConstraintSolution,
        other: &Self,
        other_solution: &TypeConstraintSolution,
    ) -> bool {
        if !self.replay_eq(other) {
            return false;
        }
        if solution == other_solution {
            return true;
        }
        let authorized_ordinals = self
            .variables
            .iter()
            .map(|variable| variable.variable.index())
            .collect::<BTreeSet<_>>();
        solution
            .checked_rebind_effect_issuer(self.issuer, other.issuer, &authorized_ordinals)
            .is_ok_and(|rebound| rebound == *other_solution)
    }

    pub(crate) fn variables(&self) -> &[PreparedCallableEffectVariable] {
        &self.variables
    }

    pub(crate) fn evidence(&self) -> PreparedCallableEffectInstantiationEvidence {
        PreparedCallableEffectInstantiationEvidence {
            issuer: self.issuer,
            positions: Arc::clone(&self.positions),
            variables: Arc::clone(&self.variables),
        }
    }

    pub(crate) fn project_parameter(
        &self,
        schema: &CallableSignatureSchema,
        coordinate: CallableParameterCoordinate,
    ) -> Result<Option<TypeKind>, CallConstraintInvariant> {
        let declared = schema
            .group(coordinate.group())
            .and_then(|group| group.parameter(coordinate.parameter()))
            .and_then(|parameter| parameter.declared_type());
        declared
            .map(|declared| {
                self.project_type(
                    CallableEffectPositionRoot::Parameter(coordinate),
                    declared,
                    &mut Vec::new(),
                )
            })
            .transpose()
    }

    pub(crate) fn project_result(
        &self,
        schema: &CallableSignatureSchema,
    ) -> Result<TypeKind, CallConstraintInvariant> {
        self.project_type(
            CallableEffectPositionRoot::Result,
            schema.result(),
            &mut Vec::new(),
        )
    }

    pub(crate) fn project_invocation_effects(
        &self,
        schema: &CallableSignatureSchema,
    ) -> Result<EffectRow, CallConstraintInvariant> {
        let position = self.position(CallableEffectPositionRoot::Invocation, &[]);
        match (schema.effects().fixed_row(), position) {
            (Some(row), None) if matches!(row.tail(), EffectRowTail::Closed) => Ok(row.clone()),
            (Some(row), Some(position)) if source_matches(position.source, Some(row.tail())) => {
                Ok(EffectRow::open(row.concrete().clone(), position.variable))
            }
            (None, Some(position))
                if matches!(position.source, PreparedEffectSourceTail::ProjectedCallable) =>
            {
                Ok(EffectRow::open(
                    crate::effects::EffectSet::new(),
                    position.variable,
                ))
            }
            _ => Err(CallConstraintInvariant::MalformedSchemaInventory),
        }
    }

    pub(crate) fn into_checked(
        self,
        checked_issuer: EffectVarIssuer,
    ) -> CheckedCallableEffectInstantiation {
        let positions = self
            .positions
            .iter()
            .map(|position| CheckedCallableEffectPosition {
                root: position.root,
                path: position.path.clone(),
                ordinal: position.variable.index(),
                first_use: position.first_use,
            })
            .collect();
        let variables = self
            .variables
            .iter()
            .map(|variable| CheckedCallableEffectVariable {
                ordinal: variable.variable.index(),
                first_use: variable.first_use,
            })
            .collect();
        CheckedCallableEffectInstantiation {
            issuer: checked_issuer,
            positions,
            variables,
        }
    }

    fn position(
        &self,
        root: CallableEffectPositionRoot,
        path: &[CallableEffectTypeEdge],
    ) -> Option<&PreparedCallableEffectPosition> {
        self.positions
            .binary_search_by(|position| {
                (&position.root, position.path.as_ref()).cmp(&(&root, path))
            })
            .ok()
            .map(|index| &self.positions[index])
    }

    fn project_type(
        &self,
        root: CallableEffectPositionRoot,
        ty: &TypeKind,
        path: &mut Vec<CallableEffectTypeEdge>,
    ) -> Result<TypeKind, CallConstraintInvariant> {
        let shape = ty.constraint_shape();
        if matches!(shape, TypeConstraintShape::Unresolved) {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
        let children = typed_children(shape)?
            .into_iter()
            .map(|(edge, child)| {
                path.push(edge);
                let projected = self.project_type(root, child, path);
                path.pop();
                projected
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut rebuilt = shape
            .rebuild(children)
            .map_err(|_| CallConstraintInvariant::MalformedSchemaInventory)?;
        if let TypeKind::Function { effects, .. } = &mut rebuilt {
            let position = self.position(root, path);
            match (effects.tail(), position) {
                (EffectRowTail::Closed, None) => {}
                (tail, Some(position)) if source_matches(position.source, Some(tail)) => {
                    *effects = EffectRow::open(effects.concrete().clone(), position.variable);
                }
                _ => return Err(CallConstraintInvariant::MalformedSchemaInventory),
            }
        }
        Ok(rebuilt)
    }

    /// Consumes one definition-issued source projection by replacing only
    /// unresolved effect tails that the original schema owned at the same
    /// typed position. The raw semantic expression remains unchanged; this
    /// projected value exists only as lower input.
    pub(super) fn seal_source_actual(
        &self,
        source: &TypeKind,
        projected: &TypeKind,
        actual: &TypeKind,
    ) -> Result<TypeKind, CallConstraintInvariant> {
        seal_source_actual_inner(self.issuer, source, projected, actual)
    }
}

fn seal_source_actual_inner(
    issuer: EffectVarIssuer,
    source: &TypeKind,
    projected: &TypeKind,
    actual: &TypeKind,
) -> Result<TypeKind, CallConstraintInvariant> {
    let source_shape = source.constraint_shape();
    let projected_shape = projected.constraint_shape();
    if matches!(source_shape, TypeConstraintShape::Unresolved)
        || !source_shape.same_header(projected_shape)
    {
        return Err(CallConstraintInvariant::PreparedEffectSourceShapeMismatch);
    }

    if matches!(source_shape, TypeConstraintShape::Generic(_)) {
        ensure_source_actual_is_closed(actual)?;
        return Ok(actual.clone());
    }

    let actual_shape = actual.constraint_shape();
    if !source_shape.same_header(actual_shape) {
        // Ordinary type incompatibility remains lower-owned. A source
        // whose shape does not correspond to this schema position cannot,
        // however, borrow the definition's effect-variable namespace.
        ensure_source_actual_is_closed(actual)?;
        return Ok(actual.clone());
    }

    let source_children = source_shape.children().collect::<Vec<_>>();
    let projected_children = projected_shape.children().collect::<Vec<_>>();
    let actual_children = actual_shape.children().collect::<Vec<_>>();
    if source_children.len() != projected_children.len()
        || source_children.len() != actual_children.len()
    {
        return Err(CallConstraintInvariant::PreparedEffectSourceShapeMismatch);
    }
    let children = source_children
        .into_iter()
        .zip(projected_children)
        .zip(actual_children)
        .map(|((source, projected), actual)| {
            seal_source_actual_inner(issuer, source, projected, actual)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut rebuilt = actual_shape
        .rebuild(children)
        .map_err(|_| CallConstraintInvariant::PreparedEffectSourceShapeMismatch)?;

    if let (
        TypeKind::Function {
            effects: source_effects,
            ..
        },
        TypeKind::Function {
            effects: projected_effects,
            ..
        },
        TypeKind::Function {
            effects: actual_effects,
            ..
        },
    ) = (source, projected, &mut rebuilt)
    {
        *actual_effects =
            seal_source_actual_row(issuer, source_effects, projected_effects, actual_effects)?;
    }
    Ok(rebuilt)
}

fn seal_source_actual_row(
    issuer: EffectVarIssuer,
    source: &EffectRow,
    projected: &EffectRow,
    actual: &EffectRow,
) -> Result<EffectRow, CallConstraintInvariant> {
    let projected_variable = match (source.tail(), projected.tail()) {
        (EffectRowTail::Closed, EffectRowTail::Closed) if source == projected => None,
        (EffectRowTail::Unknown, EffectRowTail::Variable(projected_variable))
            if source.concrete() == projected.concrete()
                && projected_variable.issuer() == issuer =>
        {
            Some(projected_variable)
        }
        (
            EffectRowTail::Variable(_source_variable),
            EffectRowTail::Variable(projected_variable),
        ) if source.concrete() == projected.concrete() && projected_variable.issuer() == issuer => {
            Some(projected_variable)
        }
        _ => return Err(CallConstraintInvariant::PreparedEffectInstantiationMismatch),
    };

    match actual.tail() {
        EffectRowTail::Closed => Ok(EffectRow::closed(actual.concrete().clone())),
        EffectRowTail::Unknown
            if matches!(source.tail(), EffectRowTail::Unknown) && projected_variable.is_some() =>
        {
            Ok(EffectRow::open(
                actual.concrete().clone(),
                projected_variable.expect("guarded above"),
            ))
        }
        EffectRowTail::Variable(actual_variable)
            if matches!(
                source.tail(),
                EffectRowTail::Variable(source_variable)
                    if source_variable == actual_variable
            ) && projected_variable.is_some() =>
        {
            Ok(EffectRow::open(
                actual.concrete().clone(),
                projected_variable.expect("guarded above"),
            ))
        }
        EffectRowTail::Unknown => Err(CallConstraintInvariant::PreparedEffectSourceTailMismatch),
        EffectRowTail::Variable(_) => {
            Err(CallConstraintInvariant::PreparedEffectSourceForeignVariable)
        }
    }
}

fn ensure_source_actual_is_closed(actual: &TypeKind) -> Result<(), CallConstraintInvariant> {
    let shape = actual.constraint_shape();
    if let TypeConstraintShape::Function { effects, .. } = shape {
        match effects.tail() {
            EffectRowTail::Closed => {}
            EffectRowTail::Unknown => {
                return Err(CallConstraintInvariant::PreparedEffectSourceTailMismatch);
            }
            EffectRowTail::Variable(_) => {
                return Err(CallConstraintInvariant::PreparedEffectSourceForeignVariable);
            }
        }
    }
    shape
        .children()
        .try_for_each(ensure_source_actual_is_closed)
}

impl PreparedCallableEffectInstantiationEvidence {
    pub(crate) const fn issuer(&self) -> EffectVarIssuer {
        self.issuer
    }

    pub(crate) fn matches_checked(&self, checked: &CheckedCallableEffectInstantiation) -> bool {
        self.positions.len() == checked.positions.len()
            && self.variables.len() == checked.variables.len()
            && self
                .positions
                .iter()
                .zip(&checked.positions)
                .all(|(left, right)| {
                    left.root == right.root
                        && left.path == right.path
                        && left.variable.index() == right.ordinal
                        && left.first_use == right.first_use
                })
            && self
                .variables
                .iter()
                .zip(&checked.variables)
                .all(|(left, right)| {
                    left.variable.index() == right.ordinal && left.first_use == right.first_use
                })
    }
}

impl CheckedCallableEffectInstantiation {
    pub(crate) const fn issuer(&self) -> EffectVarIssuer {
        self.issuer
    }

    pub(crate) fn variables(&self) -> impl ExactSizeIterator<Item = EffectVar> + '_ {
        self.variables
            .iter()
            .map(|variable| EffectVar::issued(self.issuer, variable.ordinal))
    }

    pub(crate) fn seal_source_actual(
        &self,
        source: &TypeKind,
        projected: &TypeKind,
        actual: &TypeKind,
    ) -> Result<TypeKind, CallConstraintInvariant> {
        seal_source_actual_inner(self.issuer, source, projected, actual)
    }

    pub(crate) fn project_parameter(
        &self,
        schema: &CallableSignatureSchema,
        coordinate: CallableParameterCoordinate,
    ) -> Result<Option<TypeKind>, CallConstraintInvariant> {
        schema
            .group(coordinate.group())
            .and_then(|group| group.parameter(coordinate.parameter()))
            .and_then(|parameter| parameter.declared_type())
            .map(|declared| {
                self.project_type(
                    CallableEffectPositionRoot::Parameter(coordinate),
                    declared,
                    &mut Vec::new(),
                )
            })
            .transpose()
    }

    pub(crate) fn project_result(
        &self,
        schema: &CallableSignatureSchema,
    ) -> Result<TypeKind, CallConstraintInvariant> {
        self.project_type(
            CallableEffectPositionRoot::Result,
            schema.result(),
            &mut Vec::new(),
        )
    }

    pub(crate) fn project_invocation_effects(
        &self,
        schema: &CallableSignatureSchema,
    ) -> Result<EffectRow, CallConstraintInvariant> {
        let position = self.position(CallableEffectPositionRoot::Invocation, &[]);
        match (schema.effects().fixed_row(), position) {
            (Some(row), None) if matches!(row.tail(), EffectRowTail::Closed) => Ok(row.clone()),
            (Some(row), Some(position)) if !matches!(row.tail(), EffectRowTail::Closed) => {
                Ok(EffectRow::open(
                    row.concrete().clone(),
                    EffectVar::issued(self.issuer, position.ordinal),
                ))
            }
            (None, Some(position)) => Ok(EffectRow::open(
                crate::effects::EffectSet::new(),
                EffectVar::issued(self.issuer, position.ordinal),
            )),
            _ => Err(CallConstraintInvariant::PreparedEffectInstantiationMismatch),
        }
    }

    fn position(
        &self,
        root: CallableEffectPositionRoot,
        path: &[CallableEffectTypeEdge],
    ) -> Option<&CheckedCallableEffectPosition> {
        self.positions
            .binary_search_by(|position| {
                (&position.root, position.path.as_ref()).cmp(&(&root, path))
            })
            .ok()
            .map(|index| &self.positions[index])
    }

    fn project_type(
        &self,
        root: CallableEffectPositionRoot,
        ty: &TypeKind,
        path: &mut Vec<CallableEffectTypeEdge>,
    ) -> Result<TypeKind, CallConstraintInvariant> {
        let shape = ty.constraint_shape();
        if matches!(shape, TypeConstraintShape::Unresolved) {
            return Err(CallConstraintInvariant::PreparedEffectInstantiationMismatch);
        }
        let children = typed_children(shape)?
            .into_iter()
            .map(|(edge, child)| {
                path.push(edge);
                let projected = self.project_type(root, child, path);
                path.pop();
                projected
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut rebuilt = shape
            .rebuild(children)
            .map_err(|_| CallConstraintInvariant::PreparedEffectInstantiationMismatch)?;
        if let TypeKind::Function { effects, .. } = &mut rebuilt {
            let position = self.position(root, path);
            match (effects.tail(), position) {
                (EffectRowTail::Closed, None) => {}
                (EffectRowTail::Unknown | EffectRowTail::Variable(_), Some(position)) => {
                    *effects = EffectRow::open(
                        effects.concrete().clone(),
                        EffectVar::issued(self.issuer, position.ordinal),
                    );
                }
                _ => return Err(CallConstraintInvariant::PreparedEffectInstantiationMismatch),
            }
        }
        Ok(rebuilt)
    }
}

impl OverlayBuilder {
    fn scan_type(
        &mut self,
        root: CallableEffectPositionRoot,
        first_use: CallableGenericFirstUse,
        ty: &TypeKind,
        path: &mut Vec<CallableEffectTypeEdge>,
    ) -> Result<(), CallConstraintInvariant> {
        let shape = ty.constraint_shape();
        if matches!(shape, TypeConstraintShape::Unresolved) {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
        if let TypeConstraintShape::Function { effects, .. } = shape {
            self.record_row(root, path, first_use, effects.tail())?;
        }
        for (edge, child) in typed_children(shape)? {
            path.push(edge);
            self.scan_type(root, first_use, child, path)?;
            path.pop();
        }
        Ok(())
    }

    fn record_row(
        &mut self,
        root: CallableEffectPositionRoot,
        path: &[CallableEffectTypeEdge],
        first_use: CallableGenericFirstUse,
        tail: EffectRowTail,
    ) -> Result<(), CallConstraintInvariant> {
        match tail {
            EffectRowTail::Closed => Ok(()),
            EffectRowTail::Unknown => {
                self.record_source(root, path, first_use, PreparedEffectSourceTail::Unknown)
            }
            EffectRowTail::Variable(variable) => self.record_source(
                root,
                path,
                first_use,
                PreparedEffectSourceTail::Existing(variable),
            ),
        }
    }

    fn record_source(
        &mut self,
        root: CallableEffectPositionRoot,
        path: &[CallableEffectTypeEdge],
        first_use: CallableGenericFirstUse,
        source: PreparedEffectSourceTail,
    ) -> Result<(), CallConstraintInvariant> {
        let variable = match source {
            PreparedEffectSourceTail::Existing(source) => {
                if let Some(variable) = self.existing.get(&source) {
                    *variable
                } else {
                    let variable = self.fresh()?;
                    self.existing.insert(source, variable);
                    variable
                }
            }
            PreparedEffectSourceTail::Unknown | PreparedEffectSourceTail::ProjectedCallable => {
                self.fresh()?
            }
        };
        self.variables
            .entry(variable)
            .and_modify(|existing| *existing = (*existing).min(first_use))
            .or_insert(first_use);
        self.positions.push(PreparedCallableEffectPosition {
            root,
            path: path.into(),
            variable,
            first_use,
            source,
        });
        Ok(())
    }

    fn fresh(&mut self) -> Result<EffectVar, CallConstraintInvariant> {
        let ordinal = self.next;
        self.next = ordinal
            .checked_add(1)
            .ok_or(CallConstraintInvariant::MalformedSchemaInventory)?;
        Ok(EffectVar::issued(self.issuer, ordinal))
    }
}

fn source_matches(source: PreparedEffectSourceTail, tail: Option<EffectRowTail>) -> bool {
    matches!(
        (source, tail),
        (
            PreparedEffectSourceTail::Unknown,
            Some(EffectRowTail::Unknown)
        ) | (
            PreparedEffectSourceTail::Existing(_),
            Some(EffectRowTail::Variable(_))
        )
    ) && match (source, tail) {
        (PreparedEffectSourceTail::Existing(expected), Some(EffectRowTail::Variable(actual))) => {
            expected == actual
        }
        _ => true,
    }
}

fn typed_children(
    shape: TypeConstraintShape<'_>,
) -> Result<Vec<(CallableEffectTypeEdge, &TypeKind)>, CallConstraintInvariant> {
    let rows = match shape {
        TypeConstraintShape::Leaf(_)
        | TypeConstraintShape::Never
        | TypeConstraintShape::Generic(_) => Vec::new(),
        TypeConstraintShape::Unresolved => {
            return Err(CallConstraintInvariant::MalformedSchemaInventory);
        }
        TypeConstraintShape::Unary { child, .. } => {
            vec![(CallableEffectTypeEdge::Unary, child)]
        }
        TypeConstraintShape::Iterator { item, .. } => {
            vec![(CallableEffectTypeEdge::IteratorItem, item)]
        }
        TypeConstraintShape::Array { item, .. } => {
            vec![(CallableEffectTypeEdge::ArrayItem, item)]
        }
        TypeConstraintShape::Ref(entity) => entity
            .value()
            .map(|value| vec![(CallableEffectTypeEdge::RefValue, value)])
            .unwrap_or_default(),
        TypeConstraintShape::Map { key, value, .. } => vec![
            (CallableEffectTypeEdge::MapKey, key),
            (CallableEffectTypeEdge::MapValue, value),
        ],
        TypeConstraintShape::Borrow { inner, .. } => {
            vec![(CallableEffectTypeEdge::BorrowInner, inner)]
        }
        TypeConstraintShape::Pair { first, second, .. } => vec![
            (CallableEffectTypeEdge::PairFirst, first),
            (CallableEffectTypeEdge::PairSecond, second),
        ],
        TypeConstraintShape::Function { params, result, .. } => {
            let mut rows = params
                .iter()
                .enumerate()
                .map(|(index, parameter)| {
                    Ok((
                        CallableEffectTypeEdge::FunctionParameter(
                            u32::try_from(index)
                                .map_err(|_| CallConstraintInvariant::MalformedSchemaInventory)?,
                        ),
                        parameter,
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?;
            rows.push((CallableEffectTypeEdge::FunctionResult, result));
            rows
        }
        TypeConstraintShape::Nominal { arguments, .. } => {
            indexed_children(arguments, CallableEffectTypeEdge::NominalArgument)?
        }
        TypeConstraintShape::Tuple(items) => {
            indexed_children(items, CallableEffectTypeEdge::TupleElement)?
        }
        TypeConstraintShape::Choice(items) => {
            indexed_children(items, CallableEffectTypeEdge::ChoiceAlternative)?
        }
    };
    Ok(rows)
}

fn indexed_children<'a>(
    children: &'a [TypeKind],
    edge: impl Fn(u32) -> CallableEffectTypeEdge,
) -> Result<Vec<(CallableEffectTypeEdge, &'a TypeKind)>, CallConstraintInvariant> {
    children
        .iter()
        .enumerate()
        .map(|(index, child)| {
            Ok((
                edge(
                    u32::try_from(index)
                        .map_err(|_| CallConstraintInvariant::MalformedSchemaInventory)?,
                ),
                child,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callable::{
        CallableArgumentPolicy, CallableEffectSchema, CallableGenericParameterIssuer,
        CallableGroupKind, CallableName, CallableParameter, CallableParameterAdmission,
        CallableParameterGroup, CallableParameterIndex, CallableParameterPassing,
        CallableParameterPresence, CallableValidator, PRODUCTION_CALLABLE_LIMITS,
        SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
    };
    use std::collections::BTreeSet;

    fn schema(parameter: TypeKind, result: TypeKind) -> CallableSignatureSchema {
        let limits = PRODUCTION_CALLABLE_LIMITS;
        let parameter = CallableParameter::try_new(
            CallableParameterIndex::try_from_usize(0).expect("parameter"),
            Some(CallableName::try_new("callback").expect("name")),
            CallableParameterAdmission::checked(parameter),
            CallableParameterPassing::PositionalOnly,
            CallableParameterPresence::Required,
            None,
            None,
        )
        .expect("parameter");
        let group = CallableParameterGroup::try_new(
            CallableGroupIndex::ZERO,
            CallableGroupKind::Initial,
            vec![parameter],
            &limits,
        )
        .expect("group");
        CallableSignatureSchema::try_new(
            vec![group],
            result,
            CallableEffectSchema::fixed(EffectRow::unknown()),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::Reject,
                SpreadArgumentPolicy::Reject,
            ),
            CallableValidator::Ordinary,
            CallableGenericParameterIssuer::empty(),
            &limits,
        )
        .expect("schema")
    }

    #[test]
    fn overlay_projects_all_function_positions_without_rewriting_schema() {
        let parameter =
            TypeKind::function_with_effects([TypeKind::I32], TypeKind::I32, EffectRow::unknown());
        let result = TypeKind::function_with_effects(
            [TypeKind::String],
            TypeKind::Unit,
            EffectRow::unknown(),
        );
        let schema = schema(parameter, result);
        let overlay = PreparedCallableEffectInstantiation::seal(&schema).expect("overlay");

        assert_eq!(overlay.variables().len(), 3);
        assert!(matches!(
            schema.effects().fixed_row().expect("fixed").tail(),
            EffectRowTail::Unknown
        ));
        assert!(matches!(
            schema
                .groups()[0]
                .parameters()[0]
                .declared_type()
                .expect("declared"),
            TypeKind::Function { effects, .. }
                if matches!(effects.tail(), EffectRowTail::Unknown)
        ));
        let projected_parameter = overlay
            .project_parameter(
                &schema,
                CallableParameterCoordinate::new(
                    CallableGroupIndex::ZERO,
                    CallableParameterIndex::try_from_usize(0).expect("parameter"),
                ),
            )
            .expect("projection")
            .expect("checked parameter");
        let projected_result = overlay.project_result(&schema).expect("projection");
        let invocation = overlay
            .project_invocation_effects(&schema)
            .expect("invocation");
        let tails = [projected_parameter, projected_result]
            .into_iter()
            .map(|ty| match ty {
                TypeKind::Function { effects, .. } => effects.tail(),
                _ => panic!("function projection"),
            })
            .chain([invocation.tail()])
            .collect::<Vec<_>>();
        assert!(tails.iter().all(|tail| matches!(
            tail,
            EffectRowTail::Variable(variable) if variable.issuer() == overlay.issuer()
        )));
        let ordinals = tails
            .into_iter()
            .map(|tail| match tail {
                EffectRowTail::Variable(variable) => variable.index(),
                _ => unreachable!(),
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(ordinals, BTreeSet::from([0, 1, 2]));
    }

    #[test]
    fn overlay_preserves_shared_source_variable_identity() {
        let source = EffectVar::from_index(9);
        let higher_order = || {
            TypeKind::function_with_effects(
                [TypeKind::I32],
                TypeKind::I32,
                EffectRow::open(crate::effects::EffectSet::new(), source),
            )
        };
        let schema = schema(higher_order(), higher_order());
        let overlay = PreparedCallableEffectInstantiation::seal(&schema).expect("overlay");
        let parameter = overlay
            .project_parameter(
                &schema,
                CallableParameterCoordinate::new(
                    CallableGroupIndex::ZERO,
                    CallableParameterIndex::try_from_usize(0).expect("parameter"),
                ),
            )
            .expect("projection")
            .expect("parameter");
        let result = overlay.project_result(&schema).expect("result");
        let tail = |ty: TypeKind| match ty {
            TypeKind::Function { effects, .. } => effects.tail(),
            _ => panic!("function"),
        };
        assert_eq!(tail(parameter), tail(result));
    }

    fn projected_parameter(
        schema: &CallableSignatureSchema,
        overlay: &PreparedCallableEffectInstantiation,
    ) -> TypeKind {
        overlay
            .project_parameter(
                schema,
                CallableParameterCoordinate::new(
                    CallableGroupIndex::ZERO,
                    CallableParameterIndex::try_from_usize(0).expect("parameter"),
                ),
            )
            .expect("projection")
            .expect("parameter")
    }

    #[test]
    fn source_projection_preserves_closed_actual_rows() {
        let source = TypeKind::Tuple(vec![
            TypeKind::function([TypeKind::I32], TypeKind::I32),
            TypeKind::function([TypeKind::I32], TypeKind::I32),
        ]);
        let schema = schema(source.clone(), TypeKind::Unit);
        let overlay = PreparedCallableEffectInstantiation::seal(&schema).expect("overlay");
        let projected = projected_parameter(&schema, &overlay);
        let closed =
            EffectRow::closed(crate::effects::EffectSet::from_labels(["fs.read"]).expect("effect"));
        let actual = TypeKind::Tuple(vec![
            TypeKind::function([TypeKind::I32], TypeKind::I32),
            TypeKind::function_with_effects([TypeKind::I32], TypeKind::I32, closed.clone()),
        ]);

        let sealed = overlay
            .seal_source_actual(&source, &projected, &actual)
            .expect("owned projection");
        let TypeKind::Tuple(items) = sealed else {
            panic!("tuple")
        };
        assert!(matches!(
            &items[0],
            TypeKind::Function { effects, .. }
                if matches!(effects.tail(), EffectRowTail::Variable(variable)
                    if variable.issuer() == overlay.issuer())
        ));
        assert!(matches!(
            &items[1],
            TypeKind::Function { effects, .. } if effects == &closed
        ));
    }

    #[test]
    fn source_projection_rejects_unknown_at_another_typed_path() {
        let source = TypeKind::Tuple(vec![
            TypeKind::function([TypeKind::I32], TypeKind::I32),
            TypeKind::I32,
        ]);
        let schema = schema(source.clone(), TypeKind::Unit);
        let overlay = PreparedCallableEffectInstantiation::seal(&schema).expect("overlay");
        let projected = projected_parameter(&schema, &overlay);
        let tampered = TypeKind::Tuple(vec![
            TypeKind::I32,
            TypeKind::function([TypeKind::I32], TypeKind::I32),
        ]);

        assert_eq!(
            overlay.seal_source_actual(&source, &projected, &tampered),
            Err(CallConstraintInvariant::PreparedEffectSourceTailMismatch)
        );
    }

    #[test]
    fn source_projection_rejects_a_foreign_effect_variable() {
        let source = TypeKind::function([TypeKind::I32], TypeKind::I32);
        let schema = schema(source.clone(), TypeKind::Unit);
        let overlay = PreparedCallableEffectInstantiation::seal(&schema).expect("overlay");
        let projected = projected_parameter(&schema, &overlay);
        let foreign = EffectVar::issued(
            EffectVarIssuer::fresh_prepared().expect("foreign issuer"),
            0,
        );
        let actual = TypeKind::function_with_effects(
            [TypeKind::I32],
            TypeKind::I32,
            EffectRow::open(crate::effects::EffectSet::new(), foreign),
        );

        assert_eq!(
            overlay.seal_source_actual(&source, &projected, &actual),
            Err(CallConstraintInvariant::PreparedEffectSourceForeignVariable)
        );
    }

    #[test]
    fn source_projection_rejects_unknown_against_a_closed_schema_position() {
        let source = TypeKind::function_with_effects(
            [TypeKind::I32],
            TypeKind::I32,
            EffectRow::closed(crate::effects::EffectSet::new()),
        );
        let schema = schema(source.clone(), TypeKind::Unit);
        let overlay = PreparedCallableEffectInstantiation::seal(&schema).expect("overlay");
        let projected = projected_parameter(&schema, &overlay);
        let actual = TypeKind::function([TypeKind::I32], TypeKind::I32);

        assert_eq!(
            overlay.seal_source_actual(&source, &projected, &actual),
            Err(CallConstraintInvariant::PreparedEffectSourceTailMismatch)
        );
    }
}
