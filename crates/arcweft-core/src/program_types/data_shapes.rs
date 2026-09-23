//! Borrowed per-occurrence codec views over the original executable type graph.
//!
//! The transient index contains references to policy nodes and existing type
//! coordinates only. It owns no descriptor, type identity, or nominal graph.

use std::{borrow::Cow, cell::OnceCell, collections::BTreeSet, num::NonZeroU32};

use arcweft_data::{ShapeAccess, ShapeId, ShapeRef, TypeShape};
use thiserror::Error;

use super::{RuntimeProgramTypeError, RuntimeProgramTypes};
use crate::{
    awbc::schema::{AwbcRuntimeTypeShape as AwbcType, AwbcVariantIdentity},
    entry::RuntimeSchemaLimits,
    entry::schema::{RuntimeCodecUse, RuntimeCodecUseError, RuntimeFieldCodecUse},
    pattern::{RuntimeBuiltinVariantIdentity, RuntimeSemanticTypeId},
    plan::RuntimePlanTypeProjection as PlanType,
    runtime_id::RuntimePlanTypeId,
};

mod defaults;
pub use defaults::{RuntimeDataFieldDefaultError, RuntimeDataFieldDefaultRequest};

#[derive(Clone, Debug)]
pub struct RuntimeProgramDataShapes<'a> {
    types: RuntimeProgramTypes<'a>,
    occurrences: OnceCell<Vec<Occurrence<'a>>>,
}

