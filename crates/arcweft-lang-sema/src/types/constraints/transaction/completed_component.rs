//! Canonical component values read one already closed original calculation.

use super::*;
use crate::effect_row::prepared::PreparedEffectReadView;
use crate::types::constraints::CompletedConstraintComponent;

impl<D: ConstraintDomain> MaterializationInput<D> {
    pub(in crate::types::constraints::transaction) fn from_closed_original<
        A: TypeConstraintAccounting,
        L,
    >(
        source: &ConstraintPath<D>,
        selected: ConstraintApplicationId,
        branch: &ConstraintInferenceBranch<D, L>,
        projection: &mut ConstraintEffectProjection<D>,
        graph: PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<Self, TypeConstraintError> {
        context.check_cancelled()?;
        projection.owner().require_branch(branch)?;
        ensure_unique_sources(source)?;
        // Source identity and the joint calculation are separate roles. Every
        // source application must be the same opening retained by the branch.
        let mut ordered = BTreeMap::new();
        for application in source.applications.applications() {
            context.enter_node()?;
            let owner = branch
                .applications
                .require_application_owner(application.id())?;
            if !Arc::ptr_eq(
                owner,
                source
                    .applications
                    .require_application_owner(application.id())?,
            ) || ordered
                .insert(application.application(), application.id())
                .is_some()
            {
                return Err(TypeConstraintInvariant::OriginalProducer.into());
            }
        }
        let selected_domain = source
            .applications
            .require_application(selected)?
            .application();
        for request in &source.lineage.projections {
            context.enter_node()?;
            source
                .applications
                .require_application(request.application)?;
        }
        let (closed_sources, preparations) = source
            .lineage
            .close_trace(branch, projection, graph, context)?;
        for equation in &source.equations {
            context.enter_node()?;
            match equation.source_ordinal {
                Some(ordinal) => {
                    let mut found = None;
                    for probe in &closed_sources {
                        context.enter_node()?;
                        if probe.ordinal() == ordinal && found.replace(probe).is_some() {
                            return Err(protocol_error(
                                TypeConstraintSourceProtocolInvariant::WrongSource,
                            ));
                        }
                    }
                    let closed = found.ok_or_else(|| {
                        protocol_error(TypeConstraintSourceProtocolInvariant::WrongSource)
                    })?;
                    equation
                        .validate_completed_source(closed, branch, projection, graph, context)?;
                }
                None => equation.validate_completed(branch, projection, graph, context)?,
            }
        }
        context.validate_type_and_const_completion(branch)?;
        let mut completed = BTreeMap::new();
        for (domain, application) in ordered {
            context.enter_node()?;
            let solution = Arc::new(TypeConstraintSolution::complete_application_with_effects(
                branch,
                application,
                projection,
                graph,
                context,
            )?);
            let projections = Self::finish_projections(
                source,
                branch,
                application,
                &solution,
                projection,
                graph,
                context,
            )?;
            context.enter_node()?;
            completed.insert(
                domain,
                super::super::CompletedConstraintApplication::new(solution, projections),
            );
        }
        let component = CompletedConstraintComponent::new(
            selected_domain,
            completed,
            ClosedConstraintSourceTrace::new(
                selected,
                Arc::clone(&branch.applications),
                closed_sources,
            ),
        );
        Self::new(component, preparations, context)
    }

    fn finish_projections<A: TypeConstraintAccounting, L>(
        source: &ConstraintPath<D>,
        branch: &ConstraintInferenceBranch<D, L>,
        application: ConstraintApplicationId,
        solution: &TypeConstraintSolution,
        projection: &mut ConstraintEffectProjection<D>,
        graph: crate::effect_row::prepared::PreparedEffectReadView<'_>,
        context: &mut TypeConstraintContext<'_, A, D>,
    ) -> Result<
        Box<[super::super::KeyedConstraintProjection<D::Application, D::Projection>]>,
        TypeConstraintError,
    > {
        let mut projections = Vec::new();
        for request in source
            .lineage
            .projections
            .iter()
            .filter(|request| request.application == application)
        {
            let policy = match request.closure {
                TypeConstraintProjectionClosure::Closed => {
                    ConstraintClosurePolicy::ProjectionClosed
                }
                TypeConstraintProjectionClosure::AllowFutureEligible => {
                    ConstraintClosurePolicy::ProjectionFuture
                }
            };
            let value = projection
                .project_operand(request.value.view(), branch, policy, graph, context)
                .map_err(projection_error)?;
            let input = request
                .input
                .as_ref()
                .map(|input| {
                    let projected = projection
                        .project_operand(
                            input.view(),
                            branch,
                            ConstraintClosurePolicy::ProjectionClosed,
                            graph,
                            context,
                        )
                        .map_err(projection_error)?;
                    solution
                        .reify_projection_input(
                            &projected,
                            branch,
                            application,
                            projection.owner(),
                            graph,
                            context,
                        )
                        .map_err(projection_error)
                })
                .transpose()?;
            projections.push(
                solution
                    .reify_projection(
                        Arc::clone(&request.key),
                        &value,
                        branch,
                        application,
                        policy,
                        projection.owner(),
                        graph,
                        context,
                    )
                    .map_err(projection_error)?
                    .with_source(
                        request
                            .source
                            .as_ref()
                            .map(|origin| {
                                context.enter_node()?;
                                Ok::<_, TypeConstraintError>(
                                    super::super::completion::CompletedProjectionAddress {
                                        application: source
                                            .applications
                                            .require_application(origin.application)?
                                            .application(),
                                        key: Arc::clone(&origin.key),
                                    },
                                )
                            })
                            .transpose()?,
                        input,
                    ),
            );
        }
        projections.sort_by(|left, right| left.key().cmp(right.key()));
        if projections
            .windows(2)
            .any(|pair| pair[0].key() == pair[1].key())
        {
            return Err(TypeConstraintError::Invariant(
                TypeConstraintInvariant::Projection(
                    TypeConstraintProjectionInvariant::DuplicateKey,
                ),
            ));
        }
        Ok(projections.into_boxed_slice())
    }
}
