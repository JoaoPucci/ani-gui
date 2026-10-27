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

/// Takes the key off the running map when the caller running it is done
/// with it — answered, or dropped before it answered, so a caller left
/// waiting runs the lookup itself.
struct Running<'a, V> {
    running: &'a Mutex<HashMap<String, broadcast::Sender<V>>>,
    key: &'a str,
    /// The run's own channel: what its entry under `key` holds.
    sender: broadcast::Sender<V>,
}

impl<V> Running<'_, V> {
    fn take(&self) -> Option<broadcast::Sender<V>> {
        self.running
            .lock()
            .expect("inflight map poisoned")
            .remove(self.key)
    }
}

impl<V> Drop for Running<'_, V> {
    fn drop(&mut self) {
        let _ = self.take();
    }
}

impl<V: Clone> Inflight<V> {
    /// Run `start` under `key`, or wait for the caller already running it
    /// and take a copy of its answer. A caller whose leader went away
    /// without answering runs the lookup itself.
    pub(crate) async fn run<F, Fut>(&self, key: String, start: F) -> V
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V>,
    {
        loop {
            let waiting = {
                let mut running = self.running.lock().expect("inflight map poisoned");
                match running.get(&key) {
                    Some(sender) => Ok(sender.subscribe()),
                    None => {
                        let sender = broadcast::channel(1).0;
                        running.insert(key.clone(), sender.clone());
                        Err(sender)
                    }
                }
            };
            match waiting {
                Ok(mut answer) => {
                    if let Ok(value) = answer.recv().await {
                        return value;
                    }
                    // The caller running it went away unanswered: ask again.
                }
                Err(sender) => {
                    let guard = Running {
                        running: &self.running,
                        key: &key,
                        sender,
                    };
                    let value = start().await;
                    // Off the map first, then answered: a caller arriving
                    // after this runs anew rather than waiting on a sender
                    // that has already spoken.
                    if let Some(sender) = guard.take() {
                        let _ = sender.send(value.clone());
                    }
                    return value;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "inflight_test.rs"]
mod tests;
