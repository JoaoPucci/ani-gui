//! Ending the backend when the Electron process that spawned it is gone.
//!
//! Placeholder: the watch is not implemented yet.

use std::io::Read;

/// Environment variable the Electron main process sets on the backend
/// it spawns, alongside a stdin pipe it holds open for its lifetime.
pub const PARENT_STDIN_ENV: &str = "ANI_GUI_PARENT_STDIN";

/// Read `reader` until end of file or an error, then flip the returned
/// receiver to `true`.
pub fn watch_for_eof<R: Read + Send + 'static>(reader: R) -> tokio::sync::watch::Receiver<bool> {
    let (_tx, rx) = tokio::sync::watch::channel(false);
    drop(reader);
    rx
}

#[cfg(test)]
#[path = "parent_watch_test.rs"]
mod tests;
