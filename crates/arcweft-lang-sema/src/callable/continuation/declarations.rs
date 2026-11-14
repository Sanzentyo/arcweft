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
