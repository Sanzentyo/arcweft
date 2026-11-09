//! Monotone storage witnesses for checked expression and application states.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry},
    rc::Rc,
};

use arcweft_lang_hir::identity::{ExprId, ItemId};

use crate::callable::{
    CallableParameterCoordinate, CheckedCallSite, SpecializedDeclarationProjection,
};

use super::{
    FinalSemanticAnalysisError, ReceiverBorrowQuery, localization::RetainedBodyProjection,
};

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum TypeStorage {
    Ordinary,
    Borrowed,
    Callable,
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ApplicationStorage {
    pub(super) site: CheckedCallSite,
    pub(super) prefix: Option<CheckedCallSite>,
    pub(super) item: ItemId,
    pub(super) types: Box<[TypeStorage]>,
    pub(super) zero_lengths: Box<[bool]>,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct StorageFrameId(usize);

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum FrameOwner {
    Root,
    Function(ItemId),
    Closure {
        producer: ExprId,
        creator: StorageFrameId,
    },
}

#[derive(Clone)]
pub(super) struct StorageFrame {
    pub(super) owner: FrameOwner,
    pub(super) projections: Rc<[BodyProjection]>,
}

#[derive(Clone)]
pub(super) enum BodyProjection {
    SelectedCall(CheckedCallSite),
    SpecializedValue(Rc<SpecializedDeclarationProjection>),
    RetainedPrefix(Rc<RetainedBodyProjection>),
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct InvocationId(usize);

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Observation {
    Stored,
    Invoke(InvokedValue),
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct InvokedValue {
    pub(super) id: InvocationId,
    pub(super) specialization: Option<ValueSpecialization>,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ValueSpecialization {
    pub(super) owner: ExprId,
    pub(super) frame: StorageFrameId,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Invocation {
    pub(super) site: CheckedCallSite,
    pub(super) caller: StorageFrameId,
    pub(super) result: Observation,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ValueOrigin {
    pub(super) expression: ExprId,
    pub(super) frame: StorageFrameId,
    pub(super) observation: Observation,
}

/// An invocation caller may share the same recursive state as a producer.
/// Preserve the formal's role until that invocation returns to its caller.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ParameterDemand {
    pub(super) frame: StorageFrameId,
    pub(super) coordinate: CallableParameterCoordinate,
    pub(super) observation: Observation,
    pub(super) invocation_caller: Option<InvocationId>,
}

#[derive(Clone)]
pub(super) enum BorrowDependencies {
    Borrowed,
    Parameters(BTreeSet<ParameterDemand>),
}

impl Default for BorrowDependencies {
    fn default() -> Self {
        Self::Parameters(BTreeSet::new())
    }
}

impl BorrowDependencies {
    pub(super) const fn is_borrowed(&self) -> bool {
        matches!(self, Self::Borrowed)
    }

    pub(super) fn map_parameters(self, map: impl Fn(ParameterDemand) -> ParameterDemand) -> Self {
        match self {
            Self::Borrowed => Self::Borrowed,
            Self::Parameters(parameters) => {
                Self::Parameters(parameters.into_iter().map(map).collect())
            }
        }
    }

    pub(super) fn include(&mut self, other: Self) -> bool {
        let Self::Parameters(parameters) = self else {
            return false;
        };
        match other {
            Self::Borrowed => {
                *self = Self::Borrowed;
                true
            }
            Self::Parameters(other) => {
                let previous = parameters.len();
                parameters.extend(other);
                parameters.len() != previous
            }
        }
    }
}

#[derive(Default)]
struct Witness {
    dependencies: BorrowDependencies,
    queued: bool,
    consumers: BTreeSet<ValueOrigin>,
}

pub(super) struct CaptureWorklist {
    root: ValueOrigin,
    witnesses: BTreeMap<ValueOrigin, Witness>,
    frames: Vec<StorageFrame>,
    applications: BTreeMap<ApplicationStorage, StorageFrameId>,
    closures: BTreeMap<(ExprId, StorageFrameId), StorageFrameId>,
    invocation_ids: BTreeMap<Invocation, InvocationId>,
    invocations: Vec<Invocation>,
    queue: VecDeque<ValueOrigin>,
}

impl CaptureWorklist {
    pub(super) fn new(root: ExprId) -> Self {
        let root = ValueOrigin {
            expression: root,
            frame: StorageFrameId(0),
            observation: Observation::Stored,
        };
        Self {
            root,
            witnesses: BTreeMap::from([(
                root,
                Witness {
                    queued: true,
                    ..Witness::default()
                },
            )]),
            frames: vec![StorageFrame {
                owner: FrameOwner::Root,
                projections: Rc::from([]),
            }],
            applications: BTreeMap::new(),
            closures: BTreeMap::new(),
            invocation_ids: BTreeMap::new(),
            invocations: Vec::new(),
            queue: VecDeque::from([root]),
        }
    }

    pub(super) fn is_root(&self, origin: ValueOrigin) -> bool {
        self.root == origin
    }

    pub(super) fn frame(
        &self,
        id: StorageFrameId,
    ) -> Result<StorageFrame, FinalSemanticAnalysisError> {
        self.frames
            .get(id.0)
            .cloned()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)
    }

    pub(super) fn intern(
        &mut self,
        key: ApplicationStorage,
        parent: &[BodyProjection],
        projection: BodyProjection,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<StorageFrameId, FinalSemanticAnalysisError> {
        let entry = match self.applications.entry(key) {
            Entry::Occupied(entry) => return Ok(*entry.get()),
            Entry::Vacant(entry) => entry,
        };
        receiver.charge(
            u64::try_from(parent.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
                + 1,
        )?;
        let mut projections = parent.to_vec();
        projections.push(projection);
        let frame = StorageFrameId(self.frames.len());
        // The first path supplies the exact projection for this storage shape.
        self.frames.push(StorageFrame {
            owner: FrameOwner::Function(entry.key().item),
            projections: Rc::from(projections),
        });
        entry.insert(frame);
        Ok(frame)
    }

    pub(super) fn intern_closure(
        &mut self,
        producer: ExprId,
        creator: StorageFrameId,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<StorageFrameId, FinalSemanticAnalysisError> {
        let entry = match self.closures.entry((producer, creator)) {
            Entry::Occupied(entry) => return Ok(*entry.get()),
            Entry::Vacant(entry) => entry,
        };
        receiver.charge(1)?;
        let projections = Rc::clone(
            &self
                .frames
                .get(creator.0)
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
                .projections,
        );
        let frame = StorageFrameId(self.frames.len());
        self.frames.push(StorageFrame {
            owner: FrameOwner::Closure { producer, creator },
            projections,
        });
        entry.insert(frame);
        Ok(frame)
    }

    pub(super) fn intern_invocation(
        &mut self,
        invocation: Invocation,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<InvocationId, FinalSemanticAnalysisError> {
        let entry = match self.invocation_ids.entry(invocation) {
            Entry::Occupied(entry) => return Ok(*entry.get()),
            Entry::Vacant(entry) => entry,
        };
        receiver.charge(1)?;
        let id = InvocationId(self.invocations.len());
        self.invocations.push(invocation);
        entry.insert(id);
        Ok(id)
    }

    pub(super) fn invocation(
        &self,
        id: InvocationId,
    ) -> Result<Invocation, FinalSemanticAnalysisError> {
        self.invocations
            .get(id.0)
            .copied()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)
    }

    pub(super) fn next(&mut self) -> Result<Option<ValueOrigin>, FinalSemanticAnalysisError> {
        let Some(origin) = self.queue.pop_front() else {
            return Ok(None);
        };
        let witness = self
            .witnesses
            .get_mut(&origin)
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        witness.queued = false;
        Ok(Some(origin))
    }

    /// Returns the dependencies known so far and subscribes the consumer to
    /// new dependencies. A direct borrow supersedes every formal dependency.
    pub(super) fn demand(
        &mut self,
        provider: ValueOrigin,
        consumer: ValueOrigin,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<BorrowDependencies, FinalSemanticAnalysisError> {
        receiver.charge(1)?;
        let witness = match self.witnesses.entry(provider) {
            Entry::Vacant(entry) => {
                self.queue.push_back(provider);
                entry.insert(Witness {
                    queued: true,
                    ..Witness::default()
                })
            }
            Entry::Occupied(entry) => entry.into_mut(),
        };
        if let BorrowDependencies::Parameters(parameters) = &witness.dependencies {
            receiver.charge(
                u64::try_from(parameters.len())
                    .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?,
            )?;
            witness.consumers.insert(consumer);
        }
        Ok(witness.dependencies.clone())
    }

    pub(super) fn record(
        &mut self,
        origin: ValueOrigin,
        dependencies: BorrowDependencies,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let witness = self
            .witnesses
            .get_mut(&origin)
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        if !witness.dependencies.include(dependencies) {
            return Ok(());
        }
        let borrowed = witness.dependencies.is_borrowed();
        let consumers = std::mem::take(&mut witness.consumers);
        for &consumer in &consumers {
            receiver.charge(1)?;
            let downstream = self
                .witnesses
                .get_mut(&consumer)
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
            if !downstream.dependencies.is_borrowed() && !downstream.queued {
                downstream.queued = true;
                self.queue.push_back(consumer);
            }
        }
        if !borrowed {
            self.witnesses
                .get_mut(&origin)
                .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
                .consumers = consumers;
        }
        Ok(())
    }
}
