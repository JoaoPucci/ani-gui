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

/// Read `reader` until end of file or an error, then flip the returned
/// receiver to `true`. Anything read before that is discarded — the
/// parent never writes, and a stray byte is no reason to stop.
///
/// The read runs on a plain thread rather than the runtime's blocking
/// pool: it blocks for the life of the process, and a runtime shutting
/// down waits for its blocking tasks.
pub fn watch_for_eof<R: Read + Send + 'static>(
    mut reader: R,
) -> tokio::sync::watch::Receiver<bool> {
    let (tx, rx) = tokio::sync::watch::channel(false);
    std::thread::Builder::new()
        .name("parent-watch".into())
        .spawn(move || {
            let mut buf = [0_u8; 256];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(_) => break,
                }
            }
            // A receiver reads the current value before it looks at
            // whether the sender is gone, so `true` outlives this thread.
            let _ = tx.send(true);
        })
        .expect("spawn the parent-watch thread");
    rx
}

#[cfg(test)]
#[path = "parent_watch_test.rs"]
mod tests;
