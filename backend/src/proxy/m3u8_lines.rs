//! Where the player's reading of a playlist's lines differs from
//! m3u8-rs's, before the rewrite parses it ([`super::m3u8_guard`]):
//!
//! - [`player_lines`] turns U+2028 and U+2029, which JavaScript ends a
//!   line at and m3u8-rs reads as ordinary text inside a title or an
//!   unknown tag, into newlines. m3u8-rs still reads a quoted value
//!   across lines; the check of the output reads those as the player
//!   does.
//! - [`reads_as_media`] is hls.js's test for a media playlist: any line
//!   starting `#EXTINF:` or `#EXT-X-TARGETDURATION:`.

use std::borrow::Cow;

/// `body` with every U+2028 and U+2029 replaced by a newline, so
/// m3u8-rs sees the lines hls.js would.
pub(super) fn player_lines(body: &[u8]) -> Cow<'_, [u8]> {
    const SEPARATORS: [&[u8]; 2] = ["\u{2028}".as_bytes(), "\u{2029}".as_bytes()];
    if !body.windows(3).any(|w| SEPARATORS.contains(&w)) {
        return Cow::Borrowed(body);
    }
    let mut out = Vec::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        if body.get(i..i + 3).is_some_and(|w| SEPARATORS.contains(&w)) {
            out.push(b'\n');
            i += 3;
        } else {
            out.push(body[i]);
            i += 1;
        }
    }
    Cow::Owned(out)
}

/// hls.js's test for a media playlist.
pub(super) fn reads_as_media(body: &[u8]) -> bool {
    body.split(|&b| b == b'\n' || b == b'\r')
        .any(|l| l.starts_with(b"#EXTINF:") || l.starts_with(b"#EXT-X-TARGETDURATION:"))
}

/// An `#EXTINF` line without its title, which the player reads as no
/// attribute list.
pub(super) fn untitled_extinf(line: &str) -> Option<String> {
    let duration = line.strip_prefix("#EXTINF:")?;
    Some(format!(
        "#EXTINF:{},",
        duration.split(',').next().unwrap_or("")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_without_separators_is_borrowed() {
        assert!(matches!(player_lines(b"#EXTM3U\n"), Cow::Borrowed(_)));
        assert_eq!(
            player_lines("a\u{2028}b\u{2029}c".as_bytes()).as_ref(),
            b"a\nb\nc"
        );
    }
}
