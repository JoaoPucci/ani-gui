//! Stopping the backend: what asks for it, and how the server winds
//! down once something has.
//!
//! Stopping has to be something the backend does, not something done
//! to it. The downloader runs yt-dlp and ffmpeg in process groups of
//! their own — that is how cancelling a download takes a tool's whole
//! tree down — so nothing outside the backend can reach them: a signal
//! sent to the backend's group stops there. What stops a tool is the
//! guard that owns it ([`crate::spawn::TreeKillChild`]), and a guard
//! runs only when its task is dropped. A signal left to its default
//! action kills the process with every guard unrun.
//!
//! So each way the backend is told to stop is a request, and they all
//! lead to the same wind-down: stop accepting, let requests in flight
//! finish for a bounded time, then return so the runtime is torn down
//! and every remaining task — a running download among them — dropped.

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
    /// SIGTERM: what Electron's quit sends on Linux and macOS.
    Terminated,
    /// SIGINT, or Ctrl+C in a Windows console: a backend run by hand.
    Interrupted,
    /// SIGHUP: the terminal a backend was run from by hand has closed.
    HungUp,
}

/// A future that resolves with the first request to stop.
///
/// The signal handlers are installed by this call, not by the future's
/// first poll, so a caller that makes it before announcing the backend
/// is up leaves no window in which a quit would still kill outright.
/// Call it inside the runtime: installing a handler needs its reactor.
///
/// Windows has no SIGTERM. A quit there runs `taskkill /F /T` against
/// the backend, which ends its whole tree by parent pid — tools
/// included — without asking, so there is nothing to handle.
pub fn requested() -> impl Future<Output = Reason> + Send + 'static {
    let signalled = signalled();
    async move {
        tokio::select! {
            () = crate::parent_watch::parent_gone() => Reason::ParentGone,
            reason = signalled => reason,
        }
    }
}

#[cfg(unix)]
fn signalled() -> impl Future<Output = Reason> + Send + 'static {
    use tokio::signal::unix::SignalKind;
    let terminated = received(SignalKind::terminate());
    let interrupted = received(SignalKind::interrupt());
    let hung_up = received(SignalKind::hangup());
    async move {
        tokio::select! {
            () = terminated => Reason::Terminated,
            () = interrupted => Reason::Interrupted,
            () = hung_up => Reason::HungUp,
        }
    }
}

/// Install the handler for `kind` now; the future resolves when the
/// signal arrives. A handler that cannot be installed is logged and
/// never resolves — the signal then keeps its default action, and a
/// failure to listen is not a request to stop.
#[cfg(unix)]
fn received(kind: tokio::signal::unix::SignalKind) -> impl Future<Output = ()> + Send + 'static {
    let stream = tokio::signal::unix::signal(kind);
    async move {
        match stream {
            Ok(mut stream) => {
                if stream.recv().await.is_some() {
                    return;
                }
            }
            Err(e) => tracing::warn!(error = %e, ?kind, "signal handler not installed"),
        }
        std::future::pending::<()>().await;
    }
}

#[cfg(not(unix))]
async fn signalled() -> Reason {
    match tokio::signal::ctrl_c().await {
        Ok(()) => Reason::Interrupted,
        Err(e) => {
            tracing::warn!(error = %e, "Ctrl+C handler not installed");
            std::future::pending().await
        }
    }
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
