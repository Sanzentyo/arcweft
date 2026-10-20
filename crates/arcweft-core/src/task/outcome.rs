use std::sync::Arc;

use thiserror::Error;

use crate::{
    awbc::schema::AwbcProgram,
    entry::RuntimeSchemaLimits,
    pattern::{RuntimeCheckedType, RuntimeSemanticTypeId},
    plan::RuntimePlan,
    program_types::{RuntimeProgramTypeError, RuntimeProgramTypes},
    task::{TaskOutcomeContract, TaskSpec, TaskSubmission},
    value::{RuntimePayload, RuntimeValue},
};

/// Exact executable selected to validate a program-owned task outcome.
#[derive(Clone, Debug)]
pub enum RuntimeProgramOwner {
    Plan(Arc<RuntimePlan>),
    Awbc(Arc<AwbcProgram>),
}

impl RuntimeProgramOwner {
    /// Borrows the original executable's type authority.
    pub fn types(&self) -> RuntimeProgramTypes<'_> {
        match self {
            Self::Plan(plan) => RuntimeProgramTypes::Plan(plan),
            Self::Awbc(program) => RuntimeProgramTypes::Awbc(program),
        }
    }

    /// Tests whether both leases select the same immutable executable instance.
    #[must_use]
    pub fn same_program(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Plan(left), Self::Plan(right)) => Arc::ptr_eq(left, right),
            (Self::Awbc(left), Self::Awbc(right)) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}

/// Failure to bind a task outcome contract to the required type authority.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TaskOutcomeBindingError {
    #[error("standalone task outcome contains a program-owned nominal type or exceeds type limits")]
    InvalidStandaloneType,
    #[error("a standalone task outcome contract cannot be bound to a program")]
    StandaloneRequiresStandaloneBinding,
    #[error("a program task outcome contract requires a selected executable")]
    ProgramRequiresExecutable,
    #[error("a standalone task outcome cannot carry a program invocation binding")]
    StandaloneInvocationBinding,
    #[error(transparent)]
    ProgramType(#[from] RuntimeProgramTypeError),
}

/// Failure to admit a task outcome value through its bound type authority.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TaskOutcomeValueError {
    #[error("task outcome resource admission failed: {0}")]
    Resource(Box<crate::entry::RuntimeSchemaError>),
    #[error(
        "task outcome type {semantic_type:?} does not belong to the enclosing original program"
    )]
    WrongProgramOwner {
        semantic_type: RuntimeSemanticTypeId,
    },
    #[error("standalone task outcome rejected its value: {0}")]
    Standalone(String),
    #[error("program task outcome rejected its value: {0}")]
    Program(#[from] RuntimeProgramTypeError),
    #[error("program task outcome type {semantic_type:?} is not a Result")]
    NotResult {
        semantic_type: RuntimeSemanticTypeId,
    },
}

impl From<crate::entry::RuntimeSchemaError> for TaskOutcomeValueError {
    fn from(error: crate::entry::RuntimeSchemaError) -> Self {
        Self::Resource(Box::new(error))
    }
}

#[derive(Clone, Debug)]
enum BoundTaskOutcomeType {
    Standalone(RuntimeCheckedType),
    Program {
        payload: RuntimeSemanticTypeId,
        owner: RuntimeProgramOwner,
        limits: RuntimeSchemaLimits,
        binding: Option<crate::program_types::RuntimeFunctionEffectInstantiation>,
    },
}

/// Borrows either the original descriptor or its already bound authority.
/// It owns no type graph, value, or executable lease.
enum TaskOutcomeTypeView<'a> {
    Standalone(&'a RuntimeCheckedType),
    Program {
        payload: RuntimeSemanticTypeId,
        owner: &'a RuntimeProgramOwner,
        binding: Option<&'a crate::program_types::RuntimeFunctionEffectInstantiation>,
    },
}

impl TaskOutcomeTypeView<'_> {
    fn validate_admitted(
        self,
        value: crate::value::RuntimeValueView<'_>,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<(), TaskOutcomeValueError> {
        match self {
            Self::Standalone(payload) => {
                resources.validate_standalone_outcome_type(payload)?;
                if payload.accepts_admitted_value_view(value, &resources.work())? {
                    Ok(())
                } else {
                    Err(TaskOutcomeValueError::Standalone(
                        "runtime payload does not satisfy its checked type".to_owned(),
                    ))
                }
            }
            Self::Program {
                payload,
                owner,
                binding,
            } => {
                owner
                    .types()
                    .value_type(payload)?
                    .validate_admitted_view(value, binding, resources)?;
                Ok(())
            }
        }
    }
}

