//! One transaction's graph and semantic-projection work accounting.

use std::cell::{Cell, RefCell};
use std::sync::atomic::Ordering;

use arcweft_lang_sema::types::{TypeProjectionControl, TypeProjectionNodeKind};

use super::{
    ProjectInstantiationControl, ProjectInstantiationError, ProjectInstantiationLimitKind as Kind,
    ProjectInstantiationOrigin,
};

#[derive(Clone, Copy, Default)]
pub(super) struct ProjectionWorkCounters {
    pub(super) work: u64,
    pub(super) structural_nodes: u64,
    pub(super) type_depth: u64,
}

/// Graph membership becomes immutable after discovery. Accounting continues
/// through materialization on this same owner; individual visits do not hold
/// an interior borrow while invoking any projection callback.
pub(super) struct ProjectInstantiationWork {
    control: ProjectInstantiationControl,
    counters: Cell<ProjectionWorkCounters>,
    failure: RefCell<Option<ProjectInstantiationError>>,
}

impl ProjectInstantiationWork {
    pub(super) fn new(control: ProjectInstantiationControl) -> Self {
        Self {
            control,
            counters: Cell::new(ProjectionWorkCounters::default()),
            failure: RefCell::new(None),
        }
    }

    pub(super) fn check(
        &self,
        origin: Option<ProjectInstantiationOrigin>,
    ) -> Result<(), ProjectInstantiationError> {
        if let Some(error) = self.failure.borrow().as_ref() {
            return Err(error.clone());
        }
        if self.control.cancellation.load(Ordering::Relaxed) {
            return Err(self.abort(ProjectInstantiationError::Cancelled { origin }));
        }
        Ok(())
    }

    pub(super) fn abort(&self, error: ProjectInstantiationError) -> ProjectInstantiationError {
        self.failure.borrow_mut().get_or_insert(error).clone()
    }

    fn proposed(
        &self,
        origin: ProjectInstantiationOrigin,
        work: u64,
        depth: Option<u64>,
    ) -> Result<ProjectionWorkCounters, ProjectInstantiationError> {
        self.check(Some(origin))?;
        let current = self.counters.get();
        let result = (|| {
            let work = current.work.checked_add(work).ok_or(
                ProjectInstantiationError::ArithmeticOverflow {
                    kind: Kind::Work,
                    origin,
                },
            )?;
            let structural_nodes = current
                .structural_nodes
                .checked_add(u64::from(depth.is_some()))
                .ok_or(ProjectInstantiationError::ArithmeticOverflow {
                    kind: Kind::StructuralNodes,
                    origin,
                })?;
            let type_depth = current.type_depth.max(depth.unwrap_or(0));
            for (kind, limit, attempted) in [
                (Kind::TypeDepth, self.control.limits.type_depth, type_depth),
                (
                    Kind::StructuralNodes,
                    self.control.limits.structural_nodes,
                    structural_nodes,
                ),
                (Kind::Work, self.control.limits.work, work),
            ] {
                if attempted > limit {
                    return Err(ProjectInstantiationError::LimitExceeded {
                        kind,
                        limit,
                        attempted,
                        origin,
                    });
                }
            }
            Ok(ProjectionWorkCounters {
                work,
                structural_nodes,
                type_depth,
            })
        })();
        result.map_err(|error| self.abort(error))
    }

    pub(super) fn charge_graph(
        &self,
        origin: ProjectInstantiationOrigin,
        work: u64,
        instances: u64,
        edges: u64,
    ) -> Result<(), ProjectInstantiationError> {
        let next = self.proposed(origin, work, None)?;
        for (kind, limit, attempted) in [
            (Kind::Instances, self.control.limits.instances, instances),
            (Kind::Edges, self.control.limits.edges, edges),
        ] {
            if attempted > limit {
                return Err(self.abort(ProjectInstantiationError::LimitExceeded {
                    kind,
                    limit,
                    attempted,
                    origin,
                }));
            }
        }
        self.counters.set(next);
        Ok(())
    }

    pub(super) fn type_control(
        &self,
        origin: ProjectInstantiationOrigin,
    ) -> ProjectInstanceTypeControl<'_> {
        ProjectInstanceTypeControl { work: self, origin }
    }

    #[cfg(test)]
    pub(super) fn counters(&self) -> ProjectionWorkCounters {
        self.counters.get()
    }
}

pub(super) struct ProjectInstanceTypeControl<'work> {
    work: &'work ProjectInstantiationWork,
    origin: ProjectInstantiationOrigin,
}

impl TypeProjectionControl for ProjectInstanceTypeControl<'_> {
    type Error = ProjectInstantiationError;

    fn check(&mut self) -> Result<(), Self::Error> {
        self.work.check(Some(self.origin))
    }

    fn visit_node(&mut self, _: TypeProjectionNodeKind, depth: u64) -> Result<(), Self::Error> {
        let next = self.work.proposed(self.origin, 1, Some(depth))?;
        self.work.counters.set(next);
        Ok(())
    }

    fn visit_binding(&mut self) -> Result<(), Self::Error> {
        let next = self.work.proposed(self.origin, 1, None)?;
        self.work.counters.set(next);
        Ok(())
    }
}
