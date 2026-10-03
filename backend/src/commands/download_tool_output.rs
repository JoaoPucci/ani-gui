//! Reading a download tool's output while it runs: its stderr lines go
//! to the dock as they come, and its progress on stdout feeds the run's
//! meter, whose speed is reported to the dock once a second. Split out
//! of the tool runner so the runner keeps to spawning, stopping and
//! the deadline.

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{ChildStderr, ChildStdout};

use super::download_progress::{progress_bytes, rate_report, RateMeter, RATE_REPORT_EVERY};
use crate::error::{AniError, Result};

/// Read the tool's stderr and stdout until both close. A stderr line is
/// handed to `on_line`, unless it is yt-dlp's report that it left
/// MPEG-TS under the .mp4 name, which sets `repackage_failed` and ends
/// the read. Progress on stdout feeds the meter, and once a second,
/// from the first progress the tool gives, the run's speed is handed to
/// `on_line` as a rate report.
///
/// # Errors
/// [`AniError::FfmpegMissing`] when yt-dlp reports the repackage failed.
pub(crate) async fn read_tool_output<F>(
    stderr: ChildStderr,
    stdout: ChildStdout,
    on_line: &mut F,
    repackage_failed: &mut bool,
) -> Result<()>
where
    F: FnMut(&str) + Send,
{
    let mut lines = BufReader::new(stderr).lines();
    let mut progress = BufReader::new(stdout).lines();
    let (mut lines_open, mut progress_open) = (true, true);
    // The run's speed, reported to the dock once a second from the
    // first progress the tool gives.
    let mut meter = RateMeter::default();
    let mut measured = false;
    let mut report = tokio::time::interval(RATE_REPORT_EVERY);
    report.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    report.tick().await;
    while lines_open || progress_open {
        tokio::select! {
            next = lines.next_line(), if lines_open => {
                let Ok(Some(raw)) = next else {
                    lines_open = false;
                    continue;
                };
                // Defense in depth behind the environment above:
                // the dock's DownloadProgress promises stripped
                // text, and a tool that colorizes anyway must not
                // reach it.
                let line = crate::spawn::strip_ansi(raw.as_bytes());
                // The run is condemned the moment yt-dlp reports
                // it left MPEG-TS under the .mp4 name: how it ends
                // stops mattering (exit 0 included), and stopping
                // now spares the rest of a transfer whose output
                // is already wrong. The armed guard takes the tool
                // down on return.
                if crate::commands::download_tool::yt_dlp_could_not_repackage(&line) {
                    *repackage_failed = true;
                    return Err(AniError::FfmpegMissing);
                }
                on_line(&line);
            }
            next = progress.next_line(), if progress_open => {
                let Ok(Some(raw)) = next else {
                    progress_open = false;
                    continue;
                };
                // Progress is the meter's, not the dock's text;
                // anything else on stdout is the tool's chatter.
                let line = crate::spawn::strip_ansi(raw.as_bytes());
                if let Some(bytes) = progress_bytes(&line) {
                    meter.observe(bytes, tokio::time::Instant::now());
                    measured = true;
                }
            }
            _ = report.tick() => {
                if measured {
                    on_line(&rate_report(meter.rate_at(tokio::time::Instant::now())));
                }
            }
        }
    }
    Ok(())
}
