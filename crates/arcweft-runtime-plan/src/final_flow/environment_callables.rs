//! Executable declaration frames for accepted external function values.

use super::callable_states;
use crate::{
    flow::RuntimePlanLowerError,
    semantic_facts::{
        RuntimeNormalizedType, RuntimePlanSemanticFacts, RuntimeResolvedValue, RuntimeTypeShape,
    },
};
use arcweft_core::plan::{
    RuntimeCallArgumentSeed, RuntimeCallableAttachedContract, RuntimeCallableParameterCoordinate,
    RuntimeCallableParameterInput, RuntimeCallableParameterKind, RuntimeCallableStateSeedId,
    RuntimeEffectSet, RuntimeExecutableBodySeed, RuntimeExprSeed, RuntimeExprSeedKind,
    RuntimeFlowOpSeed, RuntimeFunctionDefinitionIdentity, RuntimeFunctionInputBindingSeed,
    RuntimeFunctionInputOrigin, RuntimeFunctionInputSource, RuntimeFunctionInputTransfer,
    RuntimeFunctionParameterPassing, RuntimeFunctionSemanticRole, RuntimeFunctionSiteBodyKind,
    RuntimeFunctionSiteBodySeed, RuntimeFunctionSiteDeclarationSeed, RuntimeLocalDeclarationSeed,
    RuntimeLocalReadSeed, RuntimeLocalSeedId, RuntimePatternSeed, RuntimePatternSeedKind,
    RuntimePlanBuilder, RuntimeValueOwnershipRequirement,
};
use arcweft_core::value::{RuntimeCallArgumentMode, RuntimeLocalReadMode};
use arcweft_lang_sema::{
    callable::{CallableParameterPassing, CallableValidator, CheckedCallableDigest},
    final_analysis::CheckedEnvironmentCallableValueProjection,
    types::SemanticTypeDigest,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct EnvironmentCallableKey {
    declaration: CheckedCallableDigest,
    function_type: SemanticTypeDigest,
}

impl EnvironmentCallableKey {
    pub(crate) fn of(value: &CheckedEnvironmentCallableValueProjection) -> Self {
        Self {
            declaration: value.declaration().id().semantic_digest(),
            function_type: value.type_identity(),
        }
    }
}

pub(crate) type EnvironmentCallableStates =
    BTreeMap<EnvironmentCallableKey, Box<[RuntimeCallableStateSeedId]>>;
pub(super) type EnvironmentCallableLocals =
    BTreeMap<EnvironmentCallableKey, Box<[RuntimeLocalSeedId]>>;

struct EnvironmentParameter<'a> {
    coordinate: RuntimeCallableParameterCoordinate,
    ty: &'a RuntimeNormalizedType,
}

struct EnvironmentDefinition<'a> {
    value: &'a CheckedEnvironmentCallableValueProjection,
    ty: &'a RuntimeNormalizedType,
    parameters: Vec<EnvironmentParameter<'a>>,
    terminal: usize,
    result: &'a RuntimeNormalizedType,
    effects: RuntimeEffectSet,
}

pub(super) struct EnvironmentCallableDefinitions<'a> {
    definitions: BTreeMap<EnvironmentCallableKey, EnvironmentDefinition<'a>>,
}

impl<'a> EnvironmentCallableDefinitions<'a> {
    pub(super) fn prepare(
        facts: &'a RuntimePlanSemanticFacts,
    ) -> Result<Self, RuntimePlanLowerError> {
        let mut definitions = BTreeMap::new();
        let mut failure = None;
        facts.visit_callable_values(&mut |_, value, ty| {
            let RuntimeResolvedValue::EnvironmentCallable(value) = value else {
                return;
            };
            let key = EnvironmentCallableKey::of(value);
            if definitions.contains_key(&key) || failure.is_some() {
                return;
            }
            match ty
                .ok_or_else(|| {
                    RuntimePlanLowerError::new("environment callable has no normalized source type")
                })
                .and_then(|ty| EnvironmentDefinition::prepare(value, ty))
            {
                Ok(definition) => {
                    definitions.insert(key, definition);
                }
                Err(error) => failure = Some(error),
            }
        });
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(Self { definitions })
    }

