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
    body.lines()
        .filter_map(|line| line.trim_end().strip_prefix(DEFINE))
        .filter_map(|list| definition(list, url, imported))
        .collect()
}

/// The variable one `EXT-X-DEFINE` attribute list defines, if its value
/// is there to take. A query parameter's value is taken as the URL
/// carries it, encoded: decoded, it could split the query it lands in.
fn definition(list: &str, url: &Url, imported: &Variables) -> Option<(String, String)> {
    let attrs = attributes(list);
    if let (Some(name), Some(value)) = (attrs.get("NAME"), attrs.get("VALUE")) {
        return Some((name.clone(), value.clone()));
    }
    if let Some(param) = attrs.get("QUERYPARAM") {
        return url
            .query()?
            .split('&')
            .filter_map(|pair| pair.split_once('=').or(Some((pair, ""))))
            .find(|(k, _)| k == param)
            .map(|(_, v)| (param.clone(), v.to_string()));
    }
    let name = attrs.get("IMPORT")?;
    imported.get(name).map(|v| (name.clone(), v.clone()))
}

/// The playlist with every `{$name}` it defines spelled out — in URI
/// lines and in quoted attribute values, where the spec allows
/// references, each spelled once and its value never read again — and
/// the `EXT-X-DEFINE` lines that defined something gone: what they
/// defined is in every line that used it, so nothing downstream needs
/// them. A definition that defined nothing stays, as does a reference
/// to nothing defined.
#[must_use]
pub fn substitute(body: &[u8], url: &Url, imported: &Variables) -> Vec<u8> {
    let text = String::from_utf8_lossy(body);
    if !text.contains(DEFINE) {
        return body.to_vec();
    }
    let vars = defined(&text, url, imported);
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        if let Some(list) = line.strip_prefix(DEFINE) {
            if definition(list.trim_end(), url, imported).is_none() {
                out.push_str(line);
            }
        } else if line.starts_with('#') {
            out.push_str(&spell_quoted(line, &vars));
        } else {
            out.push_str(&spell(line, &vars));
        }
    }
    out.into_bytes()
}

/// `text` with each `{$name}` reference to a variable in `vars`
/// replaced by its value, in one pass: an inserted value is not read
/// for references.
fn spell(text: &str, vars: &Variables) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("{$") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        match after.find('}') {
            Some(end) if vars.contains_key(&after[..end]) => {
                out.push_str(&vars[&after[..end]]);
                rest = &after[end + 1..];
            }
            _ => {
                out.push_str("{$");
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// A tag line with references spelled out inside its quoted values
/// only.
fn spell_quoted(line: &str, vars: &Variables) -> String {
    let mut out = String::with_capacity(line.len());
    for (i, part) in line.split('"').enumerate() {
        if i > 0 {
            out.push('"');
        }
        if i % 2 == 1 {
            out.push_str(&spell(part, vars));
        } else {
            out.push_str(part);
        }
    }
    out
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
