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
use std::sync::{Arc, Mutex, PoisonError};

/// Environment variable the Electron main process sets on the backend
/// it spawns, alongside a stdin pipe it holds open for its lifetime.
/// The watch runs only when it is `1`.
pub const PARENT_STDIN_ENV: &str = "ANI_GUI_PARENT_STDIN";

/// What the watch runs on its thread.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// A watch on the parent, armed as the backend starts.
///
/// It reads from the moment it is armed. An end of file before
/// [`Watch::serving`] means the parent went while the backend was
/// still starting up: nothing has been started that needs winding
/// down, so the watch runs the startup action — in the backend, an
/// exit — on its own thread, which is the only one free to act while
/// startup blocks the main one. After `serving`, the end of file
/// resolves the future `serving` returned, and the backend winds down
/// the way any other stop does.
pub struct Watch {
    /// `None` when there is nothing to report from: a backend run by
    /// hand, or a watch that could not start.
    report: Option<tokio::sync::oneshot::Receiver<()>>,
    serving: Arc<Mutex<bool>>,
}

/// Arm the watch this process was asked for: on stdin when Electron
/// set [`PARENT_STDIN_ENV`], otherwise one that never reports (a
/// backend run by hand). See [`arm`].
pub fn arm_from_env<F>(during_startup: F) -> Watch
where
    F: FnOnce() + Send + 'static,
{
    if std::env::var(PARENT_STDIN_ENV).as_deref() != Ok("1") {
        return Watch::unarmed();
    }
    arm(std::io::stdin(), during_startup)
}

/// Start watching `reader` now. See [`Watch`] for what an end of file
/// does before and after serving begins.
///
/// Anything read before the end of file is discarded — the parent
/// never writes, and a stray byte is no reason to stop. The read runs
/// on a plain thread rather than the runtime's blocking pool: it
/// blocks for the life of the process, and a runtime shutting down
/// waits for its blocking tasks.
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
///
/// A watch that cannot watch never reports and never acts. Neither a
/// spawn that fails nor a thread that ends without reporting says
/// anything about the parent, which is, as far as anyone knows, alive
/// and using this backend. Both are logged; the cost is that this
/// backend will not notice its parent going, which is the lesser one.
fn arm_with<R, F, S>(mut reader: R, during_startup: F, spawn: S) -> Watch
where
    R: Read + Send + 'static,
    F: FnOnce() + Send + 'static,
    S: FnOnce(Job) -> std::io::Result<()>,
{
    let serving = Arc::new(Mutex::new(false));
    let phase = Arc::clone(&serving);
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    let started = spawn(Box::new(move || {
        read_to_eof(&mut reader);
        // Held while the startup action runs, so serving cannot begin
        // underneath an exit.
        let serving = lock(&phase);
        if !*serving {
            during_startup();
        }
        drop(serving);
        let _ = tx.send(());
    }));
    match started {
        Ok(()) => Watch {
            report: Some(rx),
            serving,
        },
        Err(e) => {
            tracing::error!(error = %e, "parent watch could not start");
            Watch {
                report: None,
                serving,
            }
        }
    }
}

/// Read `reader` until end of file or an error, discarding the bytes.
fn read_to_eof<R: Read>(reader: &mut R) {
    let mut buf = [0_u8; 256];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return,
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return,
        }
    }
}

/// The phase flag, whatever a panicking holder left it as: a bool
/// cannot be left half-written.
fn lock(flag: &Mutex<bool>) -> std::sync::MutexGuard<'_, bool> {
    flag.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Watch {
    fn unarmed() -> Self {
        Self {
            report: None,
            serving: Arc::new(Mutex::new(false)),
        }
    }

    /// Serving begins: from here an end of file is reported through
    /// the returned future, which resolves once the parent is gone and
    /// never for a watch that cannot watch. Takes effect on the call,
    /// not on the first poll, so a caller that makes it before
    /// announcing the backend leaves no window between the two.
    pub fn serving(self) -> impl Future<Output = ()> + Send + 'static {
        *lock(&self.serving) = true;
        let report = self.report;
        async move {
            if let Some(rx) = report {
                if rx.await.is_ok() {
                    return;
                }
                tracing::error!("parent watch ended without a report");
            }
            std::future::pending::<()>().await;
        }
    }
}

#[cfg(test)]
#[path = "parent_watch_test.rs"]
mod tests;
