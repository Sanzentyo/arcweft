use std::sync::Arc;

use thiserror::Error;

use crate::{
    awbc::schema::AwbcProgram,
    entry::RuntimeSchemaLimits,
    pattern::{RuntimeCheckedType, RuntimeSemanticTypeId},
    plan::{RuntimeAgentTypeProjection, RuntimePlan},
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
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TaskOutcomeBindingError {
    #[error("standalone task outcome contains a program-owned nominal type or exceeds type limits")]
    InvalidStandaloneType,
    #[error("a standalone task outcome contract cannot be bound to a program")]
    StandaloneRequiresStandaloneBinding,
    #[error("a program task outcome contract requires a selected executable")]
    ProgramRequiresExecutable,
    #[error(transparent)]
    ProgramType(#[from] RuntimeProgramTypeError),
}

/// Failure to admit a task outcome value through its bound type authority.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum TaskOutcomeValueError {
    #[error("standalone task outcome rejected its value: {0}")]
    Standalone(String),
    #[error("program task outcome rejected its value: {0}")]
    Program(#[from] RuntimeProgramTypeError),
    #[error("program task outcome type {semantic_type:?} is not a Result")]
    NotResult {
        semantic_type: RuntimeSemanticTypeId,
    },
}

#[derive(Clone, Debug)]
enum BoundTaskOutcomeType {
    Standalone(RuntimeCheckedType),
    Program {
        payload: RuntimeSemanticTypeId,
        owner: RuntimeProgramOwner,
        limits: RuntimeSchemaLimits,
    },
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
            Some(owner) => spec.outcome.bind_program(owner, limits)?,
            None => spec.outcome.bind_standalone()?,
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

impl TaskOutcomeContract {
    pub(crate) fn standalone_contract_is_valid(&self) -> bool {
        let Self::Standalone { payload } = self else {
            return false;
        };
        let limits = RuntimeSchemaLimits::engine_default();
        let mut work = vec![(payload, 0_usize)];
        let mut nodes = 0_usize;
        while let Some((ty, depth)) = work.pop() {
            nodes += 1;
            if !limits.permits_nodes(nodes) || !limits.permits_depth(depth) {
                return false;
            }
            let child_depth = depth + 1;
            match ty {
                RuntimeCheckedType::ExecutableRef(_)
                | RuntimeCheckedType::Nominal { .. }
                | RuntimeCheckedType::Agent(RuntimeAgentTypeProjection::DataShape(_))
                | RuntimeCheckedType::Variant {
                    owner: crate::pattern::RuntimeVariantIdentity::Nominal { .. },
                    ..
                } => return false,
                RuntimeCheckedType::Sequence(child)
                | RuntimeCheckedType::Option(child)
                | RuntimeCheckedType::Array { item: child, .. } => {
                    work.push((child, child_depth));
                }
                RuntimeCheckedType::Map { key, value, .. } => {
                    work.push((key, child_depth));
                    work.push((value, child_depth));
                }
                RuntimeCheckedType::Result { ok, error } => {
                    work.push((ok, child_depth));
                    work.push((error, child_depth));
                }
                RuntimeCheckedType::Tuple(items) | RuntimeCheckedType::Choice(items) => {
                    work.extend(items.iter().map(|child| (child, child_depth)));
                }
                RuntimeCheckedType::Record(fields) => {
                    work.extend(fields.iter().map(|field| (field.ty(), child_depth)));
                }
                RuntimeCheckedType::Variant {
                    arguments, cases, ..
                } => {
                    work.extend(arguments.iter().map(|child| (child, child_depth)));
                    work.extend(
                        cases
                            .iter()
                            .filter_map(|case| case.payload.as_deref())
                            .map(|child| (child, child_depth)),
                    );
                }
                RuntimeCheckedType::Agent(RuntimeAgentTypeProjection::Probe(child)) => {
                    work.push((child, child_depth));
                }
                RuntimeCheckedType::Never
                | RuntimeCheckedType::Unit
                | RuntimeCheckedType::Bool
                | RuntimeCheckedType::Signed(_)
                | RuntimeCheckedType::Unsigned(_)
                | RuntimeCheckedType::F32
                | RuntimeCheckedType::F64
                | RuntimeCheckedType::String
                | RuntimeCheckedType::Color
                | RuntimeCheckedType::Char
                | RuntimeCheckedType::Duration
                | RuntimeCheckedType::Progress
                | RuntimeCheckedType::EntityReference
                | RuntimeCheckedType::AgentValue
                | RuntimeCheckedType::Bytes
                | RuntimeCheckedType::Opaque { .. }
                | RuntimeCheckedType::Agent(_) => {}
            }
        }
        true
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
        let Self::Program { payload } = self else {
            return Err(TaskOutcomeBindingError::StandaloneRequiresStandaloneBinding);
        };
        owner.types().require_type(*payload)?;
        Ok(BoundTaskOutcome {
            ty: BoundTaskOutcomeType::Program {
                payload: *payload,
                owner,
                limits,
            },
        })
    }
}

impl BoundTaskOutcome {
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
                },
                BoundTaskOutcomeType::Program {
                    payload: right_payload,
                    owner: right_owner,
                    limits: right_limits,
                },
            ) => {
                left_payload == right_payload
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
        match &self.ty {
            BoundTaskOutcomeType::Standalone(payload) => {
                if payload.accepts_value(value) {
                    Ok(())
                } else {
                    Err(TaskOutcomeValueError::Standalone(
                        "runtime payload does not satisfy its checked type".to_owned(),
                    ))
                }
            }
            BoundTaskOutcomeType::Program {
                payload,
                owner,
                limits,
            } => {
                owner
                    .types()
                    .validate_live_value(*payload, value, *limits)?;
                Ok(())
            }
        }
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
            BoundTaskOutcomeType::Program {
                payload,
                owner,
                limits,
            } => {
                let types = owner.types();
                if !types.is_result_type(*payload)? {
                    return Err(TaskOutcomeValueError::NotResult {
                        semantic_type: *payload,
                    });
                }
                let result = RuntimeValue::result_ok(value);
                types.validate_live_value(*payload, &result, *limits)?;
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
            BoundTaskOutcomeType::Program {
                payload,
                owner,
                limits,
            } => {
                let types = owner.types();
                if !types.is_result_type(*payload)? {
                    return Err(TaskOutcomeValueError::NotResult {
                        semantic_type: *payload,
                    });
                }
                let result = RuntimeValue::result_err(value);
                types.validate_live_value(*payload, &result, *limits)?;
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
