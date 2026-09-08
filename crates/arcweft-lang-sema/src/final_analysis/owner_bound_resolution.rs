//! Atomic sealing for expression facts whose identity is owned by an
//! accepted semantic coordinate.
//!
//! This phase is intentionally between structural-edge construction and call
//! finalization.  It consumes the analyzer's temporary implicit-capture ledger
//! and publishes only complete checked expression facts.  HIR IDs remain
//! lookup evidence inside the terminal rows; they are never used as semantic
//! identity inputs.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arcweft_lang_hir::{
    identity::{ExprId, LocalId},
    project::HirProjectEvaluationTopology,
};

use crate::effects::EffectSet;
use crate::semantic_coordinate::{CheckedSemanticPath, SemanticCoordinateIndex};

use super::prepared::{PreparedImplicitCallable, PreparedTry, PreparedTryBoundary};
use super::{
    CheckedExpression, CheckedExpressionResolution, CheckedImplicitCallable,
    CheckedImplicitCallableBody, CheckedImplicitCallableIdentityEvidence, CheckedImplicitCapture,
    CheckedImplicitCaptureOccurrence, CheckedImplicitParameter, CheckedImplicitParameterOccurrence,
    CheckedPipe, CheckedPipeLeft, CheckedPipeLeftOccurrence, CheckedTry, CheckedTryBoundary,
    CheckedTryBoundaryOwner, CheckedTryCallableBoundary, CheckedTryCarrier,
    CheckedTryExpressionBoundary, CheckedTryFunctionSite, CheckedTryOperandAuthorityViolation,
    FinalSemanticAnalysisError, PreparedExpressionFact, PreparedExpressionShell,
    PreparedImplicitCallableBody, PreparedOwnerBoundResolution, TypeKind,
};

