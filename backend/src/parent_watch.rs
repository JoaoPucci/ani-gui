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

use std::io::Read;

/// Environment variable the Electron main process sets on the backend
/// it spawns, alongside a stdin pipe it holds open for its lifetime.
/// The watch runs only when it is `1`.
pub const PARENT_STDIN_ENV: &str = "ANI_GUI_PARENT_STDIN";

/// Resolves once `reader` reaches end of file or fails. Anything read
/// before that is discarded — the parent never writes, and a stray
/// byte is no reason to stop.
///
/// The read runs on a plain thread rather than the runtime's blocking
/// pool: it blocks for the life of the process, and a runtime shutting
/// down waits for its blocking tasks.
pub async fn until_eof<R: Read + Send + 'static>(reader: R) {
    until_eof_with(reader, on_a_thread).await;
}

/// What the watch runs on its thread.
type Job = Box<dyn FnOnce() + Send + 'static>;

fn on_a_thread(job: Job) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("parent-watch".into())
        .spawn(job)
        .map(drop)
}

/// [`until_eof`] with the thread spawn explicit — the seam the tests
/// use to make it fail. As the watch stood: a spawn that fails
/// panics, and a thread that ends without reporting counts as the
/// parent being gone.
async fn until_eof_with<R, S>(mut reader: R, spawn: S)
where
    R: Read + Send + 'static,
    S: FnOnce(Job) -> std::io::Result<()>,
{
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    spawn(Box::new(move || {
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
    }))
    .expect("spawn the parent-watch thread");
    let _ = rx.await;
}

/// Resolves once the parent is gone, or never when the parent did not
/// ask to be watched (a backend run by hand).
pub async fn parent_gone() {
    if std::env::var(PARENT_STDIN_ENV).as_deref() != Ok("1") {
        return std::future::pending().await;
    }
    until_eof(std::io::stdin()).await;
}

#[cfg(test)]
#[path = "parent_watch_test.rs"]
mod tests;
