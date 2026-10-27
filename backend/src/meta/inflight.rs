//! Identical lookups made at the same time share one.
//!
//! Two views asking for the same thing at once — the episode page and a
//! Continue Watching card filling the same show's thumbnails, a detail
//! read and the tracker episode map reading the same show — each sent
//! its own request, and each spent a request of AniList's allowance. A
//! lookup run through [`Inflight::run`] under a key another caller is
//! already running waits for that caller's answer and takes a copy of
//! it. Nothing is remembered once the answer is out: the next caller
//! with the key runs it again, and whatever caches the answer is the
//! lookup's own business.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;

use tokio::sync::broadcast;

/// The lookups running now, by key.
pub(crate) struct Inflight<V> {
    running: Mutex<HashMap<String, broadcast::Sender<V>>>,
}

impl<V: Clone> Default for Inflight<V> {
    fn default() -> Self {
        Self {
            running: Mutex::new(HashMap::new()),
        }
    }
}

impl<V: Clone> Inflight<V> {
    /// Run `start` under `key`. Each caller runs its own for now.
    pub(crate) async fn run<F, Fut>(&self, key: String, start: F) -> V
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V>,
    {
        let _ = (&self.running, key);
        start().await
    }
}

#[cfg(test)]
#[path = "inflight_test.rs"]
mod tests;