/// Seals every prepared owner-bound row in one transaction-local batch.
///
/// The returned map contains replacements for the owner-bound entries only.
/// Callers must install all replacements after this function succeeds so a
/// failed batch cannot expose a partially sealed expression map.
pub(super) fn seal(
    topology: Arc<HirProjectEvaluationTopology>,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    local_types: &BTreeMap<LocalId, TypeKind>,
    pending_capture_uses: &BTreeMap<ExprId, Box<[(ExprId, LocalId)]>>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    structural_edges: &super::match_edges::CheckedStructuralEdgeDraft,
    checked_callables: &crate::callable::CheckedCallableCatalog,
) -> Result<BTreeMap<ExprId, PreparedExpressionFact>, FinalSemanticAnalysisError> {
    let owners = expressions
        .iter()
        .filter_map(|(owner, fact)| {
            matches!(fact, PreparedExpressionFact::OwnerBound(_)).then_some(*owner)
        })
        .collect::<Vec<_>>();
    let callable_owners = owners
        .iter()
        .copied()
        .filter(|owner| {
            matches!(
                expressions.get(owner),
                Some(PreparedExpressionFact::OwnerBound(prepared))
                    if matches!(
                        prepared.resolution(),
                        PreparedOwnerBoundResolution::ImplicitCallable(_)
                    )
            )
        })
        .collect::<BTreeSet<_>>();
    let pipe_owners = owners
        .iter()
        .copied()
        .filter(|owner| {
            matches!(
                expressions.get(owner),
                Some(PreparedExpressionFact::OwnerBound(prepared))
                    if matches!(
                        prepared.resolution(),
                        PreparedOwnerBoundResolution::Pipe(_)
                    )
            )
        })
        .collect::<BTreeSet<_>>();
    let try_owners = owners
        .iter()
        .copied()
        .filter(|owner| {
            matches!(
                expressions.get(owner),
                Some(PreparedExpressionFact::OwnerBound(prepared))
                    if matches!(
                        prepared.resolution(),
                        PreparedOwnerBoundResolution::Try(_)
                    )
            )
        })
        .collect::<BTreeSet<_>>();
    if pending_capture_uses
        .keys()
        .any(|owner| !callable_owners.contains(owner))
    {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }

    let mut replacements = BTreeMap::new();
    // Pipe identities have no callable dependency, so issue their complete
    // rows first.  Callable body closure can then join Pipe/PipeLeft owner
    // facts without publishing an intermediate parent identity.
    let mut pipe_rows = BTreeMap::new();
    for owner in &pipe_owners {
        let Some(PreparedExpressionFact::OwnerBound(prepared)) = expressions.get(owner) else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let PreparedOwnerBoundResolution::Pipe(pipe) = prepared.resolution() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        if pipe_rows.insert(*owner, pipe.clone()).is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    }
    for owner in &callable_owners {
        let Some(PreparedExpressionFact::OwnerBound(prepared)) = expressions.get(owner) else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let PreparedOwnerBoundResolution::ImplicitCallable(callable) = prepared.resolution() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        if let PreparedImplicitCallableBody::OwnerBound(body) = callable.body()
            && let PreparedOwnerBoundResolution::Pipe(pipe) = body.resolution()
        {
            if pipe_rows.insert(*owner, pipe.clone()).is_some() {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
        }
    }
    let (pipes, pipe_replacements) = seal_pipe_rows(expressions, &pipe_rows, coordinates)?;
    replacements.extend(pipe_replacements);
    // Phase one issues body-independent callable identities from one named
    // seed per owner.  Phase two consumes those same seeds and closes bodies
    // only after the complete identity map exists.
    let mut callable_seeds = BTreeMap::new();
    let mut callable_evidence = BTreeMap::new();
    let mut callable_identities = BTreeMap::new();
    for owner in callable_owners.iter().copied() {
        let Some(PreparedExpressionFact::OwnerBound(prepared)) = expressions.get(&owner) else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let PreparedOwnerBoundResolution::ImplicitCallable(callable) = prepared.resolution() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let (_, ty, _) = prepared_shell_parts(prepared.shell(), owner)?;
        let function_type = ty.semantic_identity_digest()?;
        let seed = prepare_callable_seed(
            &topology,
            owner,
            function_type,
            callable,
            pending_capture_uses
                .get(&owner)
                .map(Box::as_ref)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?,
            expressions,
            local_types,
            coordinates,
        )?;
        let evidence = seed.issue_identity(&topology)?;
        let identity = evidence.identity();
        if callable_seeds.insert(owner, seed).is_some()
            || callable_evidence.insert(owner, evidence).is_some()
            || callable_identities.insert(owner, identity).is_some()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
    }

    let mut callables = BTreeMap::new();
    for owner in callable_owners.iter().copied() {
        let seed = callable_seeds
            .get(&owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let evidence = callable_evidence
            .remove(&owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let checked = seal_callable_seed(
            &topology,
            seed,
            evidence,
            expressions,
            &pipes,
            &callable_identities,
            checked_callables,
            coordinates,
            structural_edges,
        )?;
        let Some(PreparedExpressionFact::OwnerBound(prepared)) = expressions.get(&owner) else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let (shell, _ty, effects) = prepared_shell_parts(prepared.shell(), owner)?;
        let (ty, selection, _) = shell
            .into_value_parts()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if ty.semantic_identity_digest()? != seed.function_type
            || callables.insert(owner, checked.clone()).is_some()
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        replacements.insert(
            owner,
            PreparedExpressionFact::Complete(CheckedExpression::value(
                ty,
                selection,
                effects,
                CheckedExpressionResolution::ImplicitCallable(Box::new(checked)),
            )),
        );
    }

    // Try rows are closed only after callable identities and expression
    // coordinates are available.  Their exact operand evidence is issued
    // from the already-sealed authored child edge; the final row retains the
    // operand, carrier, and boundary authorities together.
    for owner in try_owners {
        let Some(PreparedExpressionFact::OwnerBound(prepared)) = expressions.get(&owner) else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let PreparedOwnerBoundResolution::Try(tried) = prepared.resolution() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let checked = seal_try(
            owner,
            tried,
            expressions,
            &callable_identities,
            structural_edges,
            checked_callables,
            coordinates,
        )?;
        let (shell, _, effects) = prepared_shell_parts(prepared.shell(), owner)?;
        let (ty, selection, _) = shell
            .into_value_parts()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        replacements.insert(
            owner,
            PreparedExpressionFact::Complete(CheckedExpression::value(
                ty,
                selection,
                effects,
                CheckedExpressionResolution::Try(checked),
            )),
        );
    }

    for owner in owners.iter().copied() {
        let Some(PreparedExpressionFact::OwnerBound(prepared)) = expressions.get(&owner) else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let PreparedOwnerBoundResolution::ImplicitParameter(parameter) = prepared.resolution()
        else {
            continue;
        };
        let callable = callables
            .get(&parameter.callable())
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if parameter.parameter() != callable.parameter() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        let occurrence = callable
            .parameter_occurrences()
            .iter()
            .find(|occurrence| occurrence.lookup_expression() == owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let (shell, ty, effects) = prepared_shell_parts(prepared.shell(), owner)?;
        let (_, selection, _) = shell
            .into_value_parts()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if ty.semantic_identity_digest()? != callable.parameter().semantic_identity_digest()? {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        replacements.insert(
            owner,
            PreparedExpressionFact::Complete(CheckedExpression::value(
                parameter.parameter().clone(),
                selection,
                effects,
                CheckedExpressionResolution::ImplicitParameter(CheckedImplicitParameter::new(
                    callable.identity(),
                    occurrence.ordinal(),
                    parameter.parameter().semantic_identity_digest()?,
                )),
            )),
        );
    }

    for owner in owners
        .iter()
        .copied()
        .filter(|owner| {
            matches!(
                expressions.get(owner),
                Some(PreparedExpressionFact::OwnerBound(prepared))
                    if matches!(
                        prepared.resolution(),
                        PreparedOwnerBoundResolution::PipeLeft(_)
                    )
            )
        })
        .collect::<Vec<_>>()
    {
        let Some(PreparedExpressionFact::OwnerBound(prepared)) = expressions.get(&owner) else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let PreparedOwnerBoundResolution::PipeLeft(pipe_left) = prepared.resolution() else {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        };
        let pipe = pipes
            .get(&pipe_left.pipe())
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let occurrence = pipe
            .occurrences()
            .iter()
            .find(|occurrence| occurrence.lookup_expression() == owner)
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let (shell, value_type, effects) = prepared_shell_parts(prepared.shell(), owner)?;
        let (ty, selection, _) = shell
            .into_value_parts()
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let value_type = value_type.semantic_identity_digest()?;
        if value_type != pipe.value_type()
            || usize::try_from(occurrence.ordinal())
                .ok()
                .is_none_or(|ordinal| ordinal >= pipe.occurrences().len())
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        replacements.insert(
            owner,
            PreparedExpressionFact::Complete(CheckedExpression::value(
                ty,
                selection,
                effects,
                CheckedExpressionResolution::PipeLeft(CheckedPipeLeft::new(
                    pipe.binding_identity(),
                    occurrence.ordinal(),
                    value_type,
                )),
            )),
        );
    }
    if replacements.len() != owners.len() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    Ok(replacements)
}

fn prepared_shell_parts(
    shell: &PreparedExpressionShell,
    owner: ExprId,
) -> Result<(PreparedExpressionShell, TypeKind, EffectSet), FinalSemanticAnalysisError> {
    let ty = shell
        .value_type()
        .cloned()
        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
    Ok((shell.clone(), ty, shell.effects().clone()))
}

fn seal_pipe_rows(
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    pipe_rows: &BTreeMap<ExprId, super::prepared::PreparedPipe>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<
    (
        BTreeMap<ExprId, CheckedPipe>,
        BTreeMap<ExprId, PreparedExpressionFact>,
    ),
    FinalSemanticAnalysisError,
> {
    let mut pipes = BTreeMap::new();
    let mut replacements = BTreeMap::new();
    for (owner, pipe) in pipe_rows {
        let coordinate = coordinates
            .expression(*owner)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let occurrences = pipe
            .placeholders()
            .iter()
            .copied()
            .enumerate()
            .map(|(ordinal, expression)| {
                let ordinal = u32::try_from(ordinal)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let coordinate = coordinates
                    .expression(expression)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                Ok(CheckedPipeLeftOccurrence::new(
                    expression, coordinate, ordinal,
                ))
            })
            .collect::<Result<Vec<_>, FinalSemanticAnalysisError>>()?;
        let checked = CheckedPipe::seal_owner_bound(
            pipe.lookup_left(),
            pipe.lookup_right(),
            coordinate,
            pipe.left_value_type().semantic_identity_digest()?,
            occurrences,
        )
        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let direct = matches!(
            expressions.get(owner),
            Some(PreparedExpressionFact::OwnerBound(prepared))
                if matches!(prepared.resolution(), PreparedOwnerBoundResolution::Pipe(_))
        );
        if pipes.insert(*owner, checked.clone()).is_some() {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if direct {
            let Some(PreparedExpressionFact::OwnerBound(prepared)) = expressions.get(owner) else {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            };
            let (shell, _, effects) = prepared_shell_parts(prepared.shell(), *owner)?;
            let (ty, selection, _) = shell
                .into_value_parts()
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            replacements.insert(
                *owner,
                PreparedExpressionFact::Complete(CheckedExpression::value(
                    ty,
                    selection,
                    effects,
                    CheckedExpressionResolution::Pipe(checked),
                )),
            );
        }
    }
    Ok((pipes, replacements))
}

fn seal_try(
    owner: ExprId,
    prepared: &PreparedTry,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    callable_identities: &BTreeMap<ExprId, super::CheckedImplicitCallableIdentity>,
    structural_edges: &super::match_edges::CheckedStructuralEdgeDraft,
    checked_callables: &crate::callable::CheckedCallableCatalog,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<CheckedTry, FinalSemanticAnalysisError> {
    let carrier = prepared.carrier().clone();
    let operand_id = structural_edges
        .exact_operand_child(owner)
        .map_err(FinalSemanticAnalysisError::from)?;
    let operand_type = expressions
        .get(&operand_id)
        .and_then(PreparedExpressionFact::value_type)
        .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner: operand_id })?;
    let carrier_type = carrier.as_type();
    if operand_type != &carrier_type {
        return Err(FinalSemanticAnalysisError::TryOperandAuthority {
            violation: CheckedTryOperandAuthorityViolation::OperandTypeMismatch {
                owner,
                expected: Box::new(carrier_type),
                actual: Box::new(operand_type.clone()),
            },
        });
    }
    let operand = super::CheckedTryOperand::from_evidence(
        coordinates
            .expression_evidence(operand_id)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?,
        operand_type.clone(),
    );
    let (boundary_type, boundary_owner) = match prepared.boundary() {
        PreparedTryBoundary::Infallible => {
            if !carrier.is_infallible() {
                return Err(try_boundary_mismatch(owner, &carrier, None));
            }
            (carrier.as_type(), CheckedTryBoundaryOwner::Infallible)
        }
        PreparedTryBoundary::CarrierBlock { lookup_owner } => {
            let boundary_type = expressions
                .get(lookup_owner)
                .and_then(PreparedExpressionFact::value_type)
                .cloned()
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable {
                    owner: *lookup_owner,
                })?;
            if !carrier.accepts_boundary_type(&boundary_type) {
                return Err(try_boundary_mismatch(owner, &carrier, Some(&boundary_type)));
            }
            let evidence = coordinates
                .expression_evidence(*lookup_owner)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            (
                boundary_type,
                CheckedTryBoundaryOwner::CarrierBlock(CheckedTryExpressionBoundary::from_evidence(
                    evidence,
                )),
            )
        }
        PreparedTryBoundary::ExplicitFunctionSite {
            lookup_owner,
            boundary_type,
        } => {
            if !carrier.accepts_boundary_type(boundary_type) {
                return Err(try_boundary_mismatch(owner, &carrier, Some(boundary_type)));
            }
            let evidence = coordinates
                .expression_evidence(*lookup_owner)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            (
                boundary_type.clone(),
                CheckedTryBoundaryOwner::FunctionSite(CheckedTryFunctionSite::Explicit(
                    CheckedTryExpressionBoundary::from_evidence(evidence),
                )),
            )
        }
        PreparedTryBoundary::ImplicitFunctionSite {
            lookup_owner,
            boundary_type,
        } => {
            if !carrier.accepts_boundary_type(boundary_type) {
                return Err(try_boundary_mismatch(owner, &carrier, Some(boundary_type)));
            }
            let callable = callable_identities
                .get(lookup_owner)
                .copied()
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            let evidence = coordinates
                .expression_evidence(*lookup_owner)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            (
                boundary_type.clone(),
                CheckedTryBoundaryOwner::FunctionSite(CheckedTryFunctionSite::Implicit {
                    site: CheckedTryExpressionBoundary::from_evidence(evidence),
                    callable,
                }),
            )
        }
        PreparedTryBoundary::Callable {
            declaration,
            boundary_type,
        } => {
            let facts = checked_callables
                .project_callable(declaration)
                .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            if facts.signature().value_type() != Some(boundary_type)
                || !carrier.accepts_boundary_type(boundary_type)
            {
                return Err(try_boundary_mismatch(owner, &carrier, Some(boundary_type)));
            }
            let accepted = coordinates
                .accepted_declaration(declaration)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            (
                boundary_type.clone(),
                CheckedTryBoundaryOwner::Callable(CheckedTryCallableBoundary::new(
                    declaration.clone(),
                    accepted,
                )),
            )
        }
    };
    Ok(CheckedTry::new(
        operand,
        carrier,
        CheckedTryBoundary::new(boundary_type, boundary_owner),
    ))
}

fn try_boundary_mismatch(
    owner: ExprId,
    carrier: &CheckedTryCarrier,
    boundary: Option<&TypeKind>,
) -> FinalSemanticAnalysisError {
    FinalSemanticAnalysisError::TryOperandAuthority {
        violation: CheckedTryOperandAuthorityViolation::BoundaryMismatch {
            owner,
            carrier: Box::new(carrier.as_type()),
            boundary: boundary.cloned().map(Box::new),
        },
    }
}

fn seal_owner_bound_body(
    topology: &Arc<HirProjectEvaluationTopology>,
    owner: ExprId,
    prepared: &super::PreparedOwnerBoundExpression,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    pipes: &BTreeMap<ExprId, CheckedPipe>,
    callable_identities: &BTreeMap<ExprId, super::CheckedImplicitCallableIdentity>,
    checked_callables: &crate::callable::CheckedCallableCatalog,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    structural_edges: &super::match_edges::CheckedStructuralEdgeDraft,
) -> Result<CheckedExpressionResolution, FinalSemanticAnalysisError> {
    match prepared.resolution() {
        PreparedOwnerBoundResolution::Try(tried) => Ok(CheckedExpressionResolution::Try(seal_try(
            owner,
            tried,
            expressions,
            callable_identities,
            structural_edges,
            checked_callables,
            coordinates,
        )?)),
        PreparedOwnerBoundResolution::ImplicitParameter(parameter) => {
            let callable_identity = callable_identities
                .get(&parameter.callable())
                .copied()
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            let region = topology
                .module(parameter.callable().module())
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
                .expression_uses()
                .implicit_callable_region(parameter.callable())
                .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
            let occurrence_ordinal = region
                .placeholders()
                .enumerate()
                .find_map(|(ordinal, expression)| (expression == owner).then_some(ordinal))
                .and_then(|ordinal| u32::try_from(ordinal).ok())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            Ok(CheckedExpressionResolution::ImplicitParameter(
                super::CheckedImplicitParameter::new(
                    callable_identity,
                    occurrence_ordinal,
                    parameter.parameter().semantic_identity_digest()?,
                ),
            ))
        }
        PreparedOwnerBoundResolution::Pipe(_) => {
            let checked = pipes
                .get(&owner)
                .cloned()
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            Ok(CheckedExpressionResolution::Pipe(checked))
        }
        PreparedOwnerBoundResolution::PipeLeft(pipe_left) => {
            let pipe = pipes
                .get(&pipe_left.pipe())
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            let occurrence = pipe
                .occurrences()
                .iter()
                .find(|occurrence| occurrence.lookup_expression() == owner)
                .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
            let value_type = prepared
                .value_type()
                .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?
                .semantic_identity_digest()?;
            if value_type != pipe.value_type() {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            Ok(CheckedExpressionResolution::PipeLeft(CheckedPipeLeft::new(
                pipe.binding_identity(),
                occurrence.ordinal(),
                value_type,
            )))
        }
        PreparedOwnerBoundResolution::ImplicitCallable(_) => {
            Err(FinalSemanticAnalysisError::WrongPayloadFamily)
        }
    }
}

struct PreparedImplicitCallableSealSeed {
    owner: ExprId,
    function_type: crate::types::SemanticTypeDigest,
    coordinate: CheckedSemanticPath,
    parameter: TypeKind,
    result: TypeKind,
    parameter_occurrences: Box<[CheckedImplicitParameterOccurrence]>,
    capture_occurrences: Box<[CheckedImplicitCaptureOccurrence]>,
    captures: Box<[CheckedImplicitCapture]>,
    body: PreparedImplicitCallableBody,
}

impl PreparedImplicitCallableSealSeed {
    fn issue_identity(
        &self,
        topology: &Arc<HirProjectEvaluationTopology>,
    ) -> Result<CheckedImplicitCallableIdentityEvidence, FinalSemanticAnalysisError> {
        CheckedImplicitCallableIdentityEvidence::issue(
            Arc::clone(topology),
            self.owner,
            self.coordinate.clone(),
            self.function_type,
            self.parameter.clone(),
            self.result.clone(),
            self.parameter_occurrences.clone(),
            self.capture_occurrences.clone(),
            self.captures.clone(),
        )
        .map_err(Into::into)
    }
}

fn prepare_callable_seed(
    topology: &Arc<HirProjectEvaluationTopology>,
    owner: ExprId,
    function_type: crate::types::SemanticTypeDigest,
    prepared: &PreparedImplicitCallable,
    pending_uses: &[(ExprId, LocalId)],
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    local_types: &BTreeMap<LocalId, TypeKind>,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
) -> Result<PreparedImplicitCallableSealSeed, FinalSemanticAnalysisError> {
    let module = topology
        .module(owner.module())
        .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
    let region = module
        .expression_uses()
        .implicit_callable_region(owner)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    let owner_coordinate = coordinates
        .expression(owner)
        .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
    let parameter_occurrences = region
        .placeholders()
        .enumerate()
        .map(|(ordinal, expression)| {
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            let coordinate = coordinates
                .expression(expression)
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
            Ok(CheckedImplicitParameterOccurrence::new(
                expression, coordinate, ordinal,
            ))
        })
        .collect::<Result<Vec<_>, FinalSemanticAnalysisError>>()?;
    if parameter_occurrences.is_empty() {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }

    let expected_uses = module
        .expression_uses()
        .rows()
        .iter()
        .filter(|row| region.contains_expression(row.expression()))
        .map(|row| {
            let expression = row.expression();
            let fact = expressions.get(&expression).ok_or(
                FinalSemanticAnalysisError::CaptureAuthority {
                    violation: super::CheckedCaptureAuthorityViolation::MissingExpressionUse {
                        expression,
                    },
                },
            )?;
            let Some(local) = fact.execution_local_use() else {
                // Non-local execution facts, including compile-time scalars,
                // intentionally contribute no capture ledger row.
                return Ok(None);
            };
            let binding = module.local_origins().binding(local).ok_or(
                FinalSemanticAnalysisError::CaptureAuthority {
                    violation: super::CheckedCaptureAuthorityViolation::MissingLocalBinding {
                        local,
                    },
                },
            )?;
            Ok((!region.contains_binding(binding)).then_some((expression, local)))
        })
        .collect::<Result<Vec<_>, FinalSemanticAnalysisError>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if pending_uses != expected_uses.as_slice() {
        return Err(FinalSemanticAnalysisError::CaptureAuthority {
            violation: super::CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch,
        });
    }

    let uses = expected_uses
        .iter()
        .map(|(expression, local)| {
            let row = module
                .expression_uses()
                .row(*expression)
                .filter(|_| region.contains_expression(*expression))
                .ok_or(FinalSemanticAnalysisError::CaptureAuthority {
                    violation: super::CheckedCaptureAuthorityViolation::MissingExpressionUse {
                        expression: *expression,
                    },
                })?;
            Ok((
                row.source_ordinal(),
                *expression,
                *local,
                row.capture_access(),
            ))
        })
        .collect::<Result<Vec<_>, FinalSemanticAnalysisError>>()?;

    let mut capture_occurrences = Vec::with_capacity(uses.len());
    let mut captures = Vec::new();
    let mut capture_indices = BTreeMap::<LocalId, usize>::new();
    for (ordinal, (_, expression, local, access)) in uses.into_iter().enumerate() {
        let ordinal =
            u32::try_from(ordinal).map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let value_type = local_types
            .get(&local)
            .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
        let coordinate = coordinates
            .expression(expression)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let origin = coordinates
            .binding(local)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        let occurrence = CheckedImplicitCaptureOccurrence::new(
            expression,
            local,
            coordinate,
            origin.clone(),
            value_type.semantic_identity_digest()?,
            access,
            ordinal,
        );
        capture_occurrences.push(occurrence);
        if let Some(index) = capture_indices.get(&local).copied() {
            if access == arcweft_lang_hir::scope::CaptureAccess::Reassign {
                captures[index] = CheckedImplicitCapture::new(
                    local,
                    origin,
                    value_type.semantic_identity_digest()?,
                    access,
                );
            }
        } else {
            capture_indices.insert(local, captures.len());
            captures.push(CheckedImplicitCapture::new(
                local,
                origin,
                value_type.semantic_identity_digest()?,
                access,
            ));
        }
    }

    let TypeKind::Function {
        params,
        return_type,
        ..
    } = prepared_function_type(prepared)
    else {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    };
    if params.len() != 1 || params[0] != *prepared.parameter() || *return_type != *prepared.result()
    {
        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
    }
    Ok(PreparedImplicitCallableSealSeed {
        owner,
        function_type,
        coordinate: owner_coordinate,
        parameter: prepared.parameter().clone(),
        result: prepared.result().clone(),
        parameter_occurrences: parameter_occurrences.into_boxed_slice(),
        capture_occurrences: capture_occurrences.into_boxed_slice(),
        captures: captures.into_boxed_slice(),
        body: prepared.body().clone(),
    })
}

fn seal_callable_seed(
    topology: &Arc<HirProjectEvaluationTopology>,
    seed: &PreparedImplicitCallableSealSeed,
    evidence: CheckedImplicitCallableIdentityEvidence,
    expressions: &BTreeMap<ExprId, PreparedExpressionFact>,
    pipes: &BTreeMap<ExprId, CheckedPipe>,
    callable_identities: &BTreeMap<ExprId, super::CheckedImplicitCallableIdentity>,
    checked_callables: &crate::callable::CheckedCallableCatalog,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    structural_edges: &super::match_edges::CheckedStructuralEdgeDraft,
) -> Result<CheckedImplicitCallable, FinalSemanticAnalysisError> {
    let body = match &seed.body {
        PreparedImplicitCallableBody::Complete(body) => {
            CheckedImplicitCallableBody::Plain(Box::new(body.resolution().clone()))
        }
        PreparedImplicitCallableBody::OwnerBound(owner_bound) => {
            let resolution = seal_owner_bound_body(
                topology,
                seed.owner,
                owner_bound,
                expressions,
                pipes,
                callable_identities,
                checked_callables,
                coordinates,
                structural_edges,
            )?;
            match resolution {
                CheckedExpressionResolution::Try(tried) => CheckedImplicitCallableBody::Try(tried),
                CheckedExpressionResolution::Pipe(pipe) => CheckedImplicitCallableBody::Pipe(pipe),
                resolution => CheckedImplicitCallableBody::Plain(Box::new(resolution)),
            }
        }
    };
    evidence.finalize(body).map_err(Into::into)
}

fn prepared_function_type(prepared: &PreparedImplicitCallable) -> TypeKind {
    // The enclosing shell is checked by the caller.  Returning the function
    // shape here keeps the body/type join in the same owner-bound phase.
    TypeKind::function_with_effects(
        [prepared.parameter().clone()],
        prepared.result().clone(),
        crate::effect_row::EffectRow::closed(match prepared.body() {
            PreparedImplicitCallableBody::Complete(body) => body.effects().clone(),
            PreparedImplicitCallableBody::OwnerBound(body) => body.effects().clone(),
        }),
    )
}
