//! The rewrite reads a playlist with m3u8-rs; the player reads the
//! result with hls.js. Where the two parsers disagree, text m3u8-rs
//! keeps as opaque could name something hls.js fetches, so the rewrite
//! reads its input and checks its output the way hls.js does:
//!
//! - [`super::m3u8_lines`] ends lines where hls.js does, before the
//!   parse, and tells a media playlist by hls.js's own test.
//! - [`keep_player_view_on_proxy`] reads the rewritten manifest's lines
//!   as hls.js ends them, `\r` included. A line not starting `#` must
//!   be on the proxy (or, in a media playlist, an inline segment) and
//!   carry no `#` — a quoted value m3u8-rs kept can carry lines of its
//!   own, and hls.js reads a master's tags wherever in a line they
//!   begin. A tag line is read as hls.js reads an
//!   attribute list, where a name is trimmed (`URI ="…"` is its `URI`);
//!   one in which any attribute whose name ends in `URI`, `URL` or
//!   `-LIST` would resolve off the proxy is dropped. An `#EXTINF` line
//!   loses its title instead, so the segment after it keeps its
//!   duration.
//!
//! What may stay off the proxy is what [`super::m3u8_reach`] keeps as
//! written: an inline key, init segment, session key or session data.

use url::Url;

use super::m3u8::ProxyOrigin;
use super::m3u8_reach::on_proxy;

/// `out` without any line through which hls.js would fetch something
/// off the proxy. Lines are read as the player reads them, ended by
/// `\r`, `\n`, U+2028 and U+2029 alike, and every line is checked: a
/// quoted value m3u8-rs kept can span lines of its own, so a line not
/// starting `#` need not be one the rewrite wrote. Relative URIs are
/// resolved against the proxy's own `/s/` path, where the player
/// fetched the manifest from.
pub(super) fn keep_player_view_on_proxy(out: &str, origin: &ProxyOrigin) -> String {
    let Ok(base) = Url::parse(&format!("{}/s/", origin.base)) else {
        return out.to_owned();
    };
    let media = super::m3u8_lines::reads_as_media(out.as_bytes());
    let mut kept = String::with_capacity(out.len());
    let body = out.strip_suffix('\n').unwrap_or(out);
    for line in body.split(['\n', '\r', '\u{2028}', '\u{2029}']) {
        let shown = if line.starts_with('#') {
            line_stays(line, &base, origin).then(|| line.to_owned())
        } else {
            uri_line_stays(line, &base, origin, media).then(|| line.to_owned())
        };
        if let Some(line) = shown.or_else(|| super::m3u8_lines::untitled_extinf(line)) {
            kept.push_str(&line);
            kept.push('\n');
        }
    }
    kept
}

/// Whether a line not starting `#` — which in a master names a variant
/// and in a media playlist a segment — is on the proxy, or, in a media
/// playlist, an inline segment, and carries no `#`. hls.js reads a
/// master's tags wherever in a line they begin, so a URI with a
/// fragment can carry one; the URIs the rewrite writes have none.
fn uri_line_stays(line: &str, base: &Url, origin: &ProxyOrigin, media: bool) -> bool {
    line.trim().is_empty() || (!line.contains('#') && value_stays(line.trim(), base, origin, media))
}

/// Whether every URI-naming attribute hls.js could read in `line`
/// resolves onto the proxy, or is inline media. Every `=` is taken as
/// the end of a name, which covers each split hls.js could make.
fn line_stays(line: &str, base: &Url, origin: &ProxyOrigin) -> bool {
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
#[path = "m3u8_guard_test.rs"]
mod tests;
