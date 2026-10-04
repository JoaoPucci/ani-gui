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

/// A future that resolves with the first request to stop: a signal,
/// or `parent_gone` resolving — the parent watch's report (see
/// [`crate::parent_watch::Watch::serving`]).
///
/// On Unix the signal handlers are installed by this call, not by the
/// future's first poll, so a caller that makes it before announcing
/// the backend is up leaves no window in which a quit would still
/// kill outright. Call it inside the runtime: installing a handler
/// needs its reactor.
///
/// A signal the backend was started ignoring is left ignored, and its
/// future never resolves. A process starts that way when someone
/// means it to outlive the signal — `nohup` for a hangup, a shell
/// without job control for an interrupt on a background job — and a
/// handler would undo that. This holds on Linux, where the ignored
/// set can be read; elsewhere on Unix it cannot be read without
/// unsafe code, and every handler is installed. It never applies to
/// the backend Electron runs: a process Node spawns starts with every
/// disposition reset to its default.
///
/// Windows has no SIGTERM, and a quit there asks nothing: it runs
/// `taskkill /F /T` against the backend, which ends its whole tree by
/// parent pid, tools included. The one signal handled is Ctrl+C in a
/// console, for a backend run by hand; its handler starts when the
/// future is first polled, which serving does at once.
pub fn requested<P>(parent_gone: P) -> impl Future<Output = Reason> + Send + 'static
where
    P: Future<Output = ()> + Send + 'static,
{
    let signalled = signalled();
    async move {
        tokio::select! {
            () = parent_gone => Reason::ParentGone,
            reason = signalled => reason,
        }
    }
}

#[cfg(unix)]
fn signalled() -> impl Future<Output = Reason> + Send + 'static {
    use tokio::signal::unix::SignalKind;
    // Read before any handler goes in: installing one takes its signal
    // out of the ignored set.
    let ignored = inherited_ignores();
    let terminated = received(SignalKind::terminate(), ignored);
    let interrupted = received(SignalKind::interrupt(), ignored);
    let hung_up = received(SignalKind::hangup(), ignored);
    async move {
        tokio::select! {
            () = terminated => Reason::Terminated,
            () = interrupted => Reason::Interrupted,
            () = hung_up => Reason::HungUp,
        }
    }
}

/// Install the handler for `kind` now, unless `ignored` — the set the
/// process was started with — has it; the future resolves when the
/// signal arrives. A signal left ignored never resolves. Nor does a
/// handler that cannot be installed, which is logged: the signal then
/// keeps its default action, and a failure to listen is not a request
/// to stop.
#[cfg(unix)]
fn received(
    kind: tokio::signal::unix::SignalKind,
    ignored: u64,
) -> impl Future<Output = ()> + Send + 'static {
    let stream = if ignores(ignored, kind.as_raw_value()) {
        tracing::debug!(?kind, "ignored when the backend started; left ignored");
        None
    } else {
        Some(tokio::signal::unix::signal(kind))
    };
    async move {
        match stream {
            Some(Ok(mut stream)) => {
                if stream.recv().await.is_some() {
                    return;
                }
            }
            Some(Err(e)) => tracing::warn!(error = %e, ?kind, "signal handler not installed"),
            None => {}
        }
        std::future::pending::<()>().await;
    }
}

/// The signals this process is ignoring, as the kernel's mask: bit
/// `n - 1` for signal `n`. Empty when it cannot be read, which then
/// means every handler is installed.
#[cfg(target_os = "linux")]
fn inherited_ignores() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| sig_ign_mask(&status))
        .unwrap_or(0)
}

/// Nothing outside Linux says what a process ignores without unsafe
/// code, so nothing is taken to be.
#[cfg(all(unix, not(target_os = "linux")))]
fn inherited_ignores() -> u64 {
    0
}

/// The set of signals a process ignores, read off the text of its
/// `/proc/<pid>/status`: the `SigIgn` line, sixteen hexadecimal
/// digits. `None` when the line is missing or is not a mask.
#[cfg(target_os = "linux")]
fn sig_ign_mask(status: &str) -> Option<u64> {
    let mask = status
        .lines()
        .find_map(|line| line.strip_prefix("SigIgn:"))?;
    u64::from_str_radix(mask.trim(), 16).ok()
}

/// Whether `mask` has `signal` in it: bit `signal - 1`, for the
/// signals a 64-bit mask can name.
#[cfg(unix)]
fn ignores(mask: u64, signal: i32) -> bool {
    (1..=64).contains(&signal) && mask & (1_u64 << (signal - 1)) != 0
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

#[cfg(all(test, target_os = "linux"))]
#[path = "shutdown_prop_test.rs"]
mod prop_tests;
