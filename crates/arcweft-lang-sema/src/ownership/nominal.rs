//! Structural retention of an original, generation-bound nominal document.

use super::{
    OwnershipEvidenceRow, OwnershipTraversal, RuntimeOwnershipError, RuntimeOwnershipPath,
    RuntimeOwnershipPathSegment as Step, RuntimeOwnershipProjection,
    RuntimeOwnershipRejection as Rejection, RuntimeProducerArgumentAdmission,
    RuntimeProducerArgumentClassifier,
};
use crate::{
    final_analysis::{RuntimeNominalGraphProjectionError, RuntimeNominalGraphProjectionLimits},
    types::AcceptedNominalType,
};
use arcweft_core::{
    entry::{RuntimeNominalSchemaBody, RuntimeNominalSchemaGraph, RuntimeTypeSchema as Schema},
    pattern::{RuntimeCheckedType, RuntimeSemanticTypeId},
    value::RuntimeOpaqueValueClass,
};
use std::collections::BTreeSet;

impl RuntimeProducerArgumentClassifier<'_> {
    pub(super) fn classify_rust_nominal(
        &self,
        nominal: &AcceptedNominalType,
        path: &RuntimeOwnershipPath,
        depth: u64,
        traversal: &mut OwnershipTraversal,
    ) -> Result<RuntimeProducerArgumentAdmission, RuntimeOwnershipError> {
        let (analysis, world, _) = self.authority().ok_or_else(|| {
            RuntimeOwnershipError::rejected(path, Rejection::MissingRuntimeSnapshotOwner)
        })?;
        let hard = RuntimeNominalGraphProjectionLimits::PRODUCTION;
        let limits = RuntimeNominalGraphProjectionLimits {
            max_type_nodes: traversal
                .limits
                .max_type_nodes
                .saturating_sub(traversal.type_nodes),
            max_nominal_edges: traversal
                .limits
                .max_nominal_edges
                .saturating_sub(traversal.nominal_edges),
            max_type_depth: traversal
                .limits
                .max_recursion_depth
                .saturating_sub(depth)
                .saturating_add(1)
                .min(hard.max_type_depth),
            max_active_nominal_depth: traversal
                .limits
                .max_active_nominal_depth
                .saturating_sub(traversal.active_nominals.len() as u64),
            ..hard
        };
        let projected = analysis
            .project_accepted_rust_nominal(world, nominal, limits)
            .map_err(|source| match source {
                RuntimeNominalGraphProjectionError::Limit { .. } => {
                    RuntimeOwnershipError::WorkLimit
                }
                RuntimeNominalGraphProjectionError::StaleGeneration => {
                    RuntimeOwnershipError::rejected(path, Rejection::StaleAuthority)
                }
                source => RuntimeOwnershipError::NominalSchemaProjection {
                    path: path.clone(),
                    source: Box::new(source),
                },
            })?;
        let work = projected.work();
        traversal.type_nodes = traversal
            .type_nodes
            .checked_add(work.type_nodes)
            .ok_or(RuntimeOwnershipError::WorkLimit)?;
        traversal.nominal_edges = traversal
            .nominal_edges
            .checked_add(work.nominal_edges)
            .ok_or(RuntimeOwnershipError::WorkLimit)?;
        if traversal.type_nodes > traversal.limits.max_type_nodes
            || traversal.nominal_edges > traversal.limits.max_nominal_edges
        {
            return Err(RuntimeOwnershipError::WorkLimit);
        }
        let active = u64::try_from(traversal.active_nominals.len())
            .map_err(|_| RuntimeOwnershipError::WorkLimit)?;
        NominalRetention {
            graph: projected.graph(),
            traversal,
            visited: BTreeSet::new(),
        }
        .nominal(projected.root(), path, depth, active.saturating_add(1))?;
        traversal.push_evidence(|| OwnershipEvidenceRow::NominalGraph {
            semantic_identity: *projected.root().as_bytes(),
            checked_type: *projected.root().as_bytes(),
            declaration_shape: *projected.layout().as_bytes(),
        })?;
        Ok(RuntimeProducerArgumentAdmission::SnapshotClone(
            RuntimeOwnershipProjection::Nominal {
                // Recursive diagnostic types refer to their exact admitted row.
                // The complete graph, rather than this reference, admits values.
                checked: RuntimeCheckedType::ExecutableRef(projected.root()),
                graph: std::sync::Arc::clone(projected.graph()),
                identity: projected.root(),
            },
        ))
    }
}

struct NominalRetention<'a> {
    graph: &'a RuntimeNominalSchemaGraph,
    traversal: &'a mut OwnershipTraversal,
    visited: BTreeSet<RuntimeSemanticTypeId>,
}

