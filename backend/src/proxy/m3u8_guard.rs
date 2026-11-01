//! The rewrite reads a playlist with m3u8-rs; the player reads the
//! result with hls.js. Where the two parsers disagree, text m3u8-rs
//! keeps as opaque could name something hls.js fetches, so the rewrite
//! reads its input and checks its output the way hls.js does:
//!
//! - [`super::m3u8_lines`] ends lines where hls.js does, before the
//!   parse, and tells a media playlist by hls.js's own test.
//! - [`keep_player_view_on_proxy`] reads each tag line of the rewritten
//!   manifest as hls.js reads an attribute list, where a name is
//!   trimmed (`URI ="…"` is its `URI`). A line in which any attribute
//!   whose name ends in `URI`, `URL` or `-LIST` would resolve off the
//!   proxy is dropped; an `#EXTINF` line loses its title instead, so
//!   the segment after it keeps its duration.
//!
//! What may stay off the proxy is what [`super::m3u8_reach`] keeps as
//! written: an inline key, init segment, session key or session data.

use url::Url;

use super::m3u8::ProxyOrigin;
use super::m3u8_reach::on_proxy;

/// `out` without any tag line through which hls.js would fetch
/// something off the proxy. `base` is the URL the player resolves the
/// manifest's relative URIs against: the proxy URL it fetched it from.
pub(super) fn keep_player_view_on_proxy(out: &str, origin: &ProxyOrigin) -> String {
    let Ok(base) = Url::parse(&format!("{}/s/", origin.base)) else {
        return out.to_owned();
    };
    let mut kept = String::with_capacity(out.len());
    for line in out.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        if !body.starts_with('#') || stays_on_proxy(body, &base, origin) {
            kept.push_str(line);
        } else if let Some(untitled) = untitled_extinf(body) {
            kept.push_str(&untitled);
            kept.push('\n');
        }
    }
    kept
}

/// An `#EXTINF` line without its title, which the player reads as no
/// attribute list.
fn untitled_extinf(line: &str) -> Option<String> {
    let duration = line.strip_prefix("#EXTINF:")?;
    Some(format!(
        "#EXTINF:{},",
        duration.split(',').next().unwrap_or("")
    ))
}

/// Whether every URI-naming attribute hls.js could read in `line`
/// resolves onto the proxy, or is inline media. Every `=` is taken as
/// the end of a name, which covers each split hls.js could make.
fn stays_on_proxy(line: &str, base: &Url, origin: &ProxyOrigin) -> bool {
    let inline_allowed = [
        "#EXT-X-KEY:",
        "#EXT-X-MAP:",
        "#EXT-X-SESSION-KEY:",
        "#EXT-X-SESSION-DATA:",
    ]
    .iter()
    .any(|tag| line.starts_with(tag));
    line.match_indices('=').all(|(eq, _)| {
        !names_a_fetch(&line[..eq])
            || value_stays(
                attribute_value(&line[eq + 1..]),
                base,
                origin,
                inline_allowed,
            )
    })
}

/// Whether an attribute name ending at this text — trimmed, as hls.js
/// trims it — names something to fetch.
fn names_a_fetch(before: &str) -> bool {
    let name = before
        .trim_end_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .to_ascii_uppercase();
    ["URI", "URL", "-LIST"].iter().any(|s| name.ends_with(s))
}

/// The value hls.js reads after an `=`: a quoted string up to the first
/// closing quote followed by a comma or the end, or else up to the next
/// comma; quotes around it are removed.
fn attribute_value(after: &str) -> &str {
    let quoted = after.strip_prefix('"').and_then(|rest| {
        rest.match_indices('"')
            .find(|&(i, _)| matches!(rest.as_bytes().get(i + 1), None | Some(b',')))
            .map(|(i, _)| &rest[..i])
    });
    quoted.unwrap_or_else(|| {
        let raw = after.split(',').next().unwrap_or("");
        raw.strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(raw)
    })
}

/// Whether the player's fetch of `value` stays off upstream.
fn value_stays(value: &str, base: &Url, origin: &ProxyOrigin, inline_allowed: bool) -> bool {
    if value.contains("{$") {
        return false;
    }
    base.join(value).is_ok_and(|url| {
        on_proxy(&url, origin) || (inline_allowed && !matches!(url.scheme(), "http" | "https"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quoted_value_runs_to_a_quote_before_a_comma_or_the_end() {
        assert_eq!(attribute_value("\"data:,x\",B=1"), "data:,x");
        assert_eq!(attribute_value("\"a\"b\""), "a\"b");
        assert_eq!(attribute_value("plain,B=1"), "plain");
        assert_eq!(attribute_value("\"open,B=1"), "\"open");
    }
}