/// Owned outcome contract bound to one exact standalone predicate or program.
#[derive(Clone, Debug)]
pub struct BoundTaskOutcome {
    ty: BoundTaskOutcomeType,
}

/// A complete task specification whose outcome is bound to its exact runtime
/// type authority. The contained contract is always derived from `spec` by
/// [`BoundTaskSpec::bind`].
#[derive(Clone, Debug)]
pub struct BoundTaskSpec {
    submission: TaskSubmission,
    outcome: BoundTaskOutcome,
}

impl BoundTaskSpec {
    /// Binds the outcome declared by `spec` to the supplied authority.
    ///
    /// A standalone outcome requires `owner: None`; a program outcome requires
    /// the exact selected executable. This constructor does not accept an
    /// independently constructed bound outcome, so the specification and its
    /// admission authority cannot disagree.
    pub fn bind(
        submission: TaskSubmission,
        owner: Option<RuntimeProgramOwner>,
        limits: RuntimeSchemaLimits,
    ) -> Result<Self, TaskOutcomeBindingError> {
        let spec = submission.spec();
        let outcome = match owner {
            Some(owner) => {
                spec.outcome
                    .bind_program_in_invocation(owner, spec.producer.binding(), limits)?
            }
            None if spec.producer.binding().is_none() => spec.outcome.bind_standalone()?,
            None => return Err(TaskOutcomeBindingError::StandaloneInvocationBinding),
        };
        Ok(Self {
            submission,
            outcome,
        })
    }

    #[must_use]
    pub const fn spec(&self) -> &TaskSpec {
        self.submission.spec()
    }

    pub const fn handle(&self) -> super::TaskHandle {
        self.submission.handle()
    }

    pub const fn task_id(&self) -> super::TaskId {
        self.submission.task_id()
    }

    pub const fn task_key(&self) -> super::TaskKey {
        self.submission.task_key()
    }

    #[must_use]
    pub const fn outcome(&self) -> &BoundTaskOutcome {
        &self.outcome
    }

    /// Whether this task has the same non-identity scheduling contract as
    /// another task. Task id and diagnostic label are intentionally excluded.
    #[must_use]
    pub fn same_join_contract(&self, other: &Self) -> bool {
        self.spec().same_join_contract(other.spec()) && self.outcome.same_contract(&other.outcome)
    }

    /// Whether two specifications for the same task id are identical apart
    /// from the diagnostic-only label.
    #[must_use]
    pub fn same_identity_spec(&self, other: &Self) -> bool {
        self.handle() == other.handle() && self.same_join_contract(other)
    }
}

impl PartialEq for BoundTaskSpec {
    fn eq(&self, other: &Self) -> bool {
        self.submission == other.submission && self.outcome.same_contract(&other.outcome)
    }
}

