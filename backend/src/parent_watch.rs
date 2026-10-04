//! Ending the backend when the Electron process that spawned it is gone.
//!
//! Electron spawns the backend with a stdin pipe it never writes to or
//! closes, and sets [`PARENT_STDIN_ENV`]. However the Electron main
//! process ends — a quit, a crash, SIGKILL, `taskkill /F` — the OS
//! closes its end of that pipe, and a read on the backend's end returns
//! end of file (on Windows, a broken pipe, which the standard library
//! also reports as end of file). That read is the whole watch: it needs
//! no platform code and no polling, and it sees every way the parent
//! can die, which a signal-based quit path cannot.
//!
//! A backend run by hand from a terminal has no such variable, and its
//! stdin — a terminal, or `/dev/null` — says nothing about a parent, so
//! it is not watched.

use std::future::Future;
use std::io::Read;

/// Environment variable the Electron main process sets on the backend
/// it spawns, alongside a stdin pipe it holds open for its lifetime.
/// The watch runs only when it is `1`.
pub const PARENT_STDIN_ENV: &str = "ANI_GUI_PARENT_STDIN";

/// What the watch runs on its thread.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// How the watch starts its thread.
type Spawner = Box<dyn FnOnce(Job) -> std::io::Result<()> + Send + 'static>;

/// A watch on the parent, armed as the backend starts.
pub struct Watch {
    start: Option<(Box<dyn Read + Send>, Spawner)>,
}

/// Arm the watch this process was asked for: on stdin when Electron
/// set [`PARENT_STDIN_ENV`], otherwise one that never reports (a
/// backend run by hand). See [`arm`].
pub fn arm_from_env<F>(during_startup: F) -> Watch
where
    F: FnOnce() + Send + 'static,
{
    if std::env::var(PARENT_STDIN_ENV).as_deref() != Ok("1") {
        return Watch { start: None };
    }
    arm(std::io::stdin(), during_startup)
}

/// Arm a watch on `reader`. `during_startup` is what an end of file
/// before [`Watch::serving`] runs; the one after it resolves the
/// future `serving` returns.
pub fn arm<R, F>(reader: R, during_startup: F) -> Watch
where
    R: Read + Send + 'static,
    F: FnOnce() + Send + 'static,
{
    arm_with(reader, during_startup, on_a_thread)
}

fn on_a_thread(job: Job) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("parent-watch".into())
        .spawn(job)
        .map(drop)
}

/// [`arm`] with the thread spawn explicit — the seam the tests use to
/// make it fail.
fn arm_with<R, F, S>(reader: R, during_startup: F, spawn: S) -> Watch
where
    R: Read + Send + 'static,
    F: FnOnce() + Send + 'static,
    S: FnOnce(Job) -> std::io::Result<()> + Send + 'static,
{
    drop(during_startup);
    Watch {
        start: Some((Box::new(reader), Box::new(spawn))),
    }
}

impl Watch {
    /// The backend is serving. The returned future resolves once the
    /// parent is gone, and never for a watch that cannot watch.
    pub fn serving(self) -> impl Future<Output = ()> + Send + 'static {
        let start = self.start;
        async move {
            match start {
                Some((reader, spawn)) => until_eof_with(reader, spawn).await,
                None => std::future::pending().await,
            }
        }
    }
}

/// Resolves once `reader` reaches end of file or fails. Anything read
/// before that is discarded — the parent never writes, and a stray
/// byte is no reason to stop.
///
/// The read runs on a plain thread rather than the runtime's blocking
/// pool: it blocks for the life of the process, and a runtime shutting
/// down waits for its blocking tasks.
///
/// A watch that cannot watch never resolves. Neither a spawn that
/// fails nor a thread that ends without reporting says anything about
/// the parent, which is, as far as anyone knows, alive and using this
/// backend. Both are logged; the cost is that this backend will not
/// notice its parent going, which is the lesser one.
async fn until_eof_with<R, S>(mut reader: R, spawn: S)
where
    R: Read + Send + 'static,
    S: FnOnce(Job) -> std::io::Result<()>,
{
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let started = spawn(Box::new(move || {
        let mut buf = [0_u8; 256];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        let _ = tx.send(());
    }));
    match started {
        Err(e) => tracing::error!(error = %e, "parent watch could not start"),
        Ok(()) => {
            if rx.await.is_ok() {
                return;
            }
            tracing::error!("parent watch ended without a report");
        }
    }
    std::future::pending::<()>().await;
}

#[cfg(test)]
#[path = "parent_watch_test.rs"]
mod tests;
