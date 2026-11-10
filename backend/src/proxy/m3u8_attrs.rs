//! An attribute list as hls.js reads it, for the check of the
//! rewritten manifest ([`super::m3u8_guard`]): a name trimmed, and the
//! value its regex `(".*?"|.*?)(?:,|$)` takes after the `=`.

/// Whether an attribute name ending at this text — trimmed, as hls.js
/// trims it — names something to fetch.
pub(super) fn names_a_fetch(before: &str) -> bool {
    let name = before
        .trim_end_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .to_ascii_uppercase();
    ["URI", "URL", "-LIST"].iter().any(|s| name.ends_with(s))
}

/// The value hls.js reads after an `=`: a quoted string up to the first
/// closing quote followed by a comma or the end, or else up to the next
/// comma; quotes around it are removed.
pub(super) fn attribute_value(after: &str) -> &str {
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

#[cfg(test)]
#[path = "m3u8_attrs_test.rs"]
mod tests;