    pub(super) fn local_seeds(&self) -> impl Iterator<Item = RuntimeLocalDeclarationSeed> + '_ {
        self.definitions.values().flat_map(|definition| {
            let identity = definition.identity();
            definition
                .parameters
                .iter()
                .zip(0_u32..)
                .map(move |(parameter, position)| {
                    RuntimeLocalDeclarationSeed::new(
                        arcweft_core::plan::RuntimeLocalDeclarationSource::Parameter(
                            identity.generated_parameter(position),
                        ),
                        parameter.ty.identity(),
                    )
                })
        })
    }

    pub(super) fn bind_locals(
        &self,
        admitted: &mut impl Iterator<Item = RuntimeLocalSeedId>,
    ) -> Result<EnvironmentCallableLocals, RuntimePlanLowerError> {
        self.definitions
            .iter()
            .map(|(key, definition)| {
                let rows = (0..definition.parameters.len())
                    .map(|_| {
                        admitted.next().ok_or_else(|| {
                            RuntimePlanLowerError::new("environment formal has no admitted local")
                        })
                    })
                    .collect::<Result<Box<[_]>, _>>()?;
                Ok((*key, rows))
            })
            .collect()
    }

    pub(super) fn materialize(
        &self,
        locals: &EnvironmentCallableLocals,
        builder: &mut RuntimePlanBuilder,
    ) -> Result<EnvironmentCallableStates, RuntimePlanLowerError> {
        self.definitions
            .iter()
            .map(|(key, definition)| {
                let locals = locals.get(key).ok_or_else(|| {
                    RuntimePlanLowerError::new("environment declaration frame is absent")
                })?;
                let states = definition.materialize(locals, builder)?;
                Ok((*key, states))
            })
            .collect()
    }
}

impl<'a> EnvironmentDefinition<'a> {
    fn prepare(
        value: &'a CheckedEnvironmentCallableValueProjection,
        ty: &'a RuntimeNormalizedType,
    ) -> Result<Self, RuntimePlanLowerError> {
        let error = |message: &str| RuntimePlanLowerError::new(message);
        let schema = value.declaration().signature();
        if ty.identity().as_bytes() != value.type_identity().as_bytes()
            || schema.groups().is_empty()
        {
            return Err(error(
                "environment declaration source type or group authority differs",
            ));
        }
        if schema.validator() != &CallableValidator::Ordinary {
            return Err(error(
                "environment declaration value needs its selected compiler-owned body recipe",
            ));
        }
        let count = schema
            .groups()
            .iter()
            .try_fold(0usize, |count, group| {
                count.checked_add(group.parameters().len())
            })
            .filter(|count| u32::try_from(*count).is_ok())
            .ok_or_else(|| error("environment whole-formal count exceeds its coordinate"))?;
        let mut parameters = Vec::with_capacity(count);
        let mut arrow = ty;
        let mut effects = RuntimeEffectSet::empty();
        for group in schema.groups() {
            let RuntimeTypeShape::Function {
                parameters: types,
                result,
                contract,
            } = arrow.shape()
            else {
                return Err(error(
                    "environment declaration has an incomplete normalized group chain",
                ));
            };
            if !contract.binder().is_empty() || contract.invocation().variables().next().is_some() {
                return Err(error(
                    "environment declaration value requires checked specialization before executable emission",
                ));
            }
            if types.len() != group.parameters().len() {
                return Err(error(
                    "environment declaration lost a whole formal in normalization",
                ));
            }
            for (parameter, ty) in group.parameters().iter().zip(types.iter()) {
                if matches!(
                    parameter.passing(),
                    CallableParameterPassing::RestPositional | CallableParameterPassing::RestNamed
                ) {
                    return Err(error(
                        "environment rest formal requires its checked pack binding ABI",
                    ));
                }
                parameters.push(EnvironmentParameter {
                    coordinate: RuntimeCallableParameterCoordinate {
                        group: u32::try_from(group.index().get())
                            .map_err(|_| error("environment group exceeds its coordinate"))?,
                        parameter: u32::try_from(parameter.index().get())
                            .map_err(|_| error("environment formal exceeds its coordinate"))?,
                    },
                    ty,
                });
            }
            effects = RuntimeEffectSet::try_from_effects(contract.invocation().constant_effects())
                .map_err(|cause| error(&cause.to_string()))?;
            arrow = result;
        }
        Ok(Self {
            value,
            ty,
            parameters,
            terminal: schema.groups().len() - 1,
            result: arrow,
            effects,
        })
    }

