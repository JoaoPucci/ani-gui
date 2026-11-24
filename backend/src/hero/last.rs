//! The last walk the hero's pick returned, kept in memory for the
//! Debug page. Nothing else reads it, and it does not outlive the
//! process.

use std::sync::{Arc, Mutex, PoisonError};

use super::Walk;

/// The last walk, shared by the routes that record and read it.
#[derive(Debug, Clone, Default)]
pub struct LastWalk(Arc<Mutex<Option<Walk>>>);

impl LastWalk {
    /// Keep `walk` as the last one, in place of any before it.
    pub fn record(&self, walk: Walk) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some(walk);
    }

    /// The last walk recorded, if any pick has run.
    #[must_use]
    pub fn get(&self) -> Option<Walk> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}
