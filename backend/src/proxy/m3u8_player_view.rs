//! The URIs a player would fetch from a rewritten manifest, read the way
//! hls.js reads it rather than the way m3u8-rs does: lines end at every
//! JavaScript line terminator, attribute lists are split by hls.js's
//! `(.+?)=(".*?"|.*?)(?:,|$)` with each name trimmed, and an attribute
//! whose name ends in `URI`, `URL` or `-LIST` names something to fetch.
//! Test-only: the oracle the rewrite's properties are checked against.

use url::Url;

/// The proxy every test origin is on.
pub(super) const PROXY: &str = "http://127.0.0.1:42337";

/// One URI the player would fetch, and the line it came from.
pub(super) struct Named {
    pub line: String,
    pub uri: String,
}

/// Every URI a player would read from `out`: each non-tag line, and
/// each URI-naming attribute of each tag line.
pub(super) fn player_uris(out: &str) -> Vec<Named> {
    let mut found = Vec::new();
    for line in out.split(['\n', '\r', '\u{2028}', '\u{2029}']) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with('#') {
            found.push(Named {
                line: line.to_owned(),
                uri: trimmed.to_owned(),
            });
            continue;
        }
        let attrs = line.split_once(':').map_or("", |(_, rest)| rest);
        for (name, value) in attribute_list(attrs) {
            let name = name.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
            if ["URI", "URL", "-LIST"].iter().any(|s| name.ends_with(s)) {
                found.push(Named {
                    line: line.to_owned(),
                    uri: value,
                });
            }
        }
    }
    found
}

/// hls.js's attribute regex, applied in sequence.
fn attribute_list(mut rest: &str) -> Vec<(String, String)> {
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

/// Whether the player's fetch of `named` stays off upstream: it
/// resolves onto the proxy, or it is inline media a key, init segment,
/// session key or session data names — or, in a media playlist, a
/// segment — that names nothing further.
pub(super) fn stays_on_proxy(named: &Named, media_playlist: bool) -> bool {
    let base = Url::parse(&format!("{PROXY}/s/x/seg?u=a&t=b")).expect("base");
    let Ok(url) = base.join(&named.uri) else {
        return false;
    };
    if named.uri.contains("{$") {
        return false;
    }
    if url.scheme() == "http" && url.host_str() == Some("127.0.0.1") && url.port() == Some(42_337) {
        return true;
    }
    let media = [
        "#EXT-X-KEY:",
        "#EXT-X-MAP:",
        "#EXT-X-SESSION-KEY:",
        "#EXT-X-SESSION-DATA:",
    ]
    .iter()
    .any(|t| named.line.starts_with(t))
        || (media_playlist && !named.line.trim_start().starts_with('#'));
    media && !matches!(url.scheme(), "http" | "https")
}

/// hls.js's test for a media playlist: any line, after any JavaScript
/// line terminator, that starts `#EXTINF:` or `#EXT-X-TARGETDURATION:`.
pub(super) fn reads_as_media(out: &str) -> bool {
    out.split(['\n', '\r', '\u{2028}', '\u{2029}'])
        .any(|l| l.starts_with("#EXTINF:") || l.starts_with("#EXT-X-TARGETDURATION:"))
}

/// The URIs in `out` the player would fetch past the proxy.
pub(super) fn escapes(out: &str) -> Vec<String> {
    let media = reads_as_media(out);
    player_uris(out)
        .into_iter()
        .filter(|n| !stays_on_proxy(n, media))
        .map(|n| n.uri)
        .collect()
}
