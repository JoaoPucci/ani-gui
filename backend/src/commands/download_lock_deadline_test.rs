//! A bound on every download-lock test that touches the filesystem, so
//! one that hangs fails the run with its name instead of holding the
//! job until CI cancels it.
//!
//! The harness only warns that a test has run past sixty seconds, and
//! a timeout around a test's future cannot help when what hangs is a
//! thread: a lock taken with a blocking call parks the thread that
//! drives the test, and nothing on it is polled again. That is how a
//! handover test once sat for 37 minutes on macOS. A watchdog thread
//! sees past that, and ends the process, because a parked thread
//! cannot be made to fail any other way.

/// How long one of these tests may run. The slowest finishes in about
/// seven seconds alone, and the lock's own deadlines in them are thirty
/// at most; the rest is room for a loaded runner.
const BOUND: std::time::Duration = std::time::Duration::from_secs(120);

/// Armed for the life of one test: dropping it — the test returning or
/// panicking — disarms it.
pub(crate) struct Deadline {
    _armed: std::sync::mpsc::Sender<()>,
}

impl Deadline {
    /// Arm the watchdog for the test running on this thread, which the
    /// harness names after the test.
    pub(crate) fn arm() -> Self {
        let name = std::thread::current()
            .name()
            .unwrap_or("a download-lock test")
            .to_owned();
        let (armed, disarmed) = std::sync::mpsc::channel::<()>();
        std::thread::spawn(move || {
            if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = disarmed.recv_timeout(BOUND) {
                // Straight to stderr: the harness captures what the
                // print macros write, and exiting discards it.
                let _ = std::io::Write::write_all(
                    &mut std::io::stderr(),
                    format!("{name} is still running after {BOUND:?}: ending the run, since a hung lock test would otherwise hold the job until it is cancelled\n").as_bytes(),
                );
                std::process::exit(101);
            }
        });
        Self { _armed: armed }
    }
}
