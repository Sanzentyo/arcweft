//! Original declaration schemes owned by the real prepared call graph.

use super::*;
use crate::{
    callable::{
        CallableSignatureSchema, CallableSignatureSchemaDigest, CallableTerminalEffectProjection,
        CheckedCallResolverAuthority, CheckedCallableCatalogBuilder, CheckedCallableId,
        EffectPermission, PreparedResolvedCallable,
    },
    effect_row::prepared::{
        PreparedEffectDeclaration, PreparedEffectFailure, PreparedEffectGraph,
        PreparedEffectInvariant, PreparedEffectRow,
    },
    effect_row::{DecisionControl, DecisionWork},
};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum PreparedCallableValueTypeFailure<E: std::error::Error + 'static> {
    #[error(transparent)]
    Authority(#[from] CallConstraintInvariant),
    #[error(transparent)]
    Effect(#[from] PreparedEffectFailure<E>),
    #[error(transparent)]
    Projection(
        #[from]
        crate::callable::schema::CallableFunctionTypeProjectionError<
            crate::callable::schema::PreparedCallableFunctionTypeFailure<E>,
        >,
    ),
    #[error(transparent)]
    Binding(
        #[from]
        crate::types::TypeProjectionError<crate::types::PreparedTypeEffectProjectionFailure<E>>,
    ),
}

/// Value results enter inference with their live effect terms. Content has no
/// type projection and retains its typed operation identity.
pub(crate) enum PreparedApplicationResult {
    Value(TypeKind<crate::types::PreparedEffects>),
    ContentEmission(crate::callable::ContentCallableIdentity),
}

pub(crate) struct PreparedCallableEffectDeclarations {
    graph: PreparedEffectGraph,
    declarations: BTreeMap<CheckedCallableId, PreparedCallableEffectDeclaration>,
}

pub(crate) struct PreparedCallableEffectDeclaration {
    schema: CallableSignatureSchemaDigest,
    header: PreparedEffectDeclaration,
    /// Application of this scheme to its own ordered formals. Value uses
    /// share the live term while body preparation updates its meaning.
    formal_application: PreparedEffectRow,
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
    #[cfg(test)]
    pub(crate) fn empty_for_test() -> Self {
        Self {
            graph: PreparedEffectGraph::default(),
            declarations: BTreeMap::new(),
        }
    }

    pub(crate) fn read(&self) -> crate::effect_row::prepared::PreparedEffectReadView<'_> {
        self.graph.read()
    }

    pub(crate) fn observation_dependency_owner<C: DecisionControl>(
        &self,
        scheme: &crate::effect_row::prepared::PreparedEffectSchemeRef,
        control: &mut C,
    ) -> Result<&CheckedCallableId, PreparedEffectFailure<C::Error>> {
        for (owner, declaration) in &self.declarations {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            if declaration.header.scheme() == scheme {
                declaration.header.validate(&self.graph)?;
                return Ok(owner);
            }
        }
        Err(PreparedEffectInvariant::StaleReference.into())
    }

    pub(crate) fn declaration_body_is_prepared(
        &self,
        owner: &CheckedCallableId,
    ) -> Result<bool, PreparedEffectInvariant> {
        let declaration = self
            .declarations
            .get(owner)
            .ok_or(PreparedEffectInvariant::StaleReference)?;
        self.read()
            .scheme_body_is_prepared(declaration.header.scheme())
    }

    pub(super) fn with_read_scope<C: DecisionControl, T>(
        &self,
        scope: Option<&PreparedEffectReadScope>,
        control: &mut C,
        read: impl FnOnce(crate::effect_row::prepared::PreparedEffectReadView<'_>, &mut C) -> T,
    ) -> Result<T, PreparedEffectFailure<C::Error>> {
        let Some(scope) = scope else {
            return Ok(read(self.read(), control));
        };
        let scope = self.graph.compose_read_scopes([scope], control)?;
        let graph = self.graph.read_scoped(&scope, control)?;
        Ok(read(graph, control))
    }

    pub(super) fn retain_probe_effect_reads<C: DecisionControl>(
        &self,
        delta: &PreparedRelationDelta,
        control: &mut C,
    ) -> Result<PreparedEffectReadScope, PreparedEffectFailure<C::Error>> {
        self.graph.read_scope([delta], control)
    }

    pub(super) fn validate_checkpoint(
        &self,
        checkpoint: &crate::effect_row::prepared::PreparedEffectCheckpoint,
    ) -> Result<(), CallConstraintInvariant> {
        self.graph.validate_checkpoint(checkpoint).map_err(|error| {
            CallConstraintInvariant::EffectTransfer(PreparedEffectFailure::Invariant(error))
        })
    }

    fn prepare_callable_arrow<C>(
        &self,
        builder: &CheckedCallableCatalogBuilder,
        candidate: &PreparedResolvedCallable,
        position: CallableParameterCoordinate,
        terminal: CallableTerminalEffectProjection<'_>,
        control: &mut C,
    ) -> Result<
        crate::types::TypeKind<crate::types::PreparedEffects>,
        PreparedCallableValueTypeFailure<<C as crate::types::TypeProjectionControl>::Error>,
    >
    where
        C: crate::types::TypeProjectionControl
            + DecisionControl<Error = <C as crate::types::TypeProjectionControl>::Error>,
    {
        let schema = candidate.schema();
        // An inferred result can refine the payload of an arrow without
        // changing which declaration scheme owns its terminal effect.
        if let Some(owner) = candidate.checked() {
            let pending = builder
                .pending_by_id(owner)
                .map_err(|_| CallConstraintInvariant::CheckedCallableAuthorityMismatch)?;
            if pending.body_contract().is_some_and(|contract| {
                matches!(contract.permission(), EffectPermission::UnboundedInference)
            }) {
                return self.prepare_declared_arrow(
                    owner,
                    schema,
                    position,
                    terminal.inferred_result_schema(),
                    control,
                );
            }
        }
        match terminal {
            CallableTerminalEffectProjection::Known {
                effects,
                result_schema,
            } => self.prepare_known_arrow(schema, position, effects, result_schema, control),
            CallableTerminalEffectProjection::Pending(owner) => {
                self.prepare_declared_arrow(owner, schema, position, None, control)
            }
        }
    }

    fn prepare_declared_arrow<C>(
        &self,
        owner: &CheckedCallableId,
        schema: &CallableSignatureSchema,
        position: CallableParameterCoordinate,
        result: Option<&crate::callable::CallableResultSchema>,
        control: &mut C,
    ) -> Result<
        TypeKind<crate::types::PreparedEffects>,
        PreparedCallableValueTypeFailure<<C as crate::types::TypeProjectionControl>::Error>,
    >
    where
        C: crate::types::TypeProjectionControl
            + DecisionControl<Error = <C as crate::types::TypeProjectionControl>::Error>,
    {
        let declaration = self
            .declarations
            .get(owner)
            .ok_or(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::StaleReference,
            ))?;
        declaration
            .validate_schema(schema)
            .map_err(PreparedEffectFailure::Invariant)?
            .validate(&self.graph)
            .map_err(PreparedEffectFailure::Invariant)?;
        Ok(schema.prepare_function_type_with_result_from_position(
            position,
            &declaration.formal_application,
            result,
            &self.graph,
            control,
        )?)
    }

    fn prepare_known_arrow<C>(
        &self,
        schema: &CallableSignatureSchema,
        position: CallableParameterCoordinate,
        row: &crate::effect_row::EffectRow,
        result: Option<&crate::callable::CallableResultSchema>,
        control: &mut C,
    ) -> Result<
        TypeKind<crate::types::PreparedEffects>,
        PreparedCallableValueTypeFailure<<C as crate::types::TypeProjectionControl>::Error>,
    >
    where
        C: crate::types::TypeProjectionControl
            + DecisionControl<Error = <C as crate::types::TypeProjectionControl>::Error>,
    {
        let row = row
            .clone_with_control(control)
            .map_err(PreparedEffectFailure::Control)?;
        Ok(schema.prepare_function_type_with_result_from_position(
            position,
            &PreparedEffectRow::Canonical(row),
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
            let formal_application = graph.application(header.scheme(), arguments, control)?;
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            declarations.insert(
                owner.clone(),
                PreparedCallableEffectDeclaration {
                    schema: schema.semantic_digest(),
                    header,
                    formal_application,
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
    pub(crate) fn retain_child_effect_reads<
        D: crate::types::constraints::ConstraintDomain,
        C: DecisionControl,
    >(
        &self,
        delta: &PreparedCallGraphDelta<P, U>,
        pending: &mut crate::types::constraints::PendingChildConstraint<D>,
        control: &mut C,
    ) -> Result<(), PreparedCallGraphMutationFailure<C::Error>> {
        let reads = self.retain_probe_effect_reads(delta, control)?;
        let declarations = self
            .declarations
            .as_ref()
            .ok_or(CallConstraintInvariant::InvalidPreparedNodeState)?;
        pending
            .retain_effect_reads(&declarations.graph, &reads, control)
            .map_err(PreparedCallGraphMutationFailure::effect)
    }

    pub(crate) fn prepare_declaration_value<C>(
        &mut self,
        builder: &CheckedCallableCatalogBuilder,
        candidate: &PreparedResolvedCallable,
        control: &mut C,
    ) -> Result<
        TypeKind<crate::types::PreparedEffects>,
        PreparedCallableValueTypeFailure<<C as crate::types::TypeProjectionControl>::Error>,
    >
    where
        C: crate::types::TypeProjectionControl
            + DecisionControl<Error = <C as crate::types::TypeProjectionControl>::Error>,
    {
        let terminal = CheckedCallResolverAuthority::preparing(builder, self.effects.view())
            .terminal_effects_for(candidate)
            .map_err(|_| CallConstraintInvariant::CheckedCallableAuthorityMismatch)?;
        if matches!(terminal, CallableTerminalEffectProjection::Pending(_)) {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
        }
        let binder = candidate
            .schema()
            .function_value_binder_with_control(control)
            .map_err(|error| {
                use crate::types::{PreparedTypeEffectProjectionFailure, TypeProjectionError};
                match error {
                    TypeProjectionError::Instantiation(error) => {
                        TypeProjectionError::Instantiation(error)
                    }
                    TypeProjectionError::Control(error) => TypeProjectionError::Control(
                        PreparedTypeEffectProjectionFailure::Control(error),
                    ),
                }
            })?;
        let declarations = self
            .declarations
            .as_mut()
            .ok_or(CallConstraintInvariant::InvalidPreparedNodeState)?;
        let value = declarations.prepare_callable_arrow(
            builder,
            candidate,
            CallableParameterCoordinate::group_start(candidate.base_call_group()),
            terminal,
            control,
        )?;
        Ok(binder.quantify_prepared_function(value, &mut declarations.graph, control)?)
    }

    pub(crate) fn prepare_application_result<C>(
        &self,
        builder: &crate::callable::CheckedCallableCatalogBuilder,
        candidate: &super::super::PreparedResolvedCallable,
        mapping: &crate::callable::PreparedCallArgumentMapping,
        control: &mut C,
    ) -> Result<
        PreparedApplicationResult,
        PreparedCallableValueTypeFailure<<C as crate::types::TypeProjectionControl>::Error>,
    >
    where
        C: crate::types::TypeProjectionControl
            + DecisionControl<Error = <C as crate::types::TypeProjectionControl>::Error>,
    {
        use crate::callable::CallableResultSchema;

        let transition = candidate.application_transition(mapping)?;
        let terminal = CheckedCallResolverAuthority::preparing(builder, self.effects.view())
            .terminal_effects_for(candidate)
            .map_err(|_| CallConstraintInvariant::CheckedCallableAuthorityMismatch)?;
        let schema = candidate.schema();
        let Some(next) = transition.remaining_position() else {
            let result = terminal
                .inferred_result_schema()
                .unwrap_or_else(|| schema.result_schema());
            return Ok(match result {
                CallableResultSchema::Value(value) => PreparedApplicationResult::Value(
                    value
                        .prepare_effects(schema.generic_inventory().template_scope(), control)
                        .map_err(
                            super::super::schema::PreparedCallableFunctionTypeFailure::Projection,
                        )
                        .map_err(
                            super::super::schema::CallableFunctionTypeProjectionError::Projection,
                        )?,
                ),
                CallableResultSchema::ContentEmission(identity) => {
                    PreparedApplicationResult::ContentEmission(*identity)
                }
            });
        };
        let declarations = self
            .declarations
            .as_ref()
            .ok_or(CallConstraintInvariant::InvalidPreparedNodeState)?;
        let value =
            declarations.prepare_callable_arrow(builder, candidate, next, terminal, control)?;
        Ok(PreparedApplicationResult::Value(value))
    }

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
