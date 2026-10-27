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
    final_analysis::CheckedExternalCallableValueProjection,
    types::SemanticTypeDigest,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ExternalCallableKey {
    declaration: CheckedCallableDigest,
    function_type: SemanticTypeDigest,
}

impl ExternalCallableKey {
    pub(crate) fn of(value: &CheckedExternalCallableValueProjection) -> Self {
        Self {
            declaration: value.declaration().id().semantic_digest(),
            function_type: value.type_identity(),
        }
    }
}

pub(crate) type ExternalCallableStates =
    BTreeMap<ExternalCallableKey, Box<[RuntimeCallableStateSeedId]>>;
pub(super) type ExternalCallableLocals = BTreeMap<ExternalCallableKey, Box<[RuntimeLocalSeedId]>>;

struct ExternalParameter<'a> {
    coordinate: RuntimeCallableParameterCoordinate,
    ty: &'a RuntimeNormalizedType,
    name: Option<&'a str>,
}

struct ExternalDefinition<'a> {
    value: &'a CheckedExternalCallableValueProjection,
    ty: &'a RuntimeNormalizedType,
    parameters: Vec<ExternalParameter<'a>>,
    terminal: usize,
    result: &'a RuntimeNormalizedType,
    effects: RuntimeEffectSet,
}

pub(super) struct ExternalCallableDefinitions<'a> {
    definitions: BTreeMap<ExternalCallableKey, ExternalDefinition<'a>>,
}

impl<'a> ExternalCallableDefinitions<'a> {
    pub(super) fn prepare(
        facts: &'a RuntimePlanSemanticFacts,
    ) -> Result<Self, RuntimePlanLowerError> {
        let mut definitions = BTreeMap::new();
        let mut failure = None;
        facts.visit_callable_values(&mut |_, value, ty| {
            let RuntimeResolvedValue::ExternalCallable(value) = value else {
                return;
            };
            let key = ExternalCallableKey::of(value);
            if definitions.contains_key(&key) || failure.is_some() {
                return;
            }
            match ty
                .ok_or_else(|| {
                    RuntimePlanLowerError::new("environment callable has no normalized source type")
                })
                .and_then(|ty| ExternalDefinition::prepare(value, ty))
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
                .chain(
                    definition
                        .value
                        .declaration()
                        .host_call_contract()
                        .map(|_| {
                            RuntimeLocalDeclarationSeed::new(
                                arcweft_core::plan::RuntimeLocalDeclarationSource::EvaluatedResult(
                                    identity,
                                ),
                                definition.result.identity(),
                            )
                        }),
                )
        })
    }

