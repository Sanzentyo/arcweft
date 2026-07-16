//! Shared argument coordinates used by schemas, facts, and query results.

use super::{CallableGroupIndex, CallableParameterIndex};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CallableParameterCoordinate {
    group: CallableGroupIndex,
    parameter: CallableParameterIndex,
}

impl CallableParameterCoordinate {
    pub const fn new(group: CallableGroupIndex, parameter: CallableParameterIndex) -> Self {
        Self { group, parameter }
    }
    pub const fn group(self) -> CallableGroupIndex {
        self.group
    }
    pub const fn parameter(self) -> CallableParameterIndex {
        self.parameter
    }
}
