//! Reading an `EXT-X-DEFINE` line: its attribute list, and a query
//! parameter's percent-decoded value.

use std::collections::HashMap;

/// `text` with every `%XX` escape decoded, an escape that is not one
/// left as written; nothing if the result is not UTF-8 or holds what
/// a quoted string cannot carry — a `"`, a CR or an LF.
pub(super) fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                #[allow(clippy::cast_possible_truncation)]
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out)
        .ok()
        .filter(|v| !v.contains(['"', '\r', '\n']))
}

/// An attribute list's values by name, quoted values unquoted.
pub(super) fn attributes(list: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut rest = list.trim();
    while let Some(eq) = rest.find('=') {
        let name = rest[..eq].trim().to_string();
        let after = &rest[eq + 1..];
        let (value, next) = if let Some(quoted) = after.strip_prefix('"') {
            let end = quoted.find('"').unwrap_or(quoted.len());
            (&quoted[..end], quoted.get(end + 1..).unwrap_or(""))
        } else {
            let end = after.find(',').unwrap_or(after.len());
            (&after[..end], &after[end..])
        };
        out.insert(name, value.to_string());
        rest = next.trim_start_matches(',').trim_start();
    }
    out
}
