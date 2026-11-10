//! hls.js's attribute-list regex, `(.+?)=(".*?"|.*?)(?:,|$)`, applied
//! in sequence: the test oracle's reading of a tag line
//! ([`super::player_view`]).

/// hls.js's attribute regex, applied in sequence.
pub(super) fn attribute_list(mut rest: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    while let Some(eq) = rest.get(1..).and_then(|r| r.find('=')).map(|i| i + 1) {
        let name = rest[..eq].to_owned();
        let after = &rest[eq + 1..];
        let quoted = after.starts_with('"').then(|| {
            after.char_indices().skip(1).find(|&(i, c)| {
                c == '"' && matches!(after[i + 1..].chars().next(), None | Some(','))
            })
        });
        let (raw, next) = match quoted.flatten() {
            Some((end, _)) => (&after[..=end], &after[end + 1..]),
            None => after.split_once(',').map_or((after, ""), |(v, n)| (v, n)),
        };
        let value = raw
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(raw);
        out.push((name, value.to_owned()));
        rest = next.strip_prefix(',').unwrap_or(next);
    }
    out
}

/// What hls.js's media-playlist regex reads as a segment URI from the
/// rest of an `#EXTINF:` line: `#EXTINF:\s*(\d*(?:\.\d+)?)` takes the
/// number, and only a comma after it starts a title; anything else
/// after it is matched next, as a URI.
pub(super) fn extinf_tail(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("#EXTINF:")?.trim_start();
    let digits = rest.trim_start_matches(|c: char| c.is_ascii_digit());
    let after = match digits.strip_prefix('.') {
        Some(frac) if frac.starts_with(|c: char| c.is_ascii_digit()) => {
            frac.trim_start_matches(|c: char| c.is_ascii_digit())
        }
        _ => digits,
    };
    let uri = after.trim_start_matches(' ');
    (!after.starts_with(',') && !uri.trim().is_empty()).then_some(uri)
}
