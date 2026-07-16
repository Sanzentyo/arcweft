//! Crate-private grammar contract used while the lossless parser is staged.
//!
//! The current public CST remains the sole source-backed syntax authority until
//! the grammar tree, attachment table, and every consumer can switch in one
//! compiling cut. Keeping the final kind and role vocabulary here lets the
//! shadow parser use the accepted contract without exposing a second public
//! syntax model.

pub(crate) mod build;
pub(crate) mod event;
pub(crate) mod kinds;
