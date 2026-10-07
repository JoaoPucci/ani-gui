//! `EXT-X-DEFINE` variables, substituted before the rewrite reads a
//! URI. A playlist may build its URIs from `{$name}` references to
//! variables it defines by name and value, takes from a query
//! parameter of its own URL, or — a media playlist — imports from its
//! master. The rewrite resolves and proxies URIs, so it has to read the
//! URI the variables spell, not the reference.

use std::collections::HashMap;

use url::Url;

/// A playlist's variables, by name.
pub type Variables = HashMap<String, String>;

const DEFINE: &str = "#EXT-X-DEFINE:";

/// The variables a playlist served from `url` defines: by `NAME` and
/// `VALUE`, by `QUERYPARAM` from `url`'s query, and by `IMPORT` from
/// `imported`, the master's. A definition whose value is not there to
/// take defines nothing.
#[must_use]
pub fn defined(body: &str, url: &Url, imported: &Variables) -> Variables {
    let mut vars = Variables::new();
    for line in body.lines() {
        let Some(list) = line.trim_end().strip_prefix(DEFINE) else {
            continue;
        };
        let attrs = attributes(list);
        let found = if let (Some(name), Some(value)) = (attrs.get("NAME"), attrs.get("VALUE")) {
            Some((name.clone(), value.clone()))
        } else if let Some(param) = attrs.get("QUERYPARAM") {
            url.query_pairs()
                .find(|(k, _)| k == param)
                .map(|(_, v)| (param.clone(), v.into_owned()))
        } else if let Some(name) = attrs.get("IMPORT") {
            imported.get(name).map(|v| (name.clone(), v.clone()))
        } else {
            None
        };
        if let Some((name, value)) = found {
            vars.insert(name, value);
        }
    }
    vars
}

/// The playlist with every `{$name}` it defines spelled out and its
/// `EXT-X-DEFINE` lines gone: what it defined is in every line that
/// used it, so nothing downstream needs the definitions. A reference
/// to nothing defined is left as written.
#[must_use]
pub fn substitute(body: &[u8], url: &Url, imported: &Variables) -> Vec<u8> {
    let text = String::from_utf8_lossy(body);
    if !text.contains(DEFINE) {
        return body.to_vec();
    }
    let vars = defined(&text, url, imported);
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        if line.starts_with(DEFINE) {
            continue;
        }
        let mut line = line.to_string();
        for (name, value) in &vars {
            line = line.replace(&format!("{{${name}}}"), value);
        }
        out.push_str(&line);
    }
    out.into_bytes()
}

/// An attribute list's values by name, quoted values unquoted.
fn attributes(list: &str) -> HashMap<String, String> {
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
