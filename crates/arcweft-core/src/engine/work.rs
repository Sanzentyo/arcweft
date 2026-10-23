//! Native work retains original source ranges and owned runtime continuations.
//!
//! A source coordinate carries no executable authority. Its enclosing frame
//! retains and authenticates the original program before projecting work.

use super::NativeCodeCursor;
use super::code_cursor::NativeSourceOperation;
use crate::scope::{RuntimeScheduledScopeToken, RuntimeScopeIdentity};
use crate::value::{RuntimeIterator, RuntimeLocalBinding, RuntimeValue};

pub(super) type NativePendingWorkRollbackImage = NativePendingWork<
    crate::value::AwbcRuntimeValueSnapshot,
    crate::value::AwbcRuntimeValueSnapshot,
>;

pub(super) trait NativeStoredWork {
    fn logical_count(&self) -> Option<usize>;

    fn admit_work(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>;

    fn admit_queue(
        work: &std::collections::VecDeque<Self>,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError>
    where
        Self: Sized,
    {
        admission.status()?;
        admission.validate_metadata_collection(work.len())?;
        let logical = work
            .iter()
            .try_fold(0_usize, |total, item| {
                total.checked_add(item.logical_count()?)
            })
            .ok_or_else(|| crate::value::AwbcRuntimeValueSnapshotError::Message {
                message: "native work queue has an invalid logical operation count".into(),
            })?;
        admission.validate_metadata_collection(logical)?;
        for item in work {
            item.admit_work(admission, binding)?;
        }
        Ok(())
    }
}

/// One opaque native work item. Authored code stays in the original executable;
/// runtime bindings and iterator progress remain owned by their continuation.
#[derive(Debug, PartialEq)]
pub struct NativePendingWork<V = RuntimeValue, I = RuntimeIterator> {
    pub(super) kind: NativePendingWorkKind<V, I>,
}

#[derive(Debug, PartialEq)]
pub(super) enum NativePendingWorkKind<V, I> {
    Source {
        position: NativeCodeCursor,
        role: NativeSourceRangeRole,
    },
    Resume(NativeRuntimeContinuation<V, I>),
}

/// Invocation completion and nested-body completion have distinct semantics.
/// Function roots were never counted as queued source operations. Task roots
/// and nested bodies preserve the original logical pending-operation count.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeSourceRangeRole {
    FunctionRoot,
    TaskRoot,
    NestedBody,
}

#[derive(Debug, PartialEq)]
pub(super) enum NativeRuntimeContinuation<V, I> {
    Bind(Vec<RuntimeLocalBinding<V>>),
    LoopNext {
        source: NativeSourceOperation,
    },
    WhileNext {
        source: NativeSourceOperation,
    },
    WhileLetNext {
        source: NativeSourceOperation,
    },
    ForNext {
        source: NativeSourceOperation,
        iterator: I,
    },
    CompleteFormatOperand {
        source: NativeSourceOperation,
    },
    CompleteLetScope {
        source: NativeSourceOperation,
    },
    EnterScheduledScope {
        identity: RuntimeScopeIdentity,
        token: RuntimeScheduledScopeToken,
    },
    ExitScheduledScope {
        token: RuntimeScheduledScopeToken,
    },
    CompleteAwaitObserver,
}

/// Borrowed inspection distinguishes authored code from runtime work without
/// reconstructing a FlowOp or copying a value for observers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NativePendingWorkView<'a> {
    Source(&'a crate::plan::FlowOp),
    Expression(&'a crate::value::RuntimeExpr),
    EndOfFunctionBody(&'a NativeCodeCursor),
    Resume(NativeRuntimeContinuationKind),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeRuntimeContinuationKind {
    Bind,
    LoopNext,
    WhileNext,
    WhileLetNext,
    ForNext,
    CompleteFormatOperand,
    CompleteLetScope,
    EnterScheduledScope,
    ExitScheduledScope { token: RuntimeScheduledScopeToken },
    CompleteAwaitObserver,
}

impl NativeRuntimeContinuationKind {
    const fn metadata_tag(self) -> u8 {
        match self {
            Self::Bind => 0,
            Self::LoopNext => 1,
            Self::WhileNext => 2,
            Self::WhileLetNext => 3,
            Self::ForNext => 4,
            Self::CompleteFormatOperand => 5,
            Self::CompleteLetScope => 6,
            Self::EnterScheduledScope => 7,
            Self::ExitScheduledScope { .. } => 8,
            Self::CompleteAwaitObserver => 9,
        }
    }
}

impl<V, I> NativeRuntimeContinuation<V, I> {
    pub(super) const fn kind(&self) -> NativeRuntimeContinuationKind {
        match self {
            Self::Bind(_) => NativeRuntimeContinuationKind::Bind,
            Self::LoopNext { .. } => NativeRuntimeContinuationKind::LoopNext,
            Self::WhileNext { .. } => NativeRuntimeContinuationKind::WhileNext,
            Self::WhileLetNext { .. } => NativeRuntimeContinuationKind::WhileLetNext,
            Self::ForNext { .. } => NativeRuntimeContinuationKind::ForNext,
            Self::CompleteFormatOperand { .. } => {
                NativeRuntimeContinuationKind::CompleteFormatOperand
            }
            Self::CompleteLetScope { .. } => NativeRuntimeContinuationKind::CompleteLetScope,
            Self::EnterScheduledScope { .. } => NativeRuntimeContinuationKind::EnterScheduledScope,
            Self::ExitScheduledScope { token } => {
                NativeRuntimeContinuationKind::ExitScheduledScope { token: *token }
            }
            Self::CompleteAwaitObserver => NativeRuntimeContinuationKind::CompleteAwaitObserver,
        }
    }
}

impl<V, I> NativePendingWork<V, I> {
    pub(super) fn position(&self) -> Option<&NativeCodeCursor> {
        match &self.kind {
            NativePendingWorkKind::Source { position, .. } => Some(position),
            NativePendingWorkKind::Resume(_) => None,
        }
    }

    pub(super) fn position_mut(&mut self) -> Option<&mut NativeCodeCursor> {
        match &mut self.kind {
            NativePendingWorkKind::Source { position, .. } => Some(position),
            NativePendingWorkKind::Resume(_) => None,
        }
    }

    pub(super) fn source(position: NativeCodeCursor, role: NativeSourceRangeRole) -> Self {
        Self {
            kind: NativePendingWorkKind::Source { position, role },
        }
    }

    pub(super) fn resume(continuation: NativeRuntimeContinuation<V, I>) -> Self {
        Self {
            kind: NativePendingWorkKind::Resume(continuation),
        }
    }

    pub(super) fn code_owner(&self) -> Option<crate::plan::code::RuntimeCodeOwner> {
        match &self.kind {
            NativePendingWorkKind::Source { position, .. } => Some(position.owner()),
            NativePendingWorkKind::Resume(
                NativeRuntimeContinuation::LoopNext { source }
                | NativeRuntimeContinuation::WhileNext { source }
                | NativeRuntimeContinuation::WhileLetNext { source }
                | NativeRuntimeContinuation::ForNext { source, .. }
                | NativeRuntimeContinuation::CompleteFormatOperand { source }
                | NativeRuntimeContinuation::CompleteLetScope { source },
            ) => Some(source.owner()),
            NativePendingWorkKind::Resume(_) => None,
        }
    }

    pub(super) fn logical_pending_count(&self) -> Option<usize> {
        match &self.kind {
            NativePendingWorkKind::Source {
                role: NativeSourceRangeRole::FunctionRoot,
                ..
            } => Some(0),
            NativePendingWorkKind::Source { position, .. } => position
                .remaining_operation_count()
                .or_else(|| position.operation_index().is_none().then_some(1)),
            NativePendingWorkKind::Resume(_) => Some(1),
        }
    }

    pub(super) fn views<'a>(
        &'a self,
        plan: &'a crate::plan::RuntimePlan,
    ) -> Result<
        impl Iterator<Item = NativePendingWorkView<'a>>,
        crate::value::AwbcRuntimeValueSnapshotError,
    > {
        let mut operations = None;
        let mut single = None;
        match &self.kind {
            NativePendingWorkKind::Source { position, role } => {
                if let Some(source) = position.operations(plan) {
                    operations = Some(source);
                    if source.is_empty() && *role == NativeSourceRangeRole::FunctionRoot {
                        single = Some(NativePendingWorkView::EndOfFunctionBody(position));
                    }
                } else if let Some(expression) = position.expression(plan) {
                    single = Some(NativePendingWorkView::Expression(expression));
                } else {
                    return Err(crate::value::AwbcRuntimeValueSnapshotError::Message {
                        message: "native work references an absent original body".into(),
                    });
                }
            }
            NativePendingWorkKind::Resume(resume) => {
                single = Some(NativePendingWorkView::Resume(resume.kind()));
            }
        }
        Ok(operations
            .into_iter()
            .flatten()
            .map(NativePendingWorkView::Source)
            .chain(single))
    }

    /// The enclosing frame admits the complete borrowed group before this
    /// projection. Only metadata is copied here; storage callbacks project
    /// values through their original executable and retained-value boundary.
    pub(super) fn project_storage<W, J, E>(
        &self,
        mut value: impl FnMut(&V) -> Result<W, E>,
        mut iterator: impl FnMut(&I) -> Result<J, E>,
    ) -> Result<NativePendingWork<W, J>, E> {
        use NativeRuntimeContinuation as Resume;
        let kind = match &self.kind {
            NativePendingWorkKind::Source { position, role } => NativePendingWorkKind::Source {
                position: position.clone(),
                role: *role,
            },
            NativePendingWorkKind::Resume(resume) => NativePendingWorkKind::Resume(match resume {
                Resume::Bind(bindings) => Resume::Bind(
                    bindings
                        .iter()
                        .map(|binding| {
                            Ok(RuntimeLocalBinding {
                                local: binding.local,
                                value: value(&binding.value)?,
                            })
                        })
                        .collect::<Result<_, E>>()?,
                ),
                Resume::LoopNext { source } => Resume::LoopNext {
                    source: source.clone(),
                },
                Resume::WhileNext { source } => Resume::WhileNext {
                    source: source.clone(),
                },
                Resume::WhileLetNext { source } => Resume::WhileLetNext {
                    source: source.clone(),
                },
                Resume::ForNext {
                    source,
                    iterator: i,
                } => Resume::ForNext {
                    source: source.clone(),
                    iterator: iterator(i)?,
                },
                Resume::CompleteFormatOperand { source } => Resume::CompleteFormatOperand {
                    source: source.clone(),
                },
                Resume::CompleteLetScope { source } => Resume::CompleteLetScope {
                    source: source.clone(),
                },
                Resume::EnterScheduledScope { identity, token } => Resume::EnterScheduledScope {
                    identity: identity.clone(),
                    token: *token,
                },
                Resume::ExitScheduledScope { token } => {
                    Resume::ExitScheduledScope { token: *token }
                }
                Resume::CompleteAwaitObserver => Resume::CompleteAwaitObserver,
            }),
        };
        Ok(NativePendingWork { kind })
    }

    /// Restoration moves already admitted storage, so neither source metadata
    /// nor runtime values require an additional copy at the ownership switch.
    pub(super) fn into_storage<W, J, E>(
        self,
        mut value: impl FnMut(V) -> Result<W, E>,
        mut iterator: impl FnMut(I) -> Result<J, E>,
    ) -> Result<NativePendingWork<W, J>, E> {
        use NativeRuntimeContinuation as Resume;
        let kind = match self.kind {
            NativePendingWorkKind::Source { position, role } => {
                NativePendingWorkKind::Source { position, role }
            }
            NativePendingWorkKind::Resume(resume) => NativePendingWorkKind::Resume(match resume {
                Resume::Bind(bindings) => Resume::Bind(
                    bindings
                        .into_iter()
                        .map(|binding| {
                            Ok(RuntimeLocalBinding {
                                local: binding.local,
                                value: value(binding.value)?,
                            })
                        })
                        .collect::<Result<_, E>>()?,
                ),
                Resume::LoopNext { source } => Resume::LoopNext { source },
                Resume::WhileNext { source } => Resume::WhileNext { source },
                Resume::WhileLetNext { source } => Resume::WhileLetNext { source },
                Resume::ForNext {
                    source,
                    iterator: i,
                } => Resume::ForNext {
                    source,
                    iterator: iterator(i)?,
                },
                Resume::CompleteFormatOperand { source } => {
                    Resume::CompleteFormatOperand { source }
                }
                Resume::CompleteLetScope { source } => Resume::CompleteLetScope { source },
                Resume::EnterScheduledScope { identity, token } => {
                    Resume::EnterScheduledScope { identity, token }
                }
                Resume::ExitScheduledScope { token } => Resume::ExitScheduledScope { token },
                Resume::CompleteAwaitObserver => Resume::CompleteAwaitObserver,
            }),
        };
        Ok(NativePendingWork { kind })
    }
}

impl<V: crate::value::RuntimeStoredValue, I: crate::value::RuntimeStoredValue>
    NativePendingWork<V, I>
{
    pub(super) fn admit_retained(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        admission.status()?;
        use crate::plan::FlowOp;
        use NativeRuntimeContinuation as Resume;
        let invalid = |message: &str| crate::value::AwbcRuntimeValueSnapshotError::Message {
            message: message.into(),
        };
        let Some(crate::task::RuntimeProgramOwner::Plan(plan)) = admission.program_owner().cloned()
        else {
            return Err(invalid("native work requires its original Plan executable"));
        };
        match &self.kind {
            NativePendingWorkKind::Source { position, role } => {
                position.admit_retained(admission)?;
                let tag = match role {
                    NativeSourceRangeRole::FunctionRoot => 0_u8,
                    NativeSourceRangeRole::TaskRoot => 1,
                    NativeSourceRangeRole::NestedBody => 2,
                };
                admission.validate_metadata(&tag)?;
                if *role == NativeSourceRangeRole::FunctionRoot
                    && (!position.is_root()
                        || !matches!(
                            position.owner(),
                            crate::plan::code::RuntimeCodeOwner::Function(_)
                        ))
                {
                    return Err(invalid(
                        "native function root has a different original code role",
                    ));
                }
                if *role == NativeSourceRangeRole::NestedBody
                    && (position.is_root() || position.expression(&plan).is_some())
                {
                    return Err(invalid(
                        "a native nested body cannot substitute an original code root",
                    ));
                }
            }
            NativePendingWorkKind::Resume(resume) => {
                admission.validate_metadata(&resume.kind().metadata_tag())?;
                match resume {
                    Resume::LoopNext { source }
                    | Resume::WhileNext { source }
                    | Resume::WhileLetNext { source }
                    | Resume::ForNext { source, .. }
                    | Resume::CompleteFormatOperand { source }
                    | Resume::CompleteLetScope { source } => {
                        source.admit_retained(admission)?;
                        let matches = matches!(
                            (resume, source.operation(&plan)),
                            (Resume::LoopNext { .. }, Some(FlowOp::Loop { .. }))
                                | (Resume::WhileNext { .. }, Some(FlowOp::While { .. }))
                                | (Resume::WhileLetNext { .. }, Some(FlowOp::WhileLet { .. }))
                                | (Resume::ForNext { .. }, Some(FlowOp::For { .. }))
                                | (
                                    Resume::CompleteFormatOperand { .. },
                                    Some(FlowOp::FormatOperandAttempt { .. })
                                )
                                | (
                                    Resume::CompleteLetScope { .. },
                                    Some(FlowOp::LetScope { .. })
                                )
                        );
                        if !matches {
                            return Err(invalid(
                                "native continuation differs from its original operation",
                            ));
                        }
                    }
                    Resume::Bind(_)
                    | Resume::EnterScheduledScope { .. }
                    | Resume::ExitScheduledScope { .. }
                    | Resume::CompleteAwaitObserver => {}
                }
                match resume {
                    Resume::Bind(bindings) => {
                        admission.validate_metadata_collection(bindings.len())?;
                        for field in bindings {
                            admission.validate_metadata(&field.local.get())?;
                            let ty = plan
                                .local_type_in_context(field.local, binding)
                                .map_err(|error| invalid(&error.to_string()))?;
                            let expected = plan
                                .type_table()
                                .get(ty)
                                .expect(
                                    "the original local type was checked before value admission",
                                )
                                .semantic_identity();
                            admission.validate_owned_typed_borrowed_in_invocation(
                                field.value.borrowed_value(),
                                expected,
                                binding,
                            )?;
                        }
                    }
                    Resume::ForNext { iterator, .. } => {
                        if !matches!(
                            iterator.borrowed_view(),
                            crate::value::RuntimeValueView::Iterator(_)
                        ) {
                            return Err(invalid(
                                "native iterator continuation contains another value kind",
                            ));
                        }
                        admission.validate_owned_borrowed(iterator.borrowed_value())?;
                    }
                    Resume::EnterScheduledScope { identity, token } => {
                        admission.validate_metadata(identity)?;
                        admission.validate_metadata(token)?;
                    }
                    Resume::ExitScheduledScope { token } => admission.validate_metadata(token)?,
                    Resume::LoopNext { .. }
                    | Resume::WhileNext { .. }
                    | Resume::WhileLetNext { .. }
                    | Resume::CompleteFormatOperand { .. }
                    | Resume::CompleteLetScope { .. }
                    | Resume::CompleteAwaitObserver => {}
                }
            }
        }
        Ok(())
    }
}

impl<V: crate::value::RuntimeStoredValue, I: crate::value::RuntimeStoredValue> NativeStoredWork
    for NativePendingWork<V, I>
{
    fn logical_count(&self) -> Option<usize> {
        self.logical_pending_count()
    }

    fn admit_work(
        &self,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), crate::value::AwbcRuntimeValueSnapshotError> {
        self.admit_retained(admission, binding)
    }
}

impl Clone for NativePendingWorkRollbackImage {
    fn clone(&self) -> Self {
        self.project_storage(
            |value| Ok::<_, std::convert::Infallible>(value.clone()),
            |iterator| Ok(iterator.clone()),
        )
        .unwrap_or_else(|error| match error {})
    }
}

impl NativePendingWorkRollbackImage {
    pub(super) fn from_admitted_live(
        work: &NativePendingWork,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        use crate::value::AwbcRuntimeValueSnapshot as Snapshot;
        work.project_storage(
            |value| {
                Snapshot::from_admitted_runtime_value(value, owner)
                    .map_err(|error| error.to_string())
            },
            |iterator| {
                Snapshot::from_runtime_iterator_for_program(iterator, owner)
                    .map_err(|error| error.to_string())
            },
        )
    }

    pub(super) fn into_admitted_live(
        self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<NativePendingWork, String> {
        self.into_storage(
            |value| {
                value
                    .into_runtime_value_for_program(owner)
                    .map_err(|error| error.to_string())
            },
            |iterator| {
                iterator
                    .into_runtime_iterator_for_program(owner)
                    .map_err(|error| error.to_string())
            },
        )
    }
}
