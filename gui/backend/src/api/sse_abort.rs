//! Abort-on-drop wrapper for SSE response streams.
//!
//! Lives in its own module so the router file's lizard count stays
//! under the firm CRAP ceiling; `api::mod` just calls
//! [`abort_on_drop`] from its two stream handlers.

use futures_util::stream::{Stream, StreamExt};

/// Wrap an SSE body stream so dropping it aborts the task feeding it.
/// Axum drops the body when the client disconnects (page unmount,
/// EventSource.close, the play-cache's click bypass aborting a
/// prefetch, the download dock's Cancel) — without this, the detached
/// resolution task keeps its ani-cli child running against allanime
/// with nobody listening. Aborting the task drops its in-flight
/// future, and the child is spawned `kill_on_drop`, so the subprocess
/// is reaped with it. When the stream instead ends naturally (channel
/// closed after the terminal event), the task has already finished
/// and the abort is a no-op.
pub(super) fn abort_on_drop<S: Stream>(
    stream: S,
    handle: tokio::task::JoinHandle<()>,
) -> impl Stream<Item = S::Item> {
    struct AbortGuard(tokio::task::JoinHandle<()>);
    impl Drop for AbortGuard {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let guard = AbortGuard(handle);
    stream.map(move |item| {
        let _guard = &guard;
        item
    })
}

#[cfg(test)]
#[path = "sse_abort_test.rs"]
mod tests;
