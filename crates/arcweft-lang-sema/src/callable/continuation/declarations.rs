//! Original declaration schemes owned by the real prepared call graph.

use super::*;
use crate::{
    callable::{CallableSignatureSchema, CallableSignatureSchemaDigest, CheckedCallableId},
    effect_row::prepared::{
        PreparedEffectDeclaration, PreparedEffectFailure, PreparedEffectGraph,
        PreparedEffectInvariant,
    },
    effect_row::{DecisionControl, DecisionWork},
};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PreparedCallableValueTypeFailure<E: std::error::Error + 'static> {
    #[error(transparent)]
    Effect(#[from] PreparedEffectFailure<E>),
    #[error(transparent)]
    Projection(
        #[from]
        crate::callable::schema::CallableFunctionTypeProjectionError<
            crate::callable::schema::PreparedCallableFunctionTypeFailure<E>,
        >,
    ),
}

pub(crate) struct PreparedCallableEffectDeclarations {
    graph: PreparedEffectGraph,
    declarations: BTreeMap<CheckedCallableId, PreparedCallableEffectDeclaration>,
}

pub(crate) struct PreparedCallableEffectDeclaration {
    schema: CallableSignatureSchemaDigest,
    header: PreparedEffectDeclaration,
}

impl PreparedCallableEffectDeclaration {
    pub(crate) fn validate_schema(
        &self,
        schema: &CallableSignatureSchema,
    ) -> Result<&PreparedEffectDeclaration, PreparedEffectInvariant> {
        if self.schema != schema.semantic_digest() {
            return Err(PreparedEffectInvariant::StaleReference);
        }
        Ok(&self.header)
    }
}

impl PreparedCallableEffectDeclarations {
    pub(crate) fn prepare_declared_value_type<C>(
        &mut self,
        owner: &CheckedCallableId,
        schema: &CallableSignatureSchema,
        control: &mut C,
    ) -> Result<
        crate::types::TypeKind<crate::types::PreparedEffects>,
        PreparedCallableValueTypeFailure<<C as crate::types::TypeProjectionControl>::Error>,
    >
    where
        C: crate::types::TypeProjectionControl
            + DecisionControl<Error = <C as crate::types::TypeProjectionControl>::Error>,
    {
        use crate::effect_row::prepared::PreparedEffectRow;
        let header = self
            .declarations
            .get(owner)
            .ok_or(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::StaleReference,
            ))?
            .validate_schema(schema)
            .map_err(PreparedEffectFailure::Invariant)?;
        header
            .validate(&self.graph)
            .map_err(PreparedEffectFailure::Invariant)?;
        let mut checkpoint = self.graph.checkpoint(control)?;
        let prepared = (|| {
            let mut arguments = Vec::new();
            for parameter in header.parameters() {
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                arguments.push(PreparedEffectRow::Canonical(
                    crate::effect_row::EffectRow::open(
                        crate::effects::EffectSet::new(),
                        parameter.clone(),
                    ),
                ));
            }
            let row = self
                .graph
                .application(header.scheme(), arguments, control)?;
            Ok(schema.prepare_function_type_from_position(
                crate::callable::CallableParameterCoordinate::group_start(
                    crate::callable::CallableGroupIndex::ZERO,
                ),
                &row,
                &self.graph,
                control,
            )?)
        })();
        if prepared.is_ok() {
            self.graph
                .commit(&checkpoint)
                .map_err(PreparedEffectFailure::Invariant)?;
        } else {
            self.graph
                .rollback(&mut checkpoint)
                .map_err(PreparedEffectFailure::Invariant)?;
        }
        prepared
    }

    pub(crate) fn prepare_known_value_type<C>(
        &self,
        schema: &CallableSignatureSchema,
        row: &crate::effect_row::EffectRow,
        result: Option<&crate::callable::CallableResultSchema>,
        control: &mut C,
    ) -> Result<
        crate::types::TypeKind<crate::types::PreparedEffects>,
        PreparedCallableValueTypeFailure<<C as crate::types::TypeProjectionControl>::Error>,
    >
    where
        C: crate::types::TypeProjectionControl
            + DecisionControl<Error = <C as crate::types::TypeProjectionControl>::Error>,
    {
        let row = row
            .try_clone_with_control(control)
            .map_err(PreparedEffectFailure::Control)?;
        Ok(schema.prepare_function_type_with_result_from_position(
            crate::callable::CallableParameterCoordinate::group_start(
                crate::callable::CallableGroupIndex::ZERO,
            ),
            &crate::effect_row::prepared::PreparedEffectRow::Canonical(row),
            result,
            &self.graph,
            control,
        )?)
    }

    /// Admission completes before this owner is installed into the live graph.
    pub(crate) fn declare<'a, C: DecisionControl>(
        inputs: impl IntoIterator<Item = (&'a CheckedCallableId, &'a CallableSignatureSchema)>,
        control: &mut C,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let mut graph = PreparedEffectGraph::default();
        let mut declarations = BTreeMap::new();
        for (owner, schema) in inputs {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            if declarations.contains_key(owner) {
                return Err(PreparedEffectInvariant::SchemeState.into());
            }
            let mut parameters = Vec::new();
            for parameter in schema.generic_inventory().effects() {
                control
                    .charge(DecisionWork::Node)
                    .map_err(PreparedEffectFailure::Control)?;
                parameters.push(parameter.parameter().clone());
            }
            let scheme =
                graph.declare_borrowed_domain(parameters, schema.effect_predicate(), control)?;
            let header = graph.declaration(&scheme, control)?;
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            declarations.insert(
                owner.clone(),
                PreparedCallableEffectDeclaration {
                    schema: schema.semantic_digest(),
                    header,
                },
            );
        }
        Ok(Self {
            graph,
            declarations,
        })
    }

    pub(crate) fn relation(
        &mut self,
    ) -> (
        &mut PreparedEffectGraph,
        &BTreeMap<CheckedCallableId, PreparedCallableEffectDeclaration>,
    ) {
        (&mut self.graph, &self.declarations)
    }
}

impl<P, U> PreparedCallGraph<P, U> {
    pub(crate) fn effect_declarations(
        &self,
    ) -> Result<&PreparedCallableEffectDeclarations, CallConstraintInvariant> {
        self.declarations
            .as_ref()
            .ok_or(CallConstraintInvariant::InvalidPreparedNodeState)
    }
    pub(crate) fn initialize_effect_declarations(
        &mut self,
        declarations: PreparedCallableEffectDeclarations,
    ) -> Result<(), CallConstraintInvariant> {
        if self.declarations.is_some() || !self.active_deltas.is_empty() {
            return Err(CallConstraintInvariant::InvalidPreparedNodeState);
        }
        self.declarations = Some(declarations);
        Ok(())
    }

    pub(crate) fn effect_declarations_mut(
        &mut self,
    ) -> Result<&mut PreparedCallableEffectDeclarations, CallConstraintInvariant> {
        self.declarations
            .as_mut()
            .ok_or(CallConstraintInvariant::InvalidPreparedNodeState)
    }
}
