//! Reading the tools' progress lines: yt-dlp's line of the app's own
//! shape, and ffmpeg's `total_size=`.

/// A progress line from either tool: the format yt-dlp is fetching, if
/// it names one, and the bytes.
#[must_use]
pub(crate) fn progress(line: &str) -> Option<(Option<String>, u64)> {
    ytdlp_progress(line).or_else(|| ffmpeg_progress_bytes(line).map(|bytes| (None, bytes)))
}

/// What yt-dlp's progress template prints ahead of the format's id and
/// the byte count.
pub(crate) const YTDLP_PROGRESS_MARK: &str = "ani-gui-progress";

/// The bytes yt-dlp has downloaded, from a line its progress template
/// printed; `None` for any other line, or before yt-dlp knows.
#[must_use]
#[cfg(test)]
pub(crate) fn ytdlp_progress_bytes(line: &str) -> Option<u64> {
    ytdlp_progress(line).map(|(_, bytes)| bytes)
}

/// The format yt-dlp is fetching and the bytes it has of it, from a line
/// its progress template printed — the format's id ahead of the count,
/// or the count alone; `None` for any other line, or before yt-dlp knows.
/// The count is the last word, and the id everything between the mark
/// and it, so an id is read whole whatever it holds — yt-dlp replaces an
/// id's whitespace today, and the reading does not depend on it.
#[must_use]
pub(crate) fn ytdlp_progress(line: &str) -> Option<(Option<String>, u64)> {
    let rest = line.trim().strip_prefix(YTDLP_PROGRESS_MARK)?.trim();
    let (format, count) = rest
        .rsplit_once(' ')
        .map_or((None, rest), |(id, count)| (Some(id.trim()), count));
    let bytes = count.parse().ok()?;
    Some((format.filter(|id| !id.is_empty()).map(str::to_owned), bytes))
}

/// The bytes ffmpeg has written, from its `total_size=` progress line;
/// `None` for any other line, or before ffmpeg knows.
#[must_use]
pub(crate) fn ffmpeg_progress_bytes(line: &str) -> Option<u64> {
    line.trim().strip_prefix("total_size=")?.parse().ok()
}
