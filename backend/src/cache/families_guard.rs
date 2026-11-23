//! Test builds only: the guard that holds the app's own cache reads
//! and writes to the registry ([`super`]).

use super::declared;

thread_local! {
    static UNDECLARED_ALLOWED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Test builds only: a row this thread stores or reads by its exact key
/// — `meta_cache_get`, `put`, `row`, `fetched_at`, `rebody`, `shorten` —
/// must belong to a declared family, so code that starts keeping a family the registry
/// does not name fails the suite instead of losing its rows to the
/// launch sweep. A case that seeds rows earlier builds kept — the
/// very rows the sweep is for — runs inside [`undeclared_allowed`].
///
/// Only the app's own code is held to it: a call made from a test file
/// (`*_test.rs`, named by the caller's location) seeds or inspects rows
/// as the case needs, legacy keys included.
#[track_caller]
pub(crate) fn check(key: &str) {
    let caller = std::panic::Location::caller().file();
    let from_a_case = caller.ends_with("_test.rs");
    let allowed = UNDECLARED_ALLOWED.with(std::cell::Cell::get);
    assert!(
        from_a_case || allowed || declared(key),
        "cache key {key:?} belongs to no family in cache::families::live (called from {caller})"
    );
}

/// Allow rows of undeclared families on this thread for the rest of
/// the case: for a file of cases that key their rows by a fixed key of
/// an earlier version, the key being beside the point. The test harness
/// runs each case on a thread of its own, so the allowance ends with
/// the case.
pub(crate) fn allow_undeclared_on_this_thread() {
    UNDECLARED_ALLOWED.with(|a| a.set(true));
}

/// Run `f` with rows of undeclared families allowed on this thread: for
/// a case about rows earlier builds kept.
pub(crate) fn undeclared_allowed<T>(f: impl FnOnce() -> T) -> T {
    /// Puts the thread's allowance back, on return and on unwind.
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            UNDECLARED_ALLOWED.with(|a| a.set(self.0));
        }
    }
    let _restore = Restore(UNDECLARED_ALLOWED.with(|a| a.replace(true)));
    f()
}