    fn identity(&self) -> RuntimeFunctionDefinitionIdentity {
        RuntimeFunctionDefinitionIdentity::from_accepted_identity(
            *self.value.declaration().id().semantic_digest().as_bytes(),
        )
    }

    fn materialize(
        &self,
        locals: &[RuntimeLocalSeedId],
        builder: &mut RuntimePlanBuilder,
    ) -> Result<Box<[RuntimeCallableStateSeedId]>, RuntimePlanLowerError> {
        let error = |message: &str| RuntimePlanLowerError::new(message);
        if locals.len() != self.parameters.len() {
            return Err(error(
                "environment frame differs from its whole-formal inventory",
            ));
        }
        let identity = self.identity();
        let prefix = self
            .parameters
            .iter()
            .take_while(|parameter| (parameter.coordinate.group as usize) < self.terminal)
            .count();
        let prefix = u32::try_from(prefix)
            .map_err(|_| error("environment capture prefix exceeds its coordinate"))?;
        let inputs = self
            .parameters
            .iter()
            .zip(locals)
            .zip(0_u32..)
            .map(
                |((parameter, local), position)| RuntimeFunctionInputBindingSeed {
                    transfer: RuntimeFunctionInputTransfer::Formal,
                    origin: RuntimeFunctionInputOrigin::Parameter(
                        identity.generated_parameter(position),
                    ),
                    source: if position < prefix {
                        RuntimeFunctionInputSource::CapturedParameter {
                            position,
                            passing: RuntimeFunctionParameterPassing::Value,
                        }
                    } else {
                        RuntimeFunctionInputSource::Parameter {
                            position: position - prefix,
                            passing: RuntimeFunctionParameterPassing::Value,
                        }
                    },
                    input_local: local.clone(),
                    pattern: RuntimePatternSeed::new(
                        parameter.ty.identity(),
                        RuntimePatternSeedKind::Bind {
                            local: local.clone(),
                            mutable: false,
                        },
                    ),
                    ownership: RuntimeValueOwnershipRequirement::Owned,
                    unrestricted_bindings: Box::new([]),
                },
            )
            .collect();
        let body_kind = if self.effects.is_empty() {
            RuntimeFunctionSiteBodyKind::Expression
        } else {
            RuntimeFunctionSiteBodyKind::Executable
        };
        let site = builder
            .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                definition: identity,
                role: RuntimeFunctionSemanticRole::Ordinary,
                function_type: None,
                inputs,
                result: self.result.identity(),
                body_kind,
                effects: self.effects.clone(),
            })
            .map_err(|cause| error(&cause.to_string()))?;
        let arguments = self
            .parameters
            .iter()
            .zip(locals)
            .zip(0_u32..)
            .map(|((parameter, local), position)| {
                RuntimeCallArgumentSeed::new(
                    RuntimeExprSeed::new(
                        parameter.ty.identity(),
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            local.clone(),
                            RuntimeLocalReadMode::Move,
                        )),
                    ),
                    RuntimeCallArgumentMode::Value,
                    position,
                )
            })
            .collect();
        let target = self
            .value
            .declaration()
            .id()
            .runtime_target()
            .map_err(|cause| error(&cause.to_string()))?;
        let expression = RuntimeExprSeed::new(
            self.result.identity(),
            RuntimeExprSeedKind::Call {
                callee: target,
                args: arguments,
            },
        );
        let body = match body_kind {
            RuntimeFunctionSiteBodyKind::Expression => {
                RuntimeFunctionSiteBodySeed::Expression(expression)
            }
            RuntimeFunctionSiteBodyKind::Executable => {
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: self.effects.clone(),
                    ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(expression)]),
                })
            }
        };
        builder
            .define_function_site_seed(&site, body)
            .map_err(|cause| error(&cause.to_string()))?;
        let parameters = self
            .parameters
            .iter()
            .map(|parameter| RuntimeCallableParameterInput {
                coordinate: parameter.coordinate,
                kind: RuntimeCallableParameterKind::Fixed,
                abi_ty: parameter.ty.identity(),
                binding_ty: parameter.ty.identity(),
            })
            .collect::<Vec<_>>();
        callable_states::CallableGroupChain::new(
            self.ty,
            &parameters,
            self.terminal,
            &site,
            RuntimeCallableAttachedContract::None,
        )
        .materialize_from(0, None, builder)
    }
}
