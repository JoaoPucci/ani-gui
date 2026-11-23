//! A gate a test shares under its mock's host ([`super`]), and what the
//! test reads from it. Test-only; split from [`super`] so the file the
//! app runs stays inside the CRAP gate's per-file bar.

use std::sync::Arc;

use super::{authority, shared, Class, Gate, Pace};

/// Share one gate under `url`'s host with `pace` until the guard drops.
pub(crate) fn share_for_tests(url: &str, pace: Pace) -> SharedForTests {
    let key = authority(url);
    let gate = Arc::new(Gate::new(pace));
    shared().insert(key.clone(), Arc::clone(&gate));
    SharedForTests(key, gate)
}

/// Removes a test's shared gate when dropped. While it lives, the test
/// reads what the gate let go and who waits on it.
pub(crate) struct SharedForTests(String, Arc<Gate>);

impl SharedForTests {
    /// The class of every request the gate let go, in the order it let
    /// them go. A mock server records requests in the order its own
    /// thread reads them, and two requests out at the same time can
    /// reach that thread in either order.
    pub(crate) fn let_go(&self) -> Vec<Class> {
        self.1
            .let_go
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// How many requests other than retries, and lanes holding lookups
    /// for a turn, wait for their turn now: a request counts from its
    /// first wait until its turn, its refusal once the turn would come
    /// past its wait, or its caller dropping it; a lane from its first
    /// wait until the request it hands over takes its turn.
    pub(crate) fn waiting(&self) -> usize {
        self.1
            .fresh_waiting
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for SharedForTests {
    fn drop(&mut self) {
        shared().remove(&self.0);
    }
}
