//! How fast a download is going, read from what its tool reports.
//!
//! yt-dlp is asked for a progress line of the app's own shape carrying
//! the bytes it has downloaded, and ffmpeg for its key=value progress,
//! whose `total_size` is the bytes it has written. Each reads as a byte
//! count; [`RateMeter`] turns the counts into a speed over the last few
//! seconds, which the transfer reports to the dock once a second.

use std::collections::VecDeque;
use std::time::Duration;

use tokio::time::Instant;

/// How often a running transfer reports its speed to the dock.
pub(crate) const RATE_REPORT_EVERY: Duration = Duration::from_secs(1);

/// The status line a speed report travels as, ahead of the bytes a
/// second, among the other `status.download.` lines the dock reads.
pub(crate) const RATE_STATUS: &str = "status.download.rate";

/// The line that reports `bytes_per_second` to the dock.
#[must_use]
pub(crate) fn rate_report(bytes_per_second: f64) -> String {
    format!("{RATE_STATUS} {}", bytes_per_second.max(0.0).round() as u64)
}

/// The bytes of a progress line from either tool.
#[must_use]
pub(crate) fn progress_bytes(line: &str) -> Option<u64> {
    ytdlp_progress_bytes(line).or_else(|| ffmpeg_progress_bytes(line))
}

/// What yt-dlp's progress template prints ahead of the byte count.
pub(crate) const YTDLP_PROGRESS_MARK: &str = "ani-gui-progress";

/// How far back the speed looks: long enough to smooth over a fragment
/// arriving whole, short enough that a stall reads as one within
/// seconds.
pub(crate) const RATE_WINDOW: Duration = Duration::from_secs(5);

/// The bytes yt-dlp has downloaded, from a line its progress template
/// printed; `None` for any other line, or before yt-dlp knows.
#[must_use]
pub(crate) fn ytdlp_progress_bytes(line: &str) -> Option<u64> {
    let rest = line.trim().strip_prefix(YTDLP_PROGRESS_MARK)?;
    rest.trim().parse().ok()
}

/// The bytes ffmpeg has written, from its `total_size=` progress line;
/// `None` for any other line, or before ffmpeg knows.
#[must_use]
pub(crate) fn ffmpeg_progress_bytes(line: &str) -> Option<u64> {
    line.trim().strip_prefix("total_size=")?.parse().ok()
}

/// The speed of a transfer from the byte counts its tool reports: the
/// bytes gained since the oldest count within [`RATE_WINDOW`], over the
/// time since that count. A count lower than the last moves the older
/// counts down by the step, so the speed carries on through it.
#[derive(Debug, Default)]
pub(crate) struct RateMeter {
    samples: VecDeque<(Instant, u64)>,
}

impl RateMeter {
    /// Record that the tool had `bytes` at `at`.
    pub(crate) fn observe(&mut self, bytes: u64, at: Instant) {
        // A count that steps back — yt-dlp counting a retried
        // fragment again — moves the older counts down by the step, so
        // the speed carries on from the bytes gained since rather than
        // reading zero until the window refills.
        if let Some(&(_, last)) = self.samples.back() {
            if bytes < last {
                let step = last - bytes;
                for sample in &mut self.samples {
                    sample.1 = sample.1.saturating_sub(step);
                }
            }
        }
        self.samples.push_back((at, bytes));
        // Keep the window, and the newest count before it as the
        // baseline a stall is measured from.
        let cutoff = at.checked_sub(RATE_WINDOW).unwrap_or(at);
        while self.samples.len() > 1 && self.samples[1].0 < cutoff {
            self.samples.pop_front();
        }
    }

    /// The speed at `now`, in bytes a second: zero once nothing has
    /// been gained for a whole window.
    #[must_use]
    pub(crate) fn rate_at(&self, now: Instant) -> f64 {
        let Some(&(_, latest)) = self.samples.back() else {
            return 0.0;
        };
        let cutoff = now.checked_sub(RATE_WINDOW).unwrap_or(now);
        // The newest count from before the window, or the oldest within
        // it when the meter has nothing older.
        let Some(&(since, from)) = self
            .samples
            .iter()
            .rev()
            .find(|&&(t, _)| t < cutoff)
            .or_else(|| self.samples.front())
        else {
            return 0.0;
        };
        let elapsed = now
            .saturating_duration_since(since)
            .max(Duration::from_secs(1));
        latest.saturating_sub(from) as f64 / elapsed.as_secs_f64()
    }
}