#[derive(Clone, Copy, Debug)]
enum Policy<'a> {
    Root,
    Use(&'a RuntimeCodecUse),
    BuiltinPayload(&'a RuntimeCodecUse),
}

#[derive(Clone, Debug)]
struct Occurrence<'a> {
    row: usize,
    policy: Policy<'a>,
    children: Vec<ShapeId>,
    error: Option<RuntimeProgramDataShapeError>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RuntimeProgramDataShapeError {
    #[error(transparent)]
    ProgramType(#[from] RuntimeProgramTypeError),
    #[error(transparent)]
    FieldDefault(#[from] RuntimeDataFieldDefaultError),
    #[error("selected program has no shape coordinate {index}")]
    MissingCoordinate { index: usize },
    #[error("selected type {semantic_type:?} is missing its source-owned {role} codec descriptor")]
    MissingCodecMetadata {
        semantic_type: RuntimeSemanticTypeId,
        role: &'static str,
    },
    #[error("codec-use topology does not match selected type {semantic_type:?}")]
    PolicyMismatch {
        semantic_type: RuntimeSemanticTypeId,
    },
    #[error("selected type {semantic_type:?} has invalid codec metadata: {source}")]
    CodecUse {
        semantic_type: RuntimeSemanticTypeId,
        source: RuntimeCodecUseError,
    },
    #[error("selected type {semantic_type:?} has no data codec representation")]
    NoDataRepresentation {
        semantic_type: RuntimeSemanticTypeId,
    },
    #[error("selected data shape traversal exceeds {limit}")]
    Limit { limit: &'static str },
}

impl<'a> RuntimeProgramDataShapes<'a> {
    /// The sole source policy proof that this occurrence has a transparent
    /// one-slot wire wrapper. Nominal back-edge aliases are not wrappers.
    pub fn transparent_child(
        &self,
        id: ShapeId,
    ) -> Result<Option<ShapeId>, RuntimeProgramDataShapeError> {
        let occurrence = self
            .index()?
            .get(id.index())
            .ok_or(RuntimeProgramDataShapeError::MissingCoordinate { index: id.index() })?;
        if let Some(error) = &occurrence.error {
            return Err(error.clone());
        }
        if !matches!(
            self.policy(occurrence),
            Some(RuntimeCodecUse::Newtype { .. })
        ) {
            return Ok(None);
        }
        let [child] = occurrence.children.as_slice() else {
            return Err(RuntimeProgramDataShapeError::MissingCoordinate { index: id.index() });
        };
        Ok(Some(*child))
    }

    #[must_use]
    pub const fn new(types: RuntimeProgramTypes<'a>) -> Self {
        Self {
            types,
            occurrences: OnceCell::new(),
        }
    }

    /// Validates all retained policy occurrences without requiring an external
    /// format representation. Verifiers use this before publishing a program.
    pub fn validate_codec_uses(
        &self,
        limits: RuntimeSchemaLimits,
    ) -> Result<(), RuntimeProgramDataShapeError> {
        let occurrences = self.index_with_limits(limits)?;
        if !limits.permits_nodes(occurrences.len()) {
            return Err(RuntimeProgramDataShapeError::Limit { limit: "max_nodes" });
        }
        for (index, occurrence) in occurrences.iter().enumerate() {
            if let Some(error) = &occurrence.error {
                return Err(error.clone());
            }
            if let Some(RuntimeCodecUse::Record { fields, .. }) = self.policy(occurrence) {
                for field in 0..fields.len() {
                    self.field_default_request(ShapeId::new(index), field)?;
                }
            }
        }
        Ok(())
    }

    pub fn root(
        &self,
        semantic_type: RuntimeSemanticTypeId,
        limits: RuntimeSchemaLimits,
    ) -> Result<ShapeRef<'a>, RuntimeProgramDataShapeError> {
        self.types.require_type(semantic_type)?;
        self.validate_codec_uses(limits)?;
        let row = match self.types {
            RuntimeProgramTypes::Plan(plan) => plan
                .type_table()
                .id_for_semantic(semantic_type)
                .map(plan_row)
                .ok_or(RuntimeProgramTypeError::Missing { semantic_type })?,
            RuntimeProgramTypes::Awbc(_) => self.types.awbc_type(semantic_type)?.index(),
        };
        let root = ShapeId::new(row);
        let mut visited = BTreeSet::new();
        let mut pending = vec![(root, 0_usize)];
        while let Some((id, depth)) = pending.pop() {
            if !visited.insert(id) {
                continue;
            }
            if !limits.permits_nodes(visited.len()) {
                return Err(RuntimeProgramDataShapeError::Limit { limit: "max_nodes" });
            }
            if !limits.permits_depth(depth) {
                return Err(RuntimeProgramDataShapeError::Limit { limit: "max_depth" });
            }
            let shape = self.project(id)?;
            let mut children = Vec::new();
            collect_references(&shape, &mut children);
            if !limits.permits_sequence_items(children.len()) {
                return Err(RuntimeProgramDataShapeError::Limit {
                    limit: "max_sequence_items",
                });
            }
            pending.extend(children.into_iter().map(|child| (child, depth + 1)));
        }
        Ok(ShapeRef::Id(root))
    }

    /// IDs select occurrences, so two IDs may have the same logical source type
    /// and different source-declared wire policies.
    #[must_use]
    pub fn semantic_type(&self, id: ShapeId) -> Option<RuntimeSemanticTypeId> {
        self.row_semantic(self.index().ok()?.get(id.index())?.row)
    }

    fn row_semantic(&self, row: usize) -> Option<RuntimeSemanticTypeId> {
        match self.types {
            RuntimeProgramTypes::Plan(plan) => plan
                .type_table()
                .get(decode_plan_row(row)?)
                .map(crate::plan::RuntimePlanTypeDeclaration::semantic_identity),
            RuntimeProgramTypes::Awbc(program) => program
                .runtime_types
                .get(row)
                .map(crate::awbc::schema::AwbcRuntimeType::semantic_identity),
        }
    }

    fn root_policy(&self, row: usize) -> Option<&'a RuntimeCodecUse> {
        match self.types {
            RuntimeProgramTypes::Plan(plan) => {
                let ty = decode_plan_row(row)?;
                if let Some(domain) = plan.nominal_record_domains().get(ty) {
                    return domain.data_codec().map(|codec| &codec.body);
                }
                if let Some(domain) = plan.variant_domains().get(ty) {
                    return domain.data_codec().map(|codec| &codec.body);
                }
                plan.type_table().get(ty)?.data_codec()
            }
            RuntimeProgramTypes::Awbc(program) => program.runtime_types.get(row)?.data_codec(),
        }
    }

    fn policy(&self, occurrence: &Occurrence<'a>) -> Option<&'a RuntimeCodecUse> {
        match occurrence.policy {
            Policy::Root => self.root_policy(occurrence.row),
            Policy::Use(policy) => Some(policy),
            Policy::BuiltinPayload(_) => None,
        }
    }

    fn row_count(&self) -> usize {
        match self.types {
            RuntimeProgramTypes::Plan(plan) => plan.type_table().len(),
            RuntimeProgramTypes::Awbc(program) => program.runtime_types.len(),
        }
    }

    fn index(&self) -> Result<&[Occurrence<'a>], RuntimeProgramDataShapeError> {
        match self.occurrences.get() {
            Some(rows) => Ok(rows),
            None => self.index_with_limits(RuntimeSchemaLimits::engine_default()),
        }
    }

    fn index_with_limits(
        &self,
        limits: RuntimeSchemaLimits,
    ) -> Result<&[Occurrence<'a>], RuntimeProgramDataShapeError> {
        let count = self.row_count();
        let node_limit = || RuntimeProgramDataShapeError::Limit { limit: "max_nodes" };
        if !limits.permits_nodes(count) {
            return Err(node_limit());
        }
        // Selected source proofs already passed this boundary. AWBC verification
        // also accepts directly constructed candidates, so it must preflight
        // policy sizes before allocating the borrowed occurrence index.
        let mut work = crate::entry::schema::value_budget::ValidationWork::new(limits);
        for row in 0..count {
            let semantic_type = self
                .row_semantic(row)
                .ok_or(RuntimeProgramDataShapeError::MissingCoordinate { index: row })?;
            let check =
                |policy: &RuntimeCodecUse,
                 work: &mut crate::entry::schema::value_budget::ValidationWork| {
                    policy.validate_limits(limits).map_err(|source| {
                        RuntimeProgramDataShapeError::CodecUse {
                            semantic_type,
                            source,
                        }
                    })?;
                    policy
                        .walk(|_, depth| work.charge(depth))
                        .map_err(|source| RuntimeProgramDataShapeError::CodecUse {
                            semantic_type,
                            source: source.into(),
                        })
                };
            if let Some(policy) = self.root_policy(row) {
                check(policy, &mut work)?;
            }
            for (_, policy) in self.argument_uses(row)? {
                check(policy, &mut work)?;
            }
        }
        if let Some(rows) = self.occurrences.get() {
            return if limits.permits_nodes(rows.len()) {
                Ok(rows)
            } else {
                Err(node_limit())
            };
        }
        let rows = {
            let mut rows = (0..count)
                .map(|row| Occurrence {
                    row,
                    policy: Policy::Root,
                    children: vec![],
                    error: None,
                })
                .collect::<Vec<_>>();
            for row in 0..count {
                match self.argument_uses(row) {
                    Ok(arguments) => {
                        if !limits.permits_nodes(rows.len().saturating_add(arguments.len())) {
                            return Err(node_limit());
                        }
                        rows.extend(arguments.into_iter().map(|(row, codec)| Occurrence {
                            row,
                            policy: Policy::Use(codec),
                            children: vec![],
                            error: None,
                        }))
                    }
                    Err(error) => rows[row].error = Some(error),
                }
            }
            let mut cursor = 0;
            while cursor < rows.len() {
                match self.child_uses(&rows[cursor]) {
                    Ok(children) => {
                        let mut ids = Vec::with_capacity(children.len());
                        for (row, policy) in children {
                            if matches!(policy, Policy::Root) {
                                ids.push(ShapeId::new(row));
                            } else {
                                if !limits.permits_nodes(rows.len().saturating_add(1)) {
                                    return Err(node_limit());
                                }
                                ids.push(ShapeId::new(rows.len()));
                                rows.push(Occurrence {
                                    row,
                                    policy,
                                    children: vec![],
                                    error: None,
                                });
                            }
                        }
                        rows[cursor].children = ids;
                    }
                    Err(error) => rows[cursor].error = Some(error),
                }
                cursor += 1;
            }
            rows
        };
        // OnceCell is local to this borrowed view; initialization cannot race.
        let _ = self.occurrences.set(rows);
        Ok(self
            .occurrences
            .get()
            .expect("the occurrence index was initialized"))
    }

    fn argument_uses(
        &self,
        row: usize,
    ) -> Result<Vec<(usize, &'a RuntimeCodecUse)>, RuntimeProgramDataShapeError> {
        let semantic_type = self
            .row_semantic(row)
            .ok_or(RuntimeProgramDataShapeError::MissingCoordinate { index: row })?;
        let mismatch = || RuntimeProgramDataShapeError::PolicyMismatch { semantic_type };
        let (arguments, policies): (Vec<usize>, Option<&'a [RuntimeCodecUse]>) = match self.types {
            RuntimeProgramTypes::Plan(plan) => {
                let ty = decode_plan_row(row).ok_or_else(mismatch)?;
                let declaration = plan.type_table().get(ty).ok_or_else(mismatch)?;
                let PlanType::Nominal { arguments, .. } = declaration.projection() else {
                    return Ok(vec![]);
                };
                let codec = if let Some(record) = plan.nominal_record_domains().get(ty) {
                    record.data_codec()
                } else {
                    plan.variant_domains()
                        .get(ty)
                        .and_then(crate::plan::RuntimeVariantDomain::data_codec)
                };
                (
                    arguments.iter().copied().map(plan_row).collect(),
                    codec.map(|codec| codec.arguments.as_ref()),
                )
            }
            RuntimeProgramTypes::Awbc(program) => {
                let declaration = &program.runtime_types[row];
                let arguments = match declaration.shape() {
                    AwbcType::Nominal { arguments, .. }
                    | AwbcType::NominalRecord { arguments, .. }
                    | AwbcType::Variant {
                        owner: AwbcVariantIdentity::Nominal { .. },
                        arguments,
                        ..
                    } => arguments,
                    _ => {
                        return if declaration.data_codec_arguments().is_none() {
                            Ok(vec![])
                        } else {
                            Err(mismatch())
                        };
                    }
                };
                (
                    arguments.iter().map(|ty| ty.index()).collect(),
                    declaration.data_codec_arguments(),
                )
            }
        };
        let Some(policies) = policies else {
            return Ok(vec![]);
        };
        if arguments.len() != policies.len() {
            return Err(mismatch());
        }
        Ok(arguments.into_iter().zip(policies.iter()).collect())
    }

    fn logical_children(&self, row: usize) -> Vec<usize> {
        match self.types {
            RuntimeProgramTypes::Plan(plan) => {
                let Some(ty) = decode_plan_row(row) else {
                    return vec![];
                };
                if let Some(record) = plan.nominal_record_domains().get(ty) {
                    return record
                        .fields()
                        .iter()
                        .map(|field| plan_row(field.ty()))
                        .collect();
                }
                if let Some(variant) = plan.variant_domains().get(ty) {
                    return variant
                        .cases()
                        .iter()
                        .filter_map(|case| case.payload())
                        .map(plan_row)
                        .collect();
                }
                let Some(declaration) = plan.type_table().get(ty) else {
                    return vec![];
                };
                match declaration.projection() {
                    PlanType::Option { item, .. } => vec![plan_row(*item)],
                    PlanType::Result {
                        value_payload,
                        error_payload,
                        ..
                    } => vec![plan_row(*value_payload), plan_row(*error_payload)],
                    PlanType::Record(fields) => {
                        fields.iter().map(|field| plan_row(*field.ty())).collect()
                    }
                    projection => projection
                        .children()
                        .into_iter()
                        .copied()
                        .map(plan_row)
                        .collect(),
                }
            }
            RuntimeProgramTypes::Awbc(program) => {
                let Some(row) = program.runtime_types.get(row) else {
                    return vec![];
                };
                match row.shape() {
                    AwbcType::Tuple(items) | AwbcType::Choice(items) => {
                        items.iter().map(|item| item.index()).collect()
                    }
                    AwbcType::Sequence(item) | AwbcType::Array { item, .. } => vec![item.index()],
                    AwbcType::Map { key, value, .. } => vec![key.index(), value.index()],
                    AwbcType::Record { fields, .. } | AwbcType::NominalRecord { fields, .. } => {
                        fields.iter().map(|field| field.ty.index()).collect()
                    }
                    AwbcType::Variant {
                        owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
                        cases,
                        ..
                    } => cases
                        .first()
                        .and_then(|case| case.payload)
                        .and_then(|payload| program.runtime_types.get(payload.index()))
                        .and_then(|row| match row.shape() {
                            AwbcType::Tuple(items) if items.len() == 1 => {
                                Some(vec![items[0].index()])
                            }
                            _ => None,
                        })
                        .unwrap_or_default(),
                    AwbcType::Variant { cases, .. } => cases
                        .iter()
                        .filter_map(|case| case.payload)
                        .map(|ty| ty.index())
                        .collect(),
                    AwbcType::Opaque { arguments, .. } | AwbcType::Nominal { arguments, .. } => {
                        arguments.iter().map(|ty| ty.index()).collect()
                    }
                    _ => vec![],
                }
            }
        }
    }

    fn is_nominal(&self, row: usize) -> bool {
        match self.types {
            RuntimeProgramTypes::Plan(plan) => decode_plan_row(row)
                .and_then(|ty| plan.type_table().get(ty))
                .is_some_and(|row| matches!(row.projection(), PlanType::Nominal { .. })),
            RuntimeProgramTypes::Awbc(program) => {
                program.runtime_types.get(row).is_some_and(|row| {
                    matches!(
                        row.shape(),
                        AwbcType::NominalRecord { .. }
                            | AwbcType::Nominal { .. }
                            | AwbcType::Variant {
                                owner: AwbcVariantIdentity::Nominal { .. },
                                ..
                            }
                    )
                })
            }
        }
    }

    fn builtin(&self, row: usize) -> Option<RuntimeBuiltinVariantIdentity> {
        match self.types {
            RuntimeProgramTypes::Plan(plan) => {
                match plan.type_table().get(decode_plan_row(row)?)?.projection() {
                    PlanType::Option { .. } => Some(RuntimeBuiltinVariantIdentity::Option),
                    PlanType::Result { .. } => Some(RuntimeBuiltinVariantIdentity::Result),
                    PlanType::BuiltinVariant { owner, .. } => Some(*owner),
                    _ => None,
                }
            }
            RuntimeProgramTypes::Awbc(program) => match program.runtime_types.get(row)?.shape() {
                AwbcType::Variant {
                    owner: AwbcVariantIdentity::Builtin(owner),
                    ..
                } => Some(*owner),
                _ => None,
            },
        }
    }

    fn child_uses(
        &self,
        occurrence: &Occurrence<'a>,
    ) -> Result<Vec<(usize, Policy<'a>)>, RuntimeProgramDataShapeError> {
        let semantic_type = self.row_semantic(occurrence.row).ok_or(
            RuntimeProgramDataShapeError::MissingCoordinate {
                index: occurrence.row,
            },
        )?;
        let mismatch = || RuntimeProgramDataShapeError::PolicyMismatch { semantic_type };
        let children = self.logical_children(occurrence.row);
        if let Policy::BuiltinPayload(policy) = occurrence.policy {
            let tuple = match self.types {
                RuntimeProgramTypes::Plan(plan) => decode_plan_row(occurrence.row).and_then(|ty| plan.type_table().get(ty)).is_some_and(|row| matches!(row.projection(), PlanType::Tuple(items) if items.len() == 1)),
                RuntimeProgramTypes::Awbc(program) => program.runtime_types.get(occurrence.row).is_some_and(|row| matches!(row.shape(), AwbcType::Tuple(items) if items.len() == 1)),
            };
            return if tuple && children.len() == 1 {
                Ok(vec![(children[0], Policy::Use(policy))])
            } else {
                Err(mismatch())
            };
        }
        let Some(policy) = self.policy(occurrence) else {
            return Ok(children
                .into_iter()
                .map(|row| (row, Policy::Root))
                .collect());
        };
        if !self.policy_matches(occurrence.row, policy) {
            return Err(mismatch());
        }
        if matches!(policy, RuntimeCodecUse::NominalRef) {
            return if self.is_nominal(occurrence.row) && !matches!(occurrence.policy, Policy::Root)
            {
                Ok(vec![])
            } else {
                Err(mismatch())
            };
        }
        let policies = policy.children();
        if children.len() != policies.len() {
            return Err(mismatch());
        }
        let tuple_payload = matches!(policy, RuntimeCodecUse::Builtin { .. })
            && self
                .builtin(occurrence.row)
                .is_some_and(|owner| owner != RuntimeBuiltinVariantIdentity::Option);
        Ok(children
            .into_iter()
            .zip(policies)
            .map(|(row, policy)| {
                (
                    row,
                    if tuple_payload {
                        Policy::BuiltinPayload(policy)
                    } else {
                        Policy::Use(policy)
                    },
                )
            })
            .collect())
    }

    fn policy_matches(&self, row: usize, policy: &RuntimeCodecUse) -> bool {
        use crate::entry::RuntimeNominalRecordShape as RecordShape;
        use RuntimeCodecUse as Use;
        let variant_cases = |cases: &[crate::plan::RuntimeVariantCase]| {
            let Use::Enum {
                cases: policies, ..
            } = policy
            else {
                return false;
            };
            cases.len() == policies.len()
                && cases
                    .iter()
                    .zip(policies)
                    .all(|(case, policy)| case.payload().is_some() == policy.payload.is_some())
        };
        match self.types {
            RuntimeProgramTypes::Plan(plan) => {
                let Some(ty) = decode_plan_row(row) else {
                    return false;
                };
                if let Some(record) = plan.nominal_record_domains().get(ty) {
                    return matches!(
                        (record.shape(), policy),
                        (_, Use::NominalRef)
                            | (RecordShape::Unit, Use::Plain)
                            | (RecordShape::Newtype, Use::Newtype { .. })
                            | (RecordShape::Tuple, Use::Tuple { .. })
                            | (RecordShape::Record, Use::Record { .. })
                    );
                }
                if let Some(variant) = plan.variant_domains().get(ty) {
                    return matches!(policy, Use::NominalRef) || variant_cases(variant.cases());
                }
                let Some(row) = plan.type_table().get(ty) else {
                    return false;
                };
                if matches!((row.projection(), policy), (PlanType::Tuple(items), Use::Newtype { .. }) if items.len() == 1)
                {
                    return true;
                }
                matches!(
                    (row.projection(), policy),
                    (
                        PlanType::Unit
                            | PlanType::Bool
                            | PlanType::Signed(_)
                            | PlanType::Unsigned(_)
                            | PlanType::F32
                            | PlanType::F64
                            | PlanType::String
                            | PlanType::Char
                            | PlanType::Never
                            | PlanType::Duration
                            | PlanType::Progress
                            | PlanType::EntityReference
                            | PlanType::AgentValue,
                        Use::Plain
                    ) | (PlanType::Bytes, Use::Bytes { .. })
                        | (
                            PlanType::Sequence { .. } | PlanType::Array { .. },
                            Use::Unary { .. }
                        )
                        | (PlanType::Tuple(_), Use::Tuple { .. })
                        | (PlanType::Map { .. }, Use::Map { .. })
                        | (
                            PlanType::Record(_),
                            Use::Record { .. } | Use::RecordFields { .. }
                        )
                        | (
                            PlanType::Option { .. }
                                | PlanType::Result { .. }
                                | PlanType::BuiltinVariant { .. },
                            Use::Builtin { .. }
                        )
                        | (PlanType::Choice(_), Use::Choice { .. })
                        | (PlanType::Opaque { .. }, Use::Opaque { .. })
                        | (PlanType::Nominal { .. }, Use::NominalRef)
                )
            }
            RuntimeProgramTypes::Awbc(program) => {
                let Some(row) = program.runtime_types.get(row) else {
                    return false;
                };
                match (row.shape(), policy) {
                    (AwbcType::Tuple(items), Use::Newtype { .. }) if items.len() == 1 => true,
                    (AwbcType::NominalRecord { shape, .. }, policy) => matches!(
                        (*shape, policy),
                        (_, Use::NominalRef)
                            | (RecordShape::Unit, Use::Plain)
                            | (RecordShape::Newtype, Use::Newtype { .. })
                            | (RecordShape::Tuple, Use::Tuple { .. })
                            | (RecordShape::Record, Use::Record { .. })
                    ),
                    (
                        AwbcType::Variant {
                            owner: AwbcVariantIdentity::Nominal { .. },
                            cases,
                            ..
                        },
                        Use::Enum {
                            cases: policies, ..
                        },
                    ) => {
                        cases.len() == policies.len()
                            && cases.iter().zip(policies).all(|(case, policy)| {
                                case.payload.is_some() == policy.payload.is_some()
                            })
                    }
                    (
                        AwbcType::Variant {
                            owner: AwbcVariantIdentity::Nominal { .. },
                            ..
                        }
                        | AwbcType::Nominal { .. },
                        Use::NominalRef,
                    ) => true,
                    (
                        AwbcType::Unit
                        | AwbcType::Bool
                        | AwbcType::Int(_)
                        | AwbcType::UInt(_)
                        | AwbcType::F32
                        | AwbcType::F64
                        | AwbcType::String
                        | AwbcType::Char
                        | AwbcType::Never
                        | AwbcType::Duration
                        | AwbcType::Progress
                        | AwbcType::EntityRef
                        | AwbcType::AgentValue,
                        Use::Plain,
                    )
                    | (AwbcType::Bytes, Use::Bytes { .. })
                    | (AwbcType::Sequence(_) | AwbcType::Array { .. }, Use::Unary { .. })
                    | (AwbcType::Tuple(_), Use::Tuple { .. })
                    | (AwbcType::Map { .. }, Use::Map { .. })
                    | (AwbcType::Record { .. }, Use::Record { .. } | Use::RecordFields { .. })
                    | (
                        AwbcType::Variant {
                            owner: AwbcVariantIdentity::Builtin(_),
                            ..
                        },
                        Use::Builtin { .. },
                    )
                    | (AwbcType::Choice(_), Use::Choice { .. })
                    | (AwbcType::Opaque { .. }, Use::Opaque { .. }) => true,
                    _ => false,
                }
            }
        }
    }

    fn project(&self, id: ShapeId) -> Result<TypeShape, RuntimeProgramDataShapeError> {
        let occurrence = self
            .index()?
            .get(id.index())
            .ok_or(RuntimeProgramDataShapeError::MissingCoordinate { index: id.index() })?;
        if let Some(error) = &occurrence.error {
            return Err(error.clone());
        }
        let semantic_type = self.row_semantic(occurrence.row).ok_or(
            RuntimeProgramDataShapeError::MissingCoordinate {
                index: occurrence.row,
            },
        )?;
        let missing = |role| RuntimeProgramDataShapeError::MissingCodecMetadata {
            semantic_type,
            role,
        };
        let unsupported = || RuntimeProgramDataShapeError::NoDataRepresentation { semantic_type };
        let policy = self.policy(occurrence);
        let children = &occurrence.children;
        let child = |index| {
            children
                .get(index)
                .copied()
                .map(TypeShape::Ref)
                .ok_or(RuntimeProgramDataShapeError::PolicyMismatch { semantic_type })
        };
        if matches!(policy, Some(RuntimeCodecUse::NominalRef)) {
            return Ok(TypeShape::Ref(ShapeId::new(occurrence.row)));
        }
        if matches!(occurrence.policy, Policy::BuiltinPayload(_)) {
            return Ok(TypeShape::tuple([child(0)?]));
        }
        if matches!(policy, Some(RuntimeCodecUse::Newtype { .. })) {
            return child(0);
        }
        if let Some(owner) = self.builtin(occurrence.row)
            && owner != RuntimeBuiltinVariantIdentity::Option
        {
            let mut payload = 0;
            let variants = owner
                .cases()
                .iter()
                .map(|case| {
                    let shape = if case.has_payload() {
                        let shape = child(payload)?;
                        payload += 1;
                        Some(shape)
                    } else {
                        None
                    };
                    Ok(arcweft_data::VariantShape {
                        rust_name: case.name().to_owned(),
                        wire_name: case.name().to_owned(),
                        payload: shape,
                        discriminant: None,
                    })
                })
                .collect::<Result<_, RuntimeProgramDataShapeError>>()?;
            return Ok(TypeShape::Enum {
                name: owner.codec_name().to_owned(),
                variants,
                tag: arcweft_data::EnumTagStyle::External,
                repr: None,
            });
        }
        if let Some(RuntimeCodecUse::Record {
            name,
            deny_unknown_fields,
            fields,
        }) = policy
        {
            let names = self
                .record_names(occurrence.row)
                .ok_or_else(|| missing("record fields"))?;
            let fields = fields
                .iter()
                .zip(names)
                .enumerate()
                .map(|(index, (field, rust_name))| Ok(field_shape(rust_name, field, child(index)?)))
                .collect::<Result<_, RuntimeProgramDataShapeError>>()?;
            return Ok(TypeShape::Record {
                name: name.clone(),
                fields,
                policy: arcweft_data::RecordPolicy {
                    deny_unknown_fields: *deny_unknown_fields,
                },
            });
        }
        if let Some(RuntimeCodecUse::Enum {
            name,
            tag,
            repr,
            cases,
        }) = policy
        {
            let names = self
                .case_names(occurrence.row)
                .ok_or_else(|| missing("variant cases"))?;
            if names.len() != cases.len() {
                return Err(RuntimeProgramDataShapeError::PolicyMismatch { semantic_type });
            }
            let mut payload = 0;
            let variants = cases
                .iter()
                .zip(names)
                .map(|(case, rust_name)| {
                    let shape = if case.payload.is_some() {
                        let shape = child(payload)?;
                        payload += 1;
                        Some(shape)
                    } else {
                        None
                    };
                    Ok(arcweft_data::VariantShape {
                        rust_name,
                        wire_name: case.wire_name.clone(),
                        discriminant: case.discriminant,
                        payload: shape,
                    })
                })
                .collect::<Result<_, RuntimeProgramDataShapeError>>()?;
            return Ok(TypeShape::Enum {
                name: name.clone(),
                variants,
                tag: tag.into(),
                repr: repr.map(Into::into),
            });
        }
        Ok(match self.types {
            RuntimeProgramTypes::Plan(plan) => {
                let ty = decode_plan_row(occurrence.row).ok_or_else(unsupported)?;
                match plan
                    .type_table()
                    .get(ty)
                    .ok_or_else(unsupported)?
                    .projection()
                {
                    PlanType::Unit => TypeShape::Unit,
                    PlanType::Bool => TypeShape::Bool,
                    PlanType::Signed(width) => TypeShape::from(*width),
                    PlanType::Unsigned(width) => TypeShape::from(*width),
                    PlanType::F32 => TypeShape::F32,
                    PlanType::F64 => TypeShape::F64,
                    PlanType::String => TypeShape::String,
                    PlanType::Char => TypeShape::Char,
                    PlanType::Bytes => match policy {
                        Some(RuntimeCodecUse::Bytes { format }) => TypeShape::Bytes {
                            format: (*format).into(),
                        },
                        _ => return Err(missing("Bytes use-site")),
                    },
                    PlanType::Option { .. } => TypeShape::option(child(0)?),
                    PlanType::Sequence { .. } | PlanType::Array { .. } => TypeShape::seq(child(0)?),
                    PlanType::Tuple(_) => {
                        TypeShape::tuple(children.iter().copied().map(TypeShape::Ref))
                    }
                    PlanType::Map { kind, .. } => {
                        TypeShape::map(child(0)?, child(1)?, (*kind).into())
                    }
                    PlanType::Nominal { .. }
                        if matches!(policy, Some(RuntimeCodecUse::Tuple { .. })) =>
                    {
                        TypeShape::tuple(children.iter().copied().map(TypeShape::Ref))
                    }
                    PlanType::Nominal { .. }
                        if matches!(policy, Some(RuntimeCodecUse::Plain))
                            && children.is_empty() =>
                    {
                        TypeShape::Unit
                    }
                    PlanType::Nominal { .. }
                    | PlanType::Record(_)
                    | PlanType::Result { .. }
                    | PlanType::BuiltinVariant { .. } => return Err(missing("record/enum")),
                    _ => return Err(unsupported()),
                }
            }
            RuntimeProgramTypes::Awbc(program) => match program.runtime_types[occurrence.row]
                .shape()
            {
                AwbcType::Unit => TypeShape::Unit,
                AwbcType::Bool => TypeShape::Bool,
                AwbcType::Int(width) => {
                    TypeShape::from(crate::value::RuntimeSignedIntWidth::from(*width))
                }
                AwbcType::UInt(width) => {
                    TypeShape::from(crate::value::RuntimeUnsignedIntWidth::from(*width))
                }
                AwbcType::F32 => TypeShape::F32,
                AwbcType::F64 => TypeShape::F64,
                AwbcType::String => TypeShape::String,
                AwbcType::Char => TypeShape::Char,
                AwbcType::Bytes => match policy {
                    Some(RuntimeCodecUse::Bytes { format }) => TypeShape::Bytes {
                        format: (*format).into(),
                    },
                    _ => return Err(missing("Bytes use-site")),
                },
                AwbcType::Sequence(_) | AwbcType::Array { .. } => TypeShape::seq(child(0)?),
                AwbcType::Tuple(_) => {
                    TypeShape::tuple(children.iter().copied().map(TypeShape::Ref))
                }
                AwbcType::Map { kind, .. } => TypeShape::map(child(0)?, child(1)?, (*kind).into()),
                AwbcType::Variant {
                    owner: AwbcVariantIdentity::Builtin(RuntimeBuiltinVariantIdentity::Option),
                    ..
                } => TypeShape::option(child(0)?),
                AwbcType::NominalRecord { .. }
                    if matches!(policy, Some(RuntimeCodecUse::Tuple { .. })) =>
                {
                    TypeShape::tuple(children.iter().copied().map(TypeShape::Ref))
                }
                AwbcType::NominalRecord { .. }
                    if matches!(policy, Some(RuntimeCodecUse::Plain)) && children.is_empty() =>
                {
                    TypeShape::Unit
                }
                AwbcType::Record { .. }
                | AwbcType::NominalRecord { .. }
                | AwbcType::Nominal { .. }
                | AwbcType::Variant { .. } => return Err(missing("record/enum")),
                _ => return Err(unsupported()),
            },
        })
    }

    fn record_names(&self, row: usize) -> Option<Vec<String>> {
        match self.types {
            RuntimeProgramTypes::Plan(plan) => {
                let ty = decode_plan_row(row)?;
                if let Some(record) = plan.nominal_record_domains().get(ty) {
                    return record
                        .fields()
                        .iter()
                        .map(|field| field.name().map(str::to_owned))
                        .collect();
                }
                let PlanType::Record(fields) = plan.type_table().get(ty)?.projection() else {
                    return None;
                };
                Some(
                    fields
                        .iter()
                        .map(|field| field.diagnostic_name().to_owned())
                        .collect(),
                )
            }
            RuntimeProgramTypes::Awbc(program) => match program.runtime_types.get(row)?.shape() {
                AwbcType::Record { fields, .. } | AwbcType::NominalRecord { fields, .. } => fields
                    .iter()
                    .map(|field| {
                        field
                            .name
                            .and_then(|name| program.strings.get(name.index()))
                            .cloned()
                    })
                    .collect(),
                _ => None,
            },
        }
    }

    fn case_names(&self, row: usize) -> Option<Vec<String>> {
        match self.types {
            RuntimeProgramTypes::Plan(plan) => plan
                .variant_domains()
                .get(decode_plan_row(row)?)
                .map(|domain| {
                    domain
                        .cases()
                        .iter()
                        .map(|case| case.name().to_owned())
                        .collect()
                }),
            RuntimeProgramTypes::Awbc(program) => match program.runtime_types.get(row)?.shape() {
                AwbcType::Variant { cases, .. } => cases
                    .iter()
                    .map(|case| program.strings.get(case.name.index()).cloned())
                    .collect(),
                _ => None,
            },
        }
    }
}

impl ShapeAccess for RuntimeProgramDataShapes<'_> {
    fn get_shape(&self, id: ShapeId) -> Option<Cow<'_, TypeShape>> {
        self.project(id).ok().map(Cow::Owned)
    }
}

fn plan_row(ty: RuntimePlanTypeId) -> usize {
    (ty.get().get() - 1) as usize
}
fn decode_plan_row(row: usize) -> Option<RuntimePlanTypeId> {
    row.checked_add(1)
        .and_then(|raw| u32::try_from(raw).ok())
        .and_then(NonZeroU32::new)
        .map(RuntimePlanTypeId::from_accepted_ordinal)
}
fn field_shape(
    rust_name: String,
    field: &RuntimeFieldCodecUse,
    shape: TypeShape,
) -> arcweft_data::FieldShape {
    arcweft_data::FieldShape {
        rust_name,
        wire_name: field.wire_name.clone(),
        shape,
        has_default: field.has_default,
        skip: field.skip,
        bytes_format: field.bytes_format.map(Into::into),
    }
}
fn collect_references(shape: &TypeShape, output: &mut Vec<ShapeId>) {
    match shape {
        TypeShape::Ref(id) => output.push(*id),
        TypeShape::Option(item) | TypeShape::Seq(item) => collect_references(item, output),
        TypeShape::Tuple(items) => items
            .iter()
            .for_each(|item| collect_references(item, output)),
        TypeShape::Map { key, value, .. } => {
            collect_references(key, output);
            collect_references(value, output);
        }
        TypeShape::Record { fields, .. } => fields
            .iter()
            .for_each(|field| collect_references(&field.shape, output)),
        TypeShape::Enum { variants, .. } => variants
            .iter()
            .filter_map(|case| case.payload.as_ref())
            .for_each(|payload| collect_references(payload, output)),
        _ => {}
    }
}

#[cfg(test)]
mod tests;
