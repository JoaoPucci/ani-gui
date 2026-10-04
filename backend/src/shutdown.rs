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
//! finish for a bounded time ([`serve_until`]), then tear the runtime
//! down, which drops every remaining task — a running download among
//! them — and is bounded too ([`teardown`]).

use std::future::Future;
use std::time::Duration;

/// How long requests in flight get to finish once a stop is requested
/// before the server stops waiting for them.
///
/// Rarely spent. On a quit or a dead parent the requests' client is
/// gone as well, and a request whose connection has closed is dropped
/// at once; the grace is for one that does not notice, and for a
/// client that never hangs up.
///
/// It bounds the server's part of a stop, not the stop: the runtime's
/// teardown follows, under [`TEARDOWN_LIMIT`]. A backend asked to stop
/// has exited within the two together.
pub const REQUEST_GRACE: Duration = Duration::from_secs(3);

/// How long the runtime's teardown may take once the server has
/// stopped.
///
/// The teardown drops every task still running, which is what runs
/// the guards that stop download tools, and then waits for the
/// runtime's threads. The guards take milliseconds; what the limit
/// bounds is a blocking call that will not return, which a runtime
/// otherwise waits for without end. Past the limit the process exits
/// and the call is abandoned with it.
///
/// Generous on purpose. A guard cut short would leave a tool running,
/// and the limit costs time only when something is already stuck.
pub const TEARDOWN_LIMIT: Duration = Duration::from_secs(5);

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

/// Tear `runtime` down once the server has stopped: drop every task
/// still running, then wait for the runtime's threads — at most
/// `limit`. See [`TEARDOWN_LIMIT`].
///
/// Not a plain drop, which waits for blocking calls without limit, and
/// not `shutdown_background`, which does not wait at all: the tasks
/// are dropped on the runtime's own threads, so returning at once
/// could end the process before a guard had run.
pub fn teardown(runtime: tokio::runtime::Runtime, limit: Duration) {
    runtime.shutdown_timeout(limit);
}

#[cfg(test)]
#[path = "shutdown_test.rs"]
mod tests;
