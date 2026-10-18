//! Test-only: what a test sees of the rows this thread stores, as each
//! one is stored. Ordering is the thing it is for — what else had or
//! had not been written by the time a row landed — which a read after
//! the work returns cannot tell.

use std::cell::RefCell;

/// What a test runs as each row lands.
type Observer = Box<dyn Fn(&str)>;

thread_local! {
    static OBSERVER: RefCell<Option<Observer>> = const { RefCell::new(None) };
}

/// Run `f` with the key of each row this thread stores, until the
/// returned guard drops. A `#[tokio::test]` runs its futures on its
/// own thread, so the rows its work stores are the ones it sees.
#[must_use]
pub fn observe_puts(f: impl Fn(&str) + 'static) -> Observing {
    OBSERVER.with(|o| *o.borrow_mut() = Some(Box::new(f)));
    Observing
}

/// Stops the observation when dropped.
pub struct Observing;

impl Drop for Observing {
    fn drop(&mut self) {
        OBSERVER.with(|o| *o.borrow_mut() = None);
    }
}

/// A row is about to be stored under `key`.
pub(super) fn saw(key: &str) {
    OBSERVER.with(|o| {
        if let Some(f) = o.borrow().as_ref() {
            f(key);
        }
    });
}
