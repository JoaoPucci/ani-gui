//! Stopping the backend: what asks for it, and how the server winds
//! down once something has.
//!
//! Lifted out of the binary so the winding-down can be exercised
//! without a process. Behaviour is the binary's as it stood: the only
//! request to stop the backend hears is its parent being gone.

use std::future::Future;
use std::time::Duration;

/// How long in-flight requests get to finish once a stop is requested
/// before the server stops waiting for them.
pub const REQUEST_GRACE: Duration = Duration::from_secs(3);

/// Why the backend is stopping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The process that spawned the backend is gone — see
    /// [`crate::parent_watch`].
    ParentGone,
}

/// Resolves with the first request to stop.
pub async fn requested() -> Reason {
    crate::parent_watch::parent_gone().await;
    Reason::ParentGone
}

/// Serve `router` on `listener` until `stop` resolves, then stop
/// accepting connections and let requests already in flight finish —
/// for at most `grace`. Returns once they have, or once the grace is
/// spent, whichever is first.
///
/// # Errors
/// Whatever the server itself fails with.
pub async fn serve_until<F>(
    listener: tokio::net::TcpListener,
    router: axum::Router,
    stop: F,
    grace: Duration,
) -> std::io::Result<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let (stopping_tx, mut stopping_rx) = tokio::sync::watch::channel(false);
    let serve = axum::serve(listener, router).with_graceful_shutdown(async move {
        stop.await;
        let _ = stopping_tx.send(true);
    });
    tokio::select! {
        served = serve => served,
        () = async move {
            let _ = stopping_rx.wait_for(|stopping| *stopping).await;
            tokio::time::sleep(grace).await;
        } => {
            tracing::warn!("in-flight requests outlived the shutdown grace");
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "shutdown_test.rs"]
mod tests;
