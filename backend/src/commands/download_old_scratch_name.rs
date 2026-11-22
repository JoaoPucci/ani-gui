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
//! - after `<key>.part.mp4` or a split format's stream, what yt-dlp
//!   keeps beside a file it is writing: `.part`, the `.ytdl` resume
//!   state, and a fragment, `.part-Frag<n>`, with a `.part` of its own
//!   while it is in flight.
//!
//! yt-dlp merges a split format into mp4 when its streams are mp4 and
//! m4a, as an HLS listing's are. A merge into another container names
//! its files after `<key>.part.mp4.` instead; those are not parsed, and
//! would stay.
//!
//! The format this version gives its names — a 16-digit key, and only
//! inside the working folder — never parses.

const KEY_PREFIX: &str = ".ani-gui-";
/// The digits of a `u32` pid, at most.
const PID_MAX_DIGITS: usize = 10;
/// A uuid's simple form.
const ID_LEN: usize = 32;
/// The containers yt-dlp writes one stream of a split format in.
const STREAM_EXTS: [&str; 8] = ["mp4", "m4a", "m4v", "ts", "aac", "mp3", "webm", "mkv"];

/// Whether `name` is a scratch file an earlier version left beside
/// its episodes.
#[must_use]
pub(crate) fn is_old_scratch(name: &str) -> bool {
    after_key(name).is_some_and(|tail| {
        matches!(tail, ".part.vtt" | ".part.temp.mp4")
            || tail
                .strip_prefix(".part.")
                .is_some_and(|rest| is_tool_file(without_tool_suffix(rest)))
    })
}

/// What follows `.ani-gui-<pid>-<32 hex>` in `name`.
fn after_key(name: &str) -> Option<&str> {
    let rest = name.strip_prefix(KEY_PREFIX)?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 || digits > PID_MAX_DIGITS {
        return None;
    }
    let rest = rest[digits..].strip_prefix('-')?;
    let id = rest.get(..ID_LEN)?;
    id.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        .then(|| &rest[ID_LEN..])
}

/// `rest` without the one suffix yt-dlp adds beside a file it writes,
/// if it carries one.
fn without_tool_suffix(rest: &str) -> &str {
    if let Some(base) = rest.strip_suffix(".ytdl") {
        return base;
    }
    let partial = rest.strip_suffix(".part").unwrap_or(rest);
    match partial.rsplit_once(".part-Frag") {
        Some((base, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => base,
        _ => partial,
    }
}

/// Whether `file`, what follows `<key>.part.`, is a file yt-dlp wrote
/// to: `mp4`, or `f<format id>.<ext>`.
fn is_tool_file(file: &str) -> bool {
    if file == "mp4" {
        return true;
    }
    file.strip_prefix('f')
        .and_then(|rest| rest.rsplit_once('.'))
        .is_some_and(|(id, ext)| is_format_id(id) && STREAM_EXTS.contains(&ext))
}

/// A format id as yt-dlp puts it in a name: built from the listing's
/// group and rendition names, with whitespace and `,/+[]()` replaced
/// by `_` (on Windows, the characters Windows forbids in a name by
/// `#`), so any letter can be in it and none of those can. One holding
/// any of them, or a path separator, is not yt-dlp's: a copy of a
/// leftover, say, renamed `… (1).mp4` or `… - Copy.mp4` by a file
/// manager. A copy named with only letters, digits, `_`, `-` and `.`
/// added is indistinguishable from an id, and still matches.
fn is_format_id(id: &str) -> bool {
    !id.is_empty()
        && !id.chars().any(|c| {
            c.is_control()
                || c.is_whitespace()
                || matches!(c, '/' | '\\' | ',' | '+' | '[' | ']' | '(' | ')')
        })
}
