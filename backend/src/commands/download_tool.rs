//! How to tell a yt-dlp run went wrong from what it says.
//!
//! Reads yt-dlp specifically — it matches the tool's own warning line
//! — so it sits beside the downloader rather than with the generic
//! spawn plumbing in [`crate::spawn`].
//!
//! A companion used to live here that deleted a mislabeled file from
//! the user's download folder, identifying it by the MPEG-TS sync
//! byte and by yt-dlp having named it. Transfers write to a scratch
//! name now and are published only once whole, so a condemned file is
//! this run's own and goes without any of that care.

/// yt-dlp's own report that it left MPEG-TS inside an `.mp4`.
///
/// Anchored to the warning line, not to the stream. yt-dlp emits it
/// through `report_warning`, which prefixes `WARNING:` — measured
/// against 2025.08.11 the line reads:
///
/// ```text
/// WARNING: out: Possible MPEG-TS in MP4 container or malformed
/// AAC timestamps. Install ffmpeg to fix this automatically
/// ```
///
/// Everywhere else in the output the same words would be someone's
/// show title, echoed back by the destination and progress lines. A
/// title is whatever the provider called the entry, so searching the
/// whole stream lets a name decide whether downloads succeed.
///
/// Within the line the match stays on the stable middle: the leading
/// id and the trailing advice are respectively arbitrary and
/// reworded across releases, and pinning either would turn a future
/// yt-dlp into a silent regression.
pub(crate) fn yt_dlp_could_not_repackage(stderr: &str) -> bool {
    stderr.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("WARNING:") && line.contains("Possible MPEG-TS in MP4 container")
    })
}
