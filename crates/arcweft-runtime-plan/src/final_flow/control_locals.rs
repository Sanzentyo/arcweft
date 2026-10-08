//! Control temporaries admitted from one closed executable semantic view.

use std::collections::BTreeMap;

use arcweft_core::plan::{RuntimeLocalDeclarationSeed, RuntimeLocalSeedId, RuntimePlanBuilder};
use arcweft_lang_hir::identity::ExprId;

use crate::errors::RuntimePlanLowerError;
use crate::semantic_facts::{
    RuntimeExecutableSemanticFactView, RuntimeNormalizedType, RuntimeScopeOwner,
};

use super::{AwaitLocalSeeds, ScopeLocalSeeds, TryLocalSeeds};
use arcweft_lang_sema::semantic_coordinate::{
    CheckedGeneratedLocalRole as Role, CheckedSemanticPath,
};

#[derive(Clone, Default)]
pub(super) struct ControlLocals {
    pub(super) awaits: BTreeMap<ExprId, AwaitLocalSeeds>,
    pub(super) tries: BTreeMap<ExprId, TryLocalSeeds>,
    pub(super) pipes: BTreeMap<ExprId, RuntimeLocalSeedId>,
    pub(super) expression_values: BTreeMap<ExprId, RuntimeLocalSeedId>,
    pub(super) guard_values: BTreeMap<ExprId, RuntimeLocalSeedId>,
    pub(super) expression_final_values: BTreeMap<ExprId, RuntimeLocalSeedId>,
    pub(super) scopes: BTreeMap<RuntimeScopeOwner, ScopeLocalSeeds>,
}

enum ControlLocal {
    Await,
    Try { residual: bool },
    Pipe,
    ExpressionValue,
    ExpressionFinalValue,
    GuardValue,
}