    pub(super) fn bind_locals(
        &self,
        admitted: &mut impl Iterator<Item = RuntimeLocalSeedId>,
    ) -> Result<ExternalCallableLocals, RuntimePlanLowerError> {
        self.definitions
            .iter()
            .map(|(key, definition)| {
                let count = definition.parameters.len()
                    + usize::from(
                        definition
                            .value
                            .declaration()
                            .host_call_contract()
                            .is_some(),
                    );
                let rows = (0..count)
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
        locals: &ExternalCallableLocals,
        builder: &mut RuntimePlanBuilder,
    ) -> Result<ExternalCallableStates, RuntimePlanLowerError> {
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

impl<'a> ExternalDefinition<'a> {
    fn host_body(
        &self,
        locals: &[RuntimeLocalSeedId],
        builder: &RuntimePlanBuilder,
    ) -> Result<RuntimeFunctionSiteBodySeed, RuntimePlanLowerError> {
        use arcweft_core::plan::{
            RuntimeHostArgumentSeed, RuntimeHostCallTargetSeed, RuntimeNeedProducerStartTargetSeed,
        };
        use arcweft_core::step::RuntimeHostCallMode;
        use arcweft_core::task::{
            CancelScopeId, HostCallProducerDefinition, HostCapabilityId, HostRestartPolicy,
            NeedProducerContractDigest, NeedProducerRequestProjection, NeedProducerSiteDigest,
            NeedProducerTaskPlan, RuntimeRequestRoleIdentity, TaskClass, TaskPolicy, TaskPriority,
        };
        use arcweft_lang_hir::symbol::CallableDeclarationKey;
        use arcweft_lang_sema::callable::CheckedCallableDeclaration;
        let error = |message: &str| RuntimePlanLowerError::new(message);
        let CheckedCallableDeclaration::Project(CallableDeclarationKey::Existing(declaration)) =
            self.value.declaration().id().declaration()
        else {
            return Err(error(
                "external Host value has no checked declaration owner",
            ));
        };
        let capability = declaration
            .owner_path()
            .iter()
            .map(arcweft_lang_syntax::ast::module_path::ModuleSegment::as_str)
            .collect::<Vec<_>>()
            .join(".");
        if capability.is_empty() {
            return Err(error("external Host value has no capability path"));
        }
        let operation = declaration.name().to_owned();
        let host_contract = self
            .value
            .declaration()
            .host_call_contract()
            .ok_or_else(|| error("external Host value lost its manifest contract"))?;
        let contract = NeedProducerContractDigest::from_bytes(*host_contract.as_bytes());
        let identity = self.identity();
        let site = NeedProducerSiteDigest::from_bytes(*identity.as_bytes());
        let task_plan = builder
            .task_plan_construction()
            .reserve_original_source()
            .map_err(|cause| error(&cause.to_string()))?;
        let arguments = self
            .parameters
            .iter()
            .zip(locals)
            .map(|(parameter, local)| {
                let mut hash = blake3::Hasher::new();
                hash.update(b"arcweft.external-callable.argument-role.v1\0");
                hash.update(identity.as_bytes());
                hash.update(&parameter.coordinate.group.to_le_bytes());
                hash.update(&parameter.coordinate.parameter.to_le_bytes());
                let role =
                    RuntimeRequestRoleIdentity::from_accepted_identity(*hash.finalize().as_bytes());
                let value = RuntimeExprSeed::new(
                    parameter.ty.identity(),
                    RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                        local.clone(),
                        RuntimeLocalReadMode::Move,
                    )),
                );
                if let Some(name) = parameter.name {
                    RuntimeHostArgumentSeed::Named(
                        role,
                        arcweft_core::task::NamedHostArg {
                            name: name.to_owned(),
                            value,
                        },
                    )
                } else {
                    RuntimeHostArgumentSeed::Positional(role, value)
                }
            })
            .collect();
        let local = locals[self.parameters.len()].clone();
        let binding = RuntimePatternSeed::new(
            self.result.identity(),
            RuntimePatternSeedKind::Bind {
                local: local.clone(),
                mutable: false,
            },
        );
        let operation = if let RuntimeTypeShape::Need(payload) = self.result.shape() {
            // This generated original function owns the complete checked input
            // ABI. Its Source admits actual values before issuing a Host Need;
            // no synthetic selected-call or argument certificate is fabricated.
            let plan = NeedProducerTaskPlan::try_new(
                contract,
                site,
                NeedProducerRequestProjection::ExternCapability {
                    capability: HostCapabilityId(capability),
                    operation,
                    contract: host_contract,
                    argument_names: self
                        .parameters
                        .iter()
                        .map(|parameter| parameter.name.map(str::to_owned))
                        .collect(),
                },
                self.parameters
                    .iter()
                    .map(|parameter| parameter.ty.identity())
                    .collect(),
                payload.identity(),
                TaskPolicy::AlwaysStart,
                HostRestartPolicy::MustBeQuiescent,
                TaskClass::Background,
                TaskPriority(0),
                CancelScopeId("flow".into()),
            )
            .map_err(|cause| error(&cause.to_string()))?;
            RuntimeFlowOpSeed::StartNeedProducer {
                binding,
                target: RuntimeNeedProducerStartTargetSeed {
                    plan,
                    task_plan,
                    arguments,
                },
            }
        } else {
            RuntimeFlowOpSeed::HostCall {
                binding: Some(binding),
                target: RuntimeHostCallTargetSeed {
                    producer: HostCallProducerDefinition {
                        contract,
                        plan: task_plan,
                        site,
                    },
                    public_id: format!("{capability}.{operation}"),
                    capability,
                    operation,
                    contract: Some(host_contract),
                    args: arguments,
                    result: self.result.identity(),
                    mode: RuntimeHostCallMode::Immediate,
                    deterministic: false,
                },
            }
        };
        Ok(RuntimeFunctionSiteBodySeed::Executable(
            RuntimeExecutableBodySeed {
                effects: self.effects.clone(),
                ops: Box::new([
                    operation,
                    RuntimeFlowOpSeed::ReturnExpr(RuntimeExprSeed::new(
                        self.result.identity(),
                        RuntimeExprSeedKind::Local(RuntimeLocalReadSeed::new(
                            local,
                            RuntimeLocalReadMode::Move,
                        )),
                    )),
                ]),
            },
        ))
    }

    fn prepare(
        value: &'a CheckedExternalCallableValueProjection,
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
            .filter(|count| {
                count
                    .checked_add(usize::from(
                        value.declaration().host_call_contract().is_some(),
                    ))
                    .is_some_and(|locals| u32::try_from(locals).is_ok())
            })
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
                parameters.push(ExternalParameter {
                    coordinate: RuntimeCallableParameterCoordinate {
                        group: u32::try_from(group.index().get())
                            .map_err(|_| error("environment group exceeds its coordinate"))?,
                        parameter: u32::try_from(parameter.index().get())
                            .map_err(|_| error("environment formal exceeds its coordinate"))?,
                    },
                    ty,
                    name: if parameter.passing() == CallableParameterPassing::NamedOnly {
                        Some(
                            parameter
                                .name()
                                .ok_or_else(|| error("external named formal has no checked name"))?
                                .as_str(),
                        )
                    } else {
                        None
                    },
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
        let mut hash = blake3::Hasher::new();
        hash.update(b"arcweft.external-callable.definition.v1\0");
        hash.update(self.value.declaration().id().semantic_digest().as_bytes());
        hash.update(self.value.type_identity().as_bytes());
        RuntimeFunctionDefinitionIdentity::from_accepted_identity(*hash.finalize().as_bytes())
    }

    fn materialize(
        &self,
        locals: &[RuntimeLocalSeedId],
        builder: &mut RuntimePlanBuilder,
    ) -> Result<Box<[RuntimeCallableStateSeedId]>, RuntimePlanLowerError> {
        let error = |message: &str| RuntimePlanLowerError::new(message);
        let host = self.value.declaration().host_call_contract().is_some();
        if locals.len() != self.parameters.len() + usize::from(host) {
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
        let body_kind = if self.effects.is_empty() && !host {
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
        let body = if host {
            self.host_body(locals, builder)?
        } else {
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
            match body_kind {
                RuntimeFunctionSiteBodyKind::Expression => {
                    RuntimeFunctionSiteBodySeed::Expression(expression)
                }
                RuntimeFunctionSiteBodyKind::Executable => {
                    RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                        effects: self.effects.clone(),
                        ops: Box::new([RuntimeFlowOpSeed::ReturnExpr(expression)]),
                    })
                }
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
