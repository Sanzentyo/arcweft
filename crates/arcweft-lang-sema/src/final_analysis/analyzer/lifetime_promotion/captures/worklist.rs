//! Monotone storage witnesses for checked expression and application states.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry},
    rc::Rc,
};

use arcweft_lang_hir::identity::{ExprId, ItemId};

use crate::callable::{CallableParameterCoordinate, CheckedCallSite};

use super::{FinalSemanticAnalysisError, ReceiverBorrowQuery};

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum TypeStorage {
    Ordinary,
    Borrowed,
    Callable,
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ApplicationStorage {
    pub(super) site: CheckedCallSite,
    pub(super) types: Box<[TypeStorage]>,
    pub(super) zero_lengths: Box<[bool]>,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct StorageFrameId(usize);

#[derive(Clone)]
pub(super) struct StorageFrame {
    pub(super) item: Option<ItemId>,
    pub(super) applications: Rc<[CheckedCallSite]>,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ValueOrigin {
    pub(super) expression: ExprId,
    pub(super) frame: StorageFrameId,
}

#[derive(Clone)]
pub(super) enum BorrowDependencies {
    Borrowed,
    Parameters(BTreeSet<CallableParameterCoordinate>),
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
    queue: VecDeque<ValueOrigin>,
}

impl CaptureWorklist {
    pub(super) fn new(root: ExprId) -> Self {
        let root = ValueOrigin {
            expression: root,
            frame: StorageFrameId(0),
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
                item: None,
                applications: Rc::from([]),
            }],
            applications: BTreeMap::new(),
            queue: VecDeque::from([root]),
        }
    }

    pub(super) fn is_root(&self, origin: ValueOrigin) -> bool {
        self.root == origin
    }

    pub(super) fn frame(
        &self,
        origin: ValueOrigin,
    ) -> Result<StorageFrame, FinalSemanticAnalysisError> {
        self.frames
            .get(origin.frame.0)
            .cloned()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)
    }

    pub(super) fn intern(
        &mut self,
        key: ApplicationStorage,
        item: ItemId,
        caller: &StorageFrame,
        receiver: &mut ReceiverBorrowQuery<'_>,
    ) -> Result<StorageFrameId, FinalSemanticAnalysisError> {
        let entry = match self.applications.entry(key) {
            Entry::Occupied(entry) => return Ok(*entry.get()),
            Entry::Vacant(entry) => entry,
        };
        receiver.charge(
            u64::try_from(caller.applications.len())
                .map_err(|_| FinalSemanticAnalysisError::AccountingOverflow)?
                + 1,
        )?;
        let mut applications = caller.applications.to_vec();
        applications.push(entry.key().site);
        let frame = StorageFrameId(self.frames.len());
        // The first path supplies the exact projection for this storage shape.
        self.frames.push(StorageFrame {
            item: Some(item),
            applications: Rc::from(applications),
        });
        entry.insert(frame);
        Ok(frame)
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