impl ControlLocals {
    #[allow(
        clippy::too_many_lines,
        reason = "one control-local owner allocates and checks its complete typed temporary inventory"
    )]
    pub(super) fn admit(
        facts: RuntimeExecutableSemanticFactView<'_>,
        function: Option<&RuntimeNormalizedType>,
        builder: &mut RuntimePlanBuilder,
    ) -> Result<Self, RuntimePlanLowerError> {
        let declaration =
            |ty: &RuntimeNormalizedType, coordinate: Option<&CheckedSemanticPath>, role: Role| {
                let origin = coordinate
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(
                            "generated local has no accepted structural coordinate",
                        )
                    })?
                    .runtime_generated_local_source(role)
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
                Ok::<_, RuntimePlanLowerError>(match function.filter(|_| !ty.scope().is_root()) {
                    Some(function) => RuntimeLocalDeclarationSeed::in_function(
                        origin,
                        ty.identity(),
                        function.identity(),
                    ),
                    None => RuntimeLocalDeclarationSeed::new(origin, ty.identity()),
                })
            };
        let mut owners = Vec::new();
        let mut seeds = Vec::new();
        let mut error = None;
        facts.visit_runtime_expression_types(&mut |owner, ty| {
            let coordinate = facts.expression_coordinate(owner);
            let expression_value_type = facts.expression_source_type(owner).unwrap_or(ty);
            owners.push((owner, ControlLocal::ExpressionValue));
            seeds.push(declaration(
                expression_value_type,
                coordinate,
                Role::ExpressionSource,
            ));
            if expression_value_type.identity() != ty.identity() {
                owners.push((owner, ControlLocal::ExpressionFinalValue));
                seeds.push(declaration(ty, coordinate, Role::ExpressionResult));
            }
            if facts.is_pattern_guard(owner)
                && !matches!(ty.shape(), crate::semantic_facts::RuntimeTypeShape::Never)
            {
                owners.push((owner, ControlLocal::GuardValue));
                seeds.push(declaration(ty, coordinate, Role::GuardValue));
            }
            if facts.awaited(owner).is_some() {
                owners.push((owner, ControlLocal::Await));
                seeds.push(declaration(ty, coordinate, Role::AwaitPayload));
            }
            if let Some(tried) = facts.tried(owner) {
                let residual = tried.carrier().residual();
                owners.push((
                    owner,
                    ControlLocal::Try {
                        residual: residual.is_some(),
                    },
                ));
                seeds.push(declaration(
                    tried.carrier().success(),
                    coordinate,
                    Role::TrySuccess,
                ));
                seeds.extend(residual.map(|ty| declaration(ty, coordinate, Role::TryResidual)));
            }
            if let Some(pipe) = facts.pipe(owner) {
                if let Some(left) = facts.expression_type(pipe.left()) {
                    owners.push((owner, ControlLocal::Pipe));
                    seeds.push(declaration(left, coordinate, Role::PipeOperand));
                } else {
                    error.get_or_insert_with(|| {
                        RuntimePlanLowerError::new(format!(
                            "pipe {owner:?} has no closed left operand type"
                        ))
                    });
                }
            }
        });
        facts.visit_untyped_evaluated_effect_pipes(&mut |owner, pipe| {
            if let Some(left) = facts.expression_type(pipe.left()) {
                owners.push((owner, ControlLocal::Pipe));
                seeds.push(declaration(
                    left,
                    facts.expression_coordinate(owner),
                    Role::PipeOperand,
                ));
            } else {
                error.get_or_insert_with(|| {
                    RuntimePlanLowerError::new(format!(
                        "pipe {owner:?} has no closed left operand type"
                    ))
                });
            }
        });
        if let Some(error) = error {
            return Err(error);
        }
        let admission = builder
            .admit_type_batch([], seeds.into_iter().collect::<Result<Vec<_>, _>>()?)
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let mut admitted = admission.local_ids().iter().cloned();
        let mut result = Self::default();
        for (owner, kind) in owners {
            let local = admitted.next().ok_or_else(|| {
                RuntimePlanLowerError::new("control temporary admission omitted a local")
            })?;
            let duplicate = match kind {
                ControlLocal::Await => result
                    .awaits
                    .insert(owner, AwaitLocalSeeds { payload: local })
                    .is_some(),
                ControlLocal::Try { residual } => {
                    let residual = if residual {
                        Some(admitted.next().ok_or_else(|| {
                            RuntimePlanLowerError::new(
                                "Try temporary admission omitted its residual local",
                            )
                        })?)
                    } else {
                        None
                    };
                    result
                        .tries
                        .insert(
                            owner,
                            TryLocalSeeds {
                                success: local,
                                residual,
                            },
                        )
                        .is_some()
                }
                ControlLocal::Pipe => result.pipes.insert(owner, local).is_some(),
                ControlLocal::GuardValue => result.guard_values.insert(owner, local).is_some(),
                ControlLocal::ExpressionValue => {
                    result.expression_values.insert(owner, local).is_some()
                }
                ControlLocal::ExpressionFinalValue => result
                    .expression_final_values
                    .insert(owner, local)
                    .is_some(),
            };
            if duplicate {
                return Err(RuntimePlanLowerError::new(
                    "control temporary has more than one owner",
                ));
            }
        }
        if admitted.next().is_some() {
            return Err(RuntimePlanLowerError::new(
                "control temporary admission retained an extra local",
            ));
        }
        let mut scope_owners = Vec::new();
        let mut scope_seeds = Vec::new();
        facts.visit_scope_continuations(&mut |owner, continuation| {
            let coordinate = match owner {
                RuntimeScopeOwner::Expression(owner) => facts.expression_scope(owner),
                RuntimeScopeOwner::Statement(owner) => facts.statement_scope(owner),
            }
            .map(|fact| fact.origin().coordinate());
            scope_owners.push((owner, continuation.residual_type().is_some()));
            scope_seeds.extend([
                declaration(continuation.carrier_type(), coordinate, Role::ScopeCarrier),
                declaration(continuation.value_type(), coordinate, Role::ScopeSuccess),
            ]);
            scope_seeds.extend(
                continuation
                    .residual_type()
                    .map(|ty| declaration(ty, coordinate, Role::ScopeResidual)),
            );
        });
        let admission = builder
            .admit_type_batch([], scope_seeds.into_iter().collect::<Result<Vec<_>, _>>()?)
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let mut admitted = admission.local_ids().iter().cloned();
        let missing = || RuntimePlanLowerError::new("scope continuation admission omitted a local");
        for (owner, has_residual) in scope_owners {
            let carrier = admitted.next().ok_or_else(missing)?;
            let success = admitted.next().ok_or_else(missing)?;
            let residual = has_residual
                .then(|| admitted.next().ok_or_else(missing))
                .transpose()?;
            if result
                .scopes
                .insert(
                    owner,
                    ScopeLocalSeeds {
                        carrier,
                        success,
                        residual,
                    },
                )
                .is_some()
            {
                return Err(RuntimePlanLowerError::new(
                    "scope continuation has duplicate local ownership",
                ));
            }
        }
        if admitted.next().is_some() {
            return Err(RuntimePlanLowerError::new(
                "scope continuation admission retained an extra local",
            ));
        }
        Ok(result)
    }
}