impl<P> TaskSpec<P> {
    pub(crate) fn validate_admitted_value_view(
        &self,
        value: crate::value::RuntimeValueView<'_>,
        owner: Option<&RuntimeProgramOwner>,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<(), TaskOutcomeValueError> {
        self.outcome.validate_admitted_in_invocation(
            value,
            owner,
            self.producer.binding(),
            resources,
        )
    }

    pub(crate) fn validate_outcome_storage_owner(
        &self,
        owner: Option<&RuntimeProgramOwner>,
        resources: &crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<(), TaskOutcomeBindingError> {
        self.outcome.validate_storage_owner(owner)?;
        match &self.outcome {
            TaskOutcomeContract::Program { payload } => owner
                .ok_or(TaskOutcomeBindingError::ProgramRequiresExecutable)?
                .types()
                .value_type(*payload)?
                .validate_invocation_binding(self.producer.binding(), resources)?,
            TaskOutcomeContract::Standalone { .. } if self.producer.binding().is_some() => {
                return Err(TaskOutcomeBindingError::StandaloneInvocationBinding);
            }
            TaskOutcomeContract::Standalone { .. } => (),
        }
        Ok(())
    }
}

impl TaskOutcomeContract {
    /// The enclosing admitted task or suspended activation supplies the
    /// original declaration solution; the candidate value cannot select it.
    pub(crate) fn validate_admitted_in_invocation(
        &self,
        value: crate::value::RuntimeValueView<'_>,
        owner: Option<&RuntimeProgramOwner>,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<(), TaskOutcomeValueError> {
        let expected = match self {
            Self::Standalone { payload } => TaskOutcomeTypeView::Standalone(payload),
            Self::Program { payload } => TaskOutcomeTypeView::Program {
                payload: *payload,
                owner: owner.ok_or(TaskOutcomeValueError::WrongProgramOwner {
                    semantic_type: *payload,
                })?,
                binding,
            },
        };
        expected.validate_admitted(value, resources)
    }

    /// Checks the original executable coordinate before any retained payload
    /// is projected or restored. Standalone predicates have no program row.
    pub(crate) fn validate_storage_owner(
        &self,
        owner: Option<&RuntimeProgramOwner>,
    ) -> Result<(), TaskOutcomeBindingError> {
        if let Self::Program { payload } = self {
            owner
                .ok_or(TaskOutcomeBindingError::ProgramRequiresExecutable)?
                .types()
                .require_type(*payload)?;
        }
        Ok(())
    }

    pub(crate) fn standalone_contract_is_valid(&self) -> bool {
        let Self::Standalone { payload } = self else {
            return false;
        };
        crate::entry::schema::value_encoding::validate_standalone_task_type(
            payload,
            RuntimeSchemaLimits::engine_default(),
        )
        .is_ok()
    }
    /// Binds an explicit finite contract that has no executable owner.
    pub fn bind_standalone(&self) -> Result<BoundTaskOutcome, TaskOutcomeBindingError> {
        match self {
            Self::Standalone { payload } if self.standalone_contract_is_valid() => {
                Ok(BoundTaskOutcome {
                    ty: BoundTaskOutcomeType::Standalone(payload.clone()),
                })
            }
            Self::Standalone { .. } => Err(TaskOutcomeBindingError::InvalidStandaloneType),
            Self::Program { .. } => Err(TaskOutcomeBindingError::ProgramRequiresExecutable),
        }
    }

    /// Binds the contract to one selected executable and validates its type row.
    pub fn bind_program(
        &self,
        owner: RuntimeProgramOwner,
        limits: RuntimeSchemaLimits,
    ) -> Result<BoundTaskOutcome, TaskOutcomeBindingError> {
        self.bind_program_in_invocation(owner, None, limits)
    }

    fn bind_program_in_invocation(
        &self,
        owner: RuntimeProgramOwner,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
        limits: RuntimeSchemaLimits,
    ) -> Result<BoundTaskOutcome, TaskOutcomeBindingError> {
        let Self::Program { payload } = self else {
            return Err(TaskOutcomeBindingError::StandaloneRequiresStandaloneBinding);
        };
        self.validate_storage_owner(Some(&owner))?;
        let mut resources =
            crate::entry::schema::value_encoding::ValueResourceAdmission::live(limits);
        if let Some(binding) = binding {
            resources
                .function_instantiation(
                    binding,
                    crate::entry::schema::value_budget::ResourceByteKind::Metadata,
                )
                .map_err(RuntimeProgramTypeError::from)?;
        }
        owner
            .types()
            .value_type(*payload)?
            .validate_invocation_binding(binding, &resources)?;
        Ok(BoundTaskOutcome {
            ty: BoundTaskOutcomeType::Program {
                payload: *payload,
                owner,
                limits,
                binding: binding.cloned(),
            },
        })
    }
}

impl BoundTaskOutcome {
    /// Resolves the original outcome while a retained candidate is borrowed.
    /// Its value graph must already belong to this resource allowance.
    pub(crate) fn validate_admitted_value_view(
        &self,
        value: crate::value::RuntimeValueView<'_>,
        original_owner: Option<&RuntimeProgramOwner>,
        resources: &mut crate::entry::schema::value_encoding::ValueResourceAdmission,
    ) -> Result<(), TaskOutcomeValueError> {
        let expected = match &self.ty {
            BoundTaskOutcomeType::Standalone(payload) => TaskOutcomeTypeView::Standalone(payload),
            BoundTaskOutcomeType::Program {
                payload,
                owner,
                binding,
                ..
            } => {
                if original_owner.is_none_or(|original| !owner.same_program(original)) {
                    return Err(TaskOutcomeValueError::WrongProgramOwner {
                        semantic_type: *payload,
                    });
                }
                TaskOutcomeTypeView::Program {
                    payload: *payload,
                    owner,
                    binding: binding.as_ref(),
                }
            }
        };
        expected.validate_admitted(value, resources)
    }

    /// Whether two waiters can share one published result under the same
    /// executable, result row and admission budget.
    #[must_use]
    pub fn same_contract(&self, other: &Self) -> bool {
        match (&self.ty, &other.ty) {
            (BoundTaskOutcomeType::Standalone(left), BoundTaskOutcomeType::Standalone(right)) => {
                left == right
            }
            (
                BoundTaskOutcomeType::Program {
                    payload: left_payload,
                    owner: left_owner,
                    limits: left_limits,
                    binding: left_binding,
                },
                BoundTaskOutcomeType::Program {
                    payload: right_payload,
                    owner: right_owner,
                    limits: right_limits,
                    binding: right_binding,
                },
            ) => {
                left_payload == right_payload
                    && left_binding == right_binding
                    && left_limits == right_limits
                    && left_owner.same_program(right_owner)
            }
            _ => false,
        }
    }

    /// Reports the outer Result carrier without expanding recursive children.
    pub fn is_result_type(&self) -> Result<bool, RuntimeProgramTypeError> {
        match &self.ty {
            BoundTaskOutcomeType::Standalone(payload) => {
                Ok(matches!(payload, RuntimeCheckedType::Result { .. }))
            }
            BoundTaskOutcomeType::Program { payload, owner, .. } => {
                owner.types().is_result_type(*payload)
            }
        }
    }

    /// Admits one exact payload value and returns it in the temporal carrier.
    pub fn try_payload(
        &self,
        value: RuntimeValue,
    ) -> Result<RuntimePayload, TaskOutcomeValueError> {
        self.validate_value(&value)?;
        Ok(value.into())
    }

    /// Checks the borrowed value without duplicating an affine payload.
    pub fn validate_value(&self, value: &RuntimeValue) -> Result<(), TaskOutcomeValueError> {
        let (owner, limits) = match &self.ty {
            BoundTaskOutcomeType::Standalone(_) => (None, RuntimeSchemaLimits::engine_default()),
            BoundTaskOutcomeType::Program { owner, limits, .. } => (Some(owner), *limits),
        };
        let mut resources =
            crate::entry::schema::value_encoding::ValueResourceAdmission::live(limits);
        resources.validate_view(value.view(), 0, None)?;
        self.validate_admitted_value_view(value.view(), owner, &mut resources)
    }

    /// Constructs and admits the canonical Result::Ok carrier.
    pub fn try_result_ok(
        &self,
        value: RuntimeValue,
    ) -> Result<RuntimePayload, TaskOutcomeValueError> {
        match &self.ty {
            BoundTaskOutcomeType::Standalone(payload) => payload
                .try_result_payload(Ok(value))
                .map_err(TaskOutcomeValueError::Standalone),
            BoundTaskOutcomeType::Program { payload, owner, .. } => {
                let types = owner.types();
                if !types.is_result_type(*payload)? {
                    return Err(TaskOutcomeValueError::NotResult {
                        semantic_type: *payload,
                    });
                }
                let result = RuntimeValue::result_ok(value);
                self.validate_value(&result)?;
                Ok(result.into())
            }
        }
    }

    /// Constructs and admits the canonical Result::Err carrier.
    pub fn try_result_err(
        &self,
        value: RuntimeValue,
    ) -> Result<RuntimePayload, TaskOutcomeValueError> {
        match &self.ty {
            BoundTaskOutcomeType::Standalone(payload) => payload
                .try_result_payload(Err(value))
                .map_err(TaskOutcomeValueError::Standalone),
            BoundTaskOutcomeType::Program { payload, owner, .. } => {
                let types = owner.types();
                if !types.is_result_type(*payload)? {
                    return Err(TaskOutcomeValueError::NotResult {
                        semantic_type: *payload,
                    });
                }
                let result = RuntimeValue::result_err(value);
                self.validate_value(&result)?;
                Ok(result.into())
            }
        }
    }

    /// Projects only a Result's Err child for finite diagnostics.
    pub fn result_error_checked(
        &self,
    ) -> Result<Option<RuntimeCheckedType>, RuntimeProgramTypeError> {
        match &self.ty {
            BoundTaskOutcomeType::Standalone(payload) => Ok(match payload {
                RuntimeCheckedType::Result { error, .. } => Some((**error).clone()),
                _ => None,
            }),
            BoundTaskOutcomeType::Program { payload, owner, .. } => {
                owner.types().result_error_checked(*payload)
            }
        }
    }
}

#[cfg(test)]
mod tests;
