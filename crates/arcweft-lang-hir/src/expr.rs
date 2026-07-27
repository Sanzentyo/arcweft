//! Private final-owner substrate for semantic HIR expressions.
//!
//! This module stays private until every expression consumer is ready for the
//! single public authority switch. Types move here only when their previous
//! provisional owner is deleted in the same compiling cut.

/// One canonical semantic identifier without source-range ownership.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HirName(Box<str>);
