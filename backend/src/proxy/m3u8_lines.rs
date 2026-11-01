//! Where the player's reading of a playlist's lines differs from
//! m3u8-rs's, before the rewrite parses it ([`super::m3u8_guard`]):
//!
//! - [`player_lines`] ends lines where hls.js does. JavaScript's line
//!   terminators include U+2028 and U+2029, which m3u8-rs reads as
//!   ordinary text inside a title or an unknown tag.
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