impl NominalRetention<'_> {
    fn nominal(
        &mut self,
        identity: RuntimeSemanticTypeId,
        path: &RuntimeOwnershipPath,
        depth: u64,
        active: u64,
    ) -> Result<(), RuntimeOwnershipError> {
        self.traversal.nominal_edges = self
            .traversal
            .nominal_edges
            .checked_add(1)
            .ok_or(RuntimeOwnershipError::WorkLimit)?;
        if self.traversal.nominal_edges > self.traversal.limits.max_nominal_edges {
            return Err(RuntimeOwnershipError::WorkLimit);
        }
        // The source graph has already rejected structural/argument cycles.
        // Reached nominal body back-edges close this finite retention proof.
        if self.visited.contains(&identity) {
            return Ok(());
        }
        if active > self.traversal.limits.max_active_nominal_depth {
            return Err(RuntimeOwnershipError::WorkLimit);
        }
        self.visited.insert(identity);
        let definition = self.graph.definition(identity).ok_or_else(|| {
            RuntimeOwnershipError::rejected(path, Rejection::MissingRuntimeSnapshotOwner)
        })?;
        let child_depth = OwnershipTraversal::child_depth(depth)?;
        for (ordinal, argument) in definition.arguments().iter().enumerate() {
            self.schema(
                argument,
                &path.pushed(Step::AcceptedNominalArgument(index(ordinal)?)),
                child_depth,
                active,
            )?;
        }
        match definition.body() {
            RuntimeNominalSchemaBody::Record { fields, .. } => {
                for (ordinal, field) in fields.iter().enumerate() {
                    self.schema(
                        field.schema(),
                        &path.pushed(Step::NominalMember(index(ordinal)?)),
                        child_depth,
                        active,
                    )?;
                }
            }
            RuntimeNominalSchemaBody::Variant { cases } => {
                for case in cases {
                    if let Some(payload) = case.payload() {
                        self.schema(
                            payload,
                            &path.pushed(Step::NominalMember(case.ordinal())),
                            child_depth,
                            active,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn schema(
        &mut self,
        schema: &Schema,
        path: &RuntimeOwnershipPath,
        depth: u64,
        active: u64,
    ) -> Result<(), RuntimeOwnershipError> {
        self.traversal.charge_type(depth)?;
        let next = OwnershipTraversal::child_depth(depth)?;
        match schema {
            Schema::NominalRef(identity) => self.nominal(
                identity.semantic_identity(),
                path,
                depth,
                active
                    .checked_add(1)
                    .ok_or(RuntimeOwnershipError::WorkLimit)?,
            )?,
            Schema::ExecutableRef(_) => {
                return Err(RuntimeOwnershipError::rejected(
                    path,
                    Rejection::FunctionValueRequiresCertificate,
                ));
            }
            Schema::ExactOpaque { owner, arguments } => {
                for (ordinal, argument) in arguments.iter().enumerate() {
                    self.schema(
                        argument,
                        &path.pushed(Step::AcceptedNominalArgument(index(ordinal)?)),
                        next,
                        active,
                    )?;
                }
                if owner.value_class() != RuntimeOpaqueValueClass::Plain {
                    return Err(RuntimeOwnershipError::rejected(
                        path,
                        Rejection::AffineValue,
                    ));
                }
                self.traversal
                    .push_evidence(|| OwnershipEvidenceRow::AcceptedOpaque {
                        semantic_identity: *owner.semantic_identity().as_bytes(),
                        runtime_producer: owner.producer().as_str().to_owned(),
                        value_class: owner.value_class(),
                        persistence: owner.persistence(),
                    })?;
            }
            Schema::Seq(item) | Schema::Array { item, .. } => {
                self.schema(item, &path.pushed(Step::SequenceItem), next, active)?
            }
            Schema::Map { key, value, .. } => {
                self.schema(key, &path.pushed(Step::TupleItem(0)), next, active)?;
                self.schema(value, &path.pushed(Step::TupleItem(1)), next, active)?;
            }
            Schema::Tuple(items) | Schema::Choice(items) => {
                for (ordinal, item) in items.iter().enumerate() {
                    self.schema(
                        item,
                        &path.pushed(Step::TupleItem(index(ordinal)?)),
                        next,
                        active,
                    )?;
                }
            }
            Schema::RecordValue { fields } => {
                for (ordinal, field) in fields.iter().enumerate() {
                    self.schema(
                        field.schema(),
                        &path.pushed(Step::VariantPayloadField(index(ordinal)?)),
                        next,
                        active,
                    )?;
                }
            }
            Schema::Builtin(builtin) => {
                for (ordinal, payload) in builtin.payloads().iter().enumerate() {
                    self.schema(
                        payload,
                        &path.pushed(Step::VariantPayloadField(index(ordinal)?)),
                        next,
                        active,
                    )?;
                }
            }
            Schema::Named(_) | Schema::Record { .. } | Schema::Enum { .. } => {
                return Err(RuntimeOwnershipError::rejected(
                    path,
                    Rejection::MissingRuntimeSnapshotOwner,
                ));
            }
            Schema::Unit
            | Schema::Bool
            | Schema::I8
            | Schema::I16
            | Schema::I32
            | Schema::I64
            | Schema::I128
            | Schema::ISize
            | Schema::U8
            | Schema::U16
            | Schema::U32
            | Schema::U64
            | Schema::U128
            | Schema::USize
            | Schema::F32
            | Schema::F64
            | Schema::String
            | Schema::Color
            | Schema::Char
            | Schema::Bytes { .. }
            | Schema::Never
            | Schema::Duration
            | Schema::Progress
            | Schema::DenseNumeric(_)
            | Schema::EntityReference
            | Schema::AgentValue => {}
        }
        Ok(())
    }
}

fn index(ordinal: usize) -> Result<u32, RuntimeOwnershipError> {
    u32::try_from(ordinal).map_err(|_| RuntimeOwnershipError::WorkLimit)
}
