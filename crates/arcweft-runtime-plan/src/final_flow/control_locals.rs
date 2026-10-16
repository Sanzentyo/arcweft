//! Generated locals for one closed executable view. Repeated Map rows remain
//! borrowed requests until their complete inventory passes the Core issuer.
use super::{AwaitLocalSeeds, ScopeLocalSeeds, TryLocalSeeds};
use crate::errors::RuntimePlanLowerError;
use crate::semantic_facts::{
    RuntimeBuiltinIteratorFact, RuntimeExecutableSemanticFactView, RuntimeNormalizedType,
    RuntimeScopeOwner,
};
use arcweft_core::plan::{RuntimeLocalDeclarationSeed, RuntimeLocalSeedId, RuntimePlanBuilder};
use arcweft_lang_hir::identity::ExprId;
use arcweft_lang_sema::semantic_coordinate::{
    CheckedGeneratedLocalRole as Role, CheckedSemanticPath,
};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(super) struct ControlLocals {
    pub(super) awaits: BTreeMap<ExprId, AwaitLocalSeeds>,
    pub(super) tries: BTreeMap<ExprId, TryLocalSeeds>,
    pub(super) pipes: BTreeMap<ExprId, RuntimeLocalSeedId>,
    pub(super) expression_values: BTreeMap<ExprId, RuntimeLocalSeedId>,
    pub(super) guard_values: BTreeMap<ExprId, RuntimeLocalSeedId>,
    pub(super) expression_final_values: BTreeMap<ExprId, RuntimeLocalSeedId>,
    pub(super) scopes: BTreeMap<RuntimeScopeOwner, ScopeLocalSeeds>,
    pub(super) maps: BTreeMap<ExprId, MapLocalSeeds>,
}
#[derive(Clone, Default)]
pub(super) struct MapLocalSeeds {
    pub(super) callable: Option<RuntimeLocalSeedId>,
    pub(super) items: Vec<RuntimeLocalSeedId>,
    pub(super) results: Vec<RuntimeLocalSeedId>,
    pub(super) iterator: Option<RuntimeLocalSeedId>,
    pub(super) next_iterator: Option<RuntimeLocalSeedId>,
    pub(super) residual: Option<RuntimeLocalSeedId>,
}
#[derive(Clone, Copy)]
enum ValueRole {
    Await,
    Pipe,
    Expression,
    FinalExpression,
    Guard,
}
impl ValueRole {
    const fn semantic_role(self) -> Role {
        match self {
            Self::Await => Role::AwaitPayload,
            Self::Pipe => Role::PipeOperand,
            Self::Expression => Role::ExpressionSource,
            Self::FinalExpression => Role::ExpressionResult,
            Self::Guard => Role::GuardValue,
        }
    }
}
enum RequestKind<'a> {
    Value {
        owner: ExprId,
        role: ValueRole,
        ty: &'a RuntimeNormalizedType,
    },
    Try {
        owner: ExprId,
        success: &'a RuntimeNormalizedType,
        residual: Option<&'a RuntimeNormalizedType>,
    },
    Map {
        owner: ExprId,
        callable: &'a RuntimeNormalizedType,
        input: &'a RuntimeNormalizedType,
        output: &'a RuntimeNormalizedType,
        rows: u32,
        iterator: Option<&'a RuntimeNormalizedType>,
        residual: Option<&'a RuntimeNormalizedType>,
    },
    Scope {
        owner: RuntimeScopeOwner,
        carrier: &'a RuntimeNormalizedType,
        success: &'a RuntimeNormalizedType,
        residual: Option<&'a RuntimeNormalizedType>,
    },
}
struct LocalRequest<'a> {
    coordinate: &'a CheckedSemanticPath,
    kind: RequestKind<'a>,
}
impl LocalRequest<'_> {
    const fn owner(&self) -> RuntimeScopeOwner {
        match &self.kind {
            RequestKind::Value { owner, .. }
            | RequestKind::Try { owner, .. }
            | RequestKind::Map { owner, .. } => RuntimeScopeOwner::Expression(*owner),
            RequestKind::Scope { owner, .. } => *owner,
        }
    }
    const fn first_role(&self) -> Role {
        match &self.kind {
            RequestKind::Value { role, .. } => role.semantic_role(),
            RequestKind::Try { .. } => Role::TrySuccess,
            RequestKind::Map { .. } => Role::MapCallable,
            RequestKind::Scope { .. } => Role::ScopeCarrier,
        }
    }
    fn count(&self) -> u64 {
        match &self.kind {
            RequestKind::Value { .. } => 1,
            RequestKind::Try { residual, .. } => 1 + u64::from(residual.is_some()),
            RequestKind::Scope { residual, .. } => 2 + u64::from(residual.is_some()),
            RequestKind::Map {
                rows,
                iterator,
                residual,
                ..
            } => {
                1 + u64::from(*rows) * 2
                    + 2 * u64::from(iterator.is_some())
                    + u64::from(residual.is_some())
            }
        }
    }
    fn visit_declarations(
        &self,
        visit: &mut impl FnMut(&RuntimeNormalizedType, Role) -> Result<(), RuntimePlanLowerError>,
    ) -> Result<(), RuntimePlanLowerError> {
        match &self.kind {
            RequestKind::Value { role, ty, .. } => visit(ty, role.semantic_role())?,
            RequestKind::Try {
                success, residual, ..
            } => {
                visit(success, Role::TrySuccess)?;
                if let Some(ty) = residual {
                    visit(ty, Role::TryResidual)?;
                }
            }
            RequestKind::Map {
                callable,
                input,
                output,
                rows,
                iterator,
                residual,
                ..
            } => {
                visit(callable, Role::MapCallable)?;
                for ordinal in 0..*rows {
                    visit(input, Role::MapItem { ordinal })?;
                    visit(output, Role::MapResult { ordinal })?;
                }
                if let Some(ty) = iterator {
                    visit(ty, Role::MapIterator)?;
                    visit(ty, Role::MapNextIterator)?;
                }
                if let Some(ty) = residual {
                    visit(ty, Role::MapResidual)?;
                }
            }
            RequestKind::Scope {
                carrier,
                success,
                residual,
                ..
            } => {
                visit(carrier, Role::ScopeCarrier)?;
                visit(success, Role::ScopeSuccess)?;
                if let Some(ty) = residual {
                    visit(ty, Role::ScopeResidual)?;
                }
            }
        }
        Ok(())
    }
    fn bind(
        &self,
        result: &mut ControlLocals,
        admitted: &mut impl Iterator<Item = RuntimeLocalSeedId>,
    ) -> Result<(), RuntimePlanLowerError> {
        let mut next = || {
            admitted.next().ok_or_else(|| {
                RuntimePlanLowerError::new("control temporary admission omitted a local")
            })
        };
        match &self.kind {
            RequestKind::Value { owner, role, .. } => {
                let local = next()?;
                match role {
                    ValueRole::Await => {
                        result
                            .awaits
                            .insert(*owner, AwaitLocalSeeds { payload: local });
                    }
                    ValueRole::Pipe => {
                        result.pipes.insert(*owner, local);
                    }
                    ValueRole::Expression => {
                        result.expression_values.insert(*owner, local);
                    }
                    ValueRole::FinalExpression => {
                        result.expression_final_values.insert(*owner, local);
                    }
                    ValueRole::Guard => {
                        result.guard_values.insert(*owner, local);
                    }
                }
            }
            RequestKind::Try {
                owner, residual, ..
            } => {
                result.tries.insert(
                    *owner,
                    TryLocalSeeds {
                        success: next()?,
                        residual: residual.is_some().then(&mut next).transpose()?,
                    },
                );
            }
            RequestKind::Map {
                owner,
                rows,
                iterator,
                residual,
                ..
            } => {
                let mut map = MapLocalSeeds {
                    callable: Some(next()?),
                    ..MapLocalSeeds::default()
                };
                for _ in 0..*rows {
                    map.items.push(next()?);
                    map.results.push(next()?);
                }
                if iterator.is_some() {
                    map.iterator = Some(next()?);
                    map.next_iterator = Some(next()?);
                }
                map.residual = residual.is_some().then(&mut next).transpose()?;
                result.maps.insert(*owner, map);
            }
            RequestKind::Scope {
                owner, residual, ..
            } => {
                result.scopes.insert(
                    *owner,
                    ScopeLocalSeeds {
                        carrier: next()?,
                        success: next()?,
                        residual: residual.is_some().then(&mut next).transpose()?,
                    },
                );
            }
        }
        Ok(())
    }
}
/// A compact, unique inventory; only admitted requests may expand into seeds.
struct ControlLocalInventory<'a> {
    requests: Vec<LocalRequest<'a>>,
    count: u64,
}
impl<'a> ControlLocalInventory<'a> {
    fn from_facts(
        facts: RuntimeExecutableSemanticFactView<'a>,
    ) -> Result<Self, RuntimePlanLowerError> {
        let mut inventory = Self {
            requests: Vec::new(),
            count: 0,
        };
        let mut checked = Ok(());
        facts.visit_runtime_expression_types(&mut |owner, ty| {
            if checked.is_ok() {
                checked = inventory.expression(facts, owner, ty);
            }
        });
        checked?;
        let mut checked = Ok(());
        facts.visit_untyped_evaluated_effect_pipes(&mut |owner, pipe| {
            if checked.is_err() {
                return;
            }
            checked = (|| {
                let left = facts.expression_type(pipe.left()).ok_or_else(|| {
                    RuntimePlanLowerError::new(format!(
                        "pipe {owner:?} has no closed left operand type"
                    ))
                })?;
                inventory.value(facts, owner, left, ValueRole::Pipe)
            })();
        });
        checked?;
        let mut checked = Ok(());
        facts.visit_scope_continuations(&mut |owner, continuation| {
            if checked.is_err() {
                return;
            }
            checked = (|| {
                let coordinate = match owner {
                    RuntimeScopeOwner::Expression(owner) => facts.expression_scope(owner),
                    RuntimeScopeOwner::Statement(owner) => facts.statement_scope(owner),
                }
                .map(|fact| fact.origin().coordinate())
                .ok_or_else(|| {
                    RuntimePlanLowerError::new(
                        "scope continuation has no accepted structural coordinate",
                    )
                })?;
                inventory.requests.push(LocalRequest {
                    coordinate,
                    kind: RequestKind::Scope {
                        owner,
                        carrier: continuation.carrier_type(),
                        success: continuation.value_type(),
                        residual: continuation.residual_type(),
                    },
                });
                Ok(())
            })();
        });
        checked?;
        let mut owners = BTreeMap::<RuntimeScopeOwner, Vec<Role>>::new();
        for request in &inventory.requests {
            let roles = owners.entry(request.owner()).or_default();
            let role = request.first_role();
            if roles.contains(&role) {
                return Err(RuntimePlanLowerError::new(
                    "control temporary has more than one owner",
                ));
            }
            roles.push(role);
            inventory.count = inventory
                .count
                .checked_add(request.count())
                .ok_or_else(|| {
                    RuntimePlanLowerError::new("control temporary inventory count overflow")
                })?;
        }
        Ok(inventory)
    }
    fn value(
        &mut self,
        facts: RuntimeExecutableSemanticFactView<'a>,
        owner: ExprId,
        ty: &'a RuntimeNormalizedType,
        role: ValueRole,
    ) -> Result<(), RuntimePlanLowerError> {
        let coordinate = facts.expression_coordinate(owner).ok_or_else(|| {
            RuntimePlanLowerError::new("generated local has no accepted structural coordinate")
        })?;
        self.requests.push(LocalRequest {
            coordinate,
            kind: RequestKind::Value { owner, role, ty },
        });
        Ok(())
    }
    fn expression(
        &mut self,
        facts: RuntimeExecutableSemanticFactView<'a>,
        owner: ExprId,
        ty: &'a RuntimeNormalizedType,
    ) -> Result<(), RuntimePlanLowerError> {
        let source = facts.expression_source_type(owner).unwrap_or(ty);
        self.value(facts, owner, source, ValueRole::Expression)?;
        if source.identity() != ty.identity() {
            self.value(facts, owner, ty, ValueRole::FinalExpression)?;
        }
        if facts.is_pattern_guard(owner)
            && !matches!(ty.shape(), crate::semantic_facts::RuntimeTypeShape::Never)
        {
            self.value(facts, owner, ty, ValueRole::Guard)?;
        }
        if facts.awaited(owner).is_some() {
            self.value(facts, owner, ty, ValueRole::Await)?;
        }
        let coordinate = facts.expression_coordinate(owner).ok_or_else(|| {
            RuntimePlanLowerError::new("generated local has no accepted structural coordinate")
        })?;
        if let Some(tried) = facts.tried(owner) {
            self.requests.push(LocalRequest {
                coordinate,
                kind: RequestKind::Try {
                    owner,
                    success: tried.carrier().success(),
                    residual: tried.carrier().residual(),
                },
            });
        }
        if let Some(pipe) = facts.pipe(owner) {
            let left = facts.expression_type(pipe.left()).ok_or_else(|| {
                RuntimePlanLowerError::new(format!(
                    "pipe {owner:?} has no closed left operand type"
                ))
            })?;
            self.value(facts, owner, left, ValueRole::Pipe)?;
        }
        if let Some(call) = facts.call(owner)
            && let crate::semantic_facts::RuntimeResolvedCallDispatch::Static(
                crate::semantic_facts::RuntimeResolvedStaticCallTarget::StandardMap(map),
            ) = call.dispatch()
        {
            let receiver = facts
                .expression_source_type(map.receiver())
                .ok_or_else(|| RuntimePlanLowerError::new("Map receiver has no checked type"))?;
            let callable = facts
                .expression_source_type(map.mapping())
                .ok_or_else(|| RuntimePlanLowerError::new("Map callback has no checked type"))?;
            let (input, output) = map.item_types(receiver, source).ok_or_else(|| {
                RuntimePlanLowerError::new("Map family has inconsistent checked types")
            })?;
            let rows = if let crate::semantic_facts::RuntimeTypeShape::Array { length, .. } =
                receiver.shape()
            {
                length
                    .constant()
                    .and_then(|length| u32::try_from(length).ok())
                    .ok_or_else(|| {
                        RuntimePlanLowerError::new(
                            "Map Array length does not fit its generated local inventory",
                        )
                    })?
            } else {
                1
            };
            let residual = match receiver.shape() {
                crate::semantic_facts::RuntimeTypeShape::Result { error, .. } => {
                    Some(error.as_ref())
                }
                _ => None,
            };
            self.requests.push(LocalRequest {
                coordinate,
                kind: RequestKind::Map {
                    owner,
                    callable,
                    input,
                    output,
                    rows,
                    iterator: map.iteration().map(RuntimeBuiltinIteratorFact::iterator),
                    residual,
                },
            });
        }
        Ok(())
    }
    fn declarations(
        &self,
        function: Option<&RuntimeNormalizedType>,
    ) -> Result<Vec<RuntimeLocalDeclarationSeed>, RuntimePlanLowerError> {
        let mut seeds = Vec::new();
        let count = usize::try_from(self.count).map_err(|_| {
            RuntimePlanLowerError::new(
                "admitted control temporary count does not fit allocation sizes",
            )
        })?;
        seeds.try_reserve_exact(count).map_err(|error| {
            RuntimePlanLowerError::new(format!(
                "control temporary storage admission failed: {error}"
            ))
        })?;
        for request in &self.requests {
            request.visit_declarations(&mut |ty, role| {
                let origin = request
                    .coordinate
                    .runtime_generated_local_source(role)
                    .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
                seeds.push(match function.filter(|_| !ty.scope().is_root()) {
                    Some(function) => RuntimeLocalDeclarationSeed::in_function(
                        origin,
                        ty.identity(),
                        function.identity(),
                    ),
                    None => RuntimeLocalDeclarationSeed::new(origin, ty.identity()),
                });
                Ok(())
            })?;
        }
        if seeds.len() != count {
            return Err(RuntimePlanLowerError::new(
                "control temporary declaration count disagrees with its inventory",
            ));
        }
        Ok(seeds)
    }
}
impl ControlLocals {
    pub(super) fn admit(
        facts: RuntimeExecutableSemanticFactView<'_>,
        function: Option<&RuntimeNormalizedType>,
        builder: &mut RuntimePlanBuilder,
    ) -> Result<Self, RuntimePlanLowerError> {
        let inventory = ControlLocalInventory::from_facts(facts)?;
        builder
            .check_local_request_capacity(inventory.count)
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let declarations = inventory.declarations(function)?;
        let admission = builder
            .admit_type_batch(
                [arcweft_core::plan::RuntimePlanTypeSeed::new(
                    arcweft_core::pattern::RuntimeCheckedType::Unit.semantic_identity_digest(),
                    arcweft_core::plan::RuntimePlanTypeProjection::Unit,
                )],
                declarations,
            )
            .map_err(|error| RuntimePlanLowerError::new(error.to_string()))?;
        let mut admitted = admission.local_ids().iter().cloned();
        let mut result = Self::default();
        for request in &inventory.requests {
            request.bind(&mut result, &mut admitted)?;
        }
        if admitted.next().is_some() {
            return Err(RuntimePlanLowerError::new(
                "control temporary admission retained an extra local",
            ));
        }
        Ok(result)
    }
}
