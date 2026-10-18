//! Test-only: work run between two reads of the numbering store on
//! this thread, so a test can put a write exactly where a reader that
//! reads the store more than once would see half of it.

use std::cell::RefCell;

/// The work, and the reads still to go before it runs.
type Pending = Option<(usize, Box<dyn FnOnce()>)>;

thread_local! {
    static PENDING: RefCell<Pending> = const { RefCell::new(None) };
}

/// Run `f` just before this thread's `n`th read of the store from now
/// (1 is the next one). It runs once; a read inside it is not counted.
pub fn before_read(n: usize, f: impl FnOnce() + 'static) {
    PENDING.with(|p| *p.borrow_mut() = Some((n, Box::new(f))));
}

/// Forget work that never ran, so it cannot reach another case.
pub fn clear() {
    PENDING.with(|p| *p.borrow_mut() = None);
}

/// The store is about to be read.
pub(super) fn reading() {
    let due = PENDING.with(|p| {
        let mut p = p.borrow_mut();
        match p.as_mut() {
            Some((n, _)) if *n > 1 => {
                *n -= 1;
                None
            }
            Some(_) => p.take().map(|(_, f)| f),
            None => None,
        }
    });
    if let Some(f) = due {
        f();
    }
}
