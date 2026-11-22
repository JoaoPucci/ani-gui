//! Which names in a download folder are a transfer's scratch as
//! versions 0.12.0 to 0.18.0 spelled it, beside the episode rather
//! than in the working folder.
//!
//! Those versions wrote `.ani-gui-<pid>-<32 lowercase hex>.part.mp4`
//! (the hex a v4 uuid's simple form) and handed it to yt-dlp, or to
//! ffmpeg when yt-dlp failed, which derive more names from it; a
//! subtitle's scratch was `<key>.part.vtt`, written by the app alone.
//! The parse accepts exactly those names and nothing near them, since
//! a name it accepts is removed:
//!
//! - `<key>.part.vtt` — a subtitle's scratch.
//! - `<key>.part.mp4` — the transfer's `-o` path, and ffmpeg's output.
//! - `<key>.part.temp.mp4` — yt-dlp's postprocessors' output before it
//!   replaces the file it fixed or merged.
//! - `<key>.part.f<format id>.<ext>` — one stream of a format yt-dlp
//!   fetched video and audio apart for, before the merge, with `<ext>`
//!   a container yt-dlp writes a stream in.
//! - after either of the last two shapes but one, what yt-dlp keeps
//!   beside a file it is writing: `.part`, the `.ytdl` resume state,
//!   and a fragment in flight, `.part-Frag<n>`, with its own `.part`.
//!
//! The format this version gives its names — a 16-digit key, and only
//! inside the working folder — never parses.

/// Whether `name` is a scratch file an earlier version left beside
/// its episodes.
#[must_use]
pub(crate) fn is_old_scratch(_name: &str) -> bool {
    false
}
