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

#[path = "m3u8_define_parse.rs"]
mod parse;
use parse::{attributes, percent_decode};
#[cfg(test)]
#[path = "m3u8_define_prop_test.rs"]
mod prop_tests;
#[path = "m3u8_define_spell.rs"]
mod spell;
use spell::{spell, spell_attributes};

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
/// is there to take. A query parameter's value is percent-decoded, as
/// the spec has it, and not form-decoded: a `+` stays a `+`. One with
/// no value, or whose decoded value a quoted string cannot carry,
/// defines nothing.
fn definition(list: &str, url: &Url, imported: &Variables) -> Option<(String, String)> {
    let attrs = attributes(list);
    if let (Some(name), Some(value)) = (attrs.get("NAME"), attrs.get("VALUE")) {
        return Some((name.clone(), value.clone()));
    }
    if let Some(param) = attrs.get("QUERYPARAM") {
        return url
            .query()?
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .find(|(k, _)| k == param)
            .and_then(|(_, v)| percent_decode(v))
            .map(|v| (param.clone(), v));
    }
    let name = attrs.get("IMPORT")?;
    imported.get(name).map(|v| (name.clone(), v.clone()))
}

/// The playlist with every `{$name}` it defines spelled out — in URI
/// lines, quoted-string values and hexadecimal-sequence values, the
/// three places the spec allows references, each spelled once and its
/// value never read again, and only after the definition it names —
/// and the `EXT-X-DEFINE` lines that defined something gone: what they
/// defined is in every line that used it, so nothing downstream needs
/// them. A definition that defined nothing stays, as does a reference
/// to nothing defined or anywhere else: a segment's title, a comment,
/// an enumerated or decimal value.
#[must_use]
pub fn substitute(body: &[u8], url: &Url, imported: &Variables) -> Vec<u8> {
    let text = String::from_utf8_lossy(body);
    if !text.contains(DEFINE) {
        return body.to_vec();
    }
    let mut vars = Variables::new();
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        if let Some(list) = line.strip_prefix(DEFINE) {
            match definition(list.trim_end(), url, imported) {
                Some((name, value)) => {
                    vars.insert(name, value);
                }
                None => out.push_str(line),
            }
        } else if line.starts_with("#EXTINF:") || !line.starts_with("#EXT") {
            out.push_str(&if line.starts_with('#') {
                line.to_owned()
            } else {
                spell(line, &vars)
            });
        } else {
            out.push_str(&spell_attributes(line, &vars));
        }
    }
    out.into_bytes()
}
