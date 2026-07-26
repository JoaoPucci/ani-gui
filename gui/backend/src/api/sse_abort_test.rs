//! Tests for `crate::api::sse_abort`. Extracted via `#[path]` per
//! the repo's inline-test CRAP convention.

use super::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;

/// Dropping an SSE response stream (client disconnected: page
/// unmount, EventSource.close, the play-cache's click bypass
/// aborting a prefetch, the download dock's Cancel) must abort the
/// resolution task driving it. A detached task would keep its
/// ani-cli child hitting allanime with nobody listening — the
/// renderer-side abort would be a lie.
#[tokio::test]
async fn dropping_the_sse_stream_aborts_the_resolution_task() {
    struct SetOnDrop(Arc<AtomicBool>);
    impl Drop for SetOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let reaped = Arc::new(AtomicBool::new(false));
    let canary = SetOnDrop(reaped.clone());
    let (tx, rx) = mpsc::unbounded_channel::<i32>();
    let handle = tokio::spawn(async move {
        let _canary = canary;
        std::future::pending::<()>().await;
        drop(tx);
    });
    let stream = abort_on_drop(UnboundedReceiverStream::new(rx), handle);
    drop(stream);
    // Abortion lands asynchronously — yield until the runtime reaps
    // the task and drops its future (and with it, in production, the
    // kill_on_drop ani-cli child).
    for _ in 0..100 {
        if reaped.load(Ordering::SeqCst) {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert!(
        reaped.load(Ordering::SeqCst),
        "dropping the stream must abort the driving task"
    );
}

/// The wrapper is transparent while the client stays connected:
/// items flow through untouched and natural channel closure still
/// ends the stream.
#[tokio::test]
async fn passes_items_through_and_ends_on_channel_close() {
    let (tx, rx) = mpsc::unbounded_channel::<i32>();
    let handle = tokio::spawn(async {});
    let mut stream = abort_on_drop(UnboundedReceiverStream::new(rx), handle);
    tx.send(7).expect("send");
    drop(tx);
    assert_eq!(stream.next().await, Some(7));
    assert_eq!(stream.next().await, None);
}
