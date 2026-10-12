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

#[path = "download_progress_line.rs"]
mod line;
// The tests read the parsers through this module, as the reader does.
#[cfg(test)]
pub(crate) use line::{ffmpeg_progress_bytes, ytdlp_progress, ytdlp_progress_bytes};
pub(crate) use line::{progress, YTDLP_PROGRESS_MARK};

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

/// How far back the speed looks: long enough to smooth over a fragment
/// arriving whole, short enough that a stall reads as one within
/// seconds.
pub(crate) const RATE_WINDOW: Duration = Duration::from_secs(5);

/// The speed of a transfer from the byte counts its tool reports: the
/// bytes gained since the newest count before [`RATE_WINDOW`] — or the
/// oldest within it, with nothing older — over the time since that
/// count. What is kept is what the tool gained, every
/// rise counted and a fall counted as nothing — a count lower than the
/// last takes nothing back from the bytes that arrived before it, and
/// the speed carries on from the bytes gained after it.
#[derive(Debug, Default)]
pub(crate) struct RateMeter {
    /// When, and how much the tool had gained by then.
    samples: VecDeque<(Instant, u64)>,
    /// The tool's last count, which the next is measured against.
    last_count: Option<u64>,
    /// Everything gained so far.
    gained: u64,
    /// What the run resumes from, each format's count measured from its
    /// own partial.
    resumed: Resumed,
    /// The format the last count was for, once one has been.
    format: Option<Option<String>>,
    /// What the current format resumes from, and since when, until its
    /// count passes it or [`RATE_WINDOW`] has gone by: counts below it
    /// are resumed bytes arriving, not a fall.
    floor: Option<(u64, Instant)>,
}

/// What a yt-dlp run resumes from: the partial file it keeps beside its
/// output, `whole`, or one per format it fetches apart, by format id —
/// the bytes it reports as downloaded when it starts each.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Resumed {
    pub(crate) whole: u64,
    pub(crate) formats: std::collections::HashMap<String, u64>,
}

impl RateMeter {
    /// A meter for a run that starts at `at` with `resumed` bytes
    /// already counted — what an earlier run of the same transfer left
    /// for it to resume from, which the tool reports as downloaded, or
    /// zero. The run's first count is measured against that, so the
    /// first block it fetches is part of its speed and the resumed bytes
    /// are not.
    #[cfg(test)]
    pub(crate) fn starting_from(resumed: u64, at: Instant) -> Self {
        Self::starting_from_partials(
            Resumed {
                whole: resumed,
                formats: std::collections::HashMap::new(),
            },
            at,
        )
    }

    /// A meter for a run that starts at `at` resuming from `resumed`:
    /// each format's first count is measured from that format's own
    /// partial, the first format's from the whole partial when it keeps
    /// none of its own, and a later one's from zero.
    pub(crate) fn starting_from_partials(resumed: Resumed, at: Instant) -> Self {
        Self {
            samples: VecDeque::from([(at, 0)]),
            last_count: Some(resumed.whole),
            floor: Some((resumed.whole, at)),
            resumed,
            ..Self::default()
        }
    }

    /// Record that the tool had `bytes` of `format` at `at`: a format
    /// other than the last count's starts from its own partial.
    pub(crate) fn observe_format(&mut self, format: Option<&str>, bytes: u64, at: Instant) {
        let format = format.map(str::to_owned);
        if self.format.as_ref() != Some(&format) {
            let own = format
                .as_ref()
                .and_then(|f| self.resumed.formats.get(f))
                .copied();
            let first = self.format.is_none();
            let from = own.unwrap_or(if first { self.resumed.whole } else { 0 });
            self.last_count = Some(from);
            self.floor = Some((from, at));
            self.format = Some(format);
        }
        self.observe(bytes, at);
    }

    /// Record that the tool had `bytes` at `at`.
    pub(crate) fn observe(&mut self, bytes: u64, at: Instant) {
        // Below what the run resumes from, a count is resumed bytes
        // still arriving — a fragment in flight reports its own partial
        // when its thread first does — and gains nothing, nor lowers
        // the point gains are measured from.
        // Held at most a window from the format's first count: the
        // fragments that start with it — as many as the run keeps in
        // flight — report their resumed bytes with their first counts,
        // so a point still unreached then holds bytes that may never be
        // reported, and gains count from where the count stands. A
        // fragment partial whose fragment starts later — behind a run
        // that keeps fewer in flight than the one it resumes, the
        // relay's turns, or a first connection that hangs — reports
        // after the window and reads as speed once (deferred-work.md).
        let held = self
            .floor
            .filter(|&(_, since)| at.saturating_duration_since(since) < RATE_WINDOW);
        if held.is_some_and(|(floor, _)| bytes < floor) {
            // Held: nothing gained, the point gains count from kept.
        } else if self.floor.is_some_and(|(floor, _)| bytes < floor) {
            self.floor = None;
            self.last_count = Some(bytes);
        } else {
            self.floor = None;
            // A count that steps back — yt-dlp counting a retried
            // fragment again — gains nothing, and the next rise is
            // gained from it.
            if let Some(last) = self.last_count {
                self.gained += bytes.saturating_sub(last);
            }
            self.last_count = Some(bytes);
        }
        self.samples.push_back((at, self.gained));
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
