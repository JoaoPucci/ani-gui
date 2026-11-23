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

/// An `#EXTINF` line's number, as hls.js's media-playlist regex takes
/// it — `#EXTINF:\s*(\d*(?:\.\d+)?)` — and what follows it.
fn extinf_number(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("#EXTINF:")?.trim_start();
    let int = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let frac = rest[int..].strip_prefix('.').map_or(0, |f| {
        f.len() - f.trim_start_matches(|c: char| c.is_ascii_digit()).len()
    });
    let end = if frac > 0 { int + 1 + frac } else { int };
    Some(rest.split_at(end))
}

/// Whether the player reads an `#EXTINF` line as a duration and a
/// title only: hls.js starts a title only at a comma after the number,
/// and reads anything else that follows it as the segment's URI.
/// Every other line passes.
pub(super) fn extinf_reads_whole(line: &str) -> bool {
    extinf_number(line).is_none_or(|(_, after)| after.is_empty() || after.starts_with(','))
}

/// An `#EXTINF` line as its number alone, which the player reads as no
/// attribute list and no URI.
pub(super) fn untitled_extinf(line: &str) -> Option<String> {
    extinf_number(line).map(|(number, _)| format!("#EXTINF:{number},"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_extinf_line_reads_whole_only_when_a_comma_ends_its_number() {
        assert!(extinf_reads_whole("#EXTINF:4.5,title"));
        assert!(extinf_reads_whole("#EXTINF: 4"));
        assert!(extinf_reads_whole("#EXT-X-ENDLIST"));
        assert!(!extinf_reads_whole("#EXTINF:4https://evil.example/x"));
        assert!(!extinf_reads_whole("#EXTINF:4. x"));
        assert_eq!(
            untitled_extinf("#EXTINF:4.5https://e/x,X").as_deref(),
            Some("#EXTINF:4.5,")
        );
    }

    #[test]
    fn a_body_without_separators_is_borrowed() {
        assert!(matches!(player_lines(b"#EXTM3U\n"), Cow::Borrowed(_)));
        assert_eq!(
            player_lines("a\u{2028}b\u{2029}c".as_bytes()).as_ref(),
            b"a\nb\nc"
        );
    }
}
