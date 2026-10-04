//! The moment an operation on a history began.
//!
//! What writes a history row, or what a row leaves behind, takes the
//! moment its work began, so that a removal of history can be told
//! apart from work begun before it. Nothing moves the moment yet:
//! every operation begins at the same one.

use std::path::Path;

/// A moment in a history's removals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Epoch(u64);

/// The moment an operation on the history at `path` begins.
#[must_use]
pub fn epoch(_path: &Path) -> Epoch {
    Epoch(0)
}
