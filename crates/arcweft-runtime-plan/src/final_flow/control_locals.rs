//! Control temporaries admitted from one closed executable semantic view.

use std::collections::BTreeMap;

use arcweft_core::plan::{RuntimeLocalDeclarationSeed, RuntimeLocalSeedId, RuntimePlanBuilder};
use arcweft_lang_hir::identity::ExprId;

use crate::errors::RuntimePlanLowerError;
use crate::semantic_facts::{RuntimeExecutableSemanticFactView, RuntimeScopeOwner};

use super::{AwaitLocalSeeds, ScopeLocalSeeds, TryLocalSeeds};

#[derive(Clone, Default)]
pub(super) struct ControlLocals {
    pub(super) awaits: BTreeMap<ExprId, AwaitLocalSeeds>,
    pub(super) tries: BTreeMap<ExprId, TryLocalSeeds>,
    pub(super) pipes: BTreeMap<ExprId, RuntimeLocalSeedId>,
    pub(super) expression_values: BTreeMap<ExprId, RuntimeLocalSeedId>,
    pub(super) scopes: BTreeMap<RuntimeScopeOwner, ScopeLocalSeeds>,
}

enum ControlLocal {
    Await,
    Try { residual: bool },
    Pipe,
    ExpressionValue,
}

impl ControlLocals {
    #[allow(
        clippy::too_many_lines,
        reason = "one control-local owner allocates and checks its complete typed temporary inventory"
    )]
    pub(super) fn admit(
        facts: RuntimeExecutableSemanticFactView<'_>,
        builder: &mut RuntimePlanBuilder,
    ) -> Result<Self, RuntimePlanLowerError> {
        let mut owners = Vec::new();
        let mut seeds = Vec::new();
        let mut error = None;
        facts.visit_runtime_expression_types(&mut |owner, ty| {
            owners.push((owner, ControlLocal::ExpressionValue));
            seeds.push(RuntimeLocalDeclarationSeed::new(ty.identity()));
            if facts.awaited(owner).is_some() {
                owners.push((owner, ControlLocal::Await));
                seeds.push(RuntimeLocalDeclarationSeed::new(ty.identity()));
            }
            if let Some(tried) = facts.tried(owner) {
                let residual = tried.carrier().residual();
                owners.push((
                    owner,
                    ControlLocal::Try {
                        residual: residual.is_some(),
                    },
                ));
                seeds.push(RuntimeLocalDeclarationSeed::new(
                    tried.carrier().success().identity(),
                ));
                seeds.extend(residual.map(|ty| RuntimeLocalDeclarationSeed::new(ty.identity())));
            }
            if let Some(pipe) = facts.pipe(owner) {
                if let Some(left) = facts.expression_type(pipe.left()) {
                    owners.push((owner, ControlLocal::Pipe));
                    seeds.push(RuntimeLocalDeclarationSeed::new(left.identity()));
                } else {
                    error.get_or_insert_with(|| {
                        RuntimePlanLowerError::new(format!(
                            "pipe {owner:?} has no closed left operand type"
                        ))
                    });
                }
            }
        });
        if let Some(error) = error {
            return Err(error);
        }
        let admission = builder
            .admit_type_batch([], seeds)
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
                ControlLocal::ExpressionValue => {
                    result.expression_values.insert(owner, local).is_some()
                }
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
            scope_owners.push((owner, continuation.residual_type().is_some()));
            scope_seeds.extend([
                RuntimeLocalDeclarationSeed::new(continuation.carrier_type().identity()),
                RuntimeLocalDeclarationSeed::new(continuation.value_type().identity()),
            ]);
            scope_seeds.extend(
                continuation
                    .residual_type()
                    .map(|ty| RuntimeLocalDeclarationSeed::new(ty.identity())),
            );
        });
        let admission = builder
            .admit_type_batch([], scope_seeds)
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
